/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { shouldRefreshOnEvent } from '../media-store-events';

it('refreshes only on media-store-ready', () => {
  expect(shouldRefreshOnEvent('file-uploaded')).toBe(false);
  expect(shouldRefreshOnEvent('media-store-ready')).toBe(true);
});
