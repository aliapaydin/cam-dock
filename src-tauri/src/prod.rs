//! Verimlilik widget'ları: yapışkan notlar (masaüstüne bırakılabilir),
//! yapılacaklar listesi ve odak (Pomodoro) zamanlayıcısı.
//! Notlar ve görevler %APPDATA%\com.ali.camdock\notes.json'da durur.

use crate::{commands::run, config, dlog, AppState};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn new_id() -> String {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    format!("{:x}{:x}", now_ms(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

// ---------------------------------------------------------------------------
// Depo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Note {
    pub id: String,
    pub text: String,
    /// "yellow" | "pink" | "blue" | "green" | "purple" | "orange" | "gray"
    pub color: String,
    /// Masaüstünde açık mı
    pub desktop: bool,
    /// Masaüstündeyken diğer pencerelerin üstünde dursun
    pub top: bool,
    /// Masaüstündeki yeri ve boyutu (fiziksel piksel). w = 0: henüz yerleşmedi
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub created: u64,
    pub updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub created: u64,
    pub done_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Store {
    notes: Vec<Note>,
    todos: Vec<Todo>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn path() -> std::path::PathBuf {
    config::base_dir().join("notes.json")
}

fn with<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(|| {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    });
    f(s)
}

fn save() {
    let text = with(|s| serde_json::to_string_pretty(s).unwrap_or_default());
    if !text.is_empty() {
        let tmp = path().with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path());
        }
    }
}

fn notes() -> Vec<Note> {
    with(|s| s.notes.clone())
}

fn emit_notes(app: &AppHandle) {
    let _ = app.emit("notes-changed", notes());
}

fn emit_todos(app: &AppHandle) {
    let _ = app.emit("todos-changed", with(|s| s.todos.clone()));
}

// ---------------------------------------------------------------------------
// Notlar
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn notes_list() -> Vec<Note> {
    let mut n = notes();
    n.sort_by(|a, b| b.updated.cmp(&a.updated));
    n
}

/// Yeni not. `desktop`: hemen masaüstüne de bırak.
#[tauri::command]
pub async fn note_new(app: AppHandle, text: Option<String>, color: Option<String>, desktop: Option<bool>) -> Result<Note, String> {
    run(move || {
        let t = now_ms();
        let colors = ["yellow", "pink", "blue", "green", "purple", "orange"];
        let n = Note {
            id: new_id(),
            text: text.unwrap_or_default(),
            color: color.unwrap_or_else(|| colors[with(|s| s.notes.len()) % colors.len()].into()),
            desktop: desktop.unwrap_or(false),
            created: t,
            updated: t,
            ..Default::default()
        };
        with(|s| s.notes.push(n.clone()));
        save();
        if n.desktop {
            open_window(&app, &n);
        }
        emit_notes(&app);
        n
    })
    .await
}

/// Notun metnini, rengini ya da "üstte tut" ayarını değiştirir.
#[tauri::command]
pub async fn note_update(app: AppHandle, id: String, text: Option<String>, color: Option<String>, top: Option<bool>) -> Result<(), String> {
    run(move || {
        let changed = with(|s| {
            let Some(n) = s.notes.iter_mut().find(|n| n.id == id) else { return None };
            if let Some(t) = text {
                n.text = t;
            }
            if let Some(c) = color {
                n.color = c;
            }
            if let Some(t) = top {
                n.top = t;
            }
            n.updated = now_ms();
            Some(n.clone())
        });
        let Some(n) = changed else { return };
        save();
        if let Some(w) = app.get_webview_window(&label(&n.id)) {
            let _ = w.set_always_on_top(n.top);
        }
        emit_notes(&app);
    })
    .await
}

#[tauri::command]
pub async fn note_delete(app: AppHandle, id: String) -> Result<(), String> {
    run(move || {
        with(|s| s.notes.retain(|n| n.id != id));
        save();
        close_window(&app, &id);
        emit_notes(&app);
    })
    .await
}

/// Notu masaüstüne bırakır ya da masaüstünden kaldırır (not silinmez).
#[tauri::command]
pub async fn note_desktop(app: AppHandle, id: String, on: bool) -> Result<(), String> {
    run(move || set_desktop(&app, &id, on)).await
}

fn set_desktop(app: &AppHandle, id: &str, on: bool) {
    let n = with(|s| {
        let n = s.notes.iter_mut().find(|n| n.id == id)?;
        n.desktop = on;
        Some(n.clone())
    });
    let Some(n) = n else { return };
    save();
    if on {
        open_window(app, &n);
    } else {
        close_window(app, id);
    }
    emit_notes(app);
}

/// Tüm notları masaüstünde göster ya da hepsini gizle.
#[tauri::command]
pub async fn notes_desktop_all(app: AppHandle, on: bool) -> Result<(), String> {
    run(move || {
        for n in notes() {
            if n.desktop != on {
                set_desktop(&app, &n.id, on);
            }
        }
    })
    .await
}

// ---------------------------------------------------------------------------
// Masaüstündeki not pencereleri
// ---------------------------------------------------------------------------

fn label(id: &str) -> String {
    format!("note-{id}")
}

fn id_of(w: &WebviewWindow) -> String {
    w.label().trim_start_matches("note-").to_string()
}

/// Not pencerelerinin tutamaçları (dock'un pencere listesinden çıkarmak için).
pub fn note_hwnds(app: &AppHandle) -> Vec<isize> {
    #[cfg(windows)]
    {
        app.webview_windows()
            .into_iter()
            .filter(|(l, _)| l.starts_with("note-"))
            .filter_map(|(_, w)| w.hwnd().ok().map(|h| h.0 as isize))
            .collect()
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Vec::new()
    }
}

/// Açılışta masaüstündeki notları geri getirir.
pub fn restore(app: &AppHandle) {
    for n in notes().into_iter().filter(|n| n.desktop) {
        open_window(app, &n);
    }
}

fn open_window(app: &AppHandle, n: &Note) {
    let lbl = label(&n.id);
    if let Some(w) = app.get_webview_window(&lbl) {
        let _ = w.emit("note-flash", ());
        return;
    }
    let built = WebviewWindowBuilder::new(app, &lbl, WebviewUrl::App("note.html".into()))
        .title("Not")
        .inner_size(260.0, 250.0)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(n.top)
        .visible(false)
        .focused(false)
        .build();
    let w = match built {
        Ok(w) => w,
        Err(e) => {
            dlog!("not penceresi açılamadı: {e}");
            return;
        }
    };
    // İlk kez masaüstüne bırakılıyorsa ana ekranın sağ üstüne, öncekilerin yanına diz.
    let (x, y, ww, hh) = if n.w > 0 {
        (n.x, n.y, n.w, n.h)
    } else {
        let scale = w.scale_factor().unwrap_or(1.0);
        let (ww, hh) = ((260.0 * scale) as i32, (250.0 * scale) as i32);
        let (wx, wy, wwid, _) = work_area(&w);
        let k = notes().iter().filter(|o| o.desktop && o.w > 0).count() as i32;
        let x = wx + wwid - ww - (16.0 * scale) as i32 - (k % 4) * (ww - (20.0 * scale) as i32);
        let y = wy + (16.0 * scale) as i32 + (k / 4) * (hh - (10.0 * scale) as i32);
        with(|s| {
            if let Some(m) = s.notes.iter_mut().find(|o| o.id == n.id) {
                (m.x, m.y, m.w, m.h) = (x, y, ww, hh);
            }
        });
        save();
        (x, y, ww, hh)
    };
    let (x, y) = keep_on_screen(app, x, y, ww, hh);
    let _ = w.set_size(PhysicalSize::new(ww.max(120) as u32, hh.max(100) as u32));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    #[cfg(windows)]
    if let Ok(h) = w.hwnd() {
        let h = h.0 as isize;
        // Alt+Tab ve dock'un pencere listesinde görünmesin (araç penceresi)
        let _ = app.run_on_main_thread(move || {
            crate::platform::make_popup(h);
        });
    }
    // Sayfa çizilince note_ready ile animasyonla görünür.
}

/// Ekran düzeni değiştiyse (monitör çıkarıldı) not ekran dışında kalmasın.
fn keep_on_screen(app: &AppHandle, x: i32, y: i32, w: i32, _h: i32) -> (i32, i32) {
    let Ok(mons) = app.available_monitors() else { return (x, y) };
    let visible = mons.iter().any(|m| {
        let (p, s) = (m.position(), m.size());
        x + w / 2 >= p.x && x + w / 2 < p.x + s.width as i32 && y + 20 >= p.y && y + 20 < p.y + s.height as i32
    });
    if visible || mons.is_empty() {
        return (x, y);
    }
    let m = app.primary_monitor().ok().flatten().unwrap_or_else(|| mons[0].clone());
    let wa = m.work_area();
    (wa.position.x + wa.size.width as i32 - w - 24, wa.position.y + 24)
}

fn close_window(app: &AppHandle, id: &str) {
    if let Some(w) = app.get_webview_window(&label(id)) {
        // Sayfa kapanış animasyonunu oynatsın, sonra pencere kapansın.
        let _ = w.emit("note-close", ());
        let w2 = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(260));
            let _ = w2.close();
        });
    }
}

