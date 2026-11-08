//! Telefon ekranı: Android telefon Cam Dock'a ikinci ekran/kumanda olarak bağlanır.
//!
//! - Yerel ağda 47800 numaralı bağlantı noktasında küçük bir HTTP + WebSocket
//!   sunucusu: telefona arayüzü (src/phone) gönderir, canlı veriyi akıtır,
//!   komutları (medya, ses, kısayollar) alır. Yalnızca özel ağ adreslerinden
//!   gelen bağlantılar kabul edilir.
//! - 47801'de UDP yayını: telefon uygulaması aynı ağdaki bilgisayarları bulur.
//! - Eşleşme: ilk bağlantıda Ayarlar'daki 6 haneli PIN; sonra cihaz anahtarıyla.
//! - USB: kablo takılınca ADB ile telefondaki 47800'ü bilgisayara yönlendirir,
//!   uygulama yüklü değilse kurar ve anahtarıyla başlatır (Wi-Fi gerekmez).

use crate::config::{self, PhoneDevice};
use crate::{dlog, platform, AppState};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

pub const PORT: u16 = 47800;
const BEACON_PORT: u16 = 47801;
pub const PACKAGE: &str = "com.camdock.phone";
pub const APK_URL: &str = "https://github.com/aliapaydin/cam-dock-releases/releases/latest/download/CamDock-Telefon.apk";
const ADB_ZIP: &str = "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";

const INDEX: &str = include_str!("../../src/phone/index.html");
const JS: &str = include_str!("../../src/phone/phone.js");
const CSS: &str = include_str!("../../src/phone/phone.css");

/// Sunucu çalışsın mı (ayar açık)
static RUNNING: AtomicBool = AtomicBool::new(false);
/// İş parçacıkları başlatıldı mı
static STARTED: AtomicBool = AtomicBool::new(false);
/// Dinleme başarılı mı (bağlantı noktası doluysa false)
static LISTENING: AtomicBool = AtomicBool::new(false);
static CLIENTS: AtomicUsize = AtomicUsize::new(0);
static PIN: Mutex<String> = Mutex::new(String::new());
static USB_STATUS: Mutex<String> = Mutex::new(String::new());
/// Bağlı telefon adları
static CONNECTED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn new_pin() -> String {
    let b = uuid::Uuid::new_v4().as_u128();
    format!("{:06}", (b % 1_000_000) as u32)
}

fn pin() -> String {
    let mut p = PIN.lock().unwrap();
    if p.is_empty() {
        *p = new_pin();
    }
    p.clone()
}

/// Ayara göre sunucuyu aç/kapat (açılışta ve ayar kaydında).
pub fn sync(app: &AppHandle) {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().phone.clone();
    RUNNING.store(cfg.enabled, Ordering::SeqCst);
    if cfg.enabled && !STARTED.swap(true, Ordering::SeqCst) {
        let a = app.clone();
        std::thread::spawn(move || server(a));
        std::thread::spawn(beacon);
        let a = app.clone();
        std::thread::spawn(move || usb_loop(a));
    }
    if cfg.enabled && !MON_STARTED.swap(true, Ordering::SeqCst) {
        std::thread::spawn(monitor);
    }
}

/// Çıkışta: ADB sunucusunu kapat (başlattıysak).
pub fn shutdown() {
    RUNNING.store(false, Ordering::SeqCst);
    let adb = adb_path();
    if adb.is_file() {
        let _ = run_adb(&adb, &["kill-server"]);
    }
}

// ---------------------------------------------------------------------------
// Sunucu
// ---------------------------------------------------------------------------

