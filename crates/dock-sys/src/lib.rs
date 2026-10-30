//! Cam Dock — Windows sistem katmanı.
//!
//! Tauri tarafı pencere tanıtıcılarını (HWND) `isize` olarak verir; bu crate'in
//! dış API'si windows crate tiplerini sızdırmaz.

pub mod appbar;
pub mod backdrop;
pub mod glass;
pub mod icons;
pub mod input;
pub mod media;
pub mod monitor;
pub mod shortcut;
pub mod subclass;
pub mod taskbar;
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
