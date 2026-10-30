//! Başlat menüsü kısayolları (.lnk): hedef exe'yi çözer, menüdeki uygulamaları listeler.

use crate::wide;
use serde::Serialize;
use std::path::{Path, PathBuf};
use windows::core::{Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

/// Kısayolun hedefini döndürür (UWP/Store kısayollarında hedef yoktur → None).
pub fn resolve(lnk: &str) -> Option<String> {
    unsafe {
        // Zaten başlatılmışsa hata döner; önemli değil.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        let w = wide(lnk);
        file.Load(PCWSTR(w.as_ptr()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let target = String::from_utf16_lossy(&buf[..len]);
        (!target.is_empty()).then_some(target)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StartApp {
    pub name: String,
    /// Hedef exe; çözülemezse kısayolun kendisi
    pub path: String,
    pub shortcut: String,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else if p
            .extension()
            .map(|x| x.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            out.push(p);
        }
    }
}

/// Hem tüm kullanıcılar hem bu kullanıcı için Başlat menüsündeki uygulamalar.
pub fn start_menu_apps() -> Vec<StartApp> {
    let mut roots = Vec::new();
    for var in ["ProgramData", "APPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            roots.push(PathBuf::from(base).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
    }
    let mut files = Vec::new();
    for r in &roots {
        walk(r, &mut files, 0);
    }
    let skip = ["uninstall", "kaldır", "readme", "help", "yardım", "website"];
    let mut apps: Vec<StartApp> = files
        .into_iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if skip.iter().any(|s| lower.contains(s)) {
                return None;
            }
            let shortcut = p.to_string_lossy().to_string();
            let path = match resolve(&shortcut) {
                Some(t) if t.to_lowercase().ends_with(".exe") => t,
                Some(_) => return None, // klasör/belge kısayolu
                None => shortcut.clone(),
            };
            Some(StartApp {
                name,
                path,
                shortcut,
            })
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.path.eq_ignore_ascii_case(&b.path));
    apps
}