fn server(app: AppHandle) {
    loop {
        while !RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
        }
        let listener = match TcpListener::bind(("0.0.0.0", PORT)) {
            Ok(l) => l,
            Err(e) => {
                dlog!("telefon: {PORT} dinlenemedi: {e}");
                LISTENING.store(false, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        let _ = listener.set_nonblocking(true);
        LISTENING.store(true, Ordering::SeqCst);
        dlog!("telefon: {PORT} dinleniyor");
        while RUNNING.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((s, addr)) => {
                    if !private(addr.ip()) {
                        continue;
                    }
                    let _ = s.set_nonblocking(false);
                    let a = app.clone();
                    std::thread::spawn(move || {
                        let _ = handle(a, s);
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(80)),
            }
        }
        LISTENING.store(false, Ordering::SeqCst);
    }
}

/// Yalnızca yerel ağ (ve USB yönlendirmesi için 127.0.0.1).
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => v.is_loopback(),
    }
}

fn handle(app: AppHandle, mut s: TcpStream) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    // İsteğin başlığını tüketmeden oku: WebSocket ise tungstenite kendisi okur
    let mut buf = [0u8; 4096];
    let mut n = 0;
    for _ in 0..50 {
        n = s.peek(&mut buf)?;
        if n == 0 || buf[..n].windows(4).any(|w| w == b"\r\n\r\n") || n == buf.len() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let head = String::from_utf8_lossy(&buf[..n]).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
    if head.to_ascii_lowercase().contains("upgrade: websocket") {
        return ws(app, s);
    }
    // Başlığı tüket
    let end = head.find("\r\n\r\n").map(|i| i + 4).unwrap_or(n);
    let mut sink = vec![0u8; end];
    let _ = s.read_exact(&mut sink);
    let route = path.split('?').next().unwrap_or("/");
    let (status, ctype, body): (&str, &str, Vec<u8>) = match route {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", INDEX.as_bytes().to_vec()),
        "/phone.js" => ("200 OK", "text/javascript; charset=utf-8", JS.as_bytes().to_vec()),
        "/phone.css" => ("200 OK", "text/css; charset=utf-8", CSS.as_bytes().to_vec()),
        "/ping" => ("200 OK", "application/json", json!({ "camdock": 1, "name": pc_name() }).to_string().into_bytes()),
        _ => ("404 Not Found", "text/plain", b"yok".to_vec()),
    };
    let hdr = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(hdr.as_bytes())?;
    s.write_all(&body)?;
    let _ = app;
    Ok(())
}

pub fn pc_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Bilgisayar".into())
}

struct Session {
    device: Option<String>,
    wrong: u32,
    last_push: Instant,
    last_cfg: String,
    track: String,
    net_prev: Option<(u64, u64, Instant)>,
    info_at: Option<Instant>,
    battery: (i32, bool),
    icons: std::collections::HashSet<String>,
    last_claude: String,
}

