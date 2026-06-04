# Color Grading Real-time Preview Performance Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce Android color grading preview latency from 500ms–2s per frame to ~50–100ms by downscaling preview resolution, lowering JPEG quality, eliminating disk I/O, and adding slider throttle.

**Architecture:** Four parallel optimization paths applied to the existing C++ → Rust → JNI → Kotlin → WebView pipeline: (1) C++ side adds resize + buffer-based JPEG encoding + updated API; (2) Rust/Kotlin layers update to pass screen resolution and receive in-memory JPEG bytes; (3) JS slider debounces via 50ms throttle with trailing-edge fire.

**Tech Stack:** C++ (rawalchemy lib, libjpeg-turbo), Rust (libloading FFI, tokio, JNI), Kotlin (Android WebView, JNI), vanilla JS (WebView HTML)

---

### Task 1: C++ Library — Add resize, buffer JPEG, and update raApplyPreviewGrading

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/image_resize.h`
- Create: `src-tauri/lib/rawalchemy/src/image_resize.cpp`
- Modify: `src-tauri/lib/rawalchemy/include/jpeg_writer.h`
- Modify: `src-tauri/lib/rawalchemy/src/jpeg_writer.cpp`
- Modify: `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h`
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`

- [ ] **Step 1: Create image_resize.h**

```cpp
// src-tauri/lib/rawalchemy/include/image_resize.h
#pragma once

#include "common.h"

namespace rawalchemy {

/** Downscale an image to fit inside maxWidth × maxHeight, preserving aspect ratio.
 *  If both maxWidth and maxHeight are 0, returns a copy unchanged. */
ImageBuffer resizeImage(const ImageBuffer& src, int maxWidth, int maxHeight);

} // namespace rawalchemy
```

- [ ] **Step 2: Create image_resize.cpp**

Use bilinear interpolation:

```cpp
// src-tauri/lib/rawalchemy/src/image_resize.cpp
#include "image_resize.h"
#include <algorithm>
#include <cmath>

namespace rawalchemy {

ImageBuffer resizeImage(const ImageBuffer& src, int maxWidth, int maxHeight) {
    if (maxWidth <= 0 && maxHeight <= 0) return src;

    int dstW = src.width;
    int dstH = src.height;

    if (maxWidth > 0 && dstW > maxWidth) {
        float scale = static_cast<float>(maxWidth) / dstW;
        dstW = maxWidth;
        dstH = static_cast<int>(src.height * scale);
    }
    if (maxHeight > 0 && dstH > maxHeight) {
        float scale = static_cast<float>(maxHeight) / dstH;
        dstH = maxHeight;
        dstW = static_cast<int>(dstW * scale);
    }
    if (dstW < 1) dstW = 1;
    if (dstH < 1) dstH = 1;
    if (dstW == src.width && dstH == src.height) return src;

    ImageBuffer dst(dstW, dstH);
    float xRatio = static_cast<float>(src.width) / dstW;
    float yRatio = static_cast<float>(src.height) / dstH;

    for (int y = 0; y < dstH; ++y) {
        float srcY = y * yRatio;
        int y0 = static_cast<int>(srcY);
        int y1 = std::min(y0 + 1, src.height - 1);
        float fy = srcY - y0;

        for (int x = 0; x < dstW; ++x) {
            float srcX = x * xRatio;
            int x0 = static_cast<int>(srcX);
            int x1 = std::min(x0 + 1, src.width - 1);
            float fx = srcX - x0;

            for (int c = 0; c < src.channels; ++c) {
                float v00 = *src.pixel(y0, x0) + c;
                float v10 = *src.pixel(y0, x1) + c;
                float v01 = *src.pixel(y1, x0) + c;
                float v11 = *src.pixel(y1, x1) + c;

                float v0 = v00 + (v10 - v00) * fx;
                float v1 = v01 + (v11 - v01) * fx;
                *dst.pixel(y, x) + c = v0 + (v1 - v0) * fy;
            }
        }
    }
    return dst;
}

} // namespace rawalchemy
```

- [ ] **Step 3: Add writeJpegToBuffer to jpeg_writer.h**

