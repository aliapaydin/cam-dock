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

/// Bloklayan işleri Tauri'nin komut havuzundan ayrı iş parçacıklarında çalıştır.
/// (Havuz küçük ve sabit; bloklayan bir komut tüm arayüzü kilitleyebiliyordu.)
pub async fn run<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

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
/// "...\WindowsApps\5319275A.WhatsAppDesktop_2.2540.6.0_x64__cv1g1gvanyjgm\WhatsApp.Root.exe"
/// → paket ailesi "5319275a.whatsappdesktop_cv1g1gvanyjgm"
fn package_family(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    let rest = &lower[lower.find("\\windowsapps\\")? + 13..];
    let full = rest.split('\\').next()?;
    let parts: Vec<&str> = full.split('_').collect();
    if parts.len() < 5 {
        return None;
    }
    Some(format!("{}_{}", parts[0], parts[parts.len() - 1]))
}

/// Eski sürümlerde Store uygulamaları WindowsApps içindeki exe yoluyla
/// sabitleniyordu. Bazı uygulamaların (WhatsApp) exe'sinde simge yok ve
/// uygulama kapalıyken genel pencere simgesi görünüyordu; ayrıca uygulama
/// güncellenince sürüm klasörü değişip yol bozuluyordu. Uygulama kimliğine çevir.
fn fix_store_pins(state: &AppState) {
    let has = state.cfg.lock().unwrap().pinned.iter().any(|p| package_family(&p.path).is_some());
    if !has {
        return;
    }
    let cat = catalog(state, false);
    let mut cfg = state.cfg.lock().unwrap();
    let mut changed = false;
    for p in cfg.pinned.iter_mut() {
        let Some(fam) = package_family(&config::expand_env(&p.path)) else { continue };
        let prefix = format!("{fam}!");
        let mut hits: Vec<&platform::CatalogApp> =
            cat.iter().filter(|a| a.aumid.to_lowercase().starts_with(&prefix)).collect();
        // Birden çok giriş noktası varsa ana uygulama ("!App") önce
        hits.sort_by_key(|a| !a.aumid.to_lowercase().ends_with("!app"));
        if let Some(a) = hits.first() {
            dlog!("Store uygulaması kimliğiyle sabitlendi: {} → {}", p.path, a.aumid);
            p.path = format!("shell:AppsFolder\\{}", a.aumid);
            changed = true;
        }
    }
    if changed {
        config::save(&cfg);
        state.icons.lock().unwrap().clear();
    }
}

