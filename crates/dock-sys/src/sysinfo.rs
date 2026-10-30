//! Sistem kaynakları: CPU, RAM, GPU, Disk kullanımı (yüzde).
//! Görev Yöneticisi'nin kullandığı kaynaklar: GetSystemTimes, GlobalMemoryStatusEx
//! ve performans sayaçları (PDH).

use crate::wide;
use serde::Serialize;
use windows::core::PCWSTR;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct Stats {
    pub cpu: f32,
    pub ram: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    /// GPU 3D motor kullanımı; sayaç yoksa -1
    pub gpu: f32,
    /// Disk etkinliği (% meşgul); sayaç yoksa -1
    pub disk: f32,
}

pub struct Sampler {
    query: Option<PDH_HQUERY>,
    disk: Option<PDH_HCOUNTER>,
    gpu: Option<PDH_HCOUNTER>,
    last_cpu: Option<(u64, u64)>,
}

// PDH tanıtıcıları yalnızca örnekleyici iş parçacığında kullanılır.
unsafe impl Send for Sampler {}

fn ft(f: FILETIME) -> u64 {
    (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64
}

impl Sampler {
    pub fn new() -> Self {
        let mut s = Sampler { query: None, disk: None, gpu: None, last_cpu: None };
        unsafe {
            let mut q = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) == 0 {
                let add = |path: &str| {
                    let w = wide(path);
                    let mut c = PDH_HCOUNTER::default();
                    (PdhAddEnglishCounterW(q, PCWSTR(w.as_ptr()), 0, &mut c) == 0).then_some(c)
                };
                s.disk = add("\\PhysicalDisk(_Total)\\% Idle Time");
                s.gpu = add("\\GPU Engine(*engtype_3D)\\Utilization Percentage");
                let _ = PdhCollectQueryData(q);
                s.query = Some(q);
            }
        }
        s
    }

    fn cpu(&mut self) -> f32 {
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe {
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_err() {
                return 0.0;
            }
        }
        // Çekirdek süresi boşta süreyi de içerir.
        let total = ft(kernel) + ft(user);
        let idle = ft(idle);
        let out = match self.last_cpu {
            Some((t0, i0)) if total > t0 => {
                let dt = (total - t0) as f64;
                let di = idle.saturating_sub(i0) as f64;
                ((1.0 - di / dt) * 100.0).clamp(0.0, 100.0) as f32
            }
            _ => 0.0,
        };
        self.last_cpu = Some((total, idle));
        out
    }

    /// Bir önceki çağrıdan bu yana geçen sürenin ortalaması (1 sn aralıkla çağır).
    pub fn sample(&mut self) -> Stats {
        let mut st = Stats { cpu: self.cpu(), gpu: -1.0, disk: -1.0, ..Default::default() };
        unsafe {
            let mut ms = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            if GlobalMemoryStatusEx(&mut ms).is_ok() {
                let gb = 1024.0 * 1024.0 * 1024.0;
                st.ram = ms.dwMemoryLoad as f32;
                st.ram_total_gb = (ms.ullTotalPhys as f64 / gb) as f32;
                st.ram_used_gb = ((ms.ullTotalPhys - ms.ullAvailPhys) as f64 / gb) as f32;
            }
            let Some(q) = self.query else { return st };
            if PdhCollectQueryData(q) != 0 {
                return st;
            }
            if let Some(c) = self.disk {
                let mut v = PDH_FMT_COUNTERVALUE::default();
                if PdhGetFormattedCounterValue(c, PDH_FMT_DOUBLE, None, &mut v) == 0 {
                    st.disk = (100.0 - v.Anonymous.doubleValue).clamp(0.0, 100.0) as f32;
                }
            }
            if let Some(c) = self.gpu {
                let (mut size, mut count) = (0u32, 0u32);
                let _ = PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, None);
                if size > 0 {
                    let mut buf = vec![0u8; size as usize];
                    let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                    if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) == 0 {
                        let slice = std::slice::from_raw_parts(items, count as usize);
                        let sum: f64 = slice.iter().map(|i| i.FmtValue.Anonymous.doubleValue.max(0.0)).sum();
                        st.gpu = sum.clamp(0.0, 100.0) as f32;
                    }
                } else {
                    st.gpu = 0.0;
                }
            }
        }
        st
    }
}

/// Kullanım panelinde gösterilen, yavaş değişen sistem bilgileri.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Info {
    pub uptime_sec: u64,
    pub disk_free_gb: f32,
    pub disk_total_gb: f32,
    /// Pil yüzdesi; pil yoksa -1
    pub battery: i32,
    pub charging: bool,
    pub procs: u32,
    pub cores: u32,
    pub cpu_name: String,
    pub os: String,
}

fn reg_string(root: windows::Win32::System::Registry::HKEY, key: &str, name: &str) -> Option<String> {
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};
    let k = wide(key);
    let n = wide(name);
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            root,
            PCWSTR(k.as_ptr()),
            PCWSTR(n.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]).trim().to_string())
}

pub fn info() -> Info {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
    use windows::Win32::System::SystemInformation::{GetSystemInfo, GetTickCount64, SYSTEM_INFO};
    use windows::Win32::Foundation::CloseHandle;

    let mut out = Info { battery: -1, ..Default::default() };
    unsafe {
        out.uptime_sec = GetTickCount64() / 1000;

        let drive = wide("C:\\");
        let (mut free, mut total) = (0u64, 0u64);
        if GetDiskFreeSpaceExW(PCWSTR(drive.as_ptr()), Some(&mut free), Some(&mut total), None).is_ok() {
            let gb = 1024.0 * 1024.0 * 1024.0;
            out.disk_free_gb = (free as f64 / gb) as f32;
            out.disk_total_gb = (total as f64 / gb) as f32;
        }

        let mut ps = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut ps).is_ok() && ps.BatteryFlag != 128 && ps.BatteryLifePercent <= 100 {
            out.battery = ps.BatteryLifePercent as i32;
            out.charging = ps.ACLineStatus == 1;
        }

        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            if Process32FirstW(snap, &mut e).is_ok() {
                out.procs = 1;
                while Process32NextW(snap, &mut e).is_ok() {
                    out.procs += 1;
                }
            }
            let _ = CloseHandle(snap);
        }

        let mut si = SYSTEM_INFO::default();
        GetSystemInfo(&mut si);
        out.cores = si.dwNumberOfProcessors;
    }
    out.cpu_name = reg_string(
        HKEY_LOCAL_MACHINE,
        "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
        "ProcessorNameString",
    )
    .unwrap_or_default();
    let nt = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
    let build: u32 = reg_string(HKEY_LOCAL_MACHINE, nt, "CurrentBuild")
        .and_then(|b| b.parse().ok())
        .unwrap_or(0);
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    let ver = reg_string(HKEY_LOCAL_MACHINE, nt, "DisplayVersion").unwrap_or_default();
    out.os = format!("{name} {ver} · derleme {build}").replace("  ", " ");
    out
}
