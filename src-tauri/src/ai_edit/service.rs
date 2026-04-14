// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tauri::{AppHandle, Manager};
use tracing::{info, warn, debug};

use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use super::image_processor;
use super::providers;

const MANUAL_QUEUE_CAPACITY: usize = 4;
const AUTO_QUEUE_CAPACITY: usize = 32;
const AIEDIT_SUBDIR: &str = "AIEdit";

/// Default prompt when user leaves the prompt field empty
const DEFAULT_EDIT_PROMPT: &str = "提升画质，使照片更清晰";

struct AiEditTask {
    file_path: PathBuf,
    is_auto_trigger: bool,
    result_tx: Option<oneshot::Sender<Result<PathBuf, AppError>>>,
}

pub struct AiEditService {
    manual_sender: mpsc::Sender<AiEditTask>,
    auto_sender: mpsc::Sender<AiEditTask>,
}

impl AiEditService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (manual_sender, manual_receiver) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);
        let (auto_sender, auto_receiver) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let config_service_clone = config_service.clone();

        tauri::async_runtime::spawn(async move {
            worker_loop(manual_receiver, auto_receiver, app_handle, config_service_clone).await;
        });

        Self { manual_sender, auto_sender }
    }

    /// Auto-trigger: non-blocking enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        if let Err(e) = self.auto_sender.try_send(AiEditTask {
            file_path,
            is_auto_trigger: true,
            result_tx: None,
        }) {
            warn!("AI edit queue full, dropping task: {}", e);
        }
    }

    /// Manual trigger: enqueue and wait for result.
    pub async fn edit_single(&self, file_path: PathBuf) -> Result<PathBuf, AppError> {
        let (tx, rx) = oneshot::channel();

        self.manual_sender
            .send(AiEditTask {
                file_path,
                is_auto_trigger: false,
                result_tx: Some(tx),
            })
            .await
            .map_err(|_| AppError::AiEditError("AI edit service shut down".to_string()))?;

        rx.await
            .map_err(|_| AppError::AiEditError("AI edit worker dropped the task".to_string()))?
    }
}

async fn worker_loop(
    mut manual_rx: mpsc::Receiver<AiEditTask>,
    mut auto_rx: mpsc::Receiver<AiEditTask>,
    app_handle: AppHandle,
    config_service: Arc<ConfigService>,
) {
    info!("AI edit worker started");

    let mut cached_provider: Option<Box<dyn providers::AiEditProvider>> = None;
    let mut cached_api_key: Option<String> = None;

    loop {
        // Fast path: drain pending manual tasks first (high priority)
        let task = if let Ok(task) = manual_rx.try_recv() {
            task
        } else {
            // Slow path: wait on either channel, manual has priority via biased select
            tokio::select! {
                biased;

                task = manual_rx.recv() => {
                    match task {
                        Some(t) => t,
                        None => break,
                    }
                }
                task = auto_rx.recv() => {
                    match task {
                        Some(t) => t,
                        None => {
                            // Auto channel closed; wait for manual tasks or shutdown
                            match manual_rx.recv().await {
                                Some(t) => t,
                                None => break,
                            }
                        }
                    }
                }
            }
        };

        let result = process_task(&task, &config_service, &mut cached_provider, &mut cached_api_key).await;

        match result {
            Ok(ref output_path) => {
                info!(input = %task.file_path.display(), output = %output_path.display(), "AI edit completed");

                if let Some(file_index) = app_handle.try_state::<Arc<FileIndexService>>() {
                    if let Err(e) = file_index.add_file(output_path.clone()).await {
                        debug!(path = %output_path.display(), error = %e, "Failed to index AI-edited file");
                    }
                }
            }
            Err(ref e) => {
                if task.result_tx.is_some() {
                    warn!(input = %task.file_path.display(), error = %e, "AI edit failed");
                } else {
                    debug!(input = %task.file_path.display(), error = %e, "Auto AI edit failed");
                }
            }
        }

        if let Some(tx) = task.result_tx {
            let _ = tx.send(result);
        }
    }

    info!("AI edit worker stopped");
}

