# Repository Cleanup and Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove dead repository surface area, consolidate duplicated configuration/gallery entry points, and simplify the highest-maintenance live code without changing CameraFTP behavior.

**Architecture:** Implement this as four staged, behavior-preserving refactors. First delete high-confidence dead code and stale dependencies, then collapse duplicated runtime entry points (`ConfigService`, Gallery V2), and finally extract smaller units from the most complex live files while preserving current UI and backend behavior.

**Tech Stack:** React 18, TypeScript 5, Vitest, Tauri v2, Rust 2021, Kotlin/Android, TailwindCSS

---

## File Map

- `README.md` — project metadata; update stale version badge.
- `src/hooks/useGalleryGrid.ts` — dead legacy hook; remove.
- `src/utils/server-stats-refresh.ts` — stale utility detached from runtime flow; remove with its tests/mocks if no production usage remains.
- `src/services/server-events.ts` — confirm runtime path still uses incremental gallery updates only.
- `src/services/gallery-media.ts` — V1 gallery availability facade; remove after frontend switches to canonical V2 availability.
- `src/services/gallery-media-v2.ts` — canonical gallery bridge API after wrapper cleanup.
- `src/components/GalleryCard.tsx` — switch availability gate to V2.
- `src/services/latest-photo.ts` — use canonical V2 API names only.
- `src/hooks/useThumbnailScheduler.ts` — remove dead state, unify cleanup, trim debug noise.
- `src/components/PreviewWindow.tsx` — slim down by extracting focused hooks.
- `src/hooks/usePreviewExif.ts` — new hook for EXIF loading and reset behavior.
- `src/hooks/usePreviewZoomPan.ts` — new hook for zoom/pan/drag behavior.
- `src/hooks/usePreviewToolbarAutoHide.ts` — new hook for toolbar hide timer behavior.
- `src-tauri/src/lib.rs` — remove dead Tauri command registrations and add any new helper module registration.
- `src-tauri/src/commands/server.rs` — remove dead command implementations.
- `src-tauri/src/commands/storage.rs` — remove dead command implementations.
- `src-tauri/src/constants.rs` — remove dead constants.
- `src-tauri/src/config.rs` — keep pure config data model/defaults only.
- `src-tauri/src/config_service.rs` — own config path, load, save, normalize, and persist flow.
- `src-tauri/src/exif_support.rs` — new shared EXIF parsing helper.
- `src-tauri/src/commands/exif.rs` — consume shared helper for formatted EXIF response.
- `src-tauri/src/file_index/service.rs` — consume shared helper for EXIF timestamp extraction.
- `src-tauri/Cargo.toml` — remove dead dependencies after verification.
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt` — dead helper; remove.

### Task 1: Baseline and Safe File Cleanup

**Files:**
- Modify: `README.md:1-8`
- Delete: `src/hooks/useGalleryGrid.ts`
- Delete: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt`
- Test: `src/services/__tests__/server-events.test.ts`

- [ ] **Step 1: Write the failing characterization check for the incremental gallery path**

```ts
// src/services/__tests__/server-events.test.ts
it('does not trigger any full gallery refresh helper during stats updates', async () => {
  const { initializeServerEvents } = await import('../server-events');
  const cleanup = await initializeServerEvents();

  window.dispatchEvent(new CustomEvent('stats-update', {
    detail: { filesReceived: 2, totalBytesReceived: 10 },
  }));

  expect(window.dispatchEvent).not.toHaveBeenCalledWith(
    expect.objectContaining({ type: 'gallery-refresh-requested' }),
  );

  cleanup();
});
```

- [ ] **Step 2: Run the targeted test to lock the current behavior**

Run: `npm test -- src/services/__tests__/server-events.test.ts`
Expected: PASS, proving runtime already uses incremental gallery updates.

- [ ] **Step 3: Delete dead files and update the README version badge**

```diff
--- a/README.md
+++ b/README.md
@@
-![版本](https://img.shields.io/badge/version-0.1.0-blue)
+![版本](https://img.shields.io/badge/version-1.3.1-blue)
```

