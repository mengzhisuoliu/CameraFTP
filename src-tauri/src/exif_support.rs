// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;
use std::time::SystemTime;

use nom_exif::{Exif, ExifIter, ExifTag, MediaParser, MediaSource};

#[derive(Debug)]
pub enum ParseExifError {
    /// Failed to open/build media source from the provided path.
    Source(String),
    /// Media source looked EXIF-capable but parsing EXIF payload failed.
    Parse(String),
}

/// Parse EXIF metadata from a file path.
///
/// Returns:
/// - `Ok(Some(exif))` when EXIF is present and parsed successfully
/// - `Ok(None)` when file has no EXIF data
/// - `Err(ParseExifError::Source(_))` when file source creation fails
/// - `Err(ParseExifError::Parse(_))` when EXIF parsing fails
pub fn parse_exif(path: &Path) -> Result<Option<Exif>, ParseExifError> {
    let mut parser = MediaParser::new();
    let media_source = MediaSource::file_path(path)
        .map_err(|error| ParseExifError::Source(format!("{error:?}")))?;

    if !media_source.has_exif() {
        return Ok(None);
    }

    let iter: ExifIter = parser
        .parse(media_source)
        .map_err(|error| ParseExifError::Parse(format!("{error:?}")))?;

    Ok(Some(iter.into()))
}

pub fn exif_datetime_original_string(exif: &Exif) -> Option<String> {
    exif.get(ExifTag::DateTimeOriginal)
        .and_then(|value| value.as_time_components())
        .map(|(datetime, _offset)| datetime.format("%Y-%m-%d %H:%M:%S").to_string())
}

pub fn exif_datetime_original_system_time(exif: &Exif) -> Option<SystemTime> {
    exif.get(ExifTag::DateTimeOriginal)
        .and_then(|value| value.as_time_components())
        .and_then(|(datetime, _offset)| datetime.and_utc().try_into().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_exif_returns_error_for_missing_file() {
        let result = parse_exif(Path::new("/definitely-not-found/cameraftp-missing.jpg"));
        assert!(matches!(result, Err(ParseExifError::Source(_))));
    }

    #[test]
    fn parse_exif_non_exif_input_is_not_source_error() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(b"plain text, not an image")
            .expect("write test data");

        let result = parse_exif(file.path());
        assert!(
            matches!(result, Ok(None) | Err(ParseExifError::Parse(_))),
            "expected no exif or parse failure (but never source error), got {:?}",
            result.map(|value| value.is_some())
        );
    }
}
