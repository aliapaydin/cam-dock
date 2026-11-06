//! Uygulamaların atlama listeleri (taskbar'da sağ tıkla açılan kendi menüleri):
//! görevler ("Yeni pencere", "Yeni InPrivate pencere"…), uygulamanın kendi
//! kategorileri ("Sık ziyaret edilenler"…) ve son kullanılan öğeler.
//!
//! Windows bu listeleri okumak için belgelenmiş bir API sunmuyor; uygulamaların
//! yazdığı `CustomDestinations\<AppID karması>.customDestinations-ms`
//! dosyasını çözüyoruz. Dosyadaki her öğe kayıtlı bir COM nesnesi (çoğunlukla
//! ShellLink); IPersistStream ile yükleyip okuyoruz. Son kullanılanlar için
//! belgelenmiş IApplicationDocumentLists kullanılıyor.

use crate::wide;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{Interface, IUnknown, GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Storage::EnhancedStorage::{
    PKEY_AppUserModel_ID, PKEY_AppUserModel_IsDestListSeparator, PKEY_Title,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantToBoolean, PropVariantToStringAlloc};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, IPersistStream, IStream,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::Common::IObjectArray;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    ApplicationDocumentLists, IApplicationDocumentLists, IShellItem, IShellLinkW,
    SHCreateMemStream, SHGetKnownFolderPath, SHGetNameFromIDList, SHLoadIndirectString, ShellLink,
    ADLT_FREQUENT, ADLT_RECENT, KF_FLAG_DEFAULT, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH,
    SIGDN_NORMALDISPLAY,
};

#[derive(Debug, Clone, Serialize, Default)]
pub struct Entry {
    pub title: String,
    /// Çalıştırılacak dosya / kabuk yolu
    pub path: String,
    pub args: String,
    /// Görevler arasındaki ayraç
    pub separator: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct JumpList {
    /// (kategori adı, öğeler); görevler "" adıyla en sonda
    pub categories: Vec<(String, Vec<Entry>)>,
    pub tasks: Vec<Entry>,
}

impl JumpList {
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.categories.iter().all(|c| c.1.is_empty())
    }
}

/// Windows'un AppID karması: büyük harfe çevrilmiş kimliğin UTF-16LE baytları
/// üzerinde yansıtılmış CRC-64 (polinom 0x92C64265D32139A4, başlangıç ~0).
pub fn appid_hash(appid: &str) -> String {
    const POLY: u64 = 0x92C6_4265_D321_39A4;
    let mut crc: u64 = !0;
    for u in appid.to_uppercase().encode_utf16() {
        for b in u.to_le_bytes() {
            crc ^= b as u64;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ POLY } else { crc >> 1 };
            }
        }
    }
    format!("{crc:016x}")
}

unsafe fn take(p: PWSTR) -> String {
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
    s
}

fn known_folder(id: &GUID) -> Option<String> {
    unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok().map(|p| take(p)) }
}

/// Açık kimliği olmayan uygulamalar için Windows'un varsayılan AppID'si:
/// exe yolu, bilinen klasör öneki GUID ile değiştirilmiş hali.
pub fn default_appid(exe: &str) -> String {
    use windows::Win32::UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86, FOLDERID_RoamingAppData,
        FOLDERID_System, FOLDERID_SystemX86, FOLDERID_UserProgramFiles, FOLDERID_Windows,
    };
    let folders = [
        FOLDERID_UserProgramFiles,
        FOLDERID_ProgramFilesX64,
        FOLDERID_ProgramFilesX86,
        FOLDERID_System,
        FOLDERID_SystemX86,
        FOLDERID_Windows,
        FOLDERID_LocalAppData,
        FOLDERID_RoamingAppData,
    ];
    let lower = exe.to_lowercase();
    let mut best: Option<(usize, String)> = None;
    for id in folders {
        let Some(base) = known_folder(&id) else { continue };
        let b = base.trim_end_matches('\\').to_lowercase();
        if b.is_empty() || !lower.starts_with(&b) || lower.as_bytes().get(b.len()) != Some(&b'\\') {
            continue;
        }
        if best.as_ref().map_or(true, |(n, _)| b.len() > *n) {
            best = Some((b.len(), format!("{{{:?}}}{}", id, &exe[b.len()..])));
        }
    }
    best.map(|(_, s)| s.to_uppercase()).unwrap_or_else(|| exe.to_string())
}

