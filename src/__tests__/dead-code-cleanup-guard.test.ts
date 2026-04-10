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

  // D1: gallery-v2 internal-only thumbnail types should not be exported
  it('gallery-v2 does not export ThumbSizeBucket, ThumbPriority, ThumbStatus, ThumbErrorCode', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/types/gallery-v2.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+type\s+ThumbSizeBucket/);
    expect(source).not.toMatch(/export\s+type\s+ThumbPriority/);
    expect(source).not.toMatch(/export\s+type\s+ThumbStatus/);
    expect(source).not.toMatch(/export\s+type\s+ThumbErrorCode/);
  });

  // D2: handleDelete should not contain duplicate setShowMenu(false)
  it('useGallerySelection handleDelete does not call setShowMenu(false) twice', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/hooks/useGallerySelection.ts'), 'utf-8');

    // Extract handleDelete callback body
    const handleDeleteMatch = source.match(/const handleDelete[\s\S]*?\}, \[.*?\]\);/);
    expect(handleDeleteMatch).toBeTruthy();
    const body = handleDeleteMatch![0];
    const count = (body.match(/setShowMenu\(false\)/g) || []).length;
    expect(count).toBe(1);
  });

  // D4: UseGalleryPagerResult should not be exported
  it('useGalleryPager does not export UseGalleryPagerResult', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/hooks/useGalleryPager.ts'), 'utf-8');

    expect(source).not.toMatch(/export\s+interface\s+UseGalleryPagerResult/);
  });

  // S3: PreviewWindow should use single import from @tauri-apps/api/core
  it('PreviewWindow does not have duplicate imports from @tauri-apps/api/core', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/components/PreviewWindow.tsx'), 'utf-8');

    const importCount = (source.match(/from\s+['"]@tauri-apps\/api\/core['"]/g) || []).length;
    expect(importCount).toBe(1);
  });

  // S10: configStore mergeDraftWithBackend uses preserveIfDirty helper
  it('configStore mergeDraftWithBackend uses preserveIfDirty helper to avoid repetition', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/configStore.ts'), 'utf-8');

    expect(source).toContain('preserveIfDirty');
  });
});
