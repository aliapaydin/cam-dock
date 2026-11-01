// Release derlemede arkada konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod docks;
mod log;
mod platform;
mod popover;
mod screentime;
mod theme;
mod pinreq;
mod telemetry;
mod quickset;
mod notify;
mod clip;
mod claude;
mod updater;

use config::Config;
use std::collections::HashMap;
use std::sync::atomic::AtomicIsize;
use std::sync::Mutex;
use tauri::{Emitter, Manager, RunEvent};

pub struct AppState {
    pub cfg: Mutex<Config>,
    /// Pencere etiketi → dock
    pub docks: Mutex<HashMap<String, docks::Dock>>,
    /// Dock'a tıklanmadan önceki ön plan penceresi
    pub last_fg: AtomicIsize,
    /// Kullanıcı menüden "Çık" dedi (taskbar mutlaka geri gelsin)
    pub quit_requested: std::sync::atomic::AtomicBool,
    /// Taskbar'ı biz gizlediysek önceki durumu
    pub taskbar_prev: Mutex<Option<u32>>,
    pub icons: Mutex<HashMap<String, Option<String>>>,
    pub screen: Mutex<screentime::ScreenTime>,
    pub thumb: Mutex<(String, Option<String>)>,
    pub monitor_sig: Mutex<String>,
    /// Klasik taskbar geçici olarak gösteriliyorsa bitiş zamanı
    pub peek_until: Mutex<Option<std::time::Instant>>,
    pub pop: Mutex<popover::PopState>,
    /// Windows uygulama kataloğu (önbellek)
    pub catalog: Mutex<Option<Vec<platform::CatalogApp>>>,
    /// Son sistem kaynakları örneği
    pub stats: Mutex<platform::SysStats>,
    /// Son ~2 dakikanın örnekleri (kullanım panelindeki çizgiler için)
    pub stats_hist: Mutex<std::collections::VecDeque<platform::SysStats>>,
    /// Son okunan Windows teması (açık mı)
    pub system_light: Mutex<Option<bool>>,
    /// Tüm pencerelerin kullandığı geçerli tema (dinamik mod dahil)
    pub eff_dark: Mutex<Option<bool>>,
    pub update: Mutex<updater::Status>,
    pub notif: Mutex<notify::NotifState>,
    pub toast: Mutex<notify::ToastWin>,
    pub clips: Mutex<clip::History>,
    pub claude: Mutex<claude::Usage>,
    pub quick: Mutex<quickset::QuickState>,
    /// Sağ tık menüsü açılan uygulama
    pub app_menu: Mutex<Option<commands::AppMenuItem>>,
}

pub fn emit_config(app: &tauri::AppHandle) {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().clone();
    let _ = app.emit("config-changed", cfg);
}

fn cleanup(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    // Windows kapanırken/oturum kapanırken taskbar'ı geri açma: açılışta klasik
    // taskbar görünmesin, dock gelene kadar gizli kalsın. "Çık" denince ve
    // kaldırırken taskbar her zaman geri gelir.
    let keep_hidden = platform::session_ending()
        && !state.quit_requested.load(std::sync::atomic::Ordering::SeqCst)
        && {
            let cfg = state.cfg.lock().unwrap();
            cfg.autostart && cfg.hide_taskbar
        };
    if keep_hidden {
        dlog!("Windows kapanıyor: taskbar gizli bırakıldı (açılışta görünmesin)");
    } else {
        docks::restore_taskbar(&state);
    }
    platform::release_all();
    state.screen.lock().unwrap().save();
}

fn on_menu(app: &tauri::AppHandle, id: &str) {
    if id.starts_with("am:") {
        commands::app_menu_action(app, id);
        return;
    }
    if id == "apps:lock" {
        let locked = !app.state::<AppState>().cfg.lock().unwrap().apps_locked;
        commands::set_apps_locked(app.clone(), locked);
        return;
    }
    if id.starts_with("sys:") {
        commands::system_shortcut(id);
        return;
    }
    let state = app.state::<AppState>();
    if let Some(theme) = id.strip_prefix("theme:") {
        {
            let mut cfg = state.cfg.lock().unwrap();
            cfg.theme = theme.to_string();
            config::save(&cfg);
        }
        emit_config(app);
        docks::refresh_theme(app);
    } else if id == "settings" {
        // Pencere oluşturmayı ana iş parçacığının dışında yap.
        let app = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = docks::open_settings(&app) {
                eprintln!("ayarlar açılamadı: {e}");
            }
        });
    } else if id == "quit" {
        state.quit_requested.store(true, std::sync::atomic::Ordering::SeqCst);
        app.exit(0);
    }
}