fn custom_file(appid: &str) -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let p = PathBuf::from(appdata)
        .join("Microsoft\\Windows\\Recent\\CustomDestinations")
        .join(format!("{}.customDestinations-ms", appid_hash(appid)));
    p.is_file().then_some(p)
}

fn automatic_file(appid: &str) -> bool {
    std::env::var_os("APPDATA")
        .map(|a| {
            PathBuf::from(a)
                .join("Microsoft\\Windows\\Recent\\AutomaticDestinations")
                .join(format!("{}.automaticDestinations-ms", appid_hash(appid)))
                .is_file()
        })
        .unwrap_or(false)
}

/// Kısayolun içindeki AppUserModelID (varsa).
fn lnk_appid(lnk: &str) -> Option<String> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let w = wide(lnk);
        link.cast::<IPersistFile>().ok()?.Load(PCWSTR(w.as_ptr()), STGM_READ).ok()?;
        let id = prop_string(&link.cast::<IPropertyStore>().ok()?, &PKEY_AppUserModel_ID);
        (!id.is_empty()).then_some(id)
    }
}

/// Başlat menüsü kataloğunda bu exe'ye ait kimlik (kısayolundaki AppID).
/// Katalog taraması pahalı: 10 dakikada bir yenilenir.
static CATALOG: Mutex<Option<(Instant, HashMap<String, String>)>> = Mutex::new(None);

fn catalog_appid(exe: &str) -> Option<String> {
    let mut g = CATALOG.lock().unwrap();
    if g.as_ref().map_or(true, |(t, _)| t.elapsed() > Duration::from_secs(600)) {
        let map = crate::catalog::apps()
            .into_iter()
            .filter(|a| !a.id.is_empty() && a.path.to_lowercase().ends_with(".exe"))
            .map(|a| (a.path.to_lowercase(), a.id))
            .collect();
        *g = Some((Instant::now(), map));
    }
    g.as_ref().and_then(|(_, m)| m.get(&exe.to_lowercase()).cloned())
}

/// Bir dock öğesi için denenecek AppID'ler (öncelik sırasıyla).
pub fn candidates(path: &str, aumid: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    };
    push(aumid.to_string());
    let lower = path.to_lowercase();
    if let Some(id) = lower.strip_prefix("shell:appsfolder\\").map(|_| &path[17..]) {
        push(id.to_string());
    } else if lower.ends_with(".lnk") {
        if let Some(id) = lnk_appid(path) {
            push(id);
        }
    } else if lower.ends_with(".exe") {
        if let Some(id) = catalog_appid(path) {
            push(id);
        }
        push(default_appid(path));
    }
    out
}

unsafe fn prop_string(store: &IPropertyStore, key: &PROPERTYKEY) -> String {
    let Ok(pv) = store.GetValue(key) else { return String::new() };
    PropVariantToStringAlloc(&pv).map(|p| take(p)).unwrap_or_default()
}

fn indirect(s: String) -> String {
    if !s.starts_with('@') {
        return s;
    }
    let src = wide(&s);
    let mut buf = vec![0u16; 512];
    unsafe {
        if SHLoadIndirectString(PCWSTR(src.as_ptr()), &mut buf, None).is_ok() {
            let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
            return String::from_utf16_lossy(&buf[..n]);
        }
    }
    String::new()
}

fn buf_str(b: &[u16]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..n])
}