/// (x, y, genişlik, yükseklik) — pencerenin bulunduğu ekranın çalışma alanı.
fn work_area(w: &WebviewWindow) -> (i32, i32, i32, i32) {
    let m = w.current_monitor().ok().flatten().or_else(|| w.primary_monitor().ok().flatten());
    match m {
        Some(m) => {
            let a = m.work_area();
            (a.position.x, a.position.y, a.size.width as i32, a.size.height as i32)
        }
        None => (0, 0, 1920, 1040),
    }
}

/// Not penceresinin sayfası: kendi notunu ister.
#[tauri::command]
pub fn note_self(window: WebviewWindow) -> Option<Note> {
    let id = id_of(&window);
    notes().into_iter().find(|n| n.id == id)
}

/// Sayfa çizildi: pencereyi odak çalmadan göster.
#[tauri::command]
pub fn note_ready(window: WebviewWindow) {
    #[cfg(windows)]
    if let Ok(h) = window.hwnd() {
        let h = h.0 as isize;
        let app = window.app_handle().clone();
        // Odağı çalmadan göster (açılışta yazdığın pencereden klavyeyi almasın)
        let _ = app.run_on_main_thread(move || crate::platform::show_no_activate(h));
        return;
    }
    let _ = window.show();
}

static DRAG: Mutex<Option<(String, i32, i32)>> = Mutex::new(None);