```bash
rm "src/hooks/useGalleryGrid.ts"
rm "src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt"
```

- [ ] **Step 4: Verify cleanup did not change behavior**

Run: `npm test -- src/services/__tests__/server-events.test.ts && ./build.sh windows android`
Expected: tests PASS; Windows and Android builds complete successfully.

- [ ] **Step 5: Commit**

```bash
git add README.md src/services/__tests__/server-events.test.ts src/hooks/useGalleryGrid.ts src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt
git commit -m "chore: remove dead gallery and android helper code"
```

### Task 2: Remove Dead Runtime Commands, Constants, and Stale Utility

**Files:**
- Modify: `src-tauri/src/lib.rs:181-245`
- Modify: `src-tauri/src/commands/server.rs:129-155`
- Modify: `src-tauri/src/commands/storage.rs:53-75`
- Modify: `src-tauri/src/constants.rs:79-88`
- Delete: `src/utils/server-stats-refresh.ts`
- Modify: `src/services/__tests__/server-events.test.ts`
- Delete: `src/utils/__tests__/server-stats-refresh.test.ts`

- [ ] **Step 1: Write the failing test proving frontend runtime state uses only the live command surface**

```ts
// src/services/__tests__/server-events.test.ts
it('syncs server state from get_server_runtime_state only', async () => {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === 'get_server_runtime_state') {
      return { serverInfo: null, stats: { isRunning: false, filesReceived: 0, totalBytesReceived: 0 } };
    }
    throw new Error(`unexpected command: ${command}`);
  });

  const { initializeServerEvents } = await import('../server-events');
  const cleanup = await initializeServerEvents();

  expect(invokeMock).toHaveBeenCalledWith('get_server_runtime_state');
  expect(invokeMock).not.toHaveBeenCalledWith('get_server_status');
  expect(invokeMock).not.toHaveBeenCalledWith('get_server_info');

  cleanup();
});
```

- [ ] **Step 2: Run the targeted tests before deletion**

Run: `npm test -- src/services/__tests__/server-events.test.ts src/utils/__tests__/server-stats-refresh.test.ts`
Expected: PASS, confirming the stale utility is test-only and the runtime path is isolated.

- [ ] **Step 3: Remove dead commands, constants, and the stale utility**

```diff
--- a/src-tauri/src/lib.rs
+++ b/src-tauri/src/lib.rs
@@
-            get_server_status,
-            get_server_info,
             get_server_runtime_state,
@@
-            open_all_files_access_settings,
@@
-            check_storage_permission,
             check_server_start_prerequisites,
-            needs_storage_permission,
```

```diff
--- a/src-tauri/src/constants.rs
+++ b/src-tauri/src/constants.rs
@@
-/// Tauri 监听器注册最大重试次数
-pub const TAURI_LISTENER_MAX_RETRIES: i32 = 50;
-
-/// Tauri 监听器注册重试延迟（毫秒）
-pub const TAURI_LISTENER_RETRY_DELAY_MS: i64 = 50;
```

```bash
rm "src/utils/server-stats-refresh.ts"
rm "src/utils/__tests__/server-stats-refresh.test.ts"
```

- [ ] **Step 4: Verify there are no call-site regressions**

Run: `npm test -- src/services/__tests__/server-events.test.ts && ./build.sh windows android`
Expected: tests PASS; builds PASS with command registrations aligned to real usage.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/server.rs src-tauri/src/commands/storage.rs src-tauri/src/constants.rs src/services/__tests__/server-events.test.ts src/utils/server-stats-refresh.ts src/utils/__tests__/server-stats-refresh.test.ts
git commit -m "chore: remove dead runtime commands and stale refresh utility"
```

### Task 3: Remove Dead Rust Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml:40-47`
- Test: `src-tauri/src/file_index/service.rs:267-293`

- [ ] **Step 1: Write a build-safety note in the plan branch commit message context by locking the exact dependency removal diff**

```diff
--- a/src-tauri/Cargo.toml
+++ b/src-tauri/Cargo.toml
@@
 nom-exif = "2.7"
-chrono = "0.4"
 
 # 密码哈希
 argon2 = "0.5"
-rand = "0.8"  # 提供 rand_core/getrandom feature，argon2 依赖需要
 zeroize = "1.8"  # 内存安全：密码使用后自动清零
```

