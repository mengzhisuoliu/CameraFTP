# Image Viewer Menu & Color Grading for Android Native

## Problem

The Android native `ImageViewerActivity` bottom bar currently has three action buttons (AI Edit, Rotate, Delete). Adding a fourth button for Color Grading would crowd the bar and reduce space for filename/EXIF info. Color Grading also needs to be disabled for non-RAW images.

## Solution

Replace the AI Edit button with an overflow menu button. Tapping it shows a popup with both AI Edit and Color Grading options. Color Grading is grayed out when the current image is not RAW. Selecting Color Grading opens a WebView overlay dialog (same pattern as AI Edit) with a film simulation preset dropdown.

## Design

### 1. Bottom Bar Change

Replace `btn_ai_edit` (ImageButton) with `btn_menu` (ImageButton) using a three-dot vertical overflow icon. The button sits in the same position. Rotate and Delete buttons remain unchanged.

**Affected layouts:**
- `res/layout/activity_image_viewer.xml`
- `res/layout-land/activity_image_viewer.xml`

### 2. Popup Menu

A `PopupWindow` anchored above the menu button, styled to match the dark bottom bar aesthetic (rounded corners, `#1F2937` background, white text). Contains two items:

| Item | Icon | Behavior |
|------|------|----------|
| AI修图 | Sparkle (reuse `ic_ai_edit`) | Calls existing `triggerAiEditForCurrentImage()` |
| 调色 | Palette (`ic_color_grading`) | Opens Color Grading dialog; **disabled** if current image is not RAW |

The popup dismisses on outside touch or item selection.

### 3. RAW Detection

Use Android's `ContentResolver.getType(uri)` to get the MIME type from MediaStore. RAW files have MIME types starting with `image/x-` (e.g., `image/x-nikon-nef`), while standard images have `image/jpeg`, `image/heif`, etc.

```kotlin
private fun isRawImage(uriString: String): Boolean {
    val uri = Uri.parse(uriString)
    val mimeType = contentResolver.getType(uri)
    return mimeType?.startsWith("image/x-") == true
}
```

This is the same approach used by the existing frontend (`GalleryCard.tsx: item?.mimeType?.startsWith('image/x-')`).

### 4. Color Grading WebView Dialog

Follows the exact same pattern as `showPromptWebViewOverlay()` for AI Edit:

- **Title:** 调色
- **Subtitle:** 使用胶片模拟调色处理 RAW 照片
- **Icon:** Palette SVG in violet (`#7c3aed`)
- **Content:** Single dropdown with 11 Fujifilm film simulation presets (same list as WebView version)
- **Buttons:** 取消 / 应用
- **NativeBridge callbacks:** `onConfirm(lutId)`, `onCancel()`
- **Style:** Same white card, rounded corners, drop shadow as AI Edit dialog

Preset list (hardcoded in HTML to match `presets.rs`):

| ID | Display Name |
|----|-------------|
| acros | ACROS |
| astia | Astia |
| classic-chrome | Classic Chrome |
| classic-neg | Classic Neg |
| eterna | ETERNA |
| eterna-bb | ETERNA Bleach Bypass |
| pro-neg-std | PRO Neg. Std |
| provia | Provia |
| reala-ace | REALA ACE |
| velvia | Velvia |
| flog2c-709 | F-Log2C → Rec.709 |

### 5. Processing Trigger

On confirm, the dialog calls into the main WebView via `evaluateJavascript` to invoke the Tauri command, matching the AI Edit dispatch pattern:

```kotlin
val js = "(function(){" +
    "if(window.__tauriTriggerColorGrading){" +
    "window.__tauriTriggerColorGrading('$filePath','$lutId');" +
    "return 'ok';}" +
    "return 'no_handler';})();"
```

A new `window.__tauriTriggerColorGrading` function is registered in the frontend. It calls `invoke('enqueue_color_grading', { filePaths: [filePath], lutId })` and uses the existing `useColorGradingProgress` Zustand store for progress tracking.

The existing color grading progress bar in the WebView will display processing status. The image viewer does not need its own progress bar for color grading since the WebView's progress overlay is already visible and functional.

### 6. Image Viewer State Update

After the menu is dismissed and an action is triggered, the bottom bar remains visible. The toggle behavior (tap to show/hide) is unchanged.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `res/layout/activity_image_viewer.xml` | Modify | Replace `btn_ai_edit` with `btn_menu` |
| `res/layout-land/activity_image_viewer.xml` | Modify | Same change for landscape |
| `res/drawable/ic_menu_overflow.xml` | Create | Three-dot vertical overflow icon |
| `res/drawable/ic_color_grading.xml` | Create | Palette/paintbrush icon |
| `res/drawable/menu_popup_bg.xml` | Create | Dark rounded background for popup |
| `res/layout/popup_image_menu.xml` | Create | Popup menu layout with two items |
| `ImageViewerActivity.kt` | Modify | Add menu popup, color grading dialog, RAW detection, update button bindings |
| Frontend JS handler | Modify | Add `window.__tauriTriggerColorGrading` function |

## Out of Scope

- Color grading progress bar in the native image viewer (uses WebView's existing progress)
- Batch color grading from image viewer (single image only, matching viewer's single-image context)
- Any changes to the WebView gallery flow
