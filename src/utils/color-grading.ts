/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

interface AutoColorGrading {
  enabled?: boolean;
  presetId?: string;
  useAutoExposure?: boolean;
  meteringMode?: string;
  manualEv?: number;
  [key: string]: unknown;
}

interface ColorGradingLastUsed {
  presetId: string;
  useAutoExposure: boolean;
  meteringMode: string;
  manualEv: number;
  syncToAuto: boolean;
}

export interface HasColorGradingDraft {
  colorGradingLastUsed: ColorGradingLastUsed | null;
  autoColorGrading?: AutoColorGrading | null;
  [key: string]: unknown;
}

export function saveColorGradingConfig<T extends HasColorGradingDraft>(
  updateDraft: (fn: (draft: T) => T) => void,
  params: {
    presetId: string;
    useAutoExposure: boolean;
    meteringMode: string;
    manualEv: number;
    syncToAuto: boolean;
  },
) {
  updateDraft(d => ({
    ...d,
    colorGradingLastUsed: {
      presetId: params.presetId,
      useAutoExposure: params.useAutoExposure,
      meteringMode: params.meteringMode,
      manualEv: params.manualEv,
      syncToAuto: params.syncToAuto,
    },
    ...(params.syncToAuto && d.autoColorGrading ? {
      autoColorGrading: {
        ...d.autoColorGrading,
        presetId: params.presetId,
        useAutoExposure: params.useAutoExposure,
        meteringMode: params.meteringMode,
        manualEv: params.manualEv,
      },
    } : {}),
  }));
}
