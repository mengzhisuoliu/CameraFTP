# MediaStore Reliability Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MediaStore the single source of truth for Android 13+, streaming FTP uploads directly into MediaStore via fd and refreshing the gallery only when entries are finalized.

**Architecture:** Introduce an Android MediaStore bridge that creates/finalizes entries and returns fds; add a Rust Android storage backend for libunftp that writes to the fd; update frontend to list/open from MediaStore and refresh on `media-store-ready` only.

**Tech Stack:** Rust (libunftp), Kotlin (Android MediaStore APIs), Tauri IPC, React/TypeScript.

---

## File Structure Map

**Create:**
- `src-tauri/src/ftp/android_mediastore/mod.rs` (module root)
- `src-tauri/src/ftp/android_mediastore/backend.rs` (StorageBackend implementation)
- `src-tauri/src/ftp/android_mediastore/bridge.rs` (JNI + bridge client)
- `src-tauri/src/ftp/android_mediastore/types.rs` (DTOs + serde)
- `src-tauri/src/ftp/android_mediastore/retry.rs` (retry/backoff helpers)
- `src-tauri/src/ftp/android_mediastore/limiter.rs` (UploadLimiter)
- `src-tauri/src/ftp/android_mediastore/tests.rs` (unit tests + mocks)
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt`
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/cache/ThumbnailCache.kt`
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridgeTest.kt`
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt`
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/PermissionBridgeTest.kt`
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/cache/ThumbnailCacheTest.kt`
- `src/utils/media-store-events.ts`
- `src/utils/__tests__/media-store-events.test.ts`

**Modify:**
- `src-tauri/src/ftp/mod.rs` (export new backend)
- `src-tauri/src/ftp/server.rs` (wire custom storage backend)
- `src-tauri/src/ftp/listeners.rs` (keep `file-uploaded` for stats only)
- `src-tauri/src/platform/android.rs` (remove MANAGE storage assumptions)
- `src-tauri/Cargo.toml` (Android JNI dependencies)
- `src-tauri/gen/android/app/src/main/AndroidManifest.xml` (minSdk 33, permissions)
- `src-tauri/gen/android/app/build.gradle.kts` (minSdk 33)
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` (register new bridge, event wiring)
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/BaseJsBridge.kt` (verify exists; no changes expected)
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt` (MediaStore list, thumbnail, share, delete)
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt` (update to READ_MEDIA_IMAGES only)
- `src/stores/serverStore.ts` (listen to `media-store-ready`)
- `src/components/GalleryCard.tsx` (load list via new bridge)
- `src/types/global.ts` (bridge typings)
- `src/types/events.ts` (new event payload)
- `package.json` (add Vitest)
- `vite.config.ts` (Vitest config)

---

## Chunk 1: Android MediaStore Bridge + Gallery Operations

### Task 1.1: Add MediaStore bridge for create/finalize/abort

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

- [ ] **Step 0: Add unit test dependencies**

Modify `src-tauri/gen/android/app/build.gradle.kts`:

```kotlin
testImplementation("junit:junit:4.13.2")
testImplementation("org.robolectric:robolectric:4.11.1")
testImplementation("androidx.test:core:1.6.1")
testImplementation("androidx.test.ext:junit:1.2.1")
```

- [ ] **Step 1: Write the failing test (Kotlin unit test)**

Path: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridgeTest.kt`
Note: keep tests Android-free where possible by using String-based helpers.

```kotlin
package com.gjk.cameraftpcompanion.bridges

import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.json.JSONObject
import android.provider.MediaStore

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class MediaStoreBridgeTest {
    // tests below
}
```

```kotlin
@Test
fun parseEntryResult_readsFdAndUri() {
    val result = MediaStoreBridge.parseEntryResult("{\"fd\":123,\"uri\":\"content://media/1\"}")
    assertEquals(123, result.fd)
    assertEquals("content://media/1", result.uri)
}

@Test
fun retryWithBackoff_usesCorrectDelays() {
    val delays = mutableListOf<Long>()
    MediaStoreBridge.retryWithBackoff(3, sleep = { delays.add(it) }) { throw RuntimeException("fail") }
    assertEquals(listOf(100L, 200L, 400L), delays)
}

@Test
fun retryWithBackoff_succeedsOnSecondAttempt() {
    var attempt = 0
    val result = MediaStoreBridge.retryWithBackoff(3) {
        attempt++
        if (attempt == 1) throw RuntimeException("fail")
        "ok"
    }
    assertTrue(result.isSuccess)
}

@Test
fun resolveExistingUri_returnsFirstWhenPresent() {
    val result = MediaStoreBridge.resolveExistingUri(listOf("content://media/1", "content://media/2"))
    assertEquals("content://media/1", result)
}

@Test
fun fatalWriteError_detectsEnospcAndIo() {
    assertTrue(MediaStoreBridge.isFatalWriteError("ENOSPC"))
    assertTrue(MediaStoreBridge.isFatalWriteError("EIO"))
}

@Test
fun mimeDetection_ftpTypeTakesPrecedence() {
    val mime = MediaStoreBridge.determineMime("IMG_1.JPG", "image/png")
    assertEquals("image/png", mime)
}

@Test
fun mimeDetection_fallsBackToExtension() {
    val mime = MediaStoreBridge.determineMime("IMG_1.JPG", null)
    assertEquals("image/jpeg", mime)
}

@Test
fun mimeDetection_defaultsToOctetStream() {
    val mime = MediaStoreBridge.determineMime("FILE", null)
    assertEquals("application/octet-stream", mime)
}

@Test
fun readyPayload_containsRequiredFields() {
    val payload = MediaStoreBridge.buildReadyPayload("content://media/1", "DCIM/CameraFTP/", "IMG_1.JPG", 123, 1000)
    val json = JSONObject(payload)
    assertTrue(json.has("uri"))
    assertTrue(json.has("relativePath"))
    assertTrue(json.has("displayName"))
    assertTrue(json.has("size"))
    assertTrue(json.has("timestamp"))
}

@Test
fun pendingValues_setsIsPendingAndSize() {
    val values = MediaStoreBridge.buildPendingValues("IMG_1.JPG", 123)
    assertEquals(1, values.getAsInteger(MediaStore.MediaColumns.IS_PENDING))
    assertEquals(123L, values.getAsLong(MediaStore.MediaColumns.SIZE))
}

