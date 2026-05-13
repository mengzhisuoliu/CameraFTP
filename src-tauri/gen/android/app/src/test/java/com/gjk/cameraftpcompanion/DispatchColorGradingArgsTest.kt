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

    // --- buildColorGradingArgsJson serializes booleans and floats as strings ---

    @Test
    fun argsJson_booleansSerializedAsStrings() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", true, "highlight-safe", 0.0f, false,
        )
        val arr = JSONArray(json)

        assertEquals(6, arr.length())
        assertEquals("/photo.nef", arr.getString(0))
        assertEquals("fujifilm-provia", arr.getString(1))
        assertEquals("true", arr.getString(2))
        assertEquals("highlight-safe", arr.getString(3))
        assertEquals("0.0", arr.getString(4))
        assertEquals("false", arr.getString(5))
    }

    @Test
    fun argsJson_falseBoolean_notStringTrue() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "kodak-vision-2383", false, "matrix", 1.5f, true,
        )
        val arr = JSONArray(json)

        assertEquals("false", arr.getString(2))
        assertEquals("true", arr.getString(5))
    }

    @Test
    fun argsJson_floatSerializedAsString() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", false, "spot", -2.5f, false,
        )
        val arr = JSONArray(json)

        assertEquals("-2.5", arr.getString(4))
    }

    @Test
    fun argsJson_negativeEv_preservedAsString() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", true, "highlight-safe", -3.7f, false,
        )
        val arr = JSONArray(json)

        val evStr = arr.getString(4)
        assertEquals(-3.7f, evStr.toFloat(), 0.01f)
    }

    @Test
    fun argsJson_producesValidJsonArray() {
        val json = ImageViewerActivity.buildColorGradingArgsJson(
            "/photo.nef", "fujifilm-provia", true, "highlight-safe", 0.0f, false,
        )
        // Should parse without error
        val arr = JSONArray(json)
        assertEquals(6, arr.length())
    }
}
