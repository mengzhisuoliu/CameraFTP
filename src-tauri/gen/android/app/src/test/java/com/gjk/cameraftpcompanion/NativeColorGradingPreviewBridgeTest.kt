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
class NativeColorGradingPreviewBridgeTest {

    @Test
    fun buildArgsJson_nativeTypes_preservedAcrossJsonSerialization() {
        val json = NativeColorGradingPreviewBridge.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", "matrix", 0.0f, false,
        )
        val arr = JSONArray(json)

        assertEquals(5, arr.length())
        assertEquals("/photo.nef", arr.getString(0))
        assertEquals("fujifilm-provia", arr.getString(1))
        assertEquals("matrix", arr.getString(2))
        assertEquals(0.0, arr.getDouble(3), 0.001)
        assertFalse(arr.getBoolean(4))
    }

    @Test
    fun buildArgsJson_positiveEvAndSyncToAuto_roundTripsCorrectly() {
        val json = NativeColorGradingPreviewBridge.buildColorGradingArgsJson(
            "/photo.nef", "kodak-vision-2383", "matrix", 2.5f, true,
        )
        val arr = JSONArray(json)

        assertEquals("kodak-vision-2383", arr.getString(1))
        assertEquals("matrix", arr.getString(2))
        assertEquals(2.5, arr.getDouble(3), 0.001)
        assertTrue(arr.getBoolean(4))
    }

    @Test
    fun buildArgsJson_negativeEv_preservedAsNativeNumber() {
        val json = NativeColorGradingPreviewBridge.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", "matrix", -3.7f, false,
        )
        val arr = JSONArray(json)

        assertEquals(-3.7, arr.getDouble(3), 0.01)
    }

    @Test
    fun buildArgsJson_specialCharactersInStrings_properlyEscaped() {
        val json = NativeColorGradingPreviewBridge.buildColorGradingArgsJson(
            """C:\Photos\test "image".jpg""",
            """preset "name"""",
            """mode\nvalue""",
            0.0f, false,
        )
        val arr = JSONArray(json)

        // JSONObject.quote ensures proper JSON escaping — round-trip preserves content
        assertEquals("""C:\Photos\test "image".jpg""", arr.getString(0))
        assertEquals("""preset "name"""", arr.getString(1))
        assertEquals("""mode\nvalue""", arr.getString(2))
    }
}