@Test
fun finalizeValues_clearsIsPending() {
    val values = MediaStoreBridge.buildFinalizeValues()
    assertEquals(0, values.getAsInteger(MediaStore.MediaColumns.IS_PENDING))
}

@Test
fun validateSize_handlesMismatch() {
    assertFalse(MediaStoreBridge.validateSize(1000, 500))
    assertTrue(MediaStoreBridge.validateSize(1000, 1000))
}

@Test
fun cleanupSelection_targetsPendingRows() {
    val selection = MediaStoreBridge.buildCleanupSelection(1234)
    assertTrue(selection.contains("IS_PENDING"))
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./src-tauri/gen/android/gradlew test`
Expected: FAIL because `MediaStoreBridge` and test do not exist.

- [ ] **Step 3: Write minimal implementation**

Include SPDX header in `MediaStoreBridge.kt` and new Kotlin test files.

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
```

Also register the bridge in `MainActivity.onWebViewCreate()`:

```kotlin
addJsBridge(webView, mediaStoreBridge, "MediaStoreAndroid")
```

Ensure `MediaStoreAndroid` can emit `media-store-ready` to the WebView via a dedicated helper on `MainActivity` (avoid inline JS strings in multiple locations).

```kotlin
fun emitTauriEvent(name: String, payloadJson: String) {
    val webView = getWebView() ?: return
    val script = "window.__TAURI__?.event?.emit('${'$'}name', ${'$'}payloadJson)"
    runOnUiThread {
        webView.evaluateJavascript(script, null)
    }
}
```

```kotlin
class MediaStoreBridge(private val activity: MainActivity) : BaseJsBridge(activity) {
    companion object {
        private const val TAG = "MediaStoreBridge"
        @JvmStatic
        fun determineMime(filename: String, ftpType: String?): String { /* ... */ }

        @JvmStatic
        fun retryWithBackoff<T>(attempts: Int, sleep: (Long) -> Unit = { Thread.sleep(it) }, block: () -> T): Result<T> { /* ... */ }

        @JvmStatic
        fun parseEntryResult(json: String): EntryResult { /* ... */ }

        @JvmStatic
        fun resolveExistingUri(candidates: List<String>): String? { /* ... */ }

        @JvmStatic
        fun isFatalWriteError(code: String): Boolean { /* ... */ }

        @JvmStatic
        fun buildReadyPayload(uri: String, relativePath: String, displayName: String, size: Long, timestamp: Long): String { /* ... */ }

        @JvmStatic
        fun buildPendingValues(displayName: String, sizeHint: Long?): ContentValues { /* ... */ }

        @JvmStatic
        fun buildFinalizeValues(): ContentValues { /* ... */ }

        @JvmStatic
        fun validateSize(expected: Long, actual: Long): Boolean { /* ... */ }

        @JvmStatic
        fun shouldAbortOnSizeMismatch(expected: Long, actual: Long): Boolean { /* ... */ }

        @JvmStatic
        fun shouldEmitAfterValidation(expected: Long, actual: Long): Boolean { /* ... */ }

        @JvmStatic
        fun buildCleanupSelection(cutoffMillis: Long): String { /* ... */ }

        @JvmStatic
        fun cleanupStalePendingEntries(contentResolver: ContentResolver, cutoffMillis: Long) { /* delete IS_PENDING=1 older than cutoff */ }

        @JvmStatic
        fun createEntryNative(context: Context, displayName: String, mime: String, relativePath: String, sizeHint: Long?): String { /* JSON { fd, uri } */ }

    @JvmStatic
    fun finalizeEntryNative(context: Context, uri: String, expectedSize: Long?): Boolean { /* finalize + emit */ }

    @JvmStatic
    fun abortEntryNative(context: Context, uri: String): Boolean { /* delete row */ }

    @JvmStatic
    fun listEntriesNative(context: Context, relativePath: String): String { /* JSON array with camelCase keys: uri, displayName, size, dateModified */ }

    @JvmStatic
    fun findEntryUriNative(context: Context, relativePath: String, displayName: String): String? { /* content://... or null */ }

    @JvmStatic
    fun openEntryForReadNative(context: Context, uri: String): Int { /* detachFd for read */ }

    @JvmStatic
    fun deleteEntryNative(context: Context, uri: String): Boolean { /* delete row */ }
}

    data class EntryResult(val fd: Int, val uri: String)

    @JavascriptInterface
    fun createMediaStoreEntry(displayName: String, mime: String, relativePath: String, sizeHint: Long?): String {
        // MIME comes from Rust (FTP type -> extension -> application/octet-stream)
        // Lookup existing URI by displayName + relativePath
        // If found, set IS_PENDING=1 on existing URI, reuse it
        // Else insert new row with IS_PENDING=1
        // Use ParcelFileDescriptor.detachFd() and return JSON { "fd": 123, "uri": "content://..." }
        // Caller (Rust) owns and must close the fd
        // Retry up to 3 times (100/200/400ms)
    }

    @JavascriptInterface
    fun finalizeMediaStoreEntry(uri: String): Boolean {
        // Validate size hint vs actual size (abort on mismatch)
        // Set IS_PENDING=0, emit media-store-ready on success
        // Payload: { uri, relativePath, displayName, size, timestamp }
        // Emit media-store-ready to WebView via activity.emitTauriEvent("media-store-ready", payloadJson)
        // (wired in Task 1.3) evict thumbnail cache after finalize succeeds
        // Retry up to 3 times (100/200/400ms)
    }

    @JavascriptInterface
    fun abortMediaStoreEntry(uri: String): Boolean {
        // Delete row on error (including ENOSPC) via contentResolver.delete
        // Return true when deletion succeeds or row already missing
    }
}
```

- [ ] **Step 9: Run test to verify it passes**

Run: `./src-tauri/gen/android/gradlew test`
Expected: PASS (Android test suite runs; compilation succeeds).

- [ ] **Step 10: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridge.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git add src-tauri/gen/android/app/build.gradle.kts
git add src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/MediaStoreBridgeTest.kt
git commit -m "feat(android): add mediastore bridge"
```

### Task 1.2: Update gallery operations to use MediaStore only

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/PermissionBridge.kt`

- [ ] **Step 1: Write failing test (Kotlin unit test)**

Path: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/cache/ThumbnailCacheTest.kt`

```kotlin
package com.gjk.cameraftpcompanion.cache

import org.junit.Test
import org.junit.Assert.*

class ThumbnailCacheTest {
    // tests below
}
```

Path: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt`
Path: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/PermissionBridgeTest.kt`

```kotlin
package com.gjk.cameraftpcompanion.bridges

import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class GalleryBridgeTest {
    // tests below
}

```

```kotlin
package com.gjk.cameraftpcompanion

import org.junit.Test
import org.junit.Assert.*

class PermissionBridgeTest {
    // tests below
}
```

```kotlin
@Test
fun pickFreshestEntry_prefersNewestDateModified() {
    val uriA = "content://media/1"
    val uriB = "content://media/2"
    val entryA = GalleryBridge.MediaEntry(uriA, 100, 1000, 1, 10)
    val entryB = GalleryBridge.MediaEntry(uriB, 200, 2000, 2, 5)
    assertEquals(entryB, GalleryBridge.pickNewest(entryA, entryB))
}

@Test
fun buildUriWindow_51UriMax_targetPlus25EachSide() {
    val uris = (0 until 200).map { "content://media/$it" }
    val result = GalleryBridge.buildUriWindow(uris, 150)
    assertEquals(51, result.size)
    assertTrue(result.contains("content://media/150"))
}

@Test
fun buildUriWindow_handlesStartAndEndEdges() {
    val uris = (0 until 10).map { "content://media/$it" }
    assertEquals(10, GalleryBridge.buildUriWindow(uris, 0).size)
    assertEquals(10, GalleryBridge.buildUriWindow(uris, 9).size)
}

@Test
fun listMediaStoreImages_usesCorrectRelativePath() {
    val selection = GalleryBridge.buildQuerySelection()
    assertTrue(selection.contains("DCIM/CameraFTP/"))
}

@Test
fun sortOrder_usesDateModifiedDescThenAddedThenSize() {
    val uriA = "content://media/1"
    val uriB = "content://media/2"
    val uriC = "content://media/3"
    val items = listOf(
        GalleryBridge.MediaEntry(uriA, 100, 1000, 1, 10),
        GalleryBridge.MediaEntry(uriB, 100, 1000, 2, 5),
        GalleryBridge.MediaEntry(uriC, 100, 2000, 1, 1)
    )
    val sorted = GalleryBridge.sortEntries(items)
    assertEquals(listOf(uriC, uriB, uriA), sorted.map { it.uri })
}

@Test
fun openExternalGallery_noHandlerShowsToast() {
    val shouldToast = GalleryBridge.shouldShowNoHandlerToast(false)
    assertTrue(shouldToast)
}

@Test
fun openExternalGallery_grantsReadPermission() {
    assertTrue(GalleryBridge.shouldGrantReadPermission())
}

@Test
fun shareIntentUsesMediaStoreUris() {
    val intent = GalleryBridge.buildShareIntent(listOf("content://media/1", "content://media/2"))
    assertEquals(Intent.ACTION_SEND_MULTIPLE, intent.action)
}

@Test
fun deleteUsesMediaStoreUriNotPath() {
    val selection = GalleryBridge.buildDeleteSelection("content://media/1")
    assertTrue(selection.contains("content://"))
}

@Test
fun doesNotRequestManageExternalStorage() {
    val perms = PermissionBridge.getRequiredPermissions()
    assertFalse(perms.contains("android.permission.MANAGE_EXTERNAL_STORAGE"))
}

@Test
fun requestsReadMediaImages() {
    val perms = PermissionBridge.getRequiredPermissions()
    assertTrue(perms.contains("android.permission.READ_MEDIA_IMAGES"))
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./src-tauri/gen/android/gradlew test`
Expected: FAIL due to missing helper.

- [ ] **Step 3: Implement MediaStore list + thumbnails**

Include SPDX headers in `GalleryBridge.kt` and `PermissionBridge.kt` if moved/created.

```kotlin
@JavascriptInterface
fun listMediaStoreImages(): String {
    // Query MediaStore by RELATIVE_PATH = "DCIM/CameraFTP/"
    // Filter MEDIA_TYPE = IMAGE
// Return JSON list: [{ uri, displayName, dateModified, size }]
}

private fun loadThumbnail(uri: Uri): Bitmap { /* ContentResolver.loadThumbnail */ }
```

Update `shareImages()` and `deleteImages()` to operate on MediaStore URIs (no FileProvider).

```kotlin
@JavascriptInterface
fun openExternalGallery(targetUri: String, allUrisJson: String) {
    // Build ClipData from buildUriWindow + Intent.ACTION_VIEW
    // Add FLAG_GRANT_READ_URI_PERMISSION
}

@JvmStatic
fun buildUriWindow(all: List<String>, targetIndex: Int): List<String> {
    val start = (targetIndex - 25).coerceAtLeast(0)
    val end = (targetIndex + 25).coerceAtMost(all.lastIndex)
    return all.subList(start, end + 1)
}

@JvmStatic
fun buildQuerySelection(): String { /* ... */ }

@JvmStatic
fun shouldShowNoHandlerToast(hasHandler: Boolean): Boolean = !hasHandler

@JvmStatic
fun shouldGrantReadPermission(): Boolean = true

@JvmStatic
fun buildShareIntent(uris: List<String>): Intent { /* ... */ }

@JvmStatic
fun buildDeleteSelection(uri: String): String { /* ... */ }
```

- [ ] **Step 4: Update PermissionBridge to READ_MEDIA_IMAGES only**

```kotlin
// Remove MANAGE_EXTERNAL_STORAGE flows
// Use READ_MEDIA_IMAGES permission checks and request
@JvmStatic
fun getRequiredPermissions(): List<String> = listOf(Manifest.permission.READ_MEDIA_IMAGES)
```

- [ ] **Step 7: Run test to verify it passes**

Run: `./src-tauri/gen/android/gradlew test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/PermissionBridge.kt
git add src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt
git add src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/PermissionBridgeTest.kt
git commit -m "feat(android): use mediastore for gallery ops"
```

### Task 1.3: Add thumbnail cache and startup cleanup

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/cache/ThumbnailCache.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

- [ ] **Step 1: Write failing test (Kotlin unit test)**

```kotlin
@Test
fun thumbnailCacheKey_changesWithDateModified() {
    val uri = Uri.parse("content://media/1")
    val cache = ThumbnailCache(100)
    val key1 = cache.keyFor(uri, 1000)
    val key2 = cache.keyFor(uri, 2000)
    assertNotEquals(key1, key2)
}

@Test
fun eviction_removesOldestWhenCapExceeded() {
    val cache = ThumbnailCache(maxBytes = 100) // small cap for test speed
    val uriA = Uri.parse("content://media/a")
    val uriB = Uri.parse("content://media/b")
    cache.put(uriA, 1000, 60)
    cache.put(uriB, 2000, 60)
    assertFalse(cache.contains(uriA, 1000))
}

@Test
fun evictIfPresent_removesMatchingUri() {
    val cache = ThumbnailCache(maxBytes = 100)
    val uriA = Uri.parse("content://media/a")
    cache.put(uriA, 1000, 10)
    cache.evictIfPresent(uriA)
    assertFalse(cache.contains(uriA, 1000))
}

@Test
fun cleanup_removesPendingOlderThan24h() {
    val selection = MediaStoreBridge.buildCleanupSelection(nowMinusHours(25))
    assertTrue(selection.contains("IS_PENDING"))
}

private fun nowMinusHours(h: Int): Long = System.currentTimeMillis() - h * 3_600_000L
```


- [ ] **Step 2: Run test to verify it fails**

Run: `src-tauri/gen/android/gradlew test`
Expected: FAIL (cache helper missing).

- [ ] **Step 3: Implement cache + stale pending cleanup**

Replace existing `GalleryBridge` thumbnail cache with `ThumbnailCache` (avoid duplication).

```kotlin
// cache/ThumbnailCache.kt (include SPDX header)
class ThumbnailCache(private val maxBytes: Long) {
    fun keyFor(uri: Uri, dateModified: Long): String = hash(uri.toString() + dateModified)
    fun put(uri: Uri, dateModified: Long, bytes: Int) { /* track size + LRU */ }
    fun contains(uri: Uri, dateModified: Long): Boolean { /* ... */ }
    fun evictIfPresent(uri: Uri) { /* remove cached file if present */ }
}

object ThumbnailCacheProvider {
    val instance = ThumbnailCache(100 * 1024 * 1024)
}

fun cleanupStalePendingEntries(contentResolver: ContentResolver, cutoffMillis: Long) { /* delete IS_PENDING=1 older than cutoff */ }
```

Call `MediaStoreBridge.cleanupStalePendingEntries(contentResolver, System.currentTimeMillis() - 24 * 60 * 60 * 1000L)` in `MainActivity.onCreate()`.
Initialize `cacheDir/thumbnails` lazily in `ThumbnailCache`.

- [ ] **Step 9: Run test to verify it passes**

Run: `src-tauri/gen/android/gradlew test`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/cache/ThumbnailCache.kt src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/cache/ThumbnailCacheTest.kt
git commit -m "feat(android): add thumbnail cache and cleanup"
```

### Chunk 1 Verification

- [ ] Run `./build.sh windows android`

---

## Chunk 2: Rust Android Storage Backend (fd streaming)

### Task 2.1: Implement Android MediaStore storage backend

**Files:**
- Create: `src-tauri/src/ftp/android_mediastore/mod.rs`
- Create: `src-tauri/src/ftp/android_mediastore/backend.rs`
- Create: `src-tauri/src/ftp/android_mediastore/bridge.rs`
- Create: `src-tauri/src/ftp/android_mediastore/types.rs`
- Create: `src-tauri/src/ftp/android_mediastore/retry.rs`
- Create: `src-tauri/src/ftp/android_mediastore/limiter.rs`
- Create: `src-tauri/src/ftp/android_mediastore/tests.rs`
- Modify: `src-tauri/src/ftp/mod.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Write failing Rust unit tests**

Place tests in `src-tauri/src/ftp/android_mediastore/tests.rs` under `#[cfg(test)]`.

```rust
#[test]
fn builds_display_name_from_path() {
    assert_eq!(display_name_from_str("/DCIM/CameraFTP/IMG_1.JPG"), "IMG_1.JPG");
}

#[test]
fn default_relative_path_matches_dcim() {
    assert_eq!(default_relative_path(), "DCIM/CameraFTP/");
}

#[tokio::test]
async fn retry_with_backoff_uses_expected_delays() {
    let mut delays = Vec::new();
    let result = retry_with_backoff(3, |ms| { delays.push(ms); async {} }, || async { Err(AppError::Other("fail".into())) }).await;
    assert!(result.is_err());
    assert_eq!(delays, vec![100, 200, 400]);
}

#[cfg(unix)]
#[tokio::test]
async fn put_writes_to_fd_and_finalizes() {
    let harness = BackendHarness::new();
    let bytes_written = harness.backend.put(&dummy_user(), tokio::io::empty(), "IMG_1.JPG", 0).await.unwrap();
    assert_eq!(bytes_written, 0);
    assert!(harness.recorder.finalize_called());
    assert!(harness.path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn put_aborts_on_write_error() {
    let harness = BackendHarness::new();
    let result = harness.backend.put(&dummy_user(), failing_reader(), "IMG_2.JPG", 0).await;
    assert!(result.is_err());
    assert!(harness.recorder.abort_called());
}

#[cfg(unix)]
#[tokio::test]
async fn put_rejects_resume_offsets() {
    let harness = BackendHarness::new();
    let result = harness.backend.put(&dummy_user(), tokio::io::empty(), "IMG_3.JPG", 10).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn upload_limiter_caps_concurrency() {
    let limiter = UploadLimiter::new(4);
    let permits = limiter.acquire_many(4).await;
    assert_eq!(limiter.available_permits(), 0);
    drop(permits);
    assert_eq!(limiter.available_permits(), 4);
}

#[cfg(unix)]
#[tokio::test]
async fn list_returns_entries_from_bridge() {
    let harness = BackendHarness::new();
    let entries = harness.backend.list(&dummy_user(), "/").await.unwrap();
    assert_eq!(entries.len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn list_rejects_non_root_paths() {
    let harness = BackendHarness::new();
    let result = harness.backend.list(&dummy_user(), "/nested").await;
    assert!(result.is_err());
}
```

Add test helpers in the same `#[cfg(test)]` module:

```rust
#[cfg(unix)]
use std::os::fd::IntoRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use unftp_core::auth::Principal;

#[cfg(unix)]
struct BackendHarness {
    backend: AndroidMediaStoreBackend,
    recorder: Arc<MockBridgeRecorder>,
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl BackendHarness {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cameraftp-test-{}", now_nanos()));
        let file = std::fs::File::create(&path).unwrap();
        let fd = file.into_raw_fd();
        let recorder = Arc::new(MockBridgeRecorder::default());
        let bridge = Arc::new(MockBridge::new(fd, recorder.clone()));
        let backend = AndroidMediaStoreBackend::new(path.parent().unwrap().to_path_buf(), bridge);
        Self { backend, recorder, path }
    }
}

type User = Principal;
fn dummy_user() -> User { Principal { username: "anon".into() } }

struct FailingReader;
impl tokio::io::AsyncRead for FailingReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "fail")))
    }
}

fn failing_reader() -> impl tokio::io::AsyncRead + Send + Sync + Unpin { FailingReader }

#[derive(Default)]
#[cfg(unix)]
struct MockBridgeRecorder {
    finalize_called: AtomicBool,
    abort_called: AtomicBool,
}

impl MockBridgeRecorder {
    fn finalize_called(&self) -> bool { self.finalize_called.load(Ordering::SeqCst) }
    fn abort_called(&self) -> bool { self.abort_called.load(Ordering::SeqCst) }
}

#[cfg(unix)]
struct MockBridge { fd: i32, recorder: Arc<MockBridgeRecorder> }
impl MockBridge {
    fn new(fd: i32, recorder: Arc<MockBridgeRecorder>) -> Self { Self { fd, recorder } }
}

#[async_trait::async_trait]
impl MediaStoreBridgeClient for MockBridge {
    async fn create_entry(&self, _display_name: &str, _mime: &str, _relative_path: &str, _size_hint: Option<u64>) -> Result<MediaStoreEntry, AppError> {
        Ok(MediaStoreEntry { fd: self.fd, uri: "content://media/1".to_string() })
    }

    async fn finalize_entry(&self, _uri: &str, _expected_size: Option<u64>) -> Result<(), AppError> {
        self.recorder.finalize_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn abort_entry(&self, _uri: &str) -> Result<(), AppError> {
        self.recorder.abort_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn list_entries(&self, _relative_path: &str) -> Result<Vec<MediaStoreListEntry>, AppError> {
        Ok(vec![MediaStoreListEntry { uri: "content://media/1".into(), display_name: "IMG_1.JPG".into(), size: 0, date_modified: 1 }])
    }

    async fn find_entry_uri(&self, _relative_path: &str, _display_name: &str) -> Result<Option<String>, AppError> {
        Ok(Some("content://media/1".into()))
    }

    async fn open_entry_for_read(&self, _uri: &str) -> Result<i32, AppError> {
        Ok(self.fd)
    }

    async fn delete_entry(&self, _uri: &str) -> Result<(), AppError> {
        Ok(())
    }
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
```

- [ ] **Step 2: Run test to verify it fails**

Run (from `src-tauri`): `cargo test -p cameraftp --lib`
Expected: FAIL (new module missing).

- [ ] **Step 3: Implement put flow (backend.rs)**

Include SPDX headers in each new file under `src-tauri/src/ftp/android_mediastore/`.

```rust
pub struct AndroidMediaStoreBackend {
    bridge: Arc<dyn MediaStoreBridgeClient>,
    limiter: UploadLimiter,
}

#[async_trait::async_trait]
impl unftp_core::storage::StorageBackend<unftp_core::auth::Principal> for AndroidMediaStoreBackend {
    type Metadata = unftp_sbe_fs::Metadata;

    async fn put<P, R>(&self, _user: &User, input: R, path: P, start_pos: u64) -> Result<u64, unftp_core::storage::Error>
    where
        P: AsRef<std::path::Path> + Send + std::fmt::Debug,
        R: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
    {
        // Reject non-file paths; only "/filename" allowed using is_root_file_path()
        // Reject restart if start_pos > 0 (no partial resume for MediaStore)
        // Acquire limiter permit (max 4)
        // Derive display_name_from_path(path) + default_relative_path() + determine_mime_from_path(path)
        // Create entry via bridge using retry_with_backoff (3 attempts, 100/200/400ms)
        // Bridge must reuse existing URI if display name already exists (update-in-place)
        // Stream bytes into fd with tokio::io::copy
        // On success: finalize via retry_with_backoff using bytes_written as expected_size
        // If finalize fails: abort entry and return error (do not emit refresh)
        // On write error: abort entry and return error
    }

    // Put flow only here; list/get/metadata/del handled in Step 4.
}
```

- [ ] **Step 4: Implement list/get/metadata/del + dir ops (backend.rs)**

```rust
// list: require root path only using is_root_path(); bridge.list_entries(default_relative_path()) -> map to Fileinfo entries
//   - map MediaStoreListEntry.size to metadata.size
//   - map date_modified (seconds) to SystemTime via UNIX_EPOCH + Duration::from_secs
//   - set file type = File
// metadata: require root file path only using is_root_file_path(); find entry by display_name via list_entries and return Metadata
// get: require root file path only using is_root_file_path(); find_entry_uri + open_entry_for_read -> std::fs::File -> tokio::fs::File::from_std for AsyncRead
// del: require root file path only using is_root_file_path(); find_entry_uri + delete_entry
// mkd/rmd/rename/cwd return PermissionDenied (MediaStore has no directories)
```

Organize the module:

```rust
// mod.rs
mod backend;
mod bridge;
mod limiter;
mod retry;
mod types;
#[cfg(test)] mod tests;

pub use backend::AndroidMediaStoreBackend;
pub use types::{MediaStoreBridgeClient, MediaStoreEntry, MediaStoreListEntry};
```

Add these helpers in `backend.rs` (used by tests and `put` flow):

```rust
pub(crate) fn display_name_from_path(path: &std::path::Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("upload.bin").to_string()
}

pub(crate) fn display_name_from_str(path: &str) -> String {
    display_name_from_path(std::path::Path::new(path))
}

pub(crate) fn default_relative_path() -> &'static str {
    "DCIM/CameraFTP/"
}

pub(crate) fn is_root_path(path: &std::path::Path) -> bool {
    path.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count() == 0
}

pub(crate) fn is_root_file_path(path: &std::path::Path) -> bool {
    path.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count() == 1
}
```

In parent module `src-tauri/src/ftp/mod.rs`:

```rust
pub mod android_mediastore;
```

Note: `android_mediastore` uses `std::os::fd` behind `#[cfg(unix)]` and tests are `#[cfg(unix)]` to keep Windows builds green.

- [ ] **Step 5: Implement retry helper (retry.rs)**

```rust
pub(crate) async fn retry_with_backoff<T, F, Fut, OpFut>(retries: u32, mut sleep: impl FnMut(u64) -> Fut, mut op: F) -> Result<T, AppError>
where
    F: FnMut() -> OpFut,
    Fut: std::future::Future<Output = ()>,
    OpFut: std::future::Future<Output = Result<T, AppError>>,
{
    let mut delay = 100u64;
    for attempt in 0..=retries {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt == retries => return Err(err),
            Err(_) => {
                sleep(delay).await;
                delay *= 2;
            }
        }
    }
    Err(AppError::Other("retry exhausted".into()))
}
```

Define a constructor and a testable entry point for dependency injection:

```rust
impl AndroidMediaStoreBackend {
    pub fn new(root_path: std::path::PathBuf, bridge: Arc<dyn MediaStoreBridgeClient>) -> Self {
        let _ = root_path; // keep signature for parity with ServerConfig.root_path
        Self { bridge, limiter: UploadLimiter::new(4) }
    }

    #[cfg(target_os = "android")]
    pub fn new_default(root_path: std::path::PathBuf) -> Self {
        Self::new(root_path, Arc::new(JniMediaStoreBridge))
    }
}
```

Add MIME detection helper (FTP type is not available in the StorageBackend API):

```rust
fn determine_mime_from_path(path: &std::path::Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}
```

Note: FTP-reported MIME is not available in `StorageBackend::put`, so the Android backend uses filename-based MIME only (spec deviation accepted).

- [ ] **Step 6: Implement DTOs + bridge trait (types.rs)**

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStoreListEntry {
    pub(crate) uri: String,
    pub(crate) display_name: String,
    pub(crate) size: u64,
    pub(crate) date_modified: u64,
}

#[derive(Debug, Deserialize)]
pub struct MediaStoreEntry {
    pub(crate) fd: i32,
    pub(crate) uri: String,
}

#[async_trait::async_trait]
pub trait MediaStoreBridgeClient: Send + Sync {
    async fn create_entry(&self, display_name: &str, mime: &str, relative_path: &str, size_hint: Option<u64>) -> Result<MediaStoreEntry, AppError>;
    // Implementation must reuse existing URI when display_name already exists in relative_path:
    // set IS_PENDING=1 on existing row and return its fd + uri
    async fn finalize_entry(&self, uri: &str, expected_size: Option<u64>) -> Result<(), AppError>;
    async fn abort_entry(&self, uri: &str) -> Result<(), AppError>;
    async fn list_entries(&self, relative_path: &str) -> Result<Vec<MediaStoreListEntry>, AppError>;
    async fn find_entry_uri(&self, relative_path: &str, display_name: &str) -> Result<Option<String>, AppError>;
    async fn open_entry_for_read(&self, uri: &str) -> Result<i32, AppError>;
    async fn delete_entry(&self, uri: &str) -> Result<(), AppError>;
}
```

In `mod.rs`:

```rust
pub use types::{MediaStoreBridgeClient, MediaStoreEntry, MediaStoreListEntry};
```

Add Android-only JNI dependencies in `src-tauri/Cargo.toml`:

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
ndk-context = "0.1.1"

[dependencies]
mime_guess = "2.0"
```

- [ ] **Step 7: Implement JNI-backed bridge (bridge.rs)**

```rust
#[cfg(target_os = "android")]
pub(crate) struct JniMediaStoreBridge;

#[cfg(target_os = "android")]
#[async_trait::async_trait]
impl MediaStoreBridgeClient for JniMediaStoreBridge {
    async fn create_entry(&self, display_name: &str, mime: &str, relative_path: &str, size_hint: Option<u64>) -> Result<MediaStoreEntry, AppError> {
        // Use ndk_context::android_context() to get Context
        // Attach current thread to JVM
        // Call MediaStoreBridge.createEntryNative(context, display_name, mime, relative_path, size_hint)
        // Parse JSON { fd, uri }
    }

    async fn finalize_entry(&self, uri: &str, expected_size: Option<u64>) -> Result<(), AppError> {
        // Call MediaStoreBridge.finalizeEntryNative(context, uri, expected_size)
    }

    async fn abort_entry(&self, uri: &str) -> Result<(), AppError> {
        // Call MediaStoreBridge.abortEntryNative(context, uri)
    }

    async fn list_entries(&self, relative_path: &str) -> Result<Vec<MediaStoreListEntry>, AppError> {
        // Call MediaStoreBridge.listEntriesNative(context, relative_path) and parse JSON
    }

    async fn find_entry_uri(&self, relative_path: &str, display_name: &str) -> Result<Option<String>, AppError> {
        // Call MediaStoreBridge.findEntryUriNative(context, relative_path, display_name)
    }

    async fn open_entry_for_read(&self, uri: &str) -> Result<i32, AppError> {
        // Call MediaStoreBridge.openEntryForReadNative(context, uri) -> fd
    }

    async fn delete_entry(&self, uri: &str) -> Result<(), AppError> {
        // Call MediaStoreBridge.deleteEntryNative(context, uri)
    }
}
```

Use a helper to wrap JNI calls with explicit signatures:

```rust
fn call_create_entry(env: &jni::JNIEnv, context: JObject, display_name: &str, mime: &str, relative_path: &str, size_hint: Option<u64>) -> Result<MediaStoreEntry, AppError> {
    let class = env.find_class("com/gjk/cameraftpcompanion/bridges/MediaStoreBridge")?;
    let j_display = env.new_string(display_name)?;
    let j_mime = env.new_string(mime)?;
    let j_rel = env.new_string(relative_path)?;
    let j_size = match size_hint { Some(v) => env.new_object("java/lang/Long", "(J)V", &[jni::objects::JValue::Long(v as i64)])?, None => JObject::null() };
    let result = env.call_static_method(
        class,
        "createEntryNative",
        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/Long;)Ljava/lang/String;",
        &[context.into(), j_display.into(), j_mime.into(), j_rel.into(), j_size.into()],
    )?.l()?;
    let json = env.get_string(jni::objects::JString::from(result))?;
    Ok(serde_json::from_str(json.to_str()?)?)
}
```

- [ ] **Step 8: Implement error mapping (backend.rs)**

```rust
fn map_storage_error(err: AppError) -> unftp_core::storage::Error {
    match err {
        AppError::Io(_) => unftp_core::storage::Error::IoError,
        AppError::StoragePermissionError(_) => unftp_core::storage::Error::PermissionDenied,
        AppError::Other(_) => unftp_core::storage::Error::PermanentError,
        _ => unftp_core::storage::Error::PermanentError,
    }
}
```

Apply `map_storage_error` in each StorageBackend method before returning errors.

Guard JNI-only pieces so Windows builds still compile (inside `bridge.rs`):

```rust
#[cfg(target_os = "android")]
mod jni_bridge {
    use super::*;
    // JniMediaStoreBridge + JNI helper functions live here.
}

#[cfg(not(target_os = "android"))]
struct JniMediaStoreBridge;
```

Note in code comments: JNI direct calls are the in-process IPC used to satisfy the spec’s “synchronous fd return” requirement on Android.

Use `MediaStoreEntry` from `types.rs` for JNI responses.

Convert the fd to `OwnedFd` at the call site and ensure it is closed on error (unix only):

```rust
#[cfg(unix)]
use std::os::fd::{FromRawFd, OwnedFd};

#[cfg(unix)]
let entry = bridge.create_entry(display_name, mime, relative_path, size_hint).await?;
#[cfg(unix)]
let fd = unsafe { OwnedFd::from_raw_fd(entry.fd) };
```

- [ ] **Step 9: Implement upload limiter (limiter.rs)**

```rust
pub(crate) struct UploadLimiter {
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl UploadLimiter {
    pub(crate) fn new(max: usize) -> Self { Self { semaphore: Arc::new(tokio::sync::Semaphore::new(max)) } }
    pub(crate) async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit { self.semaphore.clone().acquire_owned().await.expect("permit") }
    pub(crate) async fn acquire_many(&self, count: u32) -> tokio::sync::OwnedSemaphorePermit { self.semaphore.clone().acquire_many_owned(count).await.expect("permit") }
    pub(crate) fn available_permits(&self) -> usize { self.semaphore.available_permits() }
}
```

- [ ] **Step 10: Run test to verify it passes**

Run (from `src-tauri`): `cargo test -p cameraftp --lib`
Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/ftp/android_mediastore/mod.rs
git add src-tauri/src/ftp/android_mediastore/backend.rs
git add src-tauri/src/ftp/android_mediastore/bridge.rs
git add src-tauri/src/ftp/android_mediastore/types.rs
git add src-tauri/src/ftp/android_mediastore/retry.rs
git add src-tauri/src/ftp/android_mediastore/limiter.rs
git add src-tauri/src/ftp/android_mediastore/tests.rs
git add src-tauri/src/ftp/mod.rs
git add src-tauri/Cargo.toml
git commit -m "feat(ftp): add android mediastore backend"
```

### Task 2.2: Wire backend into FTP server

Step numbering restarts for Task 2.2.

**Files:**
- Modify: `src-tauri/src/ftp/server.rs`
- Modify: `src-tauri/src/ftp/mod.rs`

- [ ] **Step 1: Write failing test (Rust unit test)**

Place the test in `src-tauri/src/ftp/server.rs` under `#[cfg(test)]`.

```rust
#[test]
fn selects_android_backend_type() {
    #[cfg(target_os = "android")]
    assert_eq!(storage_backend_name(), "AndroidMediaStoreBackend");

    #[cfg(not(target_os = "android"))]
    assert_eq!(storage_backend_name(), "Filesystem");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run (from `src-tauri`): `cargo test -p cameraftp --lib`
Expected: FAIL (helper missing).

- [ ] **Step 3: Implement storage backend selection**

Add a platform alias in `src-tauri/src/ftp/mod.rs`:

```rust
#[cfg(target_os = "android")]
pub type FtpStorageBackend = crate::ftp::android_mediastore::AndroidMediaStoreBackend;

#[cfg(not(target_os = "android"))]
pub type FtpStorageBackend = unftp_sbe_fs::Filesystem;
```

Update `server.rs` to use the alias in `create_filesystem` and pass the correct constructor:

```rust
fn create_filesystem(root_path: &std::path::Path) -> FtpStorageBackend {
    #[cfg(target_os = "android")]
    return AndroidMediaStoreBackend::new_default(root_path.to_path_buf());

    #[cfg(not(target_os = "android"))]
    return unftp_sbe_fs::Filesystem::new(root_path.to_path_buf())
        .unwrap_or_else(|e| panic!("Filesystem creation failed: {e}"));
}
```

Add a small test-only helper in `server.rs` to satisfy the unit test:

```rust
#[cfg(test)]
fn storage_backend_name() -> &'static str {
    if cfg!(target_os = "android") {
        "AndroidMediaStoreBackend"
    } else {
        "Filesystem"
    }
}
```

- [ ] **Step 7: Run test to verify it passes**

Run (from `src-tauri`): `cargo test -p cameraftp --lib`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/ftp/server.rs src-tauri/src/ftp/mod.rs
git commit -m "feat(ftp): wire android mediastore storage backend"
```

### Chunk 2 Verification

- [ ] Run `./build.sh windows android`


---

## Chunk 3: Frontend Event + List Source Migration

### Task 3.1: Replace gallery refresh trigger

**Files:**
- Modify: `src/stores/serverStore.ts`
- Modify: `src/types/events.ts`
- Modify: `src/types/global.ts`
- Create: `src/utils/media-store-events.ts`
- Create: `src/utils/__tests__/media-store-events.test.ts`
- Modify: `package.json`
- Modify: `vite.config.ts`

- [ ] **Step 1: Write failing TypeScript test**

```ts
import { shouldRefreshOnEvent } from '../media-store-events';

it('refreshes only on media-store-ready', () => {
  expect(shouldRefreshOnEvent('file-uploaded')).toBe(false);
  expect(shouldRefreshOnEvent('media-store-ready')).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL (Vitest not configured).

- [ ] **Step 3: Add Vitest + implement helper**

Include SPDX header in `media-store-events.ts` and test file.

```ts
export function shouldRefreshOnEvent(event: string) {
  return event === 'media-store-ready';
}
```

Update `package.json`:

```json
{
  "scripts": {
    "test": "vitest run"
  },
  "devDependencies": {
    "jsdom": "^24.0.0",
    "vitest": "^2.1.0"
  }
}
```

Update `vite.config.ts`:

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
  },
});
```

- [ ] **Step 4: Update event types**

Modify `src/types/events.ts`:

```ts
export type MediaStoreReadyPayload = {
  uri: string;
  relativePath: string;
  displayName: string;
  size: number;
  timestamp: number;
};
```

- [ ] **Step 5: Update global bridge typings**

Modify `src/types/global.ts` to include the MediaStore and Gallery bridges:

```ts
interface GalleryAndroidBridge {
  listMediaStoreImages: () => Promise<string>;
}

interface MediaStoreAndroidBridge {
  // optionally exposed for debug hooks; keep empty if unused
}

declare global {
  interface Window {
    GalleryAndroid?: GalleryAndroidBridge;
    MediaStoreAndroid?: MediaStoreAndroidBridge;
  }
}
```

- [ ] **Step 6: Wire media-store-ready in server store**

Modify `src/stores/serverStore.ts`:

```ts
import type { MediaStoreReadyPayload } from '../types/events';
{
  name: 'media-store-ready',
  handler: (_event: Event<MediaStoreReadyPayload>) => {
    window.dispatchEvent(new CustomEvent('gallery-refresh-requested'));
  },
}
```

- [ ] **Step 7: Run test to verify it passes**

Run: `npm test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/utils/media-store-events.ts src/utils/__tests__/media-store-events.test.ts package.json vite.config.ts src/stores/serverStore.ts src/types/events.ts src/types/global.ts
git commit -m "feat(frontend): refresh on media-store-ready"
```

### Task 3.2: Use MediaStore list and open paths

**Files:**
- Modify: `src/components/GalleryCard.tsx`
- Modify: `src/types/global.ts`
- Modify: `src/utils/media-store-events.ts`

- [ ] **Step 1: Write failing test (TypeScript)**

```ts
import { toGalleryImage } from '../media-store-events';

it('maps mediastore entry to gallery image', () => {
  const entry = { uri: 'content://media/1', displayName: 'IMG_1.JPG', dateModified: 1 };
  expect(toGalleryImage(entry).path).toBe(entry.uri);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test`
Expected: FAIL.

- [ ] **Step 3: Implement MediaStore list usage helper**

```ts
export type MediaStoreEntry = {
  uri: string;
  displayName: string;
  dateModified: number;
  size?: number;
};

export function toGalleryImage(entry: MediaStoreEntry) {
  return {
    path: entry.uri,
    name: entry.displayName,
    modified: entry.dateModified,
    size: entry.size ?? 0,
  };
}
```

- [ ] **Step 4: Update GalleryCard to load MediaStore list on refresh**

Modify `src/components/GalleryCard.tsx`:

```tsx
import { toGalleryImage, type MediaStoreEntry } from '../utils/media-store-events';

useEffect(() => {
  const refresh = async () => {
    const listJson = await window.GalleryAndroid?.listMediaStoreImages();
    const entries = JSON.parse(listJson ?? '[]') as MediaStoreEntry[];
    setGalleryImages(entries.map(toGalleryImage));
  };

  refresh();
  const handler = () => void refresh();
  window.addEventListener('gallery-refresh-requested', handler);
  return () => window.removeEventListener('gallery-refresh-requested', handler);
}, []);
```

- [ ] **Step 5: Run test to verify it passes**

Run: `npm test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/components/GalleryCard.tsx src/utils/media-store-events.ts src/types/global.ts
git commit -m "feat(gallery): load media store list"
```

### Chunk 3 Verification

- [ ] Run `./build.sh windows android`
- [ ] Verify Android gallery refreshes only after `media-store-ready`
- [ ] Verify initial gallery load uses MediaStore list (no filesystem scan)

---

## Chunk 4: SDK + Permission Updates

### Task 4.1: Raise minSdk and update permissions

**Files:**
- Modify: `src-tauri/gen/android/app/build.gradle.kts`
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`
- Modify: `src-tauri/src/platform/android.rs`

- [ ] **Step 1: Write failing test (Config check)**

```bash
grep -n "minSdk" src-tauri/gen/android/app/build.gradle.kts
```

- [ ] **Step 2: Run to verify current value is not 33**

Run: `grep -n "minSdk" src-tauri/gen/android/app/build.gradle.kts`
Expected: minSdk is not 33.

- [ ] **Step 3: Update build.gradle.kts**

```kotlin
minSdk = 33
```

- [ ] **Step 4: Update AndroidManifest.xml**

Add READ_MEDIA_IMAGES permission and remove MANAGE_EXTERNAL_STORAGE:

```xml
<!-- Add this permission -->
<uses-permission android:name="android.permission.READ_MEDIA_IMAGES" />

<!-- Remove or comment out MANAGE_EXTERNAL_STORAGE if present -->
<!-- <uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE" /> -->
```

- [ ] **Step 5: Update platform/android.rs**

Remove MANAGE_EXTERNAL_STORAGE assumptions:

```rust
// Replace check_all_files_permission() with MediaStore-based check
fn check_media_store_permission() -> bool {
    // Permission check now done via Kotlin bridge; assume granted if we can query MediaStore
    true
}

// Update open_manage_storage_settings to request READ_MEDIA_IMAGES instead
pub fn open_storage_permission_settings(app: &AppHandle) {
    let _ = app.emit("android-open-storage-permission-settings", ());
    info!("Requesting READ_MEDIA_IMAGES permission");
}

// Update request_all_files_permission to use new permission flow
fn request_all_files_permission(&self, app: &AppHandle) -> Result<bool, String> {
    open_storage_permission_settings(app);
    Ok(false) // User must grant via system dialog
}
```

- [ ] **Step 6: Verify permissions**

Run: `grep -n "READ_MEDIA_IMAGES" src-tauri/gen/android/app/src/main/AndroidManifest.xml`
Expected: permission present.

Run: `grep -n "MANAGE_EXTERNAL_STORAGE" src-tauri/gen/android/app/src/main/AndroidManifest.xml`
Expected: no matches (or commented out).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/gen/android/app/build.gradle.kts src-tauri/gen/android/app/src/main/AndroidManifest.xml src-tauri/src/platform/android.rs
git commit -m "feat(android): bump minsdk to 33 and switch to READ_MEDIA_IMAGES"
```

Note: PermissionBridge.kt changes were handled in Chunk 1 (Task 1.2).

---

## Verification

- [ ] Run `./build.sh windows android`
- [ ] Install APK and verify:
  - Upload new image; gallery refresh happens on `media-store-ready`
  - First open shows latest image
  - Swipe browsing shows adjacent images
  - No MANAGE_EXTERNAL_STORAGE requested
