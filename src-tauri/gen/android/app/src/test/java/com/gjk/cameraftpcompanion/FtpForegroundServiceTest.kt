/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class FtpForegroundServiceTest {
    @Test
    fun start_update_and_stop_flow_uses_direct_native_payload_and_real_stop_path() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":2,\"bytesReceived\":1024,\"lastFile\":null}",
            1,
        )

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        val service = serviceController.get()
        service.onStartCommand(
            Intent(context, FtpForegroundService::class.java).apply {
                action = FtpForegroundService.ACTION_START
            },
            0,
            1,
        )

        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":4,\"bytesReceived\":2048,\"lastFile\":null}",
            3,
        )

        var notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
        assertNotNull(notification)
        assertTrue(notification.extras.getCharSequence("android.text")!!.contains("2.0 KB"))
        assertEquals(3, readConnectedClients(service))
        assertEquals(
            "{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":4,\"bytesReceived\":2048,\"lastFile\":null}",
            readServiceStatsJson(service),
        )

        AndroidServiceStateCoordinator.syncNativeServiceState(context, false, null, 0)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val stoppedIntent = shadowOf(context as android.app.Application).nextStoppedService
        notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
        assertTrue(!snapshot.isRunning)
        assertEquals(0, snapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, stoppedIntent.component?.className)
        assertEquals(null, stoppedIntent.action)
        assertNotNull(notification)
    }

    private fun readConnectedClients(service: FtpForegroundService): Int {
        val field = FtpForegroundService::class.java.getDeclaredField("connectedClients")
        field.isAccessible = true
        return field.getInt(service)
    }

    private fun readServiceStatsJson(service: FtpForegroundService): String? {
        val field = FtpForegroundService::class.java.getDeclaredField("serverStats")
        field.isAccessible = true
        return field.get(service)?.toString()
    }
}
