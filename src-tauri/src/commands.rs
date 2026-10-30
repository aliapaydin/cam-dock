//! Arayüzün (JS) çağırdığı komutlar.

use crate::config::{self, Config};
use crate::docks::{self, MonitorView};
use crate::platform::{self, AppWindow, MediaState, StartApp};
use crate::screentime::ScreenTime;
use crate::{dlog, AppState};
use base64::Engine;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, State, WebviewWindow};

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
    pub log_path: String,
}

fn catalog(state: &AppState, refresh: bool) -> Vec<platform::CatalogApp> {
    if !refresh {
        if let Some(c) = state.catalog.lock().unwrap().as_ref() {
            return c.clone();
        }
    }
    let c = platform::catalog_apps();
    *state.catalog.lock().unwrap() = Some(c.clone());
    c
}

fn is_uri(path: &str) -> bool {
    path.contains(':') && !path.contains(":\\")
}

/// Sabitlenen bir uygulama bu yolda yoksa (ör. Spotify/Telegram'ın Store sürümü
/// ya da farklı klasöre kurulmuş hali) Windows uygulama kataloğunda adıyla ya
/// da exe adıyla bulup yolu kalıcı olarak düzeltir.
fn fix_missing_pinned(state: &AppState) {
    let missing: Vec<usize> = {
        let cfg = state.cfg.lock().unwrap();
        cfg.pinned
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let path = config::expand_env(&p.path);
                !is_uri(&path) && !std::path::Path::new(&path).exists()
            })
            .map(|(i, _)| i)
            .collect()
    };
    if missing.is_empty() {
        return;
    }
    let cat = catalog(state, false);
    let mut cfg = state.cfg.lock().unwrap();
    let mut changed = false;
    for i in missing {
        let name = cfg.pinned[i].name.to_lowercase();
        let file = config::expand_env(&cfg.pinned[i].path)
            .rsplit('\\')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let hit = cat
            .iter()
            .find(|a| a.name.to_lowercase() == name)
            .or_else(|| (!file.is_empty()).then(|| cat.iter().find(|a| a.path.to_lowercase().ends_with(&format!("\\{file}")))).flatten())
            .or_else(|| cat.iter().find(|a| a.name.to_lowercase().starts_with(&name)));
        if let Some(a) = hit {
            dlog!("sabitleme düzeltildi: {} → {}", cfg.pinned[i].path, a.path);
            cfg.pinned[i].path = a.path.clone();
            changed = true;
        }
    }
    if changed {
        config::save(&cfg);
    }
}

#[tauri::command(async)]
pub fn get_config(state: State<'_, AppState>) -> ConfigView {
    fix_missing_pinned(&state);
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
        log_path: crate::log::path().display().to_string(),
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

#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn list_monitors(app: AppHandle) -> Vec<MonitorView> {
    docks::monitors(&app)
}

// ---------------------------------------------------------------------------
// Dock yerleşimi ve görünüm
// ---------------------------------------------------------------------------

/// Arayüz dock'un ölçüsünü (CSS pikseli) bildirir; pencereyi boyutlandırıp
/// kendi ekranının alt ortasına yerleştiririz. Kilitler yalnızca durum okunup
/// yazılırken tutulur; Windows çağrıları kilit dışında yapılır.
#[tauri::command(async)]
pub fn layout_dock(
    window: WebviewWindow,
    state: State<'_, AppState>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let (reserve, always, margin_css, radius_css) = {
        let cfg = state.cfg.lock().unwrap();
        (cfg.reserve_space, cfg.effective_visibility() == "always", cfg.bottom_margin, cfg.radius)
    };
    let _ = radius_css;
    let label = window.label().to_string();
    let Some((hwnd, mon, scale)) = state
        .docks
        .lock()
        .unwrap()
        .get(&label)
        .map(|d| (d.hwnd, d.mon_rect, d.scale))
    else {
        return Ok(());
    };
    let (mx, _my, mr, mb) = mon;
    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;
    let margin = (margin_css as f64 * scale).round() as i32;

    let bottom = if reserve && always {
        platform::reserve(hwnd, mon, h + margin * 2).3
    } else {
        platform::release(hwnd);
        mb
    };
    let x = mx + (mr - mx - w) / 2;
    let y = bottom - h - margin;

    let (first, snapshot) = {
        let mut docks = state.docks.lock().unwrap();
        let Some(d) = docks.get_mut(&label) else { return Ok(()) };
        let first = d.target.is_none();
        d.target = Some((x, y, w, h));
        if first && !d.fs_hidden {
            d.shown = true;
            d.last_inside = Instant::now();
        }
        (first, d.clone())
    };

    let place_y = if snapshot.shown && !first { y } else { mb + 40 };
    platform::set_bounds(hwnd, x, place_y, w.max(1), h.max(1));
    dlog!("{label}: yerleşim x={x} y={y} w={w} h={h} (alt sınır {bottom}, ölçek {scale})");

    if first && snapshot.shown {
        docks::slide_in(&snapshot);
        docks::emit_visibility(window.app_handle(), &label, true);
    }
    docks::apply_glass(window.app_handle(), &label);
    Ok(())
}

