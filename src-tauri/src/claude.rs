//! Claude kullanım limitleri (5 saatlik oturum, haftalık).
//!
//! Resmî bir API yok. Bilgisayarda Claude Code oturum açmışsa, Claude Code'un
//! kendi /usage ekranı için kullandığı uç noktadan (resmî olmayan, haber
//! vermeden değişebilir) aynı yüzdeleri okuyoruz. Haftalık limit claude.ai ve
//! Claude Code'da ortak olduğu için sohbet kullanımını da kapsar.
//!
//! Erişim anahtarını yalnızca okuruz; asla yenilemeyiz (yenilemek Claude
//! Code'un oturumunu bozabilir). Anahtarın süresi dolduysa son bilinen
//! değerleri gösterip Claude Code'un bir kez açılmasını bekleriz.

use crate::{dlog, AppState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Limit {
    pub key: String,
    pub label: String,
    pub percent: f64,
    /// ISO 8601
    pub resets_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    /// loading | ok | stale | no_claude_code | expired | error
    pub state: String,
    pub plan: String,
    pub limits: Vec<Limit>,
    /// Son başarılı okuma (unix ms)
    pub updated: i64,
    pub message: String,
    #[serde(skip)]
    next_try: Option<Instant>,
}

fn claude_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".claude"))
}

fn cache_path() -> PathBuf {
    crate::config::base_dir().join("claude-usage.json")
}

struct Creds {
    token: String,
    expires: i64,
    plan: String,
}

