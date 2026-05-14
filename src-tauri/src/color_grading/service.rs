// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tauri::{AppHandle, Emitter};

use crate::config::AutoColorGradingConfig;
use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::image_utils;
use crate::utils::batch_state::BatchState;
use super::progress::ColorGradingEvent;
use super::presets::find_preset;

struct ColorGradingTask {
    input_path: PathBuf,
    lut_id: String,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
}

pub struct ColorGradingService {
    config_service: Arc<ConfigService>,
    app_handle: AppHandle,
    sender: mpsc::Sender<ColorGradingTask>,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<std::sync::Mutex<CancellationToken>>,
}

impl ColorGradingService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (sender, receiver) = mpsc::channel::<ColorGradingTask>(16);
        let queue_depth = Arc::new(AtomicU32::new(0));
        let cancel_token = Arc::new(std::sync::Mutex::new(CancellationToken::new()));

        let app_handle_clone = app_handle.clone();
        let queue_depth_clone = queue_depth.clone();
        let cancel_token_clone = cancel_token.clone();

        tauri::async_runtime::spawn(async move {
            worker_loop(receiver, app_handle_clone, queue_depth_clone, cancel_token_clone).await;
        });

        Self { config_service, app_handle, sender, queue_depth, cancel_token }
    }

    pub async fn enqueue(&self, file_paths: Vec<PathBuf>, lut_id: String, use_auto_exposure: bool, metering_mode: String, manual_ev: f32) -> Result<(), AppError> {
        let preset = find_preset(&lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;

        let total = file_paths.len() as u32;
        if self.sender.is_closed() {
            return Err(AppError::ColorGradingError("Color grading queue is closed".to_string()));
        }
        self.queue_depth.fetch_add(total, Ordering::Relaxed);

        let mut sent = 0u32;
        for path in file_paths {
            match self.sender.send(ColorGradingTask {
                input_path: path,
                lut_id: preset.id.clone(),
                use_auto_exposure,
                metering_mode: metering_mode.clone(),
                manual_ev,
            }).await {
                Ok(()) => sent += 1,
                Err(_) => {
                    self.queue_depth.fetch_sub(total - sent, Ordering::Relaxed);
                    return Err(AppError::ColorGradingError("Failed to enqueue task".to_string()));
                }
            }
        }

        let depth = self.queue_depth.load(Ordering::Relaxed);
        let _ = self.app_handle.emit("color-grading-progress", &ColorGradingEvent::Queued { queue_depth: depth });

        Ok(())
    }

    pub fn cancel(&self) {
        let mut guard = self.cancel_token.lock().unwrap_or_else(|e| e.into_inner());
        guard.cancel();
        *guard = CancellationToken::new();
    }

    /// Auto-trigger: check config + RAW extension, then enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        let config = self.config_service.get().ok();
        let auto_cg = config.as_ref()
            .and_then(|c| c.auto_color_grading.as_ref());

        if !should_auto_color_grade(auto_cg, &file_path) {
            return;
        }

        let Some(cg) = auto_cg else { return };
        if let Err(e) = self.enqueue(
            vec![file_path.clone()],
            cg.preset_id.clone(),
            cg.use_auto_exposure,
            cg.metering_mode.clone(),
            cg.manual_ev,
        ).await {
            tracing::warn!("Auto color grading enqueue failed for {}: {}", file_path.display(), e);
        }
    }
}

/// Pure predicate: should auto color grading trigger for this file + config?
pub(crate) fn should_auto_color_grade(
    config: Option<&AutoColorGradingConfig>,
    file_path: &std::path::Path,
) -> bool {
    let cg = match config {
        Some(cg) if cg.enabled && !cg.preset_id.is_empty() => cg,
        _ => return false,
    };
    if find_preset(&cg.preset_id).is_none() {
        return false;
    }
    image_utils::is_raw_file(file_path)
}

async fn worker_loop(
    mut receiver: mpsc::Receiver<ColorGradingTask>,
    app_handle: AppHandle,
    queue_depth: Arc<AtomicU32>,
    cancel_token_arc: Arc<std::sync::Mutex<CancellationToken>>,
) {
    tracing::info!("Color grading worker started");

    let mut state = BatchState::default();

    fn emit_done(
        state: &mut BatchState,
        app_handle: &AppHandle,
        cancelled: bool,
    ) {
        let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Done {
            total: state.processed_count(),
            failed_count: state.failed_count,
            failed_files: std::mem::take(&mut state.failed_files),
            output_files: std::mem::take(&mut state.output_files),
            cancelled,
        });
        state.reset();
    }

    fn drain_pending_tasks(
        receiver: &mut mpsc::Receiver<ColorGradingTask>,
        queue_depth: &AtomicU32,
    ) {
        while let Ok(_) = receiver.try_recv() {
            queue_depth.fetch_sub(1, Ordering::Relaxed);
        }
    }

    loop {
        let cancel_token = cancel_token_arc.lock().unwrap_or_else(|e| e.into_inner()).clone();

        if cancel_token.is_cancelled() {
            drain_pending_tasks(&mut receiver, &queue_depth);
            if state.processed_count() > 0 {
                emit_done(&mut state, &app_handle, true);
            }
            continue;
        }

        let task = tokio::select! {
            t = receiver.recv() => match t {
                Some(t) => t,
                None => {
                    drain_pending_tasks(&mut receiver, &queue_depth);
                    if state.processed_count() > 0 {
                        emit_done(&mut state, &app_handle, true);
                    }
                    break;
                }
            },
            _ = cancel_token.cancelled() => {
                drain_pending_tasks(&mut receiver, &queue_depth);
                if state.processed_count() > 0 {
                    emit_done(&mut state, &app_handle, true);
                }
                continue;
            }
        };

        queue_depth.fetch_sub(1, Ordering::Relaxed);

        let remaining = queue_depth.load(Ordering::Relaxed);
        let current = state.processed_count() + 1;
        let total = current + remaining;
        let file_name = task.input_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Progress {
            current,
            total,
            file_name: file_name.clone(),
            failed_count: state.failed_count,
        });

        let result = tokio::select! {
            r = process_single_file(&task) => Some(r),
            _ = cancel_token.cancelled() => {
                tracing::info!("Color grading cancelled before/during task processing");
                None
            }
        };

        match result {
            Some(Ok(output_path)) => {
                tracing::info!(input = %task.input_path.display(), output = %output_path, "Color grading completed");
                state.completed_count += 1;
                state.output_files.push(output_path.clone());

                let remaining = queue_depth.load(Ordering::Relaxed);
                let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Completed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    failed_count: state.failed_count,
                    output_path,
                });
            }
            Some(Err(ref e)) => {
                tracing::error!(input = %task.input_path.display(), error = %e, "Color grading failed");
                state.failed_count += 1;
                state.failed_files.push(file_name.clone());

                let remaining = queue_depth.load(Ordering::Relaxed);
                let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Failed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    error: e.to_string(),
                    failed_count: state.failed_count,
                });
            }
            None => {
                drain_pending_tasks(&mut receiver, &queue_depth);
                emit_done(&mut state, &app_handle, true);
                continue;
            }
        }

        if queue_depth.load(Ordering::Relaxed) == 0 && state.processed_count() > 0 {
            emit_done(&mut state, &app_handle, false);
        }
    }

    tracing::info!("Color grading worker stopped");
}

