# FTP MediaStore Fixes Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Android FTP behavior for explicit non-media upload failure, virtual subdirectory navigation, explicit FTP error replies, and gallery refresh after FTP deletes.

**Architecture:** Keep MediaStore as the Android storage authority, but stop treating the FTP root as a flat directory. The Android backend will synthesize virtual directories from MediaStore relative paths, reject unsupported non-media uploads before writing, and emit explicit libunftp storage errors so clients receive deterministic FTP replies. Frontend refresh remains event-driven and gains an explicit delete-triggered refresh path.

**Tech Stack:** Rust, libunftp/unftp-core, Kotlin MediaStore bridge, React, TypeScript, Vitest.

---

## File Structure Map

**Modify:**
- `src-tauri/src/ftp/android_mediastore/backend.rs` - virtual directory semantics, explicit error mapping, non-media upload rejection, delete refresh event hook.
- `src-tauri/src/ftp/android_mediastore/bridge.rs` - bridge queries for prefix-based directory discovery and any new DTO handling.
- `src-tauri/src/ftp/android_mediastore/types.rs` - helper types/functions for media classification and virtual directory metadata.
- `src-tauri/src/ftp/android_mediastore/tests.rs` - backend regression tests for directory navigation and upload rejection.
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt` - prefix query support for virtual directories.
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridgeTest.kt` - bridge tests for the new query/path behavior.
- `src-tauri/src/ftp/listeners.rs` - explicit refresh event emission on delete if needed by final event design.
- `src/stores/serverStore.ts` - listen for delete-triggered refresh event.
- `src/utils/gallery-refresh.ts` - support explicit delete refresh reason.
- `src/utils/__tests__/gallery-refresh.test.ts` - refresh event regression coverage.

## Chunk 1: FTP Backend Semantics

### Task 1: Add failing Rust tests for explicit errors and virtual directories

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/tests.rs`
- Modify: `src-tauri/src/ftp/android_mediastore/backend.rs`

- [ ] Add tests showing non-media uploads fail explicitly before write.
- [ ] Add tests showing `cwd("/")` succeeds, `cwd("/existing")` succeeds when entries exist, and missing dirs fail deterministically.
- [ ] Add tests showing `mkd()` returns command-not-implemented semantics instead of generic unsupported IO mapping.
- [ ] Add tests showing `list("/")` and `list("/subdir")` synthesize virtual directory entries.

### Task 2: Implement minimal backend changes to pass the new Rust tests

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/backend.rs`
- Modify: `src-tauri/src/ftp/android_mediastore/types.rs`
- Modify: `src-tauri/src/ftp/android_mediastore/bridge.rs`

- [ ] Replace flat-path rejection with path parsing that supports nested relative paths.
- [ ] Introduce virtual-directory listing and metadata/cwd checks.
- [ ] Reject non-media uploads with explicit libunftp error kinds.
- [ ] Return explicit libunftp error kinds for unsupported `mkd`/`rmd` and missing directories.

## Chunk 2: Android Bridge Query Support

### Task 3: Add failing Kotlin tests for relative path queries

**Files:**
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridgeTest.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt`

- [ ] Add tests for helper logic that normalizes query prefixes for root and nested virtual directories.
- [ ] Add tests for any new selection logic used to discover child entries.

### Task 4: Implement minimal Kotlin bridge changes to pass tests

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt`

- [ ] Add prefix-based query support needed by the Rust virtual-directory backend.
- [ ] Keep existing exact-path behavior working for file lookups.

## Chunk 3: Delete Refresh Event Flow

### Task 5: Add failing frontend tests for explicit delete refresh reason

**Files:**
- Modify: `src/utils/__tests__/gallery-refresh.test.ts`
- Modify: `src/utils/gallery-refresh.ts`

- [ ] Add a test showing delete-triggered refresh events dispatch both gallery and latest-photo refreshes.

### Task 6: Implement minimal frontend refresh changes

**Files:**
- Modify: `src/stores/serverStore.ts`
- Modify: `src/utils/gallery-refresh.ts`
- Modify: `src-tauri/src/ftp/listeners.rs`

- [ ] Add an explicit delete refresh reason.
- [ ] Wire FTP delete success into the existing refresh scheduler.

## Verification

- [ ] Run targeted Rust tests for `android_mediastore`.
- [ ] Run targeted Vitest refresh tests.
- [ ] Run targeted Android bridge tests if feasible in this environment.
- [ ] Run `./build.sh windows android`.
