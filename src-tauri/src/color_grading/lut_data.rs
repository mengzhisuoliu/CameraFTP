// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::OnceLock;

use crate::error::AppError;

pub struct LutData {
    pub size: usize,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub table: Vec<f32>,
}

macro_rules! lut_embed {
    ($id:expr, $file:expr) => {
        (
            $id,
            include_bytes!(concat!(env!("OUT_DIR"), "/luts/", $file, ".gz")).as_slice(),
        )
    };
}

static LUT_CACHE: OnceLock<std::collections::HashMap<&'static str, LutData>> = OnceLock::new();

fn compressed_luts() -> Vec<(&'static str, &'static [u8])> {
    vec![
        lut_embed!("arri-alexa-classic-709", "ARRI_ALEXA_Classic-709_VLog.cube"),
        lut_embed!("fujifilm-acros", "Fujifilm_ACROS_VLog.cube"),
        lut_embed!("fujifilm-astia", "Fujifilm_ASTIA_VLog.cube"),
        lut_embed!("fujifilm-classic-chrome", "Fujifilm_CLASSIC-CHROME_VLog.cube"),
        lut_embed!("fujifilm-classic-neg", "Fujifilm_CLASSIC-Neg_VLog.cube"),
        lut_embed!("fujifilm-eterna-3513di", "Fujifilm_ETERNA-3513DI_VLog.cube"),
        lut_embed!("fujifilm-eterna-bb", "Fujifilm_ETERNA-BB_VLog.cube"),
        lut_embed!("fujifilm-eterna", "Fujifilm_ETERNA_VLog.cube"),
        lut_embed!("fujifilm-pro-neg-std", "Fujifilm_PRO-Neg.-Std_VLog.cube"),
        lut_embed!("fujifilm-provia", "Fujifilm_PROVIA_VLog.cube"),
        lut_embed!("fujifilm-reala-ace", "Fujifilm_REALA-ACE_VLog.cube"),
        lut_embed!("fujifilm-velvia", "Fujifilm_Velvia_VLog.cube"),
        lut_embed!("kodak-vision-2383", "Kodak_VISION-2383_VLog.cube"),
        lut_embed!("leica-classic", "Leica_Classic_VLog.cube"),
        lut_embed!("leica-natural", "Leica_Natural_VLog.cube"),
        lut_embed!("red-achromic", "RED_Achromic_VLog.cube"),
        lut_embed!("red-filmbias-bb", "RED_FilmBias-BB_VLog.cube"),
        lut_embed!("red-filmbias-offset", "RED_FilmBias-Offset_VLog.cube"),
        lut_embed!("red-filmbias", "RED_FilmBias_VLog.cube"),
        lut_embed!("red-rec-709", "RED_Rec.709_VLog.cube"),
    ]
}

pub fn get_lut_data(preset_id: &str) -> Result<&'static LutData, AppError> {
    let cache = LUT_CACHE.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for (id, compressed) in compressed_luts() {
            match decompress_and_parse(compressed) {
                Ok(data) => {
                    tracing::info!(
                        "LUT '{}' loaded: {}^3, {} entries",
                        id,
                        data.size,
                        data.table.len() / 3
                    );
                    map.insert(id, data);
                }
                Err(e) => {
                    tracing::error!("Failed to load LUT '{}': {}", id, e);
                }
            }
        }
        map
    });

    cache.get(preset_id).ok_or_else(|| {
        AppError::ColorGradingError(format!("LUT data not available for preset: {}", preset_id))
    })
}

fn decompress_and_parse(compressed: &[u8]) -> Result<LutData, AppError> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(compressed);
    let mut text = String::new();
    decoder
        .read_to_string(&mut text)
        .map_err(|e| AppError::ColorGradingError(format!("LUT decompression failed: {}", e)))?;
    parse_cube_text(&text)
}

fn parse_cube_text(text: &str) -> Result<LutData, AppError> {
    let mut size: usize = 0;
    let mut domain_min = [0.0f32; 3];
    let mut domain_max = [1.0f32; 3];
    let mut table = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("LUT_3D_SIZE") {
            size = rest
                .trim()
                .parse::<usize>()
                .map_err(|e| AppError::ColorGradingError(format!("Invalid LUT_3D_SIZE: {}", e)))?;
            table.reserve(size * size * size * 3);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("DOMAIN_MIN") {
            parse_three_floats(rest, &mut domain_min)?;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("DOMAIN_MAX") {
            parse_three_floats(rest, &mut domain_max)?;
            continue;
        }

        // Skip other header keywords
        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        {
            continue;
        }

        // Data line: "R G B"
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let r: f32 = parts[0].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            let g: f32 = parts[1].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            let b: f32 = parts[2].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            table.push(r);
            table.push(g);
            table.push(b);
        }
    }

    let expected = size * size * size * 3;
    if table.len() != expected {
        return Err(AppError::ColorGradingError(format!(
            "LUT parse error: expected {} floats, got {}",
            expected,
            table.len()
        )));
    }

    Ok(LutData {
        size,
        domain_min,
        domain_max,
        table,
    })
}

fn parse_three_floats(s: &str, out: &mut [f32; 3]) -> Result<(), AppError> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return Err(AppError::ColorGradingError(
            "Expected 3 float values".into(),
        ));
    }
    for i in 0..3 {
        out[i] = parts[i].parse().map_err(|e| {
            AppError::ColorGradingError(format!("Invalid float: {}", e))
        })?;
    }
    Ok(())
}
