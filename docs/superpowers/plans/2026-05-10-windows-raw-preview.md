# Windows RAW Image Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable RAW camera file indexing and viewing on Windows by extracting embedded JPEG previews via rawler with in-memory caching.

**Architecture:** A custom Tauri protocol (`raw-preview`) serves JPEG bytes from an in-memory cache. The `rawler` crate extracts embedded previews. PreviewWindow detects RAW extensions and uses the custom protocol URL instead of `convertFileSrc()`.

**Tech Stack:** Rust (rawler 0.7, image 0.25), Tauri v2 custom protocol, React/TypeScript

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/Cargo.toml` | Add rawler dependency (Windows-only) |
| Create | `src-tauri/src/raw_preview/mod.rs` | Cache struct + get_or_extract method |
| Create | `src-tauri/src/raw_preview/extract.rs` | rawler JPEG extraction function |
| Modify | `src-tauri/src/lib.rs` | Register module, managed state, custom protocol |
| Modify | `src-tauri/src/file_index/service.rs:216-223` | Add RAW extensions to is_supported_image() |
| Modify | `src/components/PreviewWindow.tsx:224-226` | RAW detection + custom protocol URL |

---

### Task 1: Add rawler Dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add rawler to Windows-only dependencies**

Add the following to `src-tauri/Cargo.toml`, inside the existing `[target.'cfg(target_os = "windows")'.dependencies]` section (after the `notify` line):

```toml
# RAW file preview extraction (Windows only)
rawler = "0.7"
```

- [ ] **Step 2: Verify dependency resolves**

Run: `cargo.exe check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: Compiles without errors (may take a while first time for rawler build)

---

### Task 2: Create raw_preview Module

**Files:**
- Create: `src-tauri/src/raw_preview/mod.rs`
- Create: `src-tauri/src/raw_preview/extract.rs`

- [ ] **Step 1: Create mod.rs with cache struct**

Create `src-tauri/src/raw_preview/mod.rs`:

```rust
// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod extract;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// In-memory cache for extracted RAW preview JPEG bytes.
/// Key: canonical file path string. Value: JPEG bytes.
pub struct RawPreviewCache {
    cache: RwLock<HashMap<String, Arc<Vec<u8>>>>,
}

impl RawPreviewCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get cached preview JPEG, or extract from RAW file and cache it.
    pub fn get_or_extract(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        // Fast path: check cache with read lock
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = cache.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        // Slow path: extract (no lock held, concurrent extraction is benign)
        let bytes = Arc::new(extract::extract_preview_jpeg(path)?);

        // Store in cache
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            cache.insert(key, Arc::clone(&bytes));
        }

        Ok(bytes)
    }
}
```

- [ ] **Step 2: Create extract.rs with rawler extraction**

Create `src-tauri/src/raw_preview/extract.rs`:

```rust
// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Cursor;
use std::path::Path;

use image::ImageFormat;

/// Extract embedded JPEG preview from a RAW file using rawler.
/// Returns JPEG bytes re-encoded at quality 95.
pub fn extract_preview_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let path_str = path.to_string_lossy();

    tracing::debug!("Extracting RAW preview from: {}", path_str);

    // Use rawler's convenience function with fallback chain:
    // preview_image → full_image
    let dynamic_image = rawler::analyze::extract_preview_pixels(path, &rawler::decoders::RawDecodeParams::default())
        .map_err(|e| format!("Failed to extract preview from {}: {}", path_str, e))?;

    tracing::debug!(
        "Extracted preview: {}x{} from {}",
        dynamic_image.width(),
        dynamic_image.height(),
        path_str
    );

    // Re-encode as JPEG at quality 95
    let mut buf = Vec::with_capacity(dynamic_image.width() as usize * dynamic_image.height() as usize / 3);
    let mut cursor = Cursor::new(&mut buf);

    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 95);
    dynamic_image
        .write_with_encoder(encoder)
        .map_err(|e| format!("Failed to encode JPEG for {}: {}", path_str, e))?;

    tracing::debug!("Encoded preview JPEG: {} bytes for {}", buf.len(), path_str);

    Ok(buf)
}
```

- [ ] **Step 3: Verify module compiles**

Run: `cargo.exe check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: Compiles without errors

---

### Task 3: Register Module, State, and Custom Protocol

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add raw_preview module declaration**

Add after the existing `pub mod color_grading;` line (around line 15) in `src-tauri/src/lib.rs`:

```rust
#[cfg(target_os = "windows")]
pub mod raw_preview;
```

- [ ] **Step 2: Import RawPreviewCache and add state management**

Add to the imports section (around line 20-22), after the existing `use` statements:

```rust
#[cfg(target_os = "windows")]
use raw_preview::RawPreviewCache;
```

Then, inside the `.setup()` closure (after the `app.manage(ai_edit::...)` line around line 169), add:

```rust
// RAW preview cache (Windows only)
#[cfg(target_os = "windows")]
app.manage(Arc::new(RawPreviewCache::new()));
```

- [ ] **Step 3: Register custom protocol with percent-decode utility**

Add a percent-decode utility function and register the protocol. First, add this helper function at the module level (near the end of the file, before or after `spawn_background_tasks`):

```rust
/// Percent-decode a URI path component.
/// Handles UTF-8 encoded file paths (e.g., Chinese characters in paths).
#[cfg(target_os = "windows")]
fn percent_decode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
```

Then modify the builder chain. After the `.invoke_handler(...)` call and before `.run(...)`, add the conditional protocol registration:

```rust
// The existing builder chain ends with .invoke_handler(...)
// After it, add this conditional block:

