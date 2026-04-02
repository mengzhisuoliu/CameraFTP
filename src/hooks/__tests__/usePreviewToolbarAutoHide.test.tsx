/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePreviewToolbarAutoHide } from '../usePreviewToolbarAutoHide';

function Harness() {
  const toolbar = usePreviewToolbarAutoHide();

  return (
    <div>
      <span data-testid="visible">{toolbar.showToolbar ? 'yes' : 'no'}</span>
      <button data-testid="move" onClick={toolbar.showToolbarOnPointerMove}>move</button>
      <button data-testid="enter" onMouseEnter={toolbar.handleToolbarMouseEnter} onClick={toolbar.handleToolbarMouseEnter}>enter</button>
      <button data-testid="leave" onMouseLeave={toolbar.handleToolbarMouseLeave} onClick={toolbar.handleToolbarMouseLeave}>leave</button>
    </div>
  );
}

describe('usePreviewToolbarAutoHide', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
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
    vi.useRealTimers();
  });

  it('hides toolbar after idle timeout and keeps visible while hovered', async () => {
    await act(async () => {
      root.render(<Harness />);
    });

    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('yes');

    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('no');

    await act(async () => {
      container.querySelector('[data-testid="move"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      container.querySelector('[data-testid="enter"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(3000);
    });

    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('yes');
  });

  it('restarts idle timer on repeated pointer movement while visible', async () => {
    await act(async () => {
      root.render(<Harness />);
    });

    await act(async () => {
      vi.advanceTimersByTime(2500);
      container.querySelector('[data-testid="move"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(700);
    });

    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('yes');

    await act(async () => {
      vi.advanceTimersByTime(2300);
    });

    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('no');
  });
});
