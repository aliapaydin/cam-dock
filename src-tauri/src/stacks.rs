//! Klasör yığınları: klasördeki son dosyalar ve küçük resimleri.

use crate::commands::run;
use crate::{config, dlog, platform, AppState};
use base64::Engine as _;
use serde::Serialize;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    /// Unix ms
    pub modified: i64,
    pub size: u64,
}

const SKIP_EXT: [&str; 7] = ["crdownload", "part", "partial", "download", "opdownload", "tmp", "ini"];

#[tauri::command]
pub async fn stack_list(path: String, sort: Option<String>, limit: Option<usize>) -> Vec<Entry> {
    run(move || {
        let dir = config::expand_env(&path);
        let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
        let mut v: Vec<Entry> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name.starts_with("~$") {
                    return None;
                }
                let lower = name.to_lowercase();
                if SKIP_EXT.iter().any(|x| lower.ends_with(&format!(".{x}"))) {
                    return None;
                }
                let meta = e.metadata().ok()?;
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    // Gizli ve sistem dosyaları
                    if meta.file_attributes() & (0x2 | 0x4) != 0 {
                        return None;
                    }
                }
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                Some(Entry { name, path: e.path().to_string_lossy().to_string(), dir: meta.is_dir(), modified, size: meta.len() })
            })
            .collect();
        if sort.as_deref() == Some("name") {
            v.sort_by_key(|e| e.name.to_lowercase());
        } else {
            v.sort_by(|a, b| b.modified.cmp(&a.modified));
        }
        v.truncate(limit.unwrap_or(60).min(200));
        v
    })
    .await
    .unwrap_or_default()
}

/// Dosyanın küçük resmi (görsel/video/PDF) ya da simgesi, data: URL.
#[tauri::command]
pub async fn file_thumb(app: AppHandle, path: String, size: Option<i32>) -> Option<String> {
    run(move || {
        let size = size.unwrap_or(96).clamp(16, 256);
        let key = format!("thumb:{size}:{path}");
        let state = app.state::<AppState>();
        if let Some(hit) = state.icons.lock().unwrap().get(&key) {
            return hit.clone();
        }
        let url = platform::thumbnail_png(&path, size)
            .map(|png| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)));
        let mut cache = state.icons.lock().unwrap();
        if cache.len() > 600 {
            cache.retain(|k, _| !k.starts_with("thumb:"));
        }
        cache.insert(key, url.clone());
        url
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn pick_folder(window: WebviewWindow) -> Option<String> {
    #[cfg(windows)]
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    #[cfg(not(windows))]
    let owner = {
        let _ = &window;
        0
    };
    run(move || platform::pick_folder(owner)).await.unwrap_or_default()
}

/// Yığın ekler (aynı klasör varsa eklemez). Dönüş: eklendi mi.
pub fn add(app: &AppHandle, path: &str) -> bool {
    let p = path.trim().trim_matches('"').trim_end_matches('\\').to_string();
    if p.is_empty() || !std::path::Path::new(&config::expand_env(&p)).is_dir() {
        return false;
    }
    let name = p.rsplit('\\').next().unwrap_or(&p).to_string();
    let state = app.state::<AppState>();
    let mut cfg = state.cfg.lock().unwrap();
    if cfg.stacks.iter().any(|s| config::expand_env(&s.path).eq_ignore_ascii_case(&config::expand_env(&p))) {
        return false;
    }
    cfg.stacks.push(config::Stack { name, path: p, ..Default::default() });
    if !cfg.widgets.iter().any(|w| w == "stacks") {
        cfg.widgets.insert(0, "stacks".into());
    }
    config::save(&cfg);
    dlog!("klasör yığını eklendi");
    true
}

/// Yığının son kullanılan görünümünü ve sıralamasını kaydeder (bir sonraki
/// açılışta aynı görünümle açılsın).
#[tauri::command]
pub fn stack_prefs(app: AppHandle, path: String, view: Option<String>, sort: Option<String>) {
    let cfg = {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        let Some(st) = cfg.stacks.iter_mut().find(|s| s.path.eq_ignore_ascii_case(&path)) else { return };
        if let Some(v) = view.filter(|v| v == "grid" || v == "fan") {
            st.view = v;
        }
        if let Some(s) = sort.filter(|s| s == "date" || s == "name") {
            st.sort = s;
        }
        cfg.clone()
    };
    config::save(&cfg);
    crate::emit_config(&app);
}
