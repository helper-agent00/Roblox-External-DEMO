use rand::Rng;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::AutoClickerConfig;
use crate::utils::input::Input;

/// Minimum delay between clicks in milliseconds.
const MIN_CLICK_DELAY_MS: u64 = 10;
/// Maximum buttons that can be in a sequence.
pub const MAX_SEQUENCE_BUTTONS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickButton {
    LeftMouse,
    RightMouse,
    MiddleMouse,
    Key(i32),
}

impl ClickButton {
    #[allow(dead_code)]
    pub fn vk_code(&self) -> i32 {
        match self {
            ClickButton::LeftMouse => 0x01,  // VK_LBUTTON
            ClickButton::RightMouse => 0x02, // VK_RBUTTON
            ClickButton::MiddleMouse => 0x04, // VK_MBUTTON
            ClickButton::Key(code) => *code,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            ClickButton::LeftMouse => "LMB".to_string(),
            ClickButton::RightMouse => "RMB".to_string(),
            ClickButton::MiddleMouse => "MMB".to_string(),
            ClickButton::Key(code) => {
                match *code {
                    0x41..=0x5A => format!("{}", ((*code - 0x41) as u8 + b'A') as char),
                    0x30..=0x39 => format!("{}", (*code - 0x30) as u8),
                    0x20 => "Space".to_string(),
                    0x0D => "Enter".to_string(),
                    0x09 => "Tab".to_string(),
                    0x10 => "Shift".to_string(),
                    0x11 => "Ctrl".to_string(),
                    0x12 => "Alt".to_string(),
                    0x1B => "Esc".to_string(),
                    _ => format!("0x{:02X}", code),
                }
            }
        }
    }

    pub fn is_mouse(&self) -> bool {
        matches!(self, ClickButton::LeftMouse | ClickButton::RightMouse | ClickButton::MiddleMouse)
    }
}

#[derive(Default)]
pub struct AutoClickerState {
    /// Buttons in the click sequence.
    pub sequence: Vec<ClickButton>,
    /// Whether recording new buttons.
    pub recording: bool,
    /// Current index in the sequence (for display).
    pub current_index: usize,
    /// Total clicks performed this session.
    pub total_clicks: u64,
    /// Last recorded key for debounce.
    last_recorded_key: Option<i32>,
}

