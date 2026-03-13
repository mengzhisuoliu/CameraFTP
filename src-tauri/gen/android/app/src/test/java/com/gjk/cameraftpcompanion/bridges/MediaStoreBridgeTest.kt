/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

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
}
