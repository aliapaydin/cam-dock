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