Add new function declaration before `bool writeJpeg(...)`:

```cpp
/** Encode an ImageBuffer to JPEG in memory, returning the bytes via vector.
 *  Same parameters as writeJpeg but writes to a std::vector instead of a file. */
std::vector<uint8_t> writeJpegToBuffer(const ImageBuffer& img,
                                       int quality = 95,
                                       bool optimize = false,
                                       const std::vector<uint8_t>* exifData = nullptr);
```

- [ ] **Step 4: Add writeJpegToBuffer to jpeg_writer.cpp**

Add implementation after the existing `writeJpeg` function (around line 146):

```cpp
std::vector<uint8_t> writeJpegToBuffer(const ImageBuffer& img,
                                       int quality, bool optimize,
                                       const std::vector<uint8_t>* exifData) {
    if (img.width <= 0 || img.height <= 0 || img.data.empty()) {
        return {};
    }
    quality = std::max(1, std::min(quality, 100));

    const int w = img.width;
    const int h = img.height;
    std::vector<uint8_t> pixels(static_cast<size_t>(w) * h * 3);
    const float* src = img.data.data();
    uint8_t* dst = pixels.data();
    for (size_t i = 0; i < pixels.size(); ++i) {
        float v = src[i];
        if (v < 0.0f) v = 0.0f; else if (v > 1.0f) v = 1.0f;
        dst[i] = static_cast<uint8_t>(v * 255.0f + 0.5f);
    }

    tjhandle compressor = tj3Init(TJINIT_COMPRESS);
    if (!compressor) return {};
    tj3Set(compressor, TJPARAM_QUALITY, quality);
    tj3Set(compressor, TJPARAM_SUBSAMP, TJSAMP_444);
    tj3Set(compressor, TJPARAM_FASTDCT, 1);  // fast DCT for speed
    if (optimize) tj3Set(compressor, TJPARAM_OPTIMIZE, 1);
    tj3SetICCProfile(compressor, const_cast<unsigned char*>(SRGB_ICC_PROFILE), SRGB_ICC_PROFILE_SIZE);

    unsigned char* jpegBuf = nullptr;
    size_t jpegSize = 0;
    int result = tj3Compress8(compressor, pixels.data(), w, w * 3, h, TJPF_RGB, &jpegBuf, &jpegSize);
    if (result != 0) {
        tj3Destroy(compressor);
        return {};
    }

    std::vector<uint8_t> output;
    if (exifData && !exifData->empty()) {
        output = rawalchemy::injectExifIntoJpeg(jpegBuf, jpegSize, *exifData);
    } else {
        output.assign(jpegBuf, jpegBuf + jpegSize);
    }

    tj3Free(jpegBuf);
    tj3Destroy(compressor);
    return output;
}
```

Note: `TJPARAM_FASTDCT` is set to `1` (fast) for preview, unlike the file version which uses `0` (accurate). This is an additional speed gain.

- [ ] **Step 5: Update raApplyPreviewGrading in raw_alchemy_capi.h**

Replace the existing `raApplyPreviewGrading` declaration (lines 206–217) with:

```cpp
/** Apply grading to preview session and return JPEG bytes.
 *  Buffer is allocated internally and must be freed by the caller with raFreePreviewBuffer.
 *
 *  @param outBuffer     Receives JPEG data (caller must free via raFreePreviewBuffer).
 *  @param outLen        Receives JPEG data length.
 *  @param maxWidth      Max output width (0 = keep original resolution).
 *  @param maxHeight     Max output height (0 = keep original resolution).
 */
RA_API RaResult RA_CALL raApplyPreviewGrading(
    RaPreviewSession session,
    const char*      logSpace,
    const float*     lutTable,
    int              lutSize,
    const float*     lutDomainMin,
    const float*     lutDomainMax,
    const char*      metering,
    float            evOffset,
    int              jpegQuality,
    int              maxWidth,
    int              maxHeight,
    unsigned char**  outBuffer,
    int*             outLen
);
```

Add after the `raApplyPreviewGrading` declaration:

