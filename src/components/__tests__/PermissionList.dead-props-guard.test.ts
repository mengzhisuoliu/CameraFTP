/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('PermissionList dead props guard', () => {
  // D3: showStorage, showNotification, showBattery props are never passed
  it('PermissionList does not accept showStorage, showNotification, showBattery props', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/components/PermissionList.tsx'), 'utf-8');

    expect(source).not.toContain('showStorage');
    expect(source).not.toContain('showNotification');
    expect(source).not.toContain('showBattery');
  });
});
