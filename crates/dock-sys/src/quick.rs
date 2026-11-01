//! Hızlı ayarlar: ses ve çıkış aygıtı, parlaklık (DDC/CI), Wi-Fi ve
//! Bluetooth radyoları, Wi-Fi ağları.
#![allow(non_snake_case)]

use crate::monitor::Rect;
use crate::wide;
use serde::Serialize;
use windows::core::{interface, IUnknown, IUnknown_Vtbl, GUID, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{HANDLE, POINT};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eConsole, eMultimedia, eRender, ERole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::NetworkManagement::WiFi::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;

fn com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

// ---------------------------------------------------------------------------
// Ses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AudioState {
    /// 0–100
    pub volume: f32,
    pub muted: bool,
    pub devices: Vec<AudioDevice>,
}

unsafe fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const _));
    s
}

unsafe fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    com();
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
}

unsafe fn endpoint_volume() -> windows::core::Result<IAudioEndpointVolume> {
    let dev = enumerator()?.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
    dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
}

unsafe fn device_name(d: &IMMDevice) -> String {
    let Ok(store) = d.OpenPropertyStore(STGM_READ) else { return String::new() };
    let Ok(v) = store.GetValue(&PKEY_Device_FriendlyName) else { return String::new() };
    PropVariantToStringAlloc(&v).map(|p| take(p)).unwrap_or_default()
}

pub fn audio_state() -> AudioState {
    unsafe {
        let mut st = AudioState::default();
        if let Ok(v) = endpoint_volume() {
            st.volume = v.GetMasterVolumeLevelScalar().unwrap_or(0.0) * 100.0;
            st.muted = v.GetMute().map(|b| b.as_bool()).unwrap_or(false);
        }
        if let Ok(e) = enumerator() {
            let def = e
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .and_then(|d| d.GetId())
                .map(|p| take(p))
                .unwrap_or_default();
            if let Ok(col) = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
                for i in 0..col.GetCount().unwrap_or(0) {
                    let Ok(d) = col.Item(i) else { continue };
                    let id = d.GetId().map(|p| take(p)).unwrap_or_default();
                    st.devices.push(AudioDevice { name: device_name(&d), default: id == def, id });
                }
            }
        }
        st
    }
}

pub fn set_volume(v: f32) -> bool {
    unsafe {
        endpoint_volume()
            .and_then(|e| e.SetMasterVolumeLevelScalar((v / 100.0).clamp(0.0, 1.0), std::ptr::null()))
            .is_ok()
    }
}

pub fn set_mute(m: bool) -> bool {
    unsafe { endpoint_volume().and_then(|e| e.SetMute(m, std::ptr::null())).is_ok() }
}

// Varsayılan çıkış aygıtını değiştirmek için Windows'un belgelenmemiş ama
// Ses ayarlarının da kullandığı arayüzü (Win7'den beri sabit).
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, id: PCWSTR, f: *mut *mut core::ffi::c_void) -> HRESULT;
    fn GetDeviceFormat(&self, id: PCWSTR, def: i32, f: *mut *mut core::ffi::c_void) -> HRESULT;
    fn ResetDeviceFormat(&self, id: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, id: PCWSTR, a: *mut core::ffi::c_void, b: *mut core::ffi::c_void) -> HRESULT;
    fn GetProcessingPeriod(&self, id: PCWSTR, def: i32, a: *mut i64, b: *mut i64) -> HRESULT;
    fn SetProcessingPeriod(&self, id: PCWSTR, a: *mut i64) -> HRESULT;
    fn GetShareMode(&self, id: PCWSTR, m: *mut core::ffi::c_void) -> HRESULT;
    fn SetShareMode(&self, id: PCWSTR, m: *mut core::ffi::c_void) -> HRESULT;
    fn GetPropertyValue(&self, id: PCWSTR, key: *const core::ffi::c_void, v: *mut core::ffi::c_void) -> HRESULT;
    fn SetPropertyValue(&self, id: PCWSTR, key: *const core::ffi::c_void, v: *mut core::ffi::c_void) -> HRESULT;
    fn SetDefaultEndpoint(&self, id: PCWSTR, role: ERole) -> HRESULT;
    fn SetEndpointVisibility(&self, id: PCWSTR, visible: i32) -> HRESULT;
}
const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

