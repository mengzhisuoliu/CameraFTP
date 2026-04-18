// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: PathBuf,
    pub filename: String,
    #[ts(skip)]
    pub exif_time: Option<SystemTime>,
    #[ts(skip)]
    pub modified_time: SystemTime,
    pub sort_time: u64, // 时间戳（毫秒）用于TypeScript
}

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FileIndex {
    pub(crate) files: Arc<Vec<FileInfo>>,
    pub current_index: Option<usize>,
    pub(crate) path_set: HashSet<PathBuf>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Vec::new()),
            current_index: None,
            path_set: HashSet::new(),
        }
    }

    pub fn files(&self) -> &Arc<Vec<FileInfo>> {
        &self.files
    }

    /// Check if a path is already indexed (O(1) via HashSet)
    pub fn contains_path(&self, path: &PathBuf) -> bool {
        self.path_set.contains(path)
    }

    /// Update files. Callers provide the new complete vector.
    pub fn set_files(&mut self, new_files: Vec<FileInfo>) {
        self.path_set = new_files.iter().map(|f| f.path.clone()).collect();
        self.files = Arc::new(new_files);
    }
}

impl Default for FileIndex {
    fn default() -> Self {
        Self::new()
    }
}
