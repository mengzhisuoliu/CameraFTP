/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Assert.*
import org.junit.Test

class ExifPrefetchEscapeTest {

    @Test
    fun escapeForJsString_noSpecialChars_returnsUnchanged() {
        assertEquals("hello world", ImageViewerActivity.escapeForJsString("hello world"))
    }

    @Test
    fun escapeForJsString_escapesBackslashes() {
        assertEquals("C:\\\\Photos", ImageViewerActivity.escapeForJsString("C:\\Photos"))
    }

    @Test
    fun escapeForJsString_escapesSingleQuotes() {
        assertEquals("it\\'s", ImageViewerActivity.escapeForJsString("it's"))
    }

    @Test
    fun escapeForJsString_escapesNewlines() {
        assertEquals("line1\\nline2", ImageViewerActivity.escapeForJsString("line1\nline2"))
    }

    @Test
    fun escapeForJsString_escapesCarriageReturns() {
        assertEquals("line1\\rline2", ImageViewerActivity.escapeForJsString("line1\rline2"))
    }

    @Test
    fun escapeForJsString_mixedSpecialChars() {
        val input = "path\\to\\'file\\'\nwith\nnewlines\rand\rcarriage"
        val expected = "path\\\\to\\\\\\'file\\\\\\'\\nwith\\nnewlines\\rand\\rcarriage"
        assertEquals(expected, ImageViewerActivity.escapeForJsString(input))
    }
}
