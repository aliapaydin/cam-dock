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

/// Süreç kapanana kadar bekler (bekçi süreci için).
pub fn wait_exit(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE};
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, INFINITE);
            let _ = CloseHandle(h);
        }
    }
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
    /// Çalışan consent.exe (UAC) süreçleri
    consent: Vec<u32>,
}

/// Çalışan consent.exe süreçlerinin PID'leri. consent.exe SYSTEM olarak çalışır;
/// pencere sınıf adına güvenmek yerine pencerenin bu süreçlerden birine ait
/// olmasına bakıyoruz (Windows sürümüne göre sınıf adları değişebiliyor).
pub fn consent_pids() -> Vec<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                if String::from_utf16_lossy(&e.szExeFile[..n]).eq_ignore_ascii_case("consent.exe") {
                    out.push(e.th32ProcessID);
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// consent.exe'ye ait tüm üst düzey pencereler (görünmeyenler dahil):
/// (hwnd, sınıf, başlık, görünür mü, dikdörtgen). Teşhis ve yedek seçim için.
pub fn consent_windows() -> Vec<(isize, String, String, bool, (i32, i32, i32, i32))> {
    let pids = consent_pids();
    if pids.is_empty() {
        return Vec::new();
    }
    struct C {
        pids: Vec<u32>,
        out: Vec<(isize, String, String, bool, (i32, i32, i32, i32))>,
    }
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        let c = &mut *(lp.0 as *mut C);
        if c.pids.contains(&pid_of(h)) {
            let mut r = windows::Win32::Foundation::RECT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(h, &mut r);
            c.out.push((
                raw(h),
                crate::monitor::class_name(raw(h)),
                title_of(h),
                IsWindowVisible(h).as_bool(),
                (r.left, r.top, r.right - r.left, r.bottom - r.top),
            ));
        }
        BOOL(1)
    }
    let mut c = C { pids, out: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut c as *mut _ as isize));
    }
    c.out
}

/// Pencere bir UAC izin penceresi mi (dock'ta kalkan olarak gösterilecek)?
pub fn is_uac(h: isize) -> bool {
    let w = hwnd(h);
    uac_window(w) || {
        let p = pid_of(w);
        consent_pids().contains(&p)
    }
}

fn uac_item(h: HWND) -> AppWindow {
    let t = title_of(h);
    AppWindow {
        hwnd: raw(h),
        title: if t.is_empty() { "Kullanıcı Hesabı Denetimi".into() } else { t },
        exe: UAC_EXE.into(),
        minimized: false,
        aumid: String::new(),
        // Ekran dışındaki sahte pencere bir monitöre düşmeyebilir: boş bırak,
        // dock bunu her ekranda gösterir.
        monitor: crate::monitor::monitor_of_window(raw(h)).map(|(n, _)| n).unwrap_or_default(),
    }
}

/// UAC izin ekranını öne getirir. Taskbar'daki kalkan düğmesine tıklamakla aynı:
/// pencereyi etkinleştirince consent.exe güvenli masaüstüne geçip soruyu gösterir.
pub fn activate_uac(h: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{SwitchToThisWindow, SW_SHOW};
    let w = hwnd(h);
    unsafe {
        if IsIconic(w).as_bool() {
            let _ = ShowWindow(w, SW_RESTORE);
        } else {
            let _ = ShowWindow(w, SW_SHOW);
        }
        let _ = BringWindowToTop(w);
        let _ = SetForegroundWindow(w);
        SwitchToThisWindow(w, true);
    }
}

/// Yönetici izni (UAC) penceresi. Güvenli masaüstündeki izin penceresi arkada
/// kalınca Windows taskbar'a yanıp sönen bir kalkan düğmesi koyar; o düğmeyi
/// temsil eden pencereler bunlar. Dock'ta gösterilir, tıklayınca izin ekranı açılır.
const UAC_CLASSES: [&str; 2] = ["$$$Secure UAP Dummy Window Class For Interim Dialog", "Credential Dialog Xaml Host"];
pub const UAC_EXE: &str = "C:\\Windows\\System32\\consent.exe";

fn uac_window(h: HWND) -> bool {
    unsafe {
        IsWindowVisible(h).as_bool() && {
            let c = crate::monitor::class_name(raw(h));
            UAC_CLASSES.iter().any(|u| *u == c)
        }
    }
}

unsafe extern "system" fn enum_cb(h: HWND, lp: LPARAM) -> BOOL {
    let ctx = &mut *(lp.0 as *mut ListCtx<'_>);
    if ctx.exclude.contains(&raw(h)) {
        return BOOL(1);
    }
    if uac_window(h) || (!ctx.consent.is_empty() && IsWindowVisible(h).as_bool() && ctx.consent.contains(&pid_of(h))) {
        ctx.out.push(uac_item(h));
        return BOOL(1);
    }
    if !is_app_window(h) {
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
        consent: consent_pids(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    // consent.exe çalışıyor ama görünür penceresi yok: yine de tıklanabilir bir
    // kalkan göster (başlığı olan ilk penceresi; yoksa herhangi biri).
    if !ctx.consent.is_empty() && !ctx.out.iter().any(|w| w.exe == UAC_EXE) {
        let all = consent_windows();
        let pick = all.iter().find(|w| !w.2.is_empty()).or(all.first());
        if let Some(w) = pick {
            ctx.out.insert(0, uac_item(hwnd(w.0)));
        }
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
    if let Some(&u) = windows.iter().find(|w| is_uac(**w)) {
        return activate_uac(u);
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
    if is_uac(window) {
        return activate_uac(window);
    }
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
    launch_verb(target, args, "open")
}

/// `verb`: "open" ya da "runas" (yönetici olarak çalıştır).
pub fn launch_verb(target: &str, args: Option<&str>, verb: &str) -> bool {
    let op = crate::wide(verb);
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

/// Pencerenin ait olduğu uygulamanın exe yolu ve pencere başlığı
/// (ekran süresinde uygulama bazlı sayım için).
pub fn app_of_window(h: isize) -> Option<(String, String)> {
    let hw = crate::hwnd(h);
    let exe = exe_of_pid(pid_of(hw))?;
    Some((exe, title_of(hw)))
}
