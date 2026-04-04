# Android Build Staging Normalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Android APK packaging deterministic by moving generated asset/JNI staging out of `src/main`, making Gradle own staging, and preserving Android asset behavior like `saveImageToGallery("wechat.png")`.

**Architecture:** Android packaging will consume only explicit staging directories under `src-tauri/gen/android/app/build/tauri-staging/`. Tracked Android-only assets will live under `src-tauri/android-build/static-assets/`, while Gradle tasks and the Rust plugin will prepare staged assets and JNI libs per build variant. Legacy `src/main/assets` and `src/main/jniLibs` will no longer participate in packaging.

**Tech Stack:** Bash build scripts, Gradle Kotlin DSL, Android Gradle Plugin, Kotlin buildSrc plugin, Tauri Android build flow

---

## File Map

- **Create:** `src-tauri/android-build/static-assets/wechat.png`
  - tracked Android-only asset source
- **Modify:** `src-tauri/gen/android/app/build.gradle.kts`
  - define staged asset/JNI directories, register staging + validation tasks, wire source sets
- **Modify:** `src-tauri/gen/android/buildSrc/src/main/java/com/gjk/cameraftpcompanion/kotlin/RustPlugin.kt`
  - expose Rust output locations and make merge tasks depend on staging tasks after Rust outputs exist
- **Modify:** `scripts/build-common.sh`
  - clean new Gradle-owned staging outputs, not legacy `src/main` staging
- **Verify:** `./build.sh windows android`
- **Verify:** inspect `out/CameraFTP_v1.3.1.apk` contents for assets/JNI payload parity

## Task 1: Add tracked Android static asset source

**Files:**
- Create: `src-tauri/android-build/static-assets/wechat.png`
- Verify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt`

- [ ] **Step 1: Confirm the runtime contract still expects `wechat.png` from APK assets**

Read the existing Android bridge call path and confirm these unchanged lines are still the contract:

```kotlin
// src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt
@JavascriptInterface
fun saveImageToGallery(assetPath: String?): String {
    activity.assets.open(assetPath).use { input ->
        FileOutputStream(destFile).use { output ->
            input.copyTo(output)
        }
    }
}
```

Expected result: the build must continue packaging an APK asset named `wechat.png`.

- [ ] **Step 2: Create the tracked Android asset source directory**

Create this filesystem layout:

```text
src-tauri/
  android-build/
    static-assets/
      wechat.png
