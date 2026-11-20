//! Araç widget'ları için sistem çağrıları: diskler, Geri Dönüşüm Kutusu,
//! ekrandan renk alma, Ekran görüntüleri klasörü.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC, CLR_INVALID};
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW};
use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHQueryRecycleBinW, SHQUERYRBINFO, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, PM_REMOVE,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

#[derive(Debug, Clone, Serialize, Default)]
pub struct Drive {
    pub root: String,
    pub label: String,
    pub total: u64,
    pub free: u64,
    /// "fixed" | "removable" | "network" | "other"
    pub kind: String,
}

pub fn drives() -> Vec<Drive> {
    let mut out = Vec::new();
    unsafe {
        let mut buf = [0u16; 512];
        let n = GetLogicalDriveStringsW(Some(&mut buf)) as usize;
        for root in buf[..n.min(buf.len())].split(|&c| c == 0).filter(|s| !s.is_empty()) {
            let mut z: Vec<u16> = root.to_vec();
            z.push(0);
            let p = PCWSTR(z.as_ptr());
            let kind = match GetDriveTypeW(p) {
                3 => "fixed",
                2 => "removable",
                4 => "network",
                _ => "other",
            };
            if kind == "other" {
                continue; // CD sürücüsü, RAM disk vb.
            }
            let (mut free, mut total) = (0u64, 0u64);
            if GetDiskFreeSpaceExW(p, None, Some(&mut total), Some(&mut free)).is_err() || total == 0 {
                continue;
            }
            let mut name = [0u16; 128];
            let _ = GetVolumeInformationW(p, Some(&mut name), None, None, None, None);
            let len = name.iter().position(|&c| c == 0).unwrap_or(0);
            out.push(Drive {
                root: String::from_utf16_lossy(root),
                label: String::from_utf16_lossy(&name[..len]),
                total,
                free,
                kind: kind.into(),
            });
        }
    }
    out
}

/// (öğe sayısı, toplam bayt)
pub fn recycle_info() -> (i64, i64) {
    unsafe {
        let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
        if SHQueryRecycleBinW(PCWSTR::null(), &mut info).is_ok() {
            (info.i64NumItems, info.i64Size)
        } else {
            (0, 0)
        }
    }
}

pub fn recycle_empty() -> bool {
    unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND).is_ok() }
}

/// Ekranda bir noktanın rengi (0xRRGGBB). Koordinatlar fiziksel piksel.
pub fn pixel(x: i32, y: i32) -> Option<u32> {
    unsafe {
        let dc = GetDC(None);
        let c = GetPixel(dc, x, y);
        ReleaseDC(None, dc);
        if c.0 == CLR_INVALID {
            return None;
        }
        let v = c.0; // 0x00BBGGRR
        Some(((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF))
    }
}

static PICKING: AtomicBool = AtomicBool::new(false);
/// 0: sürüyor, 1: seçildi, 2: iptal
static PICK_STATE: AtomicI32 = AtomicI32::new(0);
static PICK_X: AtomicI32 = AtomicI32::new(0);
static PICK_Y: AtomicI32 = AtomicI32::new(0);

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && PICKING.load(Ordering::SeqCst) {
        let m = wp.0 as u32;
        let info = &*(lp.0 as *const windows::Win32::UI::WindowsAndMessaging::MSLLHOOKSTRUCT);
        match m {
            WM_LBUTTONDOWN => {
                PICK_X.store(info.pt.x, Ordering::SeqCst);
                PICK_Y.store(info.pt.y, Ordering::SeqCst);
                PICK_STATE.store(1, Ordering::SeqCst);
                return LRESULT(1); // tıklama alttaki uygulamaya gitmesin
            }
            WM_RBUTTONDOWN => {
                PICK_STATE.store(2, Ordering::SeqCst);
                return LRESULT(1);
            }
            WM_LBUTTONUP | WM_RBUTTONUP => return LRESULT(1),
            _ => {}
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

/// Ekrandan renk seçimi: sol tık seçer, sağ tık ya da Esc iptal eder (30 sn
/// sonra kendiliğinden biter). Seçim sürerken `hover` imlecin altındaki rengi alır.
/// Bu çağrı seçim bitene kadar bekler; kendi iş parçacığında çalıştırılmalı.
pub fn pick_color(hover: impl Fn(u32, i32, i32)) -> Option<u32> {
    if PICKING.swap(true, Ordering::SeqCst) {
        return None;
    }
    PICK_STATE.store(0, Ordering::SeqCst);
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0) };
    let Ok(hook) = hook else {
        PICKING.store(false, Ordering::SeqCst);
        return None;
    };
    let start = Instant::now();
    let mut last = 0u32;
    let mut result = None;
    loop {
        // Düşük seviyeli kanca bu iş parçacığının mesaj döngüsünü ister
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {}
        }
        match PICK_STATE.load(Ordering::SeqCst) {
            1 => {
                result = pixel(PICK_X.load(Ordering::SeqCst), PICK_Y.load(Ordering::SeqCst));
                break;
            }
            2 => break,
            _ => {}
        }
        if crate::input::key_down(0x1B) || start.elapsed() > Duration::from_secs(30) {
            break;
        }
        let mut pt = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        if let Some(c) = pixel(pt.x, pt.y) {
            if c != last {
                last = c;
                hover(c, pt.x, pt.y);
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
    PICKING.store(false, Ordering::SeqCst);
    result
}

/// Windows'un "Ekran görüntüleri" klasörü (Win+PrtScn ve Ekran Alıntısı Aracı buraya kaydeder).
pub fn screenshots_dir() -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Screenshots, SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Screenshots, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}
