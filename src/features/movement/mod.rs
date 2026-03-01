use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{base_part, camera, fake_datamodel, humanoid, primitive_flags};
#[allow(unused_imports)]
use crate::core::offsets::{instance, model};
use crate::utils::cache::Cache;
use crate::utils::input::Input;
use crate::utils::math::Vector3;

/// Shared state between main thread and fly thread
#[allow(dead_code)]
pub struct FlyThreadState {
    pub running: AtomicBool,
    pub enabled: AtomicBool,
    pub speed: std::sync::atomic::AtomicU32, 
    pub prim_addr: std::sync::atomic::AtomicU64,
    pub humanoid_addr: std::sync::atomic::AtomicU64,
    pub camera_addr: std::sync::atomic::AtomicU64,
    pub write_intensity: std::sync::atomic::AtomicU8,
    // Fly mode: 0 = velocity only when not moving, 1 or 2 = position+velocity when not moving
    pub fly_mode: std::sync::atomic::AtomicU8,
}

impl FlyThreadState {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            enabled: AtomicBool::new(false),
            speed: std::sync::atomic::AtomicU32::new(25.0f32.to_bits()),
            prim_addr: std::sync::atomic::AtomicU64::new(0),
            humanoid_addr: std::sync::atomic::AtomicU64::new(0),
            camera_addr: std::sync::atomic::AtomicU64::new(0),
            write_intensity: std::sync::atomic::AtomicU8::new(3), 
            fly_mode: std::sync::atomic::AtomicU8::new(1), 
        }
    }

    pub fn get_speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed))
    }

    pub fn set_speed(&self, speed: f32) {
        self.speed.store(speed.to_bits(), Ordering::Relaxed);
    }

    pub fn get_write_intensity(&self) -> u8 {
        self.write_intensity.load(Ordering::Relaxed)
    }

    pub fn set_write_intensity(&self, intensity: u8) {
        self.write_intensity.store(intensity.clamp(1, 5), Ordering::Relaxed);
    }

    pub fn get_fly_mode(&self) -> u8 {
        self.fly_mode.load(Ordering::Relaxed)
    }

    pub fn set_fly_mode(&self, mode: u8) {
        self.fly_mode.store(mode, Ordering::Relaxed);
    }
}

pub struct VoidHideThreadState {
    pub running: AtomicBool,
    #[allow(dead_code)]
    pub enabled: AtomicBool,
    pub prim_addr: std::sync::atomic::AtomicU64,
    // Store last position as 3 separate u32s (bit-cast f32)
    pub last_pos_x: std::sync::atomic::AtomicU32,
    pub last_pos_y: std::sync::atomic::AtomicU32,
    pub last_pos_z: std::sync::atomic::AtomicU32,
}

impl VoidHideThreadState {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            enabled: AtomicBool::new(false),
            prim_addr: std::sync::atomic::AtomicU64::new(0),
            last_pos_x: std::sync::atomic::AtomicU32::new(0.0f32.to_bits()),
            last_pos_y: std::sync::atomic::AtomicU32::new(0.0f32.to_bits()),
            last_pos_z: std::sync::atomic::AtomicU32::new(0.0f32.to_bits()),
        }
    }

    pub fn get_last_pos(&self) -> Vector3 {
        Vector3::new(
            f32::from_bits(self.last_pos_x.load(Ordering::Relaxed)),
            f32::from_bits(self.last_pos_y.load(Ordering::Relaxed)),
            f32::from_bits(self.last_pos_z.load(Ordering::Relaxed)),
        )
    }

    pub fn set_last_pos(&self, pos: Vector3) {
        self.last_pos_x.store(pos.x.to_bits(), Ordering::Relaxed);
        self.last_pos_y.store(pos.y.to_bits(), Ordering::Relaxed);
        self.last_pos_z.store(pos.z.to_bits(), Ordering::Relaxed);
    }
}

/// Shared state between main thread and vehicle fly thread
#[allow(dead_code)]
pub struct VehicleFlyThreadState {
    pub running: AtomicBool,
    pub enabled: AtomicBool,
    pub speed: std::sync::atomic::AtomicU32, // f32 as bits
    pub seat_prim_addr: std::sync::atomic::AtomicU64,
    pub camera_addr: std::sync::atomic::AtomicU64,
    pub write_intensity: std::sync::atomic::AtomicU8,
}

