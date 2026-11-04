//! Canlı etkinlikler: süren indirmeler, zamanlayıcı, mikrofon/kamera
//! kullanımı, şarja takılınca kısa bilgi. Dock'taki canlı hücre saniyede bir
//! `live_state` ile okur.

use crate::{dlog, platform, AppState};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Default)]
pub struct Download {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    /// Toplam boyut (0 = bilinmiyor)
    pub total: u64,
    /// bayt/sn
    pub speed: f64,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Timer {
    pub label: String,
    pub total_ms: u64,
    pub left_ms: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Snapshot {
    pub downloads: Vec<Download>,
    pub timer: Option<Timer>,
    pub mic: Vec<String>,
    pub cam: Vec<String>,
    /// Şarja yeni takıldıysa pil yüzdesi (birkaç saniye görünür)
    pub charging: Option<i32>,
}

#[derive(Default)]
pub struct LiveState {
    pub snap: Snapshot,
    /// İndirme dosyası → (son boyut, son ölçüm, hız)
    sizes: HashMap<PathBuf, (u64, Instant, f64)>,
    /// Biten indirmeler: gösterim bitişi
    finished: Vec<(Download, Instant)>,
    pub timer: Option<(String, Instant, Duration)>,
    last_ac: Option<bool>,
    charging_until: Option<(i32, Instant)>,
    /// Tarayıcı geçmişinden okunan süren indirmeler (kısmi yol küçük harf → bilgi)
    browser: HashMap<String, BrowserDl>,
    browser_at: Option<Instant>,
    /// Kısmi dosya → son bilinen gerçek ad (bitince doğru dosyayı göstermek için)
    last_names: HashMap<String, String>,
}

#[derive(Clone, Debug)]
struct BrowserDl {
    current: PathBuf,
    name: String,
    total: u64,
    received: u64,
}

const PARTIAL: [&str; 5] = ["crdownload", "part", "partial", "download", "opdownload"];

fn downloads_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    let d = PathBuf::from(home).join("Downloads");
    d.is_dir().then_some(d)
}

fn pretty_app(id: &str) -> String {
    // "C:\...\Teams.exe" → "Teams", "MSTeams_8wekyb3d8bbwe" → "MSTeams"
    let tail = id.rsplit(['\\', '/']).next().unwrap_or(id);
    let tail = tail.strip_suffix(".exe").or_else(|| tail.strip_suffix(".EXE")).unwrap_or(tail);
    let tail = tail.split('_').next().unwrap_or(tail);
    let tail = tail.rsplit('.').next().unwrap_or(tail);
    match tail.to_lowercase().as_str() {
        "mstteams" | "msteams" | "teams" => "Teams".into(),
        "whatsappdesktop" | "whatsapp" | "whatsapp.root" => "WhatsApp".into(),
        "zoom" => "Zoom".into(),
        "chrome" => "Chrome".into(),
        "msedge" => "Edge".into(),
        "discord" => "Discord".into(),
        _ => {
            let mut c = tail.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// Chrome/Edge/Brave geçmiş veritabanlarındaki süren indirmeler. Chrome kısmi
/// dosyayı çoğu zaman "Onaylanmayan 12345.crdownload" diye tutar; gerçek ad,
/// toplam boyut ve hedef klasör yalnızca buradan öğrenilir.
fn browser_downloads() -> HashMap<String, BrowserDl> {
    let mut out = HashMap::new();
    let Some(local) = std::env::var_os("LOCALAPPDATA") else { return out };
    let local = PathBuf::from(local);
    let roots = [
        local.join("Google").join("Chrome").join("User Data"),
        local.join("Microsoft").join("Edge").join("User Data"),
        local.join("BraveSoftware").join("Brave-Browser").join("User Data"),
    ];
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n != "Default" && !n.starts_with("Profile ") {
                continue;
            }
            let db = e.path().join("History");
            if db.is_file() {
                read_history(&db, &mut out);
            }
        }
    }
    out
}

fn read_history(db: &std::path::Path, out: &mut HashMap<String, BrowserDl>) {
    use rusqlite::{Connection, OpenFlags};
    // immutable=1: kilit almadan oku (tarayıcı veritabanını açık tutuyor)
    let uri = format!("file:///{}?immutable=1", db.to_string_lossy().replace('%', "%25").replace('\\', "/").replace(' ', "%20").replace('#', "%23"));
    let Ok(conn) = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else { return };
    let Ok(mut q) = conn.prepare(
        "SELECT current_path, target_path, total_bytes, received_bytes FROM downloads WHERE state = 0 ORDER BY id DESC LIMIT 20",
    ) else { return };
    let rows = q.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
    });
    let Ok(rows) = rows else { return };
    for (cur, target, total, recv) in rows.flatten() {
        if cur.is_empty() {
            continue;
        }
        let current = PathBuf::from(&cur);
        if !current.exists() {
            continue; // eski/yarım kalmış kayıt
        }
        let name = std::path::Path::new(if target.is_empty() { &cur } else { &target })
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .trim_end_matches(".crdownload")
            .to_string();
        out.insert(
            cur.to_lowercase(),
            BrowserDl { current, name, total: total.max(0) as u64, received: recv.max(0) as u64 },
        );
    }
}

