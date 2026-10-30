//! Arayüzün (JS) çağırdığı komutlar.

use crate::config::{self, Config};
use crate::docks::{self, MonitorView};
use crate::platform::{self, AppWindow, MediaState, StartApp};
use crate::screentime::ScreenTime;
use crate::AppState;
use base64::Engine;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn dock_hwnds(state: &AppState) -> Vec<isize> {
    state.docks.lock().unwrap().values().map(|d| d.hwnd).collect()
}

// ---------------------------------------------------------------------------
// Ayarlar
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct PinnedView {
    pub name: String,
    /// Ortam değişkenleri açılmış yol
    pub path: String,
    pub args: Option<String>,
    pub exists: bool,
}

#[derive(Serialize)]
pub struct ConfigView {
    #[serde(flatten)]
    pub cfg: Config,
    pub pinned_resolved: Vec<PinnedView>,
    pub config_path: String,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> ConfigView {
    let cfg = state.cfg.lock().unwrap().clone();
    let pinned_resolved = cfg
        .pinned
        .iter()
        .map(|p| {
            let path = config::expand_env(&p.path);
            let is_uri = path.contains(':') && !path.contains(":\\");
            PinnedView {
                name: p.name.clone(),
                exists: is_uri || std::path::Path::new(&path).exists(),
                path,
                args: p.args.clone(),
            }
        })
        .collect();
    ConfigView {
        cfg,
        pinned_resolved,
        config_path: config::config_path().display().to_string(),
    }
}

/// Ayarlar penceresi tüm ayarı gönderir; kaydedip hemen uygularız.
#[tauri::command]
pub async fn save_config(app: AppHandle, cfg: Config) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        *state.cfg.lock().unwrap() = cfg.clone();
        state.icons.lock().unwrap().clear();
    }
    config::save(&cfg);
    // Ekran seçimi değiştiyse dock'ları aç/kapat, taskbar'ı güncelle.
    docks::sync(&app);
    crate::emit_config(&app);
    Ok(())
}

#[derive(Serialize)]
pub struct DockInfo {
    pub label: String,
    pub monitor: String,
    pub primary: bool,
}

#[tauri::command]
pub fn dock_info(window: WebviewWindow, state: State<'_, AppState>) -> Option<DockInfo> {
    state
        .docks
        .lock()
        .unwrap()
        .get(window.label())
        .map(|d| DockInfo {
            label: d.label.clone(),
            monitor: d.monitor.clone(),
            primary: d.primary,
        })
}

#[tauri::command]
pub fn list_monitors(app: AppHandle) -> Vec<MonitorView> {
    docks::monitors(&app)
}

// ---------------------------------------------------------------------------
// Dock yerleşimi ve görünüm
// ---------------------------------------------------------------------------

/// Arayüz dock'un ölçüsünü (CSS pikseli) bildirir; pencereyi boyutlandırıp
/// kendi ekranının alt ortasına yerleştiririz.
#[tauri::command]
pub fn layout_dock(
    window: WebviewWindow,
    state: State<'_, AppState>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let cfg = state.cfg.lock().unwrap().clone();
    let mut docks = state.docks.lock().unwrap();
    let Some(d) = docks.get_mut(window.label()) else {
        return Ok(());
    };
    let (mx, my, mr, mb) = d.mon_rect;
    let mw = mr - mx;
    let scale = d.scale;
    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;
    let margin = (cfg.bottom_margin as f64 * scale).round() as i32;

    let mut bottom = mb;
    if cfg.reserve_space && cfg.dock_visibility == "always" {
        bottom = platform::reserve(d.hwnd, d.mon_rect, h + margin * 2).3;
    } else {
        platform::release(d.hwnd);
    }
    let _ = my;

    let x = mx + (mw - w) / 2;
    let y = bottom - h - margin;
    let first = d.target.is_none();
    d.target = Some((x, y, w, h));

    // Önce konum (farklı DPI'lı ekrana geçiş boyutu değiştirebilir), sonra boyut.
    if d.shown || first {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(err)?;
    }
    window
        .set_size(PhysicalSize::new(w.max(1) as u32, h.max(1) as u32))
        .map_err(err)?;
    platform::round_region(d.hwnd, w, h, (cfg.radius as f64 * scale).round() as i32);

    if first && !d.fs_hidden {
        platform::show_no_activate(d.hwnd);
        d.shown = true;
        d.last_inside = Instant::now();
    }
    Ok(())
}