impl VehicleFlyThreadState {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            enabled: AtomicBool::new(false),
            speed: std::sync::atomic::AtomicU32::new(50.0f32.to_bits()),
            seat_prim_addr: std::sync::atomic::AtomicU64::new(0),
            camera_addr: std::sync::atomic::AtomicU64::new(0),
            write_intensity: std::sync::atomic::AtomicU8::new(3),
        }
    }

    pub fn get_speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed))
    }

    pub fn set_speed(&self, speed: f32) {
        self.speed.store(speed.to_bits(), Ordering::Relaxed);
    }

    pub fn get_write_intensity(&self) -> u8 {
        self.write_intensity.load(Ordering::Relaxed)
    }

    pub fn set_write_intensity(&self, intensity: u8) {
        self.write_intensity.store(intensity.clamp(1, 5), Ordering::Relaxed);
    }
}

pub struct MovementHacks {
    memory: Arc<Memory>,
    cache: Arc<Cache>,
    local_player_name: String,
    base_address: u64,
    
    // Fly state
    was_flying: bool,
    // Dedicated fly thread
    fly_thread_state: Arc<FlyThreadState>,
    fly_thread_handle: Option<JoinHandle<()>>,
    
    // FOV state
    original_fov: Option<f32>,
    last_fov_write: f32,
    
    // Spinbot state
    spinbot_angle: f32,
    spinbot_last_time: Instant,
    spinbot_was_enabled: bool,
    
    // Void hide state
    void_hide_thread_state: Arc<VoidHideThreadState>,
    void_hide_thread_handle: Option<JoinHandle<()>>,
    void_hide_was_running: bool,
    
    // Vehicle fly state - dedicated thread for flying vehicles
    vehicle_fly_thread_state: Arc<VehicleFlyThreadState>,
    vehicle_fly_thread_handle: Option<JoinHandle<()>>,
    was_vehicle_flying: bool,
    
    // Anchor state
    anchor_position: Option<Vector3>,
    anchor_was_enabled: bool,
    
    // Waypoint state
    waypoint_saved_pos: Option<Vector3>,
    waypoint_save_key_state: bool,
    waypoint_tp_key_state: bool,
    
    // Per-frame cached addresses (avoid repeated cache snapshot lookups)
    frame_camera_address: Option<u64>,
    frame_humanoid_address: Option<u64>,
    frame_root_part_address: Option<u64>,
}

impl MovementHacks {
    pub fn new(memory: Arc<Memory>, cache: Arc<Cache>, local_player_name: String) -> Self {
        let base_address = memory.base_address();

        Self {
            memory,
            cache,
            local_player_name,
            base_address,
            was_flying: false,
            fly_thread_state: Arc::new(FlyThreadState::new()),
            fly_thread_handle: None,
            original_fov: None,
            last_fov_write: 0.0,
            spinbot_angle: 0.0,
            spinbot_last_time: Instant::now(),
            spinbot_was_enabled: false,
            void_hide_thread_state: Arc::new(VoidHideThreadState::new()),
            void_hide_thread_handle: None,
            void_hide_was_running: false,
            vehicle_fly_thread_state: Arc::new(VehicleFlyThreadState::new()),
            vehicle_fly_thread_handle: None,
            was_vehicle_flying: false,
            anchor_position: None,
            anchor_was_enabled: false,
            waypoint_saved_pos: None,
            waypoint_save_key_state: false,
            waypoint_tp_key_state: false,
            frame_camera_address: None,
            frame_humanoid_address: None,
            frame_root_part_address: None,
        }
    }

    pub fn apply_all(&mut self, config: &Config, menu_open: bool) {
        let any_movement = config.movement.fly_enabled
            || config.movement.noclip_enabled
            || config.movement.spinbot_enabled
            || config.movement.anti_sit_enabled
            || config.movement.hip_height_enabled
            || config.movement.void_hide_enabled
            || config.movement.vehicle_fly_enabled
            || config.movement.anchor_enabled
            || config.movement.waypoint_enabled
            || config.movement.no_fall_damage
            || config.movement.spiderman
            || config.movement.click_teleport
            || config.camera.fov_enabled
            || config.camera.free_camera_enabled
            || config.movement.walk_speed > 16.01
            || config.movement.jump_power > 50.01;
        if !any_movement && self.original_fov.is_none() {
            self.frame_camera_address = None;
            self.frame_humanoid_address = None;
            self.frame_root_part_address = None;
            return;
        }

        // Cache expensive lookups once per frame (avoids repeated snapshot + string search)
        self.frame_camera_address = self.get_camera_address();
        self.frame_humanoid_address = self.get_humanoid_address();
        self.frame_root_part_address = self.get_root_part_address();
        
        self.apply_jump_power(config); 
        self.apply_speed(config); 
        self.apply_fly(config, menu_open); 
        self.apply_vehicle_fly(config, menu_open); 
        self.apply_noclip(config); 
        self.apply_auto_jump(config); 
        self.apply_fov_changer(config); 
        self.apply_spinbot(config); 
        self.apply_anti_sit(config); 
        self.apply_hip_height(config); 
        self.apply_void_hide(config); 
        self.apply_no_fall_damage(config); 
        self.apply_spiderman(config); 
        self.apply_click_teleport(config, menu_open); 
        self.apply_waypoint(config, menu_open); 
        self.apply_anchor(config); 
        self.apply_free_camera(config, menu_open); 
    }

