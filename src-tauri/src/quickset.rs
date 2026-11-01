//! Kendi hızlı ayarlar panelimiz (Win+A yerine): ses ve çıkış aygıtı,
//! parlaklık, Wi-Fi/Bluetooth/uçak modu, Wi-Fi ağları, pil.

use crate::commands::run;
use crate::{dlog, AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[cfg(windows)]
mod sys {
    use super::*;
    use dock_sys::quick;
    pub fn audio() -> Value {
        json!(quick::audio_state())
    }
    pub fn set_volume(v: f32) -> bool {
        quick::set_volume(v)
    }
    pub fn set_mute(m: bool) -> bool {
        quick::set_mute(m)
    }
    pub fn set_device(id: &str) -> bool {
        quick::set_default_device(id)
    }
    pub fn radios() -> Value {
        json!(quick::radios())
    }
    pub fn set_radio(kind: &str, on: bool) -> bool {
        quick::set_radio(kind, on)
    }
    fn rect(r: crate::platform::Rect) -> dock_sys::monitor::Rect {
        dock_sys::monitor::Rect { left: r.0, top: r.1, right: r.2, bottom: r.3 }
    }
    pub fn ddc_get(r: crate::platform::Rect) -> Option<u32> {
        quick::ddc_get(rect(r))
    }
    pub fn ddc_set(r: crate::platform::Rect, v: u32) -> bool {
        quick::ddc_set(rect(r), v)
    }
    pub fn wifi(scan: bool) -> Result<Value, u32> {
        quick::wifi_networks(scan).map(|v| json!(v))
    }
    pub fn wifi_connect(p: &str) -> bool {
        quick::wifi_connect(p)
    }
    pub fn wifi_disconnect() -> bool {
        quick::wifi_disconnect()
    }
    /// Dizüstü ekranı: WMI (PowerShell). Yavaş (~0,5 sn), arka planda çağrılır.
    pub fn wmi_get() -> Option<u32> {
        let out = powershell("(Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue | Select-Object -First 1).CurrentBrightness")?;
        out.trim().parse().ok()
    }
    pub fn wmi_set(v: u32) -> bool {
        powershell(&format!(
            "Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightnessMethods -ErrorAction SilentlyContinue | Invoke-CimMethod -MethodName WmiSetBrightness -Arguments @{{Timeout=1;Brightness={}}} | Out-Null",
            v.min(100)
        ))
        .is_some()
    }
    fn powershell(cmd: &str) -> Option<String> {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

#[cfg(not(windows))]
mod sys {
    use super::*;
    pub fn audio() -> Value {
        json!({ "volume": 40.0, "muted": false, "devices": [] })
    }
    pub fn set_volume(_v: f32) -> bool { false }
    pub fn set_mute(_m: bool) -> bool { false }
    pub fn set_device(_id: &str) -> bool { false }
    pub fn radios() -> Value { json!([]) }
    pub fn set_radio(_k: &str, _on: bool) -> bool { false }
    pub fn ddc_get(_r: crate::platform::Rect) -> Option<u32> { None }
    pub fn ddc_set(_r: crate::platform::Rect, _v: u32) -> bool { false }
    pub fn wifi(_scan: bool) -> Result<Value, u32> { Ok(json!([])) }
    pub fn wifi_connect(_p: &str) -> bool { false }
    pub fn wifi_disconnect() -> bool { false }
    pub fn wmi_get() -> Option<u32> { None }
    pub fn wmi_set(_v: u32) -> bool { false }
}

#[derive(Default)]
pub struct QuickState {
    /// Uçak modundan önce açık olan radyolar
    pub before_airplane: Option<Vec<(String, bool)>>,
    /// Dizüstü ekranının WMI parlaklığı (önbellek)
    pub wmi: Option<Option<u32>>,
}

fn monitors(app: &AppHandle) -> Vec<(String, String, crate::platform::Rect, bool)> {
    crate::docks::monitors(app)
        .into_iter()
        .map(|m| (m.name, m.title, m.rect, m.primary))
        .collect()
}

#[tauri::command]
pub async fn quick_state(app: AppHandle) -> Value {
    run(move || {
        let state = app.state::<AppState>();
        let audio = sys::audio();
        let radios = sys::radios();
        // Parlaklık: önce DDC/CI (harici monitör), olmazsa ana ekranda WMI (dizüstü)
        let mut bright = Vec::new();
        let wmi_cached = state.quick.lock().unwrap().wmi;
        for (name, title, rect, primary) in monitors(&app) {
            if let Some(v) = sys::ddc_get(rect) {
                bright.push(json!({ "id": name, "title": title, "kind": "ddc", "value": v }));
            } else if primary {
                let v = match wmi_cached {
                    Some(v) => v,
                    None => {
                        let v = sys::wmi_get();
                        state.quick.lock().unwrap().wmi = Some(v);
                        v
                    }
                };
                if let Some(v) = v {
                    bright.push(json!({ "id": name, "title": title, "kind": "wmi", "value": v }));
                }
            }
        }
        let info = crate::platform::sys_info();
        let airplane = state.quick.lock().unwrap().before_airplane.is_some();
        json!({
            "audio": audio,
            "radios": radios,
            "brightness": bright,
            "battery": info.battery,
            "charging": info.charging,
            "airplane": airplane,
        })
    })
    .await
    .unwrap_or(Value::Null)
}

#[tauri::command]
pub async fn quick_volume(volume: Option<f32>, muted: Option<bool>) -> bool {
    run(move || {
        let mut ok = true;
        if let Some(v) = volume {
            ok &= sys::set_volume(v);
        }
        if let Some(m) = muted {
            ok &= sys::set_mute(m);
        }
        ok
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
pub async fn quick_audio_device(id: String) -> bool {
    run(move || {
        let ok = sys::set_device(&id);
        dlog!("ses çıkışı değiştirildi: {} ({})", id, if ok { "tamam" } else { "BAŞARISIZ" });
        ok
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
pub async fn quick_brightness(app: AppHandle, id: String, kind: String, value: u32) -> bool {
    run(move || {
        if kind == "wmi" {
            app.state::<AppState>().quick.lock().unwrap().wmi = Some(Some(value));
            return sys::wmi_set(value);
        }
        let rect = monitors(&app).into_iter().find(|m| m.0 == id).map(|m| m.2);
        rect.map(|r| sys::ddc_set(r, value)).unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

/// kind: "wifi" | "bluetooth" | "airplane"
#[tauri::command]
pub async fn quick_radio(app: AppHandle, kind: String, on: bool) -> bool {
    run(move || {
        let state = app.state::<AppState>();
        if kind == "airplane" {
            if on {
                // Önceki durumları hatırla, hepsini kapat
                let before: Vec<(String, bool)> = serde_json::from_value::<Vec<Value>>(sys::radios())
                    .unwrap_or_default()
                    .iter()
                    .map(|r| (r["kind"].as_str().unwrap_or("").to_string(), r["on"].as_bool().unwrap_or(false)))
                    .collect();
                state.quick.lock().unwrap().before_airplane = Some(before);
                return sys::set_radio("all", false);
            }
            let before = state.quick.lock().unwrap().before_airplane.take().unwrap_or_default();
            let mut ok = false;
            for (k, was_on) in before {
                if was_on {
                    ok |= sys::set_radio(&k, true);
                }
            }
            return ok;
        }
        sys::set_radio(&kind, on)
    })
    .await
    .unwrap_or(false)
}

#[tauri::command]
pub async fn wifi_list(scan: bool) -> Value {
    run(move || match sys::wifi(scan) {
        Ok(v) => json!({ "ok": true, "networks": v }),
        Err(code) => json!({ "ok": false, "code": code }),
    })
    .await
    .unwrap_or(Value::Null)
}

#[tauri::command]
pub async fn wifi_connect(profile: String) -> bool {
    run(move || sys::wifi_connect(&profile)).await.unwrap_or(false)
}

#[tauri::command]
pub async fn wifi_disconnect() -> bool {
    run(sys::wifi_disconnect).await.unwrap_or(false)
}