/// Koyu/açık temaya göre bu dock'un cam arka planını uygular.
#[tauri::command]
pub fn set_theme(window: WebviewWindow, state: State<'_, AppState>, dark: bool) {
    let mode = state.cfg.lock().unwrap().backdrop.clone();
    if let Some(d) = state.docks.lock().unwrap().get(window.label()) {
        platform::apply_backdrop(d.hwnd, &mode, dark);
    }
}

// ---------------------------------------------------------------------------
// Uygulamalar ve pencereler
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct WindowsView {
    pub windows: Vec<AppWindow>,
    /// Kullanıcının şu an çalıştığı (dock'tan önceki) pencere
    pub active: isize,
}

#[tauri::command]
pub fn list_windows(state: State<'_, AppState>) -> WindowsView {
    WindowsView {
        windows: platform::list_windows(&dock_hwnds(&state)),
        active: state.last_fg.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub fn activate_window(state: State<'_, AppState>, hwnd: isize) {
    platform::activate(hwnd, state.last_fg.load(Ordering::SeqCst));
}

#[tauri::command]
pub fn close_window(hwnd: isize) {
    platform::close(hwnd);
}

#[tauri::command]
pub fn launch_app(path: String, args: Option<String>) -> bool {
    platform::launch(&config::expand_env(&path), args.as_deref())
}

/// Bir exe'nin ikonunu data URL olarak döndürür (önbellekli).
#[tauri::command(async)]
pub fn app_icon(state: State<'_, AppState>, path: String) -> Option<String> {
    let path = config::expand_env(&path);
    let key = path.to_lowercase();
    if let Some(hit) = state.icons.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let url = platform::icon_png(&path, 64).map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    state.icons.lock().unwrap().insert(key, url.clone());
    url
}

/// Başlat menüsündeki uygulamalar (ayarlardan sabitlemek için).
#[tauri::command(async)]
pub fn start_menu_apps() -> Vec<StartApp> {
    platform::start_menu_apps()
}

/// .lnk ise hedef exe'yi döndürür, değilse yolu olduğu gibi.
#[tauri::command(async)]
pub fn resolve_shortcut(path: String) -> String {
    let expanded = config::expand_env(&path);
    if expanded.to_lowercase().ends_with(".lnk") {
        if let Some(t) = platform::resolve_shortcut(&expanded) {
            return t;
        }
    }
    path
}

// ---------------------------------------------------------------------------
// Medya, ekran süresi, sistem
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn media_state() -> MediaState {
    platform::media_state()
}

/// Kapak görseli; aynı parça için tekrar okumayız.
#[tauri::command(async)]
pub fn media_thumbnail(state: State<'_, AppState>, track_key: String) -> Option<String> {
    {
        let cache = state.thumb.lock().unwrap();
        if cache.0 == track_key {
            return cache.1.clone();
        }
    }
    let url = platform::media_thumbnail().map(|bytes| {
        let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            "image/png"
        } else {
            "image/jpeg"
        };
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    });
    *state.thumb.lock().unwrap() = (track_key, url.clone());
    url
}

#[tauri::command(async)]
pub fn media_control(action: String) -> bool {
    platform::media_control(&action)
}

#[tauri::command]
pub fn screen_time(state: State<'_, AppState>) -> ScreenTime {
    state.screen.lock().unwrap().snapshot()
}

#[tauri::command]
pub fn open_start() {
    platform::open_start();
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    docks::open_settings(&app).map_err(err)
}

/// Dock'a sağ tıklayınca açılan menü.
#[tauri::command]
pub fn show_menu(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    let app = window.app_handle();
    let theme_now = state.cfg.lock().unwrap().theme.clone();
    let check = |id: &str, text: &str, on: bool| {
        CheckMenuItemBuilder::with_id(id, text)
            .checked(on)
            .build(app)
            .map_err(err)
    };
    let theme = SubmenuBuilder::new(app, "Tema")
        .item(&check("theme:system", "Sistemle aynı", theme_now == "system")?)
        .item(&check("theme:dark", "Koyu", theme_now == "dark")?)
        .item(&check("theme:light", "Açık", theme_now == "light")?)
        .build()
        .map_err(err)?;
    let menu = MenuBuilder::new(app)
        .text("settings", "Ayarlar…")
        .item(&theme)
        .separator()
        .text("quit", "Çık (taskbar geri gelir)")
        .build()
        .map_err(err)?;
    window.popup_menu(&menu).map_err(err)
}
