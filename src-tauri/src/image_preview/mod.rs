// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod extract;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

const RAW_EXTENSIONS: &[&str] = &[
    "nef", "nrw", "cr2", "cr3", "arw", "sr2",
    "raf", "orf", "rw2", "pef", "dng", "x3f", "raw", "srw",
];

/// Check if a file path has a RAW image extension.
pub fn is_raw_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Get MIME type based on file extension.
pub fn content_type_for(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "heif" | "hif" | "heic" => "image/heic",
        _ => "image/jpeg",
    }
}

/// In-memory cache for image preview bytes.
pub struct ImagePreviewCache {
    cache: RwLock<HashMap<String, Arc<Vec<u8>>>>,
}

impl ImagePreviewCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get cached image bytes, or extract/read from file and cache the result.
    pub fn get_or_load(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        // Fast path: check cache with read lock
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = cache.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        // Slow path: load/extract
        let bytes = if is_raw_file(path) {
            Arc::new(extract::extract_preview_jpeg(path)?)
        } else {
            Arc::new(std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?)
        };

        // Store in cache
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            cache.insert(key, Arc::clone(&bytes));
        }

        Ok(bytes)
    }
}
