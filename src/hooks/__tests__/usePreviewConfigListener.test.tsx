/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { usePreviewConfigListener } from '../usePreviewConfigListener';
import type { ConfigChangedEvent } from '../../types/events';

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('usePreviewConfigListener', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('subscribes to preview-config-changed and forwards config payload to callback', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    const callback = vi.fn();

    renderHook(() => usePreviewConfigListener(callback));

    expect(listenMock).toHaveBeenCalledWith(
      'preview-config-changed',
      expect.any(Function),
    );

    const listener = listenMock.mock.calls[0]?.[1] as (event: { payload: ConfigChangedEvent }) => void;

    act(() => {
      listener({
        payload: {
          config: {
            enabled: true,
            method: 'built-in-preview',
            customPath: null,
            autoBringToFront: true,
          },
        },
      });
    });

    expect(callback).toHaveBeenCalledWith({
      enabled: true,
      method: 'built-in-preview',
      customPath: null,
      autoBringToFront: true,
    });
  });

  it('cleans up the Tauri event listener on unmount', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValue(unlisten);

    const { unmount } = renderHook(() => usePreviewConfigListener(vi.fn()));

    unmount();
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
