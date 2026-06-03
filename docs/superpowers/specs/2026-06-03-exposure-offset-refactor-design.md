# Exposure Offset Refactor Design

## Summary

Refactor the exposure system from a boolean auto/manual toggle to a unified model: always auto-meter with a user-selected metering mode, plus an EV offset slider. Remove `useAutoExposure` entirely from all layers. Rename `manualEv`/`manual_ev` to `evOffset`/`ev_offset` to reflect the new semantics (offset from auto-metered exposure rather than absolute manual value).

## Motivation

The current design forces users to choose between two mutually exclusive modes via a toggle:

- **Auto mode**: selects a metering algorithm, applies auto-metered exposure, ignores EV slider
- **Manual mode**: applies a raw EV value directly, ignores metering

In practice, users always want auto metering as a baseline, with the ability to fine-tune via an offset. The toggle adds unnecessary complexity and the "manual" mode discards useful metering data.

## New Behavior

- **Always** use the selected metering mode to compute auto exposure
- **Always** show an EV offset slider (range -5.0 to +5.0, step 0.1) that adjusts relative to the auto-metered value
- No toggle — metering mode selector and EV offset slider are always visible simultaneously

## Configuration Migration

**Old config fields are discarded, not migrated.** The semantics differ (absolute EV vs. offset from auto), so inheriting old values would produce unexpected results. Users re-configure once after upgrade. Serde `#[serde(default)]` ensures clean defaults on first load after upgrade.

## Changes by Layer

### 1. C API (Breaking Change)

**Files**: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp`, `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h`

Remove `useAutoExposure` parameter from all public functions. Rename `manualEv` to `evOffset`.

Affected functions:
- `raProcessFile`
- `raProcessFileWithLUT`
- `raProcessToBuffer`
- `raApplyPreviewGrading`
- Internal helpers: `runPipeline`, `runPipelineWithLUT`, `runGradingOnly`

Exposure logic in all three internal helpers changes from:
```cpp
if (useAutoExposure) {
    float gain = computeAutoGain(img, mode);
    img.applyGain(gain);
} else {
    img.applyGain(std::pow(2.0f, manualEv));
}
```
To:
```cpp
std::string mode(metering ? metering : "matrix");
if (!isMeteringModeSupported(mode)) {
    setError("Unsupported metering mode: " + mode);
    return RA_ERR_INVALID_PARAM;
}
float gain = computeAutoGain(img, mode);
img.applyGain(gain * std::pow(2.0f, evOffset));
```

### 2. Rust FFI Layer

**File**: `src-tauri/src/color_grading/ffi.rs`

- Remove `use_auto_exposure: bool` from `process_file_with_lut()` and `apply_preview_grading()`
- Rename `manual_ev: f32` to `ev_offset: f32`
- Update `RaProcessFileWithLUTFn` and `RaApplyPreviewGradingFn` type aliases to match new C signatures
- Remove the `if use_auto_exposure { 1 } else { 0 }` conversion; always pass `evOffset` directly

### 3. Rust Structs

**File**: `src-tauri/src/config.rs`

`AutoColorGradingConfig`:
- Remove field `use_auto_exposure: bool`
- Keep existing `#[serde(alias = "presetLutId")]` on `preset_id` (unrelated to this refactor)
- Rename `manual_ev: f32` to `ev_offset: f32`
- Update doc comments

`ColorGradingLastUsed`:
- Remove field `use_auto_exposure: bool`
- Rename `manual_ev: f32` to `ev_offset: f32`

Both structs keep `#[serde(rename_all = "camelCase", default)]`. The `default` impl is updated to reflect new fields.

**File**: `src-tauri/src/color_grading/service.rs`

`ColorGradingTask`:
- Remove `use_auto_exposure: bool`
- Rename `manual_ev: f32` to `ev_offset: f32`

### 4. Rust Commands

**File**: `src-tauri/src/commands/color_grading.rs`

`enqueue_color_grading`:
- Remove `use_auto_exposure: bool` param
- Rename `manual_ev: f32` to `ev_offset: f32`

`apply_color_grading_preview`:
- Same changes

### 5. Rust Service & Preview

**File**: `src-tauri/src/color_grading/service.rs`

- `ColorGradingService::enqueue()`: remove `use_auto_exposure`, rename `manual_ev` → `ev_offset`
- `process_single_file()`: propagate changes
- `on_file_uploaded()`: read `ev_offset` from config instead of `manual_ev`/`use_auto_exposure`

**File**: `src-tauri/src/color_grading/preview.rs`

- `ColorGradingPreviewState::apply()`: remove `use_auto_exposure`, rename `manual_ev` → `ev_offset`