unsafe fn from_link(link: &IShellLinkW) -> Entry {
    let mut e = Entry::default();
    if let Ok(store) = link.cast::<IPropertyStore>() {
        if let Ok(pv) = store.GetValue(&PKEY_AppUserModel_IsDestListSeparator) {
            if PropVariantToBoolean(&pv).map(|b| b.as_bool()).unwrap_or(false) {
                e.separator = true;
                return e;
            }
        }
        e.title = indirect(prop_string(&store, &PKEY_Title));
    }
    let mut buf = vec![0u16; 1024];
    if link.GetPath(&mut buf, std::ptr::null_mut(), 0).is_ok() {
        e.path = buf_str(&buf);
    }
    if e.path.is_empty() {
        if let Ok(pidl) = link.GetIDList() {
            if !pidl.is_null() {
                if let Ok(p) = SHGetNameFromIDList(pidl, SIGDN_DESKTOPABSOLUTEPARSING) {
                    e.path = take(p);
                }
                CoTaskMemFree(Some(pidl as *const core::ffi::c_void));
            }
        }
    }
    let mut args = vec![0u16; 2048];
    if link.GetArguments(&mut args).is_ok() {
        e.args = buf_str(&args);
    }
    if e.title.is_empty() {
        let mut d = vec![0u16; 512];
        if link.GetDescription(&mut d).is_ok() {
            e.title = indirect(buf_str(&d));
        }
    }
    if e.title.is_empty() {
        e.title = e.path.rsplit('\\').next().unwrap_or("").to_string();
    }
    e
}

unsafe fn from_item(item: &IShellItem) -> Entry {
    let title = item.GetDisplayName(SIGDN_NORMALDISPLAY).map(|p| take(p)).unwrap_or_default();
    let path = item
        .GetDisplayName(SIGDN_FILESYSPATH)
        .or_else(|_| item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING))
        .map(|p| take(p))
        .unwrap_or_default();
    Entry { title, path, args: String::new(), separator: false }
}

unsafe fn from_unknown(obj: &IUnknown) -> Option<Entry> {
    if let Ok(link) = obj.cast::<IShellLinkW>() {
        return Some(from_link(&link));
    }
    if let Ok(item) = obj.cast::<IShellItem>() {
        return Some(from_item(&item));
    }
    None
}

/// Dosyadaki bir nesneyi (CLSID + IPersistStream verisi) okur; akış nesnenin
/// sonuna ilerler.
unsafe fn read_object(stream: &IStream) -> Option<Entry> {
    let mut clsid = [0u8; 16];
    let mut got = 0u32;
    if stream.Read(clsid.as_mut_ptr() as *mut _, 16, Some(&mut got)).is_err() || got != 16 {
        return None;
    }
    let clsid = GUID::from_values(
        u32::from_le_bytes([clsid[0], clsid[1], clsid[2], clsid[3]]),
        u16::from_le_bytes([clsid[4], clsid[5]]),
        u16::from_le_bytes([clsid[6], clsid[7]]),
        [clsid[8], clsid[9], clsid[10], clsid[11], clsid[12], clsid[13], clsid[14], clsid[15]],
    );
    let obj: IUnknown = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER).ok()?;
    obj.cast::<IPersistStream>().ok()?.Load(stream).ok()?;
    from_unknown(&obj)
}

unsafe fn read_u32(stream: &IStream) -> Option<u32> {
    let mut b = [0u8; 4];
    let mut got = 0u32;
    stream.Read(b.as_mut_ptr() as *mut _, 4, Some(&mut got)).ok().ok()?;
    (got == 4).then(|| u32::from_le_bytes(b))
}

unsafe fn read_u16(stream: &IStream) -> Option<u16> {
    let mut b = [0u8; 2];
    let mut got = 0u32;
    stream.Read(b.as_mut_ptr() as *mut _, 2, Some(&mut got)).ok().ok()?;
    (got == 2).then(|| u16::from_le_bytes(b))
}

const FOOTER: u32 = 0xBABF_FBAB;