fn ws(app: AppHandle, s: TcpStream) -> std::io::Result<()> {
    use tungstenite::{Error, Message};
    let Ok(mut sock) = tungstenite::accept(s) else { return Ok(()) };
    let _ = sock.get_mut().set_read_timeout(Some(Duration::from_millis(120)));
    let mut ses = Session {
        device: None,
        wrong: 0,
        last_push: Instant::now() - Duration::from_secs(5),
        last_cfg: String::new(),
        track: String::new(),
        net_prev: None,
        info_at: None,
        battery: (-1, false),
        icons: Default::default(),
        last_claude: String::new(),
    };
    CLIENTS.fetch_add(1, Ordering::SeqCst);
    let result = (|| -> Result<(), Error> {
        loop {
            if !RUNNING.load(Ordering::SeqCst) {
                let _ = sock.close(None);
                return Ok(());
            }
            match sock.read() {
                Ok(Message::Text(t)) => {
                    let msg: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                    for out in on_message(&app, &mut ses, &msg) {
                        sock.send(Message::Text(out.to_string()))?;
                    }
                    if ses.wrong >= 5 {
                        return Ok(());
                    }
                }
                Ok(Message::Close(_)) => return Ok(()),
                Ok(_) => {}
                Err(Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(e),
            }
            if ses.device.is_some() && ses.last_push.elapsed() >= Duration::from_millis(1000) {
                ses.last_push = Instant::now();
                for out in push(&app, &mut ses) {
                    sock.send(Message::Text(out.to_string()))?;
                }
            }
        }
    })();
    CLIENTS.fetch_sub(1, Ordering::SeqCst);
    if let Some(d) = &ses.device {
        let mut c = CONNECTED.lock().unwrap();
        if let Some(i) = c.iter().position(|x| x == d) {
            c.remove(i);
        }
    }
    let _ = result;
    Ok(())
}

fn save_devices(app: &AppHandle, f: impl FnOnce(&mut Vec<PhoneDevice>)) {
    let cfg = {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        f(&mut cfg.phone.devices);
        cfg.clone()
    };
    config::save(&cfg);
}

fn authed(app: &AppHandle, ses: &mut Session, name: String) -> Vec<Value> {
    ses.device = Some(name.clone());
    CONNECTED.lock().unwrap().push(name.clone());
    dlog!("telefon: {name} bağlandı");
    let hist: Vec<Value> = {
        let state = app.state::<AppState>();
        let h = state.stats_hist.lock().unwrap();
        h.iter().rev().take(60).rev().map(|s| json!({ "cpu": s.cpu, "gpu": s.gpu, "ram": s.ram })).collect()
    };
    vec![json!({ "t": "hist", "items": hist })]
}

fn on_message(app: &AppHandle, ses: &mut Session, msg: &Value) -> Vec<Value> {
    let t = msg["t"].as_str().unwrap_or("");
    let device = msg["device"].as_str().unwrap_or("Telefon").chars().take(40).collect::<String>();
    match t {
        "hello" => {
            let token = msg["token"].as_str().unwrap_or("");
            let found = {
                let state = app.state::<AppState>();
                let cfg = state.cfg.lock().unwrap();
                (!token.is_empty()).then(|| cfg.phone.devices.iter().find(|d| d.token == token).map(|d| d.name.clone())).flatten()
            };
            match found {
                Some(name) => {
                    let tok = token.to_string();
                    save_devices(app, |d| {
                        if let Some(x) = d.iter_mut().find(|x| x.token == tok) {
                            x.last_seen = now_secs();
                        }
                    });
                    let mut out = vec![json!({ "t": "auth", "ok": true, "pc": pc_name() })];
                    out.extend(authed(app, ses, name));
                    out
                }
                None => vec![json!({ "t": "auth", "ok": false, "need": "pin", "pc": pc_name() })],
            }
        }
        "pair" => {
            let given = msg["pin"].as_str().unwrap_or("").trim().to_string();
            if !given.is_empty() && given == pin() {
                let token = uuid::Uuid::new_v4().simple().to_string();
                let tok = token.clone();
                let name = device.clone();
                save_devices(app, |d| d.push(PhoneDevice { name, token: tok, last_seen: now_secs() }));
                // PIN tek kullanımlık: bir sonraki eşleşme için yenile
                *PIN.lock().unwrap() = new_pin();
                let mut out = vec![json!({ "t": "auth", "ok": true, "token": token, "pc": pc_name() })];
                out.extend(authed(app, ses, device));
                out
            } else {
                ses.wrong += 1;
                std::thread::sleep(Duration::from_millis(600));
                vec![json!({ "t": "auth", "ok": false, "need": "pin", "wrong": true, "pc": pc_name() })]
            }
        }
        "ping" => vec![json!({ "t": "pong" })],
        _ if ses.device.is_none() => vec![json!({ "t": "auth", "ok": false, "need": "pin" })],
        "media" => {
            let a = msg["a"].as_str().unwrap_or("");
            if matches!(a, "toggle" | "next" | "prev") {
                platform::media_control(a);
            }
            ses.last_push = Instant::now() - Duration::from_millis(700);
            vec![]
        }
        "vol" => {
            if let Some(v) = msg["v"].as_f64() {
                platform::set_volume(v.clamp(0.0, 100.0) as f32);
            }
            vec![]
        }
        "mute" => {
            platform::set_mute(msg["m"].as_bool().unwrap_or(false));
            vec![]
        }
        "kill" => {
            if let Some(name) = msg["name"].as_str() {
                KILLS.lock().unwrap().push((name.to_string(), msg["all"].as_bool().unwrap_or(false)));
            }
            vec![]
        }
        "run" => {
            let i = msg["i"].as_u64().unwrap_or(u64::MAX) as usize;
            let sc = app.state::<AppState>().cfg.lock().unwrap().phone.shortcuts.get(i).cloned();
            if let Some(sc) = sc {
                run_shortcut(&sc);
            }
            vec![]
        }
        _ => vec![],
    }
}

fn run_shortcut(sc: &config::PhoneShortcut) {
    let v = sc.value.trim();
    match sc.kind.as_str() {
        "keys" => {
            platform::send_combo(v);
        }
        "action" => match v {
            "mute" => {
                let (_, m) = platform::volume();
                platform::set_mute(!m);
            }
            "lock" => {
                platform::launch("rundll32.exe", Some("user32.dll,LockWorkStation"));
            }
            "playpause" => {
                platform::media_control("toggle");
            }
            _ => {}
        },
        _ => {
            // "yol" ya da "\"yol\" argümanlar"
            let (path, args) = if let Some(rest) = v.strip_prefix('"') {
                match rest.split_once('"') {
                    Some((p, a)) => (p.to_string(), a.trim().to_string()),
                    None => (rest.to_string(), String::new()),
                }
            } else {
                (v.to_string(), String::new())
            };
            let path = config::expand_env(&path);
            platform::launch(&path, (!args.is_empty()).then_some(args.as_str()));
        }
    }
}

fn push(app: &AppHandle, ses: &mut Session) -> Vec<Value> {
    let mut out = Vec::new();
    let state = app.state::<AppState>();
    let st = *state.stats.lock().unwrap();
    // Ağ hızı
    let (rx, tx) = platform::net_bytes();
    let now = Instant::now();
    let (down, up) = match ses.net_prev {
        Some((prx, ptx, t)) => {
            let dt = now.duration_since(t).as_secs_f64().max(0.2);
            (rx.saturating_sub(prx) as f64 / dt, tx.saturating_sub(ptx) as f64 / dt)
        }
        None => (0.0, 0.0),
    };
    ses.net_prev = Some((rx, tx, now));
    if ses.info_at.map_or(true, |t| t.elapsed() > Duration::from_secs(15)) {
        ses.info_at = Some(now);
        let i = platform::sys_info();
        ses.battery = (i.battery, i.charging);
    }
    out.push(json!({
        "t": "stats",
        "cpu": st.cpu, "gpu": st.gpu, "ram": st.ram, "disk": st.disk,
        "ram_used": st.ram_used_gb, "ram_total": st.ram_total_gb,
        "down": down, "up": up,
        "battery": ses.battery.0, "charging": ses.battery.1,
    }));
    // Medya
    let m = platform::media_state();
    let mut mj = json!({
        "t": "media", "active": m.active, "title": m.title, "artist": m.artist, "album": m.album,
        "playing": m.playing, "pos": m.position_ms, "dur": m.duration_ms, "source": m.source,
    });
    let key = format!("{}|{}|{}", m.track_key, m.title, m.artist);
    if key != ses.track {
        ses.track = key;
        use base64::Engine;
        mj["cover"] = match platform::media_thumbnail() {
            Some(b) => json!(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))),
            None => Value::Null,
        };
    }
    out.push(mj);
    let (v, muted) = platform::volume();
    out.push(json!({ "t": "vol", "v": v, "m": muted }));
    // Süreçler, diskler, ping (simgeler bir kez)
    let extra = EXTRA.lock().unwrap().clone();
    if let Some(x) = &extra {
        use base64::Engine;
        let mut icons = serde_json::Map::new();
        for p in &x.procs {
            if !p.exe.is_empty() && ses.icons.insert(p.exe.clone()) {
                if let Some(png) = platform::icon_png(&p.exe, 32) {
                    icons.insert(p.exe.clone(), json!(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))));
                }
            }
        }
        out.push(json!({
            "t": "sys", "procs": x.procs, "total_procs": x.total_procs, "disks": x.disks,
            "ping": x.ping, "uptime": x.uptime, "icons": icons,
        }));
    }
    // Claude kullanımı (değişince)
    let cl = state.claude.lock().unwrap().clone();
    let cj = json!({ "t": "claude", "state": cl.state, "limits": cl.limits.iter().map(|l| json!({ "label": l.label, "percent": l.percent, "resets_at": l.resets_at })).collect::<Vec<_>>() });
    let cs = cj.to_string();
    if cs != ses.last_claude {
        ses.last_claude = cs;
        out.push(cj);
    }
    // Ayarlar değiştiyse (sayfalar, kısayollar, hava durumu konumu)
    let cfg = state.cfg.lock().unwrap().clone();
    let dark = {
        let sys = *state.system_light.lock().unwrap();
        crate::theme::effective_dark(&cfg, sys.or_else(platform::light_theme))
    };
    let cj = json!({
        "t": "cfg",
        "pc": pc_name(),
        "pages": cfg.phone.pages,
        "shortcuts": cfg.phone.shortcuts.iter().map(|s| json!({ "label": s.label, "icon": s.icon })).collect::<Vec<_>>(),
        "weather": { "enabled": cfg.weather.enabled, "lat": cfg.weather.lat, "lon": cfg.weather.lon, "city": cfg.weather.city },
        "accent": cfg.accent,
        "dark": dark,
        "theme": cfg.phone.theme,
        "idle_home": cfg.phone.idle_home,
        "night_dim": cfg.phone.night_dim,
        "cpu_name": extra.as_ref().map(|x| x.cpu_name.clone()).unwrap_or_default(),
        "gpu_name": extra.as_ref().map(|x| x.gpu_name.clone()).unwrap_or_default(),
        "cores": extra.as_ref().map(|x| x.cores).unwrap_or(0),
    });
    let cs = cj.to_string();
    if cs != ses.last_cfg {
        ses.last_cfg = cs;
        out.push(cj);
    }
    out
}

