//! Tepsi simgelerinin kendi menüleri. Uygulamalar tepsi simgelerini
//! `Shell_TrayWnd` sınıflı pencereye WM_COPYDATA ile bildirir (Shell_NotifyIcon).
//! Aynı sınıf adıyla gizli, en üstte bir pencere açıp bu mesajları önce biz
//! alıyoruz, kaydediyoruz ve olduğu gibi gerçek taskbar'a iletiyoruz. Böylece
//! her simgenin penceresi, kimliği ve geri çağırma mesajı elimizde: sağ tık
//! menüsünü taskbar'ın yaptığı gibi uygulamanın kendisine açtırabiliyoruz.
//!
//! Gerçek taskbar hiçbir mesajı kaçırmaz; pencere kapanınca (çıkış/çökme)
//! uygulamalar yine doğrudan taskbar'a yazar.

use crate::wide;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowExW, GetCursorPos,
    GetMessageW, GetWindowRect, GetWindowThreadProcessId, IsWindow, PostMessageW, RegisterClassW,
    RegisterWindowMessageW, SendMessageTimeoutW, SetTimer, SetWindowPos, TranslateMessage, HWND_BROADCAST,
    HWND_TOPMOST, MSG, SMTO_ABORTIFHUNG, SWP_NOACTIVATE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, PostQuitMessage, WM_COPYDATA,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_TIMER, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

#[derive(Debug, Clone)]
pub struct TrayIcon {
    pub hwnd: isize,
    pub uid: u32,
    pub callback: u32,
    pub version: u32,
    pub guid: Option<GUID>,
    pub pid: u32,
    pub exe: String,
    pub tip: String,
    pub hidden: bool,
}

static ICONS: Mutex<Vec<TrayIcon>> = Mutex::new(Vec::new());
static FAKE: AtomicIsize = AtomicIsize::new(0);

/// Shell_NotifyIcon'un gönderdiği veri (32/64 bit uyumlu NOTIFYICONDATA).
#[repr(C)]
#[derive(Clone, Copy)]
struct NidW32 {
    cb_size: u32,
    hwnd: u32,
    uid: u32,
    flags: u32,
    callback: u32,
    hicon: u32,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: GUID,
    balloon_icon: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TrayData {
    signature: u32,
    message: u32,
    nid: NidW32,
}

const SIGNATURE: u32 = 0x3475_3423;
const NIF_MESSAGE: u32 = 0x1;
const NIF_TIP: u32 = 0x4;
const NIF_STATE: u32 = 0x8;
const NIF_GUID: u32 = 0x20;
const NIS_HIDDEN: u32 = 0x1;

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}

/// Explorer'ın gerçek taskbar'ı (bizim sahte pencere hariç).
pub fn real_tray() -> isize {
    let class = wide("Shell_TrayWnd");
    let me = std::process::id();
    let mut after: Option<HWND> = None;
    unsafe {
        loop {
            match FindWindowExW(None, after, PCWSTR(class.as_ptr()), PCWSTR::null()) {
                Ok(h) if !h.is_invalid() => {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(h, Some(&mut pid));
                    if pid != me {
                        return h.0 as isize;
                    }
                    after = Some(h);
                }
                _ => return 0,
            }
        }
    }
}

