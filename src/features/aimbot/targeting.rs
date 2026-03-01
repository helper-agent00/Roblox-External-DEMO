use ahash::AHashSet;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::core::Memory;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache, Entity};
use crate::utils::game_support::GameId;
use crate::utils::input::Input;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::{is_teammate, RingVelocityTracker, VELOCITY_THRESHOLD};
use crate::utils::targeting::get_bone_with_fallback;

/// Minimum error below which we stop moving (dead-zone prevents micro-jitter)
const DEAD_ZONE_PX: f32 = 0.5;

/// Hardware mouse delta cap per SendInput call
const MAX_MOUSE_DELTA: f32 = 80.0;

// screen-distance zones (pixels) for adaptive gain
const ZONE_MICRO: f32 = 4.0;       // < 4px  : ultra-precision micro-corrections
const ZONE_CLOSE: f32 = 20.0;      // < 20px : fine CQC tracking
const ZONE_MEDIUM: f32 = 80.0;     // < 80px : standard engagement
const ZONE_FAR: f32 = 250.0;       // < 250px: target acquisition
                                    // > 250px: snap-acquisition (capped)

// zone gain multipliers (on top of base smoothing speed)
const GAIN_MICRO: f32 = 0.12;
const GAIN_CLOSE: f32 = 0.22;
const GAIN_MEDIUM: f32 = 0.38;
const GAIN_FAR: f32 = 0.50;
const GAIN_SNAP: f32 = 0.55;

/// Derivative damping coefficient. higher = more damping, less overshoot.
const KD_COEFFICIENT: f32 = 0.40;

// target lock
const TARGET_SWITCH_DELAY_MS: u64 = 65;
const LOCK_BREAK_DISTANCE: f32 = 450.0;

// activation ramp (human-like start-up)
const RAMP_UP_MS: f32 = 280.0;     // Time to reach full speed after activation
const RAMP_MIN: f32 = 0.10;

// target switch interpolation
const SWITCH_INTERP_MS: f32 = 140.0;

// screen-space lead compensation
/// Fraction of target's screen velocity added as lead.
const SCREEN_LEAD_FACTOR: f32 = 0.30;

// world-distance CQC dampening
const CQC_RANGE_STUDS: f32 = 15.0;
const CQC_MIN_FACTOR: f32 = 0.20;

// humanization
const HUMANIZE_VARIANCE: f32 = 0.006;
const MICRO_OVERSHOOT_CHANCE: f32 = 0.03;
const MICRO_OVERSHOOT_AMOUNT: f32 = 0.35;

// Operation One — game-specific aim tuning.

/// EMA alpha for smoothing Op1 target screen positions.
const OP1_SCREEN_EMA_ALPHA: f32 = 0.40;

/// Op1 dead zone.
const OP1_DEAD_ZONE_PX: f32 = 1.2;

/// Op1 derivative damping coefficient.
const OP1_KD_COEFFICIENT: f32 = 0.72;

/// Op1 zone gains.
const OP1_GAIN_MICRO: f32 = 0.08;
const OP1_GAIN_CLOSE: f32 = 0.15;
const OP1_GAIN_MEDIUM: f32 = 0.26;
const OP1_GAIN_FAR: f32 = 0.35;
const OP1_GAIN_SNAP: f32 = 0.40;

/// Op1 screen-lead factor.
const OP1_SCREEN_LEAD_FACTOR: f32 = 0.12;

/// Op1 lock-break distance.
const OP1_LOCK_BREAK_DISTANCE: f32 = 650.0;

/// Op1 per-frame proportional cap.
const OP1_FRAME_P_CAP: f32 = 0.30;

/// Op1 CQC dampening — tactical shooter with closer engagements.
const OP1_CQC_RANGE_STUDS: f32 = 20.0;
const OP1_CQC_MIN_FACTOR: f32 = 0.15;

#[derive(Clone)]
struct LockedTarget {
    player_name: String,
    model_address: u64,
    locked_bone: BodyPart,
    lock_time: Instant,
    #[allow(dead_code)]
    priority_score: f32,
    last_screen_pos: Vector2,
}

