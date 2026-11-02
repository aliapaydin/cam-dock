//! Gezgin sağ tık menüsü: .exe ve kısayollarda "Cam Dock'a sabitle".
//!
//! Menüden çağrılan `cam-dock.exe --pin "<yol>"` isteği bir dosyaya yazar;
//! çalışan dock bunu saniyede bir kontrol edip sabitler (Cam Dock tek örnek
//! çalıştığı için ikinci örnek hemen kapanır).

use crate::{config, dlog, platform, AppState};
use tauri::{AppHandle, Emitter, Manager};

const VERB: &str = "CamDockPin";
const CLASSES: [&str; 2] = ["exefile", "lnkfile"];

fn req_path() -> std::path::PathBuf {
    config::base_dir().join("pin-requests.txt")
}

/// `--pin` ile açıldıysa isteği kuyruğa yaz.
pub fn queue_from_args() {
    let args: Vec<String> = std::env::args().collect();
    for (flag, prefix) in [("--pin", ""), ("--stack", "stack:")] {
        if let Some(i) = args.iter().position(|a| a == flag) {
            if let Some(p) = args.get(i + 1) {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(req_path()) {
                    let _ = writeln!(f, "{prefix}{p}");
                }
            }
        }
    }
}

/// Menü kayıtlarını ayara göre ekler/kaldırır (exe yolu değişmiş olabilir).
pub fn sync_menu(enabled: bool) {
    for class in CLASSES {
        let key = format!("Software\\Classes\\{class}\\shell\\{VERB}");
        if !enabled {
            platform::reg_delete_tree(&key);
            continue;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let exe = exe.to_string_lossy().to_string();
        platform::reg_set_sz(&key, None, "Cam Dock'a sabitle");
        platform::reg_set_sz(&key, Some("Icon"), &format!("\"{exe}\",0"));
        platform::reg_set_sz(&format!("{key}\\command"), None, &format!("\"{exe}\" --pin \"%1\""));
    }
    // Klasörler: "Cam Dock'a yığın olarak ekle"
    let key = "Software\\Classes\\Directory\\shell\\CamDockStack";
    if !enabled {
        platform::reg_delete_tree(key);
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().to_string();
    platform::reg_set_sz(key, None, "Cam Dock'a yığın olarak ekle");
    platform::reg_set_sz(key, Some("Icon"), &format!("\"{exe}\",0"));
    platform::reg_set_sz(&format!("{key}\\command"), None, &format!("\"{exe}\" --stack \"%1\""));
}

fn stem(p: &str) -> String {
    let f = p.rsplit(['\\', '/']).next().unwrap_or(p);
    let s = f.rsplit_once('.').map(|(a, _)| a).unwrap_or(f);
    s.trim().to_string()
}

/// Kuyruktaki istekleri sabitler. Watcher'dan saniyede bir çağrılır.
pub fn process(app: &AppHandle) {
    let path = req_path();
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let _ = std::fs::remove_file(&path);
    let mut added = Vec::new();
    let mut stacks_to_add: Vec<String> = Vec::new();
    {
        let state = app.state::<AppState>();
        let mut cfg = state.cfg.lock().unwrap();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if let Some(dir) = line.strip_prefix("stack:") {
                stacks_to_add.push(dir.to_string());
                continue;
            }
            let raw = line.trim_matches('"').to_string();
            let lower = raw.to_lowercase();
            // Kısayolun adı okunaklı ("WhatsApp"); hedef exe ise pencereyle eşleşsin diye exe'yi sabitle.
            let name = stem(&raw);
            let target = if lower.ends_with(".lnk") {
                platform::resolve_shortcut(&raw)
                    .filter(|t| t.to_lowercase().ends_with(".exe") && std::path::Path::new(t).exists())
                    .unwrap_or_else(|| raw.clone())
            } else {
                raw.clone()
            };
            let exists = cfg.pinned.iter().any(|p| config::expand_env(&p.path).eq_ignore_ascii_case(&target));
            if exists {
                continue;
            }
            cfg.pinned.push(config::Pinned { name: name.clone(), path: target, args: None });
            added.push(name);
        }
        if !added.is_empty() {
            config::save(&cfg);
        }
    }
    for d in stacks_to_add {
        if crate::stacks::add(app, &d) {
            let name = d.trim_matches('"').trim_end_matches('\\').rsplit('\\').next().unwrap_or("").to_string();
            added.push(format!("{name} (yığın)"));
        }
    }
    if !added.is_empty() {
        dlog!("Gezgin'den sabitlendi: {}", added.join(", "));
        crate::emit_config(app);
        let _ = app.emit("pinned-added", added);
    }
}
