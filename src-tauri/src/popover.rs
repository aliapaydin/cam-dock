//! Dock'un üstünde açılan cam panel: pencere önizlemeleri ve takvim.
//!
//! Her dock'un (ekranın) bir panel penceresi vardır ("pop-…"). Panel gizli
//! bekler; açılınca içerik ölçülür, dock'un üstüne yerleşir, kayarak/büyüyerek
//! belirir. Dışarı tıklayınca, başka bir uygulamaya geçince ya da (önizlemede)
//! fare ayrılınca kapanır. Açıkken dock otomatik gizlenmez.

use crate::docks::{self, Rect};
use crate::platform;
use crate::commands::run;
use crate::{dlog, AppState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Debug, Clone)]
pub struct Open {
    pub dock: String,
    pub pop: String,
    pub kind: String,
    pub key: String,
    pub payload: Value,
    pub serial: u64,
    /// Ekran koordinatında panelin dikeyde ortalanacağı x (fiziksel)
    pub anchor_x: i32,
    /// Yerleşimden sonra panel dikdörtgeni: sol, üst, sağ, alt
    pub rect: Option<Rect>,
    /// Açıldığı anda ön plandaki pencere (değişirse panel kapanır)
    pub fg_at_open: isize,
}

#[derive(Default)]
pub struct PopState {
    pub open: Option<Open>,
    pub dock_hover: bool,
    pub pop_hover: bool,
    pub thumbs: Vec<isize>,
    pub serial: u64,
    /// Camın açılış animasyonu hangi açılış için oynatıldı
    pub glass_serial: u64,
    /// Panel pencereleri: etiket → hwnd
    pub windows: std::collections::HashMap<String, isize>,
}

fn pop_label(dock: &str) -> String {
    format!("pop-{}", dock.trim_start_matches("dock-"))
}

#[derive(Serialize, Clone)]
struct ShowEvent {
    kind: String,
    key: String,
    payload: Value,
    serial: u64,
}

fn clear_thumbs(state: &AppState) {
    let thumbs = std::mem::take(&mut state.pop.lock().unwrap().thumbs);
    for t in thumbs {
        platform::thumb_remove(t);
    }
}

/// Paneli kapatır (animasyonla). Kapanınca pencereyi panel JS'i gizletir.
pub fn close(app: &AppHandle) {
    let state = app.state::<AppState>();
    let open = state.pop.lock().unwrap().open.take();
    clear_thumbs(&state);
    if let Some(o) = open {
        // Cam da panelle birlikte küçülerek sönsün (arkada asılı kalmasın).
        if let Some(h) = state.pop.lock().unwrap().windows.get(&o.pop).copied() {
            let _ = app.run_on_main_thread(move || platform::glass_animate(h, false, 190));
        }
        let _ = app.emit_to(o.pop.as_str(), "popover-out", o.serial);
        let _ = app.emit_to(o.dock.as_str(), "popover-closed", o.kind.clone());
    }
}

/// Paneli açar; aynı panel zaten açıksa takvimde kapatır (aç/kapa düğmesi).
#[tauri::command]
pub async fn popover_open(app: AppHandle, window: WebviewWindow, kind: String, key: String, anchor: f64, payload: Value) -> Result<String, String> {
    match run(move || {
        let state = app.state::<AppState>();

    let app = window.app_handle().clone();
    let dock = window.label().to_string();
    let Some((target, scale, mon)) = state
        .docks
        .lock()
        .unwrap()
        .get(&dock)
        .map(|d| (d.target, d.scale, d.mon_rect))
    else {
        return Ok("none".into());
    };
    let Some((dx, _, _, _)) = target else { return Ok("none".into()) };

    {
        let p = state.pop.lock().unwrap();
        if let Some(o) = &p.open {
            if o.dock == dock && o.kind == kind && o.key == key {
                drop(p);
                if kind != "windows" {
                    close(&app);
                    return Ok("closed".into());
                }
                return Ok("open".into());
            }
        }
    }
    clear_thumbs(&state);

    let pop = pop_label(&dock);
    // Kilidi bırakarak oku: eskiden bu `match` kilidi kollar boyunca tutuyor,
    // `None` kolunda aynı kilidi tekrar almaya çalışıp sonsuza dek kilitleniyordu.
    let existing = state.pop.lock().unwrap().windows.get(&pop).copied();
    let hwnd = match existing {
        Some(h) => h,
        None => {
            let h = docks::create_window(&app, &pop, mon, "popover.html").map_err(|e| e.to_string())?;
            dlog!("{pop}: panel penceresi oluşturuldu");
            state.pop.lock().unwrap().windows.insert(pop.clone(), h);
            h
        }
    };
    let _ = hwnd;

    let serial = {
        let mut p = state.pop.lock().unwrap();
        p.serial += 1;
        p.pop_hover = false;
        p.open = Some(Open {
            dock: dock.clone(),
            pop: pop.clone(),
            kind: kind.clone(),
            key: key.clone(),
            payload: payload.clone(),
            serial: p.serial,
            anchor_x: dx + (anchor * scale).round() as i32,
            rect: None,
            fg_at_open: state.last_fg.load(std::sync::atomic::Ordering::SeqCst),
        });
        p.serial
    };
    let _ = app.emit_to(
        pop.as_str(),
        "popover-show",
        ShowEvent { kind, key, payload, serial },
    );
    Ok("opened".into())

    }).await {
        Ok(r) => r,
        Err(e) => Err(e),
    }
}

