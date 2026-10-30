//! Tanılama kaydı: %APPDATA%\com.ali.camdock\dock.log
//! Dock neden hareket etti, cam katmanı kuruldu mu gibi olayları yazar.
//! Uzaktan hata ayıklamak için: Ayarlar → Hakkında → Tanılama kaydı.

use std::io::Write;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 256 * 1024;

pub fn path() -> std::path::PathBuf {
    crate::config::base_dir().join("dock.log")
}

pub fn write(msg: impl AsRef<str>) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path();
    let too_big = std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES).unwrap_or(false);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!too_big)
        .truncate(too_big)
        .open(&p);
    if let Ok(mut f) = file {
        let now = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "{now}  {}", msg.as_ref());
    }
}

#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => { $crate::log::write(format!($($arg)*)) };
}