#[cfg(target_os = "windows")]
let builder = builder.register_asynchronous_uri_scheme_protocol(
    "raw-preview",
    |ctx, request, responder| {
        use std::path::PathBuf;
        use std::sync::Arc;

        let cache: Arc<RawPreviewCache> = ctx
            .app_handle()
            .state::<Arc<RawPreviewCache>>()
            .inner()
            .clone();
        let path_encoded = request
            .uri()
            .path()
            .strip_prefix('/')
            .unwrap_or("")
            .to_string();

        std::thread::spawn(move || {
            let path = PathBuf::from(percent_decode(&path_encoded));
            match cache.get_or_extract(&path) {
                Ok(bytes) => responder.respond(
                    tauri::http::Response::builder()
                        .status(200)
                        .header("Content-Type", "image/jpeg")
                        .body(bytes.to_vec())
                        .unwrap(),
                ),
                Err(e) => {
                    tracing::error!("Failed to extract RAW preview for {}: {}", path_encoded, e);
                    responder.respond(
                        tauri::http::Response::builder()
                            .status(500)
                            .body(b"Failed to extract RAW preview".to_vec())
                            .unwrap(),
                    );
                }
            }
        });
    },
);
```

**Important**: This requires changing the builder from an inline chain to a `let builder` variable. The existing code uses a fluent chain starting with `tauri::Builder::default()` and ending with `.run(...)`. Modify it to:

```rust
let builder = tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(FtpServerState(Arc::new(Mutex::new(None))))
    .setup(move |app| {
        // ... existing setup code unchanged ...
        Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        // ... existing handlers unchanged ...
    ]);

#[cfg(target_os = "windows")]
let builder = builder.register_asynchronous_uri_scheme_protocol(
    "raw-preview",
    // ... handler as above ...
);

builder.run(tauri::generate_context!())
    .unwrap_or_else(|e| {
        eprintln!("Fatal error running Tauri application: {}", e);
        std::process::exit(1);
    });
```

- [ ] **Step 4: Verify compilation**

Run: `cargo.exe check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: Compiles without errors

---

### Task 4: Extend File Index with RAW Extensions

**Files:**
- Modify: `src-tauri/src/file_index/service.rs:216-223`

- [ ] **Step 1: Add RAW extensions to is_supported_image()**

Replace the existing `is_supported_image()` method (lines 216-223):

```rust
    /// Check if a file is a supported image format (including RAW files).
    pub fn is_supported_image(path: &Path) -> bool {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        matches!(ext.as_str(), 
            "jpg" | "jpeg" | "heif" | "hif" | "heic" |
            "nef" | "nrw" | "cr2" | "cr3" | "arw" | "sr2" |
            "raf" | "orf" | "rw2" | "pef" | "dng" | "x3f" | "raw" | "srw"
        )
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo.exe check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: Compiles without errors

---

### Task 5: Modify PreviewWindow for RAW File Detection

**Files:**
- Modify: `src/components/PreviewWindow.tsx:224-226`

- [ ] **Step 1: Add RAW detection utility**

At the top of `src/components/PreviewWindow.tsx` (after the existing imports), add:

```typescript
const RAW_EXTENSIONS = new Set([
  'nef', 'nrw', 'cr2', 'cr3', 'arw', 'sr2',
  'raf', 'orf', 'rw2', 'pef', 'dng', 'x3f', 'raw', 'srw',
]);

function isRawFile(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase();
  return ext ? RAW_EXTENSIONS.has(ext) : false;
}
```

- [ ] **Step 2: Replace image source logic**

Replace the existing image source line (line 226):

```typescript
  // Existing:
  const imageSrc = convertFileSrc(imagePath);
```

With:

```typescript
  // RAW files served via custom protocol with extracted embedded JPEG
  const imageSrc = isRawFile(imagePath)
    ? `http://raw-preview.localhost/${encodeURIComponent(imagePath)}`
    : convertFileSrc(imagePath);
```

- [ ] **Step 3: Verify frontend builds**

Run: `npm run build` (or `pnpm build`)
Expected: Build succeeds without TypeScript errors

---

### Task 6: Build Both Platforms and Verify

- [ ] **Step 1: Full build**

Run: `./build.sh windows android`
Expected: Both platforms build successfully

- [ ] **Step 2: Commit**

```bash
git add -A
git commit -m "feat: add RAW image preview support on Windows

- Add rawler dependency for RAW file parsing
- Create raw_preview module with in-memory JPEG cache
- Register custom Tauri protocol for serving RAW previews
- Extend file index to recognize 14 RAW formats
- PreviewWindow detects RAW files and uses custom protocol"
```
