//! Dock pencereleri: her monitöre bir dock, klasik taskbar'ın gizlenmesi ve
//! dock'un görünürlük modları (her zaman / otomatik gizle / akıllı gizle).

use crate::config::{self, Config};
#[allow(unused_imports)]
use tauri::Emitter as _;
pub use crate::platform::Rect;
use crate::platform;
use crate::{dlog, AppState};
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
    /// Arayüzün bildirdiği tema (cam renk tabakası için)
    pub dark: bool,
    /// Dinamik modda duvar kâğıdından gelen cam rengi
    pub tint: Option<(u8, u8, u8)>,
    /// Pencerenin dock'un üstünde büyüteç için ayırdığı boşluk (fiziksel px).
    /// `target` dock'un kendi dikdörtgenidir; pencere bu kadar yukarıdan başlar.
    pub head: i32,
    /// Fare bu dock'un ekranında mı (değilken animasyonlar durur)
    pub cursor_here: bool,
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

pub fn monitors(_app: &AppHandle) -> Vec<MonitorView> {
    // Doğrudan Windows'tan oku: Tauri'nin monitör API'si ana iş parçacığına gidip
    // cevap bekliyor; arka plan döngüsünü ona bağlamak donmalara yol açıyordu.
    let mut list: Vec<MonitorView> = platform::monitor_list()
        .into_iter()
        .map(|(name, rect, scale, primary)| MonitorView {
            title: String::new(),
            width: (rect.2 - rect.0).max(0) as u32,
            height: (rect.3 - rect.1).max(0) as u32,
            scale,
            primary,
            rect,
            name,
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

pub fn create_window(app: &AppHandle, label: &str, mon: Rect, page: &str) -> tauri::Result<isize> {
    let (w, h) = if page == "index.html" { (1600.0, 84.0) } else { (640.0, 420.0) };
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title("Cam Dock")
        .inner_size(w, h)
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
    // Görünmeden önce ekranın altına (dışına) taşı: ilk gösterimde aşağıdan
    // kayarak gelsin, üstte belirip inmesin.
    forget_glass(hwnd);
    platform::move_to(hwnd, mon.0, mon.3 + 40);
    // Stil, pencere işleyicisi ve cam katmanı pencerenin iş parçacığında kurulmalı.
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || {
        platform::make_popup(hwnd);
        let proc_ok = platform::install_dock_proc(hwnd);
        let glass = platform::glass_attach(hwnd);
        dlog!(
            "{label}: pencere işleyicisi {}, cam katmanı {}",
            if proc_ok { "takıldı" } else { "TAKILAMADI" },
            match &glass {
                Ok(()) => "kuruldu".to_string(),
                Err(e) => format!("KURULAMADI ({e}) — eski yönteme düşülecek"),
            }
        );
    });
    Ok(hwnd)
}

/// Cam katmanını dock'un güncel boyutu, köşe yarıçapı ve temasına göre uygular.
pub fn apply_glass(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    let (radius_css, mode) = {
        let cfg = state.cfg.lock().unwrap();
        (cfg.radius, cfg.backdrop.clone())
    };
    let Some(d) = state.docks.lock().unwrap().get(label).cloned() else {
        return;
    };
    let Some((_, _, w, h)) = d.target else { return };
    let radius = (radius_css as f64 * d.scale) as f32;
    glass_raw(app, d.hwnd, w, h, radius, d.dark, d.tint, mode, d.head);
    // Üstteki büyüteç boşluğu tıklamaları alttaki pencerelere geçirsin.
    if d.head > 0 {
        platform::rect_region(d.hwnd, &[(0, d.head, w, d.head + h)]);
    }
}

/// Herhangi bir penceremize cam uygular (dock ya da panel). Composition
/// kurulamadıysa eski yönteme (accent + pencere bölgesi) düşer.
pub fn glass_raw(app: &AppHandle, hwnd: isize, w: i32, h: i32, radius: f32, dark: bool, tint: Option<(u8, u8, u8)>, mode: String, top: i32) {
    let tint = match (tint, dark) {
        (Some((r, g, b)), true) => (r, g, b, 0x40),
        (Some((r, g, b)), false) => (r, g, b, 0x50),
        (None, true) => (16, 20, 34, 0x38),
        (None, false) => (245, 247, 252, 0x48),
    };
    // Aynı değerlerle tekrar uygulama: her yerleşimde/tema geçişinde cam ve DWM
    // arka planı baştan kuruluyor, pencere bir an yanıp sönüyordu.
    let key = format!("{w}x{h}+{top}r{radius:.1}t{:?}d{dark}m{mode}", tint);
    {
        let mut cache = GLASS_KEY.lock().unwrap();
        if cache.get(&hwnd) == Some(&key) {
            return;
        }
        cache.insert(hwnd, key);
    }
    let _ = app.run_on_main_thread(move || {
        if platform::glass_attached(hwnd) {
            // DWM arka planını yalnızca tema/mod değişince kapat (pahalı ve görünür).
            let bd = format!("{dark}{mode}");
            let first = {
                let mut m = BACKDROP_KEY.lock().unwrap();
                m.insert(hwnd, bd.clone()) != Some(bd)
            };
            if first {
                platform::apply_backdrop(hwnd, "none", dark);
            }
            if let Err(e) = platform::glass_update(hwnd, w as f32, h as f32, radius, tint, mode != "none", top as f32) {
                dlog!("cam güncellenemedi: {e}");
            }
        } else {
            platform::apply_backdrop(hwnd, &mode, dark);
            platform::round_region(hwnd, w, h, radius as i32, top);
        }
    });
}

static GLASS_KEY: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<isize, String>>> =
    std::sync::LazyLock::new(Default::default);
static BACKDROP_KEY: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<isize, String>>> =
    std::sync::LazyLock::new(Default::default);

/// Pencere yeniden oluşturulunca önbelleği unut.
pub fn forget_glass(hwnd: isize) {
    GLASS_KEY.lock().unwrap().remove(&hwnd);
    BACKDROP_KEY.lock().unwrap().remove(&hwnd);
}

/// Geçerli temayı (Windows teması, dinamik moddaki saat planı) yeniden
/// hesaplar; değiştiyse tüm pencerelere "theme-dark" yayınlar.
pub fn refresh_theme(app: &AppHandle) {
    let state = app.state::<AppState>();
    let sys = *state.system_light.lock().unwrap();
    let cfg = state.cfg.lock().unwrap().clone();
    let dark = crate::theme::effective_dark(&cfg, sys);
    let mut last = state.eff_dark.lock().unwrap();
    if *last != Some(dark) {
        let first = last.is_none();
        *last = Some(dark);
        drop(last);
        if !first {
            dlog!("tema değişti: {}", if dark { "koyu" } else { "açık" });
        }
        let _ = app.emit("theme-dark", dark);
    }
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
        dlog!("{}: dock kaldırılıyor (ekran ayarı/çıkarıldı)", d.label);
        platform::release(d.hwnd);
        let h = d.hwnd;
        let _ = app.run_on_main_thread(move || platform::glass_detach(h));
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
        match create_window(app, &label, m.rect, "index.html") {
            Ok(hwnd) => {
                dlog!(
                    "{label}: oluşturuldu, ekran {} {:?} ölçek {}",
                    m.name, m.rect, m.scale
                );
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
                        dark: true,
                        tint: None,
                        head: 0,
                        cursor_here: true,
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
    let _ = app.run_on_main_thread(|| {
        let ok = platform::install_taskbar_hook();
        dlog!("taskbar kancası {}", if ok { "kuruldu" } else { "KURULAMADI" });
    });
    let mut prev = state.taskbar_prev.lock().unwrap();
    if prev.is_none() {
        // Önceki bir çalıştırma çöktüyse asıl orijinal durum dosyadadır.
        let original = guard_read().unwrap_or(current);
        let _ = std::fs::write(guard_path(), original.to_string());
        *prev = Some(original);
    }
}

pub fn restore_taskbar(state: &AppState) {
    // Kilidi Explorer'la konuşmadan önce bırak.
    let prev = state.taskbar_prev.lock().unwrap().take();
    if let Some(prev) = prev {
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

/// Dock penceresinin tek "hareket ettiricisi". Yerleşim (genişlik/x) ve kayma
/// animasyonu aynı pencereyi aynı anda taşıdığında titreme ve sağa/sola kayık
/// kalma oluyordu. Artık:
///  - yerleşim yalnızca hedef geometriyi günceller; animasyon sürüyorsa her
///    karede en güncel x/genişlik okunur,
///  - yeni bir animasyon eskisini nesil sayacıyla iptal eder,
///  - pencere hiçbir zaman iki iş parçacığından aynı anda taşınmaz.
#[derive(Clone, Copy)]
struct Geo {
    x: i32,
    w: i32,
    h: i32,
    /// Pencerenin şu anki y konumu (en son uygulanan)
    y: i32,
    gen: u64,
    moving: bool,
}

static GEO: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<isize, Geo>>> =
    std::sync::LazyLock::new(Default::default);

/// Yerleşimden gelen yeni geometri. Animasyon sürmüyorsa pencereyi hemen
/// `y` konumuna yerleştirir; sürüyorsa animasyon bir sonraki karede alır.
pub fn place(hwnd: isize, x: i32, y: i32, w: i32, h: i32) {
    let apply = {
        let mut map = GEO.lock().unwrap();
        let g = map.entry(hwnd).or_insert(Geo { x, w, h, y, gen: 0, moving: false });
        let same = g.x == x && g.w == w && g.h == h && g.y == y;
        g.x = x;
        g.w = w;
        g.h = h;
        if g.moving || same {
            false
        } else {
            g.y = y;
            true
        }
    };
    if apply {
        platform::set_bounds(hwnd, x, y, w.max(1), h.max(1));
    }
}

/// Animasyonu iptal eder (tam ekranda gizleme gibi anlık işlemler için).
fn cancel(hwnd: isize, y_now: i32) {
    let mut map = GEO.lock().unwrap();
    if let Some(g) = map.get_mut(&hwnd) {
        g.gen += 1;
        g.moving = false;
        g.y = y_now;
    }
}

/// `to` konumuna yumuşak kayma. Başka bir animasyon başlarsa erken döner ve
/// `false` verir.
fn animate(hwnd: isize, to: i32, ms: u64) -> bool {
    let (my_gen, from) = {
        let mut map = GEO.lock().unwrap();
        let Some(g) = map.get_mut(&hwnd) else { return false };
        g.gen += 1;
        g.moving = true;
        (g.gen, g.y)
    };
    let start = Instant::now();
    let total = Duration::from_millis(ms);
    loop {
        let t = (start.elapsed().as_secs_f64() / total.as_secs_f64()).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        let y = from + ((to - from) as f64 * ease).round() as i32;
        let geo = {
            let mut map = GEO.lock().unwrap();
            let Some(g) = map.get_mut(&hwnd) else { return false };
            if g.gen != my_gen {
                return false;
            }
            g.y = y;
            if t >= 1.0 {
                g.moving = false;
            }
            *g
        };
        platform::set_bounds(hwnd, geo.x, y, geo.w.max(1), geo.h.max(1));
        if t >= 1.0 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

/// DİKKAT: kayma fonksiyonları hiçbir kilit tutulmadan çağrılmalı.
pub fn slide_in(d: &Dock) {
    let Some((x, y, w, h)) = d.target else { return };
    // Pencere, dock'un üstündeki büyüteç boşluğu kadar yukarıdan başlar.
    let (y, h) = (y - d.head, h + d.head);
    let off = d.mon_rect.3;
    {
        let mut map = GEO.lock().unwrap();
        let g = map.entry(d.hwnd).or_insert(Geo { x, w, h, y: off, gen: 0, moving: false });
        // Gizliyse ekranın altından başla; yarıda kesilen kapanmadaysa
        // olduğu yerden devam et.
        if !g.moving && g.y < off {
            if !platform::is_visible(d.hwnd) {
                g.y = off;
            }
        }
        if g.y > off {
            g.y = off;
        }
        let cur = *g;
        drop(map);
        platform::set_bounds(d.hwnd, cur.x, cur.y, cur.w.max(1), cur.h.max(1));
    }
    platform::show_no_activate(d.hwnd);
    animate(d.hwnd, y, 150);
}

fn slide_out(d: &Dock) {
    let off = d.mon_rect.3;
    if animate(d.hwnd, off, 130) {
        platform::hide(d.hwnd);
    }
}

pub fn emit_visibility(app: &AppHandle, label: &str, visible: bool) {
    let _ = app.emit_to(label, "dock-visibility", visible);
}

/// Görünürlük ayarlarının döngüde kullanılan hafif kopyası.
struct Vis {
    /// 0 = her zaman, 1 = otomatik gizle, 2 = akıllı
    mode: u8,
    delay: Duration,
    fullscreen: bool,
}

impl Vis {
    fn read(state: &AppState) -> Self {
        let cfg = state.cfg.lock().unwrap();
        Vis {
            mode: match cfg.effective_visibility() {
                "autohide" => 1,
                "smart" => 2,
                _ => 0,
            },
            delay: Duration::from_millis(cfg.hide_delay_ms.max(100)),
            fullscreen: cfg.hide_in_fullscreen,
        }
    }
}

enum Act {
    Hide(Dock),
    SlideIn(Dock, &'static str),
    SlideOut(Dock, &'static str),
}

static EXPLORER_PID: std::sync::Mutex<u32> = std::sync::Mutex::new(0);

/// (son ölçüm, kaç kez aynı ölçüldü, geçerli karar)
static FS_STABLE: std::sync::Mutex<(Option<String>, u8, Option<String>)> = std::sync::Mutex::new((None, 0, None));

/// Her 100 ms'de çağrılır: tam ekran, otomatik/akıllı gizleme.
/// Kararlar kilit altında verilir, pencere işlemleri kilit bırakıldıktan sonra yapılır.
fn update_visibility(app: &AppHandle, state: &AppState, vis: &Vis, hwnds: &[isize], fg: isize) {
    let now = Instant::now();
    let cursor = platform::cursor_pos();
    // Tam ekran kararı ancak 3 ardışık ölçümde (~300 ms) aynı kalınca değişir;
    // anlık yanlış ölçümler dock'u gizleyip geri getiriyordu.
    let raw_fs = if vis.fullscreen { platform::fullscreen_monitor(hwnds) } else { None };
    let fs_mon = {
        let mut f = FS_STABLE.lock().unwrap();
        if f.0 == raw_fs {
            f.1 = f.1.saturating_add(1);
        } else {
            f.0 = raw_fs.clone();
            f.1 = 1;
        }
        if f.1 >= 3 {
            f.2 = raw_fs;
        }
        f.2.clone()
    };
    let fg_rect = if vis.mode == 2 {
        platform::foreground_app_rect(hwnds)
    } else {
        None
    };
    let _ = fg;
    // Başlat menüsü, arama, hızlı ayarlar, tepsi menüsü açıkken gizlenme.
    let flyout = vis.mode != 0 && platform::shell_flyout_open();
    let pop_dock = state.pop.lock().unwrap().open.as_ref().map(|o| o.dock.clone());

    let mut acts = Vec::new();
    let mut here_changes: Vec<(String, bool)> = Vec::new();
    {
        let mut docks = state.docks.lock().unwrap();
        for d in docks.values_mut() {
            let Some((x, y, w, h)) = d.target else { continue };
            let m = d.mon_rect;
            // Fare başka ekrandayken dock'taki sürekli animasyonlar durur: yüksek
            // yenileme hızlı ekranda kesintisiz kare üretmek, Windows'ta diğer
            // ekrandaki imlecin takılıp kaybolmasına yol açıyordu.
            let here = cursor.0 >= m.0 && cursor.0 < m.2 && cursor.1 >= m.1 && cursor.1 < m.3;
            if here != d.cursor_here {
                d.cursor_here = here;
                here_changes.push((d.label.clone(), here));
            }

            if fs_mon.as_deref() == Some(d.monitor.as_str()) {
                if !d.fs_hidden {
                    d.fs_hidden = true;
                    d.shown = false;
                    acts.push(Act::Hide(d.clone()));
                }
                continue;
            }
            let leaving_fullscreen = d.fs_hidden;
            if leaving_fullscreen {
                d.fs_hidden = false;
                d.last_inside = now;
            }

            let want = if vis.mode == 0 {
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
                let pinned_open = flyout || pop_dock.as_deref() == Some(d.label.as_str());
                if near_edge || inside || (pinned_open && d.shown) {
                    d.last_inside = now;
                }
                let recent = now.duration_since(d.last_inside) < vis.delay;
                if vis.mode == 2 {
                    let overlap = fg_rect
                        .map(|r| r.0 < x + w && x < r.2 && r.1 < y + h && y < r.3)
                        .unwrap_or(false);
                    !overlap || recent
                } else {
                    recent
                }
            };

            if want && !d.shown {
                d.shown = true;
                let why = if leaving_fullscreen { "tam ekrandan çıkıldı" } else { "fare/alan boşaldı" };
                acts.push(Act::SlideIn(d.clone(), why));
            } else if !want && d.shown {
                d.shown = false;
                let why = if vis.mode == 2 { "pencere dock'a değdi" } else { "fare ayrıldı" };
                acts.push(Act::SlideOut(d.clone(), why));
            }
        }
    } // kilit burada bırakılır

    for (label, here) in here_changes {
        let _ = app.emit_to(label.as_str(), "cursor-here", here);
    }
    for a in acts {
        match a {
            Act::Hide(d) => {
                dlog!("{}: tam ekran algılandı, dock gizlendi", d.label);
                cancel(d.hwnd, d.mon_rect.3);
                platform::hide(d.hwnd);
                emit_visibility(app, &d.label, false);
            }
            Act::SlideIn(d, why) => {
                dlog!("{}: kayarak açılıyor ({why})", d.label);
                slide_in(&d);
                emit_visibility(app, &d.label, true);
            }
            Act::SlideOut(d, why) => {
                dlog!("{}: kayarak gizleniyor ({why})", d.label);
                slide_out(&d);
                emit_visibility(app, &d.label, false);
            }
        }
    }
}

/// Başlat menüsünün en son ön planda görüldüğü an (Başlat düğmesi aç/kapa).
pub static START_SEEN: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Arka plan döngüsü (100 ms): görünürlük, taskbar'ı gizli tutma, son ön plan
/// penceresi, ekran süresi, monitör takıp çıkarma.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            tick += 1;
            let state = app.state::<AppState>();
            let vis = Vis::read(&state);
            let hwnds: Vec<isize> = state.docks.lock().unwrap().values().map(|d| d.hwnd).collect();

            let fg = platform::foreground();
            if fg != 0 && !hwnds.contains(&fg) {
                state.last_fg.store(fg, std::sync::atomic::Ordering::SeqCst);
            }

            if platform::start_menu_open() {
                *START_SEEN.lock().unwrap() = Some(std::time::Instant::now());
            }
            update_visibility(&app, &state, &vis, &hwnds, fg);
            crate::popover::watch(&app, fg);

            if state.taskbar_prev.lock().unwrap().is_some() {
                let fixed = platform::enforce_taskbar();
                if fixed > 0 && tick % 50 == 0 {
                    dlog!("taskbar bölgesi yeniden uygulandı ({fixed} pencere)");
                }
                // Explorer yeniden başladıysa anında gizleme kancasını yenile.
                if tick % 30 == 0 && !platform::taskbar_hook_current() {
                    let _ = app.run_on_main_thread(|| {
                        let ok = platform::install_taskbar_hook();
                        dlog!("taskbar kancası {}", if ok { "kuruldu" } else { "KURULAMADI" });
                    });
                }
            }
            // Windows teması değişti mi? (Ayarlar → Kişiselleştirme → Renkler)
            if tick % 10 == 5 {
                crate::pinreq::process(&app);
            }
            // Explorer geç açıldı ya da yeniden başladı: taskbar'ı yeniden gizle,
            // dock'un ekranda ayırdığı alanı yeniden kaydet.
            if tick % 20 == 10 {
                let pid = platform::explorer_pid();
                let changed = {
                    let mut last = EXPLORER_PID.lock().unwrap();
                    let c = *last != pid && pid != 0;
                    *last = pid;
                    c
                };
                if changed && tick > 20 {
                    dlog!("Gezgin (Explorer) açıldı/yeniden başladı: taskbar ve yerleşim yenileniyor");
                    apply_taskbar(&app);
                    let _ = app.emit("relayout", ());
                } else if state.taskbar_prev.lock().unwrap().is_some() {
                    platform::taskbar_ensure_autohide();
                }
            }
            if tick % 20 == 0 {
                if let Some(light) = platform::light_theme() {
                    *state.system_light.lock().unwrap() = Some(light);
                }
                refresh_theme(&app);
            }
            if tick % 10 == 0 && platform::idle_ms() < 120_000 {
                let fg_app = platform::app_of_window(state.last_fg.load(std::sync::atomic::Ordering::SeqCst));
                state.screen.lock().unwrap().tick(1, fg_app);
            }
            if tick % 1200 == 0 {
                state.screen.lock().unwrap().save();
            }
            if tick % 30 == 0 {
                let sig = monitor_signature(&monitors(&app));
                let changed = *state.monitor_sig.lock().unwrap() != sig;
                if changed {
                    dlog!("ekran düzeni değişti, dock'lar yeniden kuruluyor");
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
