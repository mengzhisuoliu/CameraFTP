/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { ToggleSwitch } from './ui/ToggleSwitch';
import { Select } from './ui/Select';
import { METERING_MODES } from '../constants/color-grading';

interface ExposureConfigSectionProps {
  useAutoExposure: boolean;
  onAutoExposureChange: (v: boolean) => void;
  meteringMode: string;
  onMeteringModeChange: (v: string) => void;
  manualEv: number;
  onManualEvChange: (v: number) => void;
  disabled?: boolean;
}

export function ExposureConfigSection({
  useAutoExposure,
  onAutoExposureChange,
  meteringMode,
  onMeteringModeChange,
  manualEv,
  onManualEvChange,
  disabled = false,
}: ExposureConfigSectionProps) {
  return (
    <>
      <div className="border-t border-gray-100 pt-3">
        <ToggleSwitch
          enabled={useAutoExposure}
          onChange={onAutoExposureChange}
          label="自动曝光"
          description={useAutoExposure ? '自动检测并调整曝光' : '手动设置曝光补偿值'}
          disabled={disabled}
        />
      </div>

      {useAutoExposure && (
        <div className="space-y-2">
          <label className="block text-sm font-medium text-gray-700">测光模式</label>
          <Select
            value={meteringMode}
            options={METERING_MODES}
            onChange={onMeteringModeChange}
            disabled={disabled}
          />
        </div>
      )}

      {!useAutoExposure && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="block text-sm font-medium text-gray-700">曝光补偿</label>
            <span className="text-sm font-mono text-gray-500">
              {manualEv > 0 ? '+' : ''}{manualEv.toFixed(1)} EV
            </span>
          </div>
          <input
            type="range"
            min={-5.0}
            max={5.0}
            step={0.1}
            value={manualEv}
            onChange={(e) => onManualEvChange(parseFloat(e.target.value))}
            disabled={disabled}
            className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600 disabled:opacity-50"
          />
          <div className="flex justify-between text-xs text-gray-400">
            <span>-5.0</span>
            <span>0</span>
            <span>+5.0</span>
          </div>
        </div>
      )}
    </>
  );
}
