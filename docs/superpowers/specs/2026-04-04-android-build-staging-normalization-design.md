# Android Build Staging Normalization Design

## Problem Statement

The Android build currently allows generated and ignored files under `src-tauri/gen/android/app/src/main/assets/` and `src-tauri/gen/android/app/src/main/jniLibs/` to be packaged into the APK. Because those directories are both ignored by git and treated as normal Android source inputs, two workspaces at the same commit can produce materially different APKs.

This already caused a release APK from the main workspace to grow from about 9.3 MB to about 19 MB by packaging stale native libraries such as `libvips.so`, `libfftw3.so`, `libzstd.so`, and related transitive files. The smaller worktree APK shows that those files are not required for the normal Android package.

The build also relies on `assets/wechat.png` for Android-only behavior via `PermissionBridge.saveImageToGallery("wechat.png")`, but that asset currently exists only as a workspace-local leftover. That means the build is both non-reproducible and functionally fragile.

## Goals

1. Make Android APK contents reproducible across workspaces for the same commit.
2. Remove all generated staging content from `src/main` paths.
3. Make JNI and generated asset staging lifecycle owned by Gradle / Android build logic rather than ad hoc workspace leftovers.
4. Preserve existing Android behavior, including `saveImageToGallery("wechat.png")`.
5. Add build-time guardrails so unexpected native libraries cannot silently enter the APK.

## Non-Goals

1. Changing app behavior outside Android build normalization.
2. Refactoring unrelated Tauri, frontend, or Rust functionality.
3. Optimizing Windows binary size.
4. Introducing dynamic asset download or runtime-generated payment assets.

## Current Root Cause

The root cause is structural:

1. `src-tauri/gen/android/app/src/main/assets/` and `src-tauri/gen/android/app/src/main/jniLibs/` are ignored directories.
2. Android packaging treats `src/main` as a normal source input location.
3. Existing cleanup removes `app/build` and Gradle caches, but does not remove those ignored `src/main` staging directories.
4. Once extra files land there, they persist across builds and are silently packaged into future APKs.

This makes package contents depend on workspace history instead of tracked source and declared build steps.

## Proposed Architecture

### 1. Separate tracked static inputs from generated staging outputs

Create a tracked Android build input area under `src-tauri/android-build/` with clear responsibility boundaries:

- `src-tauri/android-build/static-assets/`
  - tracked, human-owned Android asset files required at runtime
  - initial content includes `wechat.png`
Generated APK input staging must move under Gradle build output paths, for example:

- `src-tauri/gen/android/app/build/tauri-staging/assets/`
- `src-tauri/gen/android/app/build/tauri-staging/jniLibs/<abi>/`

Nothing under `src/main/assets` or `src/main/jniLibs` should be required for generated build inputs after this change.

### 2. Make Gradle own the staging lifecycle

Android staging should be prepared by explicit Gradle tasks, not by leftover files and not by manually managed ignored directories.

Add dedicated tasks in the Android Gradle layer to:

1. clean the staging root for the current build
2. copy tracked static Android assets into staged assets
3. copy generated Tauri asset files that must be packaged on Android, such as `tauri.conf.json`, into staged assets
4. copy the Rust-produced Android shared library into staged `jniLibs`
5. validate that only allowed staged native libraries exist

These tasks should run automatically before Android asset merge / JNI merge tasks for debug and release builds.

### 3. Use explicit source sets for packaged inputs

Update `src-tauri/gen/android/app/build.gradle.kts` to package Android assets and JNI libraries from explicit directories instead of implicit `src/main` leftovers.

The Android app module should explicitly reference:

- staged assets directory under `app/build/tauri-staging/assets`
- staged JNI directory under `app/build/tauri-staging/jniLibs`

This ensures packaged inputs come only from declared build outputs.

### 4. Keep static Android assets version-controlled

`wechat.png` must become a tracked file in the new static asset area instead of an ignored workspace artifact. Android runtime code can continue using `activity.assets.open(assetPath)` because the file will still be packaged as an APK asset after staging.

The frontend call site remains unchanged:

- `saveImageToGallery("wechat.png")`

The Android bridge remains unchanged in behavior.

### 5. Enforce a native library whitelist

The staged JNI directory should contain only the native library or libraries intentionally produced for the APK. Based on current evidence, the intended packaged library set for Android release is:

- `libcamera_ftp_companion_lib.so`

The staging task should fail the build if any additional `.so` file appears in staged `jniLibs`. That turns silent APK inflation into a visible build error.

