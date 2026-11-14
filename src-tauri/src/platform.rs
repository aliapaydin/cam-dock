//! Platform köprüsü. Windows'ta dock-sys'i çağırır; diğer sistemlerde
//! (sadece derleme/geliştirme için) etkisiz taslaklar kullanılır.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AppWindow {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
    pub minimized: bool,
    pub aumid: String,
    pub monitor: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StartApp {
    pub name: String,
    pub path: String,
    pub shortcut: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CatalogApp {
    pub name: String,
    pub path: String,
    pub aumid: String,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct SysStats {
    pub cpu: f32,
    pub ram: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub gpu: f32,
    pub disk: f32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SysInfo {
    pub uptime_sec: u64,
    pub disk_free_gb: f32,
    pub disk_total_gb: f32,
    pub battery: i32,
    pub charging: bool,
    pub procs: u32,
    pub cores: u32,
    pub cpu_name: String,
    pub os: String,
}

/// Atlama listesi öğesi (uygulamanın taskbar sağ tık menüsündeki görev/öğe).
#[derive(Debug, Clone, Serialize, Default)]
pub struct JumpEntry {
    pub title: String,
    pub path: String,
    pub args: String,
    pub separator: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct JumpList {
    pub categories: Vec<(String, Vec<JumpEntry>)>,
    pub tasks: Vec<JumpEntry>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TrayApp {
    pub name: String,
    pub exe: String,
    #[serde(skip)]
    pub icon: Option<Vec<u8>>,
    pub promoted: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MediaState {
    pub id: String,
    pub current: bool,
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub source: String,
    pub track_key: String,
}

#[allow(dead_code)]
pub struct ClipContent {
    pub text: Option<String>,
    /// Önizleme PNG'si
    pub thumb: Option<Vec<u8>>,
    pub size: Option<(u32, u32)>,
    pub dib: Option<Vec<u8>>,
    pub owner: Option<String>,
    pub private: bool,
}

#[allow(dead_code)]
pub enum Wall {
    File(String),
    Solid(u8, u8, u8),
}

/// Monitör dikdörtgeni (fiziksel piksel): sol, üst, sağ, alt
pub type Rect = (i32, i32, i32, i32);

#[cfg(windows)]
mod imp {
    use super::*;
    use dock_sys::*;

    fn r(m: Rect) -> monitor::Rect {
        monitor::Rect {
            left: m.0,
            top: m.1,
            right: m.2,
            bottom: m.3,
        }
    }
    fn t(m: monitor::Rect) -> Rect {
        (m.left, m.top, m.right, m.bottom)
    }
    pub fn hide_taskbar(monitors: &[Rect]) -> u32 {
        let rects: Vec<_> = monitors.iter().copied().map(r).collect();
        taskbar::hide(&rects)
    }
    pub fn restore_taskbar(prev: u32) {
        taskbar::restore(prev)
    }
    pub fn enforce_taskbar() -> usize {
        taskbar::enforce_hidden()
    }
    pub fn reserve(hwnd: isize, m: Rect, height: i32) -> Rect {
        t(appbar::reserve_bottom(hwnd, r(m), height))
    }
    pub fn release(hwnd: isize) {
        appbar::release(hwnd)
    }
    pub fn release_all() {
        appbar::release_all()
    }
    pub fn apply_backdrop(hwnd: isize, mode: &str, dark: bool) {
        let tint = if dark {
            (16, 20, 34, 0x50)
        } else {
            (245, 247, 252, 0x70)
        };
        backdrop::apply(hwnd, backdrop::Backdrop::parse(mode), tint);
    }
    pub fn round_region(hwnd: isize, w: i32, h: i32, r: i32, top: i32) {
        backdrop::round_region(hwnd, w, h, r, top)
    }
    pub fn rect_region(hwnd: isize, rects: &[(i32, i32, i32, i32)]) {
        backdrop::rect_region(hwnd, rects)
    }
    pub fn list_windows(exclude: &[isize]) -> Vec<AppWindow> {
        winlist::list(exclude)
            .into_iter()
            .map(|w| AppWindow {
                hwnd: w.hwnd,
                title: w.title,
                exe: w.exe,
                minimized: w.minimized,
                aumid: w.aumid,
                monitor: w.monitor,
            })
            .collect()
    }
    pub fn start_menu_apps() -> Vec<StartApp> {
        shortcut::start_menu_apps()
            .into_iter()
            .map(|a| StartApp {
                name: a.name,
                path: a.path,
                shortcut: a.shortcut,
            })
            .collect()
    }
    pub fn resolve_shortcut(path: &str) -> Option<String> {
        shortcut::resolve(path)
    }
    pub fn activate(hwnd: isize, last_fg: isize) {
        winlist::activate(hwnd, last_fg)
    }
    pub fn activate_group(hwnds: &[isize], last_fg: isize) {
        winlist::activate_group(hwnds, last_fg)
    }
    pub fn bring_front(hwnd: isize) {
        winlist::bring_front(hwnd)
    }
    pub fn mouse_down() -> bool {
        input::mouse_down()
    }
    pub fn shell_flyout_open() -> bool {
        monitor::foreground_is_shell_flyout()
    }
    pub fn thumb_show(dest: isize, src: isize, x: i32, y: i32, w: i32, h: i32) -> Option<isize> {
        thumbs::show(dest, src, x, y, w, h)
    }
    pub fn thumb_remove(t: isize) {
        thumbs::remove(t)
    }
    pub fn close(hwnd: isize) {
        winlist::close(hwnd)
    }
    pub fn launch(target: &str, args: Option<&str>) -> bool {
        winlist::launch(target, args)
    }
    pub fn launch_admin(target: &str, args: Option<&str>) -> bool {
        winlist::launch_verb(target, args, "runas")
    }
    pub fn icon_png(path: &str, size: i32) -> Option<Vec<u8>> {
        icons::icon_png(path, size)
    }
    pub fn shell_icon_png(name: &str, size: i32) -> Option<Vec<u8>> {
        icons::shell_icon_png(name, size)
    }
    pub fn window_icon_png(hwnd: isize, size: i32) -> Option<Vec<u8>> {
        icons::window_icon_png(hwnd, size)
    }
    pub fn media_state() -> MediaState {
        match media::state() {
            Ok(m) => MediaState {
                id: m.id,
                current: m.current,
                active: m.active,
                title: m.title,
                artist: m.artist,
                album: m.album,
                playing: m.playing,
                position_ms: m.position_ms,
                duration_ms: m.duration_ms,
                source: m.source,
                track_key: m.track_key,
            },
            Err(_) => MediaState::default(),
        }
    }
    pub fn media_thumbnail() -> Option<Vec<u8>> {
        media::thumbnail().ok().flatten()
    }
    pub fn media_states() -> Vec<MediaState> {
        media::states()
            .unwrap_or_default()
            .into_iter()
            .map(|m| MediaState {
                id: m.id,
                current: m.current,
                active: m.active,
                title: m.title,
                artist: m.artist,
                album: m.album,
                playing: m.playing,
                position_ms: m.position_ms,
                duration_ms: m.duration_ms,
                source: m.source,
                track_key: m.track_key,
            })
            .collect()
    }
    pub fn media_control_id(id: &str, action: &str) -> bool {
        media::control_id(id, action).unwrap_or(false)
    }
    pub fn media_thumbnail_id(id: &str) -> Option<Vec<u8>> {
        media::thumbnail_id(id).ok().flatten()
    }
    /// Katalog COM (STA) ister: kendi iş parçacığında çalıştır.
    pub fn catalog_apps() -> Vec<CatalogApp> {
        std::thread::spawn(|| {
            catalog::apps()
                .into_iter()
                .map(|a| CatalogApp { name: a.name, path: a.path, aumid: a.aumid })
                .collect()
        })
        .join()
        .unwrap_or_default()
    }
    pub fn pick_app(owner: isize) -> Option<String> {
        catalog::pick_app(owner)
    }
    pub fn tray_apps() -> Vec<TrayApp> {
        tray::running_apps()
            .into_iter()
            .map(|a| TrayApp { name: a.name, exe: a.exe, icon: a.icon, promoted: a.promoted })
            .collect()
    }
    /// Uygulamanın atlama listesi (COM/STA: kendi iş parçacığında).
    pub fn jumplist(path: &str, aumid: &str) -> JumpList {
        let (path, aumid) = (path.to_string(), aumid.to_string());
        std::thread::spawn(move || {
            let conv = |e: jumplist::Entry| JumpEntry { title: e.title, path: e.path, args: e.args, separator: e.separator };
            let jl = jumplist::for_app(&path, &aumid);
            JumpList {
                categories: jl.categories.into_iter().map(|(n, v)| (n, v.into_iter().map(conv).collect())).collect(),
                tasks: jl.tasks.into_iter().map(conv).collect(),
            }
        })
        .join()
        .unwrap_or_default()
    }
    pub fn trayhook_start() {
        trayhook::start()
    }
    pub fn send_combo(spec: &str) -> bool {
        input::send_combo(spec)
    }
    pub fn net_bytes() -> (u64, u64) {
        dock_sys::net::bytes()
    }
    /// (ses 0–100, sessiz)
    pub fn volume() -> (f32, bool) {
        let a = dock_sys::quick::audio_state();
        (a.volume, a.muted)
    }
    pub fn set_volume(v: f32) -> bool {
        dock_sys::quick::set_volume(v)
    }
    pub fn set_mute(m: bool) -> bool {
        dock_sys::quick::set_mute(m)
    }
    pub fn trayhook_stop() {
        trayhook::stop()
    }
    /// Tepsi simgesinin kendi sağ tık menüsü; simge bilinmiyorsa false.
    pub fn tray_context_menu(exe: &str) -> bool {
        if trayhook::context_menu(exe) {
            return true;
        }
        // Simge henüz bilinmiyor: uygulamalardan yeniden bildirmelerini iste, bekle, tekrar dene
        trayhook::rebroadcast();
        for _ in 0..8 {
            std::thread::sleep(std::time::Duration::from_millis(150));
            if trayhook::context_menu(exe) {
                return true;
            }
        }
        crate::dlog!("tepsi menüsü bulunamadı: {exe}; bilinenler: {:?}", trayhook::known());
        false
    }
    /// Ana iş parçacığında çağrılmalı.
    pub fn install_taskbar_hook() -> bool {
        taskbar::install_hook()
    }
    pub fn taskbar_hook_current() -> bool {
        taskbar::hook_current()
    }
    pub struct Sampler(sysinfo::Sampler);
    impl Sampler {
        pub fn new() -> Self {
            Sampler(sysinfo::Sampler::new())
        }
        pub fn sample(&mut self) -> SysStats {
            let s = self.0.sample();
            SysStats { cpu: s.cpu, ram: s.ram, ram_used_gb: s.ram_used_gb, ram_total_gb: s.ram_total_gb, gpu: s.gpu, disk: s.disk }
        }
    }
    pub fn media_control(action: &str) -> bool {
        media::control(action).unwrap_or(false)
    }
    pub fn open_start() {
        input::open_start_menu()
    }
    pub fn start_menu_open() -> bool {
        monitor::foreground_is_start_menu()
    }
    pub fn win_combo(letter: char) {
        input::win_combo(letter)
    }
    pub fn win_combo_tab() {
        input::win_tab()
    }
    pub fn monitor_list() -> Vec<(String, Rect, f64, bool)> {
        monitor::list()
            .into_iter()
            .map(|m| (m.name, t(m.rect), m.scale, m.primary))
            .collect()
    }
    /// Windows açık temada mı? Önce görev çubuğu/Başlat ("Windows modu"),
    /// okunamazsa uygulama modu.
    pub fn light_theme() -> Option<bool> {
        monitor::light_theme(true).or_else(|| monitor::light_theme(false))
    }
    pub fn idle_ms() -> u32 {
        input::idle_ms()
    }
    pub fn foreground() -> isize {
        monitor::foreground()
    }
    pub fn fullscreen_monitor(exclude: &[isize]) -> Option<String> {
        monitor::fullscreen_monitor(exclude)
    }
    pub fn foreground_app_rect(exclude: &[isize]) -> Option<Rect> {
        monitor::foreground_app_rect(exclude).map(|(_, rc)| t(rc))
    }
    pub fn cursor_pos() -> (i32, i32) {
        monitor::cursor_pos()
    }
    pub fn move_to(hwnd: isize, x: i32, y: i32) {
        monitor::move_to(hwnd, x, y)
    }
    pub fn set_bounds(hwnd: isize, x: i32, y: i32, w: i32, h: i32) {
        monitor::set_bounds(hwnd, x, y, w, h)
    }
    pub fn make_popup(hwnd: isize) -> bool {
        monitor::make_popup(hwnd)
    }
    /// Ana iş parçacığında çağrılmalı.
    pub fn install_dock_proc(hwnd: isize) -> bool {
        subclass::install(hwnd)
    }
    /// Ana iş parçacığında çağrılmalı.
    pub fn glass_attach(hwnd: isize) -> Result<(), String> {
        glass::attach(hwnd).map_err(|e| e.to_string())
    }
    pub fn glass_attached(hwnd: isize) -> bool {
        glass::is_attached(hwnd)
    }
    pub fn glass_update(hwnd: isize, w: f32, h: f32, r: f32, tint: (u8, u8, u8, u8), on: bool, top: f32) -> Result<(), String> {
        glass::update(hwnd, w, h, r, tint, on, top).map_err(|e| e.to_string())
    }
    pub fn glass_detach(hwnd: isize) {
        glass::detach(hwnd)
    }
    /// Ana iş parçacığında çağrılmalı.
    pub fn glass_animate(hwnd: isize, show: bool, ms: i64) {
        let _ = glass::animate(hwnd, show, ms);
    }
    pub fn show_no_activate(hwnd: isize) {
        monitor::show_no_activate(hwnd)
    }
    pub fn hide(hwnd: isize) {
        monitor::hide(hwnd)
    }
    pub fn is_visible(hwnd: isize) -> bool {
        monitor::is_visible(hwnd)
    }
    pub fn sys_info() -> SysInfo {
        let i = sysinfo::info();
        SysInfo {
            uptime_sec: i.uptime_sec,
            disk_free_gb: i.disk_free_gb,
            disk_total_gb: i.disk_total_gb,
            battery: i.battery,
            charging: i.charging,
            procs: i.procs,
            cores: i.cores,
            cpu_name: i.cpu_name,
            os: i.os,
        }
    }
    pub fn app_of_window(hwnd: isize) -> Option<(String, String)> {
        winlist::app_of_window(hwnd)
    }
    pub fn wallpaper(m: Rect) -> Option<Wall> {
        match dock_sys::wallpaper::for_monitor(r(m))? {
            wallpaper::Source::File(p) => Some(Wall::File(p)),
            wallpaper::Source::Solid(a, b, c) => Some(Wall::Solid(a, b, c)),
        }
    }
    pub fn autostart_get() -> bool {
        autostart::get().is_some() || autostart::task_exists()
    }
    pub fn task_exists() -> bool {
        autostart::task_exists()
    }
    pub fn task_create(exe: &str) -> bool {
        autostart::task_create(exe)
    }
    pub fn task_delete() {
        autostart::task_delete()
    }
    pub fn session_ending() -> bool {
        taskbar::session_ending()
    }
    pub fn wait_process_exit(pid: u32) {
        winlist::wait_exit(pid)
    }
    /// Microsoft Store (MSIX) sürümü mü?
    pub fn is_store() -> bool {
        static S: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *S.get_or_init(dock_sys::package::is_packaged)
    }
    pub fn store_startup_set(enabled: bool) -> bool {
        dock_sys::package::startup_set(enabled)
    }
    pub fn taskbar_ensure_autohide() {
        taskbar::ensure_autohide()
    }
    pub fn explorer_pid() -> u32 {
        taskbar::explorer_pid()
    }
    pub fn single_instance() -> bool {
        dock_sys::single_instance()
    }
    pub fn privacy_using(cap: &str) -> Vec<String> {
        dock_sys::privacy::using(cap)
    }
    pub fn thumbnail_png(path: &str, size: i32) -> Option<Vec<u8>> {
        icons::thumbnail_png(path, size)
    }
    pub fn pick_folder(owner: isize) -> Option<String> {
        catalog::pick_folder(owner)
    }
    pub fn reg_dword(key: &str, name: &str) -> Option<u32> {
        autostart::reg_dword(key, name)
    }
    pub fn reg_delete(key: &str, name: &str) {
        autostart::reg_delete(key, name)
    }
    pub fn reg_set_sz(key: &str, name: Option<&str>, value: &str) -> bool {
        autostart::reg_set_sz(key, name, value)
    }
    pub fn reg_delete_tree(key: &str) {
        autostart::reg_delete_tree(key)
    }
    pub fn clip_listen(cb: impl Fn() + Send + Sync + 'static) {
        clipboard::listen(cb)
    }
    pub fn clip_read() -> Option<ClipContent> {
        let c = clipboard::read()?;
        let thumb = c.image.as_ref().and_then(|i| clipboard::thumbnail_png(i, 360));
        Some(ClipContent {
            text: c.text,
            thumb,
            size: c.image.as_ref().map(|i| (i.0, i.1)),
            dib: c.dib,
            owner: c.owner,
            private: c.private,
        })
    }
    pub fn clip_set_text(s: &str) -> bool {
        clipboard::set_text(s)
    }
    pub fn clip_set_dib(d: &[u8]) -> bool {
        clipboard::set_dib(d)
    }
    pub fn autostart_set(exe: Option<&str>) -> bool {
        autostart::set(exe)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn hide_taskbar(_monitors: &[Rect]) -> u32 {
        2
    }
    pub fn restore_taskbar(_prev: u32) {}
    pub fn enforce_taskbar() -> usize {
        0
    }
    pub fn reserve(_hwnd: isize, m: Rect, _height: i32) -> Rect {
        m
    }
    pub fn release(_hwnd: isize) {}
    pub fn release_all() {}
    pub fn apply_backdrop(_hwnd: isize, _mode: &str, _dark: bool) {}
    pub fn round_region(_hwnd: isize, _w: i32, _h: i32, _r: i32, _top: i32) {}
    pub fn rect_region(_hwnd: isize, _rects: &[(i32, i32, i32, i32)]) {}
    pub fn list_windows(_exclude: &[isize]) -> Vec<AppWindow> {
        Vec::new()
    }
    pub fn start_menu_apps() -> Vec<StartApp> {
        Vec::new()
    }
    pub fn resolve_shortcut(_path: &str) -> Option<String> {
        None
    }
    pub fn activate(_hwnd: isize, _last_fg: isize) {}
    pub fn activate_group(_hwnds: &[isize], _last_fg: isize) {}
    pub fn bring_front(_hwnd: isize) {}
    pub fn mouse_down() -> bool {
        false
    }
    pub fn shell_flyout_open() -> bool {
        false
    }
    pub fn thumb_show(_dest: isize, _src: isize, _x: i32, _y: i32, _w: i32, _h: i32) -> Option<isize> {
        None
    }
    pub fn thumb_remove(_t: isize) {}
    pub fn close(_hwnd: isize) {}
    pub fn launch(_target: &str, _args: Option<&str>) -> bool {
        false
    }
    pub fn launch_admin(_target: &str, _args: Option<&str>) -> bool {
        false
    }
    pub fn icon_png(_path: &str, _size: i32) -> Option<Vec<u8>> {
        None
    }
    pub fn shell_icon_png(_name: &str, _size: i32) -> Option<Vec<u8>> {
        None
    }
    pub fn window_icon_png(_hwnd: isize, _size: i32) -> Option<Vec<u8>> {
        None
    }
    pub fn media_state() -> MediaState {
        MediaState::default()
    }
    pub fn media_thumbnail() -> Option<Vec<u8>> {
        None
    }
    pub fn media_states() -> Vec<MediaState> {
        Vec::new()
    }
    pub fn media_control_id(_id: &str, _action: &str) -> bool {
        false
    }
    pub fn media_thumbnail_id(_id: &str) -> Option<Vec<u8>> {
        None
    }
    pub fn catalog_apps() -> Vec<CatalogApp> {
        Vec::new()
    }
    pub fn pick_app(_owner: isize) -> Option<String> {
        None
    }
    pub fn tray_apps() -> Vec<TrayApp> {
        Vec::new()
    }
    pub fn install_taskbar_hook() -> bool {
        false
    }
    pub fn jumplist(_path: &str, _aumid: &str) -> JumpList {
        JumpList::default()
    }
    pub fn trayhook_start() {}
    pub fn send_combo(_spec: &str) -> bool {
        false
    }
    pub fn net_bytes() -> (u64, u64) {
        (0, 0)
    }
    pub fn volume() -> (f32, bool) {
        (50.0, false)
    }
    pub fn set_volume(_v: f32) -> bool {
        false
    }
    pub fn set_mute(_m: bool) -> bool {
        false
    }
    pub fn trayhook_stop() {}
    pub fn tray_context_menu(_exe: &str) -> bool {
        false
    }
    pub fn taskbar_hook_current() -> bool {
        true
    }
    pub struct Sampler;
    impl Sampler {
        pub fn new() -> Self {
            Sampler
        }
        pub fn sample(&mut self) -> SysStats {
            SysStats::default()
        }
    }
    pub fn media_control(_action: &str) -> bool {
        false
    }
    pub fn open_start() {}
    pub fn start_menu_open() -> bool {
        false
    }
    pub fn win_combo(_letter: char) {}
    pub fn win_combo_tab() {}
    pub fn monitor_list() -> Vec<(String, Rect, f64, bool)> {
        vec![("PRIMARY".into(), (0, 0, 1920, 1080), 1.0, true)]
    }
    pub fn light_theme() -> Option<bool> {
        None
    }
    pub fn idle_ms() -> u32 {
        0
    }
    pub fn foreground() -> isize {
        0
    }
    pub fn fullscreen_monitor(_exclude: &[isize]) -> Option<String> {
        None
    }
    pub fn foreground_app_rect(_exclude: &[isize]) -> Option<Rect> {
        None
    }
    pub fn cursor_pos() -> (i32, i32) {
        (0, 0)
    }
    pub fn move_to(_hwnd: isize, _x: i32, _y: i32) {}
    pub fn set_bounds(_hwnd: isize, _x: i32, _y: i32, _w: i32, _h: i32) {}
    pub fn make_popup(_hwnd: isize) -> bool {
        false
    }
    pub fn install_dock_proc(_hwnd: isize) -> bool {
        false
    }
    pub fn glass_attach(_hwnd: isize) -> Result<(), String> {
        Err("yalnızca Windows".into())
    }
    pub fn glass_attached(_hwnd: isize) -> bool {
        false
    }
    pub fn glass_update(_hwnd: isize, _w: f32, _h: f32, _r: f32, _tint: (u8, u8, u8, u8), _on: bool, _top: f32) -> Result<(), String> {
        Ok(())
    }
    pub fn glass_detach(_hwnd: isize) {}
    pub fn glass_animate(_hwnd: isize, _show: bool, _ms: i64) {}
    pub fn show_no_activate(_hwnd: isize) {}
    pub fn hide(_hwnd: isize) {}
    pub fn is_visible(_hwnd: isize) -> bool {
        false
    }
    pub fn sys_info() -> SysInfo {
        SysInfo { battery: -1, ..Default::default() }
    }
    pub fn app_of_window(_hwnd: isize) -> Option<(String, String)> {
        None
    }
    pub fn wallpaper(_m: Rect) -> Option<Wall> {
        None
    }
    pub fn autostart_get() -> bool {
        false
    }
    #[allow(dead_code)]
    pub fn task_exists() -> bool {
        false
    }
    pub fn task_create(_exe: &str) -> bool {
        false
    }
    pub fn task_delete() {}
    pub fn session_ending() -> bool {
        false
    }
    pub fn wait_process_exit(_pid: u32) {}
    pub fn is_store() -> bool {
        false
    }
    pub fn store_startup_set(_enabled: bool) -> bool {
        false
    }
    pub fn taskbar_ensure_autohide() {}
    pub fn explorer_pid() -> u32 {
        0
    }
    pub fn single_instance() -> bool {
        true
    }
    pub fn privacy_using(_cap: &str) -> Vec<String> {
        Vec::new()
    }
    pub fn thumbnail_png(_path: &str, _size: i32) -> Option<Vec<u8>> {
        None
    }
    pub fn pick_folder(_owner: isize) -> Option<String> {
        None
    }
    pub fn reg_dword(_key: &str, _name: &str) -> Option<u32> {
        None
    }
    pub fn reg_delete(_key: &str, _name: &str) {}
    pub fn reg_set_sz(_key: &str, _name: Option<&str>, _value: &str) -> bool {
        false
    }
    pub fn reg_delete_tree(_key: &str) {}
    pub fn clip_listen(_cb: impl Fn() + Send + Sync + 'static) {}
    pub fn clip_read() -> Option<ClipContent> {
        None
    }
    pub fn clip_set_text(_s: &str) -> bool {
        false
    }
    pub fn clip_set_dib(_d: &[u8]) -> bool {
        false
    }
    pub fn autostart_set(_exe: Option<&str>) -> bool {
        false
    }
}

pub use imp::*;
