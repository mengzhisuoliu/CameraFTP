/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ExifInfo } from '../types';

export function usePreviewExif(imagePath: string | null): ExifInfo | null {
  const [exifInfo, setExifInfo] = useState<ExifInfo | null>(null);

  useEffect(() => {
    if (!imagePath) {
      setExifInfo(null);
      return;
    }

    setExifInfo(null);

    let cancelled = false;

    const loadExifInfo = async () => {
      try {
        const exif = await invoke<ExifInfo | null>('get_image_exif', { filePath: imagePath });
        if (!cancelled) {
          setExifInfo(exif);
        }
      } catch {
        if (!cancelled) {
          setExifInfo(null);
        }
      }
    };

    void loadExifInfo();

    return () => {
      cancelled = true;
    };
  }, [imagePath]);

  return exifInfo;
}
