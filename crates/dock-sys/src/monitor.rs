//! Monitör, imleç ve pencere konumu yardımcıları.

use crate::{hwnd, raw};
use serde::Serialize;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE,
    SW_SHOWNOACTIVATE,
};

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn from_win(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.left < o.right && o.left < self.right && self.top < o.bottom && o.top < self.bottom
    }
}

pub fn foreground() -> isize {
    unsafe { raw(GetForegroundWindow()) }
}

fn class_name(h: isize) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd(h), &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn is_shell_window(h: isize) -> bool {
    matches!(
        class_name(h).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

/// Monitörün aygıt adı ("\\.\DISPLAY1") ve tam dikdörtgeni.
/// Tauri'nin `Monitor::name()` değeriyle aynıdır.
fn monitor_info(mon: HMONITOR) -> Option<(String, Rect)> {
    unsafe {
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(mon, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            return None;
        }
        let len = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
        Some((
            String::from_utf16_lossy(&mi.szDevice[..len]),
            Rect::from_win(mi.monitorInfo.rcMonitor),
        ))
    }
}

/// Pencerenin bulunduğu monitör (aygıt adı, dikdörtgen).
pub fn monitor_of_window(h: isize) -> Option<(String, Rect)> {
    unsafe { monitor_info(MonitorFromWindow(hwnd(h), MONITOR_DEFAULTTONEAREST)) }
}

pub fn window_rect(h: isize) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd(h), &mut r).ok()? };
    Some(Rect::from_win(r))
}

/// Ön plandaki gerçek uygulama penceresi ve dikdörtgeni (masaüstü, taskbar ve
/// `exclude` içindekiler hariç; küçültülmüşse yok sayılır).
pub fn foreground_app_rect(exclude: &[isize]) -> Option<(isize, Rect)> {
    let fg = foreground();
    if fg == 0 || exclude.contains(&fg) || is_shell_window(fg) {
        return None;
    }
    unsafe {
        if IsIconic(hwnd(fg)).as_bool() || !IsWindowVisible(hwnd(fg)).as_bool() {
            return None;
        }
    }
    window_rect(fg).map(|r| (fg, r))
}

/// Ön plandaki pencere bir monitörü tamamen kaplıyorsa (tam ekran oyun, video,
/// sunum) o monitörün aygıt adını döndürür.
pub fn fullscreen_monitor(exclude: &[isize]) -> Option<String> {
    let (fg, wr) = foreground_app_rect(exclude)?;
    let (name, m) = monitor_of_window(fg)?;
    let covers = wr.left <= m.left && wr.top <= m.top && wr.right >= m.right && wr.bottom >= m.bottom;
    covers.then_some(name)
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// Pencereyi taşır (boyut aynı kalır), en üstte tutar, odak çalmaz.
pub fn move_to(h: isize, x: i32, y: i32) {
    unsafe {
        let _ = SetWindowPos(
            hwnd(h),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Pencereyi odak çalmadan gösterir.
pub fn show_no_activate(h: isize) {
    unsafe {
        let _ = ShowWindow(hwnd(h), SW_SHOWNOACTIVATE);
    }
}

pub fn hide(h: isize) {
    unsafe {
        let _ = ShowWindow(hwnd(h), SW_HIDE);
    }
}

pub fn is_visible(h: isize) -> bool {
    unsafe { IsWindowVisible(hwnd(h)).as_bool() }
}
