//! Klavye/boşta kalma yardımcıları.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, LASTINPUTINFO, VIRTUAL_KEY, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RBUTTON,
    VK_RETURN, VK_TAB,
};

/// Sol ya da sağ fare tuşu şu an basılı mı?
pub fn mouse_down() -> bool {
    unsafe {
        (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0
    }
}

/// SetForegroundWindow reddedilirse kullanılan bilinen yöntem: Alt'a bas-bırak.
pub fn tap_alt() {
    let inputs = [key(VK_MENU, false), key(VK_MENU, true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    }
}

/// Win + harf kısayolu gönderir (ör. 'A' hızlı ayarlar, 'N' bildirimler, 'B' tepsi).
pub fn win_combo(letter: char) {
    let vk = VIRTUAL_KEY(letter.to_ascii_uppercase() as u16);
    let inputs = [key(VK_LWIN, false), key(vk, false), key(vk, true), key(VK_LWIN, true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn press_enter() {
    let inputs = [key(VK_RETURN, false), key(VK_RETURN, true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Windows tuşuna basılmış gibi Başlat menüsünü açar.
pub fn open_start_menu() {
    let key = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    let inputs = [key(false), key(true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Son klavye/fare girdisinden bu yana geçen milisaniye.
pub fn idle_ms() -> u32 {
    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if !GetLastInputInfo(&mut lii).as_bool() {
            return 0;
        }
        GetTickCount().wrapping_sub(lii.dwTime)
    }
}

/// Win+Tab: Görev görünümü (açık pencereler ve masaüstleri).
pub fn win_tab() {
    let inputs = [key(VK_LWIN, false), key(VK_TAB, false), key(VK_TAB, true), key(VK_LWIN, true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// "ctrl+shift+esc", "win+shift+s", "volume_up", "f5" gibi bir kısayolu gönderir.
/// Bilinmeyen tuş varsa hiçbir şey göndermez.
pub fn send_combo(spec: &str) -> bool {
    let mut vks: Vec<u16> = Vec::new();
    for part in spec.split('+').map(|p| p.trim().to_ascii_lowercase()) {
        let vk: u16 = match part.as_str() {
            "win" | "windows" => 0x5B,
            "ctrl" | "control" => 0x11,
            "shift" => 0x10,
            "alt" => 0x12,
            "tab" => 0x09,
            "enter" => 0x0D,
            "esc" | "escape" => 0x1B,
            "space" => 0x20,
            "backspace" => 0x08,
            "delete" | "del" => 0x2E,
            "home" => 0x24,
            "end" => 0x23,
            "pageup" => 0x21,
            "pagedown" => 0x22,
            "left" => 0x25,
            "up" => 0x26,
            "right" => 0x27,
            "down" => 0x28,
            "printscreen" | "prtsc" => 0x2C,
            "volume_mute" => 0xAD,
            "volume_down" => 0xAE,
            "volume_up" => 0xAF,
            "media_next" => 0xB0,
            "media_prev" => 0xB1,
            "media_stop" => 0xB2,
            "media_play" => 0xB3,
            p if p.len() == 1 && p.chars().all(|c| c.is_ascii_alphanumeric()) => p.to_ascii_uppercase().as_bytes()[0] as u16,
            p if p.starts_with('f') && p[1..].parse::<u16>().is_ok_and(|n| (1..=24).contains(&n)) => 0x6F + p[1..].parse::<u16>().unwrap(),
            _ => return false,
        };
        vks.push(vk);
    }
    if vks.is_empty() {
        return false;
    }
    let mut inputs: Vec<INPUT> = vks.iter().map(|&v| key(VIRTUAL_KEY(v), false)).collect();
    inputs.extend(vks.iter().rev().map(|&v| key(VIRTUAL_KEY(v), true)));
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) as usize == inputs.len() }
}