#[tauri::command]
pub fn note_drag_start(window: WebviewWindow) {
    if let Ok(p) = window.outer_position() {
        *DRAG.lock().unwrap() = Some((window.label().to_string(), p.x, p.y));
    }
}

/// Sürüklerken: başlangıca göre fark (fiziksel piksel).
#[tauri::command]
pub fn note_drag(window: WebviewWindow, dx: i32, dy: i32) {
    let d = DRAG.lock().unwrap().clone();
    if let Some((l, x, y)) = d {
        if l == window.label() {
            let _ = window.set_position(PhysicalPosition::new(x + dx, y + dy));
        }
    }
}

/// Bırakınca: ekran kenarına ya da köşesine yakınsa oraya yaslanarak oturur.
#[tauri::command]
pub fn note_drag_end(window: WebviewWindow) {
    *DRAG.lock().unwrap() = None;
    let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) else { return };
    let (ax, ay, aw, ah) = work_area(&window);
    let scale = window.scale_factor().unwrap_or(1.0);
    // Pencerenin içinde gölge için 14px boşluk var; kâğıt kenara 8px kalsın.
    let pad = (6.0 * scale) as i32;
    let th = (36.0 * scale) as i32;
    let (w, h) = (s.width as i32, s.height as i32);
    let mut x = p.x;
    let mut y = p.y;
    if (x - ax).abs() < th || x < ax {
        x = ax - pad;
    }
    if ((ax + aw) - (x + w)).abs() < th || x + w > ax + aw {
        x = ax + aw - w + pad;
    }
    if (y - ay).abs() < th || y < ay {
        y = ay - pad;
    }
    if ((ay + ah) - (y + h)).abs() < th || y + h > ay + ah {
        y = ay + ah - h + pad;
    }
    let id = id_of(&window);
    let (fx, fy) = (p.x, p.y);
    if (x, y) != (fx, fy) {
        // Yumuşak yaslanma
        let w2 = window.clone();
        std::thread::spawn(move || {
            for i in 1..=10 {
                let t = i as f64 / 10.0;
                let e = 1.0 - (1.0 - t).powi(3);
                let nx = fx + ((x - fx) as f64 * e).round() as i32;
                let ny = fy + ((y - fy) as f64 * e).round() as i32;
                let _ = w2.set_position(PhysicalPosition::new(nx, ny));
                std::thread::sleep(Duration::from_millis(14));
            }
        });
    }
    with(|st| {
        if let Some(n) = st.notes.iter_mut().find(|n| n.id == id) {
            (n.x, n.y, n.w, n.h) = (x, y, w, h);
        }
    });
    save();
}

/// Sağ alt köşeden boyutlandırma (fiziksel piksel).
#[tauri::command]
pub fn note_resize(window: WebviewWindow, w: i32, h: i32, done: bool) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let (w, h) = (w.clamp((170.0 * scale) as i32, (720.0 * scale) as i32), h.clamp((150.0 * scale) as i32, (720.0 * scale) as i32));
    let _ = window.set_size(PhysicalSize::new(w as u32, h as u32));
    if done {
        let id = id_of(&window);
        with(|st| {
            if let Some(n) = st.notes.iter_mut().find(|n| n.id == id) {
                (n.w, n.h) = (w, h);
            }
        });
        save();
    }
}

