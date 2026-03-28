/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Context
import android.util.Log
import android.webkit.JavascriptInterface

/**
 * Server State JavaScript Bridge
 * Legacy compatibility shim while Android service control is native-driven.
 */
class ServerStateBridge(@Suppress("UNUSED_PARAMETER") private val context: Context) {

    companion object {
        private const val TAG = "ServerStateBridge"
    }

    /**
     * Ignore legacy WebView-driven service updates now that Rust owns native sync.
     */
    @JavascriptInterface
    fun onServerStateChanged(
        isRunning: Boolean,
        @Suppress("UNUSED_PARAMETER")
        statsJson: String?,
        connectedClients: Int,
    ) {
        Log.d(
            TAG,
            "Ignoring legacy WebView server state update: isRunning=$isRunning, connectedClients=$connectedClients",
        )
    }
}
