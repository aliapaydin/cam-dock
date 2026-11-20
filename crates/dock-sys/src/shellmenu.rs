//! Dosya ve klasörlerin Gezgin'deki sağ tık menüsü (IContextMenu): Aç, Birlikte
//! aç, Gönder, Kes/Kopyala, Sil, Özellikler ve yüklü programların ekledikleri.
//!
//! Menü kalıcı bir STA iş parçacığında açılır: "Kopyala" gibi komutların panoya
//! koyduğu nesneler ve Özellikler penceresi bu iş parçacığı yaşadıkça geçerli kalır.

use crate::wide;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_EXTENDEDVERBS,
    CMF_NORMAL, CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, CMIC_MASK_PTINVOKE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, GetMessageW, PostMessageW,
    RegisterClassW, SetForegroundWindow, TrackPopupMenuEx, TranslateMessage, MSG, SW_SHOWNORMAL, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WM_APP, WM_DRAWITEM, WM_INITMENUPOPUP, WM_MEASUREITEM, WM_MENUCHAR, WM_NULL, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

const CMIC_MASK_UNICODE: u32 = 0x0000_4000;
const CMIC_MASK_SHIFT_DOWN: u32 = 0x1000_0000;
const WM_SHOW: u32 = WM_APP + 41;

struct Req {
    path: String,
    x: i32,
    y: i32,
    extended: bool,
}

static TX: OnceLock<Mutex<Sender<Req>>> = OnceLock::new();
static HWND_RAW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

thread_local! {
    /// Alt menüler (Gönder, Birlikte aç) çizilirken iletilen menü mesajları için
    static ACTIVE: RefCell<Option<IContextMenu>> = const { RefCell::new(None) };
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_INITMENUPOPUP | WM_DRAWITEM | WM_MEASUREITEM | WM_MENUCHAR => {
            let handled = ACTIVE.with(|a| {
                let a = a.borrow();
                let Some(cm) = a.as_ref() else { return None };
                if let Ok(cm3) = cm.cast::<IContextMenu3>() {
                    let mut res = LRESULT(0);
                    if cm3.HandleMenuMsg2(msg, wp, lp, Some(&mut res)).is_ok() {
                        return Some(res);
                    }
                } else if let Ok(cm2) = cm.cast::<IContextMenu2>() {
                    if cm2.HandleMenuMsg(msg, wp, lp).is_ok() {
                        return Some(LRESULT(0));
                    }
                }
                None
            });
            match handled {
                Some(r) => r,
                None => DefWindowProcW(h, msg, wp, lp),
            }
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

unsafe fn show(owner: HWND, r: &Req) -> bool {
    let w = wide(&r.path);
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(w.as_ptr()), None, &mut pidl, 0, None).is_err() || pidl.is_null() {
        return false;
    }
    let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
    let ok = (|| -> Option<()> {
        let folder: IShellFolder = SHBindToParent(pidl, Some(&mut child)).ok()?;
        let cm: IContextMenu = folder.GetUIObjectOf(owner, &[child as *const ITEMIDLIST], None).ok()?;
        let menu = CreatePopupMenu().ok()?;
        let flags = CMF_NORMAL | if r.extended { CMF_EXTENDEDVERBS } else { 0 };
        if cm.QueryContextMenu(menu, 0, 1, 0x7FFF, flags).is_err() {
            let _ = DestroyMenu(menu);
            return None;
        }
        ACTIVE.with(|a| *a.borrow_mut() = Some(cm.clone()));
        let _ = SetForegroundWindow(owner);
        let cmd = TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_RIGHTBUTTON).0, r.x, r.y, owner, None).0;
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        ACTIVE.with(|a| *a.borrow_mut() = None);
        if cmd > 0 {
            let id = (cmd - 1) as usize;
            let mut ci = CMINVOKECOMMANDINFOEX {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
                fMask: CMIC_MASK_UNICODE | CMIC_MASK_PTINVOKE | if r.extended { CMIC_MASK_SHIFT_DOWN } else { 0 },
                hwnd: owner,
                lpVerb: PCSTR(id as *const u8),
                lpVerbW: PCWSTR(id as *const u16),
                nShow: SW_SHOWNORMAL.0,
                ptInvoke: POINT { x: r.x, y: r.y },
                ..Default::default()
            };
            let _ = cm.InvokeCommand(&mut ci as *mut _ as *const CMINVOKECOMMANDINFO);
        }
        let _ = DestroyMenu(menu);
        Some(())
    })()
    .is_some();
    CoTaskMemFree(Some(pidl as *const core::ffi::c_void));
    ok
}

fn start() -> &'static Mutex<Sender<Req>> {
    TX.get_or_init(|| {
        let (tx, rx) = channel::<Req>();
        std::thread::spawn(move || unsafe {
            let _ = OleInitialize(None);
            let class = wide("CamDockShellMenu");
            let inst = GetModuleHandleW(None).unwrap_or_default();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: inst.into(),
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let Ok(h) = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(inst.into()),
                None,
            ) else {
                return;
            };
            HWND_RAW.store(h.0 as isize, std::sync::atomic::Ordering::SeqCst);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_SHOW {
                    while let Ok(r) = rx.try_recv() {
                        show(h, &r);
                    }
                    continue;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        Mutex::new(tx)
    })
}

/// `path` dosyasının/klasörünün Gezgin menüsünü ekranda (x, y) noktasında açar.
/// `extended`: Shift basılıyken çıkan genişletilmiş menü.
pub fn open(path: &str, x: i32, y: i32, extended: bool) {
    let tx = start();
    let _ = tx.lock().unwrap().send(Req { path: path.to_string(), x, y, extended });
    // İş parçacığı penceresini oluşturana kadar kısa bekleme
    for _ in 0..50 {
        let h = HWND_RAW.load(std::sync::atomic::Ordering::SeqCst);
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut core::ffi::c_void)), WM_SHOW, WPARAM(0), LPARAM(0));
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Fare imlecinin olduğu yerde açar.
pub fn open_at_cursor(path: &str, extended: bool) {
    let mut pt = POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
    }
    open(path, pt.x, pt.y, extended);
}
