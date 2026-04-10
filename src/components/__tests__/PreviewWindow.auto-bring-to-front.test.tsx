/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PreviewWindow } from '../PreviewWindow';

const state = {
  isOpen: true,
  currentImage: '/tmp/example.jpg',
  autoBringToFront: false,
};

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => path,
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    isFullscreen: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
    setFullscreen: vi.fn().mockResolvedValue(undefined),
    setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: (selector: (state: { updatePreviewConfig: typeof updatePreviewConfigMock }) => unknown) =>
    selector({ updatePreviewConfig: updatePreviewConfigMock }),
}));

vi.mock('../../hooks/usePreviewWindowLifecycle', () => ({
  usePreviewWindowLifecycle: () => state,
}));

vi.mock('../../hooks/usePreviewNavigation', () => ({
  usePreviewNavigation: () => ({
    currentIndex: 1,
    totalFiles: 1,
    goToPrevious: vi.fn(),
    goToNext: vi.fn(),
    goToOldest: vi.fn(),
    goToLatest: vi.fn(),
  }),
}));

vi.mock('../../hooks/usePreviewExif', () => ({
  usePreviewExif: () => null,
}));

vi.mock('../../hooks/usePreviewZoomPan', () => ({
  usePreviewZoomPan: () => ({
    scale: 1,
    panX: 0,
    panY: 0,
    isDragging: false,
    containerRef: { current: null },
    resetZoom: vi.fn(),
    handleWheel: vi.fn(),
    handleMouseDown: vi.fn(),
    handleMouseMove: vi.fn(),
    stopDragging: vi.fn(),
  }),
}));

vi.mock('../../hooks/usePreviewToolbarAutoHide', () => ({
  usePreviewToolbarAutoHide: () => ({
    showToolbar: true,
    showToolbarOnPointerMove: vi.fn(),
    handleToolbarMouseEnter: vi.fn(),
    handleToolbarMouseLeave: vi.fn(),
  }),
}));

vi.mock('../../hooks/usePreviewConfigListener', () => ({
  usePreviewConfigListener: vi.fn(),
}));

const { updatePreviewConfigMock } = vi.hoisted(() => ({
  updatePreviewConfigMock: vi.fn().mockResolvedValue({ autoBringToFront: false }),
}));

describe('PreviewWindow autoBringToFront sync', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    updatePreviewConfigMock.mockClear();
    state.isOpen = true;
    state.currentImage = '/tmp/example.jpg';
    state.autoBringToFront = false;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('updates the local toggle title when lifecycle autoBringToFront changes', async () => {
    await act(async () => {
      root.render(<PreviewWindow />);
      await Promise.resolve();
    });

    expect(container.querySelector('button[title="新图片时自动前台显示 (已关闭)"]')).toBeTruthy();

    state.autoBringToFront = true;

    await act(async () => {
      root.render(<PreviewWindow />);
      await Promise.resolve();
    });

    expect(container.querySelector('button[title="新图片时自动前台显示 (已开启)"]')).toBeTruthy();
  });
});