// ---------------------------------------------------------------------------
// Süreçler, diskler, ping (yalnızca bağlı telefon varken)
// ---------------------------------------------------------------------------

#[derive(Clone, Default, serde::Serialize)]
struct Proc {
    name: String,
    exe: String,
    count: u32,
    cpu: f32,
    mem: f64,
}

#[derive(Clone, Default, serde::Serialize)]
struct Disk {
    name: String,
    used: f64,
    total: f64,
}

#[derive(Clone, Default)]
struct Extra {
    procs: Vec<Proc>,
    total_procs: usize,
    disks: Vec<Disk>,
    ping: Option<f64>,
    uptime: u64,
    cpu_name: String,
    gpu_name: String,
    cores: usize,
}

static EXTRA: Mutex<Option<Extra>> = Mutex::new(None);
static MON_STARTED: AtomicBool = AtomicBool::new(false);
/// Telefondan gelen "kapat" istekleri: (ad, hepsi)
static KILLS: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

/// Kapatılmasına izin verilmeyen sistem süreçleri
const PROTECTED: &[&str] = &[
    "system", "registry", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "svchost", "dwm",
    "fontdrvhost", "memory compression", "secure system", "lsaiso", "explorer", "cam dock", "cam-dock",
];

fn gpu_name() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController | Sort-Object AdapterRAM -Descending | Select-Object -First 1).Name"])
            .creation_flags(0x0800_0000)
            .output();
        if let Ok(o) = out {
            return String::from_utf8_lossy(&o.stdout).trim().to_string();
        }
    }
    String::new()
}