async fn process_task(
    task: &AiEditTask,
    config_service: &ConfigService,
    cached_provider: &mut Option<Box<dyn providers::AiEditProvider>>,
    cached_api_key: &mut Option<String>,
) -> Result<PathBuf, AppError> {
    let config = config_service.get()
        .map_err(|e| AppError::AiEditError(format!("Failed to read config: {}", e)))?;

    let ai_config = &config.ai_edit;

    if !ai_config.enabled {
        return Err(AppError::AiEditError("AI edit is disabled".to_string()));
    }

    // Auto-triggered tasks require auto_edit to be enabled
    if task.is_auto_trigger && !ai_config.auto_edit {
        debug!(file = %task.file_path.display(), "Auto-edit disabled, skipping");
        return Err(AppError::AiEditError("Auto-edit is disabled".to_string()));
    }

    let super::config::ProviderConfig::SeedEdit(ref seed_config) = ai_config.provider;
    if seed_config.api_key.is_empty() {
        return Err(AppError::AiEditError("API Key is not configured".to_string()));
    }

    let file_path_clone = task.file_path.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        image_processor::prepare_for_upload(&file_path_clone)
    }).await
        .map_err(|e| AppError::AiEditError(format!("Preprocessing task panicked: {}", e)))?
        .map_err(|e| AppError::AiEditError(format!("Image preprocessing failed: {}", e)))?;

    let current_api_key = match &ai_config.provider {
        super::config::ProviderConfig::SeedEdit(cfg) => cfg.api_key.clone(),
    };

    if cached_api_key.as_ref() != Some(&current_api_key) {
        *cached_provider = Some(providers::create_provider(&ai_config.provider)?);
        *cached_api_key = Some(current_api_key);
    }

    let provider = cached_provider.as_ref()
        .ok_or_else(|| AppError::AiEditError("No provider available".to_string()))?;

    let prompt = if ai_config.prompt.is_empty() {
        DEFAULT_EDIT_PROMPT
    } else {
        &ai_config.prompt
    };
    let image_bytes = provider.edit_image(&prepared.base64_data, prepared.mime_type, prompt).await?;

    let output_dir = config.save_path.join(AIEDIT_SUBDIR);
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::AiEditError(format!("Failed to create AIEdit directory: {}", e)))?;

    let stem = task.file_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let datetime = chrono_now_string();

    let output_path = write_edited_image(&output_dir, stem, &datetime, &image_bytes).await?;

    Ok(output_path)
}

async fn write_edited_image(
    output_dir: &Path,
    stem: &str,
    datetime: &str,
    image_bytes: &[u8],
) -> Result<PathBuf, AppError> {
    let primary_name = format!("{}_AIEdit_{}.jpg", stem, datetime);
    let primary_path = output_dir.join(&primary_name);

    if try_write_exclusive(&primary_path, image_bytes).await.is_ok() {
        return Ok(primary_path);
    }

    for i in 1u32..=99 {
        let retry_name = format!("{}_AIEdit_{}_{}.jpg", stem, datetime, i);
        let retry_path = output_dir.join(&retry_name);
        if try_write_exclusive(&retry_path, image_bytes).await.is_ok() {
            return Ok(retry_path);
        }
    }

    Err(AppError::AiEditError(
        "Failed to write edited image: too many file name collisions".to_string(),
    ))
}

async fn try_write_exclusive(path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await?;
    file.write_all(data).await
}

/// Generates a timestamp string for output filenames.
/// Uses UTC to avoid chrono::Local panics on some Android devices where timezone data is unavailable.
fn chrono_now_string() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%.3f").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn auto_trigger_sends_to_auto_channel() {
        let (auto_sender, mut auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (_manual_sender, _manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        let task_path = PathBuf::from("/photos/test.jpg");
        auto_sender.try_send(AiEditTask {
            file_path: task_path.clone(),
            is_auto_trigger: true,
            result_tx: None,
        }).unwrap();

        let task = auto_rx.try_recv().expect("task should be in auto channel");
        assert_eq!(task.file_path, task_path);
        assert!(task.is_auto_trigger);
        assert!(task.result_tx.is_none());
    }

    #[tokio::test]
    async fn manual_trigger_sends_to_manual_channel() {
        let (_auto_sender, _auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (manual_sender, mut manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        let (tx, _rx) = oneshot::channel();
        manual_sender.try_send(AiEditTask {
            file_path: PathBuf::from("/photos/test.jpg"),
            is_auto_trigger: false,
            result_tx: Some(tx),
        }).unwrap();

        let task = manual_rx.try_recv().expect("task should be in manual channel");
        assert!(!task.is_auto_trigger);
        assert!(task.result_tx.is_some());
    }

    #[tokio::test]
    async fn auto_queue_full_drops_gracefully() {
        let (auto_sender, _auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (_manual_sender, _manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        for i in 0..AUTO_QUEUE_CAPACITY {
            auto_sender.try_send(AiEditTask {
                file_path: PathBuf::from(format!("/photos/img_{i}.jpg")),
                is_auto_trigger: true,
                result_tx: None,
            }).unwrap();
        }

        let result = auto_sender.try_send(AiEditTask {
            file_path: PathBuf::from("/photos/overflow.jpg"),
            is_auto_trigger: true,
            result_tx: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn constants_are_reasonable() {
        assert_eq!(AUTO_QUEUE_CAPACITY, 32);
        assert_eq!(MANUAL_QUEUE_CAPACITY, 4);
        assert_eq!(AIEDIT_SUBDIR, "AIEdit");
    }

    #[test]
    fn chrono_now_string_includes_milliseconds() {
        let s = chrono_now_string();
        assert!(s.contains('.'), "Expected millisecond separator: got {}", s);
        let parts: Vec<&str> = s.split('.').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 3, "Expected 3-digit milliseconds");
    }

    #[test]
    fn default_edit_prompt_is_chinese() {
        assert!(!DEFAULT_EDIT_PROMPT.is_empty());
        assert!(DEFAULT_EDIT_PROMPT.contains("画质"));
    }
}