// ---------------------------------------------------------------------------
// Yapılacaklar
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn todos_list() -> Vec<Todo> {
    with(|s| s.todos.clone())
}

#[tauri::command]
pub async fn todo_add(app: AppHandle, text: String) -> Result<(), String> {
    run(move || {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        with(|s| {
            // Yeni görev, tamamlanmamışların en sonuna
            let at = s.todos.iter().position(|t| t.done).unwrap_or(s.todos.len());
            s.todos.insert(at, Todo { id: new_id(), text, done: false, created: now_ms(), done_at: 0 });
        });
        save();
        emit_todos(&app);
    })
    .await
}

#[tauri::command]
pub async fn todo_toggle(app: AppHandle, id: String) -> Result<(), String> {
    run(move || {
        with(|s| {
            let Some(i) = s.todos.iter().position(|t| t.id == id) else { return };
            let mut t = s.todos.remove(i);
            t.done = !t.done;
            t.done_at = if t.done { now_ms() } else { 0 };
            // Tamamlanan alta iner; geri alınan tamamlanmamışların sonuna döner
            let at = s.todos.iter().position(|o| o.done).unwrap_or(s.todos.len());
            s.todos.insert(at, t);
        });
        save();
        emit_todos(&app);
    })
    .await
}

#[tauri::command]
pub async fn todo_edit(app: AppHandle, id: String, text: String) -> Result<(), String> {
    run(move || {
        with(|s| {
            if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                t.text = text.trim().to_string();
            }
            s.todos.retain(|t| !t.text.is_empty());
        });
        save();
        emit_todos(&app);
    })
    .await
}

#[tauri::command]
pub async fn todo_delete(app: AppHandle, id: String) -> Result<(), String> {
    run(move || {
        with(|s| s.todos.retain(|t| t.id != id));
        save();
        emit_todos(&app);
    })
    .await
}

#[tauri::command]
pub async fn todo_clear_done(app: AppHandle) -> Result<(), String> {
    run(move || {
        with(|s| s.todos.retain(|t| !t.done));
        save();
        emit_todos(&app);
    })
    .await
}

/// Sürükleyerek sıralama: görevlerin yeni sırası (kimlikler).
#[tauri::command]
pub async fn todo_order(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    run(move || {
        with(|s| {
            let mut out: Vec<Todo> = ids.iter().filter_map(|id| s.todos.iter().find(|t| &t.id == id).cloned()).collect();
            for t in &s.todos {
                if !ids.contains(&t.id) {
                    out.push(t.clone());
                }
            }
            s.todos = out;
        });
        save();
        emit_todos(&app);
    })
    .await
}

// ---------------------------------------------------------------------------
// Odak (Pomodoro)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct FocusState {
    /// "idle" | "focus" | "short" | "long"
    pub phase: String,
    pub running: bool,
    /// Çalışırken bitiş zamanı (epoch ms)
    pub ends_at: u64,
    /// Duraklatılmışken kalan süre (ms)
    pub left_ms: u64,
    pub total_ms: u64,
    /// Bu döngüde tamamlanan odak turu
    pub round: u32,
    pub rounds: u32,
    /// Bugün tamamlanan odak turu ve dakikası
    pub today: u32,
    pub today_min: u32,
}

static FOCUS: Mutex<Option<FocusState>> = Mutex::new(None);
static FOCUS_DAY: Mutex<(u64, u32, u32)> = Mutex::new((0, 0, 0));

fn day_key() -> u64 {
    // Yerel gün: Türkiye için +3 saat yeterince doğru; yalnızca "bugün" sayacı.
    (now_ms() / 1000 + 3 * 3600) / 86400
}

fn focus_cfg(app: &AppHandle) -> config::Focus {
    app.state::<AppState>().cfg.lock().unwrap().focus.clone()
}

fn phase_ms(c: &config::Focus, phase: &str) -> u64 {
    let m = match phase {
        "short" => c.short,
        "long" => c.long,
        _ => c.work,
    };
    m.clamp(1, 180) as u64 * 60_000
}

fn snapshot(app: &AppHandle) -> FocusState {
    let c = focus_cfg(app);
    let mut g = FOCUS.lock().unwrap();
    let s = g.get_or_insert_with(|| FocusState { phase: "idle".into(), ..Default::default() });
    s.rounds = c.rounds.max(1);
    if s.phase == "idle" {
        s.total_ms = phase_ms(&c, "focus");
        s.left_ms = s.total_ms;
    }
    let d = FOCUS_DAY.lock().unwrap();
    let mut out = s.clone();
    if d.0 == day_key() {
        out.today = d.1;
        out.today_min = d.2;
    }
    out
}

