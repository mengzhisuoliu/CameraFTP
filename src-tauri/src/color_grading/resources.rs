// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Color grading resource management.
//!
//! Delegates Lensfun DB extraction to the `lensfun_db` module which embeds
//! XML files at compile time and extracts them at runtime.

use std::path::PathBuf;

use crate::error::AppError;

pub fn ensure_resources(
    app_data_dir: &std::path::Path,
) -> Result<(), AppError> {
    super::lensfun_db::ensure_db(app_data_dir)
}

pub fn get_lensfun_db_dir() -> Result<PathBuf, AppError> {
    let db = super::lensfun_db::get_db()?;
    Ok(db.db_dir.clone())
}
