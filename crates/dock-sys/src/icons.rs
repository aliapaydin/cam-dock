//! Bir exe/dosyanın ikonunu PNG olarak çıkarır.

use crate::wide;
use core::ffi::c_void;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, PrivateExtractIconsW, DI_NORMAL, HICON};

/// `path` içindeki ilk ikonu `size`×`size` PNG olarak döndürür.
pub fn icon_png(path: &str, size: i32) -> Option<Vec<u8>> {
    let w = wide(path);
    if w.len() > 260 {
        return None;
    }
    let mut name = [0u16; 260];
    name[..w.len()].copy_from_slice(&w);

    let mut icons = [HICON::default()];
    let got = unsafe { PrivateExtractIconsW(&name, 0, size, size, Some(&mut icons), None, 0) };
    if got == 0 || got == u32::MAX || icons[0].is_invalid() {
        return None;
    }
    let icon = icons[0];
    let pixels = unsafe { render(icon, size) };
    unsafe {
        let _ = DestroyIcon(icon);
    }
    encode_png(&pixels?, size as u32)
}

/// İkonu 32 bit bir DIB'e çizip BGRA (ön-çarpılmış) pikselleri döndürür.
unsafe fn render(icon: HICON, size: i32) -> Option<Vec<u8>> {
    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        return None;
    }
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size, // yukarıdan aşağı
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let Ok(bmp) = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
        let _ = DeleteDC(dc);
        return None;
    };
    let old = SelectObject(dc, HGDIOBJ(bmp.0));
    let drawn = DrawIconEx(dc, 0, 0, icon, size, size, 0, None, DI_NORMAL).is_ok();
    let len = (size * size * 4) as usize;
    let out = if drawn && !bits.is_null() {
        Some(std::slice::from_raw_parts(bits as *const u8, len).to_vec())
    } else {
        None
    };
    SelectObject(dc, old);
    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
    out
}

fn encode_png(bgra: &[u8], size: u32) -> Option<Vec<u8>> {
    let mut rgba = Vec::with_capacity(bgra.len());
    let no_alpha = bgra.chunks_exact(4).all(|p| p[3] == 0);
    for p in bgra.chunks_exact(4) {
        let (b, g, r, a) = (p[0], p[1], p[2], p[3]);
        if no_alpha {
            // Eski tip (alfa kanalsız) ikon: boş olmayan pikselleri opak say.
            let opaque = r | g | b != 0;
            rgba.extend_from_slice(&[r, g, b, if opaque { 255 } else { 0 }]);
        } else if a == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            // Ön-çarpılmış alfayı geri aç.
            let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            rgba.extend_from_slice(&[un(r), un(g), un(b), a]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&rgba).ok()?;
    }
    Some(out)
}
