//! Klavye/boşta kalma yardımcıları.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    LASTINPUTINFO, VK_LWIN,
};

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
