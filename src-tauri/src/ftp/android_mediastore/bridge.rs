// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JNI bridge implementation for Android MediaStore operations.
//!
//! This module provides the bridge between Rust and Android's MediaStore API
//! via JNI. On non-Android platforms, a mock implementation is provided for testing.

use super::types::{
    mime_type_from_filename,
    relative_path_from_full_path, FileDescriptorInfo, MediaStoreBridgeClient,
    MediaStoreError, QueryResult,
};
use std::path::PathBuf;
use std::sync::Arc;

/// JNI-based MediaStore bridge for Android.
///
/// This implementation calls into Android's MediaStore API via JNI
/// to perform file operations.
#[cfg(target_os = "android")]
#[derive(Debug)]
pub struct JniMediaStoreBridge {
    // In a real implementation, this would hold a reference to the JNI environment
    // and the Java/Kotlin bridge object.
    _phantom: (),
}

#[cfg(target_os = "android")]
impl JniMediaStoreBridge {
    /// Creates a new JNI MediaStore bridge.
    pub fn new() -> Self {
        Self { _phantom: () }
    }
}

#[cfg(target_os = "android")]
#[async_trait::async_trait]
impl MediaStoreBridgeClient for JniMediaStoreBridge {
    async fn open_fd_for_read(&self, path: &str) -> Result<FileDescriptorInfo, MediaStoreError> {
        debug!(path, "Opening file descriptor for read");
        
        // TODO: Implement JNI call to MediaStoreBridge.openFdForRead(path)
        // This would call a Kotlin method that uses ContentResolver.openFileDescriptor()
        
        Err(MediaStoreError::BridgeError(
            "JNI bridge not yet implemented".to_string(),
        ))
    }

    async fn open_fd_for_write(
        &self,
        display_name: &str,
        mime_type: &str,
        relative_path: &str,
    ) -> Result<FileDescriptorInfo, MediaStoreError> {
        debug!(display_name, mime_type, relative_path, "Opening file descriptor for write");
        
        // TODO: Implement JNI call to MediaStoreBridge.openFdForWrite()
        // This would call a Kotlin method that uses ContentResolver.insert() and openFileDescriptor()
        
        Err(MediaStoreError::BridgeError(
            "JNI bridge not yet implemented".to_string(),
        ))
    }

    async fn query_files(&self, path: &str) -> Result<Vec<QueryResult>, MediaStoreError> {
        debug!(path, "Querying files");
        
        // TODO: Implement JNI call to MediaStoreBridge.queryFiles(path)
        // This would call a Kotlin method that queries MediaStore.Images.Media
        
        Err(MediaStoreError::BridgeError(
            "JNI bridge not yet implemented".to_string(),
        ))
    }

    async fn query_file(&self, path: &str) -> Result<QueryResult, MediaStoreError> {
        debug!(path, "Querying single file");
        
        // TODO: Implement JNI call to MediaStoreBridge.queryFile(path)
        
        Err(MediaStoreError::BridgeError(
            "JNI bridge not yet implemented".to_string(),
        ))
    }

    async fn delete_file(&self, path: &str) -> Result<(), MediaStoreError> {
        debug!(path, "Deleting file");
        
        // TODO: Implement JNI call to MediaStoreBridge.deleteFile(path)
        // This would call a Kotlin method that uses ContentResolver.delete()
        
        Err(MediaStoreError::BridgeError(
            "JNI bridge not yet implemented".to_string(),
        ))
    }

    async fn create_directory(&self, path: &str) -> Result<(), MediaStoreError> {
        debug!(path, "Creating directory");
        
        // MediaStore doesn't have explicit directory creation.
        // Directories are created implicitly when files are added with a relative path.
        // This is a no-op for MediaStore.
        Ok(())
    }
}

/// Mock MediaStore bridge for testing and non-Android platforms.
///
/// This implementation stores files in memory or a local directory
/// for testing purposes.
#[cfg(not(target_os = "android"))]
#[derive(Debug)]
pub struct MockMediaStoreBridge {
    base_path: PathBuf,
}

#[cfg(not(target_os = "android"))]
impl MockMediaStoreBridge {
    /// Creates a new mock bridge that stores files in the given directory.
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Creates a new mock bridge using a temporary directory.
    pub fn temp() -> Self {
        Self {
            base_path: std::env::temp_dir().join("cameraftp_mock_mediastore"),
        }
    }
}

