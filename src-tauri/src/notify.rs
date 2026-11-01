//! Windows bildirimleri: dock'ta cam balon, uygulama simgesinde sayı rozeti
//! ve zıplama.
//!
//! Windows'un resmi bildirim dinleyicisi (UserNotificationListener) yalnızca
//! paketli (Store) uygulamalara açık. Bu yüzden Windows'un bildirim merkezini
//! tuttuğu veritabanını salt okunur açıyoruz:
//! %LOCALAPPDATA%\Microsoft\Windows\Notifications\wpndatabase.db
//! Bildirim merkezinde duran her bildirim ("toast") ve uygulama rozetleri
//! ("badge": WhatsApp/Teams'in okunmamış sayısı) burada.

use crate::commands::run;
use crate::{dlog, platform, AppState};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Toast {
    pub id: i64,
    /// Bildirimi gönderen uygulamanın kimliği (AUMID ya da Windows'un verdiği ad)
    pub app: String,
    pub title: String,
    pub body: String,
    /// Unix ms
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct AppNotifs {
    /// Uygulama rozeti: "3", "99+" ya da sayısız rozetler için "•"
    pub badge: Option<String>,
    /// Bildirim merkezinde duran bildirimlerin kimlikleri
    pub toasts: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Snapshot {
    pub ok: bool,
    pub error: String,
    pub apps: BTreeMap<String, AppNotifs>,
    /// Son bildirimler (en yeni sonda, en çok 30)
    pub recent: Vec<Toast>,
}

#[derive(Default)]
pub struct NotifState {
    pub snap: Snapshot,
    last_id: Option<i64>,
}

fn db_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let p = std::path::PathBuf::from(base).join("Microsoft\\Windows\\Notifications\\wpndatabase.db");
    p.is_file().then_some(p)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Bildirim XML'indeki <text> öğeleri (başlık, gövde satırları).
fn texts(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i + 5..];
        // "<text" ile başlayan başka etiketleri (textBox vb.) atla
        if !rest.starts_with('>') && !rest.starts_with(' ') && !rest.starts_with('/') {
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        if rest[..gt].ends_with('/') {
            rest = &rest[gt + 1..];
            continue;
        }
        rest = &rest[gt + 1..];
        let Some(end) = rest.find("</text>") else { break };
        let t = unescape(rest[..end].trim());
        if !t.is_empty() {
            out.push(t);
        }
        rest = &rest[end + 7..];
    }
    out
}

fn badge_value(xml: &str) -> Option<String> {
    let i = xml.find("value=")?;
    let rest = &xml[i + 6..];
    let q = rest.chars().next()?;
    let rest = &rest[1..];
    let end = rest.find(q)?;
    let v = rest[..end].trim();
    match v.parse::<u32>() {
        Ok(0) => None,
        Ok(n) if n > 99 => Some("99+".into()),
        Ok(n) => Some(n.to_string()),
        // "none" rozeti kaldırır; diğer simgeler (newMessage, attention…) nokta olur
        Err(_) if v == "none" || v.is_empty() => None,
        Err(_) => Some("•".into()),
    }
}

fn filetime_ms(ft: i64) -> i64 {
    ft / 10_000 - 11_644_473_600_000
}

struct Row {
    id: i64,
    kind: String,
    app: String,
    payload: String,
    arrival: i64,
}

fn read_rows() -> Result<Vec<Row>, String> {
    use rusqlite::{Connection, OpenFlags};
    let path = db_path().ok_or("Bildirim veritabanı bulunamadı")?;
    let uri = format!("file:{}?mode=ro", path.to_string_lossy().replace('\\', "/"));
    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Veritabanı açılamadı: {e}"))?;
    let _ = conn.busy_timeout(Duration::from_millis(400));
    let mut stmt = conn
        .prepare(
            "SELECT n.Id, n.Type, h.PrimaryId, n.Payload, n.ArrivalTime \
             FROM Notification n JOIN NotificationHandler h ON n.HandlerId = h.RecordId \
             WHERE n.Type IN ('toast', 'badge')",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let payload: Vec<u8> = match r.get_ref(3)? {
                rusqlite::types::ValueRef::Blob(b) => b.to_vec(),
                rusqlite::types::ValueRef::Text(t) => t.to_vec(),
                _ => Vec::new(),
            };
            Ok(Row {
                id: r.get(0)?,
                kind: r.get::<_, String>(1)?,
                app: r.get::<_, String>(2)?,
                payload: String::from_utf8_lossy(&payload).into_owned(),
                arrival: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Bir kez okur; değişiklik varsa dock'lara yayınlar.
fn poll(app: &AppHandle) {
    let state = app.state::<AppState>();
    let rows = match read_rows() {
        Ok(r) => r,
        Err(e) => {
            let mut n = state.notif.lock().unwrap();
            if n.snap.ok || n.snap.error != e {
                dlog!("bildirimler okunamadı: {e}");
                n.snap.ok = false;
                n.snap.error = e;
                let snap = n.snap.clone();
                drop(n);
                let _ = app.emit("notifications", serde_json::json!({ "snap": snap, "new": [] }));
            }
            return;
        }
    };
    let mut apps: BTreeMap<String, AppNotifs> = BTreeMap::new();
    let mut toasts: Vec<Toast> = Vec::new();
    for r in rows {
        match r.kind.as_str() {
            "badge" => {
                if let Some(v) = badge_value(&r.payload) {
                    apps.entry(r.app.clone()).or_default().badge = Some(v);
                }
            }
            _ => {
                let t = texts(&r.payload);
                apps.entry(r.app.clone()).or_default().toasts.push(r.id);
                toasts.push(Toast {
                    id: r.id,
                    app: r.app,
                    title: t.first().cloned().unwrap_or_default(),
                    body: t.iter().skip(1).cloned().collect::<Vec<_>>().join("\n"),
                    time: filetime_ms(r.arrival),
                });
            }
        }
    }
    toasts.sort_by_key(|t| t.id);
    let max_id = toasts.last().map(|t| t.id).unwrap_or(0);

    let (fresh, snap) = {
        let mut n = state.notif.lock().unwrap();
        // İlk okumada eski bildirimler yeni sayılmaz (açılışta zıplamasın).
        let since = n.last_id.unwrap_or(max_id);
        let now = chrono::Utc::now().timestamp_millis();
        let fresh: Vec<Toast> = toasts
            .iter()
            .filter(|t| t.id > since && (t.time == 0 || now - t.time < 120_000))
            .cloned()
            .collect();
        n.last_id = Some(since.max(max_id));
        let recent: Vec<Toast> = toasts.iter().rev().take(30).rev().cloned().collect();
        let changed = !n.snap.ok || n.snap.apps != apps || !fresh.is_empty() || n.snap.recent.len() != recent.len();
        n.snap = Snapshot { ok: true, error: String::new(), apps, recent };
        (fresh, changed.then(|| n.snap.clone()))
    };
    if let Some(snap) = snap {
        if !fresh.is_empty() {
            dlog!("{} yeni bildirim: {}", fresh.len(), fresh.iter().map(|t| t.app.as_str()).collect::<Vec<_>>().join(", "));
        }
        let _ = app.emit("notifications", serde_json::json!({ "snap": snap, "new": fresh }));
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        let enabled = app.state::<AppState>().cfg.lock().unwrap().notifications.enabled;
        if enabled {
            poll(&app);
        }
        std::thread::sleep(Duration::from_millis(if enabled { 1200 } else { 5000 }));
    });
}

#[tauri::command]
pub fn notif_state(app: AppHandle) -> Snapshot {
    app.state::<AppState>().notif.lock().unwrap().snap.clone()
}

/// Windows bildirim merkezindeki bir bildirimi açar (uygulamaya geçiş için).
#[tauri::command]
pub fn notif_open_center() {
    platform::win_combo('N');
}

// ---------------------------------------------------------------------------
// Cam balon penceresi ("toast-…"): dock'un üstünde, sağ tarafta
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ToastWin {
    /// etiket → hwnd
    pub windows: HashMap<String, isize>,
    /// etiket → (dock etiketi, çapa x [css px])
    pub anchor: HashMap<String, (String, f64)>,
    pub pending: HashMap<String, serde_json::Value>,
}

fn toast_label(dock: &str) -> String {
    format!("toast-{}", dock.trim_start_matches("dock-"))
}

/// Dock bir balon gösterilmesini ister. `anchor`: simgenin dock içindeki x'i (css px).
#[tauri::command]
pub async fn toast_show(app: AppHandle, window: WebviewWindow, anchor: f64, payload: serde_json::Value) -> Result<(), String> {
    let dock = window.label().to_string();
    run(move || {
        let state = app.state::<AppState>();
        let Some(mon) = state.docks.lock().unwrap().get(&dock).map(|d| d.mon_rect) else {
            return Ok(());
        };
        let label = toast_label(&dock);
        let existing = state.toast.lock().unwrap().windows.get(&label).copied();
        {
            let mut t = state.toast.lock().unwrap();
            t.anchor.insert(label.clone(), (dock.clone(), anchor));
            t.pending.insert(label.clone(), payload.clone());
        }
        if existing.is_none() {
            let h = crate::docks::create_window(&app, &label, mon, "toast.html").map_err(|e| e.to_string())?;
            state.toast.lock().unwrap().windows.insert(label.clone(), h);
            // Yeni sayfa yüklenince toast_pending ile alır.
            return Ok(());
        }
        let _ = app.emit_to(label.as_str(), "toast-add", payload);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn toast_pending(app: AppHandle, window: WebviewWindow) -> Option<serde_json::Value> {
    app.state::<AppState>().toast.lock().unwrap().pending.remove(window.label())
}

/// Balon içeriği ölçüldü: dock'un üstüne, simgenin hizasına yerleştir.
#[tauri::command]
pub async fn toast_layout(app: AppHandle, window: WebviewWindow, width: f64, height: f64) {
    let label = window.label().to_string();
    let _ = run(move || {
        let state = app.state::<AppState>();
        let Some((dock, anchor)) = state.toast.lock().unwrap().anchor.get(&label).cloned() else { return };
        let Some(hwnd) = state.toast.lock().unwrap().windows.get(&label).copied() else { return };
        let Some(d) = state.docks.lock().unwrap().get(&dock).cloned() else { return };
        let Some((dx, dy, _, _)) = d.target else { return };
        let s = d.scale;
        let w = (width * s).ceil() as i32;
        let h = (height * s).ceil() as i32;
        let m = d.mon_rect;
        let edge = (10.0 * s) as i32;
        let cx = dx + (anchor * s) as i32;
        let x = (cx - w / 2).clamp(m.0 + edge, (m.2 - w - edge).max(m.0 + edge));
        // Dock gizliyse ekranın altına yakın dur.
        let base = if d.shown { dy } else { m.3 - (10.0 * s) as i32 };
        let y = base - (12.0 * s) as i32 - h;
        platform::set_bounds(hwnd, x, y, w.max(1), h.max(1));
        let mode = state.cfg.lock().unwrap().backdrop.clone();
        crate::docks::glass_raw(&app, hwnd, w, h, (18.0 * s) as f32, d.dark, d.tint, mode);
        if !platform::is_visible(hwnd) {
            let _ = app.run_on_main_thread(move || platform::glass_animate(hwnd, true, 260));
        }
        platform::show_no_activate(hwnd);
    })
    .await;
}

/// Balon kapanmaya başladı: cam da küçülerek sönsün.
#[tauri::command]
pub fn toast_fade(app: AppHandle, window: WebviewWindow) {
    let h = app.state::<AppState>().toast.lock().unwrap().windows.get(window.label()).copied();
    if let Some(h) = h {
        let _ = app.run_on_main_thread(move || platform::glass_animate(h, false, 220));
    }
}

#[tauri::command]
pub async fn toast_hide(app: AppHandle, window: WebviewWindow) {
    let label = window.label().to_string();
    let _ = run(move || {
        let h = app.state::<AppState>().toast.lock().unwrap().windows.get(&label).copied();
        if let Some(h) = h {
            platform::hide(h);
        }
    })
    .await;
}

/// Balona tıklandı: bildirimin geldiği dock'a ilet (uygulamayı öne getirsin).
#[tauri::command]
pub fn toast_click(app: AppHandle, window: WebviewWindow, key: String) {
    let dock = app.state::<AppState>().toast.lock().unwrap().anchor.get(window.label()).map(|a| a.0.clone());
    if let Some(dock) = dock {
        let _ = app.emit_to(dock.as_str(), "toast-click", key);
    }
}