/// Yeni açılan panel sayfası yüklenince bekleyen içeriği ister.
#[tauri::command]
pub async fn popover_pending(app: AppHandle, window: WebviewWindow) -> Option<Value> {
    run(move || {
        let state = app.state::<AppState>();

    let p = state.pop.lock().unwrap();
    let o = p.open.as_ref()?;
    (o.pop == window.label()).then(|| {
        serde_json::json!({ "kind": o.kind, "key": o.key, "payload": o.payload, "serial": o.serial })
    })

    }).await.unwrap_or_default()
}

/// Panel içeriği çizildi; ölçüsünü bildirir. Dock'un üstüne yerleştirip gösteririz.
#[tauri::command]
pub async fn popover_layout(app: AppHandle, window: WebviewWindow, width: f64, height: f64, serial: u64) -> Result<(), String> {
    match run(move || {
        let state = app.state::<AppState>();

    let app = window.app_handle().clone();
    let (dock, anchor_x) = {
        let p = state.pop.lock().unwrap();
        match &p.open {
            Some(o) if o.serial == serial && o.pop == window.label() => (o.dock.clone(), o.anchor_x),
            _ => return Ok(()),
        }
    };
    let Some((target, scale, mon, dark, tint)) = state
        .docks
        .lock()
        .unwrap()
        .get(&dock)
        .map(|d| (d.target, d.scale, d.mon_rect, d.dark, d.tint))
    else {
        return Ok(());
    };
    let Some((_, dy, _, _)) = target else { return Ok(()) };
    let hwnd = match state.pop.lock().unwrap().windows.get(window.label()) {
        Some(h) => *h,
        None => return Ok(()),
    };

    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;
    let gap = (12.0 * scale) as i32;
    let edge = (10.0 * scale) as i32;
    let x = (anchor_x - w / 2).clamp(mon.0 + edge, (mon.2 - w - edge).max(mon.0 + edge));
    let y = (dy - gap - h).max(mon.1 + edge);

    platform::set_bounds(hwnd, x, y, w, h);
    let mode = state.cfg.lock().unwrap().backdrop.clone();
    docks::glass_raw(&app, hwnd, w, h, (20.0 * scale) as f32, dark, tint, mode, 0);
    // Cam, panelle birlikte büyüyerek belirsin (her açılışta bir kez).
    let first = {
        let mut p = state.pop.lock().unwrap();
        let f = p.glass_serial != serial;
        p.glass_serial = serial;
        f
    };
    if first {
        let _ = app.run_on_main_thread(move || platform::glass_animate(hwnd, true, 240));
    }
    platform::show_no_activate(hwnd);
    if let Some(o) = state.pop.lock().unwrap().open.as_mut() {
        o.rect = Some((x, y, x + w, y + h));
    }
    let _ = app.emit_to(window.label(), "popover-in", serial);
    let _ = app.emit_to(dock.as_str(), "popover-opened", ());
    Ok(())

    }).await {
        Ok(r) => r,
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn popover_close(app: AppHandle) {
    let _ = run(move || {
        let _ = &app;

    close(&app);

    }).await;
}

/// Kapanış animasyonu bitti: (yeni bir panel açılmadıysa) pencereyi gizle.
#[tauri::command]
pub async fn popover_hidden(app: AppHandle, window: WebviewWindow) {
    let _ = run(move || {
        let state = app.state::<AppState>();

    let reopened = state
        .pop
        .lock()
        .unwrap()
        .open
        .as_ref()
        .map(|o| o.pop == window.label())
        .unwrap_or(false);
    if reopened {
        return;
    }
    let h = state.pop.lock().unwrap().windows.get(window.label()).copied();
    if let Some(h) = h {
        platform::hide(h);
    }

    }).await;
}

fn schedule_hover_close(app: AppHandle) {
    let serial = app.state::<AppState>().pop.lock().unwrap().serial;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(380));
        let state = app.state::<AppState>();
        let should = {
            let p = state.pop.lock().unwrap();
            p.serial == serial
                && !p.dock_hover
                && !p.pop_hover
                && p.open.as_ref().map(|o| o.kind == "windows").unwrap_or(false)
        };
        if should {
            close(&app);
        }
    });
}

