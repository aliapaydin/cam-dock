//! Sistem ve araç widget'ları: disk/çöp kutusu, ağ sayaçları, renk seçici,
//! ekran görüntüleri.

use crate::commands::run;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Default)]
pub struct DriveView {
    pub root: String,
    pub label: String,
    pub total: u64,
    pub free: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DiskInfo {
    pub drives: Vec<DriveView>,
    pub bin_items: i64,
    pub bin_bytes: i64,
}

#[tauri::command]
pub async fn disk_info() -> DiskInfo {
    run(|| {
        #[cfg(windows)]
        {
            let (items, bytes) = dock_sys::tools::recycle_info();
            DiskInfo {
                drives: dock_sys::tools::drives()
                    .into_iter()
                    .map(|d| DriveView { root: d.root, label: d.label, total: d.total, free: d.free, kind: d.kind })
                    .collect(),
                bin_items: items,
                bin_bytes: bytes,
            }
        }
        #[cfg(not(windows))]
        DiskInfo::default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn recycle_empty() -> bool {
    run(|| {
        #[cfg(windows)]
        {
            dock_sys::tools::recycle_empty()
        }
        #[cfg(not(windows))]
        false
    })
    .await
    .unwrap_or(false)
}

/// (alınan, gönderilen) toplam bayt — dock saniyede bir okuyup hızı hesaplar.
#[tauri::command]
pub fn net_counters() -> (u64, u64) {
    crate::platform::net_bytes()
}

static COLORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Ekrandan renk seçimi. Sürerken "color-hover" { hex, x, y } yayınlanır;
/// sol tık seçer (tıklama alttaki uygulamaya gitmez), sağ tık / Esc iptal.
#[tauri::command]
pub async fn color_pick(app: AppHandle) -> Option<String> {
    let _ = app.emit("color-picking", true);
    let a2 = app.clone();
    let res = run(move || {
        #[cfg(windows)]
        {
            dock_sys::tools::pick_color(|c, x, y| {
                let _ = a2.emit("color-hover", serde_json::json!({ "hex": format!("#{c:06X}"), "x": x, "y": y }));
            })
            .map(|c| format!("#{c:06X}"))
        }
        #[cfg(not(windows))]
        {
            let _ = a2;
            None::<String>
        }
    })
    .await
    .ok()
    .flatten();
    if let Some(hex) = &res {
        let mut h = COLORS.lock().unwrap();
        h.retain(|x| x != hex);
        h.insert(0, hex.clone());
        h.truncate(16);
    }
    let _ = app.emit("color-picking", false);
    if let Some(hex) = &res {
        let _ = app.emit("color-picked", hex.clone());
    }
    res
}

#[tauri::command]
pub fn color_history() -> Vec<String> {
    COLORS.lock().unwrap().clone()
}

#[tauri::command]
pub fn color_forget() {
    COLORS.lock().unwrap().clear();
}

#[tauri::command]
pub fn screenshots_dir() -> Option<String> {
    #[cfg(windows)]
    {
        dock_sys::tools::screenshots_dir().or_else(|| {
            std::env::var("USERPROFILE").ok().map(|u| format!("{u}\\Pictures\\Screenshots"))
        })
    }
    #[cfg(not(windows))]
    None
}

/// Ekran Alıntısı Aracı'nı (bölge seçimi) açar — Win+Shift+S ile aynı.
#[tauri::command]
pub fn screen_clip() -> bool {
    crate::platform::launch("ms-screenclip:", None)
}

/// Metni panoya yazar (renk kodu, çeviri, hesap sonucu).
#[tauri::command]
pub async fn copy_text(text: String) -> bool {
    run(move || {
        #[cfg(windows)]
        {
            dock_sys::clipboard::set_text(&text)
        }
        #[cfg(not(windows))]
        {
            let _ = text;
            false
        }
    })
    .await
    .unwrap_or(false)
}
