//! Geçerli koyu/açık tema: kullanıcı seçimi, Windows teması ya da dinamik
//! moddaki saat planı (gün doğumu/batımı veya sabit saatler).

use crate::config::Config;
use chrono::{Datelike, Local, NaiveTime, Offset, Timelike};

/// Gün doğumu ve batımı (yerel saat, gün içindeki dakika). NOAA'nın
/// sadeleştirilmiş güneş formülü; birkaç dakikalık hata tema için yeterli.
pub fn sun_times(lat: f64, lon: f64) -> Option<(f64, f64)> {
    let now = Local::now();
    let n = now.ordinal() as f64;
    let tz = now.offset().fix().local_minus_utc() as f64 / 3600.0;
    let g = 2.0 * std::f64::consts::PI / 365.0 * (n - 1.0 + (12.0 - 12.0) / 24.0);
    let eqtime = 229.18
        * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos()
            - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos()
        + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos()
        + 0.00148 * (3.0 * g).sin();
    let latr = lat.to_radians();
    let zenith = 90.833f64.to_radians();
    let cos_ha = zenith.cos() / (latr.cos() * decl.cos()) - latr.tan() * decl.tan();
    if !(-1.0..=1.0).contains(&cos_ha) {
        return None; // kutup gecesi / gündüzü
    }
    let ha = cos_ha.acos().to_degrees();
    let rise = 720.0 - 4.0 * (lon + ha) - eqtime + tz * 60.0;
    let set = 720.0 - 4.0 * (lon - ha) - eqtime + tz * 60.0;
    Some((rise, set))
}

fn minutes(s: &str) -> Option<f64> {
    let t = NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()?;
    Some((t.hour() * 60 + t.minute()) as f64)
}

/// Dinamik modda şu an koyu mu olmalı?
fn scheduled_dark(cfg: &Config) -> bool {
    let now = Local::now();
    let m = (now.hour() * 60 + now.minute()) as f64;
    let (light, dark) = if cfg.dynamic.schedule == "fixed" {
        (minutes(&cfg.dynamic.light_at).unwrap_or(420.0), minutes(&cfg.dynamic.dark_at).unwrap_or(1170.0))
    } else {
        sun_times(cfg.weather.lat, cfg.weather.lon).unwrap_or((420.0, 1170.0))
    };
    if light <= dark {
        !(m >= light && m < dark)
    } else {
        m >= dark && m < light
    }
}

pub fn effective_dark(cfg: &Config, system_light: Option<bool>) -> bool {
    if cfg.dynamic.enabled && cfg.dynamic.auto_theme {
        return scheduled_dark(cfg);
    }
    match cfg.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => !system_light.unwrap_or(false),
    }
}