    fn apply_jump_power(&self, config: &Config) {
        let power = config.movement.jump_power;
        
        if power <= 50.0 {
            return;
        }

        if let Some(humanoid) = self.frame_humanoid_address {
            self.memory.write::<f32>(humanoid + humanoid::jump_power(), power);
            self.memory.write::<f32>(humanoid + humanoid::jump_height(), power * 0.15);
        }
    }

    fn apply_speed(&self, config: &Config) {
        let speed = config.movement.walk_speed;
        
        if speed <= 16.0 {
            return;
        }

        if let Some(humanoid) = self.frame_humanoid_address {
            self.memory.write::<f32>(humanoid + humanoid::walkspeed(), speed);
            self.memory.write::<f32>(humanoid + humanoid::walkspeed_check(), speed);
        }
    }

    fn apply_fly(&mut self, _config: &Config, _menu_open: bool) {
        let _ = &self.fly_thread_state;
    }

    fn fly_thread_loop(_state: Arc<FlyThreadState>, _memory: Arc<Memory>) {
    }

    fn apply_vehicle_fly(&mut self, _config: &Config, _menu_open: bool) {
        let _ = &self.vehicle_fly_thread_state;
    }
    
    fn get_vehicle_root_primitive(&self, _seat_object: u64) -> u64 {
        0
    }

    fn vehicle_fly_thread_loop(_state: Arc<VehicleFlyThreadState>, _memory: Arc<Memory>) {
    }

    fn apply_noclip(&mut self, config: &Config) {
        if !config.movement.noclip_enabled {
            return;
        }

        let parts = self.get_all_part_addresses();
        let flags_offset = base_part::primitive_flags();
        let can_collide_bit = primitive_flags::can_collide() as u8; // 0x8
        
        for part_addr in parts {
            if !is_valid_address(part_addr) {
                continue;
            }
            
            // Get primitive from part
            let prim_addr = self.memory.read::<u64>(part_addr + base_part::primitive());
            if !is_valid_address(prim_addr) {
                continue;
            }
            
            // Read flags, clear CanCollide bit, write back (reduced iterations)
            let flags = self.memory.read::<u8>(prim_addr + flags_offset);
            let new_flags = flags & !can_collide_bit;
            
            // Single write is often enough, but 2-3 provides stability
            for _ in 0..2 {
                self.memory.write::<u8>(prim_addr + flags_offset, new_flags);
            }
        }
    }

    fn apply_auto_jump(&self, config: &Config) {
        if !config.movement.auto_jump {
            return;
        }

        let humanoid_addr = match self.frame_humanoid_address {
            Some(addr) => addr,
            None => return,
        };

        // Check if on ground
        let floor_material = self.memory.read::<i32>(humanoid_addr + humanoid::floor_material());
        let on_ground = floor_material != 0;

        if on_ground {
            self.memory.write::<bool>(humanoid_addr + humanoid::jump(), true);
        }
    }

    fn apply_fov_changer(&mut self, config: &Config) {
        let camera = match self.frame_camera_address {
            Some(addr) => addr,
            None => return,
        };

        if !config.camera.fov_enabled {
            if let Some(original) = self.original_fov {
                let current = self.memory.read::<f32>(camera + camera::field_of_view());
                if (current - original).abs() > 0.1 {
                    self.memory.write::<f32>(camera + camera::field_of_view(), original);
                }
                self.original_fov = None;
            }
            return;
        }

        let fov_value = config.camera.fov_value;

        if self.original_fov.is_none() {
            let current = self.memory.read::<f32>(camera + camera::field_of_view());
            if current > 0.0 && current < 180.0 {
                self.original_fov = Some(current);
            }
        }

        if (fov_value - self.last_fov_write).abs() > 0.5 {
            self.memory.write::<f32>(camera + camera::field_of_view(), fov_value);
            self.last_fov_write = fov_value;
        }
    }