- [ ] **Step 2: Run the full build immediately after the dependency removal**

Run: `./build.sh windows android`
Expected: PASS. If the build fails due to indirect `chrono` usage, restore only `chrono` and keep `rand` removed.

- [ ] **Step 3: Apply the minimal keep/remove result**

```toml
# src-tauri/Cargo.toml after the step if both are removable
nom-exif = "2.7"

# 密码哈希
argon2 = "0.5"
zeroize = "1.8"
```

```toml
# src-tauri/Cargo.toml after the step if chrono must stay
nom-exif = "2.7"
chrono = "0.4"

# 密码哈希
argon2 = "0.5"
zeroize = "1.8"
```

- [ ] **Step 4: Re-run the build to confirm the final dependency set**

Run: `./build.sh windows android`
Expected: PASS with the smallest working dependency set.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: prune unused rust dependencies"
```

### Task 4: Consolidate Configuration Persistence into ConfigService

**Files:**
- Modify: `src-tauri/src/config.rs:220-328`
- Modify: `src-tauri/src/config_service.rs:20-136`
- Test: `src-tauri/src/config_service.rs:139-265`

- [ ] **Step 1: Extend the failing Rust tests to express the single-owner persistence design**

```rust
#[test]
fn config_service_persists_defaults_without_appconfig_io_helpers() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");

    let service = ConfigService::new_with_path(config_path.clone());
    let loaded = service.load().expect("failed to load defaults");

    assert!(config_path.exists(), "config file should be created by ConfigService");
    assert_eq!(loaded, service.get().expect("failed to read in-memory config"));
}
```

- [ ] **Step 2: Run the focused Rust test set before refactoring**

Run: `cargo test --manifest-path src-tauri/Cargo.toml config_service`
Expected: PASS, giving a safe baseline for the refactor.

- [ ] **Step 3: Remove `AppConfig` IO helpers and move path ownership into `ConfigService`**

```rust
// src-tauri/src/config.rs
impl AppConfig {
    fn default_pictures_dir() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH)
        }
        #[cfg(target_os = "windows")]
        {
            dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
        }
    }
}
```

```rust
// src-tauri/src/config_service.rs
impl ConfigService {
    pub fn new() -> Result<Self, AppError> {
        let service = Self::new_with_path(Self::config_path());
        service.load()?;
        Ok(service)
    }

    fn config_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            crate::config::get_android_config_path()
        }
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir()
                .map(|d| d.join("cameraftp"))
                .unwrap_or_else(|| PathBuf::from("./config"))
                .join("config.json")
        }
    }
}
```

- [ ] **Step 4: Verify config behavior still holds**

Run: `cargo test --manifest-path src-tauri/Cargo.toml config_service && ./build.sh windows android`
Expected: Rust config tests PASS; Windows and Android builds PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/config_service.rs
git commit -m "refactor: centralize config persistence in ConfigService"
```

### Task 5: Consolidate Frontend Gallery Access on V2

**Files:**
- Modify: `src/services/gallery-media-v2.ts:26-199`
- Modify: `src/components/GalleryCard.tsx:11-13,191-193`
- Modify: `src/services/latest-photo.ts`
- Modify: `src/hooks/useThumbnailScheduler.ts:17-22`
- Delete: `src/services/gallery-media.ts`
- Test: `src/services/__tests__/gallery-media-v2.test.ts`
- Test: `src/services/__tests__/latest-photo.test.ts`
- Test: `src/components/__tests__/GalleryCard.virtualized.test.tsx`
- Test: `src/hooks/__tests__/useThumbnailScheduler.test.ts`

- [ ] **Step 1: Write the failing tests against a single canonical V2 API**

```ts
// src/services/__tests__/gallery-media-v2.test.ts
it('reports gallery availability from GalleryAndroidV2 only', () => {
  delete (window as Partial<Window>).GalleryAndroid;
  (window as Partial<Window>).GalleryAndroidV2 = {} as typeof window.GalleryAndroidV2;

  expect(isGalleryV2Available()).toBe(true);
});
```

