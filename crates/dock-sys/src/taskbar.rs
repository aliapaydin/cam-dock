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
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, DeleteObject, GetWindowRgn, SetWindowRgn, HGDIOBJ, NULLREGION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetClassNameW, GetWindowRect, GetWindowThreadProcessId,
    IsWindowVisible, SetForegroundWindow, ShowWindowAsync,
    EVENT_OBJECT_SHOW, SW_SHOWNA, OBJID_WINDOW, SW_HIDE, SW_SHOW, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
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

#[allow(dead_code)]
fn on_monitor(h: HWND, targets: &[Rect]) -> bool {
    monitor_of_window(raw(h))
        .map(|(_, r)| targets.contains(&r))
        .unwrap_or(false)
}

static HOOK: std::sync::Mutex<(isize, u32)> = std::sync::Mutex::new((0, 0));

unsafe extern "system" fn on_shell_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    h: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 || h.is_invalid() || TARGETS.lock().map(|t| t.is_none()).unwrap_or(true) {
        return;
    }
    let mut buf = [0u16; 64];
    let n = GetClassNameW(h, &mut buf);
    let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    if (class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd") && IsWindowVisible(h).as_bool() {
        let _ = ShowWindowAsync(h, SW_HIDE);
    }
}

/// Explorer'ın taskbar'ı gösterdiği anı (Win tuşu, flyout, fare kenarda) yakalayıp
/// anında geri gizleyen olay kancası. Mesaj döngüsü olan iş parçacığında (ana
/// iş parçacığı) çağrılmalı. Explorer yeniden başlarsa tekrar çağrılabilir.
pub fn install_hook() -> bool {
    // Bölge yöntemiyle kanca gerekmiyor (bkz. `hide`). Kancayı kaldır.
    let mut slot = HOOK.lock().unwrap();
    if slot.0 != 0 {
        unsafe {
            let _ = UnhookWinEvent(HWINEVENTHOOK(slot.0 as *mut core::ffi::c_void));
        }
        *slot = (0, 0);
    }
    true
}

#[allow(dead_code)]
fn install_hook_old() -> bool {
    let Some(&main) = tray_windows().first() else { return false };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(main, Some(&mut pid));
    }
    let mut slot = HOOK.lock().unwrap();
    if slot.1 == pid && slot.0 != 0 {
        return true;
    }
    unsafe {
        if slot.0 != 0 {
            let _ = UnhookWinEvent(HWINEVENTHOOK(slot.0 as *mut core::ffi::c_void));
        }
        // Yalnızca "gösterildi" olayı: taskbar'ı biz gizlediğimiz için tekrar
        // görünmesi her zaman bir SHOW olayıyla olur. (Geniş aralık, explorer'ın
        // her pencere hareketini ana iş parçacığına yığıyordu.)
        let hook = SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_SHOW,
            None,
            Some(on_shell_event),
            pid,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        *slot = (hook.0 as isize, pid);
        !hook.is_invalid()
    }
}

/// Kancanın bağlı olduğu explorer süreci hâlâ aynı mı? (Explorer yeniden
/// başlarsa false; `install_hook` yeniden çağrılmalı.)
pub fn hook_current() -> bool {
    true
}

#[allow(dead_code)]
fn hook_current_old() -> bool {
    let Some(&main) = tray_windows().first() else { return true };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(main, Some(&mut pid));
    }
    HOOK.lock().map(|s| s.1 == pid && s.0 != 0).unwrap_or(false)
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

/// Taskbar pencerelerini boş bir bölgeyle kırpar: pencere Windows'un gözünde
/// "açık" kalır ama hiçbir pikseli çizilmez ve tıklamaları yakalamaz.
///
/// Eskiden SW_HIDE ile gizleyip Windows tekrar gösterince anında geri
/// gizliyorduk; otomatik gizlenen taskbar'ın fareyi ekran kenarında görünce
/// kendini göstermeye çalışmasıyla saniyede onlarca kez çekişme oluyor, fare
/// hareketi bozuluyordu. Bölgeyle kırpınca Windows istediği gibi gösterip
/// saklayabilir; kullanıcı hiçbir şey görmez, çekişme olmaz.
unsafe fn clip_empty(h: HWND) {
    let rgn = CreateRectRgn(0, 0, 0, 0);
    if SetWindowRgn(h, Some(rgn), true) == 0 {
        let _ = DeleteObject(HGDIOBJ(rgn.0));
    }
}

unsafe fn is_clipped(h: HWND) -> bool {
    let probe = CreateRectRgn(0, 0, 0, 0);
    let kind = GetWindowRgn(h, probe);
    let _ = DeleteObject(HGDIOBJ(probe.0));
    kind == NULLREGION
}

/// Tüm ekranlardaki taskbar'ları görünmez yapar. Dönüş: geri getirirken
/// kullanılacak önceki otomatik gizleme durumu. Tekrar çağrılabilir.
pub fn hide(monitors: &[Rect]) -> u32 {
    let prev = current_state();
    // Otomatik gizle: taskbar ekranın altında alan ayırmasın.
    set_state(STATE_AUTOHIDE);
    for h in tray_windows() {
        unsafe {
            clip_empty(h);
            // Önceki sürümler SW_HIDE ile saklamış olabilir: görünür yap ki
            // Windows'un kendi otomatik gizleme mantığı normal çalışsın.
            if !IsWindowVisible(h).as_bool() {
                let _ = ShowWindowAsync(h, SW_SHOWNA);
            }
        }
    }
    *TARGETS.lock().unwrap() = Some(monitors.to_vec());
    prev
}

/// Explorer yeniden başlarsa ya da bölgeyi sıfırlarsa tekrar kırp.
/// Dönüş: düzeltilen pencere sayısı.
pub fn enforce_hidden() -> usize {
    if TARGETS.lock().unwrap().is_none() {
        return 0;
    }
    let mut fixed = 0;
    for h in tray_windows() {
        unsafe {
            if !is_clipped(h) {
                clip_empty(h);
                fixed += 1;
            }
        }
    }
    fixed
}

/// Taskbar'ı verilen önceki duruma döndürür ve tüm monitörlerde görünür yapar.
pub fn restore(prev_state: u32) {
    *TARGETS.lock().unwrap() = None;
    let state = if prev_state == 0 { STATE_NORMAL } else { prev_state };
    set_state(state);
    for h in tray_windows() {
        unsafe {
            // Bölgeyi kaldır (taskbar tekrar çizilir), görünür yap.
            SetWindowRgn(h, None, true);
            let _ = ShowWindowAsync(h, SW_SHOW);
        }
    }
}

pub fn is_hidden() -> bool {
    TARGETS.lock().unwrap().is_some()
}

/// Ana taskbar'ı geçici olarak gösterir ve odaklar (otomatik gizlenen taskbar
/// odaklanınca yukarı kayar). Dönüş: taskbar'ın ekran dikdörtgeni.
pub fn peek() -> Option<Rect> {
    let main = *tray_windows().first()?;
    unsafe {
        let _ = ShowWindowAsync(main, SW_SHOW);
        let _ = SetForegroundWindow(main);
        let mut r = RECT::default();
        GetWindowRect(main, &mut r).ok()?;
        Some(Rect::from_win(r))
    }
}

/// Ana taskbar'ın dikdörtgeni.
pub fn main_rect() -> Option<Rect> {
    let main = *tray_windows().first()?;
    let mut r = RECT::default();
    unsafe { GetWindowRect(main, &mut r).ok()? };
    Some(Rect::from_win(r))
}