/// Kısmi dosyanın gerçek boyutu. Klasör listesindeki boyut, dosya yazılmak
/// üzere açıkken güncellenmez (NTFS); dosyanın kendisinden okunmalı.
fn live_size(p: &std::path::Path) -> Option<(u64, std::time::SystemTime)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)))
}

fn is_unconfirmed(stem: &str) -> bool {
    let s = stem.to_lowercase();
    s.starts_with("onaylanmayan ") || s.starts_with("unconfirmed ")
}

fn scan_downloads(st: &mut LiveState) {
    let now = Instant::now();
    let mut seen: Vec<PathBuf> = Vec::new();
    if let Some(dir) = downloads_dir() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
                if PARTIAL.contains(&ext.as_str()) {
                    seen.push(p);
                }
            }
        }
    }
    // Tarayıcı kayıtları: kısmi dosya varken 3 sn'de, yokken 15 sn'de bir
    let every = if seen.is_empty() && st.browser.is_empty() { 15 } else { 3 };
    if st.browser_at.map_or(true, |t| now.duration_since(t).as_secs() >= every) {
        st.browser_at = Some(now);
        let fresh = browser_downloads();
        // Okuma başarısız olursa (yazılırken) eldekini koru; kısmi dosya yoksa sıfırla
        if !fresh.is_empty() || seen.is_empty() {
            st.browser = fresh;
        }
    }
    // Başka klasöre inen tarayıcı indirmeleri de
    for b in st.browser.values() {
        if !seen.iter().any(|p| p.to_string_lossy().eq_ignore_ascii_case(&b.current.to_string_lossy())) {
            seen.push(b.current.clone());
        }
    }
    let mut live: Vec<PathBuf> = Vec::new();
    for p in &seen {
        let Some((size, modified)) = live_size(p) else { continue };
        let key = p.to_string_lossy().to_lowercase();
        let known = st.browser.contains_key(&key);
        // Tarayıcının bilmediği, uzun süredir değişmeyen kısmi dosya: yarım kalmış artık
        let idle = modified.elapsed().map(|d| d.as_secs() > 600).unwrap_or(false);
        if !known && idle && !st.sizes.contains_key(p) {
            continue;
        }
        let speed = match st.sizes.get(p) {
            Some(&(prev, t, sp)) => {
                let dt = now.duration_since(t).as_secs_f64().max(0.2);
                let inst = size.saturating_sub(prev) as f64 / dt;
                sp * 0.6 + inst * 0.4
            }
            None => 0.0,
        };
        st.sizes.insert(p.clone(), (size, now, speed));
        live.push(p.clone());
    }
    // Kaybolan kısmi dosya: indirme bitti (ya da iptal edildi)
    let gone: Vec<PathBuf> = st.sizes.keys().filter(|p| !live.contains(p)).cloned().collect();
    for p in gone {
        let bytes = st.sizes.remove(&p).map(|v| v.0).unwrap_or(0);
        let mut final_path = p.with_extension("");
        if let Some(b) = st.last_names.remove(&p.to_string_lossy().to_lowercase()) {
            if let Some(dir) = p.parent() {
                final_path = dir.join(&b);
            }
        }
        if final_path.exists() {
            let name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let total = live_size(&final_path).map(|v| v.0).unwrap_or(bytes);
            dlog!("indirme tamamlandı: {name}");
            st.finished.push((
                Download { name, path: final_path.to_string_lossy().into(), bytes: total, total, speed: 0.0, done: true },
                now + Duration::from_secs(6),
            ));
        }
    }
    st.finished.retain(|(_, until)| *until > now);
    let mut list: Vec<Download> = Vec::new();
    for p in &live {
        let (size, _, speed) = st.sizes[p];
        let key = p.to_string_lossy().to_lowercase();
        let stem = p.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let (name, total, bytes) = match st.browser.get(&key) {
            Some(b) if !b.name.is_empty() && !is_unconfirmed(&b.name) => (b.name.clone(), b.total, size.max(b.received)),
            Some(b) => ("Tarayıcı indirmesi".into(), b.total, size.max(b.received)),
            None if is_unconfirmed(&stem) => ("Tarayıcı indirmesi".into(), 0, size),
            None => (stem, 0, size),
        };
        if name != "Tarayıcı indirmesi" {
            st.last_names.insert(key, name.clone());
        }
        list.push(Download { name, path: p.to_string_lossy().into(), bytes, total, speed, done: false });
    }
    list.extend(st.finished.iter().map(|(d, _)| d.clone()));
    st.snap.downloads = list;
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            {
                let state = app.state::<AppState>();
                let mut st = state.live.lock().unwrap();
                scan_downloads(&mut st);
                // Zamanlayıcı
                let mut done_label = None;
                st.snap.timer = match &st.timer {
                    Some((label, start, total)) => {
                        let el = start.elapsed();
                        if el >= *total {
                            done_label = Some(label.clone());
                            None
                        } else {
                            Some(Timer { label: label.clone(), total_ms: total.as_millis() as u64, left_ms: (*total - el).as_millis() as u64 })
                        }
                    }
                    None => None,
                };
                if done_label.is_some() {
                    st.timer = None;
                }
                drop(st);
                if let Some(l) = done_label {
                    dlog!("zamanlayıcı bitti: {l}");
                    let _ = app.emit("timer-done", l);
                }
            }
            if tick % 2 == 0 {
                let mic: Vec<String> = platform::privacy_using("microphone").iter().map(|s| pretty_app(s)).collect();
                let cam: Vec<String> = platform::privacy_using("webcam").iter().map(|s| pretty_app(s)).collect();
                let state = app.state::<AppState>();
                let mut st = state.live.lock().unwrap();
                st.snap.mic = dedup(mic);
                st.snap.cam = dedup(cam);
            }
            if tick % 3 == 0 {
                let info = platform::sys_info();
                let state = app.state::<AppState>();
                let mut st = state.live.lock().unwrap();
                if info.battery >= 0 {
                    if st.last_ac == Some(false) && info.charging {
                        st.charging_until = Some((info.battery, Instant::now() + Duration::from_secs(6)));
                    }
                    st.last_ac = Some(info.charging);
                }
                st.snap.charging = match st.charging_until {
                    Some((pct, until)) if until > Instant::now() => Some(pct),
                    _ => None,
                };
            }
            tick += 1;
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}

fn dedup(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

#[tauri::command]
pub fn live_state(app: AppHandle) -> Snapshot {
    app.state::<AppState>().live.lock().unwrap().snap.clone()
}

pub fn timer_start(app: &AppHandle, minutes: u64) {
    let state = app.state::<AppState>();
    let mut st = state.live.lock().unwrap();
    st.timer = Some((format!("{minutes} dk"), Instant::now(), Duration::from_secs(minutes * 60)));
    dlog!("zamanlayıcı başladı: {minutes} dk");
}

#[tauri::command]
pub fn live_timer(app: AppHandle, minutes: Option<u64>) {
    match minutes {
        Some(m) if m > 0 => timer_start(&app, m.min(600)),
        _ => app.state::<AppState>().live.lock().unwrap().timer = None,
    }
}
