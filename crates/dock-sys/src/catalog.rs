//! Windows'un uygulama kataloğu (Başlat menüsü → Tüm uygulamalar):
//! klasik masaüstü uygulamaları ve Microsoft Store uygulamaları birlikte.
//! Ayrıca dosya seçme penceresi (Gözat).

use crate::wide;
use serde::Serialize;
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::EnhancedStorage::PKEY_Link_TargetParsingPath;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FileOpenDialog, IEnumShellItems, IFileOpenDialog, IShellItem, IShellItem2,
    SHGetKnownFolderItem, FOLDERID_AppsFolder, FOS_FILEMUSTEXIST, FOS_PICKFOLDERS, FOS_FORCEFILESYSTEM,
    FOS_NODEREFERENCELINKS, KF_FLAG_DEFAULT, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING,
};

#[derive(Debug, Clone, Serialize)]
pub struct CatalogApp {
    pub name: String,
    /// Başlatma yolu: exe yolu ya da `shell:AppsFolder\<kimlik>`
    pub path: String,
    /// Store uygulamasıysa AppUserModelID
    pub aumid: String,
    /// Uygulamalar klasöründeki kimlik (masaüstü uygulamalarında AppID)
    #[serde(skip)]
    pub id: String,
}

unsafe fn take(p: PWSTR) -> String {
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
    s
}

/// Katalogdaki tüm uygulamalar (ada göre sıralı).
pub fn apps() -> Vec<CatalogApp> {
    let mut out = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(folder) = SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) else {
            return out;
        };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
            return out;
        };
        loop {
            let mut batch = [None];
            let mut fetched = 0u32;
            if items.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = batch[0].take() else { break };
            let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).map(|p| take(p)).unwrap_or_default();
            let id = item
                .GetDisplayName(SIGDN_PARENTRELATIVEPARSING)
                .map(|p| take(p))
                .unwrap_or_default();
            if name.is_empty() || id.is_empty() {
                continue;
            }
            let lower = name.to_lowercase();
            if ["uninstall", "kaldır", "readme", "release notes"].iter().any(|s| lower.contains(s)) {
                continue;
            }
            let target = item
                .cast::<IShellItem2>()
                .ok()
                .and_then(|i2| i2.GetString(&PKEY_Link_TargetParsingPath).ok())
                .map(|p| take(p))
                .unwrap_or_default();
            let (path, aumid) = if target.to_lowercase().ends_with(".exe") {
                (target, String::new())
            } else if id.contains('!') {
                (format!("shell:AppsFolder\\{id}"), id.clone())
            } else {
                (format!("shell:AppsFolder\\{id}"), String::new())
            };
            out.push(CatalogApp { name, path, aumid, id: id.clone() });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out.dedup_by(|a, b| a.path.eq_ignore_ascii_case(&b.path));
    out
}

/// Windows dosya seçme penceresi: exe ya da kısayol seç. Kendi iş parçacığında
/// (STA) çalışır; `owner` pencere ise kapanana kadar devre dışı kalır.
pub fn pick_app(owner: isize) -> Option<String> {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let n1 = wide("Uygulamalar ve kısayollar");
        let s1 = wide("*.exe;*.lnk");
        let n2 = wide("Tüm dosyalar");
        let s2 = wide("*.*");
        let filters = [
            COMDLG_FILTERSPEC { pszName: PCWSTR(n1.as_ptr()), pszSpec: PCWSTR(s1.as_ptr()) },
            COMDLG_FILTERSPEC { pszName: PCWSTR(n2.as_ptr()), pszSpec: PCWSTR(s2.as_ptr()) },
        ];
        let _ = dlg.SetFileTypes(&filters);
        let title = wide("Dock'a eklenecek uygulamayı seç");
        let _ = dlg.SetTitle(PCWSTR(title.as_ptr()));
        let _ = dlg.SetOptions(FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM | FOS_NODEREFERENCELINKS);
        let owner = (owner != 0).then(|| HWND(owner as *mut core::ffi::c_void));
        dlg.Show(owner).ok()?;
        let item = dlg.GetResult().ok()?;
        item.GetDisplayName(SIGDN_FILESYSPATH).ok().map(|p| take(p))
    })
    .join()
    .ok()
    .flatten()
}

/// Klasör seçme penceresi (klasör yığını eklemek için).
pub fn pick_folder(owner: isize) -> Option<String> {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let title = wide("Dock'a yığın olarak eklenecek klasörü seç");
        let _ = dlg.SetTitle(PCWSTR(title.as_ptr()));
        let _ = dlg.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
        let owner = (owner != 0).then(|| HWND(owner as *mut core::ffi::c_void));
        dlg.Show(owner).ok()?;
        let item = dlg.GetResult().ok()?;
        item.GetDisplayName(SIGDN_FILESYSPATH).ok().map(|p| take(p))
    })
    .join()
    .ok()
    .flatten()
}