If future Android-native dependencies become legitimate, they must be added deliberately to the whitelist in code and reviewed as part of source control.

## Build Flow After Change

1. Tauri / Rust build produces the Android `.so`.
2. Gradle prepare-staging task removes old `app/build/tauri-staging/` content.
3. Gradle copies tracked static assets from `src-tauri/android-build/static-assets/` into staged assets.
4. Gradle copies generated Android-required config assets, including `tauri.conf.json`, into staged assets.
5. Gradle copies the built Rust library into staged `jniLibs/<abi>/`.
6. Gradle validates staged JNI contents against the whitelist.
7. Android merge/package tasks consume only staged assets and staged JNI directories.
8. `./build.sh windows android` remains the top-level verification command, but it no longer needs to manage Android staging details directly.

## File and Responsibility Changes

### Files to modify

- `src-tauri/gen/android/app/build.gradle.kts`
  - declare explicit staged asset/JNI source directories
  - register and wire staging / validation tasks into Android build lifecycle
- `src-tauri/gen/android/buildSrc/src/main/java/com/gjk/cameraftpcompanion/kotlin/RustPlugin.kt`
  - expose or wire Rust output paths cleanly into Gradle staging flow
  - ensure merge tasks depend on staging tasks in addition to Rust build tasks
- `scripts/build-common.sh`
  - clean new staging outputs under `app/build/tauri-staging`
  - stop relying on old ignored `src/main` staging locations
- optionally `scripts/build-android.sh`
  - only if minimal adjustments are needed to align with new Gradle-managed flow

### Files to add

- `src-tauri/android-build/static-assets/wechat.png`
  - tracked Android-only asset source

### Files/directories to retire from the architecture

- `src-tauri/gen/android/app/src/main/assets/` as a generated staging area
- `src-tauri/gen/android/app/src/main/jniLibs/` as a generated staging area

These paths may still exist historically on developer machines, but the build must no longer consume them.

## Error Handling and Failure Modes

### Unexpected native libraries present

Behavior: fail the build with a clear error listing the unexpected filenames and the expected whitelist.

Why: silent packaging is the bug; loud failure is the protection.

### Missing tracked Android asset

Behavior: fail the build during staging with a clear message that the required static asset is missing.

Why: runtime failure in `saveImageToGallery` would be harder to diagnose than a build-time failure.

### Missing Rust output library

Behavior: fail staging before packaging if the expected `.so` is absent.

Why: packaging should not continue with partial or stale JNI input.

## Testing Strategy

### Automated verification

1. Run `./build.sh windows android` from the main workspace.
2. Confirm Android build succeeds.
3. Inspect resulting APK contents and confirm no stale third-party `.so` files are packaged.
4. Confirm `wechat.png` is packaged as an asset.
5. Confirm Windows build still succeeds unchanged.

### Reproducibility verification

1. Delete any old ignored directories under:
   - `src-tauri/gen/android/app/src/main/assets`
   - `src-tauri/gen/android/app/src/main/jniLibs`
2. Rebuild in the main workspace.
3. Compare APK contents and size against a clean worktree build on the same commit.
4. Confirm parity or near-parity aside from normal build metadata differences.

### Regression checks

1. Confirm `saveImageToGallery("wechat.png")` still resolves an Android asset packaged in the APK.
2. Confirm future accidental drops into ignored legacy paths do not affect APK contents.

## Migration Notes

Existing ignored files under legacy `src/main` staging paths should be considered pollution and can be deleted locally after the new flow is in place.

The repository should not rely on developers manually cleaning those paths to get a correct build. The build must be self-sanitizing and deterministic.

## Trade-Offs

### Benefits

- deterministic Android packaging
- clear separation between tracked inputs and generated outputs
- no hidden coupling to workspace history
- easier future auditing of what enters the APK

### Costs

- Gradle build logic becomes slightly more explicit and custom
- Rust-to-Gradle handoff needs clear staging integration
- initial implementation is broader than a simple cleanup patch

These costs are acceptable because the current layout has already produced incorrect release artifacts from a clean commit.

## Acceptance Criteria

1. Android APK contents are reproducible across workspaces on the same commit.
2. The build no longer packages files from `src-tauri/gen/android/app/src/main/assets/` or `src-tauri/gen/android/app/src/main/jniLibs/`.
3. `wechat.png` is sourced from a tracked location and is present in the APK.
4. Unexpected extra `.so` files cause a build failure.
5. `./build.sh windows android` passes.
6. The main workspace APK size returns to the expected range near the clean worktree APK instead of the inflated 19 MB result.