fn monitor() {
    use sysinfo::{Disks, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_cpu_all();
    let cpu_name = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let gpu = gpu_name();
    let mut disks_at: Option<Instant> = None;
    let mut disks: Vec<Disk> = Vec::new();
    let mut ping_at: Option<Instant> = None;
    let mut ping: Option<f64> = None;
    loop {
        if CLIENTS.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        sys.refresh_processes(ProcessesToUpdate::All, true);
        // Kapatma istekleri
        let kills: Vec<(String, bool)> = std::mem::take(&mut *KILLS.lock().unwrap());
        for (name, all) in kills {
            let key = name.to_lowercase();
            if PROTECTED.contains(&key.as_str()) {
                continue;
            }
            let mut list: Vec<&sysinfo::Process> = sys
                .processes()
                .values()
                .filter(|p| p.name().to_string_lossy().to_lowercase().trim_end_matches(".exe") == key)
                .collect();
            list.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal));
            for p in list.iter().take(if all { usize::MAX } else { 1 }) {
                dlog!("telefon: süreç kapatılıyor {}", name);
                p.kill();
            }
        }
        // Ada göre grupla
        let mut map: std::collections::HashMap<String, Proc> = std::collections::HashMap::new();
        let total = sys.processes().len();
        for p in sys.processes().values() {
            let raw = p.name().to_string_lossy().to_string();
            let name = raw.trim_end_matches(".exe").trim_end_matches(".EXE").to_string();
            let low = name.to_lowercase();
            if low.is_empty() || low == "idle" || low == "system idle process" || low == "memory compression" {
                continue;
            }
            let e = map.entry(low).or_insert_with(|| Proc {
                name: name.clone(),
                exe: p.exe().map(|x| x.to_string_lossy().to_string()).unwrap_or_default(),
                ..Default::default()
            });
            e.count += 1;
            e.cpu += p.cpu_usage() / cores as f32;
            e.mem += p.memory() as f64 / 1_073_741_824.0;
            if e.exe.is_empty() {
                if let Some(x) = p.exe() {
                    e.exe = x.to_string_lossy().to_string();
                }
            }
        }
        let mut procs: Vec<Proc> = map.into_values().collect();
        procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        procs.truncate(8);
        if disks_at.map_or(true, |t| t.elapsed() > Duration::from_secs(30)) {
            disks_at = Some(Instant::now());
            let list = Disks::new_with_refreshed_list();
            disks = list
                .iter()
                .filter(|d| d.total_space() > 1_000_000_000)
                .map(|d| {
                    let total = d.total_space() as f64 / 1_073_741_824.0;
                    let free = d.available_space() as f64 / 1_073_741_824.0;
                    let name = d.mount_point().to_string_lossy().trim_end_matches('\\').to_string();
                    Disk { name, used: total - free, total }
                })
                .collect();
            disks.sort_by(|a, b| a.name.cmp(&b.name));
            disks.dedup_by(|a, b| a.name == b.name);
        }
        if ping_at.map_or(true, |t| t.elapsed() > Duration::from_secs(5)) {
            ping_at = Some(Instant::now());
            let t = Instant::now();
            ping = TcpStream::connect_timeout(&"1.1.1.1:443".parse().unwrap(), Duration::from_millis(1500))
                .ok()
                .map(|_| t.elapsed().as_secs_f64() * 1000.0);
        }
        *EXTRA.lock().unwrap() = Some(Extra {
            procs,
            total_procs: total,
            disks: disks.clone(),
            ping,
            uptime: System::uptime(),
            cpu_name: cpu_name.clone(),
            gpu_name: gpu.clone(),
            cores,
        });
        std::thread::sleep(Duration::from_secs(2));
    }
}