```cpp
/** Free a buffer allocated by raApplyPreviewGrading. Safe to pass NULL. */
RA_API void RA_CALL raFreePreviewBuffer(unsigned char* buffer);
```

- [ ] **Step 6: Update raApplyPreviewGrading implementation in raw_alchemy_capi.cpp**

Replace the existing implementation (lines 682–738) with:

```cpp
RA_API RaResult RA_CALL raApplyPreviewGrading(
    RaPreviewSession session,
    const char*      logSpace,
    const float*     lutTable,
    int              lutSize,
    const float*     lutDomainMin,
    const float*     lutDomainMax,
    const char*      metering,
    float            evOffset,
    int              jpegQuality,
    int              maxWidth,
    int              maxHeight,
    unsigned char**  outBuffer,
    int*             outLen)
{
    if (!session || !outBuffer || !outLen) {
        setError("raApplyPreviewGrading: null parameter");
        return RA_ERR_INVALID_PARAM;
    }
    clearError();
    *outBuffer = nullptr;
    *outLen = 0;

    try {
        auto& source = session->useCorrected ? session->correctedImage : session->decodedImage;
        auto img = source;

        // Build LUT from pre-parsed data
        rawalchemy::LUT3D lut;
        const rawalchemy::LUT3D* lutPtr = nullptr;
        if (lutTable && lutSize > 0) {
            lut.size = lutSize;
            int totalFloats = lutSize * lutSize * lutSize * 3;
            lut.table.assign(lutTable, lutTable + totalFloats);
            if (lutDomainMin) {
                lut.domainMin[0] = lutDomainMin[0];
                lut.domainMin[1] = lutDomainMin[1];
                lut.domainMin[2] = lutDomainMin[2];
            }
            if (lutDomainMax) {
                lut.domainMax[0] = lutDomainMax[0];
                lut.domainMax[1] = lutDomainMax[1];
                lut.domainMax[2] = lutDomainMax[2];
            }
            lutPtr = &lut;
        }

        RaResult res = runGradingOnly(img, logSpace, lutPtr, metering, evOffset);
        if (res != RA_OK) return res;

        // Resize to fit screen dimensions
        img = rawalchemy::resizeImage(img, maxWidth, maxHeight);

        // Encode to memory buffer
        std::vector<uint8_t> jpegBytes = rawalchemy::writeJpegToBuffer(img, jpegQuality, false, nullptr);
        if (jpegBytes.empty()) {
            setError("Failed to encode preview JPEG");
            return RA_ERR_WRITE_FAILED;
        }

        size_t len = jpegBytes.size();
        unsigned char* buf = new unsigned char[len];
        std::memcpy(buf, jpegBytes.data(), len);
        *outBuffer = buf;
        *outLen = static_cast<int>(len);
        return RA_OK;
    } catch (...) {
        return catchExceptions("raApplyPreviewGrading");
    }
}

RA_API void RA_CALL raFreePreviewBuffer(unsigned char* buffer) {
    delete[] buffer;
}
```

- [ ] **Step 7: Update CMakeLists.txt — add new source files**

Add to the library source list:

```
src/image_resize.cpp
```

- [ ] **Step 8: Build C++ for Windows and verify it compiles**

Run: `./scripts/build-raw-alchemy.sh windows Release`
Expected: SUCCESS with no errors

- [ ] **Step 9: Commit**

```bash
git add src-tauri/lib/rawalchemy/
git commit -m "feat(rawalchemy): add resize, buffer JPEG, update raApplyPreviewGrading for perf"
```

---

### Task 2: Rust FFI — Update RawAlchemyLib bindings

**Files:**
- Modify: `src-tauri/src/color_grading/ffi.rs`

- [ ] **Step 1: Update RaApplyPreviewGradingFn type alias (line 168–179)**

```rust
type RaApplyPreviewGradingFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    *const c_char,         // logSpace
    *const c_float,        // lutTable
    c_int,                 // lutSize
    *const c_float,        // lutDomainMin
    *const c_float,        // lutDomainMax
    *const c_char,         // metering
    c_float,               // evOffset
    c_int,                 // jpegQuality
    c_int,                 // maxWidth
    c_int,                 // maxHeight
    *mut *mut u8,          // outBuffer
    *mut c_int,            // outLen
) -> c_int;
```