fn record(data: &TrayData) {
    if data.signature != SIGNATURE {
        return;
    }
    let n = &data.nid;
    let h = n.hwnd as i32 as isize;
    let by_guid = n.flags & NIF_GUID != 0 && n.guid != GUID::zeroed();
    let mut icons = ICONS.lock().unwrap();
    let pos = icons.iter().position(|i| {
        if by_guid {
            i.guid == Some(n.guid)
        } else {
            i.hwnd == h && i.uid == n.uid
        }
    });
    match data.message {
        // NIM_DELETE
        2 => {
            if let Some(p) = pos {
                icons.remove(p);
            }
        }
        // NIM_ADD, NIM_MODIFY, NIM_SETVERSION
        0 | 1 | 4 => {
            let idx = match pos {
                Some(p) => p,
                None => {
                    if h == 0 {
                        return;
                    }
                    let mut pid = 0u32;
                    unsafe {
                        GetWindowThreadProcessId(hwnd(h), Some(&mut pid));
                    }
                    let exe = crate::winlist::exe_of_pid(pid).unwrap_or_default();
                    icons.push(TrayIcon {
                        hwnd: h,
                        uid: n.uid,
                        callback: 0,
                        version: 0,
                        guid: by_guid.then_some(n.guid),
                        pid,
                        exe,
                        tip: String::new(),
                        hidden: false,
                    });
                    icons.len() - 1
                }
            };
            let i = &mut icons[idx];
            if h != 0 {
                i.hwnd = h;
                i.uid = n.uid;
            }
            if data.message == 4 {
                i.version = n.version;
                return;
            }
            if n.flags & NIF_MESSAGE != 0 {
                i.callback = n.callback;
            }
            if n.flags & NIF_TIP != 0 {
                let len = n.tip.iter().position(|&c| c == 0).unwrap_or(n.tip.len());
                i.tip = String::from_utf16_lossy(&n.tip[..len]);
            }
            if n.flags & NIF_STATE != 0 && n.state_mask & NIS_HIDDEN != 0 {
                i.hidden = n.state & NIS_HIDDEN != 0;
            }
        }
        _ => {}
    }
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let forward = |m: u32, w: WPARAM, l: LPARAM| -> LRESULT {
        let real = real_tray();
        if real == 0 {
            return LRESULT(0);
        }
        let mut out = 0usize;
        let _ = SendMessageTimeoutW(hwnd(real), m, w, l, SMTO_ABORTIFHUNG, 3000, Some(&mut out));
        LRESULT(out as isize)
    };
    match msg {
        WM_COPYDATA => {
            let cds = lp.0 as *const COPYDATASTRUCT;
            if !cds.is_null() {
                let c = &*cds;
                if c.dwData == 1 && !c.lpData.is_null() && c.cbData as usize >= 8 + 24 {
                    let mut d: TrayData = std::mem::zeroed();
                    let n = (c.cbData as usize).min(std::mem::size_of::<TrayData>());
                    std::ptr::copy_nonoverlapping(c.lpData as *const u8, &mut d as *mut _ as *mut u8, n);
                    // Kayıt hatası iletmeyi asla engellemesin
                    let _ = std::panic::catch_unwind(|| record(&d));
                }
            }
            forward(msg, wp, lp)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TIMER => {
            keep_on_top(h);
            LRESULT(0)
        }
        // Taskbar'a gönderilen komutlar (masaüstünü göster vb.) gerçeğine gitsin
        m if m == WM_COMMAND || (WM_USER..0x8000).contains(&m) => forward(msg, wp, lp),
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// Mesajlar bize gelsin diye Z sırasında gerçek taskbar'ın üstünde kal; bazı
/// uygulamalar taskbar'ın konumunu pencereden okuduğu için boyutu da aynı olsun.
unsafe fn keep_on_top(h: HWND) {
    let real = real_tray();
    let mut r = RECT::default();
    if real != 0 && GetWindowRect(hwnd(real), &mut r).is_ok() {
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE);
    }
}

/// Kancayı başlatır (bir kez). Kendi iş parçacığında mesaj döngüsüyle çalışır.
pub fn start() {
    if FAKE.load(Ordering::SeqCst) != 0 {
        return;
    }
    std::thread::spawn(|| unsafe {
        let class = wide("Shell_TrayWnd");
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let Ok(h) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
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
        FAKE.store(h.0 as isize, Ordering::SeqCst);
        keep_on_top(h);
        SetTimer(Some(h), 1, 1000, None);
        // Uygulamalar simgelerini yeniden bildirsin (Explorer yeniden başladığında
        // yaptıkları gibi); böylece mevcut simgeler de kayda geçer.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let name = wide("TaskbarCreated");
            let m = RegisterWindowMessageW(PCWSTR(name.as_ptr()));
            if m != 0 {
                let _ = PostMessageW(Some(HWND_BROADCAST), m, WPARAM(0), LPARAM(0));
            }
        });
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

/// Kancayı kapatır (çıkışta): uygulamalar yeniden doğrudan taskbar'a yazar.
pub fn stop() {
    let h = FAKE.swap(0, Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(hwnd(h)), windows::Win32::UI::WindowsAndMessaging::WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn is_fake(h: isize) -> bool {
    h != 0 && FAKE.load(Ordering::SeqCst) == h
}

/// Bu exe'nin geçerli tepsi simgesi.
pub fn icon_for(exe: &str) -> Option<TrayIcon> {
    let file = exe.rsplit('\\').next().unwrap_or(exe).to_lowercase();
    let mut icons = ICONS.lock().unwrap();
    icons.retain(|i| unsafe { IsWindow(Some(hwnd(i.hwnd))).as_bool() });
    let matches = |i: &&TrayIcon| {
        i.callback != 0
            && (i.exe.eq_ignore_ascii_case(exe) || i.exe.rsplit('\\').next().unwrap_or("").eq_ignore_ascii_case(&file))
    };
    icons
        .iter()
        .filter(matches)
        .min_by_key(|i| i.hidden)
        .cloned()
}

/// Taskbar'ın yaptığı gibi simgenin sağ tık menüsünü açtırır.
pub fn context_menu(exe: &str) -> bool {
    let Some(i) = icon_for(exe) else { return false };
    unsafe {
        let mut pt = Default::default();
        let _ = GetCursorPos(&mut pt);
        let _ = AllowSetForegroundWindow(i.pid);
        let target = Some(hwnd(i.hwnd));
        let post = |w: usize, l: isize| {
            let _ = PostMessageW(target, i.callback, WPARAM(w), LPARAM(l));
        };
        if i.version >= 4 {
            let xy = ((pt.y as u32 & 0xffff) << 16 | (pt.x as u32 & 0xffff)) as usize;
            let ev = |e: u32| ((i.uid & 0xffff) << 16 | e) as isize;
            post(xy, ev(WM_RBUTTONDOWN));
            post(xy, ev(WM_RBUTTONUP));
            post(xy, ev(WM_CONTEXTMENU));
        } else {
            post(i.uid as usize, WM_RBUTTONDOWN as isize);
            post(i.uid as usize, WM_RBUTTONUP as isize);
            if i.version == 3 {
                post(i.uid as usize, WM_CONTEXTMENU as isize);
            }
        }
    }
    true
}
