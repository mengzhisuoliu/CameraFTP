# Design: Windows Image Preview with Universal Memory Cache

**Date**: 2026-05-10
**Status**: Approved (updated)
**Scope**: Windows platform only

## Problem

Windows platform cannot index or display RAW camera files (CR2, NEF, ARW, etc.). RAW files are skipped during FTP upload processing (`is_supported_image()` only accepts `jpg|jpeg|heif|hif|heic`), and even if indexed, the browser cannot render RAW formats natively.

Additionally, there is no image caching mechanism on Windows — every image navigation re-reads from disk via `convertFileSrc()`.

## Solution

Create a custom Tauri protocol (`image-preview`) that serves ALL image types through an in-memory cache. For RAW files, the embedded JPEG preview is extracted via rawler. For JPEG/HEIF files, raw bytes are read from disk and cached. This provides a unified, cached image serving pipeline.

## Architecture

```
FTP Upload → is_supported_image() [add RAW exts]
                    ↓
            file_index.add_file()
                    ↓
         PreviewWindow loads ANY image
                    ↓
         image-preview://localhost/{path}
                    ↓
         Custom Protocol Handler
                    ↓
         Memory Cache hit? ──yes──→ return cached bytes
                    ↓ no
         ┌─── is RAW file? ───┐
         ↓                     ↓
        Yes                   No
         ↓                     ↓
   rawler extract         read file bytes
   embedded JPEG          from disk
         ↓                     ↓
   re-encode as JPEG           ↓
         ↓                     ↓
         └─────→ cache ←───────┘
                    ↓
         return bytes (with correct Content-Type)
```

## Components

### 1. Backend: `image_preview` Module (New)

**Files**: `src-tauri/src/image_preview/mod.rs`, `src-tauri/src/image_preview/extract.rs`

#### Memory Cache

```rust
pub struct ImagePreviewCache {
    cache: RwLock<HashMap<String, Arc<Vec<u8>>>>,
}
```

- **Key**: Canonical file path string
- **Value**: Image bytes (extracted JPEG for RAW, raw bytes for JPEG/HEIF)
- No disk writes, no temp files
- No eviction policy (acceptable for preview use case; typically <100 images per session)

#### Image Serving Logic

The protocol handler dispatches based on file extension:
- **RAW files** (nef, cr2, arw, etc.): Extract embedded JPEG via rawler → re-encode at quality 95 → cache → serve as `image/jpeg`
- **JPEG files** (jpg, jpeg): Read bytes from disk → cache → serve as `image/jpeg`
- **HEIF files** (heif, hif, heic): Read bytes from disk → cache → serve as `image/heic`

#### RAW Extraction (via rawler)

- `rawler::analyze::extract_preview_pixels(path, params)` with fallback chain: preview → full embedded image
- Re-encode `DynamicImage` to JPEG bytes via `image` crate (already a dependency at v0.25, compatible with rawler)
- Sync API wrapped in `std::thread::spawn` for non-blocking usage in async protocol handler

#### Supported RAW Formats (via rawler)

All 14 formats already recognized by the Android color grading module:
`nef`, `nrw`, `cr2`, `cr3`, `arw`, `sr2`, `raf`, `orf`, `rw2`, `pef`, `dng`, `x3f`, `raw`, `srw`

### 2. Backend: Custom Tauri Protocol

**File**: `src-tauri/src/lib.rs` (registration)

Register `image-preview://localhost/` custom protocol:
- URL format (Windows): `http://image-preview.localhost/{url_encoded_file_path}`
- Handler decodes URL path → file path → checks extension → serves accordingly
- Checks memory cache → processes if needed → returns image bytes as HTTP response
- Content-Type varies: `image/jpeg` for JPEG/RAW, `image/heic` for HEIF
- Async protocol handler to support `std::thread::spawn` for rawler extraction

### 3. Backend: Extended File Index

**File**: `src-tauri/src/file_index/service.rs`

Modify `is_supported_image()` to include RAW extensions:
```rust
matches!(ext.as_str(), 
    "jpg" | "jpeg" | "heif" | "hif" | "heic" |
    "nef" | "nrw" | "cr2" | "cr3" | "arw" | "sr2" | 
    "raf" | "orf" | "rw2" | "pef" | "dng" | "x3f" | "raw" | "srw"
)
```

### 4. Frontend: PreviewWindow Adaptation

**File**: `src/components/PreviewWindow.tsx`

Replace `convertFileSrc()` with custom protocol URL for ALL images:
```typescript
// No RAW detection needed — protocol handles all types
const imageSrc = `http://image-preview.localhost/${encodeURIComponent(imagePath)}`;
```

## Dependencies

| Dependency | Version | Purpose | License |
|-----------|---------|---------|---------|
| `rawler` | 0.7.x | RAW file parsing, embedded preview extraction | LGPL-2.1 (compatible with AGPL-3.0) |
| `image` | 0.25 (existing) | JPEG re-encoding of extracted preview | MIT |

No version conflicts — `rawler` depends on `image ^0.25`, which matches the existing dependency.

## Error Handling

- **Unsupported RAW format**: rawler returns error → show PreviewWindow error state
- **File not found**: return 404 response → PreviewWindow shows existing error state
- **Cache miss + processing failure**: No cache entry stored, error returned to frontend
- **Extraction too slow**: `std::thread::spawn` prevents blocking; extraction typically takes 50-200ms

## Scope Boundaries

### In Scope
- Index RAW files alongside JPEG/HEIC
- Display ALL images in PreviewWindow via custom protocol with memory cache
- Memory cache for all image bytes (RAW extracted JPEG + JPEG/HEIF raw bytes)
- All 14 common RAW formats

### Out of Scope
- Thumbnail generation for file list (Windows has no thumbnail grid)
- RAW file editing or conversion
- RAW+JPEG pairing/grouping
- Cache eviction policy (can be added if needed)
- Android platform (already has its own image pipeline)