/// Bilinen kategori: -1 sık kullanılanlar, -2 son kullanılanlar.
unsafe fn known_list(appid: &str, which: i32) -> Vec<Entry> {
    let mut out = Vec::new();
    let Ok(lists) = CoCreateInstance::<_, IApplicationDocumentLists>(&ApplicationDocumentLists, None, CLSCTX_INPROC_SERVER) else {
        return out;
    };
    let id = wide(appid);
    if lists.SetAppID(PCWSTR(id.as_ptr())).is_err() {
        return out;
    }
    let kind = if which == -1 { ADLT_FREQUENT } else { ADLT_RECENT };
    let Ok(arr) = lists.GetList::<IObjectArray>(kind, 10) else { return out };
    let n = arr.GetCount().unwrap_or(0);
    for i in 0..n {
        if let Ok(obj) = arr.GetAt::<IUnknown>(i) {
            if let Some(e) = from_unknown(&obj) {
                if !e.title.is_empty() {
                    out.push(e);
                }
            }
        }
    }
    out
}

fn parse_custom(file: &PathBuf, appid: &str) -> Option<JumpList> {
    let data = std::fs::read(file).ok()?;
    unsafe {
        let stream = SHCreateMemStream(Some(&data))?;
        let _version = read_u32(&stream)?;
        let count = read_u32(&stream)?;
        let _reserved = read_u32(&stream)?;
        let mut jl = JumpList::default();
        for _ in 0..count.min(32) {
            let kind = read_u32(&stream)?;
            match kind {
                // Uygulamanın kendi kategorisi
                0 => {
                    let len = read_u16(&stream)? as usize;
                    let mut name = vec![0u16; len];
                    let mut got = 0u32;
                    stream.Read(name.as_mut_ptr() as *mut _, (len * 2) as u32, Some(&mut got)).ok().ok()?;
                    let name = indirect(String::from_utf16_lossy(&name));
                    let n = read_u32(&stream)?;
                    let mut items = Vec::new();
                    for _ in 0..n.min(64) {
                        if let Some(e) = read_object(&stream) {
                            items.push(e);
                        } else {
                            return Some(jl);
                        }
                    }
                    jl.categories.push((name, items));
                }
                // Bilinen kategori (sık / son kullanılanlar)
                1 => {
                    let which = read_u32(&stream)? as i32;
                    let name = if which == -1 { "Sık kullanılanlar" } else { "Son kullanılanlar" };
                    jl.categories.push((name.into(), known_list(appid, which)));
                }
                // Görevler
                2 => {
                    let n = read_u32(&stream)?;
                    for _ in 0..n.min(64) {
                        if let Some(e) = read_object(&stream) {
                            jl.tasks.push(e);
                        } else {
                            return Some(jl);
                        }
                    }
                }
                _ => return Some(jl),
            }
            if read_u32(&stream)? != FOOTER {
                return Some(jl);
            }
        }
        Some(jl)
    }
}

/// Uygulamanın atlama listesi. `path`: dock öğesinin yolu, `aumid`: açık
/// penceresinden okunan kimlik (yoksa boş).
pub fn for_app(path: &str, aumid: &str) -> JumpList {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    for id in candidates(path, aumid) {
        if let Some(f) = custom_file(&id) {
            if let Some(mut jl) = parse_custom(&f, &id) {
                jl.categories.retain(|c| !c.1.is_empty());
                // Ayraç başta/sonda ya da üst üste olmasın
                jl.tasks.dedup_by(|a, b| a.separator && b.separator);
                while jl.tasks.first().is_some_and(|e| e.separator) {
                    jl.tasks.remove(0);
                }
                while jl.tasks.last().is_some_and(|e| e.separator) {
                    jl.tasks.pop();
                }
                if !jl.is_empty() {
                    return jl;
                }
            }
        }
        // Özel liste yok: Windows yalnızca son kullanılanları gösterir
        if automatic_file(&id) {
            let recent = unsafe { known_list(&id, -2) };
            if !recent.is_empty() {
                return JumpList { categories: vec![("Son kullanılanlar".into(), recent)], tasks: Vec::new() };
            }
        }
    }
    JumpList::default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn hash() {
        assert_eq!(super::appid_hash("Chrome"), "5d696d521de238c3");
        assert_eq!(super::appid_hash("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\notepad.exe"), "9b9cdc69c1c24e2b");
    }
}
