//! Açık uygulama pencerelerini listeleme ve öne getirme (taskbar'ın ana işi).

use crate::{hwnd, raw};
use serde::Serialize;
use std::path::Path;
use core::ffi::c_void;
use windows::core::PWSTR;
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumChildWindows, EnumWindows, GetAncestor, GetForegroundWindow, GetWindow,
    GetWindowLongPtrW, GA_ROOTOWNER,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    PostMessageW, SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_MINIMIZE, SW_RESTORE,
    SW_SHOWNORMAL, WM_CLOSE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR};

#[derive(Debug, Clone, Serialize)]
pub struct AppWindow {
    pub hwnd: isize,
    pub title: String,
    /// Uygulamanın exe yolu (UWP uygulamalarında gerçek uygulamanın exe'si).
    pub exe: String,
    pub minimized: bool,
    /// Store/paketli uygulamalar ve kendini tanıtan uygulamalar için kimlik
    /// (AppUserModelID). Küçültülmüş Store uygulamalarını da doğru gruplar.
    pub aumid: String,
    /// Pencerenin bulunduğu monitörün aygıt adı ("\\.\DISPLAY1")
    pub monitor: String,
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
    }
    pid
}

pub fn exe_of_pid(pid: u32) -> Option<String> {
    unsafe {
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            proc,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(proc);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn title_of(h: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(h);
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..got.max(0) as usize])
    }
}

fn is_cloaked(h: HWND) -> bool {
    let mut cloaked: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut core::ffi::c_void,
            4,
        )
        .is_ok()
            && cloaked != 0
    }
}

/// Alt+Tab'da görünecek türden bir uygulama penceresi mi?
fn is_app_window(h: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            return false;
        }
        if let Ok(owner) = GetWindow(h, GW_OWNER) {
            if !owner.is_invalid() {
                return false;
            }
        }
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        let app = ex & WS_EX_APPWINDOW.0 != 0;
        if !app && (ex & WS_EX_TOOLWINDOW.0 != 0 || ex & WS_EX_NOACTIVATE.0 != 0) {
            return false;
        }
        if GetWindowTextLengthW(h) == 0 {
            return false;
        }
        !is_cloaked(h)
    }
}

struct ChildSearch {
    host_pid: u32,
    found: u32,
}

unsafe extern "system" fn child_cb(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp.0 as *mut ChildSearch);
    let pid = pid_of(h);
    if pid != s.host_pid {
        s.found = pid;
        return BOOL(0);
    }
    BOOL(1)
}

/// UWP uygulamaları ApplicationFrameHost içinde çalışır; asıl uygulamanın exe'sini bul.
fn resolve_exe(h: HWND, exe: String) -> String {
    let is_host = Path::new(&exe)
        .file_name()
        .map(|f| f.to_string_lossy().eq_ignore_ascii_case("ApplicationFrameHost.exe"))
        .unwrap_or(false);
    if !is_host {
        return exe;
    }
    let mut s = ChildSearch {
        host_pid: pid_of(h),
        found: 0,
    };
    unsafe {
        let _ = EnumChildWindows(Some(h), Some(child_cb), LPARAM(&mut s as *mut _ as isize));
    }
    if s.found != 0 {
        exe_of_pid(s.found).unwrap_or(exe)
    } else {
        exe
    }
}

static AUMIDS: std::sync::Mutex<Option<std::collections::HashMap<isize, String>>> =
    std::sync::Mutex::new(None);

/// Pencerenin AppUserModelID'si (önce pencere özelliği, sonra paketli süreç).
fn aumid_of(h: HWND) -> String {
    if let Some(hit) = AUMIDS.lock().unwrap().as_ref().and_then(|m| m.get(&raw(h)).cloned()) {
        return hit;
    }
    let mut id = String::new();
    unsafe {
        if let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(h) {
            if let Ok(pv) = store.GetValue(&PKEY_AppUserModel_ID) {
                if let Ok(p) = PropVariantToStringAlloc(&pv) {
                    id = p.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(p.0 as *const c_void));
                }
            }
        }
        if id.is_empty() {
            if let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid_of(h)) {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32;
                if GetApplicationUserModelId(proc, &mut len, Some(PWSTR(buf.as_mut_ptr()))).0 == 0 {
                    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
                    id = String::from_utf16_lossy(&buf[..n]);
                }
                let _ = CloseHandle(proc);
            }
        }
    }
    AUMIDS
        .lock()
        .unwrap()
        .get_or_insert_with(Default::default)
        .insert(raw(h), id.clone());
    id
}

struct ListCtx<'a> {
    exclude: &'a [isize],
    out: Vec<AppWindow>,
}

