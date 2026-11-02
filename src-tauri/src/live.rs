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

fn scan_downloads(st: &mut LiveState) {
    let now = Instant::now();
    let mut seen = Vec::new();
    if let Some(dir) = downloads_dir() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
                if !PARTIAL.contains(&ext.as_str()) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                let size = meta.len();
                let (speed, _) = match st.sizes.get(&p) {
                    Some(&(prev, t, sp)) => {
                        let dt = now.duration_since(t).as_secs_f64().max(0.2);
                        let inst = size.saturating_sub(prev) as f64 / dt;
                        (sp * 0.6 + inst * 0.4, ())
                    }
                    None => (0.0, ()),
                };
                st.sizes.insert(p.clone(), (size, now, speed));
                seen.push(p);
            }
        }
    }
    // Kaybolan kısmi dosya: indirme bitti (ya da iptal edildi)
    let gone: Vec<PathBuf> = st.sizes.keys().filter(|p| !seen.contains(p)).cloned().collect();
    for p in gone {
        let bytes = st.sizes.remove(&p).map(|v| v.0).unwrap_or(0);
        let final_path = p.with_extension("");
        if final_path.exists() {
            let name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            dlog!("indirme tamamlandı: {name}");
            st.finished.push((
                Download { name, path: final_path.to_string_lossy().into(), bytes, speed: 0.0, done: true },
                now + Duration::from_secs(6),
            ));
        }
    }
    st.finished.retain(|(_, until)| *until > now);
    let mut list: Vec<Download> = seen
        .iter()
        .map(|p| {
            let (bytes, _, speed) = st.sizes[p];
            let name = p.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
            Download { name, path: p.to_string_lossy().into(), bytes, speed, done: false }
        })
        .collect();
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
