//! Dock pencereleri: her monitöre bir dock, klasik taskbar'ın gizlenmesi ve
//! dock'un görünürlük modları (her zaman / otomatik gizle / akıllı gizle).

use crate::config::{self, Config};
use crate::platform::{self, Rect};
use crate::AppState;
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone)]
pub struct Dock {
    pub label: String,
    pub hwnd: isize,
    /// Monitör aygıt adı ("\\.\DISPLAY1")
    pub monitor: String,
    /// Monitör dikdörtgeni (fiziksel piksel)
    pub mon_rect: Rect,
    pub scale: f64,
    pub primary: bool,
    /// Yerleşim sonrası hedef konum/boyut: x, y, w, h (fiziksel)
    pub target: Option<(i32, i32, i32, i32)>,
    pub shown: bool,
    pub fs_hidden: bool,
    pub last_inside: Instant,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorView {
    pub name: String,
    /// Kullanıcıya gösterilen ad: "Ekran 1"
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub primary: bool,
    /// Sol, üst, sağ, alt (fiziksel piksel) — ayarlardaki ekran çizimi için
    pub rect: Rect,
}

pub fn monitors(app: &AppHandle) -> Vec<MonitorView> {
    let primary = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());
    let mut list: Vec<MonitorView> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let name = m.name().cloned().unwrap_or_else(|| format!("MONITOR{}", i + 1));
            let (p, s) = (m.position(), m.size());
            MonitorView {
                primary: primary.as_deref() == Some(name.as_str()),
                title: String::new(),
                width: s.width,
                height: s.height,
                scale: m.scale_factor(),
                rect: (p.x, p.y, p.x + s.width as i32, p.y + s.height as i32),
                name,
            }
        })
        .collect();
    // Soldan sağa sırala, kullanıcıya "Ekran 1, 2…" diye göster.
    list.sort_by_key(|m| (m.rect.0, m.rect.1));
    for (i, m) in list.iter_mut().enumerate() {
        m.title = format!("Ekran {}", i + 1);
    }
    if !list.iter().any(|m| m.primary) {
        if let Some(first) = list.first_mut() {
            first.primary = true;
        }
    }
    list
}

pub fn label_for(monitor: &str) -> String {
    let clean: String = monitor.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("dock-{clean}")
}

fn targets<'a>(cfg: &Config, mons: &'a [MonitorView]) -> Vec<&'a MonitorView> {
    let picked: Vec<&MonitorView> = match cfg.display.mode.as_str() {
        "all" => mons.iter().collect(),
        "selected" => mons
            .iter()
            .filter(|m| cfg.display.selected.iter().any(|s| s == &m.name))
            .collect(),
        _ => mons.iter().filter(|m| m.primary).collect(),
    };
    if picked.is_empty() {
        mons.iter().filter(|m| m.primary).take(1).collect()
    } else {
        picked
    }
}

fn create_window(app: &AppHandle, label: &str, mon: Rect) -> tauri::Result<isize> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Cam Dock")
        .inner_size(1600.0, 84.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .build()?;
    #[cfg(windows)]
    let hwnd = window.hwnd()?.0 as isize;
    #[cfg(not(windows))]
    let hwnd = {
        let _ = window;
        0
    };
    // Başlık çubuğu stilini kaldır ve görünmeden önce ekranın altına (dışına)
    // taşı: ilk gösterimde aşağıdan kayarak gelsin, üstte belirip inmesin.
    platform::make_popup(hwnd);
    platform::move_to(hwnd, mon.0, mon.3 + 40);
    Ok(hwnd)
}