#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl MediaStoreBridgeClient for MockMediaStoreBridge {
    async fn open_fd_for_read(&self, path: &str) -> Result<FileDescriptorInfo, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        
        if !full_path.exists() {
            return Err(MediaStoreError::NotFound(path.to_string()));
        }

        // On non-Unix platforms, we can't return a real file descriptor.
        // This is only used for testing on Windows/Linux development machines.
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let file = std::fs::File::open(&full_path)?;
            let fd = file.as_raw_fd();
            
            Ok(FileDescriptorInfo {
                fd,
                path: full_path,
            })
        }
        
        #[cfg(not(unix))]
        {
            Err(MediaStoreError::BridgeError(
                "File descriptors not supported on this platform".to_string(),
            ))
        }
    }

    async fn open_fd_for_write(
        &self,
        display_name: &str,
        mime_type: &str,
        relative_path: &str,
    ) -> Result<FileDescriptorInfo, MediaStoreError> {
        let dir_path = self.base_path.join(relative_path.trim_start_matches('/'));
        let full_path = dir_path.join(display_name);
        
        // Create parent directories if needed
        tokio::fs::create_dir_all(&dir_path).await
            .map_err(|e| MediaStoreError::IoError(e))?;
        
        let file = std::fs::File::create(&full_path)?;
        
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            
            Ok(FileDescriptorInfo {
                fd,
                path: full_path,
            })
        }
        
        #[cfg(not(unix))]
        {
            drop(file);
            Err(MediaStoreError::BridgeError(
                "File descriptors not supported on this platform".to_string(),
            ))
        }
    }

    async fn query_files(&self, path: &str) -> Result<Vec<QueryResult>, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        
        if !full_path.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut entries = tokio::fs::read_dir(&full_path).await
            .map_err(MediaStoreError::IoError)?;
        
        while let Some(entry) = entries.next_entry().await.map_err(MediaStoreError::IoError)? {
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().await.map_err(MediaStoreError::IoError)?;
            
            let is_dir = metadata.is_dir();
            let mime_type = if is_dir {
                "inode/directory".to_string()
            } else {
                mime_type_from_filename(&name).to_string()
            };
            
            results.push(QueryResult {
                content_uri: format!("content://mock/media/{}", name),
                display_name: name,
                size: metadata.len(),
                date_modified: metadata.modified()
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64)
                    .unwrap_or(0),
                mime_type,
                relative_path: path.trim_start_matches('/').to_string(),
            });
        }

        Ok(results)
    }

    async fn query_file(&self, path: &str) -> Result<QueryResult, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        
        if !full_path.exists() {
            return Err(MediaStoreError::NotFound(path.to_string()));
        }

        let metadata = tokio::fs::metadata(&full_path).await
            .map_err(MediaStoreError::IoError)?;
        let name = full_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        
        let is_dir = metadata.is_dir();
        let mime_type = if is_dir {
            "inode/directory".to_string()
        } else {
            mime_type_from_filename(&name).to_string()
        };

        Ok(QueryResult {
            content_uri: format!("content://mock/media/{}", path.trim_start_matches('/')),
            display_name: name,
            size: metadata.len(),
            date_modified: metadata.modified()
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64)
                .unwrap_or(0),
            mime_type,
            relative_path: relative_path_from_full_path(path),
        })
    }

    async fn delete_file(&self, path: &str) -> Result<(), MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        
        if full_path.is_dir() {
            tokio::fs::remove_dir(&full_path).await
                .map_err(MediaStoreError::IoError)?;
        } else {
            tokio::fs::remove_file(&full_path).await
                .map_err(MediaStoreError::IoError)?;
        }
        
        Ok(())
    }

    async fn create_directory(&self, path: &str) -> Result<(), MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        tokio::fs::create_dir_all(&full_path).await
            .map_err(MediaStoreError::IoError)?;
        Ok(())
    }
}

/// Type alias for the platform-specific bridge.
#[cfg(target_os = "android")]
pub type PlatformBridge = JniMediaStoreBridge;

#[cfg(not(target_os = "android"))]
pub type PlatformBridge = MockMediaStoreBridge;

/// Creates a new MediaStore bridge for the current platform.
#[cfg(target_os = "android")]
pub fn create_bridge() -> Arc<dyn MediaStoreBridgeClient> {
    Arc::new(JniMediaStoreBridge::new())
}

#[cfg(not(target_os = "android"))]
pub fn create_bridge() -> Arc<dyn MediaStoreBridgeClient> {
    Arc::new(MockMediaStoreBridge::temp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(all(not(target_os = "android"), unix))]
    #[tokio::test]
    async fn test_mock_bridge_create_and_query_file() {
        let temp_dir = TempDir::new().unwrap();
        let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
        
        // Create a test file (only works on Unix)
        let fd_info = bridge.open_fd_for_write("test.jpg", "image/jpeg", "DCIM/").await.unwrap();
        assert!(fd_info.path.exists());
        
        // Query the file
        let result = bridge.query_file("DCIM/test.jpg").await.unwrap();
        assert_eq!(result.display_name, "test.jpg");
        assert_eq!(result.mime_type, "image/jpeg");
    }

    #[cfg(all(not(target_os = "android"), unix))]
    #[tokio::test]
    async fn test_mock_bridge_list_files() {
        let temp_dir = TempDir::new().unwrap();
        let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
        
        // Create multiple files (only works on Unix)
        bridge.open_fd_for_write("photo1.jpg", "image/jpeg", "DCIM/").await.unwrap();
        bridge.open_fd_for_write("photo2.png", "image/png", "DCIM/").await.unwrap();
        
        // List files
        let results = bridge.query_files("DCIM/").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[cfg(all(not(target_os = "android"), unix))]
    #[tokio::test]
    async fn test_mock_bridge_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
        
        // Create and delete a file (only works on Unix)
        bridge.open_fd_for_write("test.jpg", "image/jpeg", "DCIM/").await.unwrap();
        bridge.delete_file("DCIM/test.jpg").await.unwrap();
        
        // Verify file is gone
        let result = bridge.query_file("DCIM/test.jpg").await;
        assert!(result.is_err());
    }

    #[cfg(all(not(target_os = "android"), not(unix)))]
    #[tokio::test]
    async fn test_mock_bridge_fd_not_supported() {
        let temp_dir = TempDir::new().unwrap();
        let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
        
        // On non-Unix, file descriptors are not supported
        let result = bridge.open_fd_for_write("test.jpg", "image/jpeg", "DCIM/").await;
        assert!(result.is_err());
    }
}