Add new type alias after `RaToggleLensCorrectionFn`:

```rust
type RaFreePreviewBufferFn = unsafe extern "C" fn(
    *mut u8, // buffer
);
```

- [ ] **Step 2: Update RawAlchemyLib struct (line 191–200)**

Add `free_preview_buffer` field:

```rust
pub struct RawAlchemyLib {
    _lib: Library,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
    begin_preview_session: RaBeginPreviewSessionFn,
    apply_preview_grading: RaApplyPreviewGradingFn,
    end_preview_session: RaEndPreviewSessionFn,
    toggle_lens_correction: RaToggleLensCorrectionFn,
    free_preview_buffer: RaFreePreviewBufferFn,
}
```

- [ ] **Step 3: Load free_preview_buffer symbol (in load() method, after toggle_lens_correction)**

```rust
let free_preview_buffer = unsafe {
    *lib.get::<RaFreePreviewBufferFn>(b"raFreePreviewBuffer\0")
        .map_err(|e| {
            AppError::ColorGradingError(format!("Symbol raFreePreviewBuffer not found: {}", e))
        })?
};
```

Add `free_preview_buffer` to the struct construction at end of `load()`.

- [ ] **Step 4: Update apply_preview_grading() method (lines 437–477)**

Replace the entire method body:

```rust
pub(crate) fn apply_preview_grading(
    &self,
    session: &RaPreviewSession,
    log_space: Option<&str>,
    lut_data: &Arc<super::lut_data::LutData>,
    ev_offset: f32,
    metering_mode: &str,
    jpeg_quality: i32,
    max_width: u32,
    max_height: u32,
) -> Result<Vec<u8>, AppError> {
    let log_c = log_space
        .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid log space: {}", e))))
        .transpose()?
        .unwrap_or_else(|| std::ffi::CString::new("").expect("empty string is valid CString"));
    let metering_c = std::ffi::CString::new(metering_mode)
        .map_err(|e| AppError::ColorGradingError(format!("Invalid metering mode: {}", e)))?;

    let mut out_buf: *mut u8 = std::ptr::null_mut();
    let mut out_len: c_int = 0;

    let result = unsafe {
        (self.apply_preview_grading)(
            session.ptr,
            if log_space.is_some() { log_c.as_ptr() } else { std::ptr::null() },
            lut_data.table.as_ptr(),
            lut_data.size as c_int,
            lut_data.domain_min.as_ptr(),
            lut_data.domain_max.as_ptr(),
            metering_c.as_ptr(),
            ev_offset,
            jpeg_quality as c_int,
            max_width as c_int,
            max_height as c_int,
            &mut out_buf,
            &mut out_len,
        )
    };

    let ra_result = ra_result_from_code(result);
    if !ra_result.is_ok() {
        return Err(self.format_last_error(ra_result, result));
    }

    if out_buf.is_null() || out_len <= 0 {
        return Err(AppError::ColorGradingError("Buffer is empty".into()));
    }

    let jpeg_bytes = unsafe {
        std::slice::from_raw_parts(out_buf, out_len as usize).to_vec()
    };

    // Free the C++-allocated buffer
    unsafe { (self.free_preview_buffer)(out_buf); }

    Ok(jpeg_bytes)
}
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/color_grading/ffi.rs
git commit -m "feat(ffi): update raApplyPreviewGrading binding for buffer output and resize"
```

---

### Task 3: Rust preview.rs — Streamline ActiveSession and apply()

**Files:**
- Modify: `src-tauri/src/color_grading/preview.rs`

- [ ] **Step 1: Lower JPEG quality constant (line 14)**

```rust
const PREVIEW_JPEG_QUALITY: i32 = 50;
```

- [ ] **Step 2: Remove preview_output_path from ActiveSession (line 18–23)**

```rust
struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
    enable_lens_correction: bool,
}
```

- [ ] **Step 3: Update begin() — remove preview dir creation + output_path assignment (lines 75–88)**

