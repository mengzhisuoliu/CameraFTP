# Repository Cleanup and Simplification Design

> Scope: analyze and execute a staged cleanup/simplification effort for the current CameraFTP repository without changing product behavior.

## Goal

Reduce repository maintenance cost by removing dead code and stale dependencies, then progressively simplifying the highest-friction implementations while preserving current behavior on Windows and Android.

## Recommended Approach

Use a four-stage path:

1. **Safe cleanup first** — remove high-confidence dead code, dead registrations, dead constants, stale docs metadata, and unused dependencies.
2. **Configuration persistence consolidation** — keep configuration IO in one place.
3. **Gallery V2 consolidation** — end the current half-migrated frontend bridge state.
4. **Targeted simplification of complex live code** — shrink the remaining high-maintenance areas.

This is intentionally a "reduce surface area first, refactor second" strategy. It lowers risk by avoiding structural refactors on top of already-stale code paths.

## Non-Goals

This work does **not** include:

- new product features
- UI redesign
- Android permission model redesign
- FTP/file-index behavior changes beyond cleanup-required internal refactors
- large cross-cutting architecture rewrites unrelated to identified cleanup targets

## Current Findings Driving the Design

### High-confidence cleanup candidates

- `src/hooks/useGalleryGrid.ts`
  - legacy no-op hook
  - no live importers found
