//! Ekran süresi: bilgisayar başında aktif olunan süre (2 dk'dan uzun boşta
//! kalınan süre sayılmaz). Gün değişince sıfırlanır; son 7 günün toplamları ve
//! günün uygulama bazlı dağılımı da tutulur.

use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppUse {
    pub name: String,
    pub path: String,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DayTotal {
    pub date: String,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenTime {
    pub date: String,
    pub seconds: u64,
    /// Saat başına saniye (0–23)
    pub hours: Vec<u64>,
    /// Uygulama başına süre (anahtar: küçük harfli exe yolu)
    #[serde(default)]
    pub apps: HashMap<String, AppUse>,
    /// Önceki günler (en eski başta, en çok 7)
    #[serde(default)]
    pub history: Vec<DayTotal>,
    /// Günün ilk etkin anı ("09:12")
    #[serde(default)]
    pub first: String,
    /// Bugün kaç kez uygulama değiştirildi
    #[serde(default)]
    pub switches: u32,
    #[serde(skip)]
    last_app: String,
}

fn path() -> PathBuf {
    crate::config::base_dir().join("screentime.json")
}

fn base_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let stem = file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE")).unwrap_or(file);
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl ScreenTime {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string(self) {
            let _ = std::fs::write(path(), t);
        }
    }

    fn roll(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if self.date != today || self.hours.len() != 24 {
            let mut history = std::mem::take(&mut self.history);
            if !self.date.is_empty() && self.date != today {
                history.retain(|d| d.date != self.date);
                history.push(DayTotal { date: self.date.clone(), seconds: self.seconds });
                let extra = history.len().saturating_sub(7);
                history.drain(..extra);
            }
            *self = ScreenTime {
                date: today,
                hours: vec![0; 24],
                history,
                ..Default::default()
            };
        }
    }

    /// `app`: öndeki pencerenin exe yolu ve başlığı.
    pub fn tick(&mut self, secs: u64, app: Option<(String, String)>) {
        self.roll();
        let now = chrono::Local::now();
        if self.first.is_empty() {
            self.first = now.format("%H:%M").to_string();
        }
        self.seconds += secs;
        self.hours[now.hour() as usize] += secs;
        let Some((exe, title)) = app else { return };
        let lower = exe.to_lowercase();
        if lower.ends_with("\\explorer.exe") && title.is_empty() {
            return; // masaüstü
        }
        // Store uygulamaları ortak çerçeve sürecinde çalışır: adı başlıktan al.
        let host = lower.ends_with("\\applicationframehost.exe");
        let (key, name) = if host {
            let t = title.rsplit(" - ").next().unwrap_or(&title).trim().to_string();
            (format!("app:{}", t.to_lowercase()), t)
        } else {
            (lower, base_name(&exe))
        };
        if key != self.last_app {
            if !self.last_app.is_empty() {
                self.switches += 1;
            }
            self.last_app = key.clone();
        }
        let e = self.apps.entry(key).or_insert_with(|| AppUse {
            name,
            path: if host { String::new() } else { exe.clone() },
            seconds: 0,
        });
        e.seconds += secs;
    }

    pub fn snapshot(&mut self) -> Self {
        self.roll();
        self.clone()
    }
}