/// Koyu/açık temaya göre bu dock'un cam arka planını uygular.
#[tauri::command(async)]
pub fn set_theme(window: WebviewWindow, state: State<'_, AppState>, dark: bool) {
    if let Some(d) = state.docks.lock().unwrap().get_mut(window.label()) {
        d.dark = dark;
    }
    docks::apply_glass(window.app_handle(), window.label());
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

#[tauri::command(async)]
pub fn list_windows(state: State<'_, AppState>) -> WindowsView {
    WindowsView {
        windows: platform::list_windows(&dock_hwnds(&state)),
        active: state.last_fg.load(Ordering::SeqCst),
    }
}

#[tauri::command(async)]
pub fn activate_window(state: State<'_, AppState>, hwnd: isize) {
    platform::activate(hwnd, state.last_fg.load(Ordering::SeqCst));
}

/// Uygulama simgesine tıklama: öndeyse küçült, küçültülmüşse geri getir,
/// arkadaysa öne getir (pencereler Z sırasına göre).
#[tauri::command(async)]
pub fn activate_app(state: State<'_, AppState>, hwnds: Vec<isize>) {
    platform::activate_group(&hwnds, state.last_fg.load(Ordering::SeqCst));
}

/// Dock'taki kilit düğmesi: kilitli ↔ otomatik gizle.
#[tauri::command(async)]
pub fn toggle_lock(app: AppHandle) -> bool {
    let locked = {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        cfg.dock_locked = !cfg.dock_locked;
        config::save(&cfg);
        let now = Instant::now();
        for d in state.docks.lock().unwrap().values_mut() {
            d.last_inside = now;
        }
        cfg.dock_locked
    };
    dlog!("kilit {}", if locked { "kapatıldı (hep görünür)" } else { "açıldı (otomatik gizle)" });
    crate::emit_config(&app);
    locked
}

#[tauri::command(async)]
pub fn close_window(hwnd: isize) {
    platform::close(hwnd);
}

#[tauri::command(async)]
pub fn launch_app(path: String, args: Option<String>) -> bool {
    let path = config::expand_env(&path);
    if path.to_lowercase().starts_with("shell:") {
        // Store uygulamaları ve katalog öğeleri
        return platform::launch("explorer.exe", Some(&path));
    }
    platform::launch(&path, args.as_deref())
}

/// Bir uygulamanın ikonunu data URL olarak döndürür (önbellekli).
/// Sırayla dener: Store kimliği (AppsFolder) → exe'den çıkarma → kabuk ikonu →
/// pencerenin kendi ikonu. Store uygulamaları ve küçültülmüş pencereler için
/// ilk ve son yol gerekiyor.
#[tauri::command(async)]
pub fn app_icon(
    state: State<'_, AppState>,
    path: String,
    aumid: Option<String>,
    hwnd: Option<isize>,
) -> Option<String> {
    let path = config::expand_env(&path);
    let aumid = aumid.unwrap_or_default();
    let key = if aumid.is_empty() { path.to_lowercase() } else { format!("aumid:{}", aumid.to_lowercase()) };
    if let Some(hit) = state.icons.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let host = path.to_lowercase().ends_with("applicationframehost.exe");
    let png = (!aumid.is_empty())
        .then(|| platform::shell_icon_png(&format!("shell:AppsFolder\\{aumid}"), 64))
        .flatten()
        .or_else(|| (!host).then(|| platform::icon_png(&path, 64)).flatten())
        .or_else(|| (!host && !path.is_empty()).then(|| platform::shell_icon_png(&path, 64)).flatten())
        .or_else(|| hwnd.and_then(|h| platform::window_icon_png(h, 64)));
    let url = png.map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    // Başarısız sonucu yalnızca pencere ikonu da denendiyse önbelleğe al.
    if url.is_some() || hwnd.is_some() {
        state.icons.lock().unwrap().insert(key, url.clone());
    }
    if url.is_none() {
        dlog!("ikon bulunamadı: {path} {aumid}");
    }
    url
}

/// Windows uygulama kataloğu (Başlat → Tüm uygulamalar; Store dahil).
#[tauri::command(async)]
pub fn catalog_apps(state: State<'_, AppState>, refresh: Option<bool>) -> Vec<platform::CatalogApp> {
    catalog(&state, refresh.unwrap_or(false))
}

/// Dosya seçme penceresi; .lnk seçilirse hedef exe döner.
#[tauri::command(async)]
pub fn pick_app(window: WebviewWindow) -> Option<String> {
    #[cfg(windows)]
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    #[cfg(not(windows))]
    let owner = {
        let _ = &window;
        0
    };
    let path = platform::pick_app(owner)?;
    if path.to_lowercase().ends_with(".lnk") {
        if let Some(t) = platform::resolve_shortcut(&path) {
            return Some(t);
        }
    }
    Some(path)
}

#[tauri::command(async)]
pub fn sys_stats(state: State<'_, AppState>) -> platform::SysStats {
    *state.stats.lock().unwrap()
}

#[derive(Serialize)]
pub struct TrayView {
    pub name: String,
    pub exe: String,
    pub icon: Option<String>,
    pub promoted: bool,
}

/// Tepside simgesi olan ve çalışan uygulamalar (kendi tepsi panelimiz için).
#[tauri::command(async)]
pub fn tray_apps() -> Vec<TrayView> {
    platform::tray_apps()
        .into_iter()
        .map(|a| TrayView {
            icon: a.icon.map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))),
            name: a.name,
            exe: a.exe,
            promoted: a.promoted,
        })
        .collect()
}