pub fn set_default_device(id: &str) -> bool {
    unsafe {
        com();
        let Ok(pc) = CoCreateInstance::<_, IPolicyConfig>(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL) else {
            return false;
        };
        let w = wide(id);
        let mut ok = true;
        for role in [eConsole, eMultimedia, windows::Win32::Media::Audio::eCommunications] {
            ok &= pc.SetDefaultEndpoint(PCWSTR(w.as_ptr()), role).is_ok();
        }
        ok
    }
}

// ---------------------------------------------------------------------------
// Parlaklık: harici monitörler DDC/CI ile (dizüstü ekranı uygulama tarafında WMI)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct Brightness {
    pub rect: Rect,
    /// 0–100
    pub value: u32,
}

unsafe fn with_physical<R>(rect: Rect, f: impl Fn(HANDLE) -> Option<R>) -> Option<R> {
    let pt = POINT { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 };
    let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut n = 0u32;
    GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut n).ok()?;
    if n == 0 {
        return None;
    }
    let mut arr = vec![PHYSICAL_MONITOR::default(); n as usize];
    GetPhysicalMonitorsFromHMONITOR(hmon, &mut arr).ok()?;
    let mut out = None;
    for pm in &arr {
        if out.is_none() {
            out = f(pm.hPhysicalMonitor);
        }
    }
    let _ = DestroyPhysicalMonitors(&arr);
    out
}

/// DDC/CI destekleyen monitörün parlaklığı (%). Desteklemiyorsa None.
pub fn ddc_get(rect: Rect) -> Option<u32> {
    unsafe {
        with_physical(rect, |h| {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
            if GetMonitorBrightness(h, &mut min, &mut cur, &mut max) == 0 || max <= min {
                return None;
            }
            Some(((cur - min) * 100 / (max - min)).min(100))
        })
    }
}

pub fn ddc_set(rect: Rect, value: u32) -> bool {
    unsafe {
        with_physical(rect, |h| {
            let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
            if GetMonitorBrightness(h, &mut min, &mut cur, &mut max) == 0 || max <= min {
                return None;
            }
            let v = min + (max - min) * value.min(100) / 100;
            (SetMonitorBrightness(h, v) != 0).then_some(())
        })
        .is_some()
    }
}

// ---------------------------------------------------------------------------
// Radyolar (Wi-Fi, Bluetooth)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct RadioInfo {
    /// "wifi" | "bluetooth"
    pub kind: String,
    pub name: String,
    pub on: bool,
}

fn radios_raw() -> Vec<windows::Devices::Radios::Radio> {
    use windows::Devices::Radios::Radio;
    let mut v = Vec::new();
    if let Ok(op) = Radio::RequestAccessAsync() {
        let _ = crate::media::wait(op, 3000);
    }
    let Ok(op) = Radio::GetRadiosAsync() else { return v };
    let Ok(list) = crate::media::wait(op, 3000) else { return v };
    for i in 0..list.Size().unwrap_or(0) {
        if let Ok(r) = list.GetAt(i) {
            v.push(r);
        }
    }
    v
}

fn kind_str(r: &windows::Devices::Radios::Radio) -> Option<&'static str> {
    use windows::Devices::Radios::RadioKind;
    match r.Kind().ok()? {
        RadioKind::WiFi => Some("wifi"),
        RadioKind::Bluetooth => Some("bluetooth"),
        _ => None,
    }
}

pub fn radios() -> Vec<RadioInfo> {
    use windows::Devices::Radios::RadioState;
    radios_raw()
        .iter()
        .filter_map(|r| {
            Some(RadioInfo {
                kind: kind_str(r)?.into(),
                name: r.Name().map(|n| n.to_string()).unwrap_or_default(),
                on: r.State().ok()? == RadioState::On,
            })
        })
        .collect()
}