/// Tracks the locked target's screen-space velocity for lead compensation.
struct ScreenVelocityTracker {
    prev_pos: Vector2,
    prev_time: Instant,
    velocity: Vector2, // pixels/sec, EMA-smoothed
}

impl ScreenVelocityTracker {
    fn new(pos: Vector2) -> Self {
        Self {
            prev_pos: pos,
            prev_time: Instant::now(),
            velocity: Vector2::ZERO,
        }
    }

    /// Feed a new screen position. Returns smoothed screen velocity (px/sec).
    fn update(&mut self, pos: Vector2) -> Vector2 {
        let dt = self.prev_time.elapsed().as_secs_f32();
        if dt > 0.001 && dt < 0.15 {
            let instant = Vector2::new(
                (pos.x - self.prev_pos.x) / dt,
                (pos.y - self.prev_pos.y) / dt,
            );
            // Reject teleport-level outliers
            if instant.length() < 8000.0 {
                let alpha = 0.35;
                self.velocity = Vector2::new(
                    self.velocity.x + alpha * (instant.x - self.velocity.x),
                    self.velocity.y + alpha * (instant.y - self.velocity.y),
                );
            }
            self.prev_pos = pos;
            self.prev_time = Instant::now();
        }
        self.velocity
    }
}

pub struct AimAssist {
    cache: Arc<Cache>,
    pub visengine: Arc<VisualEngine>,

    // Sub-pixel accumulator
    accumulated_dx: f32,
    accumulated_dy: f32,

    // Target lock
    aimbot_locked_target: Option<LockedTarget>,

    // World-space velocity trackers (for projectile-style prediction)
    velocity_trackers: HashMap<usize, RingVelocityTracker>,

    // Screen-space velocity tracker (for lead compensation on locked target)
    screen_vel_tracker: Option<ScreenVelocityTracker>,

    // Frame timing
    last_frame_time: Instant,

    // PD controller: previous-frame error for derivative term
    prev_error: Vector2,
    prev_error_valid: bool,

    // PRNG
    rng_state: u32,

    // Activation state
    is_toggled_on: bool,
    last_key_state: bool,
    activation_time: Instant,
    deactivation_time: Instant,
    key_press_start: Option<Instant>,

    // Activation ramp-up (smooth human-like start)
    activation_ramp: f32,

    // Target switch interpolation
    prev_target_screen: Option<Vector2>,
    target_switch_time: Option<Instant>,

    // Cached local position from get_target_with_lock
    cached_local_pos: Vector3,

    // Operation One — EMA-smoothed screen position to filter noisy viewmodel data
    op1_smoothed_screen: Option<Vector2>,
}

impl AimAssist {
    pub fn new(_memory: Arc<Memory>, cache: Arc<Cache>, visengine: Arc<VisualEngine>) -> Self {
        Self {
            cache,
            visengine,
            accumulated_dx: 0.0,
            accumulated_dy: 0.0,
            aimbot_locked_target: None,
            velocity_trackers: HashMap::new(),
            screen_vel_tracker: None,
            last_frame_time: Instant::now(),
            prev_error: Vector2::ZERO,
            prev_error_valid: false,
            rng_state: 0xDEADBEEF,
            is_toggled_on: false,
            last_key_state: false,
            activation_time: Instant::now(),
            deactivation_time: Instant::now(),
            key_press_start: None,
            activation_ramp: 0.0,
            prev_target_screen: None,
            target_switch_time: None,
            cached_local_pos: Vector3::ZERO,
            op1_smoothed_screen: None,
        }
    }

    #[inline]
    fn fast_rand(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    #[inline]
    fn humanize(&mut self, value: f32) -> f32 {
        let variance = (self.fast_rand() - 0.5) * 2.0 * HUMANIZE_VARIANCE;
        value * (1.0 + variance)
    }

    fn get_target_with_lock(
        &mut self,
        config: &Config,
        local_player_name: &str,
        locked: Option<&LockedTarget>,
    ) -> Option<(Entity, Vector2)> {
        let fov = config.aimbot.fov;
        let target_bone = &config.aimbot.target_bone;
        let prediction_enabled = config.aimbot.prediction_enabled;
        let prediction_ms = config.aimbot.prediction_amount;
        let team_check = config.visuals.team_check;
        let hide_dead = config.visuals.hide_dead;

        let snapshot = self.cache.get_snapshot();
        let view_matrix = self.visengine.get_view_matrix();
        let dimensions = self.visengine.get_dimensions();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return None;
        }

        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);

