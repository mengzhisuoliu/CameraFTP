/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { X } from 'lucide-react';
import wechatQrCodeSrc from '../assets/donate-qrcode-wechat.png';

interface WeChatDonateDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function WeChatDonateDialog({
  isOpen,
  onClose,
}: WeChatDonateDialogProps) {
  if (!isOpen) return null;

  return (
    <div
      data-testid="wechat-donate-dialog-overlay"
      className="fixed inset-0 z-[60] bg-black/70 flex items-center justify-center p-4"
    >
      <div className="bg-white rounded-xl max-w-md w-full shadow-2xl flex flex-col">
        <div className="flex items-center justify-between p-4 border-b">
          <h2 className="text-lg font-semibold text-gray-900">微信收款</h2>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
            aria-label="关闭微信收款对话框"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-4 flex flex-col items-center gap-4">
          <div className="bg-white rounded-xl p-2 border border-gray-200">
            <img src={wechatQrCodeSrc} alt="微信收款码" className="w-72 h-auto" />
          </div>

          <p className="text-sm text-gray-600 text-left leading-6">
            请先对当前界面截图，然后打开微信扫一扫，识别截图中的收款码。
          </p>
        </div>

        <div className="border-t p-4 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors text-sm font-medium"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
