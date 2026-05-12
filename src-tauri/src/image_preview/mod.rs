// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod extract;

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::image_utils::is_raw_file;

const MAX_CACHE_ENTRIES: usize = 50;

pub fn content_type_for(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "heif" | "hif" | "heic" => "image/heic",
        _ => "application/octet-stream",
    }
}

struct CacheInner {
    data: HashMap<String, Arc<Vec<u8>>>,
    order: VecDeque<String>,
}

pub struct ImagePreviewCache {
    inner: RwLock<CacheInner>,
}

impl ImagePreviewCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(CacheInner {
                data: HashMap::new(),
                order: VecDeque::new(),
            }),
        }
    }

    pub fn get_or_load(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        {
            let inner = self.inner.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = inner.data.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        let bytes = if is_raw_file(path) {
            Arc::new(extract::extract_preview_jpeg(path)?)
        } else {
            Arc::new(std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?)
        };

        {
            let mut inner = self.inner.write().map_err(|e| e.to_string())?;

            if let Some(existing) = inner.data.get(&key) {
                return Ok(Arc::clone(existing));
            }

            inner.data.insert(key.clone(), Arc::clone(&bytes));
            inner.order.push_back(key);

            while inner.order.len() > MAX_CACHE_ENTRIES {
                let old_key = inner.order.pop_front().unwrap();
                inner.data.remove(&old_key);
            }
        }

        Ok(bytes)
    }

    pub fn invalidate(&self, path: &Path) {
        let key = path.to_string_lossy().to_string();
        let mut inner = self.inner.write().unwrap();
        inner.data.remove(&key);
        inner.order.retain(|k| k != &key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;

    #[test]
    fn content_type_for_jpeg_extensions() {
        assert_eq!(content_type_for(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.jpeg")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.JPG")), "image/jpeg");
    }

    #[test]
    fn content_type_for_heif_extensions() {
        assert_eq!(content_type_for(Path::new("photo.heif")), "image/heic");
        assert_eq!(content_type_for(Path::new("photo.hif")), "image/heic");
        assert_eq!(content_type_for(Path::new("photo.heic")), "image/heic");
    }

    #[test]
    fn content_type_for_unknown_defaults_to_octet_stream() {
        assert_eq!(content_type_for(Path::new("photo.nef")), "application/octet-stream");
        assert_eq!(content_type_for(Path::new("photo.cr2")), "application/octet-stream");
        assert_eq!(content_type_for(Path::new("photo.png")), "application/octet-stream");
        assert_eq!(content_type_for(Path::new("photo")), "application/octet-stream");
    }

    #[test]
    fn cache_returns_same_instance_for_same_path() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_instance");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.jpg");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00]).unwrap();

        let cache = ImagePreviewCache::new();
        let result1 = cache.get_or_load(&file_path).unwrap();
        let result2 = cache.get_or_load(&file_path).unwrap();
        assert!(Arc::ptr_eq(&result1, &result2));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_evicts_old_entries() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_eviction");
        std::fs::create_dir_all(&dir).unwrap();

        let cache = ImagePreviewCache::new();

        for i in 0..60 {
            let file_path = dir.join(format!("test_{}.jpg", i));
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(&[0xFF, 0xD8, 0x00, 0x00]).unwrap();
            cache.get_or_load(&file_path).unwrap();
        }

        let cache_size = cache.inner.read().unwrap().data.len();
        assert!(
            cache_size <= MAX_CACHE_ENTRIES,
            "Cache should evict, size={}",
            cache_size
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