        let local_entity = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name));
        let local_team = self.cache.get_local_team_addr();
        
        // Cache local position to avoid second snapshot in apply().
        // For workspace-player games (PF, Op1, Blox Strike) the local player
        // entity is NOT in the snapshot (the standard builder is skipped).
        // Fall back to camera position so world_distance is accurate and
        // CQC dampening works correctly at close range.
        self.cached_local_pos = local_entity
            .and_then(|e| e.root_position())
            .or_else(|| self.visengine.get_camera_position())
            .unwrap_or(Vector3::ZERO);
        
        // Use cache's local team identifier (stored separately since local player is filtered from snapshot)
        let local_team_identifier = self.cache.get_local_team_id();
        
        // Build teammate address set from whitelist
        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> = if team_check && !teammate_whitelist.is_empty() {
            snapshot.iter()
                .filter(|e| teammate_whitelist.iter().any(|n| n.eq_ignore_ascii_case(&e.name)))
                .map(|e| e.model_address)
                .collect()
        } else {
            AHashSet::new()
        };

        if let Some(lock) = locked {
            let lock_duration = lock.lock_time.elapsed().as_millis() as u64;
            
            if !lock.player_name.eq_ignore_ascii_case(local_player_name) {
                // Match by player_name first (persists across respawns), model_address is secondary
                if let Some(entity) = snapshot.iter().find(|e| 
                    e.name == lock.player_name
                ) {
                    if hide_dead && entity.is_dead() {
                        return None; 
                    }
                    
                    // Team check — game-aware dispatch
                    let game_id = self.cache.get_game_id();
                    if is_teammate(entity, team_check, local_team, &teammate_addresses, &local_team_identifier, game_id) {
                    } else if let Some(part) = entity.parts.get(&lock.locked_bone) {
                        let mut target_pos = part.position;

                        if target_pos.is_valid() && !target_pos.is_near_origin(1.0) {
                            let tracker = self.velocity_trackers
                                .entry(entity.model_address as usize)
                                .or_insert_with(|| RingVelocityTracker::new(target_pos));
                            let velocity = tracker.update(target_pos);
                            let acceleration = tracker.get_acceleration();
                            
                            if prediction_enabled && velocity.length_squared() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
                                target_pos = Self::predict_position_quadratic(
                                    target_pos, velocity, acceleration, prediction_ms
                                );
                            }

                            if let Some(screen_pos) = self.visengine.world_to_screen(target_pos, dimensions, &view_matrix) {
                                // PRO: Check if target went too far off-screen (break lock)
                                // Op1 uses a wider lock-break tolerance to ride through viewmodel position noise
                                let lock_break = if game_id == GameId::OperationOne { OP1_LOCK_BREAK_DISTANCE } else { LOCK_BREAK_DISTANCE };
                                let dist = screen_pos.distance_to(screen_center);
                                if dist < lock_break || lock_duration < TARGET_SWITCH_DELAY_MS {
                                    return Some((entity.clone(), screen_pos));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Find best target with priority scoring
        let candidates: Vec<(Entity, Vector2, f32)> = snapshot
            .par_iter()
            .filter_map(|entity| {
                if entity.name.eq_ignore_ascii_case(local_player_name) {
                    return None;
                }

                // Team check — game-aware dispatch
                let game_id = self.cache.get_game_id();
                if is_teammate(entity, team_check, local_team, &teammate_addresses, &local_team_identifier, game_id) {
                    return None;
                }

                if hide_dead && entity.is_dead() {
                    return None;
                }

                // Skip entities with no humanoid UNLESS they are game-specific or have valid body parts
                if entity.humanoid_address == 0 && !entity.is_game_specific && entity.root_part().is_none() {
                    return None;
                }

                let target_pos = get_bone_with_fallback(entity, target_bone)?;

                if !target_pos.is_valid() || target_pos.is_near_origin(1.0) {
                    return None;
                }

                let screen_pos = self.visengine.world_to_screen(target_pos, dimensions, &view_matrix)?;
                let screen_dist = screen_pos.distance_to(screen_center);

                if screen_dist > fov {
                    return None;
                }
                
                // Calculate priority score
                // Lower score = higher priority
                let mut priority = screen_dist;
                
                // Bonus for low health targets (prioritize kills)
                if entity.max_health > 0.0 {
                    let health_pct = entity.health / entity.max_health;
                    priority *= 0.5 + health_pct * 0.5;  // Low health = lower priority score
                }
                
                // Bonus for targets closer to screen center
                let center_bonus = 1.0 - (screen_dist / fov).min(1.0) * 0.3;
                priority *= center_bonus;

                Some((entity.clone(), screen_pos, priority))
            })
            .collect();

        candidates
            .into_iter()
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(entity, screen_pos, _)| (entity, screen_pos))
    }

    pub fn apply(&mut self, config: &Config, local_player_name: &str) {
        // Anti-tamper checks are done in the render loop (app.rs) — not here.

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32().clamp(0.0001, 0.05);
        self.last_frame_time = now;

        if !config.aimbot.enabled {
            self.reset_state_full();
            return;
        }

        // Activation check
        let aim_key_pressed = Input::is_key_down(config.hotkeys.aim_key as i32);
        let is_active = self.check_activation(config, aim_key_pressed);
        self.cache.set_high_priority(is_active);

        if !is_active {
            self.reset_state_soft();
            return;
        }

        // In Toggle / Always-on modes, don't send mouse deltas unless RMB
        // is held — Roblox only rotates camera while RMB is down.
        let rmb_held = Input::is_key_down(0x02);
        if config.aimbot.activation_mode >= 1 && !rmb_held {
            // Keep lock/velocity trackers alive so aimbot is ready on RMB press
            return;
        }

        // --- activation ramp-up ---
        let ramp_elapsed_ms = now.duration_since(self.activation_time).as_secs_f32() * 1000.0;
        self.activation_ramp = if ramp_elapsed_ms >= RAMP_UP_MS {
            1.0
        } else {
            let t = ramp_elapsed_ms / RAMP_UP_MS;
            RAMP_MIN + (1.0 - RAMP_MIN) * Self::ease_out_cubic(t)
        };

        let target_bone = &config.aimbot.target_bone;
        let locked_target_clone = self.aimbot_locked_target.clone();
        let target_result = self.get_target_with_lock(
            config,
            local_player_name,
            locked_target_clone.as_ref(),
        );

        let (entity, mut screen_pos) = match target_result {
            Some(t) => t,
            None => {
                self.reset_state_soft();
                return;
            }
        };

        // Detect Operation One for game-specific tuning
        let game_id = self.cache.get_game_id();
        let is_op1 = game_id == GameId::OperationOne;

        let local_pos = self.cached_local_pos;
        let target_pos = entity.parts.get(&BodyPart::HumanoidRootPart)
            .or_else(|| entity.parts.get(&BodyPart::UpperTorso))
            .or_else(|| entity.parts.get(&BodyPart::Head))
            .map(|p| p.position)
            .unwrap_or(Vector3::ZERO);
        let world_distance = local_pos.distance_to(target_pos);

        let dimensions = self.visengine.get_dimensions();
        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);
        let priority_score = screen_pos.distance_to(screen_center);

        // --- Handle target switching & lock ---
        let is_new_target = match &self.aimbot_locked_target {
            None => true,
            Some(lock) => lock.model_address != entity.model_address,
        };

        if is_new_target {
            // Record previous target's screen pos for smooth interpolation
            if let Some(ref lock) = self.aimbot_locked_target {
                self.prev_target_screen = Some(lock.last_screen_pos);
                self.target_switch_time = Some(now);
            }

            let locked_bone = Self::find_valid_bone(&entity, target_bone)
                .unwrap_or(BodyPart::Head);
            self.aimbot_locked_target = Some(LockedTarget {
                player_name: entity.name.clone(),
                model_address: entity.model_address,
                locked_bone,
                lock_time: now,
                priority_score,
                last_screen_pos: screen_pos,
            });

            // Fresh screen-velocity tracker for the new target
            self.screen_vel_tracker = Some(ScreenVelocityTracker::new(screen_pos));
            // Reset derivative to avoid spike on target switch
            self.prev_error_valid = false;
            // Reset Op1 smoother so it doesn't drag from previous target's position
            self.op1_smoothed_screen = None;
        } else if let Some(ref mut lock) = self.aimbot_locked_target {
            lock.last_screen_pos = screen_pos;
            if lock.model_address != entity.model_address {
                tracing::debug!("Target {} respawned, updating model address", entity.name);
                lock.model_address = entity.model_address;
                lock.lock_time = now;
            }
        }

        // --- Target switch interpolation (blend to new target) ---
        if let (Some(prev_screen), Some(switch_time)) = (self.prev_target_screen, self.target_switch_time) {
            let elapsed_ms = now.duration_since(switch_time).as_secs_f32() * 1000.0;
            if elapsed_ms < SWITCH_INTERP_MS {
                let t = Self::ease_out_cubic(elapsed_ms / SWITCH_INTERP_MS);
                screen_pos = prev_screen.lerp(screen_pos, t);
            } else {
                self.prev_target_screen = None;
                self.target_switch_time = None;
            }
        }

        // --- Op1: EMA screen-position smoothing ---
        // Filters noisy viewmodel positions so the PD controller gets a stable signal.
        if is_op1 {
            screen_pos = match self.op1_smoothed_screen {
                Some(prev) => {
                    let smoothed = Vector2::new(
                        prev.x + OP1_SCREEN_EMA_ALPHA * (screen_pos.x - prev.x),
                        prev.y + OP1_SCREEN_EMA_ALPHA * (screen_pos.y - prev.y),
                    );
                    self.op1_smoothed_screen = Some(smoothed);
                    smoothed
                }
                None => {
                    self.op1_smoothed_screen = Some(screen_pos);
                    screen_pos
                }
            };
        }

        // --- Screen-space lead compensation ---
        // Aim where target WILL BE given our smoothing latency.
        let lead_factor = if is_op1 { OP1_SCREEN_LEAD_FACTOR } else { SCREEN_LEAD_FACTOR };
        if let Some(ref mut tracker) = self.screen_vel_tracker {
            let screen_vel = tracker.update(screen_pos);
            let smoothing = config.aimbot.smoothing.max(1.0);
            // Lead time proportional to smoothing (~16ms at s=4, ~40ms at s=10)
            let lead_time_sec = smoothing * 0.004;
            if screen_vel.length() > 30.0 {
                screen_pos = Vector2::new(
                    screen_pos.x + screen_vel.x * lead_time_sec * lead_factor,
                    screen_pos.y + screen_vel.y * lead_time_sec * lead_factor,
                );
            }
        }

        // --- PD aim controller ---
        self.apply_pd_aim(screen_pos, world_distance, dt, config, is_op1);
    }

    // PD aim controller core
    fn apply_pd_aim(
        &mut self,
        target_screen: Vector2,
        world_distance: f32,
        dt: f32,
        config: &Config,
        is_op1: bool,
    ) {
        let dimensions = self.visengine.get_dimensions();
        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);

        let error = Vector2::new(
            target_screen.x - screen_center.x,
            target_screen.y - screen_center.y,
        );
        let error_mag = error.length();

        // Dead zone (wider for Op1 to absorb viewmodel jitter)
        let dead_zone = if is_op1 { OP1_DEAD_ZONE_PX } else { DEAD_ZONE_PX };
        if error_mag < dead_zone {
            self.accumulated_dx = 0.0;
            self.accumulated_dy = 0.0;
            self.prev_error = error;
            self.prev_error_valid = true;
            return;
        }

        // --- Base speed from smoothing config ---
        let smoothing = config.aimbot.smoothing.max(1.0);
        let base_speed = 0.5 / (smoothing * 0.15 + 1.0);

        // --- Distance-adaptive gain (5 zones) ---
        let (g_micro, g_close, g_medium, g_far, g_snap) = if is_op1 {
            (OP1_GAIN_MICRO, OP1_GAIN_CLOSE, OP1_GAIN_MEDIUM, OP1_GAIN_FAR, OP1_GAIN_SNAP)
        } else {
            (GAIN_MICRO, GAIN_CLOSE, GAIN_MEDIUM, GAIN_FAR, GAIN_SNAP)
        };
        let zone_gain = if error_mag < ZONE_MICRO {
            g_micro
        } else if error_mag < ZONE_CLOSE {
            let t = (error_mag - ZONE_MICRO) / (ZONE_CLOSE - ZONE_MICRO);
            g_micro + (g_close - g_micro) * t
        } else if error_mag < ZONE_MEDIUM {
            let t = (error_mag - ZONE_CLOSE) / (ZONE_MEDIUM - ZONE_CLOSE);
            g_close + (g_medium - g_close) * Self::ease_out_quad(t)
        } else if error_mag < ZONE_FAR {
            let t = (error_mag - ZONE_MEDIUM) / (ZONE_FAR - ZONE_MEDIUM);
            g_medium + (g_far - g_medium) * Self::ease_out_cubic(t)
        } else {
            g_snap
        };

        // --- CQC dampening (close distance = slow down to prevent jitter) ---
        let (cqc_range, cqc_min) = if is_op1 {
            (OP1_CQC_RANGE_STUDS, OP1_CQC_MIN_FACTOR)
        } else {
            (CQC_RANGE_STUDS, CQC_MIN_FACTOR)
        };
        let cqc_factor = if world_distance < cqc_range {
            cqc_min + (1.0 - cqc_min) * (world_distance / cqc_range)
        } else {
            1.0
        };

        // --- Activation ramp ---
        let ramp = self.activation_ramp;

        // --- P-term ---
        let frame_p_cap = if is_op1 { OP1_FRAME_P_CAP } else { 0.45 };
        let p_speed = base_speed * zone_gain * cqc_factor * ramp;
        let frame_p = (p_speed * dt * 60.0).min(frame_p_cap);
        let p_x = error.x * frame_p;
        let p_y = error.y * frame_p;

        // --- D-term ---
        // Op1 uses higher damping to suppress overshoot from noisy positions.
        let kd = if is_op1 { OP1_KD_COEFFICIENT } else { KD_COEFFICIENT };
        let (d_x, d_y) = if self.prev_error_valid {
            let de_x = error.x - self.prev_error.x;
            let de_y = error.y - self.prev_error.y;
            let d_scale = kd * zone_gain;
            (-de_x * d_scale, -de_y * d_scale)
        } else {
            (0.0, 0.0)
        };

        self.prev_error = error;
        self.prev_error_valid = true;

        // --- Combine P + D ---
        let mut move_x = p_x + d_x;
        let mut move_y = p_y + d_y;

        // --- Humanization: tiny variance ---
        move_x = self.humanize(move_x);
        move_y = self.humanize(move_y);

        // --- Occasional micro-overshoot for realism (only near target) ---
        if self.fast_rand() < MICRO_OVERSHOOT_CHANCE && error_mag < ZONE_CLOSE {
            let overshoot = (self.fast_rand() - 0.5) * 2.0 * MICRO_OVERSHOOT_AMOUNT;
            move_x += overshoot;
            move_y += overshoot * 0.7;
        }

        // --- Sub-pixel accumulation ---
        self.accumulated_dx += move_x;
        self.accumulated_dy += move_y;

        let int_dx = self.accumulated_dx.trunc() as i32;
        let int_dy = self.accumulated_dy.trunc() as i32;
        self.accumulated_dx -= int_dx as f32;
        self.accumulated_dy -= int_dy as f32;

        let int_dx = int_dx.clamp(-(MAX_MOUSE_DELTA as i32), MAX_MOUSE_DELTA as i32);
        let int_dy = int_dy.clamp(-(MAX_MOUSE_DELTA as i32), MAX_MOUSE_DELTA as i32);

        if int_dx != 0 || int_dy != 0 {
            Input::move_mouse(int_dx, int_dy);
        }
    }

    fn check_activation(&mut self, config: &Config, key_pressed: bool) -> bool {
        let key_just_pressed = key_pressed && !self.last_key_state;
        let key_just_released = !key_pressed && self.last_key_state;
        self.last_key_state = key_pressed;

        let hold_delay_ms = config.aimbot.hold_delay_ms;

        match config.aimbot.activation_mode {
            0 => {
                if key_just_pressed {
                    self.key_press_start = Some(Instant::now());
                } else if key_just_released {
                    self.key_press_start = None;
                    if self.is_toggled_on {
                        self.is_toggled_on = false;
                        self.deactivation_time = Instant::now();
                    }
                    return false;
                }

                if key_pressed {
                    if let Some(start) = self.key_press_start {
                        let held_ms = start.elapsed().as_millis() as u32;
                        if held_ms >= hold_delay_ms {
                            if !self.is_toggled_on {
                                self.is_toggled_on = true;
                                self.activation_time = Instant::now();
                                self.activation_ramp = RAMP_MIN;
                            }
                            return true;
                        }
                    }
                }
                false
            }
            // Toggle
            1 => {
                if key_just_pressed {
                    self.is_toggled_on = !self.is_toggled_on;
                    if self.is_toggled_on {
                        self.activation_time = Instant::now();
                        self.activation_ramp = RAMP_MIN;
                    } else {
                        self.deactivation_time = Instant::now();
                    }
                }
                self.is_toggled_on
            }
            // Always-on
            2 => {
                if !self.is_toggled_on {
                    self.is_toggled_on = true;
                    self.activation_time = Instant::now();
                    self.activation_ramp = RAMP_MIN;
                }
                true
            }
            _ => key_pressed,
        }
    }

    #[inline]
    fn ease_out_quad(t: f32) -> f32 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    #[inline]
    fn ease_out_cubic(t: f32) -> f32 {
        let t1 = 1.0 - t;
        1.0 - t1 * t1 * t1
    }

    pub fn get_locked_target_name(&self) -> Option<&str> {
        self.aimbot_locked_target.as_ref().map(|t| t.player_name.as_str())
    }

    fn reset_state_soft(&mut self) {
        self.accumulated_dx = 0.0;
        self.accumulated_dy = 0.0;
        self.aimbot_locked_target = None;
        self.screen_vel_tracker = None;
        self.prev_error_valid = false;
        self.prev_target_screen = None;
        self.target_switch_time = None;
        self.op1_smoothed_screen = None;
    }

    fn reset_state_full(&mut self) {
        self.reset_state_soft();
        self.velocity_trackers.clear();
        self.is_toggled_on = false;
        self.activation_ramp = 0.0;
    }

    fn find_valid_bone(entity: &Entity, preferred_bone: &str) -> Option<BodyPart> {
        let primary_bones: Vec<BodyPart> = match preferred_bone {
            "Head" => vec![BodyPart::Head],
            "Torso" | "UpperTorso" => vec![BodyPart::UpperTorso, BodyPart::Torso],
            "HumanoidRootPart" => vec![BodyPart::HumanoidRootPart],
            _ => vec![BodyPart::Head],
        };

        for bone in primary_bones {
            if let Some(part) = entity.parts.get(&bone) {
                if part.position.is_valid() && !part.position.is_near_origin(1.0) {
                    return Some(bone);
                }
            }
        }

        const FALLBACKS: [BodyPart; 4] = [BodyPart::Head, BodyPart::UpperTorso, BodyPart::Torso, BodyPart::HumanoidRootPart];
        for bone in FALLBACKS {
            if let Some(part) = entity.parts.get(&bone) {
                if part.position.is_valid() && !part.position.is_near_origin(1.0) {
                    return Some(bone);
                }
            }
        }

        None
    }

    #[inline]
    fn predict_position_quadratic(
        current: Vector3,
        velocity: Vector3,
        acceleration: Vector3,
        prediction_ms: f32,
    ) -> Vector3 {
        if velocity.length_squared() < VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
            return current;
        }

        let t = prediction_ms / 1000.0;
        let t2 = t * t;

        let accel_factor = 0.5;
        let max_accel = 50.0;
        let ax = acceleration.x.clamp(-max_accel, max_accel) * accel_factor;
        let ay = acceleration.y.clamp(-max_accel, max_accel) * accel_factor;
        let az = acceleration.z.clamp(-max_accel, max_accel) * accel_factor;
        
        Vector3::new(
            current.x + velocity.x * t + 0.5 * ax * t2,
            current.y + velocity.y * t + 0.5 * ay * t2,
            current.z + velocity.z * t + 0.5 * az * t2,
        )
    }
}