/// Ayarlara ve bağlı monitörlere göre dock pencerelerini oluşturur/kapatır.
pub fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().clone();
    let mons = monitors(app);
    let wanted = targets(&cfg, &mons);
    let wanted_labels: Vec<String> = wanted.iter().map(|m| label_for(&m.name)).collect();

    let to_close: Vec<Dock> = state
        .docks
        .lock()
        .unwrap()
        .values()
        .filter(|d| !wanted_labels.contains(&d.label))
        .cloned()
        .collect();
    for d in to_close {
        platform::release(d.hwnd);
        if let Some(w) = app.get_webview_window(&d.label) {
            let _ = w.destroy();
        }
        state.docks.lock().unwrap().remove(&d.label);
    }

    for m in wanted {
        let label = label_for(&m.name);
        if let Some(d) = state.docks.lock().unwrap().get_mut(&label) {
            d.mon_rect = m.rect;
            d.scale = m.scale;
            d.primary = m.primary;
            continue;
        }
        match create_window(app, &label, m.rect) {
            Ok(hwnd) => {
                state.docks.lock().unwrap().insert(
                    label.clone(),
                    Dock {
                        label,
                        hwnd,
                        monitor: m.name.clone(),
                        mon_rect: m.rect,
                        scale: m.scale,
                        primary: m.primary,
                        target: None,
                        shown: false,
                        fs_hidden: false,
                        last_inside: Instant::now(),
                    },
                );
            }
            Err(e) => eprintln!("dock penceresi açılamadı ({}): {e}", m.name),
        }
    }

    *state.monitor_sig.lock().unwrap() = monitor_signature(&mons);
    apply_taskbar(app);
    let _ = app.emit("relayout", ());
}

fn monitor_signature(mons: &[MonitorView]) -> String {
    mons.iter()
        .map(|m| format!("{}{:?}{}", m.name, m.rect, m.scale))
        .collect::<Vec<_>>()
        .join("|")
}

// ---------------------------------------------------------------------------
// Klasik taskbar. Uygulama çökse bile geri getirebilmek için önceki durumu
// diske yazıyoruz; `cam-dock.exe --restore-taskbar` bunu kullanır.
// ---------------------------------------------------------------------------

fn guard_path() -> std::path::PathBuf {
    config::base_dir().join("taskbar-state.txt")
}

