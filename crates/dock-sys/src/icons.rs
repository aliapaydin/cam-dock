//! Bir exe/dosyanın ikonunu PNG olarak çıkarır.

use crate::{hwnd, wide};
use core::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
    SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, DrawIconEx, GetClassLongPtrW, PrivateExtractIconsW, SendMessageTimeoutW,
    DI_NORMAL, GCLP_HICON, HICON, ICON_BIG, ICON_SMALL2, SMTO_ABORTIFHUNG, WM_GETICON,
};

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
    encode_png(&pixels?, size as u32, size as u32)
}

/// Kabuk (Explorer) üzerinden ikon: exe, kısayol, klasör ya da
/// `shell:AppsFolder\<AppUserModelID>` (Store uygulamaları). Store
/// uygulamalarının exe'si korumalı klasörde olduğu için ikon ancak böyle alınır.
pub fn shell_icon_png(parsing_name: &str, size: i32) -> Option<Vec<u8>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let w = wide(parsing_name);
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()?;
        let bmp = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .ok()?;
        let out = bitmap_png(bmp);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        out
    }
}

/// Dosyanın küçük resmi (görsel, video, PDF önizlemesi); yoksa simgesi.
pub fn thumbnail_png(path: &str, size: i32) -> Option<Vec<u8>> {
    use windows::Win32::UI::Shell::SIIGBF_RESIZETOFIT;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let w = wide(path);
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()?;
        let bmp = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK)
            .or_else(|_| factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK))
            .ok()?;
        let out = bitmap_png(bmp);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        out
    }
}

/// Pencerenin kendi ikonu (son çare; küçültülmüş ya da ikonu çıkarılamayan uygulamalar).
pub fn window_icon_png(window: isize, size: i32) -> Option<Vec<u8>> {
    unsafe {
        let h = hwnd(window);
        let mut icon = 0usize;
        for which in [ICON_BIG, ICON_SMALL2] {
            let mut res = 0usize;
            let ok = SendMessageTimeoutW(
                h,
                WM_GETICON,
                WPARAM(which as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                150,
                Some(&mut res),
            );
            if ok.0 != 0 && res != 0 {
                icon = res;
                break;
            }
        }
        if icon == 0 {
            icon = GetClassLongPtrW(h, GCLP_HICON);
        }
        if icon == 0 {
            return None;
        }
        // Pencereye ait ikon: yok etmiyoruz.
        let px = render(HICON(icon as *mut c_void), size)?;
        encode_png(&px, size as u32, size as u32)
    }
}

unsafe fn bitmap_png(bmp: HBITMAP) -> Option<Vec<u8>> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(bmp.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut c_void),
    ) == 0
    {
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
    if w <= 0 || h <= 0 || w > 512 || h > 512 {
        return None;
    }
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let dc = CreateCompatibleDC(None);
    let lines = GetDIBits(
        dc,
        bmp,
        0,
        h as u32,
        Some(buf.as_mut_ptr() as *mut c_void),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(dc);
    if lines == 0 {
        return None;
    }
    encode_png(&buf, w as u32, h as u32)
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

pub(crate) fn encode_png(bgra: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut rgba = Vec::with_capacity(bgra.len());
    let no_alpha = bgra.chunks_exact(4).all(|p| p[3] == 0);
    // Ön-çarpılmış mı? (hiçbir renk kanalı alfadan büyük değilse öyledir)
    let premultiplied = bgra
        .chunks_exact(4)
        .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    for p in bgra.chunks_exact(4) {
        let (b, g, r, a) = (p[0], p[1], p[2], p[3]);
        if no_alpha {
            // Eski tip (alfa kanalsız) ikon: boş olmayan pikselleri opak say.
            let opaque = r | g | b != 0;
            rgba.extend_from_slice(&[r, g, b, if opaque { 255 } else { 0 }]);
        } else if a == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else if !premultiplied {
            rgba.extend_from_slice(&[r, g, b, a]);
        } else {
            // Ön-çarpılmış alfayı geri aç.
            let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            rgba.extend_from_slice(&[un(r), un(g), un(b), a]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&rgba).ok()?;
    }
    Some(out)
}
