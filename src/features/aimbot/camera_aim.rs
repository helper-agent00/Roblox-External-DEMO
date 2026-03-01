#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::memory::Memory;
use crate::core::offsets::camera;
use crate::sdk::VisualEngine;
use crate::utils::cache::Cache;
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{VelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::{TargetContext, get_bone_with_fallback, compute_priority};

const SPOOF_WRITE_CYCLES: u32 = 7;
const CONTINUOUS_WRITE_CYCLES: u32 = 4;

/// Cooldown between shots (ms)
const SHOT_COOLDOWN_MS: u64 = 10;

/// Minimum distance to target (studs)
const MIN_TARGET_DISTANCE: f32 = 1.0;

/// Maximum distance for silent aim (studs)
const MAX_TARGET_DISTANCE: f32 = 1000.0;

/// Rotation interpolation factor (0 = snap to target, 1 = full lag)
/// Using 0 to snap directly - interpolation causes missed shots
const ROTATION_INTERP_SPEED: f32 = 0.0;

/// Maximum write verification retries
const MAX_VERIFY_RETRIES: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub struct CFrame {
    pub rotation: [f32; 9],
    /// Position: [x, y, z]
    pub position: Vector3,
}

impl CFrame {
    pub fn identity(pos: Vector3) -> Self {
        Self {
            rotation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            position: pos,
        }
    }

    /// Normalize a vector
    #[inline]
    fn normalize(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let len = (x * x + y * y + z * z).sqrt();
        if len > 0.0001 {
            (x / len, y / len, z / len)
        } else {
            (x, y, z)
        }
    }

    /// Create CFrame looking from eye to target.
    pub fn look_at(from: Vector3, to: Vector3) -> Self {
        // LookVector = direction from 'from' to 'to' (normalized)
        let look_x = to.x - from.x;
        let look_y = to.y - from.y;
        let look_z = to.z - from.z;
        
        let look_len = (look_x * look_x + look_y * look_y + look_z * look_z).sqrt();
        if look_len < 0.0001 {
            return Self {
                rotation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                position: from,
            };
        }
        
        // Normalized LookVector (forward)
        let lx = look_x / look_len;
        let ly = look_y / look_len;
        let lz = look_z / look_len;

        // World up vector
        let world_up = (0.0f32, 1.0f32, 0.0f32);
        
        // RightVector = cross(LookVector, worldUp), then normalize
        let mut rx = ly * world_up.2 - lz * world_up.1;
        let mut ry = lz * world_up.0 - lx * world_up.2;
        let mut rz = lx * world_up.1 - ly * world_up.0;
        
        let r_len = (rx * rx + ry * ry + rz * rz).sqrt();
        if r_len < 0.0001 {
            // Looking straight up or down, use world X as right
            rx = 1.0;
            ry = 0.0;
            rz = 0.0;
        } else {
            rx /= r_len;
            ry /= r_len;
            rz /= r_len;
        }

        // UpVector = cross(RightVector, LookVector)
        let ux = ry * lz - rz * ly;
        let uy = rz * lx - rx * lz;
        let uz = rx * ly - ry * lx;

        // ZVector = -LookVector (stored in column 2)
        let zx = -lx;
        let zy = -ly;
        let zz = -lz;

        // ROW-MAJOR storage: R00, R01, R02, R10, R11, R12, R20, R21, R22
        // Row 0: [Right.x, Up.x, Z.x]
        // Row 1: [Right.y, Up.y, Z.y]  
        // Row 2: [Right.z, Up.z, Z.z]
        Self {
            rotation: [
                rx, ux, zx,  // Row 0: R00, R01, R02
                ry, uy, zy,  // Row 1: R10, R11, R12
                rz, uz, zz,  // Row 2: R20, R21, R22
            ],
            position: from,
        }
    }

    /// Validate CFrame for NaN/Inf values
    pub fn is_valid(&self) -> bool {
        self.position.x.is_finite() &&
        self.position.y.is_finite() &&
        self.position.z.is_finite()
    }
}

pub struct CameraAim {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    base_address: u64,
    // state tracking
    last_shot_time: Instant,
    last_mouse_state: bool,
    is_spoofing: bool,
    // continuous spoof state
    original_cframe: Option<CFrame>,
    spoof_camera_addr: u64,
    // interpolated rotation state
    current_interp_rotation: Option<[f32; 9]>,
    
    // statistics
    shot_count: u32,
    hit_count: u32,
    write_verify_failures: u32,
    
    // velocity tracking per target
    velocity_trackers: std::collections::HashMap<u64, VelocityTracker>,
    
    // last target for hit detection
    last_target_addr: u64,
    last_target_health: f32,
    
    // current target name for highlight
    current_target_name: Option<String>,
}

impl CameraAim {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        let base_address = memory.base_address();
        Self {
            memory,
            cache,
            visengine,
            base_address,
            last_shot_time: Instant::now(),
            last_mouse_state: false,
            is_spoofing: false,
            original_cframe: None,
            spoof_camera_addr: 0,
            current_interp_rotation: None,
            shot_count: 0,
            hit_count: 0,
            write_verify_failures: 0,
            velocity_trackers: std::collections::HashMap::new(),
            last_target_addr: 0,
            last_target_health: 0.0,
            current_target_name: None,
        }
    }
    
    pub fn get_hit_rate(&self) -> f32 {
        if self.shot_count == 0 { 0.0 }
        else { (self.hit_count as f32 / self.shot_count as f32) * 100.0 }
    }
    
    pub fn get_current_target_name(&self) -> Option<&str> {
        self.current_target_name.as_deref()
    }

    pub fn update(&mut self, config: &Config, local_player_name: &str) {
        if !config.camera_aim.enabled {
            if self.is_spoofing {
                self.restore_original_cframe();
            }
            return;
        }

        let mouse_down = Input::is_key_down(0x01);
        
        if !mouse_down && self.is_spoofing {
            self.restore_original_cframe();
            self.last_mouse_state = false;
            return;
        }
        
        // Not holding mouse
        if !mouse_down {
            self.last_mouse_state = false;
            return;
        }
        
        // Detect click edge for shot counting
        let shot_initiated = mouse_down && !self.last_mouse_state;
        self.last_mouse_state = mouse_down;

        // Get camera address
        let camera_addr = match self.get_camera_address() {
            Some(addr) => addr,
            None => {
                if self.is_spoofing {
                    self.restore_original_cframe();
                }
                return;
            }
        };

        // Get camera position (eye)
        let camera_pos = self.read_camera_position(camera_addr);
        if camera_pos.is_near_origin(1.0) {
            if self.is_spoofing {
                self.restore_original_cframe();
            }
            return;
        }

        // Find best target
        let target_pos = match self.find_best_target(config, local_player_name, &camera_pos) {
            Some(pos) => pos,
            None => {
                self.current_target_name = None;
                // No target - restore if we were spoofing
                if self.is_spoofing {
                    self.restore_original_cframe();
                }
                return;
            }
        };

        // Validate target distance
        let target_distance = camera_pos.distance_to(target_pos);
        if target_distance < MIN_TARGET_DISTANCE {
            if self.is_spoofing {
                self.restore_original_cframe();
            }
            return;
        }
        
        // Continuous spoof while holding
        match self.handle_continuous_spoof(camera_addr, camera_pos, target_pos, shot_initiated) {
            Ok(_) => {
                if shot_initiated {
                    self.last_shot_time = Instant::now();
                    self.shot_count += 1;
                }
            }
            Err(_e) => {
                // Spoof failed silently
            }
        }
    }
    
    /// Restore original CFrame when done spoofing
    fn restore_original_cframe(&mut self) {
        if let Some(original) = self.original_cframe.take() {
            if self.spoof_camera_addr != 0 {
                let rot_base = self.spoof_camera_addr + camera::rotation();
                self.memory.write::<[f32; 9]>(rot_base, original.rotation);
            }
        }
        self.is_spoofing = false;
        self.spoof_camera_addr = 0;
        self.current_interp_rotation = None;
    }

    /// Handle continuous spoof while holding mouse
    fn handle_continuous_spoof(
        &mut self,
        camera_addr: u64,
        camera_pos: Vector3,
        target_pos: Vector3,
        is_new_shot: bool,
    ) -> Result<(), String> {
        let rot_base = camera_addr + camera::rotation();
        
        // first time spoofing - save original CFrame
        if !self.is_spoofing {
            let original = self.read_cframe(camera_addr);
            
            if !original.is_valid() {
                return Err("Invalid original CFrame (contains NaN/Inf)".to_string());
            }
            
            self.original_cframe = Some(original);
            self.spoof_camera_addr = camera_addr;
            self.is_spoofing = true;
            self.current_interp_rotation = Some(original.rotation);
        }

        // Calculate target CFrame 
        let target_cframe = CFrame::look_at(camera_pos, target_pos);
        
        if !target_cframe.is_valid() {
            return Err("Invalid spoofed CFrame (contains NaN/Inf)".to_string());
        }
        
        // snap directly to target rotation (lerp causes misses)
        let final_rotation = {
            let mut result = target_cframe.rotation;
            Self::normalize_rotation_matrix(&mut result);
            result
        };
        
        // Update interpolation state
        self.current_interp_rotation = Some(final_rotation);
        
        // aggressive writes to keep spoof active
        let cycles = if is_new_shot { SPOOF_WRITE_CYCLES } else { CONTINUOUS_WRITE_CYCLES };
        
        for _ in 0..cycles {
            // single syscall for all 9 rotation values (prevents torn reads)
            self.memory.write::<[f32; 9]>(rot_base, final_rotation);
        }
        
        // PRO: Write verification - read back and check first value
        if is_new_shot {
            let readback = self.memory.read::<f32>(rot_base);
            if (readback - final_rotation[0]).abs() > 0.01 {
                self.write_verify_failures += 1;
                // Retry write
                for _ in 0..MAX_VERIFY_RETRIES {
                    self.memory.write::<[f32; 9]>(rot_base, final_rotation);
                    let check = self.memory.read::<f32>(rot_base);
                    if (check - final_rotation[0]).abs() < 0.01 {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
    
    /// normalize columns of a 3x3 row-major rotation matrix to prevent drift
    fn normalize_rotation_matrix(m: &mut [f32; 9]) {
        // Column 0 (Right): m[0], m[3], m[6]
        let len0 = (m[0]*m[0] + m[3]*m[3] + m[6]*m[6]).sqrt();
        if len0 > 0.0001 { m[0] /= len0; m[3] /= len0; m[6] /= len0; }
        
        // Column 1 (Up): m[1], m[4], m[7]
        let len1 = (m[1]*m[1] + m[4]*m[4] + m[7]*m[7]).sqrt();
        if len1 > 0.0001 { m[1] /= len1; m[4] /= len1; m[7] /= len1; }
        
        // Column 2 (Z/-Look): m[2], m[5], m[8]
        let len2 = (m[2]*m[2] + m[5]*m[5] + m[8]*m[8]).sqrt();
        if len2 > 0.0001 { m[2] /= len2; m[5] /= len2; m[8] /= len2; }
    }

    fn find_best_target(
        &mut self,
        config: &Config,
        local_player_name: &str,
        camera_pos: &Vector3,
    ) -> Option<Vector3> {
        let ctx = TargetContext::build(&self.cache, &self.visengine, config, local_player_name)?;
        let view_matrix = self.visengine.get_view_matrix();
        let fov = config.camera_aim.fov;

        let mut best: Option<(Vector3, f32, u64, f32, String)> = None;

        for entity in ctx.snapshot.iter() {
            if ctx.should_skip(entity, local_player_name, config.visuals.team_check, config.visuals.hide_dead) {
                continue;
            }

            let entity_pos = entity.root_position().unwrap_or(Vector3::ZERO);
            let world_dist = camera_pos.distance_to(entity_pos);
            if world_dist > MAX_TARGET_DISTANCE {
                continue;
            }

            let mut hitbox_pos = match get_bone_with_fallback(entity, &config.camera_aim.target_bone) {
                Some(pos) => pos,
                None => continue,
            };

            if config.aimbot.prediction_enabled {
                let tracker_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                let tracker = self.velocity_trackers
                    .entry(tracker_key)
                    .or_insert_with(|| VelocityTracker::new(hitbox_pos));
                let (velocity, _accel) = tracker.update(hitbox_pos);

                if velocity.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                    let t = config.aimbot.prediction_amount / 1000.0;
                    hitbox_pos = tracker.predict(hitbox_pos, t);
                }
            }

            // Check FOV using screen distance
            if !self.is_in_fov(hitbox_pos, fov, ctx.screen_center, ctx.dimensions, &view_matrix.m) {
                continue;
            }

            let screen_pos = match self.visengine.world_to_screen(hitbox_pos, ctx.dimensions, &view_matrix) {
                Some(pos) => pos,
                None => continue,
            };

            let screen_dist = screen_pos.distance_to(ctx.screen_center);
            let priority = compute_priority(entity, screen_dist, world_dist);

            if best.is_none() || priority < best.as_ref().unwrap().1 {
                let target_key = if entity.humanoid_address != 0 { entity.humanoid_address } else { entity.model_address };
                best = Some((hitbox_pos, priority, target_key, entity.health, entity.name.clone()));
            }
        }

        if let Some((pos, _priority, addr, health, name)) = best {
            self.last_target_addr = addr;
            self.last_target_health = health;
            self.current_target_name = Some(name);
            return Some(pos);
        }

        self.current_target_name = None;
        None
    }

    fn is_in_fov(
        &self,
        world_pos: Vector3,
        fov_radius: f32,
        screen_center: Vector2,
        dimensions: Vector2,
        view_matrix: &[[f32; 4]; 4],
    ) -> bool {
        let matrix = crate::utils::math::Matrix4 { m: *view_matrix };
        match self.visengine.world_to_screen(world_pos, dimensions, &matrix) {
            Some(screen_pos) => screen_pos.distance_to(screen_center) <= fov_radius,
            None => false,
        }
    }

    fn read_cframe(&self, camera_addr: u64) -> CFrame {
        let rot_base = camera_addr + camera::rotation();
        let pos_base = camera_addr + camera::position();

        // Read all 9 rotation floats in a single syscall instead of 9
        let rotation: [f32; 9] = self.memory.read(rot_base);
        // Read position as a single Vector3 (repr(C)) in one syscall instead of 3
        let position: Vector3 = self.memory.read(pos_base);

        CFrame { rotation, position }
    }

    fn read_camera_position(&self, camera_addr: u64) -> Vector3 {
        let pos_base = camera_addr + camera::position();
        // Read all 3 floats as a single Vector3 in one syscall instead of 3
        self.memory.read::<Vector3>(pos_base)
    }

    #[inline]
    fn get_camera_address(&self) -> Option<u64> {
        self.memory.resolve_camera_address()
    }
}