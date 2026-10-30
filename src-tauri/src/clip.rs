//! Pano geçmişi: kopyalanan metin ve görseller. Yalnızca bellekte tutulur
//! (diske yazılmaz); dock kapanınca silinir. Şifre yöneticilerinin "geçmişe
//! ekleme" işaretli içerikleri hiç alınmaz.

use crate::commands::run;
use crate::{platform, AppState};
use base64::Engine as _;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
pub struct Item {
    pub id: u64,
    /// "text" | "image"
    pub kind: String,
    pub text: String,
    /// Görsel önizlemesi (data: URL)
    pub preview: String,
    pub width: u32,
    pub height: u32,
    /// Panoya yazan uygulama
    pub source: String,
    pub source_path: String,
    /// Unix ms
    pub time: i64,
    pub pinned: bool,
    #[serde(skip)]
    pub dib: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct History {
    pub items: Vec<Item>,
    next: u64,
}

fn app_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let stem = file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE")).unwrap_or(file);
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn capture(app: &AppHandle) {
    let (enabled, max) = {
        let state = app.state::<AppState>();
        let cfg = state.cfg.lock().unwrap();
        (cfg.clipboard.enabled, cfg.clipboard.max_items.clamp(5, 200) as usize)
    };
    if !enabled {
        return;
    }
    // Uygulamalar panoya birkaç biçimi arka arkaya yazar; bitmesini bekle.
    std::thread::sleep(std::time::Duration::from_millis(120));
    let Some(c) = platform::clip_read() else { return };
    if c.private {
        return;
    }
    let source_path = c.owner.clone().unwrap_or_default();
    let now = chrono::Utc::now().timestamp_millis();
    let state = app.state::<AppState>();
    let mut h = state.clips.lock().unwrap();
    let item = if let Some(text) = c.text {
        let text: String = text.chars().take(20_000).collect();
        // Aynı metin zaten varsa en üste taşı
        if let Some(i) = h.items.iter().position(|x| x.kind == "text" && x.text == text) {
            let mut it = h.items.remove(i);
            it.time = now;
            h.items.insert(0, it);
            drop(h);
            let _ = app.emit("clip-changed", ());
            return;
        }
        Item {
            id: 0,
            kind: "text".into(),
            text,
            preview: String::new(),
            width: 0,
            height: 0,
            source: app_name(&source_path),
            source_path,
            time: now,
            pinned: false,
            dib: None,
        }
    } else if let (Some(dib), Some(thumb), Some((w, hgt))) = (c.dib, c.thumb, c.size) {
        if h.items.first().map(|x| x.kind == "image" && x.dib.as_deref() == Some(&dib[..])).unwrap_or(false) {
            return;
        }
        Item {
            id: 0,
            kind: "image".into(),
            text: String::new(),
            preview: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(thumb)),
            width: w,
            height: hgt,
            source: app_name(&source_path),
            source_path,
            time: now,
            pinned: false,
            dib: Some(dib),
        }
    } else {
        return;
    };
    h.next += 1;
    let mut item = item;
    item.id = h.next;
    h.items.insert(0, item);
    // Sınırı aş: sabitlenmemiş en eskileri sil; görseller bellekte büyük, en çok 12.
    let mut images = 0;
    let mut kept = 0;
    h.items.retain(|x| {
        if x.pinned {
            return true;
        }
        if x.kind == "image" {
            images += 1;
            if images > 12 {
                return false;
            }
        }
        kept += 1;
        kept <= max
    });
    drop(h);
    let _ = app.emit("clip-changed", ());
}

pub fn spawn(app: AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    platform::clip_listen(move || {
        let _ = tx.send(());
    });
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            // Arka arkaya gelen bildirimleri tek okumaya indir
            while rx.try_recv().is_ok() {}
            capture(&app);
        }
    });
}

#[tauri::command]
pub fn clip_list(app: AppHandle) -> Vec<Item> {
    let mut v = app.state::<AppState>().clips.lock().unwrap().items.clone();
    // Sabitlenenler üstte
    v.sort_by_key(|x| !x.pinned);
    v
}

/// Öğeyi tekrar panoya kopyalar ve en üste taşır.
#[tauri::command]
pub async fn clip_copy(app: AppHandle, id: u64) -> bool {
    run(move || {
        let state = app.state::<AppState>();
        let item = state.clips.lock().unwrap().items.iter().find(|x| x.id == id).cloned();
        let Some(item) = item else { return false };
        let ok = match (&item.kind[..], &item.dib) {
            ("image", Some(d)) => platform::clip_set_dib(d),
            _ => platform::clip_set_text(&item.text),
        };
        ok
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
pub fn clip_remove(app: AppHandle, id: u64) {
    app.state::<AppState>().clips.lock().unwrap().items.retain(|x| x.id != id);
    let _ = app.emit("clip-changed", ());
}

#[tauri::command]
pub fn clip_pin(app: AppHandle, id: u64, pinned: bool) {
    if let Some(x) = app.state::<AppState>().clips.lock().unwrap().items.iter_mut().find(|x| x.id == id) {
        x.pinned = pinned;
    }
    let _ = app.emit("clip-changed", ());
}

/// Sabitlenmemiş tüm öğeleri siler.
#[tauri::command]
pub fn clip_clear(app: AppHandle) {
    app.state::<AppState>().clips.lock().unwrap().items.retain(|x| x.pinned);
    let _ = app.emit("clip-changed", ());
}
