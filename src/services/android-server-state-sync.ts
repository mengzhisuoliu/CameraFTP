/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { ServerStateSnapshot } from '../types';

export function syncAndroidServerState(
  _isRunning: boolean,
  _stats: ServerStateSnapshot | null,
  _connectedClients: number,
  _immediate = false,
): void {
  // Android foreground service state is owned by the native path now.
  // Keep this exported function as a no-op compatibility shim until the
  // obsolete WebView bridge is removed in a follow-up task.
}
