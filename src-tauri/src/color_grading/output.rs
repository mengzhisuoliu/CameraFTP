// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Generate the output path for a color-graded JPEG.
///
/// Convention: `<input_parent>/ColorGrading/<stem>_<preset_id>_<timestamp>.jpg`
/// Also ensures the `ColorGrading/` subdirectory exists (creates it if needed).
pub fn color_grading_output_path(input_path: &Path, preset_id: &str) -> Result<PathBuf, AppError> {
    let parent = input_path
        .parent()
        .ok_or_else(|| AppError::ColorGradingError("No parent directory".into()))?;
    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_dir = parent.join("ColorGrading");
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create output dir: {}", e)))?;
    let output_name = format!("{}_{}_{}.jpg", stem, preset_id, timestamp);
    Ok(output_dir.join(output_name))
}
