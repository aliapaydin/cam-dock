// Release derlemede arkada konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod docks;
mod platform;
mod screentime;

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
    /// Taskbar'ı biz gizlediysek önceki durumu
    pub taskbar_prev: Mutex<Option<u32>>,
    pub icons: Mutex<HashMap<String, Option<String>>>,
    pub screen: Mutex<screentime::ScreenTime>,
    pub thumb: Mutex<(String, Option<String>)>,
    pub monitor_sig: Mutex<String>,
}

pub fn emit_config(app: &tauri::AppHandle) {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().clone();
    let _ = app.emit("config-changed", cfg);
}

fn cleanup(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    docks::restore_taskbar(&state);
    platform::release_all();
    state.screen.lock().unwrap().save();
}

fn on_menu(app: &tauri::AppHandle, id: &str) {
    let state = app.state::<AppState>();
    if let Some(theme) = id.strip_prefix("theme:") {
        {
            let mut cfg = state.cfg.lock().unwrap();
            cfg.theme = theme.to_string();
            config::save(&cfg);
        }
        emit_config(app);
    } else if id == "settings" {
        // Pencere oluşturmayı ana iş parçacığının dışında yap.
        let app = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = docks::open_settings(&app) {
                eprintln!("ayarlar açılamadı: {e}");
            }
        });
    } else if id == "quit" {
        app.exit(0);
    }
}

fn main() {
    if std::env::args().any(|a| a == "--restore-taskbar") {
        docks::emergency_restore();
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
        taskbar_prev: Mutex::new(None),
        icons: Mutex::new(HashMap::new()),
        screen: Mutex::new(screentime::ScreenTime::load()),
        thumb: Mutex::new((String::new(), None)),
        monitor_sig: Mutex::new(String::new()),
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
            commands::close_window,
            commands::launch_app,
            commands::app_icon,
            commands::start_menu_apps,
            commands::resolve_shortcut,
            commands::media_state,
            commands::media_thumbnail,
            commands::media_control,
            commands::fetch_text,
            commands::screen_time,
            commands::open_start,
            commands::open_settings,
            commands::show_menu,
        ])
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .setup(|app| {
            let handle = app.handle().clone();
            {
                let state = handle.state::<AppState>();
                if docks::guard_read().is_some() && !state.cfg.lock().unwrap().hide_taskbar {
                    // Önceki çalıştırmadan kalan gizli taskbar'ı geri getir.
                    docks::emergency_restore();
                }
            }
            docks::sync(&handle);
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