### 6. TypeScript Types

Run `./build.sh gen-types` after Rust struct changes. Generated types will reflect:
- `AutoColorGradingConfig`: `meteringMode`, `evOffset` (no `useAutoExposure`)
- `ColorGradingLastUsed`: `meteringMode`, `evOffset` (no `useAutoExposure`)

Update `src/types/index.ts` re-exports.

### 7. TypeScript UI Components

**File**: `src/components/ExposureConfigSection.tsx` — rewrite:
- Remove `ToggleSwitch` import and usage
- Remove `useAutoExposure` / `onAutoExposureChange` props
- Rename `manualEv` / `onManualEvChange` to `evOffset` / `onEvOffsetChange`
- Always render both metering mode selector and EV offset slider
- Change slider label from "曝光补偿" to "曝光偏移"

**File**: `src/components/ColorGradingDialog.tsx`:
- Remove `useAutoExposure` state
- Rename `manualEv` state to `evOffset`
- Update `onConfirm` callback signature: `(lutId, meteringMode, evOffset)`
- Update `colorGradingLastUsed` save format

**File**: `src/components/AutoColorGradingConfigCard.tsx`:
- Remove `handleExposureToggle`
- Update `ExposureConfigSection` props: remove `useAutoExposure`/`onAutoExposureChange`, rename `manualEv` → `evOffset`

**File**: `src/components/PreviewWindow.tsx`:
- Update `handleColorGradingConfirm` signature

**File**: `src/components/GalleryCard.tsx`:
- Update `handleColorGradingConfirm` signature

**File**: `src/hooks/useColorGradingProgress.ts`:
- `enqueueColorGrading()`: remove `useAutoExposure` param, rename `manualEv` → `evOffset`

**File**: `src/types/global.ts`:
- `__tauriTriggerColorGrading`: remove `useAutoExposure`, rename `manualEv` → `evOffset`

**File**: `src/App.tsx`:
- Update bridge handler to match new signature and field names

### 8. Android Kotlin

**File**: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/controllers/WebViewOverlayController.kt`

`showColorGrading()`:
- Remove `lastUsedAutoExposure: Boolean?` param
- Remove `{{AUTO_EXPOSURE_CHECKED}}` template replacement

`NativeColorGradingBridge.onConfirm()`:
- Remove `useAutoExposure: Boolean` param
- Rename `manualEv` → `evOffset`

**File**: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt`

- `buildColorGradingArgsJson()`: remove `useAutoExposure`, rename `manualEv` → `evOffset`
- `dispatchColorGrading()`: same
- Update last-used config reading to use `evOffset` (ignore old `useAutoExposure`/`manualEv`)

**File**: `src-tauri/gen/android/app/src/main/assets/color_grading_dialog.html`:

- Remove the auto exposure toggle row entirely (lines 170-178)
- Always show metering mode selector and EV offset slider
- Rename slider label to "曝光偏移"
- Update `onConfirm()` JS function: remove `autoExp` param, pass `ev` directly
- Remove `onExposureToggle()` function and init block visibility toggling
- Remove `{{AUTO_EXPOSURE_CHECKED}}` template placeholder

## Files to Modify (Complete List)

| File | Layer |
|------|-------|
| `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` | C++ |
| `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` | C++ header |
| `src-tauri/src/color_grading/ffi.rs` | Rust FFI |
| `src-tauri/src/config.rs` | Rust config structs |
| `src-tauri/src/commands/color_grading.rs` | Rust Tauri commands |
| `src-tauri/src/color_grading/service.rs` | Rust service |
| `src-tauri/src/color_grading/preview.rs` | Rust preview |
| `src/components/ExposureConfigSection.tsx` | TS UI |
| `src/components/ColorGradingDialog.tsx` | TS UI |
| `src/components/AutoColorGradingConfigCard.tsx` | TS UI |
| `src/components/PreviewWindow.tsx` | TS UI |
| `src/components/GalleryCard.tsx` | TS UI |
| `src/hooks/useColorGradingProgress.ts` | TS hook |
| `src/types/global.ts` | TS types |
| `src/App.tsx` | TS app |
| `src/types/index.ts` | TS re-exports |
| `src-tauri/gen/android/.../controllers/WebViewOverlayController.kt` | Kotlin |
| `src-tauri/gen/android/.../ImageViewerActivity.kt` | Kotlin |
| `src-tauri/gen/android/.../assets/color_grading_dialog.html` | Android HTML |

## Verification

- `./build.sh windows android` — both platforms build successfully
- Manual test: open color grading dialog → metering mode and EV offset are both visible
- Manual test: change EV offset → preview reflects the change
- Manual test: auto color grading processes files with correct exposure