async fn process_single_file(task: &ColorGradingTask) -> Result<String, AppError> {
    let preset = find_preset(&task.lut_id)
        .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT: {}", task.lut_id)))?;

    let parent = task.input_path.parent()
        .ok_or_else(|| AppError::ColorGradingError("No parent directory".into()))?;
    let stem = task.input_path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let output_dir = parent.join("ColorGrading");
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create output dir: {}", e)))?;
    let output_name = format!("{}_{}_{}.jpg", stem, preset.id, timestamp);
    let output_path = output_dir.join(&output_name);

    let lut_data = super::lut_data::get_lut_data(&preset.id)?;
    let lib = super::ffi::RawAlchemyLib::get()?;

    let lensfun_path = super::resources::get_lensfun_db_dir()
        .ok()
        .map(|dir| dir.to_string_lossy().into_owned());

    let input_path = task.input_path.clone();
    let log_space = preset.log_space.clone();
    let metering_mode = task.metering_mode.clone();
    let use_auto_exposure = task.use_auto_exposure;
    let manual_ev = task.manual_ev;
    let stem_clone = stem.clone();
    let preset_id = preset.id.clone();

    let result_path = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        // Resolve collision inside blocking context — avoids blocking async thread
        let mut final_output_path = output_path;
        if final_output_path.exists() {
            let mut found = false;
            for seq in 1..100u32 {
                let candidate = output_dir.join(format!("{}_{}_{}_{}.jpg", stem_clone, preset_id, timestamp, seq));
                if !candidate.exists() {
                    final_output_path = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(AppError::ColorGradingError(
                    "Output path collision: could not find unique filename after 100 attempts".into(),
                ));
            }
        }
        let result = final_output_path.to_string_lossy().into_owned();

        lib.process_file_with_lut(
            &input_path,
            &final_output_path,
            Some(&log_space),
            &lut_data,
            lensfun_path.as_deref(),
            use_auto_exposure,
            &metering_mode,
            manual_ev,
        )?;

        Ok(result)
    }).await.map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

    Ok(result_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn enabled_cg() -> AutoColorGradingConfig {
        AutoColorGradingConfig { enabled: true, ..Default::default() }
    }

    #[test]
    fn should_auto_color_grade_enabled_raw_file() {
        assert!(should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.nef")));
        assert!(should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.CR3")));
    }

    #[test]
    fn should_auto_color_grade_disabled_even_for_raw() {
        let disabled = AutoColorGradingConfig { enabled: false, ..Default::default() };
        assert!(!should_auto_color_grade(Some(&disabled), Path::new("photo.nef")));
    }

    #[test]
    fn should_auto_color_grade_non_raw_even_if_enabled() {
        assert!(!should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.jpg")));
        assert!(!should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.mp4")));
    }

    #[test]
    fn should_auto_color_grade_requires_nonempty_preset() {
        let empty_preset = AutoColorGradingConfig { enabled: true, preset_id: String::new(), ..Default::default() };
        assert!(!should_auto_color_grade(Some(&empty_preset), Path::new("photo.nef")));
    }

    #[test]
    fn should_auto_color_grade_returns_false_when_no_config() {
        assert!(!should_auto_color_grade(None, Path::new("photo.nef")));
    }

    #[test]
    fn should_auto_color_grade_rejects_invalid_preset_id() {
        let bad_preset = AutoColorGradingConfig {
            enabled: true,
            preset_id: "nonexistent-preset-id".to_string(),
            ..Default::default()
        };
        assert!(!should_auto_color_grade(Some(&bad_preset), Path::new("photo.nef")));
    }

    #[test]
    fn cancel_replaces_with_fresh_token() {
        let token = Arc::new(std::sync::Mutex::new(CancellationToken::new()));
        // Cancel and replace (same logic as ColorGradingService::cancel)
        {
            let mut guard = token.lock().unwrap();
            guard.cancel();
            *guard = CancellationToken::new();
        }
        let fresh = token.lock().unwrap().clone();
        assert!(!fresh.is_cancelled(), "token should be fresh (not cancelled) after cancel()");
    }
}