unsafe extern "system" fn enum_cb(h: HWND, lp: LPARAM) -> BOOL {
    let ctx = &mut *(lp.0 as *mut ListCtx<'_>);
    if ctx.exclude.contains(&raw(h)) || !is_app_window(h) {
        return BOOL(1);
    }
    let Some(exe) = exe_of_pid(pid_of(h)) else {
        return BOOL(1);
    };
    ctx.out.push(AppWindow {
        hwnd: raw(h),
        title: title_of(h),
        exe: resolve_exe(h, exe),
        minimized: IsIconic(h).as_bool(),
        aumid: aumid_of(h),
        monitor: crate::monitor::monitor_of_window(raw(h))
            .map(|(n, _)| n)
            .unwrap_or_default(),
    });
    BOOL(1)
}

/// Açık uygulama pencereleri, Z sırasına göre (en üstteki ilk).
pub fn list(exclude: &[isize]) -> Vec<AppWindow> {
    let mut ctx = ListCtx {
        exclude,
        out: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.out
}

/// Bizim küçülttüğümüz son pencereler (geri getirirken aynısını seçmek için).
static RECENT_MIN: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());

fn remember_min(h: isize) {
    let mut v = RECENT_MIN.lock().unwrap();
    v.retain(|x| *x != h);
    v.push(h);
    if v.len() > 64 {
        v.remove(0);
    }
}

fn focus(h: HWND) {
    unsafe {
        if !SetForegroundWindow(h).as_bool() {
            crate::input::tap_alt();
            let _ = SetForegroundWindow(h);
        }
        let _ = BringWindowToTop(h);
    }
}

/// Bir uygulamanın (birden çok penceresi olabilir) dock simgesine tıklama:
/// - Uygulama öndeyse → öndeki penceresini küçült.
/// - Hiç görünür penceresi yoksa → en son küçültüleni eski konumuna getir.
/// - Görünür ama arkadaysa → öne getir.
/// `windows` Z sırasına göre (en üstteki ilk), `last_foreground` dock'a
/// tıklanmadan önceki ön plan penceresi.
pub fn activate_group(windows: &[isize], last_foreground: isize) {
    if windows.is_empty() {
        return;
    }
    unsafe {
        let fg_root = raw(GetAncestor(hwnd(last_foreground), GA_ROOTOWNER));
        let active = windows
            .iter()
            .copied()
            .find(|w| *w == last_foreground || (fg_root != 0 && *w == fg_root));
        if let Some(a) = active {
            if !IsIconic(hwnd(a)).as_bool() {
                let _ = ShowWindow(hwnd(a), SW_MINIMIZE);
                remember_min(a);
                return;
            }
        }
        let visible: Vec<isize> = windows
            .iter()
            .copied()
            .filter(|w| !IsIconic(hwnd(*w)).as_bool())
            .collect();
        if let Some(&top) = visible.first() {
            focus(hwnd(top));
            return;
        }
        // Hepsi küçültülmüş: en son bizim küçülttüğümüzü (yoksa ilkini) geri getir.
        let recent = RECENT_MIN.lock().unwrap().clone();
        let target = recent
            .iter()
            .rev()
            .copied()
            .find(|h| windows.contains(h))
            .unwrap_or(windows[0]);
        let _ = ShowWindow(hwnd(target), SW_RESTORE);
        focus(hwnd(target));
    }
}

/// Tek bir pencereyi öne getirir (önizleme kartına tıklama).
pub fn bring_front(window: isize) {
    unsafe {
        let h = hwnd(window);
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        focus(h);
    }
}

/// Taskbar gibi davranır: küçültülmüşse geri getir, zaten öndeyse küçült,
/// değilse öne getir. `last_foreground`: dock'a tıklanmadan önceki ön plan penceresi.
pub fn activate(window: isize, last_foreground: isize) {
    let h = hwnd(window);
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
            let _ = SetForegroundWindow(h);
        } else if window == last_foreground || GetForegroundWindow() == h {
            let _ = ShowWindow(h, SW_MINIMIZE);
        } else {
            let _ = SetForegroundWindow(h);
        }
    }
}

pub fn close(window: isize) {
    unsafe {
        let _ = PostMessageW(Some(hwnd(window)), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Bir exe, kısayol, klasör ya da URI'yi (ms-settings:, https:) açar.
pub fn launch(target: &str, args: Option<&str>) -> bool {
    let op = crate::wide("open");
    let file = crate::wide(target);
    let params = args.map(crate::wide);
    unsafe {
        let r = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            params
                .as_ref()
                .map(|p| PCWSTR(p.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        // ShellExecute, 32'den büyük bir değer döndürürse başarılıdır.
        r.0 as isize > 32
    }
}
