use std::sync::Arc;
use crate::config::Config;
use crate::core::Memory;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache};
use crate::utils::math::{Vector2, Vector3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBone {
    Head = 0,
    UpperTorso = 1,
    LowerTorso = 2,
    HumanoidRootPart = 3,
    LeftHand = 4,
    RightHand = 5,
    LeftFoot = 6,
    RightFoot = 7,
}

impl TargetBone {
    pub fn to_body_part(self) -> BodyPart {
        match self {
            Self::Head => BodyPart::Head,
            _ => BodyPart::Torso,
        }
    }

    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::Head,
            1 => Self::UpperTorso,
            2 => Self::LowerTorso,
            3 => Self::HumanoidRootPart,
            4 => Self::LeftHand,
            5 => Self::RightHand,
            6 => Self::LeftFoot,
            7 => Self::RightFoot,
            _ => Self::Head,
        }
    }
}

#[derive(Clone, Default)]
pub struct SilentAimDebug {
    pub is_active: bool,
    pub has_target: bool,
    pub target_name: String,
    pub target_health: f32,
    pub target_distance: f32,
    pub selected_part: String,
    pub screen_pos: Vector2,
    pub spoof_pos: Vector2,
    pub is_spoofing: bool,
    pub velocity: Vector3,
}

pub struct SilentAim {
    pub debug: SilentAimDebug,
}

impl SilentAim {
    pub fn new(_memory: Arc<Memory>, _cache: Arc<Cache>, _visengine: Arc<VisualEngine>) -> Self {
        Self {
            debug: SilentAimDebug::default(),
        }
    }

    pub fn get_current_target_name(&self) -> Option<&str> { None }
    pub fn is_locked(&self) -> bool { false }
    pub fn unlock_target(&mut self) {}
    pub fn update(&mut self, _config: &Config, _local_player_name: &str) {}

    pub fn render_debug(&self, _ui: &mut eframe::egui::Ui) {}
}

impl Drop for SilentAim {
    fn drop(&mut self) {}
}
