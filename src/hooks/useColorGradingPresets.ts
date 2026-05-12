/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingPreset } from '../types';

export function useColorGradingPresets() {
  const [presets, setPresets] = useState<ColorGradingPreset[]>([]);

  useEffect(() => {
    invoke<ColorGradingPreset[]>('get_color_grading_presets')
      .then(setPresets)
      .catch(() => {});
  }, []);

  return presets;
}
