//! Sistem tepsisindeki uygulamalar. Windows 11'in "gizli simgeler" menüsü klasik
//! taskbar'a bağlı olduğu için onu açamıyoruz; bunun yerine Windows'un tepsi
//! simgeleri için tuttuğu kaydı (HKCU\Control Panel\NotifyIconSettings) okuyup
//! şu an çalışanları simgeleriyle listeliyoruz.

use crate::wide;
use serde::Serialize;
use std::collections::HashSet;
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

#[derive(Debug, Clone, Serialize)]
pub struct TrayApp {
    pub name: String,
    pub exe: String,
    /// Windows'un sakladığı tepsi simgesi (PNG)
    #[serde(skip)]
    pub icon: Option<Vec<u8>>,
    pub promoted: bool,
    /// Yakalanan gerçek tepsi simgesinin kimliği (yoksa boş: menü exe ile aranır)
    pub id: String,
    /// Simgenin ipucu yazısı (ör. "Proton VPN – Bağlı")
    pub tip: String,
}

/// Exe'nin kendi tanıttığı ad (sürüm bilgisindeki FileDescription).
pub fn file_description(exe: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    unsafe {
        let w = wide(exe);
        let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, buf.as_mut_ptr() as *mut _).ok()?;
        let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tr = wide("\\VarFileInfo\\Translation");
        let mut langs: Vec<(u16, u16)> = Vec::new();
        if VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(tr.as_ptr()), &mut p, &mut len).as_bool() && len >= 4 {
            let n = len as usize / 4;
            let arr = std::slice::from_raw_parts(p as *const u16, n * 2);
            for k in 0..n {
                langs.push((arr[k * 2], arr[k * 2 + 1]));
            }
        }
        langs.push((0x0409, 0x04B0));
        langs.push((0x0409, 0x04E4));
        for (lang, cp) in langs {
            let q = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription"));
            if VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut p, &mut len).as_bool() && len > 1 {
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(p as *const u16, len as usize - 1));
                let s = s.trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

fn running_exes() -> HashSet<String> {
    let mut set = HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return set };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                set.insert(String::from_utf16_lossy(&e.szExeFile[..n]).to_lowercase());
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    set
}

unsafe fn reg_string(key: HKEY, sub: &str, name: &str) -> Option<String> {
    let (s, n) = (wide(sub), wide(name));
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let r = RegGetValueW(key, PCWSTR(s.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_SZ, None,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void), Some(&mut size));
    if r != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&buf[..len]))
}

