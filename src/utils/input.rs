#![allow(dead_code)]

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, SendInput, INPUT, INPUT_MOUSE, INPUT_KEYBOARD, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MapVirtualKeyW, MAPVK_VK_TO_VSC};

pub struct Input;

impl Input {
    #[cfg(target_os = "windows")]
    pub fn is_key_down(vk_code: i32) -> bool {
        unsafe { GetAsyncKeyState(vk_code) < 0 }
    }

    pub fn is_key_pressed(vk_code: i32, state: &mut bool) -> bool {
        let pressed = Self::is_key_down(vk_code);
        let just_pressed = pressed && !*state;
        *state = pressed;
        just_pressed
    }

    #[cfg(target_os = "windows")]
    pub fn move_mouse(dx: i32, dy: i32) {
        unsafe {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn click_mouse() {
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            // separate down/up so roblox registers it across frames
            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
            // ~40ms hold = 2-3 frames at 60fps
            std::thread::sleep(std::time::Duration::from_millis(40));
            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Fast mouse click with 12ms hold (Blade Ball auto-parry).
    #[cfg(target_os = "windows")]
    pub fn click_mouse_fast() {
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(12));
            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn mouse_down() {
        unsafe {
            let down = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[down], std::mem::size_of::<INPUT>() as i32);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn mouse_up() {
        unsafe {
            let up = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            SendInput(&[up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Simulate a key press with 40ms hold for Roblox input polling.
    #[cfg(target_os = "windows")]
    pub fn send_key(vk_code: u16) {
        unsafe {
            // roblox requires hardware scan codes
            let scan = MapVirtualKeyW(vk_code as u32, MAPVK_VK_TO_VSC) as u16;

            let key_down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk_code),
                        wScan: scan,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let key_up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk_code),
                        wScan: scan,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            // separate down/up so roblox registers it across frames
            SendInput(&[key_down], std::mem::size_of::<INPUT>() as i32);
            // ~40ms hold = 2-3 frames at 60fps
            std::thread::sleep(std::time::Duration::from_millis(40));
            SendInput(&[key_up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Fast key press with 12ms hold (Blade Ball auto-parry).
    #[cfg(target_os = "windows")]
    pub fn send_key_fast(vk_code: u16) {
        unsafe {
            let scan = MapVirtualKeyW(vk_code as u32, MAPVK_VK_TO_VSC) as u16;
            let key_down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk_code),
                        wScan: scan,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let key_up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk_code),
                        wScan: scan,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[key_down], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(std::time::Duration::from_millis(12));
            SendInput(&[key_up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Returns (W, A, S, D, Space, Ctrl) key states.
    #[cfg(target_os = "windows")]
    pub fn get_movement_keys() -> (bool, bool, bool, bool, bool, bool) {
        (
            Self::is_key_down(0x57), // W
            Self::is_key_down(0x41), // A
            Self::is_key_down(0x53), // S
            Self::is_key_down(0x44), // D
            Self::is_key_down(0x20), // Space (up)
            Self::is_key_down(0x11) || Self::is_key_down(0xA2) || Self::is_key_down(0xA3), // Ctrl/LCtrl/RCtrl (down)
        )
    }

    /// Returns (W, A, S, D, CapsLock, Ctrl) — CapsLock replaces Space (exits vehicles).
    #[cfg(target_os = "windows")]
    pub fn get_vehicle_movement_keys() -> (bool, bool, bool, bool, bool, bool) {
        (
            Self::is_key_down(0x57), // W
            Self::is_key_down(0x41), // A
            Self::is_key_down(0x53), // S
            Self::is_key_down(0x44), // D
            Self::is_key_down(0x14), // CapsLock (up) - 0x14 = VK_CAPITAL
            Self::is_key_down(0x11) || Self::is_key_down(0xA2) || Self::is_key_down(0xA3), // Ctrl/LCtrl/RCtrl (down)
        )
    }

    #[cfg(target_os = "windows")]
    pub fn get_mouse_position() -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        use windows::Win32::Foundation::POINT;
        
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut point);
        }
        (point.x, point.y)
    }

    #[cfg(target_os = "windows")]
    pub fn set_mouse_position(x: i32, y: i32) -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
        use windows::Win32::Foundation::POINT;
        
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut point);
            let _ = SetCursorPos(x, y);
        }
        (point.x, point.y)
    }

    #[cfg(target_os = "windows")]
    pub fn get_screen_center() -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (width / 2, height / 2)
        }
    }

}

#[allow(dead_code)]
pub mod vk {
    pub const LBUTTON: i32 = 0x01;
    pub const RBUTTON: i32 = 0x02;
    pub const SPACE: i32 = 0x20;
    pub const SHIFT: i32 = 0x10;
    pub const CTRL: i32 = 0x11;
    pub const ALT: i32 = 0x12;
    
    pub const KEY_W: i32 = 0x57;
    pub const KEY_A: i32 = 0x41;
    pub const KEY_S: i32 = 0x53;
    pub const KEY_D: i32 = 0x44;
    pub const KEY_G: i32 = 0x47;
    
    pub const F1: i32 = 0x70;
    pub const F2: i32 = 0x71;
    pub const F3: i32 = 0x72;
    pub const F4: i32 = 0x73;
    pub const F5: i32 = 0x74;
    pub const F6: i32 = 0x75;
    pub const F7: i32 = 0x76;
    pub const F8: i32 = 0x77;
    pub const F9: i32 = 0x78;
    pub const F10: i32 = 0x79;
    pub const F11: i32 = 0x7A;
    pub const F12: i32 = 0x7B;
}
