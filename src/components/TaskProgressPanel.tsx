/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// TODO: Extract Chinese UI strings for i18n when locale support is added

import { useEffect, useRef } from 'react';
import { useAiEditProgress, dismissDone as dismissAiEditDone, cancelAiEdit } from '../hooks/useAiEditProgress';
import {
  useColorGradingProgress,
  dismissColorGradingDone,
  cancelColorGrading,
} from '../hooks/useColorGradingProgress';

interface TaskProgressPanelProps {
  position: 'absolute' | 'fixed';
}

const AUTO_DISMISS_DELAY_MS = 3000;

function TaskRow({
  label,
  labelColor,
  state,
  onCancel,
  ariaLabel,
}: {
  label: string;
  labelColor: string;
  state: { isDone: boolean; total: number; current: number; failedCount: number };
  onCancel: () => void;
  ariaLabel: string;
}) {
  return (
    <div className="flex items-center px-3 py-1.5 gap-1">
      <span className={`${labelColor} text-xs shrink-0`}>{label}：</span>
      <span className="text-white/70 text-xs tabular-nums">
        {state.isDone ? state.total : state.current} / {state.total}
      </span>
      {state.failedCount > 0 && (
        <span className="text-red-400 text-xs tabular-nums">
          (失败 {state.failedCount})
        </span>
      )}
      {!state.isDone && (
        <button
          onClick={onCancel}
          className="ml-auto text-white/40 hover:text-white text-xs p-0.5 shrink-0"
          aria-label={ariaLabel}
        >
          ×
        </button>
      )}
    </div>
  );
}

export function TaskProgressPanel({ position }: TaskProgressPanelProps) {
  const aiEdit = useAiEditProgress();
  const colorGrading = useColorGradingProgress();
  const dismissTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const aiEditVisible = aiEdit.isEditing || aiEdit.isDone;
  const cgVisible = colorGrading.isProcessing || colorGrading.isDone;
  const aiEditVisibleRef = useRef(aiEditVisible);
  const cgVisibleRef = useRef(cgVisible);
  const aiEditDoneRef = useRef(aiEdit.isDone);
  const colorGradingDoneRef = useRef(colorGrading.isDone);
  aiEditVisibleRef.current = aiEditVisible;
  cgVisibleRef.current = cgVisible;
  aiEditDoneRef.current = aiEdit.isDone;
  colorGradingDoneRef.current = colorGrading.isDone;
  const hasAnyTask = aiEditVisible || cgVisible;

  const allDone = (aiEditVisible ? aiEdit.isDone : true)
    && (cgVisible ? colorGrading.isDone : true)
    && hasAnyTask;

  useEffect(() => {
    if (dismissTimerRef.current) {
      clearTimeout(dismissTimerRef.current);
      dismissTimerRef.current = null;
    }

    if (allDone) {
      dismissTimerRef.current = setTimeout(() => {
        if (aiEditVisibleRef.current && aiEditDoneRef.current) dismissAiEditDone();
        if (cgVisibleRef.current && colorGradingDoneRef.current) dismissColorGradingDone();
      }, AUTO_DISMISS_DELAY_MS);
    }

    return () => {
      if (dismissTimerRef.current) {
        clearTimeout(dismissTimerRef.current);
        dismissTimerRef.current = null;
      }
    };
  }, [allDone, aiEdit.isDone, colorGrading.isDone]);

  if (!hasAnyTask) return null;

  const containerClass = position === 'fixed' ? 'fixed z-50' : 'absolute z-10';
  const containerStyle: React.CSSProperties = position === 'fixed'
    ? { left: '12px', bottom: '5rem' }
    : { left: '12px', bottom: '76px' };

  const handleCancelAiEdit = () => { cancelAiEdit().catch(() => {}); };
  const handleCancelColorGrading = () => { cancelColorGrading().catch(() => {}); };
  const handleCancelAll = () => {
    if (aiEditVisible && !aiEdit.isDone) cancelAiEdit().catch(() => {});
    if (cgVisible && !colorGrading.isDone) cancelColorGrading().catch(() => {});
  };

  return (
    <div className={`${containerClass} animate-slide-up`} style={containerStyle}>
      <div className="bg-gray-950/80 backdrop-blur-md border border-white/10 rounded-lg overflow-hidden min-w-[180px]">
        {/* Header */}
        <div className="px-3 py-1 text-center text-[11px] text-white/50 border-b border-white/10">
          后台任务
        </div>

        {/* AI Edit row */}
        {aiEditVisible && (
          <TaskRow
            label="AI修图"
            labelColor="text-blue-400"
            state={aiEdit}
            onCancel={handleCancelAiEdit}
            ariaLabel="取消AI修图"
          />
        )}

        {/* Color Grading row */}
        {cgVisible && (
          <TaskRow
            label="调色"
            labelColor="text-violet-400"
            state={colorGrading}
            onCancel={handleCancelColorGrading}
            ariaLabel="取消调色"
          />
        )}

        {/* Footer */}
        <div className="border-t border-white/10 px-3 py-1 text-center">
          {allDone ? (
            <span className="text-green-400 text-[11px]">已完成</span>
          ) : (
            <button
              onClick={handleCancelAll}
              className="text-white/40 hover:text-white text-[11px]"
            >
              全部取消
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
