//! Basit ekran süresi: bilgisayar başında aktif olunan süre (2 dk'dan uzun
//! boşta kalınan süre sayılmaz). Gün değişince sıfırlanır.

use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenTime {
    pub date: String,
    pub seconds: u64,
    /// Saat başına saniye (0–23)
    pub hours: Vec<u64>,
}

fn path() -> PathBuf {
    crate::config::base_dir().join("screentime.json")
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
            *self = ScreenTime {
                date: today,
                seconds: 0,
                hours: vec![0; 24],
            };
        }
    }

    pub fn tick(&mut self, secs: u64) {
        self.roll();
        self.seconds += secs;
        let h = chrono::Local::now().hour() as usize;
        self.hours[h] += secs;
    }

    pub fn snapshot(&mut self) -> Self {
        self.roll();
        self.clone()
    }
}
