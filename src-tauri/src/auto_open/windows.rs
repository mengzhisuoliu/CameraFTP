// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::System::Com::{CoCreateInstance, CoInitialize, CLSCTX_LOCAL_SERVER};
use windows::Win32::UI::Shell::{
    IApplicationActivationManager, IShellItem, IShellItemArray, SHCreateItemFromParsingName,
    SHCreateShellItemArrayFromShellItem, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::error::AppError;

// CLSID_ApplicationActivationManager = {31530919-7FCD-4F9E-8F95-D7525769F0C1}
const CLSID_APPLICATION_ACTIVATION_MANAGER: windows::core::GUID = windows::core::GUID {
    data1: 0x31530919,
    data2: 0x7FCD,
    data3: 0x4F9E,
    data4: [0x8F, 0x95, 0xD7, 0x52, 0x57, 0x69, 0xF0, 0xC1],
};

/// 使用系统默认程序打开
pub fn open_with_default(file_path: &PathBuf) -> Result<(), AppError> {
    open_with_shell_execute(file_path, None, None)
}

/// 使用 Windows 照片应用打开
pub fn open_with_photos(file_path: &PathBuf) -> Result<(), AppError> {
    unsafe {
        let _ = CoInitialize(None);
    }

    // 使用 IApplicationActivationManager::ActivateForFile 正确传递文件给 UWP 应用
    // 这是微软官方推荐的方式，比命令行方式更可靠
    // 参考: https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateforfile
    let result = unsafe {
        activate_uwp_app_for_file("Microsoft.Windows.Photos_8wekyb3d8bbwe!App", file_path)
    };

    if result.is_ok() {
        return Ok(());
    }

    // Fallback: 使用系统默认程序打开
    open_with_default(file_path)
}

/// 使用 IApplicationActivationManager::ActivateForFile 激活 UWP 应用并打开文件
unsafe fn activate_uwp_app_for_file(
    app_user_model_id: &str,
    file_path: &PathBuf,
) -> Result<(), AppError> {
    // 创建 IApplicationActivationManager 实例
    let manager: IApplicationActivationManager = CoCreateInstance(
        &CLSID_APPLICATION_ACTIVATION_MANAGER,
        None,
        CLSCTX_LOCAL_SERVER,
    )
    .map_err(|e| {
        AppError::Other(format!(
            "Failed to create ApplicationActivationManager: {}",
            e
        ))
    })?;

    // 创建 IShellItem 从文件路径
    let file_wide: Vec<u16> = file_path.as_os_str().encode_wide().chain(Some(0)).collect();

    let shell_item: IShellItem = SHCreateItemFromParsingName(PCWSTR(file_wide.as_ptr()), None)
        .map_err(|e| AppError::Other(format!("SHCreateItemFromParsingName failed: {}", e)))?;

    // 创建 IShellItemArray
    let shell_item_array: IShellItemArray = SHCreateShellItemArrayFromShellItem(&shell_item)
        .map_err(|e| {
            AppError::Other(format!("SHCreateShellItemArrayFromShellItem failed: {}", e))
        })?;

    // 调用 ActivateForFile - 返回 process ID
    let app_id_wide: Vec<u16> = OsStr::new(app_user_model_id)
        .encode_wide()
        .chain(Some(0))
        .collect();

    let _process_id = manager
        .ActivateForFile(
            PCWSTR(app_id_wide.as_ptr()),
            &shell_item_array,
            PCWSTR(ptr::null()),
        )
        .map_err(|e| AppError::Other(format!("ActivateForFile failed: {}", e)))?;

    Ok(())
}

/// 使用自定义程序打开
pub fn open_with_program(file_path: &PathBuf, program: &str) -> Result<(), AppError> {
    open_with_program_execute(file_path, program)
}

/// 打开文件夹并选中文件
pub fn open_folder_and_select_file(file_path: &PathBuf) -> Result<(), AppError> {
    // 使用 explorer /select,<path> 命令打开文件夹并选中文件
    let path_str = file_path.to_string_lossy();
    let arg = format!("/select,{}", path_str);

    unsafe {
        let _ = CoInitialize(None);
    }

    let explorer_wide: Vec<u16> = OsStr::new("explorer")
        .encode_wide()
        .chain(Some(0))
        .collect();

    let arg_wide: Vec<u16> = OsStr::new(&arg).encode_wide().chain(Some(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            None,
            PCWSTR(explorer_wide.as_ptr()),
            PCWSTR(arg_wide.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };

    if result.0 as usize <= 32 {
        return Err(AppError::Other(format!(
            "ShellExecute explorer failed with code {:?}",
            result.0
        )));
    }

    Ok(())
}

fn open_with_shell_execute(
    file_path: &PathBuf,
    operation: Option<&str>,
    arguments: Option<&PathBuf>,
) -> Result<(), AppError> {
    unsafe {
        let _ = CoInitialize(None);
    }

    let file_wide: Vec<u16> = file_path.as_os_str().encode_wide().chain(Some(0)).collect();

    let operation_ptr = match operation {
        Some(op) => {
            let op_wide: Vec<u16> = OsStr::new(op).encode_wide().chain(Some(0)).collect();
            PCWSTR(op_wide.as_ptr())
        }
        None => PCWSTR(ptr::null()),
    };

    let arguments_ptr = match arguments {
        Some(arg) => {
            let arg_wide: Vec<u16> = arg.as_os_str().encode_wide().chain(Some(0)).collect();
            PCWSTR(arg_wide.as_ptr())
        }
        None => PCWSTR(ptr::null()),
    };

    let result = unsafe {
        ShellExecuteW(
            None,
            operation_ptr,
            PCWSTR(file_wide.as_ptr()),
            arguments_ptr,
            None,
            SW_SHOWNORMAL,
        )
    };

    if result.0 as usize <= 32 {
        return Err(AppError::Other(format!(
            "ShellExecute failed with code {:?}",
            result.0
        )));
    }

    Ok(())
}

fn open_with_program_execute(file_path: &PathBuf, program: &str) -> Result<(), AppError> {
    let program_path = PathBuf::from(program);
    open_with_shell_execute(&program_path, None, Some(file_path))
}
