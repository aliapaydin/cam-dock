//! Klavye/boşta kalma yardımcıları.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, LASTINPUTINFO, VIRTUAL_KEY, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RBUTTON,
    VK_RETURN,
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