```ts
// src/components/__tests__/GalleryCard.virtualized.test.tsx
vi.mock('../../services/gallery-media-v2', async () => {
  const actual = await vi.importActual<typeof import('../../services/gallery-media-v2')>('../../services/gallery-media-v2');
  return {
    ...actual,
    isGalleryV2Available: () => true,
  };
});
```

- [ ] **Step 2: Run the targeted gallery/frontend test set**

Run: `npm test -- src/services/__tests__/gallery-media-v2.test.ts src/services/__tests__/latest-photo.test.ts src/components/__tests__/GalleryCard.virtualized.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts`
Expected: FAIL where old V1 wrappers/aliases are still assumed.

- [ ] **Step 3: Collapse the frontend onto the canonical V2 exports**

```ts
// src/services/gallery-media-v2.ts
export function isGalleryV2Available(): boolean {
  return typeof window !== 'undefined' && !!window.GalleryAndroidV2;
}

export async function enqueueThumbnails(reqs: ThumbRequest[]): Promise<void> {
  const bridge = getBridge();
  await bridge.enqueueThumbnails(JSON.stringify(reqs));
}

export async function registerThumbnailListener(
  viewId: string,
  listenerId: string,
  listener: ThumbResultListener,
): Promise<void> {
  const bridge = getBridge();
  await bridge.registerThumbnailListener(viewId, listenerId);
  listeners.set(listenerId, listener);
  window.__galleryThumbDispatch = dispatchThumbnailResult;
}
```

```ts
// src/components/GalleryCard.tsx
import { invalidateMediaIds, isGalleryV2Available } from '../services/gallery-media-v2';

if (!isGalleryV2Available()) {
  return null;
}
```

```bash
rm "src/services/gallery-media.ts"
```

- [ ] **Step 4: Verify the consolidated gallery path**

Run: `npm test -- src/services/__tests__/gallery-media-v2.test.ts src/services/__tests__/latest-photo.test.ts src/components/__tests__/GalleryCard.virtualized.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts && ./build.sh windows android`
Expected: frontend tests PASS; full builds PASS.

- [ ] **Step 5: Commit**

```bash
git add src/services/gallery-media-v2.ts src/components/GalleryCard.tsx src/services/latest-photo.ts src/hooks/useThumbnailScheduler.ts src/services/gallery-media.ts src/services/__tests__/gallery-media-v2.test.ts src/services/__tests__/latest-photo.test.ts src/components/__tests__/GalleryCard.virtualized.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts
git commit -m "refactor: consolidate frontend gallery access on V2"
```

### Task 6: Simplify the Thumbnail Scheduler Cleanup Path

**Files:**
- Modify: `src/hooks/useThumbnailScheduler.ts:63-321`
- Test: `src/hooks/__tests__/useThumbnailScheduler.test.ts`

- [ ] **Step 1: Add failing tests for cleanup deduplication and state reset**

```ts
it('cancels active requests exactly once during cleanup and unmount', async () => {
  const { result, unmount } = renderHook(() => useThumbnailScheduler({ debounceMs: 0 }));

  act(() => {
    result.current.registerMedia([{ mediaId: '1', uri: 'file:///a.jpg', dateModifiedMs: 1 }]);
    result.current.updateViewport(['1'], []);
  });

  await vi.runAllTimersAsync();

  act(() => {
    result.current.cleanup();
  });
  unmount();

  expect(cancelThumbnailRequests).toHaveBeenCalledTimes(1);
});
```

- [ ] **Step 2: Run the scheduler tests before refactoring**

Run: `npm test -- src/hooks/__tests__/useThumbnailScheduler.test.ts`
Expected: FAIL if cleanup remains duplicated.

- [ ] **Step 3: Remove dead state and route unmount through a single cleanup path**

