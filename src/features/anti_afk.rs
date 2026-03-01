use rand::Rng;
use std::time::{Duration, Instant};

use crate::config::AntiAfkConfig;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, 
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MOUSEEVENTF_MOVE, MOUSEINPUT,
};

/// Minimum mouse movement (pixels) - must be noticeable to register
const MIN_MOUSE_MOVE: i32 = 2;
/// Maximum mouse movement (pixels) - keep small to not disrupt aim
const MAX_MOUSE_MOVE: i32 = 5;
/// Timing variance percentage (±20% of interval)
const TIMING_VARIANCE_PERCENT: f32 = 0.20;
/// Virtual key code for a harmless key (F13 - doesn't exist on most keyboards)
const VK_F13: u16 = 0x7C;

pub struct AntiAfk {
    last_action: Instant,
    was_enabled: bool,
    movement_pattern: u8, // Cycle through different movement patterns
    next_interval: Duration, // Randomized next interval
}

impl Default for AntiAfk {
    fn default() -> Self {
        Self::new()
    }
}

impl AntiAfk {
    pub fn new() -> Self {
        Self {
            last_action: Instant::now(),
            was_enabled: false,
            movement_pattern: 0,
            next_interval: Duration::from_secs(60),
        }
    }

    pub fn update(&mut self, config: &AntiAfkConfig) {
        if !config.enabled {
            if self.was_enabled {
                tracing::info!("[AntiAFK] Disabled");
                self.was_enabled = false;
            }
            return;
        }

        if !self.was_enabled {
            tracing::info!("[AntiAFK] Enabled - base interval: {}s", config.interval_secs);
            self.was_enabled = true;
            self.last_action = Instant::now();
            self.randomize_next_interval(config.interval_secs);
        }

        if self.last_action.elapsed() >= self.next_interval {
            self.perform_anti_afk_action();
            self.last_action = Instant::now();
            self.randomize_next_interval(config.interval_secs);
        }
    }

    fn randomize_next_interval(&mut self, base_secs: u32) {
        let mut rng = rand::thread_rng();
        let variance = (base_secs as f32 * TIMING_VARIANCE_PERCENT) as u32;
        let min = base_secs.saturating_sub(variance);
        let max = base_secs + variance;
        let next_secs = rng.gen_range(min..=max);
        self.next_interval = Duration::from_secs(next_secs as u64);
    }

    #[cfg(target_os = "windows")]
    fn perform_anti_afk_action(&mut self) {
        let mut rng = rand::thread_rng();
        
        match self.movement_pattern % 4 {
            0 => self.mouse_jiggle(&mut rng),
            1 => self.mouse_jiggle_diagonal(&mut rng),
            2 => self.keyboard_tap(),
            3 => self.mouse_jiggle(&mut rng), // Mouse more often
            _ => {}
        }
        
        self.movement_pattern = self.movement_pattern.wrapping_add(1);
        tracing::debug!("[AntiAFK] Performed action (pattern {})", self.movement_pattern);
    }

    #[cfg(target_os = "windows")]
    fn mouse_jiggle(&self, rng: &mut impl Rng) {
        let dx = rng.gen_range(MIN_MOUSE_MOVE..=MAX_MOUSE_MOVE);
        let direction = if rng.gen_bool(0.5) { 1 } else { -1 };
        
        self.send_mouse_move(dx * direction, 0);
        std::thread::sleep(Duration::from_millis(rng.gen_range(15..35)));
        self.send_mouse_move(-dx * direction, 0);
    }

    #[cfg(target_os = "windows")]
    fn mouse_jiggle_diagonal(&self, rng: &mut impl Rng) {
        let dx = rng.gen_range(MIN_MOUSE_MOVE..=MAX_MOUSE_MOVE);
        let dy = rng.gen_range(1..=2);
        let dir_x = if rng.gen_bool(0.5) { 1 } else { -1 };
        let dir_y = if rng.gen_bool(0.5) { 1 } else { -1 };
        
        self.send_mouse_move(dx * dir_x, dy * dir_y);
        std::thread::sleep(Duration::from_millis(rng.gen_range(15..35)));
        self.send_mouse_move(-dx * dir_x, -dy * dir_y);
    }

    #[cfg(target_os = "windows")]
    fn send_mouse_move(&self, dx: i32, dy: i32) {
        unsafe {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
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

    /// tap F13 (invisible key, doesn't exist on most keyboards)
    #[cfg(target_os = "windows")]
    fn keyboard_tap(&self) {
        unsafe {
            let key_down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(VK_F13),
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            let key_up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(VK_F13),
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            
            SendInput(&[key_down], std::mem::size_of::<INPUT>() as i32);
            std::thread::sleep(Duration::from_millis(10));
            SendInput(&[key_up], std::mem::size_of::<INPUT>() as i32);
        }
    }


}
