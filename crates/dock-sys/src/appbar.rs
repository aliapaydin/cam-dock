//! Dock'u bir "AppBar" olarak kaydedip ekranın altında alan ayırır.
//! Böylece ekranı kaplayan (maximize) pencereler dock'un altına girmez.
//! Her monitördeki dock ayrı bir AppBar'dır.

use crate::hwnd;
use crate::monitor::Rect;
use std::collections::HashSet;
use std::sync::Mutex;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::WM_USER;

const ABE_BOTTOM: u32 = 3;
/// AppBar bildirimleri için kullandığımız mesaj kimliği.
pub const CALLBACK_MSG: u32 = WM_USER + 0x2A;

static REGISTERED: Mutex<Option<HashSet<isize>>> = Mutex::new(None);

fn data(h: isize) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd(h),
        uCallbackMessage: CALLBACK_MSG,
        uEdge: ABE_BOTTOM,
        ..Default::default()
    }
}

/// `monitor` dikdörtgeninin altından `height` piksel (fiziksel) ayırır.
/// Ayrılan gerçek dikdörtgeni döndürür.
pub fn reserve_bottom(window: isize, monitor: Rect, height: i32) -> Rect {
    unsafe {
        {
            let mut reg = REGISTERED.lock().unwrap();
            let set = reg.get_or_insert_with(HashSet::new);
            if set.insert(window) {
                let mut abd = data(window);
                SHAppBarMessage(ABM_NEW, &mut abd);
            }
        }

        let mut abd = data(window);
        abd.rc = RECT {
            left: monitor.left,
            top: monitor.bottom - height,
            right: monitor.right,
            bottom: monitor.bottom,
        };
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        // Sistem alt kenarı yukarı itmiş olabilir; yüksekliği koru.
        abd.rc.top = abd.rc.bottom - height;
        SHAppBarMessage(ABM_SETPOS, &mut abd);

        Rect {
            left: abd.rc.left,
            top: abd.rc.top,
            right: abd.rc.right,
            bottom: abd.rc.bottom,
        }
    }
}

/// Ayrılan alanı serbest bırakır.
pub fn release(window: isize) {
    let removed = REGISTERED
        .lock()
        .unwrap()
        .as_mut()
        .map(|s| s.remove(&window))
        .unwrap_or(false);
    if !removed {
        return;
    }
    let mut abd = data(window);
    unsafe {
        SHAppBarMessage(ABM_REMOVE, &mut abd);
    }
}

/// Tüm kayıtlı AppBar'ları serbest bırakır (çıkışta).
pub fn release_all() {
    let all: Vec<isize> = REGISTERED
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    for h in all {
        release(h);
    }
}