```ts
// src/hooks/useThumbnailScheduler.ts
const cleanup = useCallback(() => {
  if (debounceRef.current !== null) {
    clearTimeout(debounceRef.current);
    debounceRef.current = null;
  }
  pendingRef.current = null;

  const allRequestIds = [...activeRequestsRef.current.keys()];
  if (allRequestIds.length > 0) {
    void cancelThumbnailRequests(allRequestIds);
  }

  activeRequestsRef.current.clear();
  failedMediaRef.current.clear();
  setLoadingThumbs(new Set());
}, []);

useEffect(() => cleanup, [cleanup]);
```

- [ ] **Step 4: Verify the scheduler still behaves the same**

Run: `npm test -- src/hooks/__tests__/useThumbnailScheduler.test.ts && ./build.sh windows android`
Expected: scheduler tests PASS; full builds PASS.

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useThumbnailScheduler.ts src/hooks/__tests__/useThumbnailScheduler.test.ts
git commit -m "refactor: simplify thumbnail scheduler cleanup"
```

### Task 7: Extract Preview Window Hooks

**Files:**
- Create: `src/hooks/usePreviewExif.ts`
- Create: `src/hooks/usePreviewZoomPan.ts`
- Create: `src/hooks/usePreviewToolbarAutoHide.ts`
- Modify: `src/components/PreviewWindow.tsx:40-317`
- Test: `src/hooks/__tests__/usePreviewWindowLifecycle.test.tsx`
- Test: `src/hooks/__tests__/usePreviewNavigation.test.tsx`

- [ ] **Step 1: Add failing hook-level tests for the extracted behavior seams**

```ts
// src/hooks/__tests__/usePreviewToolbarAutoHide.test.tsx
it('hides the toolbar after inactivity and stays visible while hovered', async () => {
  const { result, rerender } = renderHook(
    ({ hovered }) => usePreviewToolbarAutoHide({ hovered, delayMs: 3000 }),
    { initialProps: { hovered: false } },
  );

  await vi.advanceTimersByTimeAsync(3000);
  expect(result.current.showToolbar).toBe(false);

  rerender({ hovered: true });
  act(() => result.current.revealToolbar());
  await vi.advanceTimersByTimeAsync(3000);
  expect(result.current.showToolbar).toBe(true);
});
```

- [ ] **Step 2: Run the preview-related tests before extraction**

Run: `npm test -- src/hooks/__tests__/usePreviewWindowLifecycle.test.tsx src/hooks/__tests__/usePreviewNavigation.test.tsx`
Expected: PASS baseline before changing `PreviewWindow.tsx`.

- [ ] **Step 3: Extract the focused hooks and simplify `PreviewWindow.tsx`**

```ts
// src/hooks/usePreviewExif.ts
import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import type { ExifInfo } from '../types';

export function usePreviewExif(imagePath: string | null) {
  const [exifInfo, setExifInfo] = useState<ExifInfo | null>(null);

  const loadExifInfo = useCallback(async (path: string) => {
    try {
      const exif = await invoke<ExifInfo | null>('get_image_exif', { filePath: path });
      setExifInfo(exif);
    } catch {
      setExifInfo(null);
    }
  }, []);

  useEffect(() => {
    if (imagePath) {
      void loadExifInfo(imagePath);
    } else {
      setExifInfo(null);
    }
  }, [imagePath, loadExifInfo]);

  return exifInfo;
}
```

```ts
// src/hooks/usePreviewToolbarAutoHide.ts
import { useEffect, useRef, useState } from 'react';

export function usePreviewToolbarAutoHide({ hovered, delayMs = 3000 }: { hovered: boolean; delayMs?: number }) {
  const [showToolbar, setShowToolbar] = useState(true);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!showToolbar || hovered) {
      return;
    }

    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    timeoutRef.current = setTimeout(() => setShowToolbar(false), delayMs);
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [hovered, showToolbar, delayMs]);

  return { showToolbar, setShowToolbar };
}
```

- [ ] **Step 4: Verify preview behavior stays stable**

Run: `npm test -- src/hooks/__tests__/usePreviewWindowLifecycle.test.tsx src/hooks/__tests__/usePreviewNavigation.test.tsx src/hooks/__tests__/usePreviewToolbarAutoHide.test.tsx && ./build.sh windows android`
Expected: preview tests PASS; full builds PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/PreviewWindow.tsx src/hooks/usePreviewExif.ts src/hooks/usePreviewZoomPan.ts src/hooks/usePreviewToolbarAutoHide.ts src/hooks/__tests__/usePreviewToolbarAutoHide.test.tsx
git commit -m "refactor: extract preview window behavior hooks"
```

