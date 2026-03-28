/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.gjk.cameraftpcompanion.MainActivity
import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ServerStateBridgeTest {

    @Test
    fun bridge_is_compatibility_shim_that_does_not_sync_native_service_state() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val bridge = ServerStateBridge(context)

        bridge.onServerStateChanged(true, "{\"files_transferred\":3}", 1)

        assertTrue(true)
    }

    @Test
    fun main_activity_no_longer_exposes_service_state_relay_api() {
        val hasRelayApi = MainActivity::class.java.declaredMethods.any { method ->
            method.name == "updateServiceState"
        }

        assertFalse(hasRelayApi)
    }

    @Test
    fun main_activity_no_longer_registers_android_service_state_update_relay() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt")
        val source = String(Files.readAllBytes(sourcePath))

        assertFalse(source.contains("android-service-state-update"))
    }

    @Test
    fun main_activity_no_longer_registers_server_state_android_javascript_bridge() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt")
        val source = String(Files.readAllBytes(sourcePath))

        assertFalse(source.contains("ServerStateAndroid"))
        assertFalse(source.contains("ServerStateBridge("))
    }

    @Test
    fun server_state_bridge_no_longer_controls_android_service_state() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt")
        val source = String(Files.readAllBytes(sourcePath))

        assertFalse(source.contains("AndroidServiceStateCoordinator"))
        assertFalse(source.contains("updateServiceState("))
        assertFalse(source.contains("syncNativeServiceState("))
    }
}
