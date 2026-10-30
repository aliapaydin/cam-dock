//! Canlı pencere küçük resimleri (DWM thumbnail). Taskbar önizlemeleriyle aynı
//! mekanizma: kaynak pencerenin canlı görüntüsünü bizim panel penceremizin
//! içinde verilen dikdörtgene çizer.

use crate::hwnd;
use windows::Win32::Foundation::{RECT, SIZE};
use windows::Win32::Graphics::Dwm::{
    DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
    DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};

/// `dest` penceresinde `src` için küçük resim oluşturur ve kutuya (fiziksel
/// piksel, dest'in sol üstüne göre) oranını koruyarak ortalar.
/// Dönüş: küçük resim tanıtıcısı (kaldırmak için).
pub fn show(dest: isize, src: isize, x: i32, y: i32, w: i32, h: i32) -> Option<isize> {
    unsafe {
        let thumb = DwmRegisterThumbnail(hwnd(dest), hwnd(src)).ok()?;
        let size: SIZE = DwmQueryThumbnailSourceSize(thumb).unwrap_or(SIZE { cx: w, cy: h });
        let (sw, sh) = (size.cx.max(1) as f64, size.cy.max(1) as f64);
        let k = (w as f64 / sw).min(h as f64 / sh);
        let (tw, th) = ((sw * k).round() as i32, (sh * k).round() as i32);
        let (tx, ty) = (x + (w - tw) / 2, y + (h - th) / 2);
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION
                | DWM_TNP_VISIBLE
                | DWM_TNP_OPACITY
                | DWM_TNP_SOURCECLIENTAREAONLY,
            rcDestination: RECT {
                left: tx,
                top: ty,
                right: tx + tw,
                bottom: ty + th,
            },
            opacity: 255,
            fVisible: true.into(),
            fSourceClientAreaOnly: false.into(),
            ..Default::default()
        };
        if DwmUpdateThumbnailProperties(thumb, &props).is_err() {
            let _ = DwmUnregisterThumbnail(thumb);
            return None;
        }
        Some(thumb)
    }
}

pub fn remove(thumb: isize) {
    unsafe {
        let _ = DwmUnregisterThumbnail(thumb);
    }
}
