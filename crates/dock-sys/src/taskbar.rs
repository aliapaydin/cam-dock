//! Klasik Windows taskbar'ını gizleme / geri getirme.
//!
//! Gizlerken önce taskbar'ı "otomatik gizle" moduna alıyoruz; böylece ekranın
//! altında ayrılmış çalışma alanı serbest kalıyor. Sonra yalnızca dock gösterilen
//! monitörlerdeki taskbar pencerelerini tamamen saklıyoruz. Dock olmayan
//! monitörlerde klasik taskbar otomatik gizlenen halde çalışmaya devam eder.

use crate::monitor::{monitor_of_window, Rect};
use crate::{raw, wide};
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW,
};

/// ABS_AUTOHIDE
pub const STATE_AUTOHIDE: u32 = 0x1;
/// ABS_ALWAYSONTOP (Windows 11'de normal, gizlenmeyen taskbar durumu)
pub const STATE_NORMAL: u32 = 0x2;

/// Gizli tutulacak taskbar'ların bulunduğu monitörler. None = gizleme yok.
static TARGETS: Mutex<Option<Vec<Rect>>> = Mutex::new(None);

fn tray_windows() -> Vec<HWND> {
    let mut out = Vec::new();
    let main = wide("Shell_TrayWnd");
    let secondary = wide("Shell_SecondaryTrayWnd");
    unsafe {
        if let Ok(h) = FindWindowW(PCWSTR(main.as_ptr()), PCWSTR::null()) {
            if !h.is_invalid() {
                out.push(h);
            }
        }
        let mut after: Option<HWND> = None;
        loop {
            match FindWindowExW(None, after, PCWSTR(secondary.as_ptr()), PCWSTR::null()) {
                Ok(h) if !h.is_invalid() => {
                    out.push(h);
                    after = Some(h);
                }
                _ => break,
            }
        }
    }
    out
}

fn on_target(h: HWND, targets: &[Rect]) -> bool {
    monitor_of_window(raw(h))
        .map(|(_, r)| targets.contains(&r))
        .unwrap_or(false)
}

fn appbar_data() -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    }
}

/// Taskbar'ın şu anki otomatik gizleme durumu (ABS_* bayrakları).
pub fn current_state() -> u32 {
    let mut abd = appbar_data();
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32 }
}

fn set_state(state: u32) {
    let mut abd = appbar_data();
    abd.lParam = LPARAM(state as isize);
    unsafe {
        SHAppBarMessage(ABM_SETSTATE, &mut abd);
    }
}

/// `monitors` üzerindeki taskbar'ları gizler. Dönüş: geri getirirken kullanılacak
/// önceki durum. Tekrar çağrılabilir (monitör listesi değişince).
pub fn hide(monitors: &[Rect]) -> u32 {
    let prev = current_state();
    set_state(STATE_AUTOHIDE);
    for h in tray_windows() {
        unsafe {
            if on_target(h, monitors) {
                let _ = ShowWindow(h, SW_HIDE);
            } else if !IsWindowVisible(h).as_bool() {
                // Bu monitörde artık dock yok: klasik taskbar geri gelsin.
                let _ = ShowWindow(h, SW_SHOW);
            }
        }
    }
    *TARGETS.lock().unwrap() = Some(monitors.to_vec());
    prev
}

/// Explorer bazen taskbar'ı geri gösterir (Win tuşu, explorer yeniden başlaması).
/// Bunu periyodik olarak çağırıp tekrar gizliyoruz.
pub fn enforce_hidden() {
    let Some(targets) = TARGETS.lock().unwrap().clone() else {
        return;
    };
    for h in tray_windows() {
        unsafe {
            if IsWindowVisible(h).as_bool() && on_target(h, &targets) {
                let _ = ShowWindow(h, SW_HIDE);
            }
        }
    }
}

/// Taskbar'ı verilen önceki duruma döndürür ve tüm monitörlerde görünür yapar.
pub fn restore(prev_state: u32) {
    *TARGETS.lock().unwrap() = None;
    let state = if prev_state == 0 { STATE_NORMAL } else { prev_state };
    set_state(state);
    for h in tray_windows() {
        unsafe {
            let _ = ShowWindow(h, SW_SHOW);
        }
    }
}

pub fn is_hidden() -> bool {
    TARGETS.lock().unwrap().is_some()
}
