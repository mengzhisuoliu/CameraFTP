# API Key Input in Prompt Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the user triggers manual AI edit without a configured API Key, show an API Key input field at the top of the prompt dialog. The confirm button stays disabled until both API Key and prompt are filled. On confirm, save the API Key to config.

**Architecture:** Extend the existing `PromptDialog` component (React) and the Kotlin WebView overlay (`ImageViewerActivity.kt`) with a conditional API Key input section. The dialog receives a `hasApiKey` flag; when false, a password input with show/hide toggle appears. On confirm, the API Key is passed through the existing callback chain and saved to config via the config store.

**Tech Stack:** React + TailwindCSS (web), inline HTML/CSS/JS in Kotlin WebView (Android), Zustand config store, JS Bridge (`__tauriGetAiEditPrompt` / `__tauriTriggerAiEditWithPrompt`)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/components/PromptDialog.tsx` | Modify | Add conditional API Key input, pass apiKey through onConfirm |
| `src/components/PreviewWindow.tsx` | Modify | Read hasApiKey from config, pass to PromptDialog, save apiKey on confirm |
| `src/hooks/useGallerySelection.ts` | Modify | Read hasApiKey from config, pass to PromptDialog, save apiKey on confirm |
| `src/App.tsx` | Modify | Extend `__tauriGetAiEditPrompt` to return `hasApiKey`; extend `__tauriTriggerAiEditWithPrompt` to accept `apiKey` |
| `src/types/global.ts` | Modify | Update `__tauriTriggerAiEditWithPrompt` type signature |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt` | Modify | Add API Key input to WebView overlay HTML, extend NativeBridge.onConfirm |

---

### Task 1: Extend React PromptDialog with conditional API Key input

**Files:**
- Modify: `src/components/PromptDialog.tsx`

- [ ] **Step 1: Add new props and API Key state**

Extend `PromptDialogProps` with `hasApiKey?: boolean`. Add internal state for `apiKey` and `showApiKey`.

Change `canConfirm` to require both prompt and (if hasApiKey is false) apiKey.

Extend `onConfirm` callback to include `apiKey`.

When dialog opens (`isOpen` changes), reset `apiKey` state to empty string.

- [ ] **Step 2: Add API Key input section in the dialog body**

When `hasApiKey === false`, render an API Key input section **above** the model selector. It includes:
- Label: "火山引擎 API Key"
- Password input with placeholder "输入火山引擎 API Key"
- Eye/EyeOff toggle button (inline, positioned absolute inside a relative wrapper)
- Styles matching `AiEditConfigPanel.tsx` lines 100-131

- [ ] **Step 3: Update confirm button logic**

`canConfirm` = `prompt.trim().length > 0 && (hasApiKey !== false || apiKey.trim().length > 0)`

Pass apiKey to `onConfirm`: `onConfirm(prompt.trim(), model, saveAsAutoEdit, hasApiKey === false ? apiKey.trim() : undefined)`

---

### Task 2: Wire up PreviewWindow to pass hasApiKey and save apiKey

**Files:**
- Modify: `src/components/PreviewWindow.tsx`

- [ ] **Step 1: Read hasApiKey from config and pass to PromptDialog**

In `PreviewWindow`, derive `hasApiKey` from config store:
```typescript
const hasApiKey = config?.aiEdit?.provider?.type === 'seed-edit' 
  ? !!config.aiEdit.provider.apiKey 
  : true;
```

Pass `hasApiKey` prop to `<PromptDialog>`.

- [ ] **Step 2: Save apiKey in handlePromptConfirm**

Update `handlePromptConfirm` signature to accept `apiKey?: string`. When `apiKey` is provided, save it into config:
```typescript
updateDraft(d => ({
  ...d,
  aiEdit: {
    ...d.aiEdit,
    ...(apiKey ? { provider: { ...d.aiEdit.provider, apiKey } } : {}),
    manualPrompt: prompt,
    manualModel: model,
    ...(saveAsAutoEdit ? { prompt, provider: { ...d.aiEdit.provider, ...(apiKey ? { apiKey } : {}), model } } : {}),
  },
}));
```

---

### Task 3: Wire up useGallerySelection to pass hasApiKey and save apiKey

**Files:**
- Modify: `src/hooks/useGallerySelection.ts`

- [ ] **Step 1: Read hasApiKey from config and pass to PromptDialog**

In the gallery selection hook's render area (where `<PromptDialog>` is rendered via `GalleryCard`), derive `hasApiKey` from config and pass it down.

The `GalleryCard` component receives `showAiEditPrompt`, `onAiEditPromptConfirm`, `onCancelAiEditPrompt`, `defaultPrompt`, `defaultModel`, `autoEditEnabled`. Add `hasApiKey` to this chain.

- [ ] **Step 2: Save apiKey in handleAiEditPromptConfirm**

Update the callback to accept and save `apiKey` parameter, same pattern as PreviewWindow.

---

### Task 4: Extend GalleryCard to pass hasApiKey through

**Files:**
- Modify: `src/components/GalleryCard.tsx`

- [ ] **Step 1: Pass hasApiKey prop through to PromptDialog**

Add `hasApiKey` to the GalleryCard props (received from useGallerySelection) and forward it to `<PromptDialog>`.

---

