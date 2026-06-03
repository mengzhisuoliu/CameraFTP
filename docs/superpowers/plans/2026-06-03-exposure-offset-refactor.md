# Exposure Offset Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the auto/manual exposure toggle with a unified model: always auto-meter with a user-chosen metering mode, plus an EV offset slider.

**Architecture:** Remove `useAutoExposure` and rename `manualEv`/`manual_ev` to `evOffset`/`ev_offset` across all layers (C++ → Rust FFI → Rust structs → TypeScript UI → Android Kotlin). The C++ exposure logic changes from if/else to always auto-meter + offset. No config migration — old configs are silently discarded via serde defaults.

**Tech Stack:** C++ (RawAlchemy native lib), Rust (Tauri backend), TypeScript/React (frontend), Kotlin (Android native UI)

**Design spec:** `docs/superpowers/specs/2026-06-03-exposure-offset-refactor-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` | Modify | C API header — remove `useAutoExposure`, rename `manualEv` → `evOffset` |
| `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` | Modify | C API impl — change exposure logic in 3 internal helpers + update 4 public functions |
| `src-tauri/src/color_grading/ffi.rs` | Modify | Rust FFI — update type aliases and wrapper functions |
| `src-tauri/src/config.rs` | Modify | Rust config structs — remove field, rename field |
| `src-tauri/src/commands/color_grading.rs` | Modify | Rust Tauri commands — update params |
| `src-tauri/src/color_grading/service.rs` | Modify | Rust service — update task struct, enqueue, process |
| `src-tauri/src/color_grading/preview.rs` | Modify | Rust preview — update apply params |
| `src/components/ExposureConfigSection.tsx` | Modify | TS UI — rewrite to always show both controls |
| `src/components/ColorGradingDialog.tsx` | Modify | TS UI — remove toggle state |
| `src/components/AutoColorGradingConfigCard.tsx` | Modify | TS UI — remove toggle handler |
| `src/components/PreviewWindow.tsx` | Modify | TS UI — update callback signature |
| `src/components/GalleryCard.tsx` | Modify | TS UI — update callback signature |
| `src/hooks/useColorGradingProgress.ts` | Modify | TS hook — update function signature |
| `src/types/global.ts` | Modify | TS types — update bridge type |
| `src/App.tsx` | Modify | TS app — update bridge handler |
| `src/types/index.ts` | Modify | TS re-exports (auto-updated by gen-types) |
| `src-tauri/gen/android/.../controllers/WebViewOverlayController.kt` | Modify | Kotlin — update dialog builder and bridge |
| `src-tauri/gen/android/.../ImageViewerActivity.kt` | Modify | Kotlin — update dispatch and args |
| `src-tauri/gen/android/.../assets/color_grading_dialog.html` | Modify | Android HTML — remove toggle, always show both |

---

### Task 1: C++ API Header

**Files:**
- Modify: `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h`

- [ ] **Step 1: Update `raProcessFile` declaration**

In `raw_alchemy_capi.h`, replace the `raProcessFile` declaration and its doc block (lines 73–102) with:

```c
/** Process a RAW file through the full pipeline and save to disk.
 *
 *  Pipeline: Decode -> [Lens Correction] -> [Exposure] -> [Sat/Cont Boost]
 *            -> [Log Transform] -> [LUT] -> Save
 *
 *  All intermediate memory is managed internally.
 *
 *  @param inputPath   UTF-8 path to input RAW file.
 *  @param outputPath  UTF-8 path to output file (extension determines format).
 *  @param logSpace    Log space name, or NULL to skip log transform.
 *  @param lutPath     Path to .cube LUT file, or NULL to skip LUT.
 *  @param metering    Metering mode, or NULL for "matrix".
 *  @param evOffset    Exposure offset in stops, applied on top of auto-metered exposure.
 *  @param jpegQuality JPEG quality 1-100 (only used for JPEG output).
 *  @param enableLensCorrection  If non-zero, enable lens correction.
 *  @param customLensfunDb      Custom Lensfun DB path, or NULL.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raProcessFile(
    const char* inputPath,
    const char* outputPath,
    const char* logSpace,
    const char* lutPath,
    const char* metering,
    float       evOffset,
    int         jpegQuality,
    int         enableLensCorrection,
    const char* customLensfunDb
);
```

- [ ] **Step 2: Update `raProcessFileWithLUT` declaration**

Replace lines 104–140 with:

```c
/** Process a RAW file with a pre-parsed LUT (avoids repeated file I/O).
 *
 *  Same as raProcessFile but accepts LUT data directly as a flat float array
 *  instead of a file path. The table layout matches .cube format:
 *  [size³ × 3] floats, row-major (R changes fastest).
 *
 *  This allows callers to cache parsed LUT data in memory.
 *
 *  @param inputPath   UTF-8 path to input RAW file.
 *  @param outputPath  UTF-8 path to output file (extension determines format).
 *  @param logSpace    Log space name, or NULL to skip log transform.
 *  @param lutTable    Pointer to pre-parsed LUT float data [size³ × 3], or NULL to skip LUT.
 *  @param lutSize     LUT dimension (e.g., 65 for a 65³ grid). Ignored if lutTable is NULL.
 *  @param lutDomainMin  LUT domain minimum [R, G, B]. Pass NULL for default {0,0,0}.
 *  @param lutDomainMax  LUT domain maximum [R, G, B]. Pass NULL for default {1,1,1}.
 *  @param metering    Metering mode, or NULL for "matrix".
 *  @param evOffset    Exposure offset in stops, applied on top of auto-metered exposure.
 *  @param jpegQuality JPEG quality 1-100.
 *  @param enableLensCorrection  If non-zero, enable lens correction.
 *  @param customLensfunDb      Custom Lensfun DB path, or NULL.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raProcessFileWithLUT(
    const char* inputPath,
    const char* outputPath,
    const char* logSpace,
    const float* lutTable,
    int         lutSize,
    const float* lutDomainMin,
    const float* lutDomainMax,
    const char* metering,
    float       evOffset,
    int         jpegQuality,
    int         enableLensCorrection,
    const char* customLensfunDb
);
```

- [ ] **Step 3: Update `raProcessToBuffer` declaration**

Replace lines 142–167 with:

```c
/** Process a RAW file through the full pipeline and return pixel data.
 *
 *  Pipeline: Decode -> [Lens Correction] -> [Exposure] -> [Sat/Cont Boost]
 *            -> [Log Transform] -> [LUT]
 *
 *  @param inputPath   UTF-8 path to input RAW file.
 *  @param logSpace    Log space name, or NULL to skip.
 *  @param lutPath     Path to .cube LUT, or NULL to skip.
 *  @param metering    Metering mode, or NULL for "matrix".
 *  @param evOffset    Exposure offset in stops, applied on top of auto-metered exposure.
 *  @param enableLensCorrection  If non-zero, enable lens correction.
 *  @param customLensfunDb      Custom Lensfun DB path, or NULL.
 *  @param outBuf      Receives the processed image. Caller must destroy.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raProcessToBuffer(
    const char* inputPath,
    const char* logSpace,
    const char* lutPath,
    const char* metering,
    float       evOffset,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaImageBuffer* outBuf
);
```

- [ ] **Step 4: Update `raApplyPreviewGrading` declaration**

Replace lines 192–225 with:

```c
/** Apply grading parameters to the session's cached decoded image.
 *
 *  The session's internal data is NOT modified — safe to call repeatedly
 *  with different parameters.  Internally clones the cached buffer, applies
 *  the full grading pipeline, and writes the result to outputPath.
 *
 *  Pipeline on cloned data:
 *    Exposure -> Sat/Contrast Boost -> Log Transform -> LUT -> JPEG encode
 *
 *  @param session         Active preview session.
 *  @param logSpace        Log space name, or NULL to skip.
 *  @param lutTable        Pre-parsed LUT float data [size^3 x 3], or NULL.
 *  @param lutSize         LUT dimension. Ignored if lutTable is NULL.
 *  @param lutDomainMin    LUT domain min [R,G,B], or NULL for {0,0,0}.
 *  @param lutDomainMax    LUT domain max [R,G,B], or NULL for {1,1,1}.
 *  @param metering        Metering mode, or NULL for "matrix".
 *  @param evOffset        Exposure offset in stops, applied on top of auto-metered exposure.
 *  @param jpegQuality     JPEG quality 1-100.
 *  @param outputPath      UTF-8 output path.
 *  @return RA_OK on success. */
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
    const char*      outputPath
);
```

---

### Task 2: C++ Implementation