pub struct AutoClicker {
    running: Arc<AtomicBool>,
    state: Arc<Mutex<AutoClickerState>>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl Default for AutoClicker {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoClicker {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(AutoClickerState::default())),
            worker_handle: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> &Arc<Mutex<AutoClickerState>> {
        &self.state
    }

    pub fn toggle(&mut self, config: &AutoClickerConfig) {
        if self.is_running() {
            self.stop();
        } else {
            self.start(config);
        }
    }

    pub fn start(&mut self, config: &AutoClickerConfig) {
        if self.is_running() {
            return;
        }

        let sequence = {
            let state = self.state.lock().unwrap();
            if state.sequence.is_empty() {
                tracing::warn!("AutoClicker: No buttons in sequence");
                return;
            }
            state.sequence.clone()
        };

        self.running.store(true, Ordering::SeqCst);
        
        let running = Arc::clone(&self.running);
        let state = Arc::clone(&self.state);
        let turbo_mode = config.turbo_mode;
        let base_delay_ms = if turbo_mode { 1 } else { config.delay_ms.max(MIN_CLICK_DELAY_MS as f32) as u64 };
        let variance_percent = if turbo_mode { 0.0 } else { config.variance_percent.clamp(0.0, 50.0) };
        let hold_duration_ms = if turbo_mode { 1 } else { config.hold_duration_ms.max(10.0) as u64 };

        self.worker_handle = Some(thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let mut sequence_index = 0;

            tracing::info!("AutoClicker started with {} buttons", sequence.len());

            while running.load(Ordering::SeqCst) {
                let button = &sequence[sequence_index];

                // Update state for UI
                {
                    let mut state = state.lock().unwrap();
                    state.current_index = sequence_index;
                    state.total_clicks += 1;
                }

                Self::perform_click(button, hold_duration_ms);

                sequence_index = (sequence_index + 1) % sequence.len();

                let variance = if variance_percent > 0.0 {
                    let variance_range = (base_delay_ms as f32 * variance_percent / 100.0) as i64;
                    if variance_range > 0 {
                        rng.gen_range(-variance_range..=variance_range)
                    } else {
                        0
                    }
                } else {
                    0
                };

                let actual_delay = if turbo_mode {
                    0
                } else {
                    (base_delay_ms as i64 + variance).max(MIN_CLICK_DELAY_MS as i64) as u64
                };
                
                if actual_delay > 0 {
                    thread::sleep(Duration::from_millis(actual_delay));
                } else {
                    // Turbo mode: use microsecond sleep for ~2000-5000 clicks/sec
                    // This is extremely fast but won't freeze the system
                    thread::sleep(Duration::from_micros(200));
                }
            }

            tracing::info!("AutoClicker stopped");
        }));
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }

    fn perform_click(button: &ClickButton, hold_ms: u64) {
        if button.is_mouse() {
            Self::perform_mouse_click(button, hold_ms);
        } else {
            Self::perform_key_press(button, hold_ms);
        }
    }

    fn perform_mouse_click(button: &ClickButton, hold_ms: u64) {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
                MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
                MOUSEEVENTF_RIGHTUP,
            };

            let (down_flag, up_flag) = match button {
                ClickButton::LeftMouse => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                ClickButton::RightMouse => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                ClickButton::MiddleMouse => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                _ => return,
            };

            mouse_event(down_flag, 0, 0, 0, 0);

            thread::sleep(Duration::from_millis(hold_ms));

            mouse_event(up_flag, 0, 0, 0, 0);
        }
    }

    fn perform_key_press(button: &ClickButton, hold_ms: u64) {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                keybd_event, KEYEVENTF_KEYUP,
            };

            let vk = match button {
                ClickButton::Key(code) => *code as u8,
                _ => return,
            };

            keybd_event(vk, 0, Default::default(), 0);

            thread::sleep(Duration::from_millis(hold_ms));

            keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        }
    }

    pub fn start_recording(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.recording = true;
        state.last_recorded_key = None;
        tracing::info!("AutoClicker: Started recording buttons");
    }

    pub fn stop_recording(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.recording = false;
        state.last_recorded_key = None;
        tracing::info!("AutoClicker: Stopped recording, {} buttons in sequence", state.sequence.len());
    }

    pub fn update_recording(&mut self) {
        let mut state = self.state.lock().unwrap();
        if !state.recording {
            return;
        }

        if state.sequence.len() >= MAX_SEQUENCE_BUTTONS {
            state.recording = false;
            state.last_recorded_key = None;
            return;
        }

        // Check mouse buttons
        if Input::is_key_down(0x01) {
            if state.last_recorded_key != Some(0x01) {
                state.sequence.push(ClickButton::LeftMouse);
                state.last_recorded_key = Some(0x01);
                tracing::debug!("Recorded: Left Mouse");
            }
        } else if Input::is_key_down(0x02) {
            if state.last_recorded_key != Some(0x02) {
                state.sequence.push(ClickButton::RightMouse);
                state.last_recorded_key = Some(0x02);
                tracing::debug!("Recorded: Right Mouse");
            }
        } else if Input::is_key_down(0x04) {
            if state.last_recorded_key != Some(0x04) {
                state.sequence.push(ClickButton::MiddleMouse);
                state.last_recorded_key = Some(0x04);
                tracing::debug!("Recorded: Middle Mouse");
            }
        } else {
            let keys_to_check: Vec<i32> = (0x41..=0x5A)
                .chain(0x30..=0x39)
                .chain([0x20, 0x0D, 0x09, 0x10, 0x11, 0x12])
                .collect();

            for key in keys_to_check {
                if Input::is_key_down(key) {
                    if state.last_recorded_key != Some(key) {
                        state.sequence.push(ClickButton::Key(key));
                        state.last_recorded_key = Some(key);
                        tracing::debug!("Recorded: Key 0x{:02X}", key);
                    }
                    break;
                }
            }

            // Reset last recorded when no key pressed
            if !Input::is_key_down(state.last_recorded_key.unwrap_or(0)) {
                state.last_recorded_key = None;
            }
        }
    }

    pub fn clear_sequence(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.sequence.clear();
        state.current_index = 0;
        state.total_clicks = 0;
        tracing::info!("AutoClicker: Sequence cleared");
    }

    pub fn remove_last(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.sequence.pop();
        if state.current_index >= state.sequence.len() && !state.sequence.is_empty() {
            state.current_index = state.sequence.len() - 1;
        }
    }
}

impl Drop for AutoClicker {
    fn drop(&mut self) {
        self.stop();
    }
}