Replace lines 75–88 with:

```rust
tracing::info!(image = image_path, "Preview session ready");

*guard = Some(ActiveSession {
    session,
    image_path: image_path.to_string(),
    enable_lens_correction: true,
});
```

Remove the `tokio::fs::create_dir_all` block entirely.

- [ ] **Step 4: Update apply() — change return type and add resize params (lines 93–153)**

Replace the entire `apply()` method:

```rust
pub async fn apply(
    &self,
    lut_id: &str,
    enable_lens_correction: bool,
    metering_mode: &str,
    ev_offset: f32,
    max_width: u32,
    max_height: u32,
) -> Result<Vec<u8>, AppError> {
    let lib = RawAlchemyLib::get()?;
    let preset = find_preset(lut_id)
        .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
    let lut_data = lut_data::get_lut_data(&preset.id)?;

    let lensfun_db_path = super::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    let mut guard = self.inner.lock().await;
    let active = guard.as_mut()
        .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;

    if enable_lens_correction != active.enable_lens_correction {
        tracing::info!(
            from = active.enable_lens_correction,
            to = enable_lens_correction,
            "Toggling lens correction"
        );
        let session = RaPreviewSession { ptr: active.session.ptr };
        lib.toggle_lens_correction(&session, enable_lens_correction, lensfun_db_path.as_deref())?;
        active.enable_lens_correction = enable_lens_correction;
    }

    let session_addr = active.session.ptr as usize;
    let log_space = preset.log_space.clone();
    let metering = metering_mode.to_string();

    tracing::debug!(lut = lut_id, ev = ev_offset, lens = enable_lens_correction,
                    max_w = max_width, max_h = max_height, "Applying preview grading");

    tokio::task::spawn_blocking(move || {
        let session = RaPreviewSession { ptr: session_addr as *mut std::ffi::c_void };
        lib.apply_preview_grading(
            &session,
            Some(log_space.as_str()),
            &lut_data,
            ev_offset,
            &metering,
            PREVIEW_JPEG_QUALITY,
            max_width,
            max_height,
        )
    })
    .await
    .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))?
}
```

- [ ] **Step 5: Update end_session_internal — remove file cleanup (line 168–171)**

```rust
fn end_session_internal(lib: &Arc<RawAlchemyLib>, active: ActiveSession) {
    lib.end_preview_session(active.session);
}
```

- [ ] **Step 6: Remove unused imports and dead code (lines 5–6, 148–186)**

Remove: `use std::path::{Path, PathBuf}` (keep `Path` if still needed), remove `percent_encode` and its tests, remove `output_path_for_url` logic.

Remove the `percent_encode` function and its test module (lines 173–221) since they are no longer needed.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/color_grading/preview.rs
git commit -m "feat(preview): return JPEG bytes directly, lower quality to 50, add resize params"
```

---

### Task 4: Rust JNI Bridge — Update nativeApplyPreview

**Files:**
- Modify: `src-tauri/src/color_grading/jni_bridge.rs`

- [ ] **Step 1: Update nativeApplyPreview signature (lines 94–101)**

Add `max_width` and `max_height` parameters:

```rust
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeApplyPreview(
    mut env: JNIEnv,
    _class: JClass,
    lut_id: JString,
    enable_lens_correction: jboolean,
    metering_mode: JString,
    ev_offset: jfloat,
    max_width: jint,
    max_height: jint,
) -> jstring {
```

- [ ] **Step 2: Update apply call (lines 103–126)**

Replace the call and response construction:

```rust
    let lut_id_str = match env.get_string(&lut_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid lutId"),
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid meteringMode"),
    };

    let state = crate::color_grading::preview::ColorGradingPreviewState::get_global();
    let result = run_blocking(state.apply(
        &lut_id_str,
        enable_lens_correction != 0,
        &metering_str,
        ev_offset,
        max_width as u32,
        max_height as u32,
    ));

    match result {
        Ok(jpeg_bytes) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
            let json = serde_json::json!({
                "ok": true,
                "buffer": b64,
            })
            .to_string();
            new_json_string(&mut env, &json)
        }
        Err(e) => json_error(&mut env, &e.to_string()),
    }
