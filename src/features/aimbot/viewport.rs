#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::camera;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache};
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{VelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::{TargetContext, get_bone_with_fallback, compute_priority};

/// Camera Viewport offset (Vector2int16)
const VIEWPORT_OFFSET: u64 = 0x2AC;

#[inline]
fn get_viewport_offset() -> u64 {
    VIEWPORT_OFFSET
}

const MIN_TARGET_DISTANCE: f32 = 1.0;

/// Maximum distance for viewport aim (studs)
const MAX_TARGET_DISTANCE: f32 = 1000.0;

const MAX_VERIFY_RETRIES: u32 = 2;

/// Roblox Vector2int16 - two 16-bit integers packed
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector2int16 {
    pub x: i16,
    pub y: i16,
}

impl Vector2int16 {
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }
    
    /// Calculate viewport value to make hit detection point at target
    pub fn calculate_for_target(target_screen: Vector2, screen_size: Vector2) -> Self {
        Self {
            x: (2.0 * (screen_size.x - target_screen.x)) as i16,
            y: (2.0 * (screen_size.y - target_screen.y)) as i16,
        }
    }
    
    /// Create viewport from screen dimensions (normal/reset value)
    pub fn from_screen_size(width: f32, height: f32) -> Self {
        Self {
            x: width as i16,
            y: height as i16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBone {
    Head = 0,
    UpperTorso = 1,
    LowerTorso = 2,
    HumanoidRootPart = 3,
}

impl TargetBone {
    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::Head,
            1 => Self::UpperTorso,
            2 => Self::LowerTorso,
            _ => Self::HumanoidRootPart,
        }
    }
    
    pub fn to_body_part(self) -> BodyPart {
        match self {
            Self::Head => BodyPart::Head,
            Self::UpperTorso => BodyPart::UpperTorso,
            Self::LowerTorso => BodyPart::LowerTorso,
            Self::HumanoidRootPart => BodyPart::HumanoidRootPart,
        }
    }
}

/// Viewport-based silent aim.
///
/// Manipulates Camera.Viewport to redirect hit detection without
/// visually moving the camera.
pub struct ViewportAim {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    
    // State
    is_spoofing: bool,
    camera_addr: u64,
    
    // Tracking
    last_mouse_state: bool,
    velocity_trackers: HashMap<u64, VelocityTracker>,
    
    // Stats
    shot_count: u32,
    last_target_addr: u64,
    write_verify_failures: u32,
    
    // Current target name for highlight (single source of truth)
    current_target_name: Option<String>,
}

impl ViewportAim {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        Self {
            memory,
            cache,
            visengine,
            is_spoofing: false,
            camera_addr: 0,
            last_mouse_state: false,
            velocity_trackers: HashMap::new(),
            shot_count: 0,
            last_target_addr: 0,
            write_verify_failures: 0,
            current_target_name: None,
        }
    }
    
    fn read_viewport(&self, camera_addr: u64) -> Vector2int16 {
        let addr = camera_addr + get_viewport_offset();
        if is_valid_address(addr) {
            let packed = self.memory.read::<u32>(addr);
            Vector2int16 {
                x: (packed & 0xFFFF) as i16,
                y: ((packed >> 16) & 0xFFFF) as i16,
            }
        } else {
            Vector2int16::default()
        }
    }
    
    /// Write viewport with verification
    fn write_viewport(&mut self, camera_addr: u64, viewport: Vector2int16) {
        let addr = camera_addr + get_viewport_offset();
        if is_valid_address(addr) {
            let vx = viewport.x;
            let vy = viewport.y;
            let packed: u32 = (vx as u16 as u32) | ((vy as u16 as u32) << 16);
            self.memory.write(addr, packed);
            
            // PRO: Write verification
            let readback = self.memory.read::<u32>(addr);
            if readback != packed {
                self.write_verify_failures += 1;
                // Retry writes
                for _ in 0..MAX_VERIFY_RETRIES {
                    self.memory.write(addr, packed);
                    let check = self.memory.read::<u32>(addr);
                    if check == packed {
                        break;
                    }
                }
            }
        }
    }
    
    fn reset_viewport(&mut self) {
        if self.is_spoofing && self.camera_addr != 0 {
            let dims = self.visengine.get_dimensions();
            let normal_viewport = Vector2int16::from_screen_size(dims.x, dims.y);
            self.write_viewport(self.camera_addr, normal_viewport);
            self.is_spoofing = false;
        }
    }
    
    fn get_camera_position(&self, camera_addr: u64) -> Vector3 {
        let pos_addr = camera_addr + camera::position();
        if is_valid_address(pos_addr) {
            self.memory.read::<Vector3>(pos_addr)
        } else {
            Vector3::ZERO
        }
    }
    
    fn find_target(
        &mut self,
        config: &Config,
        camera_pos: Vector3,
        screen_center: Vector2,
        dims: Vector2,
        local_player_name: &str,
    ) -> Option<(u64, Vector3, Vector2, String)> {
        let ctx = TargetContext::build(&self.cache, &self.visengine, config, local_player_name)?;
        let target_bone = TargetBone::from_index(config.viewport_aim.target_bone);
        let view_matrix = self.visengine.get_view_matrix();

        let mut best_target: Option<(u64, Vector3, Vector2, f32, String)> = None;

        for entity in ctx.snapshot.iter() {
            if ctx.should_skip(entity, local_player_name, config.visuals.team_check, true) {
                continue;
            }

            // Get target bone position with fallback
            let bone_pos = match get_bone_with_fallback(entity, target_bone.to_body_part().to_name()) {
                Some(pos) => pos,
                None => continue,
            };

            // Distance check
            let distance = camera_pos.distance_to(bone_pos);
            if distance < MIN_TARGET_DISTANCE || distance > MAX_TARGET_DISTANCE {
                continue;
            }

            // World to screen
            let screen_pos = match self.visengine.world_to_screen(bone_pos, dims, &view_matrix) {
                Some(pos) => pos,
                None => continue,
            };

            let dist_to_crosshair = screen_pos.distance_to(screen_center);

            // FOV check
            if config.viewport_aim.use_fov && dist_to_crosshair > config.viewport_aim.fov {
                continue;
            }

            let priority = compute_priority(entity, dist_to_crosshair, distance);

            if best_target.is_none() || priority < best_target.as_ref().unwrap().3 {
                let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                best_target = Some((target_key, bone_pos, screen_pos, priority, entity.name.clone()));
            }
        }

        best_target.map(|(addr, pos, screen, _, name)| (addr, pos, screen, name))
    }
    
    fn predict_position(&mut self, target_addr: u64, current_pos: Vector3, config: &Config) -> Vector3 {
        if !config.aimbot.prediction_enabled {
            return current_pos;
        }
        
        let tracker = self.velocity_trackers
            .entry(target_addr)
            .or_insert_with(|| VelocityTracker::new(current_pos));
        
        let (velocity, _) = tracker.update(current_pos);
        
        if velocity.length() < VELOCITY_THRESHOLD {
            return current_pos;
        }
        
        // Use global aimbot prediction amount
        let time_ahead = config.aimbot.prediction_amount;
        tracker.predict(current_pos, time_ahead)
    }
    
    /// Main update - call every frame.
    /// Requires camera_addr from the caller.
    pub fn update(&mut self, config: &Config, local_player_name: &str, camera_addr: u64) {
        if !config.viewport_aim.enabled {
            if self.is_spoofing {
                self.reset_viewport();
            }
            self.current_target_name = None;
            return;
        }
        
        if !is_valid_address(camera_addr) {
            if self.is_spoofing {
                self.reset_viewport();
            }
            return;
        }
        
        self.camera_addr = camera_addr;
        
        let mouse_down = Input::is_key_down(0x01);
        
        // Reset on mouse release
        if !mouse_down && self.is_spoofing {
            self.reset_viewport();
            self.last_mouse_state = false;
            return;
        }
        
        if !mouse_down {
            self.last_mouse_state = false;
            return;
        }
        
        // Get screen info
        let dims = self.visengine.get_dimensions();
        if dims.x <= 0.0 || dims.y <= 0.0 {
            return;
        }
        
        let screen_center = Vector2::new(dims.x / 2.0, dims.y / 2.0);
        let camera_pos = self.get_camera_position(camera_addr);
        let view_matrix = self.visengine.get_view_matrix();
        
        // Find target
        let target = self.find_target(config, camera_pos, screen_center, dims, local_player_name);
        
        let (target_addr, target_pos, _target_screen, target_name) = match target {
            Some(t) => t,
            None => {
                self.current_target_name = None;
                self.reset_viewport();
                return;
            }
        };
        
        self.current_target_name = Some(target_name);
        
        // Apply prediction
        let predicted_pos = self.predict_position(target_addr, target_pos, config);
        
        // Convert predicted position to screen
        let predicted_screen = match self.visengine.world_to_screen(predicted_pos, dims, &view_matrix) {
            Some(pos) => pos,
            None => {
                self.reset_viewport();
                return;
            }
        };
        
        // Calculate spoofed viewport
        let spoofed_viewport = Vector2int16::calculate_for_target(
            predicted_screen,
            dims,
        );
        
        // Write spoofed viewport
        self.write_viewport(camera_addr, spoofed_viewport);
        self.is_spoofing = true;
        self.last_target_addr = target_addr;
        
        // Track shots
        let shot_initiated = mouse_down && !self.last_mouse_state;
        if shot_initiated {
            self.shot_count += 1;
        }
        self.last_mouse_state = mouse_down;
        
        // Note: viewport stays spoofed during mouse hold, resets on release.
        // Some games may need immediate reset (add config option if needed).
    }
    
    pub fn get_shot_count(&self) -> u32 {
        self.shot_count
    }
    
    pub fn is_active(&self) -> bool {
        self.is_spoofing
    }
    
    pub fn get_current_target_name(&self) -> Option<&str> {
        self.current_target_name.as_deref()
    }
    
    pub fn cleanup_trackers(&mut self) {
        let snapshot = self.cache.get_snapshot();
        let valid_addrs: std::collections::HashSet<u64> = snapshot.iter()
            .map(|e| if e.humanoid_address != 0 { e.humanoid_address } else { e.model_address })
            .collect();
        self.velocity_trackers.retain(|addr, _| valid_addrs.contains(addr));
    }
}