/// Tüm medya oturumları.
#[tauri::command(async)]
pub fn media_list() -> Vec<MediaState> {
    platform::media_states()
}

#[tauri::command(async)]
pub fn media_control_id(id: String, action: String) -> bool {
    platform::media_control_id(&id, &action)
}

#[tauri::command(async)]
pub fn media_thumbnail_id(state: State<'_, AppState>, id: String, track_key: String) -> Option<String> {
    let key = format!("{id}\u{1}{track_key}");
    {
        let cache = state.thumb.lock().unwrap();
        if cache.0 == key {
            return cache.1.clone();
        }
    }
    let url = platform::media_thumbnail_id(&id).map(|bytes| {
        let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else { "image/jpeg" };
        format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
    });
    *state.thumb.lock().unwrap() = (key, url.clone());
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

/// Piyasa verisi ve RSS için basit HTTPS GET. Tarayıcı içinden yapılan
/// istekler CORS'a takıldığı için Rust tarafında indiriyoruz.
#[tauri::command(async)]
pub fn fetch_text(url: String) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("yalnızca https adreslerine izin var".into());
    }
    let resp = ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) CamDock/0.3")
        .set("Accept", "application/json, application/rss+xml, application/xml, text/xml, */*")
        .timeout(std::time::Duration::from_secs(12))
        .call()
        .map_err(err)?;
    resp.into_string().map_err(err)
}

#[tauri::command(async)]
pub fn screen_time(state: State<'_, AppState>) -> ScreenTime {
    state.screen.lock().unwrap().snapshot()
}

#[tauri::command(async)]
pub fn open_start() {
    platform::open_start();
}

/// Dock'taki sistem düğmeleri.
#[tauri::command(async)]
pub fn system_action(action: String) {
    match action.as_str() {
        "quick" => platform::win_combo('A'),
        "notifications" => platform::win_combo('N'),
        "taskview" => platform::win_combo_tab(),
        _ => {}
    }
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