/// Fare panelin üstünde mi (panel JS'i bildirir).
#[tauri::command]
pub async fn popover_hover(app: AppHandle, hover: bool) {
    let _ = run(move || {
        let _ = &app;

    app.state::<AppState>().pop.lock().unwrap().pop_hover = hover;
    if !hover {
        schedule_hover_close(app);
    }

    }).await;
}

/// Fare önizlemesi açılan uygulama simgesinin üstünde mi (dock JS'i bildirir).
#[tauri::command]
pub async fn popover_dock_hover(app: AppHandle, hover: bool) {
    let _ = run(move || {
        let _ = &app;

    app.state::<AppState>().pop.lock().unwrap().dock_hover = hover;
    if !hover {
        schedule_hover_close(app);
    }

    }).await;
}

#[derive(Deserialize)]
pub struct ThumbReq {
    pub hwnd: isize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Önizleme kartlarındaki kutulara canlı pencere görüntülerini yerleştirir
/// (CSS pikseli, panelin sol üstüne göre).
#[tauri::command]
pub async fn popover_thumbs(app: AppHandle, window: WebviewWindow, items: Vec<ThumbReq>, serial: u64) {
    let _ = run(move || {
        let state = app.state::<AppState>();

    clear_thumbs(&state);
    let (dock, ok) = {
        let p = state.pop.lock().unwrap();
        match &p.open {
            Some(o) => (o.dock.clone(), o.serial == serial && o.pop == window.label()),
            None => (String::new(), false),
        }
    };
    if !ok {
        return;
    }
    let scale = state.docks.lock().unwrap().get(&dock).map(|d| d.scale).unwrap_or(1.0);
    let Some(dest) = state.pop.lock().unwrap().windows.get(window.label()).copied() else {
        return;
    };
    let mut made = Vec::new();
    for t in items {
        let r = |v: f64| (v * scale).round() as i32;
        if let Some(th) = platform::thumb_show(dest, t.hwnd, r(t.x), r(t.y), r(t.w), r(t.h)) {
            made.push(th);
        }
    }
    state.pop.lock().unwrap().thumbs = made;

    }).await;
}

/// Önizleme kartına tıklama: pencereyi öne getir, paneli kapat.
#[tauri::command]
pub async fn popover_activate(app: AppHandle, hwnd: isize) {
    let _ = run(move || {
        let _ = &app;

    close(&app);
    platform::bring_front(hwnd);

    }).await;
}

/// İzleyici döngüsünden (100 ms) çağrılır: dışarı tıklama ya da başka uygulamaya
/// geçiş paneli kapatır.
pub fn watch(app: &AppHandle, fg: isize) {
    let state = app.state::<AppState>();
    let (open, pops) = {
        let p = state.pop.lock().unwrap();
        (p.open.clone(), p.windows.values().copied().collect::<Vec<_>>())
    };
    let Some(o) = open else { return };
    let Some(rect) = o.rect else { return };
    let dock_rect = state
        .docks
        .lock()
        .unwrap()
        .get(&o.dock)
        .and_then(|d| d.target)
        .map(|(x, y, w, h)| (x, y, x + w, y + h));
    let docks: Vec<isize> = state.docks.lock().unwrap().values().map(|d| d.hwnd).collect();

    let inside = |r: Rect, c: (i32, i32)| c.0 >= r.0 && c.0 < r.2 && c.1 >= r.1 && c.1 < r.3;
    let c = platform::cursor_pos();
    let outside_click = platform::mouse_down()
        && !inside(rect, c)
        && !dock_rect.map(|r| inside(r, c)).unwrap_or(false);
    let switched = fg != 0 && fg != o.fg_at_open && !docks.contains(&fg) && !pops.contains(&fg);
    if outside_click || switched {
        close(app);
    }
}
