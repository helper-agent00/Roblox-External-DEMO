use std::sync::{Arc, Mutex};
use crate::config::BladeBallConfig;
use crate::core::Memory;
use crate::sdk::Instance;
use crate::utils::math::Vector3;

#[derive(Clone, Debug)]
pub struct BladeBallDebugState {
    pub ball_found: bool,
    pub ball_pos: Vector3,
    pub distance: f32,
    pub time_to_impact: f32,
    pub highlight_detected: bool,
    pub root_is_red: bool,
    pub is_targeted: bool,
    pub local_model_address: u64,
    pub last_parry_ago_ms: u64,
    pub parry_reason: String,
    pub ball_speed: f32,
    pub showdown_active: bool,
    pub in_alive_folder: bool,
    pub player_speed: f32,
    pub closing_speed: f32,
    pub ball_aimed_at_us: bool,
    pub target_character_active: bool,
    pub incoming_target_detected: bool,
    pub alive_count: usize,
}

impl Default for BladeBallDebugState {
    fn default() -> Self {
        Self {
            ball_found: false,
            ball_pos: Vector3::default(),
            distance: 0.0,
            time_to_impact: 0.0,
            highlight_detected: false,
            root_is_red: false,
            is_targeted: false,
            local_model_address: 0,
            last_parry_ago_ms: 0,
            parry_reason: String::new(),
            ball_speed: 0.0,
            showdown_active: false,
            in_alive_folder: false,
            player_speed: 0.0,
            closing_speed: 0.0,
            ball_aimed_at_us: false,
            target_character_active: false,
            incoming_target_detected: false,
            alive_count: 0,
        }
    }
}

pub struct BladeBallAutoParry {
    debug_state: Arc<Mutex<BladeBallDebugState>>,
}

impl BladeBallAutoParry {
    pub fn new(_memory: Arc<Memory>) -> Self {
        Self {
            debug_state: Arc::new(Mutex::new(BladeBallDebugState::default())),
        }
    }

    pub fn get_debug_state(&self) -> Arc<Mutex<BladeBallDebugState>> {
        Arc::clone(&self.debug_state)
    }

    pub fn update(
        &mut self,
        _config: &BladeBallConfig,
        _workspace: &Instance,
        _local_root_pos: Option<Vector3>,
        _local_name: &str,
        _local_model_addr: u64,
    ) {}

    pub fn clear_cache(&mut self) {}
}