/// `kind` ("wifi" | "bluetooth" | "all") radyolarını açar/kapatır.
pub fn set_radio(kind: &str, on: bool) -> bool {
    use windows::Devices::Radios::{RadioAccessStatus, RadioState};
    let mut any = false;
    for r in radios_raw() {
        let Some(k) = kind_str(&r) else { continue };
        if kind != "all" && k != kind {
            continue;
        }
        if let Ok(op) = r.SetStateAsync(if on { RadioState::On } else { RadioState::Off }) {
            any |= crate::media::wait(op, 4000).map(|s| s == RadioAccessStatus::Allowed).unwrap_or(false);
        }
    }
    any
}

// ---------------------------------------------------------------------------
// Wi-Fi ağları (Native Wifi)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct WifiNet {
    pub ssid: String,
    /// 0–100
    pub signal: u32,
    pub secured: bool,
    pub connected: bool,
    /// Kayıtlı profil adı (varsa doğrudan bağlanılır)
    pub profile: String,
}

struct Wlan(HANDLE);
impl Drop for Wlan {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.0, None);
        }
    }
}

unsafe fn wlan() -> Result<(Wlan, GUID), u32> {
    let mut ver = 0u32;
    let mut h = HANDLE::default();
    let r = WlanOpenHandle(2, None, &mut ver, &mut h);
    if r != 0 {
        return Err(r);
    }
    let h = Wlan(h);
    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    let r = WlanEnumInterfaces(h.0, None, &mut list);
    if r != 0 || list.is_null() {
        return Err(if r == 0 { 2 } else { r });
    }
    let n = (*list).dwNumberOfItems;
    let guid = if n > 0 { Some((*list).InterfaceInfo[0].InterfaceGuid) } else { None };
    WlanFreeMemory(list as *const _);
    guid.map(|g| (h, g)).ok_or(2) // ERROR_FILE_NOT_FOUND: kablosuz kart yok
}

/// Çevredeki ağlar. Hata kodu: 5 = erişim yok (Windows konum izni kapalı).
pub fn wifi_networks(scan: bool) -> Result<Vec<WifiNet>, u32> {
    unsafe {
        let (h, guid) = wlan()?;
        if scan {
            let _ = WlanScan(h.0, &guid, None, None, None);
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
        let r = WlanGetAvailableNetworkList(h.0, &guid, 0, None, &mut list);
        if r != 0 || list.is_null() {
            return Err(if r == 0 { 5 } else { r });
        }
        let n = (*list).dwNumberOfItems as usize;
        let items = std::slice::from_raw_parts((*list).Network.as_ptr(), n);
        let mut out: Vec<WifiNet> = Vec::new();
        for it in items {
            let len = (it.dot11Ssid.uSSIDLength as usize).min(32);
            let ssid = String::from_utf8_lossy(&it.dot11Ssid.ucSSID[..len]).to_string();
            if ssid.is_empty() {
                continue;
            }
            let plen = it.strProfileName.iter().position(|&c| c == 0).unwrap_or(0);
            let net = WifiNet {
                ssid: ssid.clone(),
                signal: it.wlanSignalQuality,
                secured: it.bSecurityEnabled.as_bool(),
                connected: it.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
                profile: String::from_utf16_lossy(&it.strProfileName[..plen]),
            };
            // Aynı ağ profilli ve profilsiz iki kez gelir: bilgisi zengin olanı tut
            if let Some(o) = out.iter_mut().find(|o| o.ssid == ssid) {
                if (net.connected && !o.connected) || (o.profile.is_empty() && !net.profile.is_empty()) {
                    *o = WifiNet { signal: o.signal.max(net.signal), ..net };
                }
                continue;
            }
            out.push(net);
        }
        WlanFreeMemory(list as *const _);
        out.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.signal.cmp(&a.signal)));
        Ok(out)
    }
}

/// Kayıtlı profille bağlanır.
pub fn wifi_connect(profile: &str) -> bool {
    unsafe {
        let Ok((h, guid)) = wlan() else { return false };
        let p = wide(profile);
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(p.as_ptr()),
            pDot11Ssid: std::ptr::null_mut(),
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_any,
            dwFlags: 0,
        };
        WlanConnect(h.0, &guid, &params, None) == 0
    }
}

pub fn wifi_disconnect() -> bool {
    unsafe {
        let Ok((h, guid)) = wlan() else { return false };
        WlanDisconnect(h.0, &guid, None) == 0
    }
}