**Files:**
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp`

- [ ] **Step 1: Update `runPipeline` signature and exposure logic**

In the `runPipeline` function (starts around line 106), change the parameter list and exposure logic.

Replace the signature (lines 106–114):
```cpp
RaResult runPipeline(rawalchemy::ImageBuffer& img,
                     const rawalchemy::CameraMetadata& meta,
                     const char* logSpace,
                     const char* lutPath,
                     const char* metering,
                     float manualEv,
                     int useAutoExposure,
                     int enableLensCorrection,
                     const char* customLensfunDb) {
```
With:
```cpp
RaResult runPipeline(rawalchemy::ImageBuffer& img,
                     const rawalchemy::CameraMetadata& meta,
                     const char* logSpace,
                     const char* lutPath,
                     const char* metering,
                     float evOffset,
                     int enableLensCorrection,
                     const char* customLensfunDb) {
```

Then replace the exposure block (lines 137–152):
```cpp
    // Exposure
    try {
        if (useAutoExposure) {
            std::string mode(metering ? metering : "matrix");
            if (!rawalchemy::isMeteringModeSupported(mode)) {
                setError(std::string("Unsupported metering mode: ") + mode);
                return RA_ERR_INVALID_PARAM;
            }
            float gain = rawalchemy::computeAutoGain(img, mode);
            img.applyGain(gain);
        } else {
            img.applyGain(std::pow(2.0f, manualEv));
        }
    } catch (...) {
        return catchExceptions("exposure");
    }
```
With:
```cpp
    // Exposure: auto-meter with offset
    try {
        std::string mode(metering ? metering : "matrix");
        if (!rawalchemy::isMeteringModeSupported(mode)) {
            setError(std::string("Unsupported metering mode: ") + mode);
            return RA_ERR_INVALID_PARAM;
        }
        float gain = rawalchemy::computeAutoGain(img, mode);
        img.applyGain(gain * std::pow(2.0f, evOffset));
    } catch (...) {
        return catchExceptions("exposure");
    }
```

- [ ] **Step 2: Update `runPipelineWithLUT` signature and exposure logic**

Find the second `runPipelineWithLUT` function in the anonymous namespace (around line 195). Apply the same changes: remove `useAutoExposure` param, rename `manualEv` to `evOffset`, replace the if/else exposure block with the unified auto-meter+offset logic.

Signature change:
```cpp
RaResult runPipelineWithLUT(rawalchemy::ImageBuffer& img,
                            const rawalchemy::CameraMetadata& meta,
                            const char* logSpace,
                            const rawalchemy::LUT3D* lut,
                            const char* metering,
                            float evOffset,
                            int enableLensCorrection,
                            const char* customLensfunDb) {
```

Exposure block — same replacement as Step 1.

- [ ] **Step 3: Update `runGradingOnly` signature and exposure logic**

Find `runGradingOnly` (around line 318). Same pattern: remove `useAutoExposure`, rename `manualEv` to `evOffset`, replace exposure logic.

Signature:
```cpp
RaResult runGradingOnly(
    rawalchemy::ImageBuffer& img,
    const char* logSpace,
    const rawalchemy::LUT3D* lut,
    const char* metering,
    float evOffset)
```

Exposure block — same replacement as Step 1 (but without `int useAutoExposure` in the `if` condition).

- [ ] **Step 4: Update `raProcessFile` public function**

Update `raProcessFile` (around line 410). Remove `useAutoExposure` param, rename `manualEv` to `evOffset`:

```cpp
RA_API RaResult RA_CALL raProcessFile(
    const char* inputPath,
    const char* outputPath,
    const char* logSpace,
    const char* lutPath,
    const char* metering,
    float       evOffset,
    int         jpegQuality,
    int         enableLensCorrection,
    const char* customLensfunDb
) {
```

Update the call to `runPipeline` — remove `useAutoExposure`, pass `evOffset`:
```cpp
        RaResult res = runPipeline(img, meta, logSpace, lutPath, metering,
                                   evOffset,
                                   enableLensCorrection, customLensfunDb);
```

- [ ] **Step 5: Update `raProcessFileWithLUT` public function**

Update `raProcessFileWithLUT` (around line 478). Same pattern:

```cpp
RA_API RaResult RA_CALL raProcessFileWithLUT(
    const char* inputPath,
    const char* outputPath,
    const char* logSpace,
    const float* lutTable,
    int         lutSize,
    const float* lutDomainMin,
    const float* lutDomainMax,
    const char* metering,
    float       evOffset,
    int         jpegQuality,
    int         enableLensCorrection,
    const char* customLensfunDb
) {
```

Update the call to `runPipelineWithLUT`:
```cpp
        RaResult res = runPipelineWithLUT(img, meta, logSpace, lutPtr, metering,
                                   evOffset,
                                   enableLensCorrection, customLensfunDb);
```

- [ ] **Step 6: Update `raProcessToBuffer` public function**

Update `raProcessToBuffer` (around line 564):

```cpp
RA_API RaResult RA_CALL raProcessToBuffer(
    const char* inputPath,
    const char* logSpace,
    const char* lutPath,
    const char* metering,
    float       evOffset,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaImageBuffer* outBuf
) {
```

Update the call to `runPipeline`:
```cpp
        RaResult res = runPipeline(img, meta, logSpace, lutPath, metering,
                                   evOffset,
                                   enableLensCorrection, customLensfunDb);
```

- [ ] **Step 7: Update `raApplyPreviewGrading` public function**

Update `raApplyPreviewGrading` (around line 700):

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
    const char*      outputPath)
```

Update the call to `runGradingOnly`:
```cpp
        RaResult res = runGradingOnly(img, logSpace, lutPtr, metering,
                                       evOffset);
```

- [ ] **Step 8: Commit C++ changes**

```bash
git add src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp
git commit -m "refactor(exposure): remove useAutoExposure from C API, rename manualEv to evOffset"
```

---

### Task 3: Rust FFI Layer

**Files:**
- Modify: `src-tauri/src/color_grading/ffi.rs`

- [ ] **Step 1: Update `RaProcessFileWithLUTFn` type alias**

In `ffi.rs`, find the `RaProcessFileWithLUTFn` type alias (around line 130). Remove `useAutoExposure` param, rename `manualEv` to `evOffset`:

```rust
type RaProcessFileWithLUTFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    *const c_char,   // outputPath
    *const c_char,   // logSpace
    *const c_float,  // lutTable
    c_int,           // lutSize
    *const c_float,  // lutDomainMin
    *const c_float,  // lutDomainMax
    *const c_char,   // metering
    c_float,         // evOffset
    c_int,           // jpegQuality
    c_int,           // enableLensCorrection
    *const c_char,   // customLensfunDb
) -> c_int;
```

- [ ] **Step 2: Update `RaApplyPreviewGradingFn` type alias**

Find `RaApplyPreviewGradingFn` (around line 166). Remove `useAutoExposure`, rename `manualEv` to `evOffset`:

```rust
type RaApplyPreviewGradingFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    *const c_char,   // logSpace
    *const c_float,  // lutTable
    c_int,           // lutSize
    *const c_float,  // lutDomainMin
    *const c_float,  // lutDomainMax
    *const c_char,   // metering
    c_float,         // evOffset
    c_int,           // jpegQuality
    *const c_char,   // outputPath
) -> c_int;
```

- [ ] **Step 3: Update `process_file_with_lut` wrapper method**

Find `pub fn process_file_with_lut` (around line 343). Remove `use_auto_exposure: bool` param, rename `manual_ev` to `ev_offset`. Update the unsafe call to remove the `if use_auto_exposure { 1 } else { 0 }` argument:

```rust
    pub fn process_file_with_lut(
        &self,
        input_path: &Path,
        output_path: &Path,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        lensfun_db_path: Option<&str>,
        ev_offset: f32,
        metering_mode: &str,
    ) -> Result<(), AppError> {
```

In the unsafe block, update the call to match new C signature — remove `useAutoExposure` argument, pass `ev_offset` in its new position:

```rust
        let result = unsafe {
            (self.process_file_with_lut)(
                input_c.as_ptr(),
                output_c.as_ptr(),
                if log_space.is_some() {
                    log_c.as_ptr()
                } else {
                    std::ptr::null()
                },
                lut_data.table.as_ptr(),
                lut_data.size as c_int,
                lut_data.domain_min.as_ptr(),
                lut_data.domain_max.as_ptr(),
                metering_c.as_ptr(),
                ev_offset,
                DEFAULT_JPEG_QUALITY,
                ENABLE_LENS_CORRECTION,
                lensfun_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };
```

- [ ] **Step 4: Update `apply_preview_grading` wrapper method**

Find `pub(crate) fn apply_preview_grading` (around line 438). Remove `use_auto_exposure: bool`, rename `manual_ev` to `ev_offset`:

```rust
    pub(crate) fn apply_preview_grading(
        &self,
        session: &RaPreviewSession,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        ev_offset: f32,
        metering_mode: &str,
        jpeg_quality: i32,
        output_path: &Path,
    ) -> Result<(), AppError> {
```

Update the unsafe call — remove `useAutoExposure` argument, pass `ev_offset`:

```rust
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
```

- [ ] **Step 5: Commit Rust FFI changes**

```bash
git add src-tauri/src/color_grading/ffi.rs
git commit -m "refactor(exposure): update Rust FFI to match new C API"
```

---

### Task 4: Rust Config Structs

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: Update `AutoColorGradingConfig`**

Find `AutoColorGradingConfig` (around line 137). Remove `use_auto_exposure` field, rename `manual_ev` to `ev_offset`:

```rust
/// 自动调色配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoColorGradingConfig {
    /// 是否启用自动调色
    pub enabled: bool,
    /// 调色预设 ID
    #[serde(alias = "presetLutId")]
    pub preset_id: String,
    /// 曝光偏移量（EV），基于自动测光结果的偏移
    pub ev_offset: f32,
    /// 自动曝光测光模式
    pub metering_mode: String,
}

impl Default for AutoColorGradingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preset_id: crate::color_grading::presets::DEFAULT_PRESET_ID.to_string(),
            ev_offset: 0.0,
            metering_mode: "highlight-safe".to_string(),
        }
    }
}
```

- [ ] **Step 2: Update `ColorGradingLastUsed`**

Find `ColorGradingLastUsed` (around line 163). Same changes:

```rust
/// 调色对话框上次使用的参数
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorGradingLastUsed {
    /// 调色预设 ID
    pub preset_id: String,
    /// 曝光偏移量（EV），基于自动测光结果的偏移
    pub ev_offset: f32,
    /// 自动曝光测光模式
    pub metering_mode: String,
}

impl Default for ColorGradingLastUsed {
    fn default() -> Self {
        Self {
            preset_id: crate::color_grading::presets::DEFAULT_PRESET_ID.to_string(),
            ev_offset: 0.0,
            metering_mode: "highlight-safe".to_string(),
        }
    }
}
```

- [ ] **Step 3: Commit config struct changes**

```bash
git add src-tauri/src/config.rs
git commit -m "refactor(exposure): remove useAutoExposure from config structs, rename manualEv to evOffset"
```

---

### Task 5: Rust Service, Commands, and Preview

**Files:**
- Modify: `src-tauri/src/color_grading/service.rs`
- Modify: `src-tauri/src/commands/color_grading.rs`
- Modify: `src-tauri/src/color_grading/preview.rs`

- [ ] **Step 1: Update `ColorGradingTask` in `service.rs`**

Find `struct ColorGradingTask` (around line 20). Remove `use_auto_exposure`, rename `manual_ev` to `ev_offset`:

```rust
struct ColorGradingTask {
    input_path: PathBuf,
    lut_id: String,
    metering_mode: String,
    ev_offset: f32,
}
```

- [ ] **Step 2: Update `ColorGradingService::enqueue` in `service.rs`**

Find `pub async fn enqueue` (around line 53). Remove `use_auto_exposure` param, rename `manual_ev` to `ev_offset`. Update task construction:

```rust
    pub async fn enqueue(&self, file_paths: Vec<PathBuf>, lut_id: String, metering_mode: String, ev_offset: f32) -> Result<(), AppError> {
```

Update the task construction inside the loop:
```rust
            match self.sender.send(ColorGradingTask {
                input_path: path,
                lut_id: preset.id.clone(),
                metering_mode: metering_mode.clone(),
                ev_offset,
            }).await {
```

- [ ] **Step 3: Update `on_file_uploaded` in `service.rs`**

Find `pub async fn on_file_uploaded` (around line 93). Update the `self.enqueue` call:

```rust
        if let Err(e) = self.enqueue(
            vec![file_path.clone()],
            cg.preset_id.clone(),
            cg.metering_mode.clone(),
            cg.ev_offset,
        ).await {
```

- [ ] **Step 4: Update `process_single_file` in `service.rs`**

Find `async fn process_single_file` (around line 261). Update the FFI call — remove `use_auto_exposure`, rename `manual_ev` to `ev_offset`, note param order change (`ev_offset` now comes before `metering_mode` in the FFI function):

```rust
    let metering_mode = task.metering_mode.clone();
    let ev_offset = task.ev_offset;

    tokio::task::spawn_blocking(move || {
        lib.process_file_with_lut(
            &input_path,
            &output_path,
            Some(&log_space),
            &lut_data,
            lensfun_path.as_deref(),
            ev_offset,
            &metering_mode,
        )
    }).await.map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;
```

- [ ] **Step 5: Update `enqueue_color_grading` command in `commands/color_grading.rs`**

```rust
#[command]
pub async fn enqueue_color_grading(
    color_grading: State<'_, ColorGradingService>,
    file_paths: Vec<String>,
    lut_id: String,
    metering_mode: String,
    ev_offset: f32,
) -> Result<(), AppError> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    color_grading.enqueue(paths, lut_id, metering_mode, ev_offset).await
}
```

- [ ] **Step 6: Update `apply_color_grading_preview` command in `commands/color_grading.rs`**

```rust
#[command]
pub async fn apply_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    enable_lens_correction: bool,
    metering_mode: String,
    ev_offset: f32,
) -> Result<String, AppError> {
    preview.apply(&lut_id, enable_lens_correction, &metering_mode, ev_offset).await
}
```

- [ ] **Step 7: Update `ColorGradingPreviewState::apply` in `preview.rs`**

Find `pub async fn apply` (around line 83). Remove `use_auto_exposure` param, rename `manual_ev` to `ev_offset`:

```rust
    pub async fn apply(
        &self,
        lut_id: &str,
        enable_lens_correction: bool,
        metering_mode: &str,
        ev_offset: f32,
    ) -> Result<String, AppError> {
```

Update the debug log and FFI call:
```rust
        tracing::debug!(lut = lut_id, ev = ev_offset, lens = enable_lens_correction, "Applying preview grading");
```

```rust
            lib.apply_preview_grading(
                &session,
                Some(log_space.as_str()),
                &lut_data,
                ev_offset,
                &metering,
                PREVIEW_JPEG_QUALITY,
                Path::new(&output_path),
            )
```

- [ ] **Step 8: Commit Rust service/command/preview changes**

```bash
git add src-tauri/src/color_grading/service.rs src-tauri/src/commands/color_grading.rs src-tauri/src/color_grading/preview.rs
git commit -m "refactor(exposure): update service, commands, and preview to use evOffset"
```

---

### Task 6: Regenerate TypeScript Types

**Files:**
- Regenerate: `src-tauri/bindings/AutoColorGradingConfig.ts`
- Regenerate: `src-tauri/bindings/ColorGradingLastUsed.ts`

- [ ] **Step 1: Run type generation**

```bash
./build.sh gen-types
```

Expected: The generated files should now have `evOffset: number` and `meteringMode: string` with no `useAutoExposure` field.

- [ ] **Step 2: Verify generated types**

Read `src-tauri/bindings/AutoColorGradingConfig.ts` — should contain:
```typescript
export interface AutoColorGradingConfig { enabled: boolean; presetId: string; evOffset: number; meteringMode: string; }
```

Read `src-tauri/bindings/ColorGradingLastUsed.ts` — should contain:
```typescript
export interface ColorGradingLastUsed { presetId: string; evOffset: number; meteringMode: string; }
```

`src/types/index.ts` re-exports are already correct (lines 30–31) — no change needed.

- [ ] **Step 3: Commit regenerated types**

```bash
git add src-tauri/bindings/AutoColorGradingConfig.ts src-tauri/bindings/ColorGradingLastUsed.ts
git commit -m "refactor(exposure): regenerate TS bindings with evOffset"
```

---

### Task 7: TypeScript UI Components

**Files:**
- Modify: `src/components/ExposureConfigSection.tsx`
- Modify: `src/components/ColorGradingDialog.tsx`
- Modify: `src/components/AutoColorGradingConfigCard.tsx`
- Modify: `src/components/PreviewWindow.tsx`
- Modify: `src/components/GalleryCard.tsx`
- Modify: `src/hooks/useColorGradingProgress.ts`
- Modify: `src/types/global.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Rewrite `ExposureConfigSection.tsx`**

Replace the entire file content with:

```tsx
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// TODO: Extract Chinese UI strings for i18n when locale support is added

import { Select } from './ui/Select';
import { METERING_MODES } from '../constants/color-grading';

interface ExposureConfigSectionProps {
  meteringMode: string;
  onMeteringModeChange: (v: string) => void;
  evOffset: number;
  onEvOffsetChange: (v: number) => void;
  disabled?: boolean;
}

export function ExposureConfigSection({
  meteringMode,
  onMeteringModeChange,
  evOffset,
  onEvOffsetChange,
  disabled = false,
}: ExposureConfigSectionProps) {
  return (
    <>
      <div className="border-t border-gray-100 pt-3" />
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700">测光模式</label>
        <Select
          value={meteringMode}
          options={METERING_MODES}
          onChange={onMeteringModeChange}
          disabled={disabled}
        />
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="block text-sm font-medium text-gray-700">曝光偏移</label>
          <span className="text-sm font-mono text-gray-500">
            {evOffset > 0 ? '+' : ''}{evOffset.toFixed(1)} EV
          </span>
        </div>
        <input
          type="range"
          min={-5.0}
          max={5.0}
          step={0.1}
          value={evOffset}
          onChange={(e) => onEvOffsetChange(parseFloat(e.target.value))}
          disabled={disabled}
          className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600 disabled:opacity-50"
        />
        <div className="flex justify-between text-xs text-gray-400">
          <span>-5.0</span>
          <span>0</span>
          <span>+5.0</span>
        </div>
      </div>
    </>
  );
}
```

- [ ] **Step 2: Update `ColorGradingDialog.tsx`**

Key changes: remove `useAutoExposure` state and all references, rename `manualEv` → `evOffset`, update `onConfirm` signature.

Replace the interface:
```tsx
interface ColorGradingDialogProps {
  isOpen: boolean;
  colorGradingPresets: ColorGradingPreset[];
  onConfirm: (lutId: string, meteringMode: string, evOffset: number) => void;
  onCancel: () => void;
}
```

Replace state declarations (lines 35–39):
```tsx
  const [selectedId, setSelectedId] = useState('');
  const [meteringMode, setMeteringMode] = useState('highlight-safe');
  const [evOffset, setEvOffset] = useState(0.0);
  const [syncToAuto, setSyncToAuto] = useState(false);
```

Replace the useEffect (lines 41–52):
```tsx
  useEffect(() => {
    if (isOpen) {
      const lastUsed = draft?.colorGradingLastUsed;
      const initialPreset = lastUsed?.presetId || colorGradingPresets[0]?.id || 'fujifilm-provia';
      setSelectedId(initialPreset);
      setMeteringMode(lastUsed?.meteringMode ?? 'highlight-safe');
      setEvOffset(lastUsed?.evOffset ?? 0.0);
      setSyncToAuto(false);
    }
  // draft intentionally excluded — effect should only run on mount/dialog open
  }, [isOpen, colorGradingPresets]);
```

Replace `handleConfirm`:
```tsx
  const handleConfirm = () => {
    if (!selectedId) return;

    updateDraft(d => ({
      ...d,
      colorGradingLastUsed: {
        presetId: selectedId,
        meteringMode,
        evOffset,
      },
      ...(syncToAuto && d.autoColorGrading ? {
        autoColorGrading: {
          ...d.autoColorGrading,
          presetId: selectedId,
          meteringMode,
          evOffset,
        },
      } : {}),
    }));

    onConfirm(selectedId, meteringMode, evOffset);
  };
```

Replace the `ExposureConfigSection` usage:
```tsx
        <ExposureConfigSection
          meteringMode={meteringMode}
          onMeteringModeChange={setMeteringMode}
          evOffset={evOffset}
          onEvOffsetChange={setEvOffset}
        />
```

Remove the unused `ToggleSwitch` import.

- [ ] **Step 3: Update `AutoColorGradingConfigCard.tsx`**

Remove `handleExposureToggle` function (lines 50–58). Update the `ExposureConfigSection` usage (lines 114–122):

```tsx
            <ExposureConfigSection
              meteringMode={draft.autoColorGrading.meteringMode}
              onMeteringModeChange={handleMeteringModeChange}
              evOffset={draft.autoColorGrading.evOffset}
              onEvOffsetChange={handleEvOffsetChange}
              disabled={isLoading}
            />
```

Rename `handleManualEvChange` to `handleEvOffsetChange`:
```tsx
  const handleEvOffsetChange = (ev: number) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        evOffset: ev,
      },
    }));
  };
```

- [ ] **Step 4: Update `PreviewWindow.tsx`**

Find `handleColorGradingConfirm` (around line 202). Update signature and `enqueueColorGrading` call:

```tsx
  const handleColorGradingConfirm = useCallback(async (lutId: string, meteringMode: string, evOffset: number) => {
    if (!imagePath) return;
    setShowColorGradingDialog(false);
    await enqueueColorGrading([imagePath], lutId, meteringMode, evOffset);
  }, [imagePath]);
```

- [ ] **Step 5: Update `GalleryCard.tsx`**

Find `handleColorGradingConfirm` (around line 153). Update:

```tsx
  const handleColorGradingConfirm = useCallback(async (lutId: string, meteringMode: string, evOffset: number) => {
    setShowColorGradingDialog(false);
    const filePaths = Array.from(selectedIds)
      .map(id => pager.items.find(item => item.mediaId === id))
      .filter((item): item is NonNullable<typeof item> => item != null)
      .map(item => window.ImageViewerAndroid?.resolveFilePath?.(item.uri) ?? item.uri);
    if (filePaths.length > 0) {
      await enqueueColorGrading(filePaths, lutId, meteringMode, evOffset);
    }
  }, [selectedIds, pager.items]);
```

- [ ] **Step 6: Update `useColorGradingProgress.ts`**

Replace the `enqueueColorGrading` function (lines 85–93):

```typescript
export async function enqueueColorGrading(
  files: string[],
  lutId: string,
  meteringMode: string = 'highlight-safe',
  evOffset: number = 0.0,
): Promise<void> {
  await invoke('enqueue_color_grading', { filePaths: files, lutId, meteringMode, evOffset });
}
```

- [ ] **Step 7: Update `types/global.ts`**

Find `__tauriTriggerColorGrading` (around line 339). Update:

```typescript
    __tauriTriggerColorGrading?: (filePath: string, lutId: string, meteringMode: string, evOffset: number, syncToAuto: boolean) => Promise<void>;
```

- [ ] **Step 8: Update `App.tsx`**

Find `__tauriTriggerColorGrading` assignment (around line 79). Update:

```typescript
    w.__tauriTriggerColorGrading = async (filePath: string, lutId: string, meteringMode: string, evOffset: number, syncToAuto: boolean) => {
      const { enqueueColorGrading } = await import('./hooks/useColorGradingProgress');
      await enqueueColorGrading([filePath], lutId, meteringMode, evOffset);

      updateDraft(d => ({
        ...d,
        colorGradingLastUsed: {
          presetId: lutId,
          meteringMode,
          evOffset,
        },
        ...(syncToAuto && d.autoColorGrading ? {
          autoColorGrading: {
            ...d.autoColorGrading,
            presetId: lutId,
            meteringMode,
            evOffset,
          },
        } : {}),
      }));
    };
```

- [ ] **Step 9: Commit TypeScript changes**

```bash
git add src/components/ExposureConfigSection.tsx src/components/ColorGradingDialog.tsx src/components/AutoColorGradingConfigCard.tsx src/components/PreviewWindow.tsx src/components/GalleryCard.tsx src/hooks/useColorGradingProgress.ts src/types/global.ts src/App.tsx
git commit -m "refactor(exposure): update TS UI to use metering+evOffset, remove auto toggle"
```

---

### Task 8: Android Kotlin + HTML

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/controllers/WebViewOverlayController.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt`
- Modify: `src-tauri/gen/android/app/src/main/assets/color_grading_dialog.html`

- [ ] **Step 1: Update `NativeColorGradingBridge.onConfirm` in `WebViewOverlayController.kt`**

Find `onConfirm` (line 30). Remove `useAutoExposure`, rename `manualEv` to `evOffset`:

```kotlin
    @JavascriptInterface
    fun onConfirm(lutId: String, meteringMode: String, evOffset: Float, syncToAuto: Boolean) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            activity.overlayController.dismissColorGrading()
            activity.dispatchColorGrading(
                filePath, lutId, meteringMode, evOffset, syncToAuto,
            )
        }
    }
```

- [ ] **Step 2: Update `showColorGrading` in `WebViewOverlayController.kt`**

Find `showColorGrading` (line 106). Remove `lastUsedAutoExposure` param, remove `{{AUTO_EXPOSURE_CHECKED}}` replacement:

```kotlin
    fun showColorGrading(
        filePath: String,
        autoColorGradingEnabled: Boolean,
        presets: List<Pair<String, String>>,
        lastUsedPresetId: String? = null,
        lastUsedMeteringMode: String? = null,
        lastUsedEvOffset: Float? = null,
    ) {
        lockOrientation()
        val rootView = activity.findViewById<FrameLayout>(android.R.id.content)

        dismissColorGrading()

        val initialPresetId = lastUsedPresetId?.takeIf { id -> presets.any { it.first == id } } ?: presets.firstOrNull()?.first
            ?: run {
                Log.w(TAG, "No color grading presets available")
                android.widget.Toast.makeText(activity, "调色预设尚未加载", android.widget.Toast.LENGTH_SHORT).show()
                return
            }
        val initialPresetLabel = presets.find { it.first == initialPresetId }?.second ?: presets.first().second
        val presetOptionsHtml = presets.joinToString("") { (value, label) ->
            """<div class="dropdown-opt${if (value == initialPresetId) " selected" else ""}" data-value="$value">$label</div>"""
        }

        val evValue = lastUsedEvOffset ?: 0.0f
        val evDisplay = if (evValue > 0) "+${"%.1f".format(evValue)} EV" else "${"%.1f".format(evValue)} EV"
        val initialMetering = lastUsedMeteringMode ?: "highlight-safe"

        val saveToggleHtml = if (autoColorGradingEnabled) {
            """<div class="save-toggle" onclick="toggleSync()">
                    <div class="toggle" id="syncToggle"></div>
                    <span>同步到自动调色</span>
                  </div>"""
        } else ""

        val html = activity.assets.open("color_grading_dialog.html").bufferedReader().use { it.readText() }
            .replace("{{FIRST_ID}}", initialPresetId)
            .replace("{{FIRST_LABEL}}", initialPresetLabel)
            .replace("{{PRESET_OPTIONS}}", presetOptionsHtml)
            .replace("{{SAVE_TOGGLE}}", saveToggleHtml)
            .replace("{{EV_VALUE}}", evValue.toString())
            .replace("{{EV_DISPLAY}}", evDisplay)
            .replace("{{SELECTED_METERING}}", initialMetering)

        val webView = WebView(activity).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            setBackgroundColor(0)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            addJavascriptInterface(NativeColorGradingBridge(activity, filePath), "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        colorGradingWebView = webView
    }
```

- [ ] **Step 3: Update `buildColorGradingArgsJson` in `ImageViewerActivity.kt`**

Find `buildColorGradingArgsJson` (line 122). Remove `useAutoExposure`, rename `manualEv` to `evOffset`:

```kotlin
        @JvmStatic
        fun buildColorGradingArgsJson(
            filePath: String, lutId: String,
            meteringMode: String, evOffset: Float, syncToAuto: Boolean,
        ): String {
            return JSONArray().apply {
                put(filePath); put(lutId)
                put(meteringMode); put(evOffset); put(syncToAuto)
            }.toString()
        }
```

- [ ] **Step 4: Update `dispatchColorGrading` in `ImageViewerActivity.kt`**

Find `dispatchColorGrading` (line 525). Remove `useAutoExposure`, rename `manualEv` to `evOffset`:

```kotlin
    internal fun dispatchColorGrading(
        filePath: String, lutId: String,
        meteringMode: String, evOffset: Float, syncToAuto: Boolean,
    ) {
        val mainActivity = MainActivity.instance ?: run {
            Log.w(TAG, "MainActivity not available for color grading"); return
        }
        val args = buildColorGradingArgsJson(filePath, lutId, meteringMode, evOffset, syncToAuto)
```

- [ ] **Step 5: Update last-used config reading in `ImageViewerActivity.kt`**

Find the code that reads last-used config from WebView (around line 514–520). Remove `useAutoExposure`, rename `manualEv` to `evOffset`:

```kotlin
                overlayController.showColorGrading(
                    filePath, enabled, presets,
                    lastUsed?.optString("presetId"),
                    lastUsed?.optString("meteringMode"),
                    lastUsed?.optDouble("evOffset", 0.0)?.toFloat(),
                )
```

- [ ] **Step 6: Rewrite `color_grading_dialog.html`**

Replace the entire file. Key changes: remove auto exposure toggle row, always show metering + EV offset slider, update `onConfirm` JS function:

```html
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.5);
    display: flex; align-items: center; justify-content: center;
    padding: 16px; z-index: 50;
  }
  .card {
    background: #fff; border-radius: 12px; width: 100%; max-width: 448px;
    box-shadow: 0 25px 50px -12px rgba(0,0,0,0.25);
    display: flex; flex-direction: column; max-height: 90vh;
  }
  .header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 16px; border-bottom: 1px solid #e5e7eb;
  }
  .title-group { display: flex; flex-direction: column; }
  .title { font-size: 18px; font-weight: 600; color: #111827; }
  .subtitle { font-size: 14px; color: #6b7280; margin-top: 2px; }
  .close-btn {
    padding: 8px; border: none; background: none; cursor: pointer;
    color: #9ca3af; border-radius: 8px;
  }
  .close-btn:hover { color: #4b5563; background: #f3f4f6; }
  .close-btn svg { width: 20px; height: 20px; }
  .content { padding: 16px; overflow: visible; }
  .field-group { margin-bottom: 12px; }
  .field-group:last-child { margin-bottom: 0; }
  .field-label { font-size: 14px; font-weight: 500; color: #374151; margin-bottom: 4px; }
  .dropdown { position: relative; }
  .dropdown-btn {
    width: 100%; padding: 8px 12px; border: 1px solid #e5e7eb;
    border-radius: 8px; font-size: 14px; color: #374151;
    background: #fff; outline: none; cursor: pointer;
    display: flex; align-items: center; justify-content: space-between;
    text-align: left; -webkit-user-select: none; user-select: none;
    -webkit-tap-highlight-color: transparent;
  }
  .dropdown-btn:hover { border-color: #d1d5db; }
  .dropdown-btn .chevron {
    width: 16px; height: 16px; color: #9ca3af;
    transition: transform 0.2s; flex-shrink: 0;
  }
  .dropdown-btn.open .chevron { transform: rotate(180deg); }
  .dropdown-panel {
    position: absolute; left: 0; right: 0;
    margin-top: 4px; background: #fff; border: 1px solid #e5e7eb;
    border-radius: 8px; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.1), 0 4px 6px -4px rgba(0,0,0,0.1);
    padding: 4px 0; z-index: 10; max-height: 240px; overflow-y: auto;
    opacity: 0; transform: scaleY(0.95) translateY(-4px);
    transform-origin: top; pointer-events: none;
    transition: opacity 0.15s ease, transform 0.15s ease;
  }
  .dropdown-panel.open {
    opacity: 1; transform: scaleY(1) translateY(0);
    pointer-events: auto;
  }
  .dropdown-opt {
    padding: 8px 12px; font-size: 14px;
    color: #374151; cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }
  .dropdown-opt:hover { background: #f9fafb; }
  .dropdown-opt.selected { background: #eff6ff; color: #1d4ed8; font-weight: 500; }
  .divider { border-top: 1px solid #f3f4f6; margin: 12px 0; }
  .slider-header {
    display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;
  }
  .slider-value { font-size: 13px; font-family: monospace; color: #6b7280; }
  input[type="range"] {
    -webkit-appearance: none; width: 100%; height: 6px;
    background: #e5e7eb; border-radius: 3px; outline: none;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 20px; height: 20px;
    background: #2563eb; border-radius: 50%; cursor: pointer;
  }
  .slider-labels {
    display: flex; justify-content: space-between;
    font-size: 11px; color: #9ca3af; margin-top: 4px;
  }
  .footer {
    display: flex; align-items: center; justify-content: space-between;
    padding: 16px; border-top: 1px solid #e5e7eb;
  }
  .save-toggle { display: flex; align-items: center; gap: 8px; cursor: pointer; }
  .save-toggle span { font-size: 14px; color: #374151; font-weight: 500; }
  .toggle {
    position: relative; width: 44px; height: 24px;
    background: #d1d5db; border-radius: 12px;
    transition: background 0.2s; cursor: pointer; flex-shrink: 0;
  }
  .toggle.on { background: #2563eb; }
  .toggle::after {
    content: ''; position: absolute;
    width: 16px; height: 16px; background: #fff;
    border-radius: 50%; top: 4px; left: 4px;
    transition: transform 0.2s;
  }
  .toggle.on::after { transform: translateX(20px); }
  .actions { display: flex; gap: 8px; margin-left: auto; }
  .btn {
    padding: 8px 16px; border-radius: 8px; font-size: 14px;
    font-weight: 500; border: none; cursor: pointer;
  }
  .btn-cancel { background: #f3f4f6; color: #374151; }
  .btn-cancel:hover { background: #e5e7eb; }
  .btn-confirm { background: #2563eb; color: #fff; }
  .btn-confirm:hover { background: #1d4ed8; }
  .header-icon { color: #7c3aed; flex-shrink: 0; }
</style>
</head>
<body>
<div class="overlay" onclick="if(event.target===this)NativeBridge.onCancel()">
  <div class="card">
    <div class="header">
      <div style="display:flex;align-items:center;gap:12px">
        <div style="width:40px;height:40px;background:#f3f4f6;border-radius:8px;display:flex;align-items:center;justify-content:center"><svg class="header-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="17.5" cy="10.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="8.5" cy="7.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="6.5" cy="12.5" r="1.5" fill="currentColor" stroke="none"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/></svg></div>
        <div class="title-group">
          <div class="title">调色</div>
          <div class="subtitle">使用胶片模拟调色处理 RAW 照片</div>
        </div>
      </div>
      <button class="close-btn" onclick="NativeBridge.onCancel()">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
    <div class="content">
      <div class="field-group">
        <div class="field-label">调色预设</div>
        <div class="dropdown" id="presetDropdown">
          <button class="dropdown-btn" type="button" onclick="toggleDropdown()">
            <span id="presetLabel">{{FIRST_LABEL}}</span>
            <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
          </button>
          <div class="dropdown-panel" id="presetPanel">{{PRESET_OPTIONS}}</div>
        </div>
      </div>
      <div class="divider"></div>
      <div class="field-group">
        <div class="field-label">测光模式</div>
        <div class="dropdown" id="meteringDropdown">
          <button class="dropdown-btn" type="button" onclick="toggleMeteringDropdown()">
            <span id="meteringLabel">高光保护</span>
            <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
          </button>
          <div class="dropdown-panel" id="meteringPanel">
            <div class="dropdown-opt selected" data-value="highlight-safe" onclick="selectMetering(this)">高光保护</div>
            <div class="dropdown-opt" data-value="matrix" onclick="selectMetering(this)">矩阵测光</div>
            <div class="dropdown-opt" data-value="center-weighted" onclick="selectMetering(this)">中央重点测光</div>
            <div class="dropdown-opt" data-value="average" onclick="selectMetering(this)">平均测光</div>
            <div class="dropdown-opt" data-value="hybrid" onclick="selectMetering(this)">混合测光</div>
          </div>
        </div>
      </div>
      <div class="field-group" style="margin-top:12px">
        <div class="slider-header">
          <span class="field-label" style="margin-bottom:0">曝光偏移</span>
          <span class="slider-value" id="evValue">{{EV_DISPLAY}}</span>
        </div>
        <input type="range" id="evSlider" min="-5.0" max="5.0" step="0.1" value="{{EV_VALUE}}" oninput="onEvChange()">
        <div class="slider-labels"><span>-5.0</span><span>0</span><span>+5.0</span></div>
      </div>
    </div>
    <div class="footer">
      {{SAVE_TOGGLE}}
      <div class="actions">
        <button class="btn btn-cancel" onclick="NativeBridge.onCancel()">取消</button>
        <button class="btn btn-confirm" onclick="onConfirm()">应用</button>
      </div>
    </div>
  </div>
</div>
<script>
  var selectedPreset = '{{FIRST_ID}}';
  function toggleDropdown() {
    var panel = document.getElementById('presetPanel');
    var btn = panel.previousElementSibling;
    var isOpen = panel.classList.contains('open');
    if (isOpen) { panel.classList.remove('open'); btn.classList.remove('open'); }
    else { panel.classList.add('open'); btn.classList.add('open'); }
  }
  function closeDropdown() {
    var panel = document.getElementById('presetPanel');
    var btn = panel.previousElementSibling;
    panel.classList.remove('open'); btn.classList.remove('open');
  }
  document.getElementById('presetPanel').addEventListener('click', function(e) {
    var opt = e.target.closest('.dropdown-opt');
    if (!opt) return;
    selectedPreset = opt.getAttribute('data-value');
    document.getElementById('presetLabel').textContent = opt.textContent;
    var allOpts = this.querySelectorAll('.dropdown-opt');
    for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
    opt.classList.add('selected');
    closeDropdown();
  });
  document.addEventListener('click', function(e) {
    if (!document.getElementById('presetDropdown').contains(e.target)) closeDropdown();
  });
  function onEvChange() {
    var val = parseFloat(document.getElementById('evSlider').value);
    document.getElementById('evValue').textContent = (val > 0 ? '+' : '') + val.toFixed(1) + ' EV';
  }
  var selectedMetering = '{{SELECTED_METERING}}';
  function toggleMeteringDropdown() {
    var panel = document.getElementById('meteringPanel');
    var btn = panel.previousElementSibling;
    var isOpen = panel.classList.contains('open');
    if (isOpen) { panel.classList.remove('open'); btn.classList.remove('open'); }
    else { panel.classList.add('open'); btn.classList.add('open'); }
  }
  function closeMeteringDropdown() {
    var panel = document.getElementById('meteringPanel');
    var btn = panel.previousElementSibling;
    panel.classList.remove('open'); btn.classList.remove('open');
  }
  function selectMetering(opt) {
    selectedMetering = opt.getAttribute('data-value');
    document.getElementById('meteringLabel').textContent = opt.textContent;
    var allOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
    for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
    opt.classList.add('selected');
    closeMeteringDropdown();
  }
  document.addEventListener('click', function(e) {
    if (!document.getElementById('meteringDropdown').contains(e.target)) closeMeteringDropdown();
  });
  var syncToAuto = false;
  function toggleSync() {
    syncToAuto = !syncToAuto;
    document.getElementById('syncToggle').className = 'toggle' + (syncToAuto ? ' on' : '');
  }
  function onConfirm() {
    var ev = parseFloat(document.getElementById('evSlider').value);
    NativeBridge.onConfirm(selectedPreset, selectedMetering, ev, syncToAuto);
  }
  (function() {
    var meteringOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
    for (var i = 0; i < meteringOpts.length; i++) {
      if (meteringOpts[i].getAttribute('data-value') === selectedMetering) {
        meteringOpts[i].classList.add('selected');
        document.getElementById('meteringLabel').textContent = meteringOpts[i].textContent;
      } else {
        meteringOpts[i].classList.remove('selected');
      }
    }
  })();
</script>
</body>
</html>
```

- [ ] **Step 7: Commit Android changes**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/controllers/WebViewOverlayController.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt src-tauri/gen/android/app/src/main/assets/color_grading_dialog.html
git commit -m "refactor(exposure): update Android UI to use metering+evOffset, remove auto toggle"
```

---

### Task 9: Build Verification

- [ ] **Step 1: Build both platforms**

```bash
./build.sh windows android
```

Expected: Build succeeds for both Windows and Android with no compilation errors.

- [ ] **Step 2: Run Rust tests**

```bash
cargo.exe test --manifest-path src-tauri/Cargo.toml
```

Expected: All existing tests pass (including the `should_auto_color_grade` tests in `service.rs`).

- [ ] **Step 3: Final commit if any fixes were needed**

If any build fixes were needed during verification, commit them:

```bash
git add -A
git commit -m "fix: build fixes from exposure refactor"
```
