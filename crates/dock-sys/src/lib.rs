//! Cam Dock — Windows sistem katmanı.
//!
//! Tauri tarafı pencere tanıtıcılarını (HWND) `isize` olarak verir; bu crate'in
//! dış API'si windows crate tiplerini sızdırmaz.

pub mod appbar;
pub mod autostart;
pub mod backdrop;
pub mod catalog;
pub mod clipboard;
pub mod glass;
pub mod icons;
pub mod input;
pub mod media;
pub mod monitor;
pub mod privacy;
pub mod quick;
pub mod shortcut;
pub mod subclass;
pub mod sysinfo;
pub mod taskbar;
pub mod thumbs;
pub mod tray;
pub mod wallpaper;
pub mod winlist;

use windows::Win32::Foundation::HWND;

#[inline]
pub(crate) fn hwnd(h: isize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}

#[inline]
pub(crate) fn raw(h: HWND) -> isize {
    h.0 as isize
}

/// Sonu NUL ile biten UTF-16 dizi.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tek örnek kilidi. Başka bir Cam Dock zaten çalışıyorsa false (Windows ile
/// başlatma + elle açma iki dock oluşturmasın).
pub fn single_instance() -> bool {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    let name = wide("Local\\CamDock.SingleInstance");
    unsafe {
        match CreateMutexW(None, false, windows::core::PCWSTR(name.as_ptr())) {
            // Tanıtıcı süreç boyunca açık kalmalı: bilerek kapatılmıyor.
            Ok(_h) => GetLastError() != ERROR_ALREADY_EXISTS,
            Err(_) => true,
        }
    }
}
