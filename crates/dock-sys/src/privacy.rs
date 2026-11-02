//! Mikrofon ve kamerayı şu an kullanan uygulamalar. Windows'un kendi
//! "mikrofon kullanımda" göstergesinin okuduğu gizlilik kayıtları:
//! HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\<yetenek>
//! Her uygulamanın alt anahtarında LastUsedTimeStop = 0 ise kullanım sürüyor.

use crate::wide;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};

const BASE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore";

unsafe fn open(parent: HKEY, sub: &str) -> Option<HKEY> {
    let w = wide(sub);
    let mut h = HKEY::default();
    RegOpenKeyExW(parent, PCWSTR(w.as_ptr()), None, KEY_READ, &mut h).ok().ok()?;
    Some(h)
}

unsafe fn subkeys(h: HKEY) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        if RegEnumKeyExW(h, i, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
        i += 1;
    }
    out
}

unsafe fn qword(h: HKEY, name: &str) -> Option<u64> {
    let w = wide(name);
    let mut v = 0u64;
    let mut size = 8u32;
    RegQueryValueExW(h, PCWSTR(w.as_ptr()), None, None, Some(&mut v as *mut u64 as *mut u8), Some(&mut size)).ok().ok()?;
    Some(v)
}

unsafe fn in_use(h: HKEY) -> bool {
    let start = qword(h, "LastUsedTimeStart").unwrap_or(0);
    let stop = qword(h, "LastUsedTimeStop");
    start != 0 && stop == Some(0)
}

/// `capability`: "microphone" | "webcam". Dönüş: kullanan uygulamaların
/// tanımlayıcıları (Store uygulaması için paket ailesi, masaüstü için exe yolu).
pub fn using(capability: &str) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let Some(root) = open(HKEY_CURRENT_USER, &format!("{BASE}\\{capability}")) else { return out };
        for name in subkeys(root) {
            if name == "NonPackaged" {
                if let Some(np) = open(root, "NonPackaged") {
                    for exe in subkeys(np) {
                        if let Some(k) = open(np, &exe) {
                            if in_use(k) {
                                out.push(exe.replace('#', "\\"));
                            }
                            let _ = RegCloseKey(k);
                        }
                    }
                    let _ = RegCloseKey(np);
                }
            } else if let Some(k) = open(root, &name) {
                if in_use(k) {
                    out.push(name);
                }
                let _ = RegCloseKey(k);
            }
        }
        let _ = RegCloseKey(root);
    }
    out
}
