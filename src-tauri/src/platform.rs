//! Platform köprüsü. Windows'ta dock-sys'i çağırır; diğer sistemlerde
//! (sadece derleme/geliştirme için) etkisiz taslaklar kullanılır.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AppWindow {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
    pub minimized: bool,
    pub monitor: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StartApp {
    pub name: String,
    pub path: String,
    pub shortcut: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MediaState {
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
    pub fn enforce_taskbar() {
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
    pub fn round_region(hwnd: isize, w: i32, h: i32, r: i32) {
        backdrop::round_region(hwnd, w, h, r)
    }
    pub fn list_windows(exclude: &[isize]) -> Vec<AppWindow> {
        winlist::list(exclude)
            .into_iter()
            .map(|w| AppWindow {
                hwnd: w.hwnd,
                title: w.title,
                exe: w.exe,
                minimized: w.minimized,
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
    pub fn close(hwnd: isize) {
        winlist::close(hwnd)
    }
    pub fn launch(target: &str, args: Option<&str>) -> bool {
        winlist::launch(target, args)
    }
    pub fn icon_png(path: &str, size: i32) -> Option<Vec<u8>> {
        icons::icon_png(path, size)
    }
    pub fn media_state() -> MediaState {
        match media::state() {
            Ok(m) => MediaState {
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
    pub fn media_control(action: &str) -> bool {
        media::control(action).unwrap_or(false)
    }
    pub fn open_start() {
        input::open_start_menu()
    }
    pub fn win_combo(letter: char) {
        input::win_combo(letter)
    }
    pub fn press_enter() {
        input::press_enter()
    }
    pub fn taskbar_peek() -> Option<Rect> {
        taskbar::peek().map(t)
    }
    pub fn taskbar_rect() -> Option<Rect> {
        taskbar::main_rect().map(t)
    }
    pub fn foreground_class() -> String {
        monitor::foreground_class()
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
    pub fn glass_update(hwnd: isize, w: f32, h: f32, r: f32, tint: (u8, u8, u8, u8), on: bool) -> Result<(), String> {
        glass::update(hwnd, w, h, r, tint, on).map_err(|e| e.to_string())
    }
    pub fn glass_detach(hwnd: isize) {
        glass::detach(hwnd)
    }
    pub fn show_no_activate(hwnd: isize) {
        monitor::show_no_activate(hwnd)
    }
    pub fn hide(hwnd: isize) {
        monitor::hide(hwnd)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn hide_taskbar(_monitors: &[Rect]) -> u32 {
        2
    }
    pub fn restore_taskbar(_prev: u32) {}
    pub fn enforce_taskbar() {}
    pub fn reserve(_hwnd: isize, m: Rect, _height: i32) -> Rect {
        m
    }
    pub fn release(_hwnd: isize) {}
    pub fn release_all() {}
    pub fn apply_backdrop(_hwnd: isize, _mode: &str, _dark: bool) {}
    pub fn round_region(_hwnd: isize, _w: i32, _h: i32, _r: i32) {}
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
    pub fn close(_hwnd: isize) {}
    pub fn launch(_target: &str, _args: Option<&str>) -> bool {
        false
    }
    pub fn icon_png(_path: &str, _size: i32) -> Option<Vec<u8>> {
        None
    }
    pub fn media_state() -> MediaState {
        MediaState::default()
    }
    pub fn media_thumbnail() -> Option<Vec<u8>> {
        None
    }
    pub fn media_control(_action: &str) -> bool {
        false
    }
    pub fn open_start() {}
    pub fn win_combo(_letter: char) {}
    pub fn press_enter() {}
    pub fn taskbar_peek() -> Option<Rect> {
        None
    }
    pub fn taskbar_rect() -> Option<Rect> {
        None
    }
    pub fn foreground_class() -> String {
        String::new()
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
    pub fn glass_update(_hwnd: isize, _w: f32, _h: f32, _r: f32, _tint: (u8, u8, u8, u8), _on: bool) -> Result<(), String> {
        Ok(())
    }
    pub fn glass_detach(_hwnd: isize) {}
    pub fn show_no_activate(_hwnd: isize) {}
    pub fn hide(_hwnd: isize) {}
}

pub use imp::*;
