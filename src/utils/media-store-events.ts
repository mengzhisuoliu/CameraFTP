/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * Determines if the gallery should refresh based on the event type
 * @param event - The event name
 * @returns true if the gallery should refresh on this event
 */
export function shouldRefreshOnEvent(event: string): boolean {
  return event === 'media-store-ready';
}
