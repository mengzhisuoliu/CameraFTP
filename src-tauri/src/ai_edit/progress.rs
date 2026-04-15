// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEditProgressEvent {
    Progress {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Completed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Failed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        error: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Done {
        total: u32,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
        #[serde(rename = "failedFiles")]
        #[ts(rename = "failedFiles")]
        failed_files: Vec<String>,
    },
}