```

Copy the current valid `wechat.png` asset into that tracked location unchanged.

- [ ] **Step 3: Verify the tracked asset exists before any Gradle changes**

Run:

```bash
ls "src-tauri/android-build/static-assets/wechat.png"
```

Expected: the file exists and is ready to be staged by Gradle.

## Task 2: Move Android packaging to explicit Gradle staging directories

**Files:**
- Modify: `src-tauri/gen/android/app/build.gradle.kts`

- [ ] **Step 1: Write the intended staging directory declarations**

Add explicit directory definitions near the top of `build.gradle.kts` so all later tasks reuse one source of truth:

```kotlin
val tauriStagingRoot = layout.buildDirectory.dir("tauri-staging")
val tauriStagedAssets = tauriStagingRoot.map { it.dir("assets") }
val tauriStagedJniLibs = tauriStagingRoot.map { it.dir("jniLibs") }
val androidStaticAssets = rootProject.file("../../../android-build/static-assets")
val allowedJniLibraries = setOf("libcamera_ftp_companion_lib.so")
```

The static asset path must resolve from `src-tauri/gen/android/app/` to `src-tauri/android-build/static-assets/` exactly.

- [ ] **Step 2: Replace implicit asset/JNI packaging with explicit source sets**

Add this `sourceSets` block inside `android {}`:

```kotlin
sourceSets {
    getByName("main") {
        assets.srcDirs(tauriStagedAssets)
        jniLibs.srcDirs(tauriStagedJniLibs)
    }
}
```

This ensures APK packaging reads staged inputs from `build/tauri-staging` instead of legacy `src/main/assets` or `src/main/jniLibs` leftovers.

- [ ] **Step 3: Preserve the existing JNI packaging mode while switching inputs**

Keep the existing packaging block intact:

```kotlin
packaging {
    jniLibs {
        useLegacyPackaging = true
    }
}
```

Expected result: packaging behavior stays the same except for the source directories.

## Task 3: Add Gradle tasks that build staged assets and JNI inputs

**Files:**
- Modify: `src-tauri/gen/android/app/build.gradle.kts`

- [ ] **Step 1: Add a cleanup task for `build/tauri-staging`**

Register a dedicated cleanup task:

```kotlin
val cleanTauriStaging by tasks.registering(Delete::class) {
    delete(tauriStagingRoot)
}
```

This task must only touch `build/tauri-staging`, not any `src/main` path.

- [ ] **Step 2: Add a task that stages tracked Android assets**

Register a copy task for static Android assets:

```kotlin
val stageTauriStaticAssets by tasks.registering(Copy::class) {
    dependsOn(cleanTauriStaging)
    from(androidStaticAssets)
    into(tauriStagedAssets)
    include("wechat.png")
}
```

If `androidStaticAssets/wechat.png` is missing, Gradle must fail rather than silently packaging nothing.

- [ ] **Step 3: Add a task that stages generated Android config assets**

Register a task that copies generated config payloads the Android app needs, including `tauri.conf.json`, into staged assets:

```kotlin
val stageTauriGeneratedAssets by tasks.registering(Copy::class) {
    dependsOn(cleanTauriStaging)
    from(layout.projectDirectory.dir("src/main/assets"))
    into(tauriStagedAssets)
    include("tauri.conf.json")
}
```

This step intentionally treats the existing generated `tauri.conf.json` as an input artifact to be copied forward, while cutting off unrelated files in that directory.

- [ ] **Step 4: Add a task that stages only the built Rust JNI library**

Register a copy task that copies only the allowed `.so` outputs into the staged JNI tree:

```kotlin
val stageTauriJniLibs by tasks.registering(Copy::class) {
    dependsOn(cleanTauriStaging)
    into(tauriStagedJniLibs)
}
```

Do not finalize this task yet; Task 4 wires the exact Rust output locations into it.

- [ ] **Step 5: Add a whitelist validation task for staged JNI files**

Register a validation task:

```kotlin
val validateTauriStagedJniLibs by tasks.registering {
    dependsOn(stageTauriJniLibs)
    doLast {
        val stagedRoot = tauriStagedJniLibs.get().asFile
        val unexpected = stagedRoot
            .walkTopDown()
            .filter { it.isFile && it.extension == "so" }
            .map { it.name }
            .filter { it !in allowedJniLibraries }
            .toList()

        if (unexpected.isNotEmpty()) {
            error("Unexpected staged JNI libraries: $unexpected; allowed: $allowedJniLibraries")
        }
    }
}
```

Expected result: accidental `libvips.so`-style payloads fail the build instead of being packaged.

## Task 4: Wire Rust outputs into Gradle-owned JNI staging

**Files:**
- Modify: `src-tauri/gen/android/buildSrc/src/main/java/com/gjk/cameraftpcompanion/kotlin/RustPlugin.kt`
- Modify: `src-tauri/gen/android/app/build.gradle.kts`

- [ ] **Step 1: Expose predictable Rust output paths from the build plugin**

Extend the plugin so the Android app module can derive or publish the built Rust `.so` locations per ABI. Keep the existing task names intact, and use the already observed Rust output pattern under `src-tauri/target/<target-triple>/<profile>/libcamera_ftp_companion_lib.so`, for example:

```kotlin
fun rustOutputFile(targetTriple: String, profile: String): File {
    return File(project.rootDir, "../../../target/$targetTriple/$profile/libcamera_ftp_companion_lib.so")
}
```

- [ ] **Step 2: Make the Rust build tasks materialize outputs before staging**

Preserve these dependencies in `RustPlugin.kt` and extend them so staging waits for Rust builds:

```kotlin
tasks["mergeUniversal${profileCapitalized}JniLibFolders"].dependsOn(buildTask)
tasks["merge${targetArchCapitalized}${profileCapitalized}JniLibFolders"].dependsOn(targetBuildTask)
```

The revised plugin/build logic must guarantee that `stageTauriJniLibs` runs only after the correct target-specific Rust `.so` exists.

- [ ] **Step 3: Populate `stageTauriJniLibs` with the actual ABI-specific Rust output**

Update `build.gradle.kts` so `stageTauriJniLibs` copies only the intended library into the staged ABI folder:

```kotlin
stageTauriJniLibs.configure {
    dependsOn("rustBuildArm64Release", "rustBuildArm64Debug")
    from(rootProject.file("../../../target/aarch64-linux-android/release")) {
        into("arm64-v8a")
        include("libcamera_ftp_companion_lib.so")
    }
}
```

If debug packaging also needs explicit support, mirror the same pattern for `debug` using `src-tauri/target/aarch64-linux-android/debug/libcamera_ftp_companion_lib.so`. The key constraint is unchanged: copy exactly one allowed Android JNI library into staged output.

- [ ] **Step 4: Wire Android merge tasks to the staging and validation tasks**

Add dependencies so both assets and JNI staging complete before Android merge/package tasks run:

```kotlin
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }.configureEach {
    dependsOn(stageTauriStaticAssets, stageTauriGeneratedAssets)
}

tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }.configureEach {
    dependsOn(stageTauriJniLibs, validateTauriStagedJniLibs)
}
```

Expected result: APK packaging consumes only freshly staged content from the current build.

## Task 5: Stop clean/build scripts from depending on legacy staging paths

**Files:**
- Modify: `scripts/build-common.sh`

- [ ] **Step 1: Extend build cleanup to remove the new staging root**

Update `clean_build_cache()` so the clean list includes:

```bash
"src-tauri/gen/android/app/build"
"src-tauri/gen/android/.gradle"
```

Those entries already exist, so no extra legacy `src/main/assets` or `src/main/jniLibs` cleanup should be added. Instead, add an inline comment clarifying that Android staging is now build-owned under `app/build/tauri-staging`.

- [ ] **Step 2: Remove any remaining script assumptions about `src/main` staging**

Search build scripts for these legacy paths and ensure no script treats them as active build inputs:

```bash
src-tauri/gen/android/app/src/main/assets
src-tauri/gen/android/app/src/main/jniLibs
```

Expected result: only Gradle owns Android staging details.

## Task 6: Add a regression check by building and inspecting the APK

**Files:**
- Verify only

- [ ] **Step 1: Remove local pollution from legacy ignored staging paths**

Delete the stale directories from the working tree before verification:

```bash
rm -rf "src-tauri/gen/android/app/src/main/assets" "src-tauri/gen/android/app/src/main/jniLibs"
```

Expected: no old pollution remains that could hide a broken dependency on legacy paths.

- [ ] **Step 2: Run the required full build verification**

Run:

```bash
./build.sh windows android
```

Expected: both targets build successfully.

- [ ] **Step 3: Inspect APK contents to verify the staged payload**

Run:

```bash
python3 - <<'PY'
import zipfile
apk='out/CameraFTP_v1.3.1.apk'
with zipfile.ZipFile(apk) as z:
    print('JNI libs:')
    for name in sorted(n for n in z.namelist() if n.startswith('lib/arm64-v8a/')):
        print(name)
    print('Assets:')
    for name in sorted(n for n in z.namelist() if n.startswith('assets/')):
        print(name)
PY
```

Expected:

```text
JNI libs:
lib/arm64-v8a/libcamera_ftp_companion_lib.so
Assets:
assets/tauri.conf.json
assets/wechat.png
```

No `libvips.so`, `libfftw3.so`, `libzstd.so`, or similar extra payloads should appear.

- [ ] **Step 4: Compare final APK size with the previously clean worktree result**

Run:

```bash
du -h "out/CameraFTP_v1.3.1.apk" ".worktrees/main-merge/out/CameraFTP_v1.3.1.apk"
```

Expected: the main-workspace APK returns near the clean-worktree size instead of remaining at the inflated ~19 MB result.

## Task 7: Final review for determinism and backward compatibility

**Files:**
- Verify: `src-tauri/gen/android/app/build.gradle.kts`
- Verify: `src-tauri/gen/android/buildSrc/src/main/java/com/gjk/cameraftpcompanion/kotlin/RustPlugin.kt`
- Verify: `scripts/build-common.sh`

- [ ] **Step 1: Re-read the packaged asset path contract**

Confirm these behavior-preserving facts remain true:

```text
Frontend still calls saveImageToGallery("wechat.png")
Android bridge still reads activity.assets.open(assetPath)
APK still contains assets/wechat.png
```

- [ ] **Step 2: Confirm legacy ignored paths are no longer real inputs**

Drop a temporary dummy file into a legacy ignored path and verify it does not appear in the APK because source sets now point only at staged build directories.

- [ ] **Step 3: Prepare commit only if explicitly requested**

If the human asks for a commit, stage only the intended files:

```bash
git add \
  "src-tauri/android-build/static-assets/wechat.png" \
  "src-tauri/gen/android/app/build.gradle.kts" \
  "src-tauri/gen/android/buildSrc/src/main/java/com/gjk/cameraftpcompanion/kotlin/RustPlugin.kt" \
  "scripts/build-common.sh" \
  "docs/superpowers/specs/2026-04-04-android-build-staging-normalization-design.md" \
  "docs/superpowers/plans/2026-04-04-android-build-staging-normalization.md"
```

Do not create a commit unless explicitly requested.
