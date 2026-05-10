// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

/// Extract the embedded JPEG preview from a RAW file by binary scanning
/// for the largest complete JPEG segment (SOI...EOI).
///
/// Camera RAW files always contain an embedded JPEG preview.
/// The largest JPEG segment is the full-size preview image.
/// This extracts the raw JPEG bytes directly — zero quality loss, no re-encoding.
pub fn extract_preview_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let path_str = path.to_string_lossy();
    tracing::debug!("Extracting RAW preview from: {}", path_str);

    let data =
        std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path_str, e))?;

    let jpeg = find_largest_jpeg(&data)
        .ok_or_else(|| format!("No embedded JPEG found in {}", path_str))?;

    tracing::debug!(
        "Extracted embedded JPEG: {} bytes from {}",
        jpeg.len(),
        path_str
    );

    Ok(jpeg)
}

/// Scan binary data for the largest complete JPEG segment.
/// JPEG segments are delimited by SOI (0xFF 0xD8) and EOI (0xFF 0xD9) markers.
fn find_largest_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut best_start = 0;
    let mut best_size = 0;

    let len = data.len();
    if len < 4 {
        return None;
    }

    let mut i = 0;
    while i < len - 1 {
        // Look for JPEG SOI marker
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            let start = i;
            // Find the corresponding EOI marker
            let mut j = i + 2;
            while j < len - 1 {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    let size = j + 2 - start;
                    if size > best_size {
                        best_start = start;
                        best_size = size;
                    }
                    break;
                }
                j += 1;
            }
            // Continue scanning from after this SOI
            i += 2;
        } else {
            i += 1;
        }
    }

    if best_size > 0 {
        Some(data[best_start..best_start + best_size].to_vec())
    } else {
        None
    }
}
