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

/// Dinamik mod: renkler duvar kâğıdından, koyu/açık tema saate göre.
/// Kapalıyken `theme` ve `accent` aynen kullanılır (hiçbiri değiştirilmez).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Dynamic {
    pub enabled: bool,
    /// Duvar kâğıdı renginin cama karışma oranı (0–100)
    pub tint: u32,
    /// Koyu/açık geçişini saate göre yap
    pub auto_theme: bool,
    /// "sun" (gün doğumu/batımı, konum hava durumundan) | "fixed"
    pub schedule: String,
    /// "fixed" iken: açık tema başlangıcı ve koyu tema başlangıcı ("HH:MM")
    pub light_at: String,
    pub dark_at: String,
}

impl Default for Dynamic {
    fn default() -> Self {
        Self {
            enabled: false,
            tint: 45,
            auto_theme: true,
            schedule: "sun".into(),
            light_at: "07:00".into(),
            dark_at: "19:30".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Weather {
    pub enabled: bool,
    /// Konumu internet bağlantısından bul
    pub auto_location: bool,
    pub city: String,
    pub lat: f64,
    pub lon: f64,
}

impl Default for Weather {
    fn default() -> Self {
        Self { enabled: true, auto_location: true, city: "İstanbul".into(), lat: 41.0082, lon: 28.9784 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    /// Windows bildirimlerini izle (rozetler)
    pub enabled: bool,
    /// Yeni bildirimde dock'un üstünde cam balon
    pub balloons: bool,
    /// Yeni bildirimde uygulama simgesi zıplasın
    pub bounce: bool,
    /// Zıplama süresi (sn)
    pub bounce_sec: u32,
    /// Balonda mesaj metnini göster (kapalıysa yalnızca uygulama adı)
    pub show_text: bool,
}

impl Default for Notifications {
    fn default() -> Self {
        Self { enabled: true, balloons: true, bounce: true, bounce_sec: 10, show_text: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Clipboard {
    pub enabled: bool,
    /// En çok kaç öğe tutulsun
    pub max_items: u32,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self { enabled: true, max_items: 40 }
    }
}

/// Klasör yığını: dock'ta klasör simgesi, tıklayınca son dosyalar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Stack {
    pub name: String,
    pub path: String,
    /// "grid" (ızgara) | "fan" (yelpaze)
    pub view: String,
    /// "date" (en yeni önce) | "name"
    pub sort: String,
}

impl Default for Stack {
    fn default() -> Self {
        Self { name: String::new(), path: String::new(), view: "grid".into(), sort: "date".into() }
    }
}

pub fn downloads_stack() -> Stack {
    Stack { name: "İndirilenler".into(), path: "%USERPROFILE%\\Downloads".into(), ..Default::default() }
}

/// Anonim kullanım istatistiği (sifirgecikme.com). Varsayılan açık, kapatılabilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Telemetry {
    pub enabled: bool,
    /// Rastgele kurulum kimliği (kişiye bağlanmaz). "Verilerimi sil"de yenilenir.
    pub install_id: String,
    /// Kurulumda sorulmadan gelenlere (eski sürümden güncelleme) bir kez bilgi verildi mi
    pub notice_shown: bool,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self { enabled: true, install_id: uuid::Uuid::new_v4().to_string(), notice_shown: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Update {
    /// Açılışta ve 6 saatte bir yeni sürüm denetle
    pub auto_check: bool,
    /// Yeni sürümü indirip bilgisayar boştayken kendiliğinden kur
    pub auto_install: bool,
}

impl Default for Update {
    fn default() -> Self {
        Self { auto_check: true, auto_install: true }
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
    /// Dock'taki simgelerin elle düzenlenmiş sırası (öğe anahtarları)
    pub app_order: Vec<String>,
    /// Simge düzeni kilitli: sürükleyerek taşınamaz
    pub apps_locked: bool,
    /// Klasör yığınları
    pub stacks: Vec<Stack>,
    /// Mac gibi büyüteç: fare üstündeyken simgeler büyür
    pub magnify: bool,
    /// Görünen en fazla uygulama simgesi (0 = ekran genişliğine göre otomatik)
    pub apps_max_visible: u32,
    /// En büyük ölçek (1.1–1.8)
    pub magnify_scale: f32,
    /// Tepsi uygulamalarının kendi sağ tık menüleri (Shell_NotifyIcon kancası)
    pub tray_menus: bool,
    /// Sırasıyla gösterilecek widget'lar:
    /// calendar, screentime, ticker, media, system, clock
    /// Önizleme (örnek veri, Faz 2'de bağlanacak): mail, claude
    pub widgets: Vec<String>,
    pub ticker: Ticker,
    pub dynamic: Dynamic,
    pub weather: Weather,
    pub update: Update,
    pub notifications: Notifications,
    pub clipboard: Clipboard,
    pub telemetry: Telemetry,
    /// Windows açılınca başlat
    pub autostart: bool,
    /// Gezgin'de .exe ve kısayollara sağ tıklayınca "Cam Dock'a sabitle"
    pub explorer_menu: bool,
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
            app_order: Vec::new(),
            apps_locked: false,
            stacks: vec![downloads_stack()],
            magnify: true,
            magnify_scale: 1.6,
            apps_max_visible: 0,
            tray_menus: true,
            pinned: vec![
                p("Dosya Gezgini", "C:\\Windows\\explorer.exe"),
                p("Chrome", "%ProgramFiles%\\Google\\Chrome\\Application\\chrome.exe"),
                p("Edge", "%ProgramFiles(x86)%\\Microsoft\\Edge\\Application\\msedge.exe"),
                p("VS Code", "%LOCALAPPDATA%\\Programs\\Microsoft VS Code\\Code.exe"),
                p("Spotify", "%APPDATA%\\Spotify\\Spotify.exe"),
                p("Telegram", "%APPDATA%\\Telegram Desktop\\Telegram.exe"),
            ],
            widgets: ["stacks", "live", "calendar", "screentime", "ticker", "media", "clipboard", "system", "clock", "lock"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ticker: Ticker::default(),
            dynamic: Dynamic::default(),
            weather: Weather::default(),
            update: Update::default(),
            notifications: Notifications::default(),
            clipboard: Clipboard::default(),
            telemetry: Telemetry::default(),
            autostart: true,
            explorer_menu: true,
            migrations: 8,
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
            // v0.9: kaynak kullanımı artık ekran süresi hücresinde dönüyor
            if cfg.migrations < 4 {
                let had = cfg.widgets.iter().any(|w| w == "sysinfo");
                cfg.widgets.retain(|w| w != "sysinfo");
                if had && !cfg.widgets.iter().any(|w| w == "screentime") {
                    cfg.widgets.insert(0, "screentime".into());
                }
                cfg.migrations = 4;
                save(&cfg);
            }
            // v0.10: Windows ile başla (taskbar'ın yerine geçtiği için varsayılan açık)
            if cfg.migrations < 5 {
                cfg.autostart = true;
                cfg.migrations = 5;
                save(&cfg);
            }
            // v0.11: pano geçmişi düğmesi (sistem düğmelerinin önüne)
            if cfg.migrations < 6 {
                if !cfg.widgets.iter().any(|w| w == "clipboard") {
                    let at = cfg.widgets.iter().position(|w| w == "system").unwrap_or(cfg.widgets.len());
                    cfg.widgets.insert(at, "clipboard".into());
                }
                cfg.migrations = 6;
                save(&cfg);
            }
            // v0.17: klasör yığınları (İndirilenler) ve canlı etkinlikler
            if cfg.migrations < 7 {
                if !cfg.widgets.iter().any(|w| w == "live") {
                    cfg.widgets.insert(0, "live".into());
                }
                if !cfg.widgets.iter().any(|w| w == "stacks") {
                    cfg.widgets.insert(0, "stacks".into());
                }
                if cfg.stacks.is_empty() {
                    cfg.stacks.push(downloads_stack());
                }
                cfg.migrations = 7;
                save(&cfg);
            }
            // v0.18: büyüteç artık dock'un üstüne taşabiliyor; varsayılan daha güçlü
            if cfg.migrations < 8 {
                if cfg.magnify_scale <= 1.45 {
                    cfg.magnify_scale = 1.6;
                }
                cfg.migrations = 8;
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
