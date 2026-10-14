use std::mem::size_of;
use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::*,
            WindowsAndMessaging::{
                DestroyIcon, GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW,
                RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, WM_APP,
            },
        },
    },
    core::{Error, Result, w},
};

pub const CALLBACK_MESSAGE: u32 = WM_APP + 2;
// windows 0.62 does not export the SDK's NIN_KEYSELECT constant.
pub const NIN_KEYSELECT: u32 = NIN_SELECT | 1;

pub struct TrayIcon {
    data: NOTIFYICONDATAW,
    pub taskbar_created: u32,
}

impl TrayIcon {
    pub unsafe fn new(hwnd: HWND) -> Result<Self> {
        let module = GetModuleHandleW(None)?;
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: CALLBACK_MESSAGE,
            hIcon: HICON(
                LoadImageW(
                    Some(module.into()),
                    w!("AQUA_ICON"),
                    IMAGE_ICON,
                    GetSystemMetrics(SM_CXSMICON),
                    GetSystemMetrics(SM_CYSMICON),
                    LR_DEFAULTCOLOR,
                )?
                .0,
            ),
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            ..Default::default()
        };
        data.szTip[..5].copy_from_slice(&super::wide("AQUA"));
        let icon = Self {
            data,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
        };
        icon.add()?;
        Ok(icon)
    }

    pub unsafe fn add(&self) -> Result<()> {
        if !Shell_NotifyIconW(NIM_ADD, &self.data).as_bool() {
            return Err(Error::new(E_FAIL, "无法添加 AQUA 托盘图标"));
        }
        if !Shell_NotifyIconW(NIM_SETVERSION, &self.data).as_bool() {
            return Err(Error::new(E_FAIL, "无法设置 AQUA 托盘图标版本"));
        }
        Ok(())
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
            let _ = DestroyIcon(self.data.hIcon);
        }
    }
}
