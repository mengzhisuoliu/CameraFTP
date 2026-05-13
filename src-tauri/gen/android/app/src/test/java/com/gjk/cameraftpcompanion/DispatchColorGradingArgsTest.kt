/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.json.JSONArray

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class DispatchColorGradingArgsTest {

    @Test
    fun argsJson_nativeTypes_preservedAcrossJsonSerialization() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", true, "highlight-safe", 0.0f, false,
        )
        val arr = JSONArray(json)

        assertEquals(6, arr.length())
        // String fields
        assertEquals("/photo.nef", arr.getString(0))
        assertEquals("fujifilm-provia", arr.getString(1))
        assertEquals("highlight-safe", arr.getString(3))
        // Native boolean fields — JSON literals true/false, not strings
        assertTrue(arr.getBoolean(2))
        assertFalse(arr.getBoolean(5))
        // Native number field
        assertEquals(0.0, arr.getDouble(4), 0.001)
    }

    @Test
    fun argsJson_falseBooleanAndPositiveEv_roundTripsCorrectly() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "kodak-vision-2383", false, "matrix", 2.5f, true,
        )
        val arr = JSONArray(json)

        assertFalse(arr.getBoolean(2))
        assertTrue(arr.getBoolean(5))
        assertEquals(2.5, arr.getDouble(4), 0.001)
    }

    @Test
    fun argsJson_negativeEv_preservedAsNativeNumber() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", true, "highlight-safe", -3.7f, false,
        )
        val arr = JSONArray(json)

        assertEquals(-3.7, arr.getDouble(4), 0.01)
    }
}