fn fix_missing_pinned(state: &AppState) {
    fix_store_pins(state);
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

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<ConfigView, String> {
    run(move || {
        let state = app.state::<AppState>();

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

    }).await
}

/// Ayarlar penceresi tüm ayarı gönderir; kaydedip hemen uygularız.
#[tauri::command]
pub async fn save_config(app: AppHandle, cfg: Config) -> Result<(), String> {
    let mut cfg = cfg;
    let telemetry_changed = {
        let state = app.state::<AppState>();
        let mut cur = state.cfg.lock().unwrap();
        // Kurulum kimliği yalnızca Rust tarafında değişir (pencerelerdeki eski kopya ezmesin).
        cfg.telemetry.install_id = cur.telemetry.install_id.clone();
        // Simge sırası ve kilidi dock'tan ayrı komutlarla değişir.
        cfg.app_order = cur.app_order.clone();
        cfg.apps_locked = cur.apps_locked;
        let changed = (cur.telemetry.enabled != cfg.telemetry.enabled).then_some(cfg.telemetry.enabled);
        *cur = cfg.clone();
        changed
    };
    if let Some(on) = telemetry_changed {
        crate::telemetry::on_toggle(&app, on);
    }
    {
        let state = app.state::<AppState>();
        state.icons.lock().unwrap().clear();
    }
    config::save(&cfg);
    if cfg.autostart != platform::autostart_get() {
        apply_autostart(cfg.autostart);
    }
    crate::pinreq::sync_menu(cfg.explorer_menu);
    // Ekran seçimi değiştiyse dock'ları aç/kapat, taskbar'ı güncelle
    // (pencere oluşturma bloklayıcı: komut havuzunun dışında).
    run(move || {
        docks::sync(&app);
        crate::emit_config(&app);
        docks::refresh_theme(&app);
    })
    .await
}

#[derive(Serialize)]
pub struct DockInfo {
    pub label: String,
    pub monitor: String,
    pub primary: bool,
}

#[tauri::command]
pub async fn dock_info(app: AppHandle, window: WebviewWindow) -> Option<DockInfo> {
    run(move || {
        let state = app.state::<AppState>();

    let info = state
        .docks
        .lock()
        .unwrap()
        .get(window.label())
        .map(|d| DockInfo {
            label: d.label.clone(),
            monitor: d.monitor.clone(),
            primary: d.primary,
        });
    info
    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn list_monitors(app: AppHandle) -> Vec<MonitorView> {
    run(move || {
        let _ = &app;

    docks::monitors(&app)

    }).await.unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Dock yerleşimi ve görünüm
// ---------------------------------------------------------------------------

/// Arayüz dock'un ölçüsünü (CSS pikseli) bildirir; pencereyi boyutlandırıp
/// kendi ekranının alt ortasına yerleştiririz. Kilitler yalnızca durum okunup
/// yazılırken tutulur; Windows çağrıları kilit dışında yapılır.
#[tauri::command]
pub async fn layout_dock(app: AppHandle, window: WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    match run(move || {
        let state = app.state::<AppState>();

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

    let place_y = if snapshot.shown && !first { y } else { mb };
    docks::place(hwnd, x, place_y, w, h);
    if first {
        dlog!("{label}: ilk yerleşim x={x} y={y} w={w} h={h} (alt sınır {bottom}, ölçek {scale})");
    }

    if first && snapshot.shown {
        docks::slide_in(&snapshot);
        docks::emit_visibility(window.app_handle(), &label, true);
    }
    docks::apply_glass(window.app_handle(), &label);
    Ok(())

    }).await {
        Ok(r) => r,
        Err(e) => Err(e),
    }
}

/// Koyu/açık temaya göre bu dock'un cam arka planını uygular.
#[tauri::command]
pub async fn set_theme(app: AppHandle, window: WebviewWindow, dark: bool, tint: Option<[u8; 3]>) {
    let _ = run(move || {
        let state = app.state::<AppState>();

    if let Some(d) = state.docks.lock().unwrap().get_mut(window.label()) {
        d.dark = dark;
        d.tint = tint.map(|t| (t[0], t[1], t[2]));
    }
    docks::apply_glass(window.app_handle(), window.label());

    }).await;
}

// ---------------------------------------------------------------------------
// Uygulamalar ve pencereler
// ---------------------------------------------------------------------------

#[derive(Serialize, Default)]
pub struct WindowsView {
    pub windows: Vec<AppWindow>,
    /// Kullanıcının şu an çalıştığı (dock'tan önceki) pencere
    pub active: isize,
}

#[tauri::command]
pub async fn list_windows(app: AppHandle) -> WindowsView {
    run(move || {
        let state = app.state::<AppState>();

    WindowsView {
        windows: platform::list_windows(&dock_hwnds(&state)),
        active: state.last_fg.load(Ordering::SeqCst),
    }

    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn activate_window(app: AppHandle, hwnd: isize) {
    let _ = run(move || {
        let state = app.state::<AppState>();

    platform::activate(hwnd, state.last_fg.load(Ordering::SeqCst));

    }).await;
}

/// Uygulama simgesine tıklama: öndeyse küçült, küçültülmüşse geri getir,
/// arkadaysa öne getir (pencereler Z sırasına göre).
#[tauri::command]
pub async fn activate_app(app: AppHandle, hwnds: Vec<isize>) {
    let _ = run(move || {
        let state = app.state::<AppState>();

    platform::activate_group(&hwnds, state.last_fg.load(Ordering::SeqCst));

    }).await;
}

/// Dock'taki kilit düğmesi: kilitli ↔ otomatik gizle.
#[tauri::command]
pub async fn toggle_lock(app: AppHandle) -> bool {
    run(move || {
        let _ = &app;

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

    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn close_window(app: AppHandle, hwnd: isize) {
    let _ = run(move || {
        let _ = &app;

    platform::close(hwnd);

    }).await;
}

#[tauri::command]
pub async fn launch_app(app: AppHandle, path: String, args: Option<String>) -> bool {
    run(move || {
        let _ = &app;

    let path = config::expand_env(&path);
    if path.to_lowercase().starts_with("shell:") {
        // Store uygulamaları ve katalog öğeleri
        return platform::launch("explorer.exe", Some(&path));
    }
    platform::launch(&path, args.as_deref())

    }).await.unwrap_or_default()
}

/// Bir uygulamanın ikonunu data URL olarak döndürür (önbellekli).
/// Sırayla dener: Store kimliği (AppsFolder) → exe'den çıkarma → kabuk ikonu →
/// pencerenin kendi ikonu. Store uygulamaları ve küçültülmüş pencereler için
/// ilk ve son yol gerekiyor.
#[tauri::command]
pub async fn app_icon(app: AppHandle, path: String, aumid: Option<String>, hwnd: Option<isize>) -> Option<String> {
    run(move || {
        let state = app.state::<AppState>();

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

    }).await.unwrap_or_default()
}

/// Windows uygulama kataloğu (Başlat → Tüm uygulamalar; Store dahil).
#[tauri::command]
pub async fn catalog_apps(app: AppHandle, refresh: Option<bool>) -> Vec<platform::CatalogApp> {
    run(move || {
        let state = app.state::<AppState>();

    catalog(&state, refresh.unwrap_or(false))

    }).await.unwrap_or_default()
}

/// Dosya seçme penceresi; .lnk seçilirse hedef exe döner.
#[tauri::command]
pub async fn pick_app(app: AppHandle, window: WebviewWindow) -> Option<String> {
    run(move || {
        let _ = &app;

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

    }).await.unwrap_or_default()
}

/// Windows koyu temada mı? (Tema "Sistemle aynı" iken kullanılır.)
#[tauri::command]
pub fn system_theme() -> bool {
    !platform::light_theme().unwrap_or(false)
}

#[tauri::command]
pub async fn sys_stats(app: AppHandle) -> platform::SysStats {
    run(move || {
        let state = app.state::<AppState>();

    let st = *state.stats.lock().unwrap();
    st

    }).await.unwrap_or_default()
}

#[derive(Serialize)]
pub struct TrayView {
    pub name: String,
    pub exe: String,
    pub icon: Option<String>,
    pub promoted: bool,
}

/// Tepside simgesi olan ve çalışan uygulamalar (kendi tepsi panelimiz için).
#[tauri::command]
pub async fn tray_apps(app: AppHandle) -> Vec<TrayView> {
    run(move || {
        let _ = &app;

    platform::tray_apps()
        .into_iter()
        .map(|a| TrayView {
            icon: a.icon.map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))),
            name: a.name,
            exe: a.exe,
            promoted: a.promoted,
        })
        .collect()

    }).await.unwrap_or_default()
}

/// Tüm medya oturumları.
#[tauri::command]
pub async fn media_list(app: AppHandle) -> Vec<MediaState> {
    run(move || {
        let _ = &app;

    platform::media_states()

    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn media_control_id(app: AppHandle, id: String, action: String) -> bool {
    run(move || {
        let _ = &app;

    platform::media_control_id(&id, &action)

    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn media_thumbnail_id(app: AppHandle, id: String, track_key: String) -> Option<String> {
    run(move || {
        let state = app.state::<AppState>();

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

    }).await.unwrap_or_default()
}

/// Başlat menüsündeki uygulamalar (ayarlardan sabitlemek için).
#[tauri::command]
pub async fn start_menu_apps(app: AppHandle) -> Vec<StartApp> {
    run(move || {
        let _ = &app;

    platform::start_menu_apps()

    }).await.unwrap_or_default()
}

/// .lnk ise hedef exe'yi döndürür, değilse yolu olduğu gibi.
#[tauri::command]
pub async fn resolve_shortcut(app: AppHandle, path: String) -> String {
    run(move || {
        let _ = &app;

    let expanded = config::expand_env(&path);
    if expanded.to_lowercase().ends_with(".lnk") {
        if let Some(t) = platform::resolve_shortcut(&expanded) {
            return t;
        }
    }
    path

    }).await.unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Medya, ekran süresi, sistem
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn media_state(app: AppHandle) -> MediaState {
    run(move || {
        let _ = &app;

    platform::media_state()

    }).await.unwrap_or_default()
}

/// Kapak görseli; aynı parça için tekrar okumayız.
#[tauri::command]
pub async fn media_thumbnail(app: AppHandle, track_key: String) -> Option<String> {
    run(move || {
        let state = app.state::<AppState>();

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

    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn media_control(app: AppHandle, action: String) -> bool {
    run(move || {
        let _ = &app;

    platform::media_control(&action)

    }).await.unwrap_or_default()
}

/// Piyasa verisi ve RSS için basit HTTPS GET. Tarayıcı içinden yapılan
/// istekler CORS'a takıldığı için Rust tarafında indiriyoruz.
#[tauri::command]
pub async fn fetch_text(app: AppHandle, url: String) -> Result<String, String> {
    match run(move || {
        let _ = &app;

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

    }).await {
        Ok(r) => r,
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn screen_time(app: AppHandle) -> ScreenTime {
    run(move || {
        let state = app.state::<AppState>();

    let snap = state.screen.lock().unwrap().snapshot();
    snap

    }).await.unwrap_or_default()
}

#[derive(Serialize, Default)]
pub struct UsageData {
    pub screen: ScreenTime,
    pub now: platform::SysStats,
    pub history: Vec<platform::SysStats>,
    pub info: platform::SysInfo,
}

/// Kullanım paneli: ekran süresi, uygulama dağılımı, kaynak geçmişi, sistem bilgisi.
#[tauri::command]
pub async fn usage_data(app: AppHandle) -> UsageData {
    run(move || {
        let state = app.state::<AppState>();
        let screen = state.screen.lock().unwrap().snapshot();
        let now = *state.stats.lock().unwrap();
        let history = state.stats_hist.lock().unwrap().iter().copied().collect();
        UsageData { screen, now, history, info: platform::sys_info() }
    })
    .await
    .unwrap_or_default()
}

/// Tüm pencerelerin kullanacağı tema (dinamik mod ve Windows teması dahil).
#[tauri::command]
pub fn theme_dark(app: AppHandle) -> bool {
    let state = app.state::<AppState>();
    let sys = *state.system_light.lock().unwrap();
    let sys = sys.or_else(platform::light_theme);
    let cfg = state.cfg.lock().unwrap().clone();
    crate::theme::effective_dark(&cfg, sys)
}

/// Dinamik modda gün doğumu/batımı (dakika) — ayarlarda bilgi olarak gösterilir.
#[tauri::command]
pub fn sun_times(app: AppHandle) -> Option<(f64, f64)> {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().clone();
    crate::theme::sun_times(cfg.weather.lat, cfg.weather.lon)
}

#[derive(Serialize, Default)]
pub struct WallpaperView {
    /// Dosya yolu + değişiklik zamanı + boyut: değişmediyse görüntü tekrar gönderilmez
    pub sig: String,
    /// data: URL (dosya duvar kâğıdı)
    pub image: Option<String>,
    /// Düz renk arka plan
    pub solid: Option<[u8; 3]>,
}

/// Bu pencerenin bulunduğu ekranın duvar kâğıdı. `known` imzası aynıysa
/// görüntü gönderilmez.
#[tauri::command]
pub async fn wallpaper(app: AppHandle, window: WebviewWindow, known: Option<String>) -> WallpaperView {
    let label = window.label().to_string();
    run(move || {
        let state = app.state::<AppState>();
        let dock = label.strip_prefix("pop-").unwrap_or(&label).to_string();
        let rect = {
            let docks = state.docks.lock().unwrap();
            docks
                .get(&dock)
                .map(|d| d.mon_rect)
                .or_else(|| docks.values().find(|d| d.primary).map(|d| d.mon_rect))
                .or_else(|| docks.values().next().map(|d| d.mon_rect))
        };
        let Some(rect) = rect else { return WallpaperView::default() };
        match platform::wallpaper(rect) {
            Some(platform::Wall::Solid(r, g, b)) => WallpaperView {
                sig: format!("solid:{r},{g},{b}"),
                image: None,
                solid: Some([r, g, b]),
            },
            Some(platform::Wall::File(path)) => {
                let meta = std::fs::metadata(&path).ok();
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let sig = format!("{path}|{mtime}|{}", meta.map(|m| m.len()).unwrap_or(0));
                if known.as_deref() == Some(sig.as_str()) {
                    return WallpaperView { sig, image: None, solid: None };
                }
                let Ok(bytes) = std::fs::read(&path) else { return WallpaperView::default() };
                let mime = match bytes.get(..4) {
                    Some([0x89, b'P', b'N', b'G']) => "image/png",
                    Some([b'B', b'M', ..]) => "image/bmp",
                    Some([b'R', b'I', b'F', b'F']) => "image/webp",
                    Some([b'G', b'I', b'F', _]) => "image/gif",
                    _ => "image/jpeg",
                };
                use base64::Engine as _;
                let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                WallpaperView { sig, image: Some(format!("data:{mime};base64,{data}")), solid: None }
            }
            None => WallpaperView::default(),
        }
    })
    .await
    .unwrap_or_default()
}

/// Otomatik bulunan konumu kaydeder (hava durumu ve gün doğumu/batımı için).
#[tauri::command]
pub async fn set_weather_location(app: AppHandle, city: String, lat: f64, lon: f64) {
    let _ = run(move || {
        let state = app.state::<AppState>();
        let cfg = {
            let mut cfg = state.cfg.lock().unwrap();
            if !cfg.weather.auto_location {
                return;
            }
            cfg.weather.city = city.clone();
            cfg.weather.lat = lat;
            cfg.weather.lon = lon;
            cfg.clone()
        };
        config::save(&cfg);
        let _ = tauri::Emitter::emit(&app, "location-changed", serde_json::json!({ "city": city, "lat": lat, "lon": lon }));
        docks::refresh_theme(&app);
    })
    .await;
}

#[tauri::command]
pub fn autostart_state() -> bool {
    platform::autostart_get()
}

/// Ayardaki "Windows ile başlat" seçimini kayıt defterine uygular.
/// Windows ile başlat: önce oturum açma görevi (Windows'un başlangıç
/// gecikmesine takılmaz), olmazsa Run anahtarı.
pub fn apply_autostart(enabled: bool) {
    let exe = std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string());
    if !enabled {
        platform::task_delete();
        let ok = platform::autostart_set(None);
        crate::dlog!("Windows ile başlat: kapalı ({})", if ok { "uygulandı" } else { "UYGULANAMADI" });
        return;
    }
    let Some(exe) = exe else { return };
    if platform::task_create(&exe) {
        // Görev varken Run kaydı ikinci kez başlatmaya çalışmasın
        platform::autostart_set(None);
        crate::dlog!("Windows ile başlat: oturum açma görevi kuruldu");
    } else {
        let ok = platform::autostart_set(Some(&exe));
        crate::dlog!("Windows ile başlat: görev kurulamadı, başlangıç uygulamalarına eklendi ({})", if ok { "tamam" } else { "BAŞARISIZ" });
    }
}

#[tauri::command]
pub async fn open_start(app: AppHandle) {
    let _ = run(move || {
        let _ = &app;

    platform::open_start();

    }).await;
}

/// Dock'taki sistem düğmeleri.
#[tauri::command]
pub async fn system_action(app: AppHandle, action: String) {
    let _ = run(move || {
        let _ = &app;

    match action.as_str() {
        "quick" => platform::win_combo('A'),
        "notifications" => platform::win_combo('N'),
        "taskview" => platform::win_combo_tab(),
        _ => {}
    }

    }).await;
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    match run(move || docks::open_settings(&app).map_err(err)).await {
        Ok(r) => r,
        Err(e) => Err(e),
    }
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
    let settings_sub = SubmenuBuilder::new(app, "Windows ayarları")
        .text("sys:ms-settings:display", "Ekran")
        .text("sys:ms-settings:sound", "Ses")
        .text("sys:ms-settings:network", "Ağ ve İnternet")
        .text("sys:ms-settings:bluetooth", "Bluetooth ve cihazlar")
        .text("sys:ms-settings:powersleep", "Güç ve pil")
        .text("sys:ms-settings:personalization", "Kişiselleştirme")
        .text("sys:ms-settings:appsfeatures", "Yüklü uygulamalar")
        .text("sys:ms-settings:windowsupdate", "Windows Update")
        .build()
        .map_err(err)?;
    let tools = SubmenuBuilder::new(app, "Yönetim araçları")
        .text("sys:devmgmt.msc", "Aygıt Yöneticisi")
        .text("sys:diskmgmt.msc", "Disk Yönetimi")
        .text("sys:compmgmt.msc", "Bilgisayar Yönetimi")
        .text("sys:services.msc", "Hizmetler")
        .text("sys:eventvwr.msc", "Olay Görüntüleyicisi")
        .text("sys:control.exe", "Denetim Masası")
        .text("sys:msinfo32.exe", "Sistem Bilgisi")
        .build()
        .map_err(err)?;
    let menu = MenuBuilder::new(app)
        .text("sys:ms-settings:", "Ayarlar")
        .text("sys:ms-settings:about", "Sistem özellikleri")
        .text("sys:taskmgr.exe", "Görev Yöneticisi")
        .text("sys:explorer.exe", "Dosya Gezgini")
        .text("sys:terminal", "Terminal")
        .item(&settings_sub)
        .item(&tools)
        .separator()
        .text("sys:run", "Çalıştır…")
        .text("sys:desktop", "Masaüstünü göster")
        .separator()
        .text("apps:lock", if state.cfg.lock().unwrap().apps_locked { "Simge düzeninin kilidini aç" } else { "Simge düzenini kaydet ve kilitle" })
        .text("settings", "Cam Dock ayarları…")
        .item(&theme)
        .text("quit", "Çık (taskbar geri gelir)")
        .build()
        .map_err(err)?;
    window.popup_menu(&menu).map_err(err)
}

/// Sürükle-bırak sonrası simge sırası.
#[tauri::command]
pub fn set_app_order(app: AppHandle, order: Vec<String>) {
    let state = app.state::<AppState>();
    let mut cfg = state.cfg.lock().unwrap();
    if cfg.apps_locked {
        return;
    }
    cfg.app_order = order.into_iter().take(200).collect();
    config::save(&cfg);
}

#[tauri::command]
pub fn set_apps_locked(app: AppHandle, locked: bool) {
    {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        cfg.apps_locked = locked;
        config::save(&cfg);
    }
    let _ = tauri::Emitter::emit(&app, "apps-locked", locked);
}

/// Dock sağ tık menüsündeki sistem kısayolları ("sys:…").
pub fn system_shortcut(id: &str) {
    let Some(target) = id.strip_prefix("sys:") else { return };
    let target = target.to_string();
    std::thread::spawn(move || match target.as_str() {
        "run" => platform::win_combo('R'),
        "desktop" => platform::win_combo('D'),
        "terminal" => {
            if !platform::launch("wt.exe", None) {
                platform::launch("powershell.exe", None);
            }
        }
        t if t.ends_with(".msc") => {
            platform::launch("mmc.exe", Some(t));
        }
        t => {
            platform::launch(t, None);
        }
    });
}

// ---------------------------------------------------------------------------
// Uygulama sağ tık menüsü
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct AppMenuItem {
    pub name: String,
    pub path: String,
    pub args: Option<String>,
    #[serde(default)]
    pub aumid: String,
    /// Açık pencereler: (hwnd, başlık)
    #[serde(default)]
    pub windows: Vec<(isize, String)>,
    #[serde(default)]
    pub pinned: bool,
}

/// Tarayıcıysa: (gizli pencere etiketi, gizli pencere bayrağı, yeni pencere bayrağı)
fn browser_flags(path: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let exe = path.rsplit('\\').next().unwrap_or("").to_lowercase();
    match exe.as_str() {
        "chrome.exe" | "brave.exe" | "vivaldi.exe" | "chromium.exe" => Some(("Yeni gizli pencere", "--incognito", "--new-window")),
        "msedge.exe" => Some(("Yeni InPrivate penceresi", "--inprivate", "--new-window")),
        "firefox.exe" => Some(("Yeni gizli pencere", "-private-window", "-new-window")),
        "opera.exe" => Some(("Yeni özel pencere", "--private", "--new-window")),
        _ => None,
    }
}

/// Bir uygulama simgesine sağ tıklama menüsü.
#[tauri::command]
pub fn app_menu(window: WebviewWindow, state: State<'_, AppState>, item: AppMenuItem) -> Result<(), String> {
    use tauri::menu::{MenuItemBuilder, PredefinedMenuItem};
    let app = window.app_handle();
    let is_file = !item.path.to_lowercase().starts_with("shell:") && item.path.contains(":\\");
    let is_exe = item.path.to_lowercase().ends_with(".exe");
    let mut b = MenuBuilder::new(app);
    b = b.item(&MenuItemBuilder::with_id("am:noop", &item.name).enabled(false).build(app).map_err(err)?);
    b = b.item(&PredefinedMenuItem::separator(app).map_err(err)?);
    if !item.windows.is_empty() {
        for (h, title) in item.windows.iter().take(10) {
            let mut t: String = title.chars().take(60).collect();
            if title.chars().count() > 60 {
                t.push('…');
            }
            b = b.text(format!("am:win:{h}"), t);
        }
        b = b.item(&PredefinedMenuItem::separator(app).map_err(err)?);
    }
    b = b.text("am:new", "Yeni pencere");
    if let Some((label, _, _)) = browser_flags(&item.path) {
        b = b.text("am:private", label);
    }
    if is_exe {
        b = b.text("am:admin", "Yönetici olarak çalıştır");
    }
    if is_file {
        b = b.text("am:folder", "Dosya konumunu aç");
    }
    b = b.item(&PredefinedMenuItem::separator(app).map_err(err)?);
    b = b.text(if item.pinned { "am:unpin" } else { "am:pin" }, if item.pinned { "Dock'tan kaldır" } else { "Dock'a sabitle" });
    let locked = app.state::<AppState>().cfg.lock().unwrap().apps_locked;
    b = b.text("apps:lock", if locked { "Simge düzeninin kilidini aç" } else { "Simge düzenini kaydet ve kilitle" });
    if !item.windows.is_empty() {
        let n = item.windows.len();
        b = b.text("am:close", if n > 1 { format!("Tüm pencereleri kapat ({n})") } else { "Pencereyi kapat".to_string() });
    }
    let menu = b.build().map_err(err)?;
    *state.app_menu.lock().unwrap() = Some(item);
    window.popup_menu(&menu).map_err(err)
}

/// Uygulama menüsünden seçim (main.rs'teki menü olayından çağrılır).
pub fn app_menu_action(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let Some(item) = state.app_menu.lock().unwrap().clone() else { return };
    let path = config::expand_env(&item.path);
    let open = |p: &str, a: Option<&str>| {
        if p.to_lowercase().starts_with("shell:") {
            platform::launch("explorer.exe", Some(p))
        } else {
            platform::launch(p, a)
        }
    };
    let id = id.to_string();
    let app = app.clone();
    // Pencere işlemleri ve ShellExecute ana iş parçacığını meşgul etmesin.
    std::thread::spawn(move || {
        if let Some(h) = id.strip_prefix("am:win:") {
            if let Ok(h) = h.parse::<isize>() {
                platform::bring_front(h);
            }
            return;
        }
        match id.as_str() {
            "am:new" => {
                let flag = browser_flags(&path).map(|f| f.2);
                open(&path, flag.or(item.args.as_deref()));
            }
            "am:private" => {
                if let Some((_, f, _)) = browser_flags(&path) {
                    open(&path, Some(f));
                }
            }
            "am:admin" => {
                platform::launch_admin(&path, item.args.as_deref());
            }
            "am:folder" => {
                platform::launch("explorer.exe", Some(&format!("/select,\"{path}\"")));
            }
            "am:pin" | "am:unpin" => {
                let state = app.state::<AppState>();
                {
                    let mut cfg = state.cfg.lock().unwrap();
                    // Store uygulamaları (WhatsApp, Teams…) uygulama kimliğiyle
                    // sabitlenir: WindowsApps içindeki exe'nin kendi simgesi
                    // yok, doğrudan da başlatılamıyor.
                    let pin_path = if item.aumid.is_empty() {
                        item.path.clone()
                    } else {
                        format!("shell:AppsFolder\\{}", item.aumid)
                    };
                    if id == "am:pin" {
                        let exists = cfg.pinned.iter().any(|p| {
                            let e = config::expand_env(&p.path);
                            e.eq_ignore_ascii_case(&path) || e.eq_ignore_ascii_case(&pin_path)
                        });
                        if !exists {
                            cfg.pinned.push(config::Pinned { name: item.name.clone(), path: pin_path, args: None });
                        }
                    } else {
                        cfg.pinned.retain(|p| {
                            let e = config::expand_env(&p.path);
                            !e.eq_ignore_ascii_case(&path) && !e.eq_ignore_ascii_case(&pin_path)
                        });
                    }
                    config::save(&cfg);
                }
                crate::emit_config(&app);
            }
            "am:close" => {
                for (h, _) in &item.windows {
                    platform::close(*h);
                }
            }
            _ => {}
        }
    });
}
