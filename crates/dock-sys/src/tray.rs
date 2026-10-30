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
            if file.is_empty() || !running.contains(&file) || file == "explorer.exe" {
                continue;
            }
            if out.iter().any(|a| a.exe.eq_ignore_ascii_case(&exe)) {
                continue;
            }
            let tip = reg_string(key, &sub, "InitialTooltip").unwrap_or_default();
            let fallback = file.trim_end_matches(".exe").to_string();
            out.push(TrayApp {
                name: if tip.trim().is_empty() { fallback } else { tip.lines().next().unwrap_or("").trim().to_string() },
                exe,
                icon: reg_binary(key, &sub, "IconSnapshot"),
                promoted: reg_dword(key, &sub, "IsPromoted").unwrap_or(0) != 0,
            });
        }
        let _ = RegCloseKey(key);
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}