- `src/utils/server-stats-refresh.ts`
  - appears detached from production flow
  - current upload/gallery refresh path is incremental in `src/services/server-events.ts`
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt`
  - no live call sites found
- Tauri commands registered but apparently not called by the frontend:
  - `get_server_status`
  - `get_server_info`
  - `check_storage_permission`
  - `needs_storage_permission`
  - `open_all_files_access_settings`
- `src-tauri/src/constants.rs`
  - `TAURI_LISTENER_MAX_RETRIES`
  - `TAURI_LISTENER_RETRY_DELAY_MS`
- `README.md`
  - version badge stale versus manifests

### Likely removable dependencies

- `src-tauri/Cargo.toml` → `rand`
  - no direct source usage found
- `src-tauri/Cargo.toml` → `chrono`
  - no direct source usage found
  - removal must still be verified via full build because indirect trait/type usage may exist through dependency APIs

### High-value simplification targets

- duplicated config persistence between `src-tauri/src/config.rs` and `src-tauri/src/config_service.rs`
- frontend gallery availability still checks V1 bridge while gallery paging/scheduling already uses V2
- `src/services/gallery-media-v2.ts` exposes redundant thin wrappers
- `src/hooks/useThumbnailScheduler.ts` contains dead state, duplicated cleanup, and bring-up logging noise
- EXIF parsing logic is duplicated between backend command and file index paths
- `src/components/PreviewWindow.tsx` is a monolith with too many responsibilities

## Staged Design

### Stage 1 — Safe Cleanup

#### Objective

Delete high-confidence dead/stale code without changing behavior.

#### Files in scope

- Frontend
  - `src/hooks/useGalleryGrid.ts`
  - `src/utils/server-stats-refresh.ts` (only after final reference confirmation)
  - `README.md`
- Rust / Tauri
  - `src-tauri/src/lib.rs`
  - `src-tauri/src/commands/server.rs`
  - `src-tauri/src/commands/storage.rs`
  - `src-tauri/src/constants.rs`
  - `src-tauri/Cargo.toml`
- Android
  - `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt`

#### Expected changes

- remove dead files with no live call sites
- remove uncalled Tauri command registrations and corresponding implementations
- remove unused constants
- remove high-confidence dead Cargo dependencies
- update stale documentation metadata such as version badge text

#### Constraints

- no behavior change
- any candidate with even moderate uncertainty must be re-checked before deletion
- dependency removal is accepted only if `./build.sh windows android` still passes

#### Risk notes

- `chrono` is textually unused but still needs build verification before being treated as removable
- command removal must account for frontend calls, tests, and any native/event-driven usage

### Stage 2 — Configuration Persistence Consolidation

#### Objective

Eliminate the current dual persistence model and keep configuration IO in a single service.

#### Files in scope

- `src-tauri/src/config.rs`
- `src-tauri/src/config_service.rs`
- any Rust call sites that currently depend on `AppConfig::load()`, `AppConfig::save()`, or `AppConfig::config_path()`

#### Target state

- `AppConfig` remains the data model and default-value container
- `ConfigService` becomes the only owner of:
  - config path resolution
  - config file loading
  - config normalization before runtime/persistence
  - file persistence
- legacy `AppConfig` IO helpers are removed

#### Invariants to preserve

- Android save path normalization remains enforced
- first run still materializes default config on disk when expected
- persisted config format stays backward-compatible

#### Risk notes

- the Android-specific fixed storage path rule is easy to regress if normalization ownership moves carelessly
- any tests around load/save/mutate persistence must continue to pass

### Stage 3 — Gallery V2 Consolidation

#### Objective

Finish the frontend migration so gallery behavior is gated and consumed through a single V2 contract.

#### Files in scope

- `src/services/gallery-media.ts`
- `src/services/gallery-media-v2.ts`
- `src/components/GalleryCard.tsx`
- `src/services/latest-photo.ts`
- `src/hooks/useThumbnailScheduler.ts`
- related tests

#### Target state

- the frontend uses one canonical gallery availability check aligned with the actual bridge in use
- `GalleryCard.tsx` no longer depends on the V1 facade
- V2 service exports one API naming scheme instead of duplicate pass-through aliases
- tests mock the canonical V2 API directly

#### Deliberate boundary

This stage focuses on the **frontend contract**. Native Android code may still temporarily register both V1 and V2 bridges if that reduces risk during transition. Removing the native V1 bridge is a separate follow-up unless it becomes obviously unused after the frontend consolidation.

#### Risk notes

- test changes must land together with service API renames because mock surfaces are coupled tightly
- gallery availability semantics must stay stable on non-Android platforms

### Stage 4 — Simplify Complex Live Code

#### Objective

Reduce maintenance burden in the highest-friction live implementations without broad redesign.

#### 4A. Thumbnail scheduler cleanup

Files:

- `src/hooks/useThumbnailScheduler.ts`

Changes:

- remove `cleanupRef`
- unify duplicate cleanup/unmount logic
- remove or gate bring-up debug logging
- keep current scheduler behavior and retry semantics unchanged

#### 4B. Preview window decomposition

Files:

- `src/components/PreviewWindow.tsx`
- new hook/component files as needed

Changes:

- extract focused logic first, before visual restructuring:
  - EXIF loading
  - zoom/pan handling
  - toolbar auto-hide behavior
- keep the rendered UI and user-facing behavior stable

#### 4C. Shared EXIF helper

Files:

- `src-tauri/src/commands/exif.rs`
- `src-tauri/src/file_index/service.rs`
- new shared helper module as needed

Changes:

- move duplicate EXIF parsing setup into one shared helper
- keep one caller focused on `ExifInfo` formatting
- keep another caller focused on time extraction for sorting/indexing

#### Risk notes

- preview interaction regressions are easy to miss because the file currently mixes many concerns
- EXIF refactors must preserve error tolerance for missing/corrupt metadata

## Dependency Ordering

The stages should be executed in this order:

1. Stage 1: Safe Cleanup
2. Stage 2: Configuration Persistence Consolidation
3. Stage 3: Gallery V2 Consolidation
4. Stage 4: Simplify Complex Live Code

Rationale:

- Stage 1 lowers noise and shrinks the surface area first.
- Stage 2 and Stage 3 remove duplicated entry points and migration leftovers before deeper simplification.
- Stage 4 is safest after the stale layers are gone.

## Verification Strategy

Per repository rules, validation must avoid `bun` and direct `cargo build`.

### Required build verification

After each stage, run:

```bash
./build.sh windows android
```

### Stage-specific expectations

#### Stage 1

- no remaining references to deleted files/constants/commands
- command registration list matches actual frontend usage
- Windows and Android builds still pass

#### Stage 2

- only one config persistence path remains
- config-related tests still pass
- Android and Windows path behavior stays consistent
- full build passes

#### Stage 3

- frontend gallery flow uses only the canonical V2 surface
- gallery-related tests still pass
- full build passes

#### Stage 4

- preview navigation / zoom / drag / fullscreen / EXIF display still behave as before
- file-index sorting still respects EXIF-or-modified-time ordering
- full build passes

## Definition of Done

This cleanup/simplification effort is complete when all of the following are true:

- high-confidence dead code and dead dependencies identified in Stage 1 are removed
- configuration persistence is no longer dual-tracked
- frontend gallery entry points are consolidated onto V2
- at least one full round of complex live-code simplification has landed for scheduler/preview/EXIF duplication
- `./build.sh windows android` passes on the resulting tree

## Open Decisions Already Resolved

- Prefer staged cleanup over one-shot large refactor
- Prioritize safe deletion before structural simplification
- Limit scope to behavior-preserving cleanup/simplification work
- Keep frontend V2 consolidation separate from any possible native V1 bridge removal

## Notes on Git

This spec is intentionally written before implementation planning. It should be reviewed before code changes begin. No commit is included automatically; commit/push actions require explicit user request.
