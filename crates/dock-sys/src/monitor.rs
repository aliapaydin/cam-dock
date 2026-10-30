//! Monitör, imleç ve pencere konumu yardımcıları.

use crate::{hwnd, raw};
use serde::Serialize;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, IsIconic, IsZoomed,
    IsWindowVisible, SetWindowLongPtrW, SetWindowPos, ShowWindowAsync, GWL_EXSTYLE, GWL_STYLE,
    HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNOACTIVATE, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
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
    unsafe {
        // Ekranı kaplayan ama başlık çubuklu ya da maximize edilmiş normal
        // pencereler (ör. büyütülmüş Chrome, Gezgin) tam ekran sayılmaz.
        let style = GetWindowLongPtrW(hwnd(fg), GWL_STYLE) as u32;
        if style & WS_CAPTION.0 == WS_CAPTION.0 || IsZoomed(hwnd(fg)).as_bool() {
            return None;
        }
    }
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
    if let Some((_, _, w, hh)) = crate::subclass::allowed(h) {
        crate::subclass::allow(h, x, y, w, hh);
    }
    unsafe {
        let _ = SetWindowPos(
            hwnd(h),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            // Async: pencerenin iş parçacığı meşgulse beklemeyiz (kilitlenme olmaz).
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        );
    }
}

/// Pencereyi taşır ve boyutlandırır (fiziksel piksel), en üstte tutar, odak çalmaz.
pub fn set_bounds(h: isize, x: i32, y: i32, w: i32, height: i32) {
    crate::subclass::allow(h, x, y, w, height);
    unsafe {
        let _ = SetWindowPos(
            hwnd(h),
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            height,
            SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        );
    }
}

const BAD_STYLE: u32 = WS_CAPTION.0 | WS_SYSMENU.0 | WS_SIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0;

/// Dock'u çerçevesiz bir "popup + araç penceresi" yapar.
///
/// Tauri'nin pencere katmanı (tao) çerçevesiz pencerelerde de WS_CAPTION'ı
/// tutup başlığı WM_NCCALCSIZE ile gizler. Pencere etkinleşince ya da
/// yeniden çizilince Windows o başlığı ("Cam Dock" yazısı, simge, üst çizgi)
/// boyayabiliyor ve köşe kırpma bölgesi bozuluyor. Stili kökten temizliyoruz.
/// Dönüş: bir şey değiştiyse true.
pub fn make_popup(h: isize) -> bool {
    unsafe {
        let style = GetWindowLongPtrW(hwnd(h), GWL_STYLE) as u32;
        let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE) as u32;
        let new_style = (style & !BAD_STYLE) | WS_POPUP.0;
        let new_ex = (ex & !(WS_EX_APPWINDOW.0 | WS_EX_WINDOWEDGE.0)) | WS_EX_TOOLWINDOW.0;
        if new_style == style && new_ex == ex {
            return false;
        }
        SetWindowLongPtrW(hwnd(h), GWL_STYLE, new_style as isize);
        SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, new_ex as isize);
        let _ = SetWindowPos(
            hwnd(h),
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        true
    }
}

/// Pencereyi odak çalmadan gösterir.
pub fn show_no_activate(h: isize) {
    unsafe {
        let _ = ShowWindowAsync(hwnd(h), SW_SHOWNOACTIVATE);
    }
}

pub fn hide(h: isize) {
    unsafe {
        let _ = ShowWindowAsync(hwnd(h), SW_HIDE);
    }
}

/// Ön plandaki pencerenin sınıf adı (taskbar/tepsi menüsü açık mı anlamak için).
pub fn foreground_class() -> String {
    class_name(foreground())
}

pub fn is_visible(h: isize) -> bool {
    unsafe { IsWindowVisible(hwnd(h)).as_bool() }
}

/// Ön planda Başlat menüsü, arama, hızlı ayarlar, bildirim merkezi, tepsi menüsü
/// gibi bir kabuk paneli mi açık? (Açıkken dock otomatik gizlenmesin.)
pub fn foreground_is_shell_flyout() -> bool {
    let fg = foreground();
    if fg == 0 {
        return false;
    }
    let class = class_name(fg);
    if matches!(
        class.as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
            | "Xaml_WindowedPopupClass"
            | "#32768"
            | "Shell_InputSwitchTopLevelWindow"
            | "ControlCenterWindow"
    ) {
        return true;
    }
    if class == "Windows.UI.Core.CoreWindow" {
        let mut pid = 0u32;
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd(fg), Some(&mut pid));
        }
        if let Some(exe) = crate::winlist::exe_of_pid(pid) {
            let name = exe.rsplit('\\').next().unwrap_or("").to_ascii_lowercase();
            return matches!(
                name.as_str(),
                "startmenuexperiencehost.exe"
                    | "searchhost.exe"
                    | "searchapp.exe"
                    | "shellexperiencehost.exe"
                    | "shellhost.exe"
            );
        }
    }
    false
}