fn emit_focus(app: &AppHandle) {
    let _ = app.emit("focus-state", snapshot(app));
}

#[tauri::command]
pub fn focus_state(app: AppHandle) -> FocusState {
    snapshot(&app)
}

/// "start" | "pause" | "toggle" | "skip" | "reset" | "break" (hemen mola)
#[tauri::command]
pub fn focus_cmd(app: AppHandle, action: String) {
    let c = focus_cfg(&app);
    {
        let mut g = FOCUS.lock().unwrap();
        let s = g.get_or_insert_with(|| FocusState { phase: "idle".into(), ..Default::default() });
        let now = now_ms();
        let action = if action == "toggle" {
            if s.running { "pause" } else { "start" }.to_string()
        } else {
            action
        };
        match action.as_str() {
            "start" => {
                if s.phase == "idle" {
                    s.phase = "focus".into();
                    s.total_ms = phase_ms(&c, "focus");
                    s.left_ms = s.total_ms;
                }
                if !s.running {
                    s.running = true;
                    s.ends_at = now + s.left_ms.max(1000);
                }
            }
            "pause" => {
                if s.running {
                    s.running = false;
                    s.left_ms = s.ends_at.saturating_sub(now);
                }
            }
            "skip" => {
                drop(g);
                advance(&app, false);
                emit_focus(&app);
                return;
            }
            "break" => {
                s.phase = "short".into();
                s.total_ms = phase_ms(&c, "short");
                s.left_ms = s.total_ms;
                s.running = true;
                s.ends_at = now + s.total_ms;
            }
            _ => {
                *s = FocusState { phase: "idle".into(), ..Default::default() };
            }
        }
    }
    ensure_thread(&app);
    emit_focus(&app);
}

/// Aşama bitti (ya da atlandı): sıradakine geç.
fn advance(app: &AppHandle, finished: bool) {
    let c = focus_cfg(app);
    let (done_phase, next) = {
        let mut g = FOCUS.lock().unwrap();
        let Some(s) = g.as_mut() else { return };
        let done = s.phase.clone();
        let next = if done == "focus" {
            s.round += 1;
            if finished {
                let mut d = FOCUS_DAY.lock().unwrap();
                if d.0 != day_key() {
                    *d = (day_key(), 0, 0);
                }
                d.1 += 1;
                d.2 += (s.total_ms / 60_000) as u32;
            }
            if s.round >= c.rounds.max(1) {
                s.round = 0;
                "long"
            } else {
                "short"
            }
        } else {
            "focus"
        };
        s.phase = next.into();
        s.total_ms = phase_ms(&c, next);
        s.left_ms = s.total_ms;
        s.running = c.auto || !finished;
        if !finished {
            s.running = false;
        }
        if s.running {
            s.ends_at = now_ms() + s.total_ms;
        }
        (done, next.to_string())
    };
    if finished {
        dlog!("odak: {done_phase} bitti, sıradaki {next}");
        let _ = app.emit("focus-done", serde_json::json!({ "done": done_phase, "next": next, "auto": c.auto, "sound": c.sound }));
    }
}

fn ensure_thread(app: &AppHandle) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    let app = app.clone();
    STARTED.call_once(move || {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(400));
            let due = {
                let g = FOCUS.lock().unwrap();
                g.as_ref().map(|s| s.running && now_ms() >= s.ends_at).unwrap_or(false)
            };
            if due {
                advance(&app, true);
                emit_focus(&app);
            }
        });
    });
}

#[tauri::command]
pub async fn set_focus_cfg(app: AppHandle, focus: config::Focus) -> Result<(), String> {
    run(move || {
        {
            let state = app.state::<AppState>();
            let mut cfg = state.cfg.lock().unwrap();
            cfg.focus = focus;
            config::save(&cfg);
        }
        crate::emit_config(&app);
        // Boştaysa yeni süre hemen görünsün
        emit_focus(&app);
    })
    .await
}

#[tauri::command]
pub async fn set_world_clocks(app: AppHandle, clocks: Vec<config::WorldClock>) -> Result<(), String> {
    run(move || {
        {
            let state = app.state::<AppState>();
            let mut cfg = state.cfg.lock().unwrap();
            cfg.world_clocks = clocks.into_iter().filter(|c| !c.tz.is_empty()).take(8).collect();
            config::save(&cfg);
        }
        crate::emit_config(&app);
    })
    .await
}
