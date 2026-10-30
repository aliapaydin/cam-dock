//! Cam arka planı: masaüstünü dock'un arkasında bulanıklaştırır.
//!
//! Windows 11'in resmi Acrylic/Mica arka planları (DWMWA_SYSTEMBACKDROP_TYPE)
//! pencere odakta değilken düz renge düşer. Dock hiçbir zaman odakta olmadığı
//! için TranslucentTB gibi araçların da kullandığı
//! `SetWindowCompositionAttribute` (ACCENT_ENABLE_ACRYLICBLURBEHIND) yolunu
//! kullanıyoruz; bu her zaman aktif kalır.

use crate::{hwnd, wide};
use core::ffi::c_void;
use windows::core::{BOOL, PCSTR, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    /// Bulanıklık yok; sadece CSS'teki yarı saydam renk.
    None,
    /// Klasik bulanıklık (Windows 10 "blur behind").
    Blur,
    /// Acrylic: bulanıklık + doygunluk + hafif doku.
    Acrylic,
}

impl Backdrop {
    pub fn parse(s: &str) -> Self {
        match s {
            "none" => Backdrop::None,
            "blur" => Backdrop::Blur,
            _ => Backdrop::Acrylic,
        }
    }
}

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct CompositionAttribData {
    attrib: u32,
    pv_data: *mut c_void,
    cb_data: usize,
}

type SwcaFn = unsafe extern "system" fn(HWND, *mut CompositionAttribData) -> BOOL;

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_DISABLED: u32 = 0;
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

fn swca() -> Option<SwcaFn> {
    unsafe {
        let name = wide("user32.dll");
        let module = LoadLibraryW(PCWSTR(name.as_ptr())).ok()?;
        let proc = GetProcAddress(module, PCSTR(b"SetWindowCompositionAttribute\0".as_ptr()))?;
        Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, SwcaFn>(proc))
    }
}

/// Arka planı uygular. `tint` = (r, g, b, a) — Acrylic'in üstüne binen renk.
/// Alfa 0 olursa bazı Windows sürümlerinde efekt tamamen kaybolur; en az 0x10 kullan.
pub fn apply(window: isize, mode: Backdrop, tint: (u8, u8, u8, u8)) -> bool {
    let Some(set) = swca() else {
        return false;
    };
    let (r, g, b, a) = tint;
    let a = a.max(0x10);
    let mut policy = AccentPolicy {
        accent_state: match mode {
            Backdrop::None => ACCENT_DISABLED,
            Backdrop::Blur => ACCENT_ENABLE_BLURBEHIND,
            Backdrop::Acrylic => ACCENT_ENABLE_ACRYLICBLURBEHIND,
        },
        // 2 = gradient_color'ı kullan
        accent_flags: 2,
        // AABBGGRR
        gradient_color: (a as u32) << 24 | (b as u32) << 16 | (g as u32) << 8 | r as u32,
        animation_id: 0,
    };
    let mut data = CompositionAttribData {
        attrib: WCA_ACCENT_POLICY,
        pv_data: &mut policy as *mut _ as *mut c_void,
        cb_data: std::mem::size_of::<AccentPolicy>(),
    };
    unsafe { set(hwnd(window), &mut data).as_bool() }
}

/// Pencereyi yuvarlak köşeli bir bölgeyle kırpar (fiziksel piksel).
/// Windows 11'in kendi 8px köşe yuvarlamasını ve 1px kenarlığını kapatır;
/// köşeyi ve kenar ışığını CSS çiziyor.
pub fn round_region(window: isize, width: i32, height: i32, radius: i32) {
    unsafe {
        let pref = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd(window),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const c_void,
            std::mem::size_of_val(&pref) as u32,
        );
        let none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
        let _ = DwmSetWindowAttribute(
            hwnd(window),
            DWMWA_BORDER_COLOR,
            &none as *const _ as *const c_void,
            4,
        );
        let d = radius * 2;
        let rgn = CreateRoundRectRgn(0, 0, width + 1, height + 1, d, d);
        // Başarılı olursa bölgenin sahipliği sisteme geçer, silmiyoruz.
        SetWindowRgn(hwnd(window), Some(rgn), true);
    }
}
