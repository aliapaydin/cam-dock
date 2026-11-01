//! Windows ile başlatma: HKCU\Software\Microsoft\Windows\CurrentVersion\Run

use crate::wide;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const NAME: &str = "CamDock";

/// Kayıtlı başlatma komutu (yoksa None).
pub fn get() -> Option<String> {
    let k = wide(KEY);
    let n = wide(NAME);
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size))
            .ok()
            .ok()?;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// `exe` verilirse kaydeder, None ise kaldırır.
pub fn set(exe: Option<&str>) -> bool {
    let k = wide(KEY);
    let n = wide(NAME);
    unsafe {
        match exe {
            Some(path) => {
                let cmd = wide(&format!("\"{path}\" --autostart"));
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(k.as_ptr()),
                    PCWSTR(n.as_ptr()),
                    REG_SZ.0,
                    Some(cmd.as_ptr() as *const _),
                    (cmd.len() * 2) as u32,
                )
                .is_ok()
            }
            None => {
                let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()));
                r.is_ok() || get().is_none()
            }
        }
    }
}

/// HKCU altındaki bir DWORD değer (yoksa None).
pub fn reg_dword(key: &str, name: &str) -> Option<u32> {
    use windows::Win32::System::Registry::RRF_RT_REG_DWORD;
    let k = wide(key);
    let n = wide(name);
    let mut v = 0u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_DWORD, None, Some(&mut v as *mut u32 as *mut _), Some(&mut size))
            .ok()
            .ok()?;
    }
    Some(v)
}

pub fn reg_delete(key: &str, name: &str) {
    let k = wide(key);
    let n = wide(name);
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()));
    }
}

/// HKCU altında anahtar oluşturup metin değer yazar (`name` None = varsayılan değer).
pub fn reg_set_sz(key: &str, name: Option<&str>, value: &str) -> bool {
    let k = wide(key);
    let n = name.map(wide);
    let v = wide(value);
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            n.as_ref().map(|n| PCWSTR(n.as_ptr())).unwrap_or(PCWSTR::null()),
            REG_SZ.0,
            Some(v.as_ptr() as *const _),
            (v.len() * 2) as u32,
        )
        .is_ok()
    }
}

/// HKCU altındaki anahtarı alt anahtarlarıyla siler.
pub fn reg_delete_tree(key: &str) {
    use windows::Win32::System::Registry::RegDeleteTreeW;
    let k = wide(key);
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()));
    }
}