```

Note: `base64` crate must be in Cargo.toml. Check if already a dependency.

- [ ] **Step 3: Check base64 dependency**

Run: `grep base64 src-tauri/Cargo.toml`

If not present, add to `[dependencies]`:

```toml
base64 = "0.22"
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/color_grading/jni_bridge.rs src-tauri/Cargo.toml
git commit -m "feat(jni): pass maxWidth/maxHeight to apply, return base64 JPEG buffer"
```

---

### Task 5: Kotlin — Update JNI Bridge and Activity

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt`

- [ ] **Step 1: Update ColorGradingJniBridge.kt — external fun signature (line 88)**

```kotlin
@JvmStatic
private external fun nativeApplyPreview(
    lutId: String,
    enableLensCorrection: Boolean,
    meteringMode: String,
    evOffset: Float,
    maxWidth: Int,
    maxHeight: Int
): String
```

- [ ] **Step 2: Update ColorGradingJniBridge.kt — applyPreview() method (lines 30–38)**

```kotlin
fun applyPreview(
    lutId: String,
    enableLensCorrection: Boolean,
    meteringMode: String,
    evOffset: Float,
    maxWidth: Int,
    maxHeight: Int
): Result<ByteArray> {
    return try {
        val json = nativeApplyPreview(lutId, enableLensCorrection, meteringMode, evOffset, maxWidth, maxHeight)
        parseResultWithBuffer(json)
    } catch (e: Exception) {
        Log.e(TAG, "applyPreview failed", e)
        Result.failure(e)
    }
}
```

- [ ] **Step 3: Update ColorGradingJniBridge.kt — add parseResultWithBuffer() (after parseResultWithUrl)**

```kotlin
private fun parseResultWithBuffer(json: String): Result<ByteArray> {
    val obj = JSONObject(json)
    if (obj.optBoolean("ok", false)) {
        val b64 = obj.optString("buffer", "")
        if (b64.isEmpty()) return Result.failure(Exception("Empty buffer"))
        return try {
            Result.success(android.util.Base64.decode(b64, android.util.Base64.DEFAULT))
        } catch (e: Exception) {
            Result.failure(Exception("Base64 decode failed: ${e.message}"))
        }
    }
    return Result.failure(Exception(obj.optString("error", "Unknown error")))
}
```

Remove `parseResultWithUrl` if no longer used.

- [ ] **Step 4: Update ColorGradingActivity.kt — replace previewFilePath with previewJpegBytes (line 35–36)**

```kotlin
@Volatile
internal var previewJpegBytes: ByteArray? = null
```

Remove `previewFilePath: String?`.

- [ ] **Step 5: Update ColorGradingActivity.kt — shouldInterceptRequest (lines 68–89)**

```kotlin
webViewClient = object : WebViewClient() {
    override fun shouldInterceptRequest(
        view: WebView, request: WebResourceRequest
    ): WebResourceResponse? {
        if (request.url.scheme == "preview" && request.url.host == "latest") {
            val bytes = previewJpegBytes
            if (bytes != null && bytes.isNotEmpty()) {
                return WebResourceResponse(
                    "image/jpeg", null, 200, "OK",
                    mapOf("Content-Length" to bytes.size.toString()),
                    java.io.ByteArrayInputStream(bytes)
                )
            }
            return WebResourceResponse(
                "image/jpeg", null, 404, "Not Found",
                emptyMap(), null
            )
        }
        return super.shouldInterceptRequest(view, request)
    }
}
```

- [ ] **Step 6: Update ColorGradingActivity.kt — NativeColorGradingPreviewBridge.applyPreview() (lines 176–202)**

```kotlin
@JavascriptInterface
fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
    val activity = activityRef.get() ?: return
    val maxWidth = activity.resources.displayMetrics.widthPixels
    val maxHeight = activity.resources.displayMetrics.heightPixels
    Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset size=${maxWidth}x${maxHeight} (JNI)")
    Thread {
        val result = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset, maxWidth, maxHeight)
        activity.runOnUiThread {
            if (result.isSuccess) {
                activity.previewJpegBytes = result.getOrDefault(ByteArray(0))
                activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
            } else {
                val msg = result.exceptionOrNull()?.message ?: "应用失败"
                activity.webView?.evaluateJavascript(
                    "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                )
            }
        }
    }.start()
}
```

