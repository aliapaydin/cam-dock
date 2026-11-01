//! Anonim kullanım istatistiği → sifirgecikme.com/api/camdock
//!
//! Gönderilenler: rastgele kurulum kimliği, sürüm, Windows sürümü, ekran sayısı,
//! açık widget'lar. Sunucu yalnızca ülke kodunu ekler; IP adresi saklanmaz.
//! Varsayılan açık; kurulumda sorulur, Ayarlar → Hakkında'dan kapatılır.
//! Kapatınca tek bir "iptal" bildirimi gider ve gönderim durur; "verilerimi
//! sil" sunucudaki tüm kaydı siler ve kimliği yeniler. Güncelleme denetimi
//! bundan etkilenmez.

use crate::{config, dlog, platform, AppState};
use serde_json::json;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const ENDPOINT: &str = "https://sifirgecikme.com/api/camdock";
/// Kurulum programının bıraktığı seçim (bir kez okunup silinir)
const REG_KEY: &str = "Software\\CamDock";

fn post(body: serde_json::Value) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .redirects(0)
        .build();
    let mut url = ENDPOINT.to_string();
    // Alan adı yönlendirmesi (ör. www.) olursa POST gövdesiyle bir kez takip et
    for _ in 0..2 {
        match agent.post(&url).set("User-Agent", "CamDock").set("Content-Type", "application/json").send_string(&body.to_string()) {
            Ok(_) => return Ok(()),
            Err(ureq::Error::Status(code, r)) if (300..400).contains(&code) => {
                let Some(loc) = r.header("Location").map(|s| s.to_string()) else {
                    return Err(format!("yönlendirme ({code})"));
                };
                url = if loc.starts_with("http") { loc } else { format!("https://sifirgecikme.com{loc}") };
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("çok fazla yönlendirme".into())
}

fn os_name() -> String {
    let i = platform::sys_info();
    i.os.split(" · ").next().unwrap_or("").to_string()
}

/// Açılışta: kurulum programındaki seçimi uygula, kimliği kalıcı yap.
pub fn init(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut cfg = state.cfg.lock().unwrap();
    if platform::reg_dword(REG_KEY, "TelemetryPending") == Some(1) {
        let choice = platform::reg_dword(REG_KEY, "Telemetry").unwrap_or(1) != 0;
        cfg.telemetry.enabled = choice;
        cfg.telemetry.notice_shown = true; // kurulumda soruldu
        platform::reg_delete(REG_KEY, "TelemetryPending");
        dlog!("kurulumdaki istatistik seçimi uygulandı: {}", if choice { "açık" } else { "kapalı" });
    }
    if cfg.telemetry.install_id.len() != 36 {
        cfg.telemetry.install_id = uuid::Uuid::new_v4().to_string();
    }
    config::save(&cfg);
}

fn ping(app: &AppHandle) -> bool {
    let (id, widgets) = {
        let state = app.state::<AppState>();
        let cfg = state.cfg.lock().unwrap();
        if !cfg.telemetry.enabled {
            return false;
        }
        (cfg.telemetry.install_id.clone(), cfg.widgets.clone())
    };
    let monitors = crate::docks::monitors(app).len() as i64;
    let res = post(json!({
        "action": "ping",
        "id": id,
        "version": env!("CARGO_PKG_VERSION"),
        "os": os_name(),
        "monitors": monitors,
        "widgets": widgets,
    }));
    if let Err(e) = &res {
        dlog!("istatistik gönderilemedi: {e}");
    }
    res.is_ok()
}

/// 1 dk sonra ilk gönderim, sonra 12 saatte bir (açıkken).
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        let mut last: Option<Instant> = None;
        loop {
            let due = last.map(|t| t.elapsed() >= Duration::from_secs(12 * 3600)).unwrap_or(true);
            if due && ping(&app) {
                last = Some(Instant::now());
            } else if due {
                // Ağ yoksa 30 dk sonra tekrar dene
                last = Some(Instant::now() - Duration::from_secs(12 * 3600 - 30 * 60));
            }
            std::thread::sleep(Duration::from_secs(10 * 60));
        }
    });
}

/// Ayarlardan aç/kapa değişti.
pub fn on_toggle(app: &AppHandle, enabled: bool) {
    let app = app.clone();
    std::thread::spawn(move || {
        if enabled {
            ping(&app);
        } else {
            let id = app.state::<AppState>().cfg.lock().unwrap().telemetry.install_id.clone();
            let r = post(json!({ "action": "revoke", "id": id, "version": env!("CARGO_PKG_VERSION") }));
            dlog!("istatistik izni kapatıldı ({})", if r.is_ok() { "bildirildi" } else { "bildirilemedi" });
        }
    });
}

/// Sunucudaki tüm veriyi sil, kimliği yenile, gönderimi kapat.
#[tauri::command]
pub async fn telemetry_delete(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let id = app.state::<AppState>().cfg.lock().unwrap().telemetry.install_id.clone();
        post(json!({ "action": "delete", "id": id, "version": env!("CARGO_PKG_VERSION") }))?;
        {
            let state = app.state::<AppState>();
            let mut cfg = state.cfg.lock().unwrap();
            cfg.telemetry.enabled = false;
            cfg.telemetry.install_id = uuid::Uuid::new_v4().to_string();
            config::save(&cfg);
        }
        dlog!("istatistik verileri silindi, kimlik yenilendi");
        crate::emit_config(&app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn telemetry_notice_seen(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut cfg = state.cfg.lock().unwrap();
    cfg.telemetry.notice_shown = true;
    config::save(&cfg);
}

/// Kaldırma programından çağrılır (`--uninstall-ping`): izin açıksa kaldırıldığını bildir.
pub fn uninstall_ping() {
    let cfg = config::load();
    if cfg.telemetry.enabled && cfg.telemetry.install_id.len() == 36 {
        let _ = post(json!({ "action": "uninstall", "id": cfg.telemetry.install_id, "version": env!("CARGO_PKG_VERSION") }));
    }
}