pub fn guard_read() -> Option<u32> {
    std::fs::read_to_string(guard_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Ayara göre dock olan ekranlardaki taskbar'ı gizler ya da hepsini geri getirir.
pub fn apply_taskbar(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.cfg.lock().unwrap().hide_taskbar {
        restore_taskbar(&state);
        return;
    }
    let rects: Vec<Rect> = state
        .docks
        .lock()
        .unwrap()
        .values()
        .map(|d| d.mon_rect)
        .collect();
    let current = platform::hide_taskbar(&rects);
    let mut prev = state.taskbar_prev.lock().unwrap();
    if prev.is_none() {
        // Önceki bir çalıştırma çöktüyse asıl orijinal durum dosyadadır.
        let original = guard_read().unwrap_or(current);
        let _ = std::fs::write(guard_path(), original.to_string());
        *prev = Some(original);
    }
}

pub fn restore_taskbar(state: &AppState) {
    if let Some(prev) = state.taskbar_prev.lock().unwrap().take() {
        platform::restore_taskbar(prev);
        let _ = std::fs::remove_file(guard_path());
    }
}

pub fn emergency_restore() {
    let prev = guard_read().unwrap_or(2);
    platform::restore_taskbar(prev);
    let _ = std::fs::remove_file(guard_path());
}

// ---------------------------------------------------------------------------
// Görünürlük: kayarak açılma/kapanma
// ---------------------------------------------------------------------------

fn animate(hwnd: isize, x: i32, from: i32, to: i32) {
    const STEPS: i32 = 9;
    for i in 1..=STEPS {
        let t = i as f64 / STEPS as f64;
        let ease = 1.0 - (1.0 - t).powi(3);
        let y = from + ((to - from) as f64 * ease).round() as i32;
        platform::move_to(hwnd, x, y);
        std::thread::sleep(Duration::from_millis(11));
    }
}

pub fn slide_in(d: &Dock) {
    let Some((x, y, _, _)) = d.target else { return };
    let off = d.mon_rect.3;
    platform::move_to(d.hwnd, x, off);
    platform::show_no_activate(d.hwnd);
    animate(d.hwnd, x, off, y);
}

fn slide_out(d: &Dock) {
    let Some((x, y, _, _)) = d.target else { return };
    animate(d.hwnd, x, y, d.mon_rect.3);
    platform::hide(d.hwnd);
}

/// Her 100 ms'de çağrılır: tam ekran, otomatik/akıllı gizleme.
fn update_visibility(state: &AppState, cfg: &Config, hwnds: &[isize], fg: isize) {
    let now = Instant::now();
    let cursor = platform::cursor_pos();
    let fs_mon = if cfg.hide_in_fullscreen {
        platform::fullscreen_monitor(hwnds)
    } else {
        None
    };
    let smart = cfg.dock_visibility == "smart";
    let always = cfg.dock_visibility != "autohide" && !smart;
    let fg_rect = if smart {
        platform::foreground_app_rect(hwnds)
    } else {
        None
    };
    let fg_is_dock = hwnds.contains(&fg);
    let delay = Duration::from_millis(cfg.hide_delay_ms);

    let mut docks = state.docks.lock().unwrap();
    for d in docks.values_mut() {
        let Some((x, y, w, h)) = d.target else { continue };
        let m = d.mon_rect;

        if fs_mon.as_deref() == Some(d.monitor.as_str()) {
            if !d.fs_hidden {
                platform::hide(d.hwnd);
                d.fs_hidden = true;
                d.shown = false;
            }
            continue;
        }
        if d.fs_hidden {
            d.fs_hidden = false;
            d.last_inside = now;
        }

        let want = if always {
            true
        } else {
            let in_x = cursor.0 >= m.0 && cursor.0 < m.2;
            let near_edge = in_x && cursor.1 >= m.3 - 3 && cursor.1 < m.3;
            let pad = 12;
            let inside = d.shown
                && cursor.0 >= x - pad
                && cursor.0 < x + w + pad
                && cursor.1 >= y - pad
                && cursor.1 < m.3;
            if near_edge || inside || (fg_is_dock && d.shown) {
                d.last_inside = now;
            }
            let recent = now.duration_since(d.last_inside) < delay;
            if smart {
                let overlap = fg_rect
                    .map(|r| r.0 < x + w && x < r.2 && r.1 < y + h && y < r.3)
                    .unwrap_or(false);
                !overlap || recent
            } else {
                recent
            }
        };

        if want && !d.shown {
            slide_in(d);
            d.shown = true;
        } else if !want && d.shown {
            slide_out(d);
            d.shown = false;
        }
    }
}

/// Arka plan döngüsü (100 ms): görünürlük, taskbar'ı gizli tutma, son ön plan
/// penceresi, ekran süresi, monitör takıp çıkarma.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            tick += 1;
            let state = app.state::<AppState>();
            let cfg = state.cfg.lock().unwrap().clone();
            let hwnds: Vec<isize> = state.docks.lock().unwrap().values().map(|d| d.hwnd).collect();

            let fg = platform::foreground();
            if fg != 0 && !hwnds.contains(&fg) {
                state.last_fg.store(fg, std::sync::atomic::Ordering::SeqCst);
            }

            update_visibility(&state, &cfg, &hwnds, fg);

            if tick % 5 == 0 && state.taskbar_prev.lock().unwrap().is_some() {
                platform::enforce_taskbar();
            }
            // Pencere katmanı stili geri koyarsa tekrar temizle.
            if tick % 10 == 0 {
                for h in &hwnds {
                    platform::make_popup(*h);
                }
            }
            if tick % 10 == 0 && platform::idle_ms() < 120_000 {
                state.screen.lock().unwrap().tick(1);
            }
            if tick % 1200 == 0 {
                state.screen.lock().unwrap().save();
            }
            if tick % 30 == 0 {
                let sig = monitor_signature(&monitors(&app));
                let changed = *state.monitor_sig.lock().unwrap() != sig;
                if changed {
                    sync(&app);
                }
            }
        }
    });
}

/// Ayarlar penceresini açar ya da öne getirir.
pub fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Cam Dock — Ayarlar")
        .inner_size(900.0, 720.0)
        .min_inner_size(760.0, 560.0)
        .center()
        .build()?;
    Ok(())
}