    fn apply_spinbot(&mut self, config: &Config) {
        if !config.movement.spinbot_enabled {
            // Only restore AutoRotate on the enable → disable transition
            if self.spinbot_was_enabled {
                if let Some(humanoid_addr) = self.frame_humanoid_address {
                    self.memory.write::<u8>(humanoid_addr + humanoid::auto_rotate(), 1);
                }
                self.spinbot_angle = 0.0;
                self.spinbot_last_time = Instant::now();
                self.spinbot_was_enabled = false;
            }
            return;
        }
        self.spinbot_was_enabled = true;

        let humanoid_addr = match self.frame_humanoid_address {
            Some(addr) => addr,
            None => return,
        };

        // Disable AutoRotate so the game doesn't fight against our rotation
        self.memory.write::<u8>(humanoid_addr + humanoid::auto_rotate(), 0);

        // Time-based rotation for smooth spinning
        let now = Instant::now();
        let delta_time = now.duration_since(self.spinbot_last_time).as_secs_f32();
        self.spinbot_last_time = now;
        
        // Clamp delta time to prevent huge jumps (e.g., after pause/lag)
        let delta_time = delta_time.min(0.1);

        // Speed: slider 1-30, treated as rotations per second
        // e.g. 6.0 = 6 full rotations/sec = 2160 deg/sec
        let degrees_per_second = config.movement.spinbot_speed * 360.0;
        let radians_per_second = degrees_per_second * (std::f32::consts::PI / 180.0);
        
        self.spinbot_angle += radians_per_second * delta_time;
        
        // Keep angle in 0..2*PI range for numerical stability
        if self.spinbot_angle >= std::f32::consts::TAU {
            self.spinbot_angle -= std::f32::consts::TAU;
        }

        // Y-axis rotation matrix (column-major, 9 floats):
        //   right = (cos θ, 0, -sin θ)
        //   up    = (0, 1, 0)
        //   look  = (sin θ, 0, cos θ)
        // Stored: [right.x, up.x, look.x, right.y, up.y, look.y, right.z, up.z, look.z]
        let cos_a = self.spinbot_angle.cos();
        let sin_a = self.spinbot_angle.sin();
        
        let rotation_matrix: [f32; 9] = [
            cos_a,  0.0,  sin_a,
            0.0,    1.0,  0.0,
            -sin_a, 0.0,  cos_a,
        ];
        
        let rotation_offset = base_part::rotation();
        let ang_vel_offset = base_part::assembly_angular_velocity();
        let zero_angular: [f32; 3] = [0.0, 0.0, 0.0];
        // Guard: skip angular velocity writes if offset not found
        let has_ang_vel = ang_vel_offset != 0;

        // Write count scales with write_intensity (same setting as fly)
        // Reduced from old values — physics engine runs at ~240Hz so we only need
        // to outpace one tick between frames, not spam thousands of syscalls.
        let write_count: u32 = match config.movement.write_intensity {
            1 => 15,     // Low
            2 => 40,     // Medium
            3 => 80,     // High
            4 => 150,    // Extreme
            5 => 300,    // Ultra
            _ => 80,     // Default
        };

        // Get local entity from cache to detect rig type and get all parts
        let snapshot = self.cache.get_snapshot();
        let local_entity = snapshot.iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name));

        let is_r15 = local_entity.map(|e| e.rig_type == 1).unwrap_or(true);

        // Helper closure: write rotation (+ optional angular velocity zero) to a primitive
        // Uses write_repeat/write_repeat_2 for efficient batched syscalls with periodic yields.
        let memory = &self.memory;
        let write_spin = |prim: u64| {
            if has_ang_vel {
                memory.write_repeat_2(
                    prim + rotation_offset, rotation_matrix,
                    prim + ang_vel_offset, zero_angular,
                    write_count,
                );
            } else {
                memory.write_repeat(prim + rotation_offset, rotation_matrix, write_count);
            }
        };

        if is_r15 {
            // R15: Motor6D joints propagate from HRP — only need to write HRP primitive
            let hrp = match self.frame_root_part_address {
                Some(addr) if is_valid_address(addr) => addr,
                _ => return,
            };
            let prim = self.memory.read::<u64>(hrp + base_part::primitive());
            if !is_valid_address(prim) {
                return;
            }
            write_spin(prim);
        } else {
            // R6: Joints don't propagate rotation the same way — write rotation
            // to ALL body part primitives so the whole character spins together
            let parts = self.get_all_part_addresses();
            if parts.is_empty() {
                // Fallback: just write to root part
                let hrp = match self.frame_root_part_address {
                    Some(addr) if is_valid_address(addr) => addr,
                    _ => return,
                };
                let prim = self.memory.read::<u64>(hrp + base_part::primitive());
                if is_valid_address(prim) {
                    write_spin(prim);
                }
                return;
            }
            for part_addr in &parts {
                if !is_valid_address(*part_addr) {
                    continue;
                }
                let prim = self.memory.read::<u64>(*part_addr + base_part::primitive());
                if !is_valid_address(prim) {
                    continue;
                }
                write_spin(prim);
            }
        }
    }

    fn apply_anti_sit(&self, config: &Config) {
        if !config.movement.anti_sit_enabled {
            return;
        }

        let humanoid_addr = match self.frame_humanoid_address {
            Some(addr) => addr,
            None => return,
        };

        // Sit offset is 0x1E1 (found via memory scanning)
        let sit_offset = humanoid::sit();
        let is_sitting: bool = self.memory.read(humanoid_addr + sit_offset);

        // If sitting, directly set Sit = false (cleaner than triggering jump)
        if is_sitting {
            self.memory.write::<bool>(humanoid_addr + sit_offset, false);
        }
    }

    fn apply_hip_height(&self, config: &Config) {
        if !config.movement.hip_height_enabled {
            return;
        }

        let humanoid_addr = match self.frame_humanoid_address {
            Some(addr) => addr,
            None => return,
        };

        let height = config.movement.hip_height_value;
        self.memory.write::<f32>(humanoid_addr + humanoid::hip_height(), height);
    }

    // Void hide: teleport player far away, spam writes to hold position.
    // On enable: capture position and start thread. On disable: restore and stop.
    fn apply_void_hide(&mut self, config: &Config) {
        let enabled = config.movement.void_hide_enabled;
        let was_running = self.void_hide_was_running;

        if enabled && !was_running {
            // Just toggled ON - start the thread
            let hrp = match self.frame_root_part_address {
                Some(addr) if is_valid_address(addr) => addr,
                _ => return,
            };

            let prim = self.memory.read::<u64>(hrp + base_part::primitive());
            if !is_valid_address(prim) {
                return;
            }

            // Capture current position BEFORE starting thread
            let current_pos: Vector3 = self.memory.read(prim + base_part::position());
            self.void_hide_thread_state.set_last_pos(current_pos);
            self.void_hide_thread_state.prim_addr.store(prim, Ordering::Relaxed);
            self.void_hide_thread_state.running.store(true, Ordering::SeqCst);

            tracing::info!("Void hide enabled - captured position {:?}, starting thread", current_pos);

            let state = Arc::clone(&self.void_hide_thread_state);
            let memory = Arc::clone(&self.memory);

            self.void_hide_thread_handle = Some(thread::spawn(move || {
                Self::void_hide_thread_loop(state, memory);
            }));

            self.void_hide_was_running = true;
        } else if !enabled && was_running {
            // Just toggled OFF - stop the thread (thread will restore position before exiting)
            tracing::info!("Void hide disabled - stopping thread");
            self.void_hide_thread_state.running.store(false, Ordering::SeqCst);
            self.void_hide_was_running = false;
            
            // Wait for thread to finish (it will restore position)
            if let Some(handle) = self.void_hide_thread_handle.take() {
                let _ = handle.join();
            }
        }
        // When disabled and not running - do nothing (no main loop impact)
    }

    /// Void hide thread - runs only while enabled
    /// Spams position writes to teleport far away
    /// On exit (when running becomes false): restores position
    fn void_hide_thread_loop(state: Arc<VoidHideThreadState>, memory: Arc<Memory>) {
        tracing::info!("Void hide thread started");

        let prim = state.prim_addr.load(Ordering::Relaxed);
        if !is_valid_address(prim) {
            tracing::warn!("Void hide thread: invalid primitive address");
            return;
        }

        let pos_offset = prim + base_part::position();
        let vel_offset = prim + base_part::assembly_linear_velocity();
        let last_pos = state.get_last_pos();

        while state.running.load(Ordering::Relaxed) {
            // Read current and offset far away
            let current_pos: Vector3 = memory.read(pos_offset);
            let far_pos = Vector3::new(
                current_pos.x + 9999.0,
                current_pos.y + 9999.0,
                current_pos.z + 9999.0,
            );
            // 100 writes at 60Hz is enough to keep position locked far away.
            // write_repeat yields every 256 writes to avoid CPU starvation.
            memory.write_repeat(pos_offset, far_pos, 100);

            thread::sleep(Duration::from_millis(16)); // ~60Hz
        }

        // Thread ending - restore position
        tracing::info!("Void hide restoring to position: {:?}", last_pos);
        let zero_velocity = Vector3::ZERO;

        memory.write_repeat_2(pos_offset, last_pos, vel_offset, zero_velocity, 500);

        tracing::info!("Void hide thread stopped");
    }

    fn apply_no_fall_damage(&self, config: &Config) {
        if !config.movement.no_fall_damage {
            return;
        }

        let hrp = match self.frame_root_part_address {
            Some(addr) if is_valid_address(addr) => addr,
            _ => return,
        };

        let prim = self.memory.read::<u64>(hrp + base_part::primitive());
        if !is_valid_address(prim) {
            return;
        }

        let vel_offset = prim + base_part::assembly_linear_velocity();
        let velocity: Vector3 = self.memory.read(vel_offset);

        // If falling fast (Y velocity below -50), clamp it to prevent fall damage
        // Most games trigger fall damage around -50 to -100 velocity
        const FALL_DAMAGE_THRESHOLD: f32 = -50.0;
        if velocity.y < FALL_DAMAGE_THRESHOLD {
            let clamped_velocity = Vector3::new(velocity.x, -10.0, velocity.z);
            self.memory.write(vel_offset, clamped_velocity);
        }
    }

    // Spiderman: climb walls when walking into them using dot product
    fn apply_spiderman(&self, _config: &Config) {
    }

    // Click teleport: raycast forward from camera
    fn apply_click_teleport(&self, config: &Config, menu_open: bool) {
        if !config.movement.click_teleport || menu_open {
            return;
        }

        // Check for configured teleport key (default: right mouse button)
        static LAST_CLICK: AtomicBool = AtomicBool::new(false);
        let key_pressed = Input::is_key_down(config.movement.click_teleport_key as i32);
        
        // Only trigger on key down (not hold)
        let was_pressed = LAST_CLICK.swap(key_pressed, Ordering::Relaxed);
        let trigger = key_pressed && !was_pressed;

        if !trigger {
            return;
        }

        let hrp = match self.frame_root_part_address {
            Some(addr) if is_valid_address(addr) => addr,
            _ => return,
        };

        let prim = self.memory.read::<u64>(hrp + base_part::primitive());
        if !is_valid_address(prim) {
            return;
        }

        let camera = match self.frame_camera_address {
            Some(addr) => addr,
            None => return,
        };

        // Get mouse position (reserved for future use - screen-to-world raycasting)
        let _mouse_pos = Input::get_mouse_position();
        
        // Get camera position and look direction for raycasting
        let cam_pos_offset = camera + camera::position();
        let cam_pos: Vector3 = self.memory.read(cam_pos_offset);
        
        let cam_rot_offset = camera + camera::rotation();
        let rot_matrix: [f32; 9] = self.memory.read(cam_rot_offset);
        
        let look_vec = Vector3::new(-rot_matrix[2], -rot_matrix[5], -rot_matrix[8]);
        
        // Calculate teleport distance (raycast forward from camera)
        // We'll teleport 50 studs forward from camera in the look direction
        const TELEPORT_DISTANCE: f32 = 50.0;
        let target_pos = Vector3::new(
            cam_pos.x + look_vec.x * TELEPORT_DISTANCE,
            cam_pos.y + look_vec.y * TELEPORT_DISTANCE + 0.5, // Small lift to avoid ground clipping
            cam_pos.z + look_vec.z * TELEPORT_DISTANCE,
        );

        // Write new position
        let cframe_offset = prim + base_part::cframe();
        
        let pos_offset = cframe_offset + 36;
        self.memory.write(pos_offset, target_pos);
        
        // Zero velocity to prevent momentum
        let vel_offset = prim + base_part::assembly_linear_velocity();
        self.memory.write(vel_offset, Vector3::ZERO);
    }

    fn apply_waypoint(&mut self, config: &Config, menu_open: bool) {
        if !config.movement.waypoint_enabled || menu_open {
            return;
        }

        let hrp = match self.frame_root_part_address {
            Some(addr) if is_valid_address(addr) => addr,
            _ => return,
        };

        let prim = self.memory.read::<u64>(hrp + base_part::primitive());
        if !is_valid_address(prim) {
            return;
        }

        let cframe_pos = prim + base_part::cframe() + 36; // rotation(9f) then position(3f)

        // Save key – one-shot on press
        let save_pressed = Input::is_key_down(config.movement.waypoint_save_key as i32);
        if save_pressed && !self.waypoint_save_key_state {
            let pos: Vector3 = self.memory.read(cframe_pos);
            if pos.x.is_finite() && pos.y.is_finite() && pos.z.is_finite() {
                self.waypoint_saved_pos = Some(pos);
                tracing::info!("Waypoint saved at ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);
            }
        }
        self.waypoint_save_key_state = save_pressed;

        // Teleport key – one-shot on press
        // Must spam writes to overpower the physics engine (single write gets
        // immediately overwritten). write_repeat_2 writes position + zero velocity
        // in a tight batched loop with periodic yields.
        let tp_pressed = Input::is_key_down(config.movement.waypoint_tp_key as i32);
        if tp_pressed && !self.waypoint_tp_key_state {
            if let Some(saved_pos) = self.waypoint_saved_pos {
                let vel_offset = prim + base_part::assembly_linear_velocity();
                self.memory.write_repeat_2(
                    cframe_pos, saved_pos,
                    vel_offset, Vector3::ZERO,
                    500,
                );
            }
        }
        self.waypoint_tp_key_state = tp_pressed;
    }

    fn apply_anchor(&mut self, config: &Config) {
        if !config.movement.anchor_enabled {
            // Released anchor - clear saved position
            if self.anchor_was_enabled {
                self.anchor_position = None;
                self.anchor_was_enabled = false;
            }
            return;
        }

        // On first enable, save current position
        if !self.anchor_was_enabled {
            self.anchor_was_enabled = true;
            if let Some(hrp) = self.frame_root_part_address {
                if is_valid_address(hrp) {
                    let prim = self.memory.read::<u64>(hrp + base_part::primitive());
                    if is_valid_address(prim) {
                        let cframe_offset = prim + base_part::cframe();
                        let pos: Vector3 = self.memory.read(cframe_offset + 36);
                        if pos.x.is_finite() && pos.y.is_finite() && pos.z.is_finite() {
                            self.anchor_position = Some(pos);
                        }
                    }
                }
            }
        }

        // Continuously write saved position back (lock in place)
        if let Some(anchor_pos) = self.anchor_position {
            if let Some(hrp) = self.frame_root_part_address {
                if is_valid_address(hrp) {
                    let prim = self.memory.read::<u64>(hrp + base_part::primitive());
                    if is_valid_address(prim) {
                        let cframe_offset = prim + base_part::cframe();
                        self.memory.write(cframe_offset + 36, anchor_pos);
                        // Zero velocity to prevent drifting
                        let vel_offset = prim + base_part::assembly_linear_velocity();
                        self.memory.write(vel_offset, Vector3::ZERO);
                    }
                }
            }
        }
    }

    fn apply_free_camera(&mut self, config: &Config, menu_open: bool) {
        if !config.camera.free_camera_enabled || menu_open {
            return;
        }

        let camera = match self.frame_camera_address {
            Some(addr) => addr,
            None => return,
        };

        // Get camera direction
        let cam_rot_offset = camera + camera::rotation();
        let rot_matrix: [f32; 9] = self.memory.read(cam_rot_offset);
        
        let look_vec = Vector3::new(-rot_matrix[2], -rot_matrix[5], -rot_matrix[8]);
        let right_vec = Vector3::new(rot_matrix[0], rot_matrix[3], rot_matrix[6]);
        let up_vec = Vector3::new(0.0, 1.0, 0.0);

        // Build direction from input
        let mut direction = Vector3::ZERO;
        
        if Input::is_key_down(0x57) { // W
            direction.x += look_vec.x;
            direction.y += look_vec.y;
            direction.z += look_vec.z;
        }
        if Input::is_key_down(0x53) { // S
            direction.x -= look_vec.x;
            direction.y -= look_vec.y;
            direction.z -= look_vec.z;
        }
        if Input::is_key_down(0x41) { // A
            direction.x -= right_vec.x;
            direction.y -= right_vec.y;
            direction.z -= right_vec.z;
        }
        if Input::is_key_down(0x44) { // D
            direction.x += right_vec.x;
            direction.y += right_vec.y;
            direction.z += right_vec.z;
        }
        if Input::is_key_down(0x20) { // Space
            direction.y += up_vec.y;
        }
        if Input::is_key_down(0x11) { // Ctrl
            direction.y -= up_vec.y;
        }

        // Normalize direction
        let magnitude = (direction.x * direction.x + direction.y * direction.y + direction.z * direction.z).sqrt();
        if magnitude < 0.001 {
            return; // No movement input
        }
        direction.x /= magnitude;
        direction.y /= magnitude;
        direction.z /= magnitude;

        // Get current camera position
        let cam_pos_offset = camera + camera::position();
        let current_pos: Vector3 = self.memory.read(cam_pos_offset);

        // Calculate new position
        let speed = config.camera.free_camera_speed / 10.0; // Scale speed for smoother movement
        let new_pos = Vector3::new(
            current_pos.x + direction.x * speed,
            current_pos.y + direction.y * speed,
            current_pos.z + direction.z * speed,
        );

        // Write new camera position
        self.memory.write(cam_pos_offset, new_pos);
    }

    fn get_humanoid_address(&self) -> Option<u64> {
        // First try: Get from cached entity (works for normal games)
        let from_cache = self.cache
            .get_snapshot()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
            .filter(|e| is_valid_address(e.humanoid_address))
            .map(|e| e.humanoid_address);
        
        if from_cache.is_some() {
            return from_cache;
        }
        
        // Fallback: Get from LocalPlayer.Character.Humanoid (works for PF and other games)
        self.get_humanoid_from_local_player()
    }
    
    /// Get humanoid from LocalPlayer.Character.Humanoid (fallback for games
    /// where the cached entity doesn't have humanoid, e.g. Phantom Forces)
    fn get_humanoid_from_local_player(&self) -> Option<u64> {
        // Get DataModel
        let fake_dm = self.memory.read::<u64>(self.base_address + fake_datamodel::pointer());
        if !is_valid_address(fake_dm) {
            return None;
        }
        
        let dm = self.memory.read::<u64>(fake_dm + fake_datamodel::real_datamodel());
        if !is_valid_address(dm) {
            return None;
        }
        
        // Get Players service via Instance traversal
        let datamodel_instance = crate::sdk::Instance::new(dm, Arc::clone(&self.memory));
        let players_instance = datamodel_instance.find_first_child_by_class("Players")?;
        
        // Get LocalPlayer
        let local_player_addr = self.memory.read::<u64>(players_instance.address + crate::core::offsets::player::localplayer());
        if !is_valid_address(local_player_addr) {
            return None;
        }
        
        // Get Character (Model) from LocalPlayer
        let player = crate::sdk::Player::new(local_player_addr, Arc::clone(&self.memory));
        let character = player.get_model_instance();
        if !character.is_valid() {
            return None;
        }
        
        // Find Humanoid in Character
        character.get_humanoid().map(|h| h.address)
    }

    fn get_root_part_address(&self) -> Option<u64> {
        // First try: Get HumanoidRootPart from cache
        let from_cache = self.cache
            .get_snapshot()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
            .and_then(|e| e.root_part())
            .map(|p| p.address);
        
        if from_cache.is_some() {
            return from_cache;
        }
        
        // Fallback: Get from LocalPlayer.Character
        self.get_root_part_from_local_player()
    }
    
    /// Get root part from LocalPlayer.Character (fallback)
    fn get_root_part_from_local_player(&self) -> Option<u64> {
        let fake_dm = self.memory.read::<u64>(self.base_address + fake_datamodel::pointer());
        if !is_valid_address(fake_dm) {
            return None;
        }
        
        let dm = self.memory.read::<u64>(fake_dm + fake_datamodel::real_datamodel());
        if !is_valid_address(dm) {
            return None;
        }
        
        // Get Players service via Instance traversal
        let datamodel_instance = crate::sdk::Instance::new(dm, Arc::clone(&self.memory));
        let players_instance = datamodel_instance.find_first_child_by_class("Players")?;
        
        let local_player_addr = self.memory.read::<u64>(players_instance.address + crate::core::offsets::player::localplayer());
        if !is_valid_address(local_player_addr) {
            return None;
        }
        
        let player = crate::sdk::Player::new(local_player_addr, Arc::clone(&self.memory));
        let character = player.get_model_instance();
        if !character.is_valid() {
            return None;
        }
        
        // Try HumanoidRootPart first, then Torso for PF
        character.find_first_child("HumanoidRootPart")
            .or_else(|| character.find_first_child("Torso"))
            .map(|part| part.address)
    }

    fn get_all_part_addresses(&self) -> Vec<u64> {
        self.cache
            .get_snapshot()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
            .map(|e| e.parts.values().map(|p| p.address).collect())
            .unwrap_or_default()
    }

    fn get_camera_address(&self) -> Option<u64> {
        self.memory.resolve_camera_address()
    }
}
