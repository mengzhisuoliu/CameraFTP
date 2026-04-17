/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect, useRef } from 'react';
import { Dialog } from './ui/Dialog';
import { ToggleSwitch } from './ui/ToggleSwitch';
import { Select } from './ui/Select';
import { SEEDREAM_MODELS, DEFAULT_SEEDREAM_MODEL } from '../constants/seedream-models';

interface PromptDialogProps {
  isOpen: boolean;
  defaultPrompt: string;
  defaultModel?: string;
  onConfirm: (prompt: string, shouldSave: boolean, model: string) => void;
  onCancel: () => void;
}

export function PromptDialog({ isOpen, defaultPrompt, defaultModel, onConfirm, onCancel }: PromptDialogProps) {
  const [prompt, setPrompt] = useState(defaultPrompt);
  const [model, setModel] = useState(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
  const [savePrompt, setSavePrompt] = useState(true);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isOpen) {
      setPrompt(defaultPrompt);
      setModel(defaultModel ?? DEFAULT_SEEDREAM_MODEL);
      setSavePrompt(true);
      // Focus textarea after dialog opens
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [isOpen, defaultPrompt, defaultModel]);

  const handleConfirm = () => {
    const trimmed = prompt.trim();
    onConfirm(trimmed, savePrompt, model);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="AI修图提示词"
      subtitle="编辑提示词后确认触发修图"
      maxWidth="max-w-md"
      footer={
        <div className="flex items-center justify-between w-full">
          <div className="flex items-center gap-2 cursor-pointer select-none" onClick={() => setSavePrompt(!savePrompt)}>
            <ToggleSwitch enabled={savePrompt} onChange={setSavePrompt} />
            <span className="text-sm font-medium text-gray-700">保存提示词</span>
          </div>
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors text-sm"
            >
              取消
            </button>
            <button
              onClick={handleConfirm}
              className="px-4 py-2 text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors text-sm"
            >
              确认修图
            </button>
          </div>
        </div>
      }
    >
      <div className="space-y-3">
        <div className="space-y-1">
          <label className="block text-xs font-medium text-gray-500">模型</label>
          <Select
            value={model}
            options={SEEDREAM_MODELS}
            onChange={setModel}
          />
        </div>
        <div className="space-y-1">
          <label className="block text-xs font-medium text-gray-500">提示词</label>
          <textarea
            ref={textareaRef}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="例如：提升画质，使照片更清晰"
            rows={4}
            className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>
      </div>
    </Dialog>
  );
}
