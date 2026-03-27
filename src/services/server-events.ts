/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { Event } from '@tauri-apps/api/event';
import type { ServerInfo, ServerStateSnapshot } from '../types';
import { storageSettingsBridge } from '../types/global';

import { createEventManager, type EventRegistration } from '../utils/events';
// Note: scheduleMediaLibraryRefresh removed - full refresh no longer needed
// FTP uploads and deletions are handled incrementally to preserve scroll position

import { useServerStore } from '../stores/serverStore';
import { syncAndroidServerState } from './android-server-state-sync';

type ServerStartedPayload = { ip: string; port: number };

const defaultStats: ServerStateSnapshot = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

function createEventRegistrations(): EventRegistration<any>[] {
  return [
    {
      name: 'server-started',
      handler: (event: Event<ServerStartedPayload>) => {
        const { ip, port } = event.payload;
        useServerStore.setState((state) => ({
          ...state,
          isRunning: true,
          serverInfo: {
            isRunning: true,
            ip,
            port,
            url: `ftp://${ip}:${port}`,
            username: 'anonymous',
            passwordInfo: '(任意密码)',
          },
          stats: { ...state.stats, isRunning: true },
        }));
        syncAndroidServerState(true, useServerStore.getState().stats, 0);
      },
    },
    {
      name: 'server-stopped',
      handler: () => {
        useServerStore.setState((state) => ({
          ...state,
          isRunning: false,
          serverInfo: null,
          stats: defaultStats,
        }));
        syncAndroidServerState(false, null, 0);
      },
    },
    {
      name: 'stats-update',
      handler: (event: Event<ServerStateSnapshot>) => {
        const stats = event.payload;
        useServerStore.setState((state) => ({ ...state, stats }));
        syncAndroidServerState(true, stats, stats.connectedClients || 0);

        // Note: FTP upload refresh is handled incrementally via gallery-items-added event
        // to preserve scroll position. Full refresh is no longer needed here.
      },
    },
    // Note: media-store-ready and media-library-refresh-requested events are now handled
    // incrementally via gallery-items-added and gallery-items-deleted events to preserve
    // scroll position. Full refresh is no longer needed for these events.
    {
      name: 'tray-start-server',
      handler: async () => {
        try {
          await useServerStore.getState().startServer();
        } catch (err) {
          console.warn('[server-events] Tray start server failed:', err);
        }
      },
    },
    {
      name: 'tray-stop-server',
      handler: async () => {
        try {
          await useServerStore.getState().stopServer();
        } catch (err) {
          console.warn('[server-events] Tray stop server failed:', err);
        }
      },
    },
    {
      name: 'android-open-manage-storage-settings',
      handler: () => {
        storageSettingsBridge.openAllFilesAccessSettings();
      },
    },
  ];
}

async function syncInitialServerState(): Promise<void> {
  try {
    const info = await invoke<ServerInfo | null>('get_server_info');
    if (info?.isRunning) {
      const status = await invoke<ServerStateSnapshot | null>('get_server_status');
      const syncedStats = status || { ...defaultStats, isRunning: true };
      useServerStore.setState((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: syncedStats,
      }));
      syncAndroidServerState(true, syncedStats, syncedStats.connectedClients || 0, true);
    }
  } catch (err) {
    console.warn('[server-events] Initial state sync failed:', err);
  }
}

export async function initializeServerEvents(): Promise<() => void> {
  const eventManager = createEventManager();
  await eventManager.registerAll(createEventRegistrations());
  await syncInitialServerState();
  return () => {
    eventManager.cleanup();
  };
}
