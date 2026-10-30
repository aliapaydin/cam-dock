//! Dock penceresinin mesajlarına araya girerek:
//!
//! - **Başlık çubuğu asla çizilmez.** Tauri'nin pencere katmanı (tao) çerçevesiz
//!   pencerelerde WS_CAPTION'ı tutar; pencere etkinleşince ya da yeniden çizilince
//!   Windows "Cam Dock" başlığını boyayabiliyor. Stil değişikliklerini süzer
//!   (WM_STYLECHANGING), çerçeve çizimini (WM_NCPAINT) ve etkinleşmede çerçeve
//!   yeniden çizimini (WM_NCACTIVATE) engelleriz.
//! - **Konum kilidi.** Dock yalnızca bizim verdiğimiz yere gidebilir. Başka biri
//!   (DPI değişimi, kabuk, pencere katmanı) taşımaya/boyutlandırmaya kalkarsa
//!   WM_WINDOWPOSCHANGING'de izin verilen dikdörtgene geri çeviririz.
//!
//! `install` pencerenin sahibi olan iş parçacığında (Tauri ana iş parçacığı)
//! çağrılmalıdır.

use crate::{hwnd, raw};
use std::collections::HashMap;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GWL_STYLE, STYLESTRUCT, SWP_NOMOVE, SWP_NOSIZE, WINDOWPOS, WM_NCACTIVATE,
    WM_NCDESTROY, WM_NCPAINT, WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_CAPTION,
    WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
    WS_SIZEBOX, WS_SYSMENU,
};

const SUBCLASS_ID: usize = 0xCA3D_0C;
const BAD_STYLE: u32 =
    WS_CAPTION.0 | WS_SYSMENU.0 | WS_SIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0;

/// Pencere → izin verilen dikdörtgen (x, y, w, h; fiziksel piksel)
static ALLOWED: Mutex<Option<HashMap<isize, (i32, i32, i32, i32)>>> = Mutex::new(None);

pub fn allow(window: isize, x: i32, y: i32, w: i32, h: i32) {
    ALLOWED
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(window, (x, y, w, h));
}

pub fn allowed(window: isize) -> Option<(i32, i32, i32, i32)> {
    ALLOWED.lock().unwrap().as_ref()?.get(&window).copied()
}

pub fn forget(window: isize) {
    if let Some(map) = ALLOWED.lock().unwrap().as_mut() {
        map.remove(&window);
    }
}

unsafe extern "system" fn dock_proc(
    h: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_STYLECHANGING => {
            let ss = &mut *(lp.0 as *mut STYLESTRUCT);
            let which = wp.0 as i32;
            if which == GWL_STYLE.0 {
                ss.styleNew = (ss.styleNew & !BAD_STYLE) | WS_POPUP.0;
            } else if which == GWL_EXSTYLE.0 {
                ss.styleNew = (ss.styleNew & !(WS_EX_APPWINDOW.0 | WS_EX_WINDOWEDGE.0))
                    | WS_EX_TOOLWINDOW.0;
            }
        }
        // Çerçeve (başlık, kenar) hiç çizilmesin.
        WM_NCPAINT => return LRESULT(0),
        // lParam = -1: etkinlik durumu işlenir ama çerçeve yeniden boyanmaz.
        WM_NCACTIVATE => return DefSubclassProc(h, msg, wp, LPARAM(-1)),
        WM_WINDOWPOSCHANGING => {
            if let Some((x, y, w, hh)) = allowed(raw(h)) {
                let pos = &mut *(lp.0 as *mut WINDOWPOS);
                if (pos.flags & SWP_NOMOVE).0 == 0 {
                    pos.x = x;
                    pos.y = y;
                }
                if (pos.flags & SWP_NOSIZE).0 == 0 {
                    pos.cx = w;
                    pos.cy = hh;
                }
            }
        }
        WM_NCDESTROY => {
            forget(raw(h));
            let _ = RemoveWindowSubclass(h, Some(dock_proc), SUBCLASS_ID);
        }
        _ => {}
    }
    DefSubclassProc(h, msg, wp, lp)
}

/// Dock pencere işleyicisini takar. Pencerenin iş parçacığında çağrılmalı.
pub fn install(window: isize) -> bool {
    unsafe { SetWindowSubclass(hwnd(window), Some(dock_proc), SUBCLASS_ID, 0).as_bool() }
}