unsafe fn reg_binary(key: HKEY, sub: &str, name: &str) -> Option<Vec<u8>> {
    let (s, n) = (wide(sub), wide(name));
    let mut size = 0u32;
    if RegGetValueW(key, PCWSTR(s.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_BINARY, None, None, Some(&mut size)) != ERROR_SUCCESS
        || size == 0
    {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let r = RegGetValueW(key, PCWSTR(s.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_BINARY, None,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void), Some(&mut size));
    (r == ERROR_SUCCESS).then(|| {
        buf.truncate(size as usize);
        buf
    })
}

unsafe fn reg_dword(key: HKEY, sub: &str, name: &str) -> Option<u32> {
    let (s, n) = (wide(sub), wide(name));
    let mut v = 0u32;
    let mut size = 4u32;
    let r = RegGetValueW(key, PCWSTR(s.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_DWORD, None,
        Some(&mut v as *mut _ as *mut core::ffi::c_void), Some(&mut size));
    (r == ERROR_SUCCESS).then_some(v)
}

/// "{F38BF404-...}\\app.exe" gibi bilinen klasör önekli yolları açar.
fn expand_known_folder(path: &str) -> String {
    if !path.starts_with('{') {
        return path.to_string();
    }
    let Some(end) = path.find('}') else { return path.to_string() };
    let Ok(guid) = GUID::try_from(&path[1..end]) else { return path.to_string() };
    unsafe {
        match SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, None) {
            Ok(p) => {
                let base = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
                format!("{base}{}", &path[end + 1..])
            }
            Err(_) => path.to_string(),
        }
    }
}

/// Tepside simgesi olan ve şu an çalışan uygulamalar.
pub fn running_apps() -> Vec<TrayApp> {
    let running = running_exes();
    let mut out: Vec<TrayApp> = Vec::new();
    unsafe {
        let root = wide("Control Panel\\NotifyIconSettings");
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(root.as_ptr()), Some(0), KEY_READ, &mut key) != ERROR_SUCCESS {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) != ERROR_SUCCESS {
                break;
            }
            i += 1;
            let sub = String::from_utf16_lossy(&name[..len as usize]);
            let Some(exe) = reg_string(key, &sub, "ExecutablePath") else { continue };
            let exe = expand_known_folder(&exe);
            let file = exe.rsplit('\\').next().unwrap_or("").to_lowercase();
            if file.is_empty() || !running.contains(&file) || file == "explorer.exe" || file == "cam-dock.exe" {
                continue;
            }
            // Aynı uygulamanın eski sürümleri (ör. her WhatsApp güncellemesi ayrı
            // klasör) ayrı kayıt bırakır: exe adına göre tekilleştir, öne
            // çıkarılmış ve simgesi olanı tercih et.
            let promoted = reg_dword(key, &sub, "IsPromoted").unwrap_or(0) != 0;
            if let Some(prev) = out.iter_mut().find(|a| {
                a.exe.rsplit('\\').next().unwrap_or("").eq_ignore_ascii_case(&file)
            }) {
                if promoted && !prev.promoted {
                    prev.promoted = true;
                    prev.exe = exe.clone();
                    if let Some(icon) = reg_binary(key, &sub, "IconSnapshot") {
                        prev.icon = Some(icon);
                    }
                }
                continue;
            }
            let tip = reg_string(key, &sub, "InitialTooltip").unwrap_or_default();
            let fallback = file.trim_end_matches(".exe").to_string();
            out.push(TrayApp {
                name: if tip.trim().is_empty() { fallback } else { tip.lines().next().unwrap_or("").trim().to_string() },
                exe,
                icon: reg_binary(key, &sub, "IconSnapshot"),
                promoted,
                id: String::new(),
                tip: String::new(),
            });
        }
        let _ = RegCloseKey(key);
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Panel listesi: yakalanan gerçek tepsi simgeleri (her birinin kendi menüsü
/// var) ve kayıt defterinde olup henüz yakalanmamış çalışan uygulamalar.
pub fn panel_apps() -> Vec<TrayApp> {
    let reg = running_apps();
    let hook = crate::trayhook::snapshot();
    let mut out: Vec<TrayApp> = Vec::new();
    let file = |p: &str| p.rsplit('\\').next().unwrap_or("").to_lowercase();
    for i in &hook {
        let f = file(&i.exe);
        if f.is_empty() || f == "explorer.exe" || f == "cam-dock.exe" {
            continue;
        }
        let r = reg.iter().find(|a| file(&a.exe) == f);
        let tip_first = i.tip.lines().next().unwrap_or("").trim().to_string();
        let name = file_description(&i.exe)
            .filter(|d| d.len() <= 40)
            .or_else(|| r.map(|a| a.name.clone()).filter(|n| !n.to_lowercase().ends_with(".exe") && !n.contains('.')))
            .or_else(|| (!tip_first.is_empty()).then(|| tip_first.clone()))
            .unwrap_or_else(|| f.trim_end_matches(".exe").to_string());
        let icon = crate::icons::hicon_png(i.hicon, 32).or_else(|| r.and_then(|a| a.icon.clone()));
        out.push(TrayApp {
            name,
            exe: i.exe.clone(),
            icon,
            promoted: r.is_some_and(|a| a.promoted),
            id: i.id(),
            tip: i.tip.trim().to_string(),
        });
    }
    // Yakalanmamış ama çalışan (menüleri yeniden bildirince gelir)
    for a in reg {
        if !out.iter().any(|o| file(&o.exe) == file(&a.exe)) {
            let mut a = a;
            if let Some(d) = file_description(&a.exe).filter(|d| d.len() <= 40) {
                a.name = d;
            }
            out.push(a);
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}