### Task 8: Share EXIF Parsing Between Backend Callers

**Files:**
- Create: `src-tauri/src/exif_support.rs`
- Modify: `src-tauri/src/lib.rs:5-16`
- Modify: `src-tauri/src/commands/exif.rs`
- Modify: `src-tauri/src/file_index/service.rs:267-293`

- [ ] **Step 1: Add failing Rust tests around the shared EXIF helper API**

```rust
// src-tauri/src/exif_support.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_exif_returns_none() {
        let result = parse_exif(std::path::Path::new("/definitely/missing.jpg"));
        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run the focused Rust test set before extraction**

Run: `cargo test --manifest-path src-tauri/Cargo.toml exif`
Expected: FAIL until the new helper exists and callers are wired to it.

- [ ] **Step 3: Extract the shared parser and update both callers**

```rust
// src-tauri/src/exif_support.rs
use std::path::Path;

use nom_exif::{Exif, ExifIter, ExifTag, MediaParser, MediaSource};

pub fn parse_exif(path: &Path) -> Option<Exif> {
    let mut parser = MediaParser::new();
    let media_source = MediaSource::file_path(path).ok()?;

    if !media_source.has_exif() {
        return None;
    }

    let iter: ExifIter = parser.parse(media_source).ok()?;
    Some(iter.into())
}
```

```rust
// src-tauri/src/file_index/service.rs
let exif_time = crate::exif_support::parse_exif(&path)
    .and_then(|exif| exif.get(ExifTag::DateTimeOriginal)
        .and_then(|v| v.as_time_components())
        .map(|(datetime, _offset)| datetime))
    .and_then(|datetime| datetime.and_utc().try_into().ok());
```

- [ ] **Step 4: Verify both backend consumers still work**

Run: `cargo test --manifest-path src-tauri/Cargo.toml exif && ./build.sh windows android`
Expected: Rust EXIF tests PASS; full builds PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/exif_support.rs src-tauri/src/lib.rs src-tauri/src/commands/exif.rs src-tauri/src/file_index/service.rs
git commit -m "refactor: share exif parsing between backend consumers"
```

## Final Verification Pass

- [ ] Run: `npm test -- src/services/__tests__/server-events.test.ts src/services/__tests__/gallery-media-v2.test.ts src/services/__tests__/latest-photo.test.ts src/components/__tests__/GalleryCard.virtualized.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts src/hooks/__tests__/usePreviewWindowLifecycle.test.tsx src/hooks/__tests__/usePreviewNavigation.test.tsx src/hooks/__tests__/usePreviewToolbarAutoHide.test.tsx`
- [ ] Expected: PASS
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml config_service`
- [ ] Expected: PASS
- [ ] Run: `cargo test --manifest-path src-tauri/Cargo.toml exif`
- [ ] Expected: PASS
- [ ] Run: `./build.sh windows android`
- [ ] Expected: PASS

## Spec Coverage Check

- Stage 1 safe cleanup is covered by Tasks 1-3.
- Stage 2 configuration persistence consolidation is covered by Task 4.
- Stage 3 Gallery V2 consolidation is covered by Task 5.
- Stage 4 complex live-code simplification is covered by Tasks 6-8.
- Required verification via `./build.sh windows android` appears after every stage and in the final verification pass.

## Self-Review Notes

- Placeholder scan complete: no `TODO`, `TBD`, or deferred "implement later" items remain.
- Scope check complete: plan stays within cleanup/simplification, with no feature creep.
- Naming consistency check complete: canonical gallery API is `isGalleryV2Available`, `enqueueThumbnails`, `cancelThumbnailRequests`, `registerThumbnailListener`, `unregisterThumbnailListener`.
