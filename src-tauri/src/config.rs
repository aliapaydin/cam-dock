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

/// Dock'taki kayan "piyasa ve haberler" hücresi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ticker {
    /// truncgil anahtarları: USD, EUR, GBP, CHF, gram-altin, ceyrek-altin
    pub markets: Vec<String>,
    pub news: bool,
    /// RSS/Atom adresleri
    pub feeds: Vec<String>,
    /// Her öğenin ekranda kalma süresi (sn)
    pub interval_sec: u64,
    /// Her turda piyasalar arasına kaç haber girsin
    pub news_per_round: u32,
}

impl Default for Ticker {
    fn default() -> Self {
        Self {
            markets: vec!["USD".into(), "EUR".into(), "gram-altin".into()],
            news: true,
            feeds: vec!["https://news.google.com/rss?hl=tr&gl=TR&ceid=TR:tr".into()],
            interval_sec: 6,
            news_per_round: 3,
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
    /// Kilit açıkken kullanılan gizlenme modu: "autohide" (otomatik gizle) |
    /// "smart" (akıllı: bir pencere dock'a değince gizlen). Eski "always" değeri
    /// kilitli kabul edilir.
    pub dock_visibility: String,
    /// Kilitli: dock hep görünür ve ekranın altında alan ayrılır (pencereler
    /// dock'un altına girmez). Dock'taki kilit düğmesiyle açılıp kapanır.
    pub dock_locked: bool,
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
    /// calendar, screentime, ticker, media, system, clock
    /// Önizleme (örnek veri, Faz 2'de bağlanacak): mail, claude
    pub widgets: Vec<String>,
    pub ticker: Ticker,
    /// Ayar dosyası sürüm geçişleri (kullanıcı düzenlemez)
    pub migrations: u32,
}

impl Config {
    /// Şu an geçerli görünürlük: "always" | "autohide" | "smart"
    pub fn effective_visibility(&self) -> &str {
        if self.dock_locked {
            "always"
        } else if self.dock_visibility == "always" {
            "autohide"
        } else {
            &self.dock_visibility
        }
    }
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
            dock_visibility: "autohide".into(),
            dock_locked: true,
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
            widgets: ["calendar", "screentime", "ticker", "media", "sysinfo", "system", "clock", "lock"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ticker: Ticker::default(),
            migrations: 3,
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
        Ok(text) => {
            let mut cfg: Config = serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("config.json okunamadı ({e}); varsayılanlar kullanılıyor");
                Config::default()
            });
            // v0.2'deki tek başına döviz hücresi artık "piyasa ve haberler".
            for w in cfg.widgets.iter_mut() {
                if w == "currency" {
                    *w = "ticker".into();
                }
            }
            // v0.5: sistem düğmeleri (tepsi, hızlı ayarlar, bildirimler, taskbar)
            if cfg.migrations < 1 {
                if !cfg.widgets.iter().any(|w| w == "system") {
                    let at = cfg.widgets.iter().position(|w| w == "clock").unwrap_or(cfg.widgets.len());
                    cfg.widgets.insert(at, "system".into());
                }
                cfg.migrations = 1;
                save(&cfg);
            }
            // v0.6: "her zaman görünür" artık kilit düğmesi
            if cfg.migrations < 2 {
                cfg.dock_locked = cfg.dock_visibility == "always";
                if cfg.dock_visibility == "always" {
                    cfg.dock_visibility = "autohide".into();
                }
                if !cfg.widgets.iter().any(|w| w == "lock") {
                    cfg.widgets.push("lock".into());
                }
                cfg.migrations = 2;
                save(&cfg);
            }
            // v0.7: sistem kaynakları hücresi
            if cfg.migrations < 3 {
                if !cfg.widgets.iter().any(|w| w == "sysinfo") {
                    let at = cfg.widgets.iter().position(|w| w == "media").map(|i| i + 1).unwrap_or(cfg.widgets.len());
                    cfg.widgets.insert(at, "sysinfo".into());
                }
                cfg.migrations = 3;
                save(&cfg);
            }
            cfg
        }
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
