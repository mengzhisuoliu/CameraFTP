/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('dead code cleanup guard', () => {
  // T1: dispatchThumbnailResult should not be exported
  it('gallery-media-v2 does not export dispatchThumbnailResult', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/services/gallery-media-v2.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+function\s+dispatchThumbnailResult/);
  });

  // T2: PortSyntaxValidationResult should not be exported
  it('usePortCheck does not export PortSyntaxValidationResult', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/hooks/usePortCheck.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+type\s+PortSyntaxValidationResult/);
  });

  // T3: ThumbnailSchedulerMedia and UseThumbnailSchedulerOptions should not be exported
  it('useThumbnailScheduler does not export internal-only types', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/hooks/useThumbnailScheduler.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+type\s+ThumbnailSchedulerMedia/);
    expect(source).not.toMatch(/export\s+type\s+UseThumbnailSchedulerOptions/);
  });

  // T4: MediaLibraryRefreshReason and MediaLibraryRefreshDetail should not be exported
  it('gallery-refresh does not export internal-only types', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/gallery-refresh.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+type\s+MediaLibraryRefreshReason/);
    expect(source).not.toMatch(/export\s+interface\s+MediaLibraryRefreshDetail/);
  });

  // S5: formatError uses simplified null check
  it('error.formatError uses == null instead of === null || === undefined', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/error.ts'), 'utf-8');

    expect(source).not.toContain('=== null || err === undefined');
  });

  // S6: useGallerySelection uses window globals directly
  it('useGallerySelection does not use verbose Window type cast', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/hooks/useGallerySelection.ts'), 'utf-8');

    expect(source).not.toContain('as Window &');
  });
});
