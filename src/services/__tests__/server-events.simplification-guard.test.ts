/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('server-events simplification guard', () => {
  // S2: ServerStartedPayload and ServerRuntimeView should be inlined
  it('server-events does not define separate ServerStartedPayload type alias', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/services/server-events.ts'), 'utf-8');

    expect(source).not.toMatch(/^type\s+ServerStartedPayload/m);
  });

  it('server-events does not define separate ServerRuntimeView type alias', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/services/server-events.ts'), 'utf-8');

    expect(source).not.toMatch(/^type\s+ServerRuntimeView/m);
  });
});
