//! Duvar kâğıdı: monitör başına dosya yolu ya da düz renk (dinamik mod için).

use crate::monitor::Rect;
use crate::wide;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};

pub enum Source {
    /// Görüntü dosyası
    File(String),
    /// Duvar kâğıdı yok: düz arka plan rengi
    Solid(u8, u8, u8),
}

unsafe fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
    s
}

fn reg_sz(key: &str, name: &str) -> Option<String> {
    let k = wide(key);
    let n = wide(name);
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

fn exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).is_file()
}

/// Verilen monitör dikdörtgenindeki duvar kâğıdı.
pub fn for_monitor(rect: Rect) -> Option<Source> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(dw) = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_ALL) {
            let count = dw.GetMonitorDevicePathCount().unwrap_or(0);
            for i in 0..count {
                let Ok(idp) = dw.GetMonitorDevicePathAt(i) else { continue };
                let id = take(idp);
                let idw = wide(&id);
                let Ok(r) = dw.GetMonitorRECT(PCWSTR(idw.as_ptr())) else { continue };
                if Rect::from_win(r) != rect {
                    continue;
                }
                if let Ok(wp) = dw.GetWallpaper(PCWSTR(idw.as_ptr())) {
                    let path = take(wp);
                    if exists(&path) {
                        return Some(Source::File(path));
                    }
                }
                break;
            }
        }
    }
    // Slayt gösterisi, Spotlight ya da tek duvar kâğıdı: Windows'un hazırladığı kopya.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let t = format!("{appdata}\\Microsoft\\Windows\\Themes\\TranscodedWallpaper");
        if exists(&t) {
            return Some(Source::File(t));
        }
    }
    if let Some(p) = reg_sz("Control Panel\\Desktop", "WallPaper") {
        if exists(&p) {
            return Some(Source::File(p));
        }
    }
    let rgb = reg_sz("Control Panel\\Colors", "Background")?;
    let v: Vec<u8> = rgb.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (v.len() == 3).then(|| Source::Solid(v[0], v[1], v[2]))
}