fn main() {
    // Kaldırma programı: (izin açıksa) kaldırıldığını bildir ve çık.
    if std::env::args().any(|a| a == "--uninstall-ping") {
        telemetry::uninstall_ping();
        return;
    }
    if std::env::args().any(|a| a == "--restore-taskbar") {
        docks::emergency_restore();
        return;
    }

    // Gezgin'den "Cam Dock'a sabitle": isteği kuyruğa yaz; dock zaten açıksa o işler.
    pinreq::queue_from_args();

    // Windows ile başlatma + elle açma iki dock oluşturmasın.
    if !platform::single_instance() {
        return;
    }

    // Panik olursa taskbar'ı ortada bırakmayalım.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        docks::emergency_restore();
        default_hook(info);
    }));

    let state = AppState {
        cfg: Mutex::new(config::load()),
        docks: Mutex::new(HashMap::new()),
        last_fg: AtomicIsize::new(0),
        quit_requested: std::sync::atomic::AtomicBool::new(false),
        taskbar_prev: Mutex::new(None),
        icons: Mutex::new(HashMap::new()),
        screen: Mutex::new(screentime::ScreenTime::load()),
        thumb: Mutex::new((String::new(), None)),
        monitor_sig: Mutex::new(String::new()),
        peek_until: Mutex::new(None),
        pop: Mutex::new(popover::PopState::default()),
        catalog: Mutex::new(None),
        stats: Mutex::new(platform::SysStats::default()),
        stats_hist: Mutex::new(std::collections::VecDeque::with_capacity(120)),
        system_light: Mutex::new(None),
        eff_dark: Mutex::new(None),
        update: Mutex::new(updater::Status::default()),
        notif: Mutex::new(notify::NotifState::default()),
        toast: Mutex::new(notify::ToastWin::default()),
        clips: Mutex::new(clip::History::default()),
        claude: Mutex::new(claude::Usage::default()),
        quick: Mutex::new(quickset::QuickState::default()),
        app_menu: Mutex::new(None),
    };

    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::dock_info,
            commands::list_monitors,
            commands::layout_dock,
            commands::set_theme,
            commands::list_windows,
            commands::activate_window,
            commands::activate_app,
            commands::toggle_lock,
            popover::popover_open,
            popover::popover_pending,
            popover::popover_layout,
            popover::popover_close,
            popover::popover_hidden,
            popover::popover_hover,
            popover::popover_dock_hover,
            popover::popover_thumbs,
            popover::popover_activate,
            commands::close_window,
            commands::launch_app,
            commands::app_icon,
            commands::start_menu_apps,
            commands::resolve_shortcut,
            commands::media_state,
            commands::media_thumbnail,
            commands::media_control,
            commands::media_list,
            commands::media_control_id,
            commands::media_thumbnail_id,
            commands::catalog_apps,
            commands::pick_app,
            commands::sys_stats,
            commands::system_theme,
            commands::tray_apps,
            commands::fetch_text,
            commands::screen_time,
            commands::usage_data,
            commands::theme_dark,
            commands::wallpaper,
            commands::sun_times,
            commands::autostart_state,
            commands::set_weather_location,
            updater::update_status,
            notify::notif_state,
            notify::notif_open_center,
            notify::toast_show,
            notify::toast_pending,
            notify::toast_layout,
            notify::toast_hide,
            notify::toast_fade,
            notify::toast_click,
            clip::clip_list,
            clip::clip_copy,
            clip::clip_remove,
            clip::clip_pin,
            clip::clip_clear,
            claude::claude_usage,
            telemetry::telemetry_delete,
            telemetry::telemetry_notice_seen,
            quickset::quick_state,
            quickset::quick_volume,
            quickset::quick_audio_device,
            quickset::quick_brightness,
            quickset::quick_radio,
            quickset::wifi_list,
            quickset::wifi_connect,
            quickset::wifi_disconnect,
            claude::claude_refresh,
            updater::update_check,
            updater::update_install,
            commands::open_start,
            commands::set_app_order,
            commands::set_apps_locked,
            commands::system_action,
            commands::open_settings,
            commands::show_menu,
            commands::app_menu,
        ])
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .setup(|app| {
            let handle = app.handle().clone();
            dlog!("---- Cam Dock {} başlıyor ----", env!("CARGO_PKG_VERSION"));
            {
                let state = handle.state::<AppState>();
                if docks::guard_read().is_some() && !state.cfg.lock().unwrap().hide_taskbar {
                    // Önceki çalıştırmadan kalan gizli taskbar'ı geri getir.
                    docks::emergency_restore();
                }
            }
            docks::sync(&handle);
            {
                // Windows ile başlat: exe yeri değişmiş olabilir, her açılışta yenile.
                // Açık ama oturum görevi yoksa (eski sürümden geçiş, exe yeri değişti) kur.
                // schtasks yavaş (~0,3 sn): açılışı bekletmesin.
                let want = handle.state::<AppState>().cfg.lock().unwrap().autostart;
                std::thread::spawn(move || {
                    if want {
                        // Görevi her açılışta yenile (kurulum yeri değişmiş olabilir).
                        commands::apply_autostart(true);
                    } else if platform::autostart_get() {
                        commands::apply_autostart(false);
                    }
                });
            }
            if let Some(light) = platform::light_theme() {
                *handle.state::<AppState>().system_light.lock().unwrap() = Some(light);
            }
            docks::refresh_theme(&handle);
            telemetry::init(&handle);
            pinreq::sync_menu(handle.state::<AppState>().cfg.lock().unwrap().explorer_menu);
            pinreq::process(&handle);
            telemetry::spawn(handle.clone());
            updater::spawn(handle.clone());
            notify::spawn(handle.clone());
            clip::spawn(handle.clone());
            claude::spawn(handle.clone());
            // Sistem kaynakları: saniyede bir örnekle.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    let mut sampler = platform::Sampler::new();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(1000));
                        let st = sampler.sample();
                        let state = h.state::<AppState>();
                        *state.stats.lock().unwrap() = st;
                        let mut hist = state.stats_hist.lock().unwrap();
                        if hist.len() >= 120 {
                            hist.pop_front();
                        }
                        hist.push_back(st);
                    }
                });
            }
            docks::spawn_watcher(handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Cam Dock başlatılamadı");

    app.run(|app, event| match event {
        // Dock pencereleri yeniden kurulurken (ekran değişimi) tüm pencereler
        // kısa süre kapanabilir; bu yüzden kendiliğinden çıkışı engelle.
        // Menüden "Çık" (app.exit) bir çıkış kodu taşır ve engellenmez.
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => cleanup(app),
        _ => {}
    });
}
