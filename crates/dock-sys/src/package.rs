//! Microsoft Store (MSIX) sürümü: paket içinde mi çalışıyoruz, Windows ile
//! başlatma (paketin StartupTask'i).

use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows_core::HSTRING;

/// AppxManifest.xml'deki görev kimliği
const TASK_ID: &str = "CamDockStartup";

/// MSIX paketi içinden mi çalışıyoruz (Store sürümü)?
pub fn is_packaged() -> bool {
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
    let mut len = 0u32;
    // Paketsiz süreçte APPMODEL_ERROR_NO_PACKAGE (15700) döner
    let rc = unsafe { GetCurrentPackageFullName(&mut len, None) };
    rc.0 != 15700
}

fn task() -> Option<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from(TASK_ID)).ok()?.join().ok()
}

/// Windows ile başlat açık mı (paket sürümü)
pub fn startup_enabled() -> bool {
    task()
        .and_then(|t| t.State().ok())
        .is_some_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
}

/// Aç/kapat. Kullanıcı Görev Yöneticisi'nden kapattıysa yeniden açılamaz
/// (Windows izin vermez); o durumda false döner.
pub fn startup_set(enabled: bool) -> bool {
    let Some(t) = task() else { return false };
    if enabled {
        t.RequestEnableAsync()
            .ok()
            .and_then(|op| op.join().ok())
            .is_some_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
    } else {
        t.Disable().is_ok()
    }
}