- [ ] **Step 7: Update endPreviewSession() — clear bytes instead of file (line 128–132)**

```kotlin
internal fun endPreviewSession() {
    isSessionActive = false
    previewJpegBytes = null
    Thread { ColorGradingJniBridge.endPreview() }.start()
}
```

- [ ] **Step 8: Remove extractFilePathFromUrl (lines 134–147)**

Delete the method — no longer needed.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridge.kt
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt
git commit -m "feat(android): in-memory JPEG preview pipeline with screen-sized resize"
```

---

### Task 6: JS — Add 50ms slider throttle

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/assets/color_grading_preview.html`

- [ ] **Step 1: Replace requestApply() with throttled version (lines 286–293)**

```javascript
var applyTimer = null;

function requestApply() {
    if (state === 'LOADING' || state === 'SAVING' || state === 'ERROR') return;
    state = 'ADJUSTING';

    // Clear any pending timer — restart the throttle window
    if (applyTimer !== null) {
        clearTimeout(applyTimer);
        applyTimer = null;
    }

    if (!applyPending) {
        applyPending = true;
        NativeBridge.applyPreview(selectedLut, selectedMetering, currentEv);
    } else {
        // Throttle: schedule an apply after 50ms — if no new input arrives,
        // fire the trailing edge
        applyTimer = setTimeout(function() {
            applyTimer = null;
            applyPending = true;
            NativeBridge.applyPreview(selectedLut, selectedMetering, currentEv);
        }, 50);
    }
}
```

- [ ] **Step 2: Update onEvChange() to use change event for trailing edge (line 276–280)**

Add a `change` event listener for the trailing edge fire when the user releases the slider. Keep `oninput` for continuous preview:

```javascript
function onEvChange() {
    currentEv = parseFloat(document.getElementById('evSlider').value);
    updateEvDisplay();
    requestApply();
}
```

Add to the init/load handler:

```javascript
// In the window.addEventListener('load', ...) block, add after existing code:
document.getElementById('evSlider').addEventListener('change', function() {
    // Trailing edge: fire immediately when user releases slider
    if (applyTimer !== null) {
        clearTimeout(applyTimer);
        applyTimer = null;
    }
    if (!applyPending) {
        applyPending = true;
        NativeBridge.applyPreview(selectedLut, selectedMetering, currentEv);
    }
});
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/assets/color_grading_preview.html
git commit -m "feat(js): add 50ms throttle with trailing-edge fire for EV slider"
```

---

### Task 7: Build Verification

**Files:** (none changed — verification only)

- [ ] **Step 1: Build Android**

```bash
./build.sh android
```

Expected: Build succeeds with no compilation errors in Rust, Kotlin, or cmake.

- [ ] **Step 2: Build Windows (for type safety)**

```bash
./build.sh windows
```

Expected: Build succeeds. Note that Windows build doesn't use the JNI bridge (it's `#[cfg(target_os = "android")]`), but the FFI/preview changes must compile on all platforms.

- [ ] **Step 3: Review build output for warnings**

Check there are no new warnings related to the changed code.

---

### Rollback Plan

If the C++/Rust API changes cause build breakage on Windows (where the DLL may not yet have the new symbols):
1. Revert the `raw_alchemy_capi.cpp` changes
2. Use the old file-based `raApplyPreviewGrading` signature but keep the resize + quality changes (call `writeJpeg` with lower quality)
3. The buffer-return optimization is Android-only; Windows can keep file-based preview

---

### Implementation Order Notes

Tasks 1–4 are tightly coupled: any C++ signature change ripples through the Rust layers. Execute them sequentially.

Task 5 depends on Tasks 3–4.

Task 6 is independent of all other tasks and can be done at any point.

Task 7 must run last, after all changes.
