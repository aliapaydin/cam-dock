//! Kullanıcı ayarları: %APPDATA%\com.ali.camdock\config.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Pinned {
    pub name: String,
    /// exe, kısayol (.lnk), klasör ya da URI. %APPDATA% gibi değişkenler açılır.
    pub path: String,
    pub args: Option<String>,
}

impl Default for Pinned {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            args: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Currency {
    pub amount: f64,
    pub from: String,
    pub to: String,
}

impl Default for Currency {
    fn default() -> Self {
        Self {
            amount: 100.0,
            from: "USD".into(),
            to: "TRY".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Display {
    /// "primary" (sadece ana ekran) | "all" (tüm ekranlar) | "selected"
    pub mode: String,
    /// mode = "selected" iken dock gösterilecek ekranların aygıt adları ("\\.\DISPLAY2")
    pub selected: Vec<String>,
    /// Her dock yalnızca kendi ekranındaki açık pencereleri göstersin
    pub apps_per_monitor: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            mode: "primary".into(),
            selected: Vec::new(),
            apps_per_monitor: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "system" | "dark" | "light"
    pub theme: String,
    /// "acrylic" | "blur" | "none"
    pub backdrop: String,
    /// Klasik taskbar'ı gizle (dock olan ekranlarda)
    pub hide_taskbar: bool,
    /// "always" (her zaman görünür) | "autohide" (otomatik gizle) |
    /// "smart" (akıllı: bir pencere dock'a değince gizlen)
    pub dock_visibility: String,
    /// Fare dock'tan ayrıldıktan sonra gizlenme gecikmesi (ms)
    pub hide_delay_ms: u64,
    /// Tam ekran uygulamada (oyun, video) dock'u gizle
    pub hide_in_fullscreen: bool,
    pub display: Display,
    /// Ekranın altında dock için alan ayır (maximize pencereler dock'un altına girmez).
    /// Yalnızca "always" modunda geçerlidir.
    pub reserve_space: bool,
    /// Ekran altı ile dock arası boşluk (px)
    pub bottom_margin: u32,
    /// Dock köşe yarıçapı (px)
    pub radius: u32,
    pub accent: String,
    /// Sabitlenmemiş açık uygulamaları da göster
    pub show_running: bool,
    pub pinned: Vec<Pinned>,
    /// Sırasıyla gösterilecek widget'lar:
    /// calendar, screentime, currency, media, clock
    /// Önizleme (örnek veri, Faz 2'de bağlanacak): mail, claude
    pub widgets: Vec<String>,
    pub currency: Currency,
}

impl Default for Config {
    fn default() -> Self {
        let p = |name: &str, path: &str| Pinned {
            name: name.into(),
            path: path.into(),
            args: None,
        };
        Self {
            theme: "system".into(),
            backdrop: "acrylic".into(),
            hide_taskbar: true,
            dock_visibility: "always".into(),
            hide_delay_ms: 600,
            hide_in_fullscreen: true,
            display: Display::default(),
            reserve_space: true,
            bottom_margin: 10,
            radius: 26,
            accent: "#2F7BFF".into(),
            show_running: true,
            pinned: vec![
                p("Dosya Gezgini", "C:\\Windows\\explorer.exe"),
                p("Chrome", "%ProgramFiles%\\Google\\Chrome\\Application\\chrome.exe"),
                p("Edge", "%ProgramFiles(x86)%\\Microsoft\\Edge\\Application\\msedge.exe"),
                p("VS Code", "%LOCALAPPDATA%\\Programs\\Microsoft VS Code\\Code.exe"),
                p("Spotify", "%APPDATA%\\Spotify\\Spotify.exe"),
                p("Telegram", "%APPDATA%\\Telegram Desktop\\Telegram.exe"),
            ],
            widgets: ["calendar", "screentime", "currency", "media", "clock"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            currency: Currency::default(),
        }
    }
}

/// %APPDATA%\com.ali.camdock — Tauri'nin app_config_dir'i ile aynı yer.
pub fn base_dir() -> PathBuf {
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir());
    let dir = root.join("com.ali.camdock");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    base_dir().join("config.json")
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("config.json okunamadı ({e}); varsayılanlar kullanılıyor");
            Config::default()
        }),
        Err(_) => {
            let cfg = Config::default();
            save(&cfg);
            cfg
        }
    }
}

pub fn save(cfg: &Config) {
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(config_path(), text);
    }
}

/// "%APPDATA%\x" gibi ortam değişkenlerini açar.
pub fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('%') {
            Some(j) => {
                let var = &after[..j];
                match std::env::var(var) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(var);
                        out.push('%');
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
