#![allow(dead_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::value_base;
use crate::sdk::Instance;

pub struct AutoReload {
    memory: Option<Arc<Memory>>,
    players_instance: Option<Arc<Instance>>,
    local_player_name: String,
    last_reload_time: Instant,
    reload_cooldown: Duration,
    last_debug_time: Instant,
}

impl Default for AutoReload {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoReload {
    pub fn new() -> Self {
        Self {
            memory: None,
            players_instance: None,
            local_player_name: String::new(),
            last_reload_time: Instant::now(),
            reload_cooldown: Duration::from_millis(300),
            last_debug_time: Instant::now(),
        }
    }

    pub fn init(&mut self, memory: Arc<Memory>, players_instance: Arc<Instance>, local_player_name: String) {
        self.memory = Some(memory);
        self.players_instance = Some(players_instance);
        self.local_player_name = local_player_name;
    }

    pub fn update(&mut self, enabled: bool) {
        if !enabled {
            return;
        }

        if self.last_reload_time.elapsed() < self.reload_cooldown {
            return;
        }

        let Some(memory) = &self.memory else { return };
        let Some(players_instance) = &self.players_instance else { return };

        let local_player = players_instance
            .get_children()
            .into_iter()
            .find(|child| child.get_name().eq_ignore_ascii_case(&self.local_player_name));

        let Some(local_player) = local_player else { return };

        // Get character (ModelInstance)
        let character_addr = memory.read::<u64>(local_player.address + crate::core::offsets::player::model_instance());
        if !is_valid_address(character_addr) {
            return;
        }
        let character = Instance::new(character_addr, Arc::clone(memory));

        let tool = character
            .get_children()
            .into_iter()
            .find(|child| {
                let class = child.get_class_name();
                class == "Tool" || class == "HopperBin"
            });

        let Some(tool) = tool else { 
            return; 
        };

        let ammo_names = ["Ammo", "ammo", "CurrentAmmo", "Clip", "Bullets", "Magazine"];
        let ammo = ammo_names.iter()
            .find_map(|name| tool.find_first_child(name));

        let Some(ammo) = ammo else { 
            return; 
        };

        let value_offset = value_base::value();
        let ammo_value = memory.read::<i32>(ammo.address + value_offset);

        if ammo_value == 0 {
            let mut is_reloading = false;
            if let Some(body_effects) = character.find_first_child("BodyEffects") {
                if let Some(reload_value) = body_effects.find_first_child("Reload") {
                    is_reloading = memory.read::<u8>(reload_value.address + value_offset) != 0;
                }
            }

            if !is_reloading {
                Self::press_reload_key();
                self.last_reload_time = Instant::now();
            }
        }
    }

    fn press_reload_key() {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                keybd_event, MapVirtualKeyW, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC,
            };

            let vk_r = 0x52;
            let scan_code = MapVirtualKeyW(vk_r, MAPVK_VK_TO_VSC) as u8;
            
            keybd_event(0, scan_code, KEYEVENTF_SCANCODE, 0);
            std::thread::sleep(Duration::from_millis(10));
            keybd_event(0, scan_code, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, 0);
        }
    }
}