### Task 5: Extend JS Bridge for Android (App.tsx + global.ts)

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/types/global.ts`

- [ ] **Step 1: Extend `__tauriGetAiEditPrompt` to return `hasApiKey`**

In `App.tsx` line 40-48, add `hasApiKey` to the returned JSON:
```typescript
const hasApiKey = draft?.aiEdit?.provider?.type === 'seed-edit' 
  ? !!draft.aiEdit.provider.apiKey 
  : true;
return JSON.stringify({ prompt, model, autoEdit, hasApiKey });
```

- [ ] **Step 2: Extend `__tauriTriggerAiEditWithPrompt` to accept apiKey**

In `App.tsx` line 50-68, add `apiKey?: string` parameter. Save it to config:
```typescript
w.__tauriTriggerAiEditWithPrompt = async (filePath: string, prompt: string, model?: string, saveAsAutoEdit?: boolean, apiKey?: string) => {
  updateDraft(d => ({
    ...d,
    aiEdit: {
      ...d.aiEdit,
      manualPrompt: prompt,
      manualModel: model ?? '',
      ...(apiKey ? { provider: { ...d.aiEdit.provider, apiKey } } : {}),
      ...(saveAsAutoEdit ? {
        prompt,
        provider: {
          ...d.aiEdit.provider,
          model: model ?? d.aiEdit.provider.model,
          ...(apiKey ? { apiKey } : {}),
        },
      } : {}),
    },
  }));
  await enqueueAiEdit([filePath], prompt, model);
};
```

- [ ] **Step 3: Update TypeScript type in global.ts**

Update `__tauriTriggerAiEditWithPrompt` signature to include `apiKey?: string` parameter.

---

### Task 6: Update Kotlin WebView overlay (ImageViewerActivity.kt)

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt`

- [ ] **Step 1: Parse hasApiKey from JS bridge response**

In `triggerAiEditForCurrentImage()` around line 484-493, parse `hasApiKey` from the JSON:
```kotlin
val hasApiKey = try {
    val json = org.json.JSONObject(jsonString)
    json.optBoolean("hasApiKey", true)
} catch (_: Exception) { true }
```

Pass it to `showPromptWebViewOverlay()`.

- [ ] **Step 2: Add API Key input HTML to WebView overlay**

In `showPromptWebViewOverlay()`, when `hasApiKey` is false, inject an API Key input section above the model selector in the HTML. The section must mirror the React PromptDialog exactly:

HTML structure (inside `.content`, before the model field-group):
```html
<div class="field-group" id="apiKeyGroup">
  <div class="field-label">火山引擎 API Key</div>
  <div style="position:relative">
    <input type="password" id="apiKey" placeholder="输入火山引擎 API Key" style="...same as textarea but single line..." />
    <button type="button" class="eye-btn" onclick="toggleApiKeyVisibility()">
      <svg id="eyeIcon" ...Eye SVG... />
    </button>
  </div>
</div>
```

CSS for the input:
```css
#apiKey {
  width: 100%; padding: 8px 40px 8px 12px; border: 1px solid #e5e7eb;
  border-radius: 8px; font-size: 14px; color: #374151;
  background: #fff; outline: none;
}
#apiKey:focus { border-color: transparent; box-shadow: 0 0 0 2px #3b82f6; }
.eye-btn {
  position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
  background: none; border: none; cursor: pointer; padding: 4px; color: #9ca3af;
}
.eye-btn:hover { color: #6b7280; }
```

- [ ] **Step 3: Update confirm button logic in JS**

`updateConfirmBtn()` must check both prompt and apiKey (when apiKeyGroup is visible):
```javascript
function updateConfirmBtn() {
  var prompt = document.getElementById('prompt').value.trim();
  var apiKeyGroup = document.getElementById('apiKeyGroup');
  var apiKeyOk = !apiKeyGroup || document.getElementById('apiKey').value.trim().length > 0;
  document.getElementById('confirmBtn').disabled = !(prompt && apiKeyOk);
}
```

Add `input` listener on `#apiKey` too.

- [ ] **Step 4: Extend NativeBridge.onConfirm to pass apiKey**

```kotlin
@JavascriptInterface
fun onConfirm(prompt: String, model: String, saveAsAutoEdit: Boolean, apiKey: String) {
    runOnUiThread {
        dismissPromptWebView()
        dispatchAiEdit(filePath, prompt, model, saveAsAutoEdit, apiKey, mainActivity)
    }
}
```

- [ ] **Step 5: Extend dispatchAiEdit to pass apiKey**

Update `dispatchAiEdit` to accept and escape `apiKey`, pass it to `__tauriTriggerAiEditWithPrompt`.

- [ ] **Step 6: Update onConfirm JS to read and pass apiKey**

```javascript
function onConfirm() {
  var prompt = document.getElementById('prompt').value.trim();
  if (!prompt) return;
  var apiKeyEl = document.getElementById('apiKey');
  var apiKey = apiKeyEl ? apiKeyEl.value.trim() : '';
  NativeBridge.onConfirm(prompt, selectedModel, saveAsAutoEdit, apiKey);
}
```

---

### Task 7: Build and verify

- [ ] **Step 1: Build all platforms**

Run: `./build.sh windows android`

Expected: Build succeeds for both platforms with no errors.

- [ ] **Step 2: Verify no TypeScript errors**

Run: `npx tsc --noEmit` (or equivalent via build script)

Expected: No type errors.
