//! Pano: değişiklikleri dinleme, metin/görsel okuma, geri kopyalama.

use crate::wide;
use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardOwner,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowThreadProcessId, RegisterClassW,
    TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
};

const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;
const WM_CLIPBOARDUPDATE: u32 = 0x031D;

static CALLBACK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        if let Some(cb) = CALLBACK.get() {
            cb();
        }
        return LRESULT(0);
    }
    DefWindowProcW(h, msg, w, l)
}

/// Pano her değiştiğinde `on_change` çağrılır (ayrı iş parçacığında). Bir kez çağır.
pub fn listen(on_change: impl Fn() + Send + Sync + 'static) {
    if CALLBACK.set(Box::new(on_change)).is_err() {
        return;
    }
    std::thread::spawn(|| unsafe {
        let class = wide("CamDockClipboardListener");
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: inst.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let Ok(h) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(inst.into()),
            None,
        ) else {
            return;
        };
        let _ = AddClipboardFormatListener(h);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

pub struct Content {
    pub text: Option<String>,
    /// Görsel: genişlik, yükseklik, BGRA (üstten alta)
    pub image: Option<(u32, u32, Vec<u8>)>,
    /// Geri kopyalamak için ham CF_DIB
    pub dib: Option<Vec<u8>>,
    /// Panoya yazan uygulamanın exe yolu
    pub owner: Option<String>,
    /// Şifre yöneticisi vb. "geçmişe ekleme" dedi
    pub private: bool,
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

unsafe fn open() -> Option<Guard> {
    for _ in 0..10 {
        if OpenClipboard(None).is_ok() {
            return Some(Guard);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    None
}

unsafe fn format(name: &str) -> u32 {
    let w = wide(name);
    RegisterClipboardFormatW(PCWSTR(w.as_ptr()))
}

unsafe fn bytes(fmt: u32) -> Option<Vec<u8>> {
    let h = GetClipboardData(fmt).ok()?;
    let g = HGLOBAL(h.0);
    let p = GlobalLock(g) as *const u8;
    if p.is_null() {
        return None;
    }
    let n = GlobalSize(g);
    let v = std::slice::from_raw_parts(p, n).to_vec();
    let _ = GlobalUnlock(g);
    Some(v)
}

/// CF_DIB → BGRA (üstten alta). 24 ve 32 bit sıkıştırmasız/BITFIELDS desteklenir.
fn dib_to_bgra(d: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let rd = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?)) };
    let hsize = rd(0)? as usize;
    let w = rd(4)? as i32;
    let h = rd(8)? as i32;
    let bpp = u16::from_le_bytes(d.get(14..16)?.try_into().ok()?);
    let comp = rd(16)?;
    let clr_used = rd(32)? as usize;
    if w <= 0 || h == 0 || !(bpp == 24 || bpp == 32) || !(comp == 0 || comp == 3) {
        return None;
    }
    let mut off = hsize;
    if comp == 3 && hsize == 40 {
        off += 12; // renk maskeleri
    }
    off += clr_used * 4;
    let (w, top_down, h) = (w as usize, h < 0, h.unsigned_abs() as usize);
    if w * h > 40_000_000 {
        return None;
    }
    let stride = (w * bpp as usize / 8 + 3) & !3;
    let px = d.get(off..off + stride * h)?;
    let mut out = vec![0u8; w * h * 4];
    let mut any_alpha = false;
    for y in 0..h {
        let src = if top_down { y } else { h - 1 - y };
        let row = &px[src * stride..];
        for x in 0..w {
            let o = (y * w + x) * 4;
            if bpp == 32 {
                out[o..o + 4].copy_from_slice(&row[x * 4..x * 4 + 4]);
                any_alpha |= row[x * 4 + 3] != 0;
            } else {
                out[o..o + 3].copy_from_slice(&row[x * 3..x * 3 + 3]);
                out[o + 3] = 255;
            }
        }
    }
    if bpp == 32 && !any_alpha {
        for a in out.iter_mut().skip(3).step_by(4) {
            *a = 255;
        }
    }
    Some((w as u32, h as u32, out))
}

/// Panodaki içerik. Panoyu açamazsa None.
pub fn read() -> Option<Content> {
    unsafe {
        let _g = open()?;
        let mut c = Content { text: None, image: None, dib: None, owner: None, private: false };
        let ex1 = format("ExcludeClipboardContentFromMonitorProcessing");
        let ex2 = format("Clipboard Viewer Ignore");
        let hist = format("CanIncludeInClipboardHistory");
        if IsClipboardFormatAvailable(ex1).is_ok() || IsClipboardFormatAvailable(ex2).is_ok() {
            c.private = true;
        }
        if IsClipboardFormatAvailable(hist).is_ok() {
            if let Some(b) = bytes(hist) {
                if b.get(..4).map(|x| u32::from_le_bytes(x.try_into().unwrap())) == Some(0) {
                    c.private = true;
                }
            }
        }
        if let Ok(owner) = GetClipboardOwner() {
            let mut pid = 0u32;
            GetWindowThreadProcessId(owner, Some(&mut pid));
            if pid != 0 {
                c.owner = crate::winlist::exe_of_pid(pid);
            }
        }
        if c.private {
            return Some(c);
        }
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() {
            if let Some(b) = bytes(CF_UNICODETEXT) {
                let u: Vec<u16> = b.chunks_exact(2).map(|x| u16::from_le_bytes([x[0], x[1]])).take_while(|&x| x != 0).collect();
                let s = String::from_utf16_lossy(&u);
                if !s.trim().is_empty() {
                    c.text = Some(s);
                }
            }
        }
        if IsClipboardFormatAvailable(CF_DIB).is_ok() {
            if let Some(b) = bytes(CF_DIB) {
                c.image = dib_to_bgra(&b);
                if c.image.is_some() {
                    c.dib = Some(b);
                }
            }
        }
        Some(c)
    }
}

/// BGRA'yı en çok `max` piksel genişliğe küçültüp PNG yapar (önizleme için).
pub fn thumbnail_png(img: &(u32, u32, Vec<u8>), max: u32) -> Option<Vec<u8>> {
    let (w, h, px) = (img.0, img.1, &img.2);
    let scale = (max as f32 / w.max(h) as f32).min(1.0);
    let (tw, th) = (((w as f32 * scale) as u32).max(1), ((h as f32 * scale) as u32).max(1));
    let mut out = vec![0u8; (tw * th * 4) as usize];
    // Kutu filtresi: her hedef pikselde kaynak bölgenin ortalaması
    for ty in 0..th {
        let y0 = ty * h / th;
        let y1 = ((ty + 1) * h / th).max(y0 + 1);
        for tx in 0..tw {
            let x0 = tx * w / tw;
            let x1 = ((tx + 1) * w / tw).max(x0 + 1);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            let step = ((x1 - x0) * (y1 - y0) / 64).max(1);
            let mut k = 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    k += 1;
                    if k % step != 0 {
                        continue;
                    }
                    let o = ((y * w + x) * 4) as usize;
                    for i in 0..4 {
                        acc[i] += px[o + i] as u32;
                    }
                    n += 1;
                }
            }
            let o = ((ty * tw + tx) * 4) as usize;
            for i in 0..4 {
                out[o + i] = (acc[i] / n.max(1)) as u8;
            }
        }
    }
    crate::icons::encode_png(&out, tw, th)
}

unsafe fn put(fmt: u32, data: &[u8]) -> bool {
    let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, data.len()) else { return false };
    let p = GlobalLock(g) as *mut u8;
    if p.is_null() {
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
    let _ = GlobalUnlock(g);
    SetClipboardData(fmt, Some(HANDLE(g.0))).is_ok()
}

pub fn set_text(s: &str) -> bool {
    unsafe {
        let Some(_g) = open() else { return false };
        let _ = EmptyClipboard();
        let w: Vec<u8> = s.encode_utf16().chain(std::iter::once(0)).flat_map(|x| x.to_le_bytes()).collect();
        put(CF_UNICODETEXT, &w)
    }
}

pub fn set_dib(dib: &[u8]) -> bool {
    unsafe {
        let Some(_g) = open() else { return false };
        let _ = EmptyClipboard();
        put(CF_DIB, dib)
    }
}
