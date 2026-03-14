/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import org.junit.Test
import org.junit.Assert.*

class PermissionBridgeTest {

    @Test
    fun does_not_request_manage_external_storage() {
        val perms = PermissionBridge.get_required_permissions()
        assertFalse(perms.contains("android.permission.MANAGE_EXTERNAL_STORAGE"))
    }

    @Test
    fun requests_read_media_images() {
        val perms = PermissionBridge.get_required_permissions()
        assertTrue(perms.contains("android.permission.READ_MEDIA_IMAGES"))
    }
}
