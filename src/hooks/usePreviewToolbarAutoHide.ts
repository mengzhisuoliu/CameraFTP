/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useRef, useState } from 'react';

export function usePreviewToolbarAutoHide() {
  const [showToolbar, setShowToolbar] = useState(true);
  const [isToolbarHovered, setIsToolbarHovered] = useState(false);
  const toolbarTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isToolbarHoveredRef = useRef(false);

  useEffect(() => {
    isToolbarHoveredRef.current = isToolbarHovered;
  }, [isToolbarHovered]);

  const restartHideTimer = useCallback(() => {
    if (toolbarTimeoutRef.current) {
      clearTimeout(toolbarTimeoutRef.current);
    }

    toolbarTimeoutRef.current = setTimeout(() => {
      if (!isToolbarHoveredRef.current) {
        setShowToolbar(false);
      }
    }, 3000);
  }, []);

  useEffect(() => {
    if (!showToolbar || isToolbarHovered) {
      return;
    }

    restartHideTimer();

    return () => {
      if (toolbarTimeoutRef.current) {
        clearTimeout(toolbarTimeoutRef.current);
      }
    };
  }, [showToolbar, isToolbarHovered, restartHideTimer]);

  const showToolbarOnPointerMove = useCallback(() => {
    setShowToolbar(true);
    if (!isToolbarHovered) {
      restartHideTimer();
    }
  }, [isToolbarHovered, restartHideTimer]);

  const handleToolbarMouseEnter = useCallback(() => {
    setIsToolbarHovered(true);
    if (toolbarTimeoutRef.current) {
      clearTimeout(toolbarTimeoutRef.current);
    }
  }, []);

  const handleToolbarMouseLeave = useCallback(() => {
    setIsToolbarHovered(false);
  }, []);

  return {
    showToolbar,
    showToolbarOnPointerMove,
    handleToolbarMouseEnter,
    handleToolbarMouseLeave,
  };
}
