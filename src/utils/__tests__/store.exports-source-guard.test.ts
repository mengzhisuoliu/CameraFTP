/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('store.ts exports (source guard)', () => {
  it('does not export retryAction', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/store.ts'), 'utf-8');
    expect(source).not.toContain('export function retryAction');
  });
});