// ---------------------------------------------------------------------------
// Ağda bulunma
// ---------------------------------------------------------------------------

/// Bilgisayarın yerel ağ adresi (varsayılan yol).
pub fn local_ip() -> Option<std::net::Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.168.1.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v) if !v.is_loopback() && !v.is_unspecified() => Some(v),
        _ => None,
    }
}

fn beacon() {
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else { return };
    let _ = sock.set_broadcast(true);
    loop {
        if RUNNING.load(Ordering::SeqCst) && LISTENING.load(Ordering::SeqCst) {
            let msg = json!({ "camdock": 1, "name": pc_name(), "port": PORT, "ip": local_ip().map(|i| i.to_string()) }).to_string();
            let _ = sock.send_to(msg.as_bytes(), ("255.255.255.255", BEACON_PORT));
            if let Some(ip) = local_ip() {
                let o = ip.octets();
                let _ = sock.send_to(msg.as_bytes(), (std::net::Ipv4Addr::new(o[0], o[1], o[2], 255), BEACON_PORT));
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

// ---------------------------------------------------------------------------
// USB (ADB)
// ---------------------------------------------------------------------------

fn tools_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("CamDock")
}

fn adb_path() -> PathBuf {
    tools_dir().join("platform-tools").join("adb.exe")
}

fn set_usb(s: impl Into<String>) {
    *USB_STATUS.lock().unwrap() = s.into();
}

fn run_adb(adb: &PathBuf, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(adb);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr))
}

fn download(url: &str) -> Option<Vec<u8>> {
    let resp = ureq::get(url).timeout(Duration::from_secs(120)).call().ok()?;
    let mut buf = Vec::new();
    resp.into_reader().take(200 * 1024 * 1024).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// ADB'yi (Google platform-tools) ilk kullanımda indirir.
fn ensure_adb() -> Option<PathBuf> {
    let adb = adb_path();
    if adb.is_file() {
        return Some(adb);
    }
    set_usb("USB araçları indiriliyor…");
    let zip = download(ADB_ZIP)?;
    let mut ar = zip::ZipArchive::new(std::io::Cursor::new(zip)).ok()?;
    let dir = tools_dir().join("platform-tools");
    std::fs::create_dir_all(&dir).ok()?;
    for name in ["adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll"] {
        let mut f = ar.by_name(&format!("platform-tools/{name}")).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        std::fs::write(dir.join(name), buf).ok()?;
    }
    adb.is_file().then_some(adb)
}

fn usb_token(app: &AppHandle, name: &str) -> String {
    let existing = {
        let state = app.state::<AppState>();
        let cfg = state.cfg.lock().unwrap();
        cfg.phone.devices.iter().find(|d| d.name == name).map(|d| d.token.clone())
    };
    if let Some(t) = existing {
        return t;
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let (n, t) = (name.to_string(), token.clone());
    save_devices(app, |d| d.push(PhoneDevice { name: n, token: t, last_seen: now_secs() }));
    token
}

fn usb_loop(app: AppHandle) {
    let mut ready: Vec<String> = Vec::new();
    let mut fail_until: Option<Instant> = None;
    loop {
        std::thread::sleep(Duration::from_secs(3));
        let usb = app.state::<AppState>().cfg.lock().unwrap().phone.usb;
        if !RUNNING.load(Ordering::SeqCst) || !usb {
            set_usb("");
            ready.clear();
            continue;
        }
        if fail_until.is_some_and(|t| Instant::now() < t) {
            continue;
        }
        let Some(adb) = ensure_adb() else {
            set_usb("USB araçları indirilemedi; 5 dakika sonra yeniden denenecek.");
            fail_until = Some(Instant::now() + Duration::from_secs(300));
            continue;
        };
        let Some(list) = run_adb(&adb, &["devices"]) else { continue };
        let mut present: Vec<(String, String)> = Vec::new();
        for line in list.lines().skip(1) {
            let mut it = line.split_whitespace();
            if let (Some(serial), Some(st)) = (it.next(), it.next()) {
                present.push((serial.to_string(), st.to_string()));
            }
        }
        ready.retain(|s| present.iter().any(|(p, st)| p == s && st == "device"));
        if present.is_empty() {
            set_usb("Kablo ile bağlı telefon yok.");
            continue;
        }
        for (serial, st) in &present {
            if st == "unauthorized" {
                set_usb("Telefonda “USB hata ayıklamaya izin ver” sorusunu onayla.");
                continue;
            }
            if st != "device" || ready.contains(serial) {
                continue;
            }
            let model = run_adb(&adb, &["-s", serial, "shell", "getprop", "ro.product.model"])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Android".into());
            let port = format!("tcp:{PORT}");
            run_adb(&adb, &["-s", serial, "reverse", &port, &port]);
            let installed = run_adb(&adb, &["-s", serial, "shell", "pm", "list", "packages", PACKAGE])
                .is_some_and(|s| s.contains(&format!("package:{PACKAGE}")));
            if !installed {
                set_usb(format!("{model}: Cam Dock Telefon kuruluyor…"));
                let apk = tools_dir().join("CamDock-Telefon.apk");
                let path = apk.to_string_lossy().to_string();
                let ok = download(APK_URL).and_then(|b| std::fs::write(&apk, b).ok()).is_some()
                    && run_adb(&adb, &["-s", serial, "install", "-r", &path]).is_some_and(|s| {
                        // Farklı imzalı eski kurulum: kaldırıp yeniden kur
                        s.contains("Success")
                            || (s.contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE")
                                && run_adb(&adb, &["-s", serial, "uninstall", PACKAGE]).is_some()
                                && run_adb(&adb, &["-s", serial, "install", &path]).is_some_and(|s| s.contains("Success")))
                    });
                if !ok {
                    set_usb(format!("{model}: uygulama kurulamadı. Telefonda “USB ile yükle” iznini aç ya da APK'yı elle kur."));
                    continue;
                }
            }
            let name = format!("{model} (USB)");
            let token = usb_token(&app, &name);
            run_adb(
                &adb,
                &[
                    "-s", serial, "shell", "am", "start", "-n", &format!("{PACKAGE}/.MainActivity"),
                    "--es", "host", "127.0.0.1", "--ei", "port", &PORT.to_string(), "--es", "token", &token,
                ],
            );
            dlog!("telefon: USB {model} hazır");
            set_usb(format!("{model} kablo ile bağlı."));
            ready.push(serial.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Ayarlar penceresi
// ---------------------------------------------------------------------------

pub fn status(app: &AppHandle) -> Value {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().phone.clone();
    let qr = qrcode::QrCode::new(APK_URL)
        .map(|c| {
            c.render::<qrcode::render::svg::Color>()
                .min_dimensions(168, 168)
                .quiet_zone(true)
                .dark_color(qrcode::render::svg::Color("#0b0e14"))
                .light_color(qrcode::render::svg::Color("#ffffff"))
                .build()
        })
        .unwrap_or_default();
    json!({
        "enabled": cfg.enabled,
        "listening": LISTENING.load(Ordering::SeqCst),
        "port": PORT,
        "ip": local_ip().map(|i| i.to_string()),
        "pc": pc_name(),
        "pin": pin(),
        "clients": CLIENTS.load(Ordering::SeqCst),
        "connected": CONNECTED.lock().unwrap().clone(),
        "devices": cfg.devices.iter().map(|d| json!({ "name": d.name, "last_seen": d.last_seen })).collect::<Vec<_>>(),
        "usb": USB_STATUS.lock().unwrap().clone(),
        "apk_url": APK_URL,
        "apk_qr": qr,
    })
}

pub fn renew_pin() {
    *PIN.lock().unwrap() = new_pin();
}

pub fn forget(app: &AppHandle, name: &str) {
    let n = name.to_string();
    save_devices(app, |d| d.retain(|x| x.name != n));
    crate::emit_config(app);
}