fn creds() -> Option<Creds> {
    let text = std::fs::read_to_string(claude_dir()?.join(".credentials.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let o = &v["claudeAiOauth"];
    Some(Creds {
        token: o["accessToken"].as_str()?.to_string(),
        expires: o["expiresAt"].as_i64().unwrap_or(0),
        plan: o["subscriptionType"].as_str().unwrap_or("").to_string(),
    })
}

fn plan_name(p: &str) -> String {
    match p.to_lowercase().as_str() {
        "max" => "Max".into(),
        "pro" => "Pro".into(),
        "team" => "Team".into(),
        "enterprise" => "Enterprise".into(),
        "" => String::new(),
        other => other.to_string(),
    }
}

fn parse(v: &serde_json::Value) -> Vec<Limit> {
    let mut out = Vec::new();
    // Yeni biçim: kendini tanımlayan "limits" dizisi
    if let Some(arr) = v["limits"].as_array() {
        for l in arr {
            let kind = l["kind"].as_str().unwrap_or("");
            let pct = l["percent"].as_f64().or_else(|| l["utilization"].as_f64());
            let Some(pct) = pct else { continue };
            let model = l["scope"]["model"]["display_name"].as_str().unwrap_or("");
            let label = match kind {
                "session" => "Oturum".to_string(),
                "weekly_all" => "Haftalık".to_string(),
                "weekly_scoped" if !model.is_empty() => model.to_string(),
                _ => kind.replace('_', " "),
            };
            out.push(Limit {
                key: format!("{kind}:{model}"),
                label,
                percent: pct,
                resets_at: l["resets_at"].as_str().unwrap_or("").to_string(),
            });
        }
        if !out.is_empty() {
            return out;
        }
    }
    // Eski biçim
    for (key, label) in [
        ("five_hour", "Oturum"),
        ("seven_day", "Haftalık"),
        ("seven_day_opus", "Opus"),
        ("seven_day_sonnet", "Sonnet"),
    ] {
        if let Some(p) = v[key]["utilization"].as_f64() {
            out.push(Limit {
                key: key.into(),
                label: label.into(),
                percent: p,
                resets_at: v[key]["resets_at"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    out
}

fn set(app: &AppHandle, f: impl FnOnce(&mut Usage)) {
    let state = app.state::<AppState>();
    let snap = {
        let mut u = state.claude.lock().unwrap();
        f(&mut u);
        u.clone()
    };
    let _ = app.emit("claude-usage", snap);
}

fn fetch(app: &AppHandle) {
    let now = chrono::Utc::now().timestamp_millis();
    let Some(c) = creds() else {
        set(app, |u| {
            u.state = if u.updated > 0 { "stale".into() } else { "no_claude_code".into() };
            u.message = "Claude Code bu bilgisayarda oturum açmamış".into();
        });
        return;
    };
    if c.expires > 0 && c.expires < now {
        set(app, |u| {
            u.plan = plan_name(&c.plan);
            u.state = "expired".into();
            u.message = "Claude Code oturum anahtarının süresi dolmuş; Claude Code'u bir kez açınca yenilenir".into();
        });
        return;
    }
    let res = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build()
        .get("https://api.anthropic.com/api/oauth/usage")
        .set("Authorization", &format!("Bearer {}", c.token))
        .set("anthropic-beta", "oauth-2025-04-20")
        .set("Accept", "application/json")
        .call();
    match res {
        Ok(r) => {
            let text = r.into_string().unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            let limits = parse(&v);
            if limits.is_empty() {
                set(app, |u| {
                    u.state = "error".into();
                    u.message = "Yanıt anlaşılamadı (biçim değişmiş olabilir)".into();
                });
                return;
            }
            set(app, |u| {
                u.state = "ok".into();
                u.plan = plan_name(&c.plan);
                u.limits = limits;
                u.updated = now;
                u.message.clear();
            });
            let snap = app.state::<AppState>().claude.lock().unwrap().clone();
            if let Ok(t) = serde_json::to_string(&snap) {
                let _ = std::fs::write(cache_path(), t);
            }
        }
        Err(ureq::Error::Status(429, _)) => {
            dlog!("Claude kullanımı: çok sık istek (429), 30 dk bekleniyor");
            set(app, |u| {
                u.state = if u.updated > 0 { "stale".into() } else { "error".into() };
                u.message = "Çok sık soruldu; 30 dakika sonra tekrar denenecek".into();
                u.next_try = Some(Instant::now() + Duration::from_secs(30 * 60));
            });
        }
        Err(ureq::Error::Status(401 | 403, _)) => set(app, |u| {
            u.state = "expired".into();
            u.message = "Claude Code oturumu geçersiz; Claude Code'u bir kez açınca yenilenir".into();
        }),
        Err(e) => set(app, |u| {
            u.state = if u.updated > 0 { "stale".into() } else { "error".into() };
            u.message = format!("Bağlanılamadı: {e}");
        }),
    }
}

fn due(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let u = state.claude.lock().unwrap();
    u.next_try.map(|t| Instant::now() >= t).unwrap_or(true)
}

pub fn spawn(app: AppHandle) {
    // Önceki değerleri hemen göster
    if let Ok(t) = std::fs::read_to_string(cache_path()) {
        if let Ok(mut u) = serde_json::from_str::<Usage>(&t) {
            u.state = "stale".into();
            *app.state::<AppState>().claude.lock().unwrap() = u;
        }
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        loop {
            let wanted = app.state::<AppState>().cfg.lock().unwrap().widgets.iter().any(|w| w == "claude");
            if wanted && due(&app) {
                fetch(&app);
                let state = app.state::<AppState>();
                let mut u = state.claude.lock().unwrap();
                if u.next_try.map(|t| t <= Instant::now()).unwrap_or(true) {
                    // Uç nokta sık istekleri hemen kısıtlıyor: 5 dakikada bir yeter.
                    u.next_try = Some(Instant::now() + Duration::from_secs(5 * 60));
                }
            }
            std::thread::sleep(Duration::from_secs(20));
        }
    });
}

#[tauri::command]
pub fn claude_usage(app: AppHandle) -> Usage {
    let mut u = app.state::<AppState>().claude.lock().unwrap().clone();
    if u.state.is_empty() {
        u.state = "loading".into();
    }
    u
}

/// Elle yenile (en erken 1 dakikada bir).
#[tauri::command]
pub async fn claude_refresh(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        {
            let state = app.state::<AppState>();
            let u = state.claude.lock().unwrap();
            if u.updated > 0 && chrono::Utc::now().timestamp_millis() - u.updated < 60_000 {
                return;
            }
            if u.next_try.map(|t| t > Instant::now() + Duration::from_secs(5 * 60)).unwrap_or(false) {
                return; // 429 bekleme süresi
            }
        }
        fetch(&app);
    })
    .await;
}
