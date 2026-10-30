//! Otomatik güncelleme. Kurulum dosyaları açık "cam-dock-releases" deposunun
//! GitHub sürümlerinde yayınlanır (kaynak kod gizli kalır). Yeni sürüm
//! bulununca kurulum dosyası arka planda indirilir; bilgisayar bir süre boşta
//! kalınca (ya da ayarlardan "Şimdi güncelle" ile) sessizce kurulur ve dock
//! yeniden açılır.

use crate::{dlog, AppState};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

const LATEST: &str = "https://api.github.com/repos/aliapaydin/cam-dock-releases/releases/latest";

#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    /// idle | checking | uptodate | downloading | ready | installing | error
    pub state: String,
    pub current: String,
    pub latest: String,
    pub notes: String,
    /// İndirme ilerlemesi (0–1)
    pub progress: f32,
    pub error: String,
    /// Son denetim (yerel saat, "30 Eki 14:05")
    pub checked: String,
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

fn parse(v: &str) -> Vec<u64> {
    v.trim_start_matches(['v', 'V'])
        .split(['.', '-'])
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

fn newer(latest: &str, current: &str) -> bool {
    parse(latest) > parse(current)
}

fn set(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let state = app.state::<AppState>();
    let snap = {
        let mut s = state.update.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("update-status", snap);
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
}

/// Denetle ve gerekiyorsa indir. Bloklar: iş parçacığında çağır.
pub fn check(app: &AppHandle, manual: bool) {
    {
        let s = app.state::<AppState>().update.lock().unwrap().clone();
        if matches!(s.state.as_str(), "checking" | "downloading" | "installing") {
            return;
        }
        if s.state == "ready" && !manual {
            return;
        }
    }
    let current = env!("CARGO_PKG_VERSION").to_string();
    set(app, |s| {
        s.state = "checking".into();
        s.current = current.clone();
        s.error.clear();
    });
    let res: Result<(), String> = (|| {
        let resp = agent()
            .get(LATEST)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "CamDock-Updater")
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404, _) => "Henüz yayınlanmış sürüm yok".to_string(),
                e => format!("Sunucuya ulaşılamadı: {e}"),
            })?;
        let text = resp.into_string().map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let tag = json["tag_name"].as_str().unwrap_or("").to_string();
        let notes = json["body"].as_str().unwrap_or("").to_string();
        let now = chrono::Local::now().format("%d.%m %H:%M").to_string();
        if tag.is_empty() || !newer(&tag, &current) {
            set(app, |s| {
                s.state = "uptodate".into();
                s.latest = tag.trim_start_matches('v').to_string();
                s.checked = now;
            });
            return Ok(());
        }
        let asset = json["assets"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["name"].as_str().map(|n| n.ends_with("-setup.exe")).unwrap_or(false)))
            .ok_or("Sürümde kurulum dosyası yok")?;
        let url = asset["browser_download_url"].as_str().ok_or("İndirme adresi yok")?.to_string();
        let name = asset["name"].as_str().unwrap_or("cam-dock-setup.exe").to_string();
        let size = asset["size"].as_u64().unwrap_or(0);
        let latest = tag.trim_start_matches('v').to_string();
        dlog!("güncelleme bulundu: {current} → {latest}");
        set(app, |s| {
            s.state = "downloading".into();
            s.latest = latest.clone();
            s.notes = notes.clone();
            s.progress = 0.0;
            s.checked = now.clone();
        });

        let dir = std::env::temp_dir().join("camdock-update");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name.replace(' ', "_"));
        let resp = agent()
            .get(&url)
            .set("User-Agent", "CamDock-Updater")
            .call()
            .map_err(|e| format!("İndirme başarısız: {e}"))?;
        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut got: u64 = 0;
        let mut last = std::time::Instant::now();
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("İndirme kesildi: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            got += n as u64;
            if size > 0 && last.elapsed().as_millis() > 300 {
                last = std::time::Instant::now();
                let p = got as f32 / size as f32;
                set(app, |s| s.progress = p);
            }
        }
        drop(file);
        if size > 0 && got != size {
            return Err(format!("İndirilen dosya eksik ({got}/{size} bayt)"));
        }
        dlog!("güncelleme indirildi: {}", path.display());
        set(app, |s| {
            s.state = "ready".into();
            s.progress = 1.0;
            s.file = Some(path);
        });
        Ok(())
    })();
    if let Err(e) = res {
        dlog!("güncelleme denetimi: {e}");
        set(app, |s| {
            s.state = "error".into();
            s.error = e;
            s.checked = chrono::Local::now().format("%d.%m %H:%M").to_string();
        });
    }
}

/// İndirilen kurulumu sessizce çalıştırır, dock'u kapatır; kurulum bitince
/// dock yeniden açılır.
pub fn install(app: &AppHandle) -> bool {
    let Some(file) = app.state::<AppState>().update.lock().unwrap().file.clone() else { return false };
    if !file.is_file() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    dlog!("güncelleme kuruluyor: {}", file.display());
    set(app, |s| s.state = "installing".into());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let script = format!(
            "timeout /t 2 /nobreak >nul & \"{}\" /S & start \"\" \"{}\"",
            file.display(),
            exe.display()
        );
        if std::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(&script)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .is_err()
        {
            set(app, |s| {
                s.state = "error".into();
                s.error = "Kurulum başlatılamadı".into();
            });
            return false;
        }
    }
    #[cfg(not(windows))]
    let _ = exe;
    // Çıkış kodu taşıyan exit engellenmez; cleanup taskbar'ı geri getirir.
    app.exit(0);
    true
}

/// Açılıştan 20 sn sonra ve 6 saatte bir denetler; indirilen güncelleme
/// bilgisayar 3 dakika boşta kalınca kurulur.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(20));
        let mut last_check = std::time::Instant::now() - std::time::Duration::from_secs(7 * 3600);
        loop {
            let (auto_check, auto_install) = {
                let state = app.state::<AppState>();
                let cfg = state.cfg.lock().unwrap();
                (cfg.update.auto_check, cfg.update.auto_install)
            };
            if auto_check && last_check.elapsed() >= std::time::Duration::from_secs(6 * 3600) {
                last_check = std::time::Instant::now();
                check(&app, false);
            }
            let ready = app.state::<AppState>().update.lock().unwrap().state == "ready";
            if ready && auto_install && crate::platform::idle_ms() >= 180_000 && !crate::platform::mouse_down() {
                install(&app);
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    });
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> Status {
    let mut s = app.state::<AppState>().update.lock().unwrap().clone();
    if s.current.is_empty() {
        s.current = env!("CARGO_PKG_VERSION").into();
        if s.state.is_empty() {
            s.state = "idle".into();
        }
    }
    s
}

#[tauri::command]
pub async fn update_check(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || check(&app, true)).await;
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> bool {
    tauri::async_runtime::spawn_blocking(move || install(&app)).await.unwrap_or(false)
}
