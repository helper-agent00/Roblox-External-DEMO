#![allow(dead_code)]

use std::sync::Arc;

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{humanoid, player};
use crate::sdk::Instance;

/// Represents a Roblox Player object.
pub struct Player {
    instance: Instance,
}

impl Player {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self {
            instance: Instance::new(address, memory),
        }
    }

    #[inline]
    pub fn address(&self) -> u64 {
        self.instance.address
    }

    pub fn get_name(&self) -> String {
        self.instance.get_name()
    }

    pub fn get_display_name(&self) -> String {
        let ptr = self.instance.memory().read::<u64>(self.instance.address + player::display_name());
        if is_valid_address(ptr) {
            self.instance.memory().read_string(ptr)
        } else {
            self.get_name()
        }
    }

    pub fn get_user_id(&self) -> i64 {
        self.instance.memory().read::<i64>(self.instance.address + player::user_id())
    }

    pub fn get_model_instance(&self) -> ModelInstance {
        let model_addr = self.instance.memory().read::<u64>(self.instance.address + player::model_instance());
        ModelInstance::new(model_addr, Arc::clone(self.instance.memory()))
    }

    /// BrickColor from Player.TeamColor — same value => same team.
    pub fn get_team_color(&self) -> u32 {
        self.instance.memory().read::<u32>(self.instance.address + player::team_color())
    }

    /// Often 0 in games without Teams service.
    /// Team check now uses whitelist system — see config.visuals.teammate_whitelist.
    pub fn get_team_address(&self) -> u64 {
        let team_offset = player::team();
        self.instance.memory().read::<u64>(self.instance.address + team_offset)
    }
}

/// Represents a character model (Humanoid's parent).
pub struct ModelInstance {
    instance: Instance,
}

impl ModelInstance {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self {
            instance: Instance::new(address, memory),
        }
    }

    #[inline]
    pub fn address(&self) -> u64 {
        self.instance.address
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.instance.is_valid()
    }

    pub fn get_children(&self) -> Vec<Instance> {
        self.instance.get_children()
    }

    pub fn find_first_child(&self, name: &str) -> Option<Instance> {
        self.instance.find_first_child(name)
    }

    pub fn get_humanoid(&self) -> Option<Humanoid> {
        self.instance
            .find_first_child("Humanoid")
            .map(|h| Humanoid::new(h.address, Arc::clone(self.instance.memory())))
    }

    pub fn get_primary_part(&self) -> Option<Instance> {
        self.instance.find_first_child("HumanoidRootPart")
    }
}

/// Represents a Humanoid (character controller).
pub struct Humanoid {
    pub address: u64,
    memory: Arc<Memory>,
}

impl Humanoid {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self { address, memory }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        is_valid_address(self.address)
    }

    /// Get rig type: 0 = R6, 1 = R15.
    pub fn get_rig_type(&self) -> u8 {
        self.memory.read::<u8>(self.address + humanoid::rig_type())
    }

    pub fn get_health(&self) -> f32 {
        self.memory.read::<f32>(self.address + humanoid::health())
    }

    pub fn get_max_health(&self) -> f32 {
        self.memory.read::<f32>(self.address + humanoid::max_health())
    }

    pub fn get_walk_speed(&self) -> f32 {
        self.memory.read::<f32>(self.address + humanoid::walkspeed())
    }

    pub fn set_walkspeed(&self, speed: f32) {
        self.memory.write::<f32>(self.address + humanoid::walkspeed(), speed);
        self.memory.write::<f32>(self.address + humanoid::walkspeed_check(), speed);
    }

    pub fn get_jump_power(&self) -> f32 {
        self.memory.read::<f32>(self.address + humanoid::jump_power())
    }

    pub fn set_jump_power(&self, power: f32) {
        self.memory.write::<f32>(self.address + humanoid::jump_power(), power);
        self.memory.write::<f32>(self.address + humanoid::jump_height(), power * 0.15);
    }

    pub fn get_floor_material(&self) -> i32 {
        self.memory.read::<i32>(self.address + humanoid::floor_material())
    }

    pub fn is_on_ground(&self) -> bool {
        self.get_floor_material() != 0
    }

    pub fn jump(&self) {
        self.memory.write::<u8>(self.address + humanoid::jump(), 1);
    }
}
