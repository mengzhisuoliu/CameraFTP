/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { AiEditProgressEvent } from '../types';

interface AiEditProgressState {
  isEditing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialState: AiEditProgressState = {
  isEditing: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const useAiEditProgressStore = create<AiEditProgressState>(() => ({ ...initialState }));

let listenerCleanup: (() => void) | null = null;
let listenerRefCount = 0;

function syncToNativeLayer(current: number, total: number, failedCount: number) {
  window.ImageViewerAndroid?.updateAiEditProgress?.(current, total, failedCount);
}

function notifyNativeDone(success: boolean, failedCount: number, failedFiles: string[]) {
  const message = success
    ? null
    : `修图完成，${failedCount}张失败：${failedFiles.join('、')}`;
  window.ImageViewerAndroid?.onAiEditComplete?.(success, message);
}

function handleEvent(event: AiEditProgressEvent) {
  switch (event.type) {
    case 'progress':
      useAiEditProgressStore.setState({
        isEditing: true,
        isDone: false,
        current: event.current,
        total: event.total,
        currentFileName: event.fileName,
        failedCount: event.failedCount,
      });
      syncToNativeLayer(event.current, event.total, event.failedCount);
      break;
    case 'completed':
      useAiEditProgressStore.setState({
        failedCount: event.failedCount,
      });
      break;
    case 'failed':
      useAiEditProgressStore.setState({
        failedCount: event.failedCount,
      });
      break;
    case 'done': {
      const hasFailures = event.failedCount > 0;
      useAiEditProgressStore.setState({
        isEditing: false,
        isDone: hasFailures,
        current: event.total,
        failedCount: event.failedCount,
        failedFiles: event.failedFiles,
      });
      if (!hasFailures) {
        setTimeout(() => {
          useAiEditProgressStore.setState({ ...initialState });
        }, 500);
      }
      notifyNativeDone(event.failedCount === 0, event.failedCount, event.failedFiles);
      break;
    }
  }
}

async function ensureListener() {
  if (listenerCleanup) return;
  const unlisten = await listen<AiEditProgressEvent>('ai-edit-progress', (e) => {
    handleEvent(e.payload);
  });
  listenerCleanup = unlisten;
}

function cleanupListener() {
  if (listenerCleanup) {
    listenerCleanup();
    listenerCleanup = null;
  }
}

export function useAiEditProgress(): AiEditProgressState {
  return useAiEditProgressStore();
}

export async function enqueueAiEdit(files: string[], prompt: string, _shouldSave: boolean): Promise<void> {
  await invoke('enqueue_ai_edit', {
    filePaths: files,
    prompt: prompt || null,
  });
}

export function dismissDone() {
  useAiEditProgressStore.setState({ ...initialState });
}

export function useAiEditProgressListener() {
  useEffect(() => {
    listenerRefCount++;
    ensureListener();

    return () => {
      listenerRefCount--;
      if (listenerRefCount <= 0) {
        listenerRefCount = 0;
        cleanupListener();
      }
    };
  }, []);
}
