/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { syncAndroidServerState } from '../android-server-state-sync';

describe('android server state sync', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('acts as a compatibility no-op for stale and fresh payloads', () => {
    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 2,
      filesReceived: 7,
      bytesReceived: 1024,
      lastFile: '/older.jpg',
    }, 2);

    syncAndroidServerState(false, null, 0);

    expect(vi.getTimerCount()).toBe(0);
  });

  it('preserves null stopped-state payloads without scheduling bridge work', () => {
    syncAndroidServerState(false, null, 0);

    expect(vi.getTimerCount()).toBe(0);
  });

  it('does not retry when asked for immediate compatibility sync', () => {
    syncAndroidServerState(false, null, 0, true);

    vi.advanceTimersByTime(3000);

    expect(vi.getTimerCount()).toBe(0);
  });

  it('ignores repeated sync requests without retaining timeout state', () => {
    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 1,
      filesReceived: 3,
      bytesReceived: 512,
      lastFile: '/older.jpg',
    }, 1);

    syncAndroidServerState(false, null, 0);

    vi.advanceTimersByTime(3000);

    expect(vi.getTimerCount()).toBe(0);
  });

  it('does not schedule any retry work after a compatibility sync call', () => {
    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 3,
      filesReceived: 8,
      bytesReceived: 2048,
      lastFile: '/latest.jpg',
    }, 3);

    vi.advanceTimersByTime(3000);

    expect(vi.getTimerCount()).toBe(0);
  });
});
