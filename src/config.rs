//! Configuration management module.
//! Handles loading, saving, and runtime modification of all settings.
//! Configuration is stored in TOML format for human readability.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
    
    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
}

// ============================================================================
// Configuration Structures
// ============================================================================

/// Current config schema version. Bump this when breaking changes occur.
pub const CONFIG_VERSION: u32 = 2;

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Config schema version for migration detection
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    pub general: GeneralConfig,
    pub visuals: VisualsConfig,
    pub aimbot: AimbotConfig,
    pub triggerbot: TriggerbotConfig,
    #[serde(default)]
    pub camera_aim: CameraAimConfig,
    #[serde(default)]
    pub silent_aim: SilentAimConfig,
    #[serde(default)]
    pub viewport_aim: ViewportAimConfig,
    pub movement: MovementConfig,
    pub world: WorldConfig,
    pub camera: CameraConfig,
    pub interface: InterfaceConfig,
    pub performance: PerformanceConfig,
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub hotkey_bindings: HotkeyBindings,
    #[serde(default)]
    pub autoclicker: AutoClickerConfig,
    #[serde(default)]
    pub hitbox: HitboxConfig,
    #[serde(default)]
    pub anti_afk: AntiAfkConfig,
    #[serde(default)]
    pub desync: DesyncConfig,
    #[serde(default)]
    pub cosmetics: CosmeticsConfig,
    #[serde(default)]
    pub blade_ball: BladeBallConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub username: String,
    pub process_name: String,
    pub window_title: String,
    pub startup_delay_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualsConfig {
    pub box_esp: bool,
    pub name_tags: bool,
    pub health_bars: bool,
    pub armor_bars: bool, // Show armor bars on ESP (only displayed if armor available)
    pub tracers: bool,
    pub chams: bool,
    /// Mesh chams mode — outline-only per-part silhouette (lighter than filled chams)
    #[serde(default)]
    pub mesh_chams: bool,
    /// When true, mesh chams draw a faint colour fill inside outlines
    #[serde(default)]
    pub mesh_chams_fill: bool,
    pub distance_colors: bool,
    pub target_highlight: bool,
    pub max_distance: f32,
    #[serde(default)]
    pub team_check: bool,
    #[serde(default)]
    pub hide_dead: bool,
    #[serde(default)]
    pub show_bots: bool,
    /// Wall check - show visible enemies in green, hidden in red
    #[serde(default)]
    pub wall_check: bool,
    /// Whitelist of player names marked as teammates (shown in blue)
    #[serde(default)]
    pub teammate_whitelist: Vec<String>,
    /// Box style: 0 = Full box, 1 = Corners only, 2 = 3D box
    #[serde(default)]
    pub box_style: u8,
    /// Fill the box with a semi-transparent color
    #[serde(default)]
    pub box_fill: bool,
    /// Box outline color [R, G, B] (0.0 - 1.0) - used when distance_colors is OFF
    #[serde(default = "default_box_color")]
    pub box_color: [f32; 3],
    /// Box fill color [R, G, B] (0.0 - 1.0) - independent from outline
    #[serde(default = "default_box_fill_color")]
    pub box_fill_color: [f32; 3],
    /// Box fill opacity (0.0 - 1.0)
    #[serde(default = "default_box_fill_opacity")]
    pub box_fill_opacity: f32,
    /// Crosshair overlay style: 0 = off, 1 = cross, 2 = dot, 3 = circle+dot, 4 = cross+dot
    #[serde(default)]
    pub crosshair_style: u8,
    /// Crosshair color [R, G, B] (0.0 - 1.0)
    #[serde(default = "default_crosshair_color")]
    pub crosshair_color: [f32; 3],
    /// Crosshair size in pixels
    #[serde(default = "default_crosshair_size")]
    pub crosshair_size: f32,
    /// Crosshair thickness
    #[serde(default = "default_crosshair_thickness")]
    pub crosshair_thickness: f32,
    /// Crosshair gap (space in center)
    #[serde(default = "default_crosshair_gap")]
    pub crosshair_gap: f32,
    /// Footprint ESP - show where players have walked
    #[serde(default)]
    pub footprints: bool,
    /// Movement trails behind players
    #[serde(default)]
    pub movement_trails: bool,
    /// Show ESP preview demo in the visuals tab
    #[serde(default)]
    pub show_esp_preview: bool,
    /// ESP preview rotation angle (for rotating the humanoid model)
    #[serde(default)]
    pub esp_preview_rotation: f32,
    /// ESP preview demo health percentage (0.0 - 1.0)
    #[serde(default = "default_esp_preview_health")]
    pub esp_preview_health: f32,
    /// ESP preview demo armor percentage (0.0 - 1.0)
    #[serde(default = "default_esp_preview_armor")]
    pub esp_preview_armor: f32,
    /// ESP preview flipped (mirrored horizontally)
    #[serde(default)]
    pub esp_preview_flipped: bool,
    /// ESP preview wall-occluded demo mode (shows occluded colors)
    #[serde(default)]
    pub esp_preview_wall_occluded: bool,
    /// ESP intensity scale (0.0 = minimum performance impact, 1.0 = maximum quality)
    /// Controls update rates, cache lifetimes, max entities, and thread timing.
    #[serde(default = "default_esp_intensity")]
    pub esp_intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimbotConfig {
    pub enabled: bool,
    pub fov: f32,
    pub smoothing: f32,
    pub show_fov: bool,
    pub target_bone: String,
    pub prediction_enabled: bool,
    pub prediction_amount: f32,
    #[serde(default)]
    pub mode: u8, // Activation mode: 0 = Hold (recommended), 1 = Toggle, 2 = Always-on when enabled
    #[serde(default)]
    pub activation_mode: u8, // Hold delay in ms - aim only activates after holding key for this duration
    #[serde(default = "default_hold_delay")]
    pub hold_delay_ms: u32, // Auto Reload - automatically presses R when ammo is 0
    #[serde(default)]
    pub auto_reload: bool,
}

fn default_config_version() -> u32 { CONFIG_VERSION }

fn default_hold_delay() -> u32 {
    50 // 50ms delay before aim activates - faster response while still allowing quick look
}

/// Triggerbot - auto-fire configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerbotConfig {
    pub enabled: bool,
    pub delay_ms: f32,
    /// Trigger radius in pixels - how close crosshair must be to target
    #[serde(default = "default_trigger_radius")]
    pub trigger_radius: f32,
    /// Wall check - only fire if target is visible (not behind walls)
    #[serde(default = "default_true")]
    pub wall_check: bool,
}

fn default_trigger_radius() -> f32 {
    8.0
}

/// Camera Aim configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CameraAimConfig {
    pub enabled: bool, // FOV radius for camera aim
    #[serde(default = "default_camera_aim_fov")]
    pub fov: f32, // Show FOV circle
    #[serde(default)]
    pub show_fov: bool, // Target bone: Head, UpperTorso, HumanoidRootPart
    #[serde(default = "default_target_bone")]
    pub target_bone: String,
}

fn default_camera_aim_fov() -> f32 { 150.0 }
fn default_target_bone() -> String { "Head".to_string() }
fn default_esp_preview_health() -> f32 { 0.75 }
fn default_esp_preview_armor() -> f32 { 0.5 }
fn default_esp_intensity() -> f32 { 0.75 }
fn default_box_color() -> [f32; 3] { [1.0, 0.0, 0.0] } // Default red
fn default_box_fill_color() -> [f32; 3] { [1.0, 0.0, 0.0] } // Default red
fn default_box_fill_opacity() -> f32 { 0.15 }
fn default_crosshair_color() -> [f32; 3] { [0.0, 1.0, 0.0] } // Green 
fn default_crosshair_size() -> f32 { 6.0 }
fn default_crosshair_thickness() -> f32 { 1.5 }
fn default_crosshair_gap() -> f32 { 3.0 }

/// Silent Aim configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilentAimConfig {
    /// Enable silent aim
    pub enabled: bool,
    
    /// FOV radius for target selection (pixels)
    #[serde(default = "default_silent_aim_fov")]
    pub fov: f32,
    
    /// Use FOV restriction
    #[serde(default)]
    pub use_fov: bool,
    
    /// Show FOV circle
    #[serde(default)]
    pub show_fov: bool,
    
    /// Target selection method: 0 = closest to mouse, 1 = closest to camera
    #[serde(default)]
    pub target_method: u8,
    
    /// Body part selection mode: 0 = fixed, 1 = closest part, 2 = closest point, 3 = random
    #[serde(default)]
    pub body_part_mode: u8,
    
    /// Target bone index (when body_part_mode = 0): 0=Head, 1=UpperTorso, etc.
    #[serde(default)]
    pub target_bone: u8,
    
    /// Sticky aim - lock on target until death
    #[serde(default)]
    pub sticky_aim: bool,
    
    /// Team check - skip teammates
    #[serde(default = "default_true")]
    pub team_check: bool,
    
    /// Unlock on target death
    #[serde(default = "default_true")]
    pub unlock_on_death: bool,
    
    /// Enable velocity prediction
    #[serde(default)]
    pub prediction_enabled: bool,
    
    /// Prediction time in milliseconds
    #[serde(default = "default_silent_prediction")]
    pub prediction_amount: f32,
    
    /// Activation mode: 0 = hold LMB, 1 = toggle, 2 = always on
    #[serde(default)]
    pub activation_mode: u8,
    
    /// Show debug overlay
    #[serde(default)]
    pub show_debug: bool,
}

fn default_silent_aim_fov() -> f32 { 200.0 }
fn default_silent_prediction() -> f32 { 50.0 }

/// Viewport Aim configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViewportAimConfig {
    /// Enable viewport aim
    pub enabled: bool,
    
    /// FOV radius for target selection (pixels)
    #[serde(default = "default_viewport_aim_fov")]
    pub fov: f32,
    
    /// Use FOV restriction
    #[serde(default)]
    pub use_fov: bool,
    
    /// Show FOV circle
    #[serde(default)]
    pub show_fov: bool,
    
    /// Target bone: 0=Head, 1=UpperTorso, 2=LowerTorso, 3=HumanoidRootPart
    #[serde(default)]
    pub target_bone: u8,
}

fn default_viewport_aim_fov() -> f32 { 200.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementConfig {
    pub jump_power: f32,
    pub walk_speed: f32,
    pub fly_enabled: bool,
    pub fly_speed: f32,
    pub noclip_enabled: bool,
    pub auto_jump: bool,
    #[serde(default = "default_write_intensity")]// Write intensity for movement features (1=low/fast, 2=medium, 3=high/stable
    pub write_intensity: u8, 
    #[serde(default)]
    pub fly_mode: u8,
    #[serde(default)]
    pub spinbot_enabled: bool,
    #[serde(default = "default_spinbot_speed")]
    pub spinbot_speed: f32,
    #[serde(default)]
    pub anti_sit_enabled: bool,
    #[serde(default)]
    pub hip_height_enabled: bool,
    #[serde(default = "default_hip_height")]
    pub hip_height_value: f32,     // Hip Height value (default is ~2.0)
    #[serde(default)]
    pub void_hide_enabled: bool,
    // Vehicle Fly - fly while seated in a vehicle
    #[serde(default)]
    pub vehicle_fly_enabled: bool,
    #[serde(default = "default_vehicle_fly_speed")]
    pub vehicle_fly_speed: f32,
    /// No Fall Damage - clamp fall velocity to prevent damage
    #[serde(default)]
    pub no_fall_damage: bool,
    /// Spiderman - climb walls when walking into them
    #[serde(default)]
    pub spiderman: bool,
    /// Spiderman climb speed
    #[serde(default = "default_spiderman_speed")]
    pub spiderman_speed: f32,
    /// Click Teleport - teleport to clicked position
    #[serde(default)]
    pub click_teleport: bool,
    /// Click Teleport activation key (VK code, default = right mouse button)
    #[serde(default = "default_click_teleport_key")]
    pub click_teleport_key: u32,
    /// Anchor - lock character in place
    #[serde(default)]
    pub anchor_enabled: bool,
    /// Waypoint system - save/restore positions
    #[serde(default)]
    pub waypoint_enabled: bool,
    /// Waypoint save key (VK code)
    #[serde(default = "default_waypoint_save_key")]
    pub waypoint_save_key: u32,
    /// Waypoint teleport key (VK code)
    #[serde(default = "default_waypoint_tp_key")]
    pub waypoint_tp_key: u32,
}

fn default_spinbot_speed() -> f32 { 6.0 } // 6 rotations/sec
fn default_hip_height() -> f32 { 2.0 }
fn default_write_intensity() -> u8 { 3 } // High by default for stability
fn default_vehicle_fly_speed() -> f32 { 50.0 } // Vehicle fly speed
fn default_spiderman_speed() -> f32 { 30.0 } // Spiderman climb speed
fn default_click_teleport_key() -> u32 { 0x02 } // VK_RBUTTON (right mouse button)
fn default_waypoint_save_key() -> u32 { 0x67 } // Numpad7
fn default_waypoint_tp_key() -> u32 { 0x68 } // Numpad8

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    pub anti_fog: bool,
    pub fog_start: f32,
    pub fog_end: f32,
    pub brightness_enabled: bool,
    pub brightness_value: f32,
    pub anti_flash: bool,
    pub max_brightness: f32,
    /// Fullbright - Maximum lighting (white ambient, high brightness)
    #[serde(default)]
    pub fullbright: bool,
    /// Force Lighting - Override all lighting properties
    #[serde(default)]
    pub force_lighting: bool,
    /// Force Lighting - Ambient color RGB
    #[serde(default = "default_ambient_color")]
    pub ambient_color: [f32; 3],
    /// Force Lighting - Outdoor ambient color RGB
    #[serde(default = "default_ambient_color")]
    pub outdoor_ambient_color: [f32; 3],
    /// Force Lighting - Clock time (0-24)
    #[serde(default = "default_clock_time")]
    pub clock_time: f32,
    /// Force Lighting - No shadows
    #[serde(default)]
    pub no_shadows: bool,
    /// Terrain Control - enabled
    #[serde(default)]
    pub terrain_enabled: bool,
    /// Terrain Control - Grass length (0.0 - 1.0)
    #[serde(default = "default_grass_length")]
    pub grass_length: f32,
    /// Terrain Control - Water transparency (0.0 - 1.0)
    #[serde(default = "default_water_transparency")]
    pub water_transparency: f32,
    /// Terrain Control - Water color RGB
    #[serde(default = "default_water_color")]
    pub water_color: [f32; 3],
    
    // === Post-processing effects (from dynamic offsets) ===
    
    /// Atmosphere control - enabled
    #[serde(default)]
    pub atmosphere_enabled: bool,
    /// Atmosphere - Density (0.0 - 1.0)
    #[serde(default = "default_atmosphere_density")]
    pub atmosphere_density: f32,
    /// Atmosphere - Haze (0.0 - 1.0)
    #[serde(default = "default_atmosphere_haze")]
    pub atmosphere_haze: f32,
    /// Atmosphere - Glare (0.0 - 1.0)
    #[serde(default = "default_atmosphere_glare")]
    pub atmosphere_glare: f32,
    /// Atmosphere - Offset (default 0)
    #[serde(default)]
    pub atmosphere_offset: f32,
    /// Atmosphere - Color RGB
    #[serde(default = "default_atmosphere_color")]
    pub atmosphere_color: [f32; 3],

    /// Bloom effect control - enabled
    #[serde(default)]
    pub bloom_enabled: bool,
    /// Bloom - Enable/Disable the bloom effect in-game
    #[serde(default = "default_true_val")]
    pub bloom_active: bool,
    /// Bloom - Intensity (0.0 - 3.0)
    #[serde(default = "default_bloom_intensity")]
    pub bloom_intensity: f32,
    /// Bloom - Size (0.0 - 100.0)
    #[serde(default = "default_bloom_size")]
    pub bloom_size: f32,
    /// Bloom - Threshold (0.0 - 5.0)
    #[serde(default = "default_bloom_threshold")]
    pub bloom_threshold: f32,

    /// Depth of Field control - enabled
    #[serde(default)]
    pub dof_enabled: bool,
    /// DOF - Enable/Disable the DOF effect in-game
    #[serde(default = "default_true_val")]
    pub dof_active: bool,
    /// DOF - Far Intensity (0.0 - 1.0)
    #[serde(default = "default_dof_far_intensity")]
    pub dof_far_intensity: f32,
    /// DOF - Focus Distance (0 - 1000)
    #[serde(default = "default_dof_focus_distance")]
    pub dof_focus_distance: f32,
    /// DOF - In-Focus Radius (0 - 500)
    #[serde(default = "default_dof_in_focus_radius")]
    pub dof_in_focus_radius: f32,
    /// DOF - Near Intensity (0.0 - 1.0)
    #[serde(default = "default_dof_near_intensity")]
    pub dof_near_intensity: f32,

    /// Sun Rays control - enabled
    #[serde(default)]
    pub sunrays_enabled: bool,
    /// Sun Rays - Enable/Disable the sun rays effect in-game
    #[serde(default = "default_true_val")]
    pub sunrays_active: bool,
    /// Sun Rays - Intensity (0.0 - 1.0)
    #[serde(default = "default_sunrays_intensity")]
    pub sunrays_intensity: f32,
    /// Sun Rays - Spread (0.0 - 1.0)
    #[serde(default = "default_sunrays_spread")]
    pub sunrays_spread: f32,
}

fn default_ambient_color() -> [f32; 3] { [1.0, 1.0, 1.0] }
fn default_clock_time() -> f32 { 14.0 }
fn default_grass_length() -> f32 { 0.1 }
fn default_water_transparency() -> f32 { 0.5 }
fn default_water_color() -> [f32; 3] { [0.2, 0.5, 0.8] }

// Post-processing defaults
fn default_true_val() -> bool { true }
fn default_atmosphere_density() -> f32 { 0.395 }
fn default_atmosphere_haze() -> f32 { 0.0 }
fn default_atmosphere_glare() -> f32 { 0.0 }
fn default_atmosphere_color() -> [f32; 3] { [0.69, 0.73, 0.79] }
fn default_bloom_intensity() -> f32 { 0.4 }
fn default_bloom_size() -> f32 { 24.0 }
fn default_bloom_threshold() -> f32 { 0.95 }
fn default_dof_far_intensity() -> f32 { 0.75 }
fn default_dof_focus_distance() -> f32 { 100.0 }
fn default_dof_in_focus_radius() -> f32 { 30.0 }
fn default_dof_near_intensity() -> f32 { 0.75 }
fn default_sunrays_intensity() -> f32 { 0.25 }
fn default_sunrays_spread() -> f32 { 1.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub fov_enabled: bool,
    pub fov_value: f32,
    /// Free Camera - detach camera from player and fly around
    #[serde(default)]
    pub free_camera_enabled: bool,
    /// Free Camera speed
    #[serde(default = "default_free_camera_speed")]
    pub free_camera_speed: f32,
}

fn default_free_camera_speed() -> f32 { 50.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub show_hotkey_hints: bool,
    #[serde(default = "default_hotkey_x")]
    pub hotkey_pos_x: f32,
    #[serde(default = "default_hotkey_y")]
    pub hotkey_pos_y: f32,
    #[serde(default = "default_hotkey_anchor")]
    pub hotkey_anchor: String, // "top-left", "top-right", "bottom-left", "bottom-right"
    /// Currently expanded aim section: 0=none, 1=aim assist, 2=triggerbot, 3=camera aim, 4=mouse pos aim
    #[serde(default)]
    pub expanded_aim_section: u8,
    /// Accent color RGB values (customizable)
    #[serde(default = "default_accent_r")]
    pub accent_r: u8,
    #[serde(default = "default_accent_g")]
    pub accent_g: u8,
    #[serde(default = "default_accent_b")]
    pub accent_b: u8,
}

fn default_hotkey_x() -> f32 { 10.0 }
fn default_hotkey_y() -> f32 { 10.0 }
fn default_hotkey_anchor() -> String { "top-left".to_string() }
fn default_accent_r() -> u8 { 100 }
fn default_accent_g() -> u8 { 100 }
fn default_accent_b() -> u8 { 220 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub target_fps: u32,
    pub cache_update_ms: u64,
    pub idle_mode: bool,
    pub idle_fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntiAfkConfig {
    pub enabled: bool,
    #[serde(default = "default_anti_afk_interval")]
    pub interval_secs: u32,
}

fn default_anti_afk_interval() -> u32 {
    60 // Default 60 seconds
}

/// Desync activation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesyncMode {
    /// Classic: hold key to freeze, release to unfreeze
    Hold,
    /// Press once to activate, press again to deactivate
    Toggle,
    /// Press once to activate, auto-releases after `auto_release_secs`
    Timed,
}

impl Default for DesyncMode {
    fn default() -> Self {
        Self::Hold
    }
}

/// Desync replication strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesyncStrength {
    /// Full freeze: bandwidth → 0 (classic, works on client-auth games)
    FullFreeze,
    /// Throttled: bandwidth → very low value (creates lag-like desync,
    /// works better on server-auth games that validate position)
    Throttled,
}

impl Default for DesyncStrength {
    fn default() -> Self {
        Self::FullFreeze
    }
}

/// Desync configuration - network replication manipulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesyncConfig {
    /// Enable desync feature (master toggle)
    pub enabled: bool,
    /// Keybind to activate desync (VK code, 0 = disabled)
    #[serde(default = "default_desync_keybind")]
    pub keybind: u32,
    /// Activation mode: Hold, Toggle, or Timed
    #[serde(default)]
    pub mode: DesyncMode,
    /// Replication strength: FullFreeze (bandwidth=0) or Throttled (low bandwidth)
    #[serde(default)]
    pub strength: DesyncStrength,
    /// Throttled mode bandwidth in bytes/sec (50–2000). Lower = more desync,
    /// higher = more responsive but weaker effect. Only used in Throttled mode.
    #[serde(default = "default_throttle_bps")]
    pub throttle_bps: i32,
    /// For Timed mode: how many seconds before auto-release (2.0–4.0)
    #[serde(default = "default_auto_release_secs")]
    pub auto_release_secs: f32,
    /// Strong mode: also freezes WorldStepMax to fully halt physics simulation.
    /// Strong mode — also freezes physics simulation for maximum effect.
    #[serde(default)]
    pub strong_mode: bool,
}

fn default_desync_keybind() -> u32 {
    0x14 // CapsLock - easy to tap while on WASD
}

fn default_auto_release_secs() -> f32 {
    3.0
}

fn default_throttle_bps() -> i32 {
    500 // 500 bytes/sec — severe throttle but not fully frozen
}

impl Default for DesyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keybind: default_desync_keybind(),
            mode: DesyncMode::default(),
            strength: DesyncStrength::default(),
            throttle_bps: default_throttle_bps(),
            auto_release_secs: default_auto_release_secs(),
            strong_mode: false,
        }
    }
}

/// Cosmetics configuration - visual character modifications
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CosmeticsConfig {
    /// Fake Korblox leg - makes right leg transparent and changes mesh
    #[serde(default)]
    pub korblox: bool,
    /// Headless - makes head invisible
    #[serde(default)]
    pub headless: bool,
    /// Face hide - makes face transparent
    #[serde(default)]
    pub hide_face: bool,
    /// Arsenal skin changer - enable melee skin modification
    #[serde(default)]
    pub arsenal_enabled: bool,
    /// Arsenal selected melee skin name
    #[serde(default = "default_arsenal_melee")]
    pub arsenal_melee_skin: String,
    /// Arsenal swing fix - reverts to default knife when swinging
    #[serde(default = "default_true")]
    pub arsenal_swing_fix: bool,
}

fn default_arsenal_melee() -> String {
    "Default".to_string()
}

/// Which input to simulate when auto-parrying in Blade Ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParryInput {
    /// Press the F key (default Blade Ball parry keybind)
    FKey,
    /// Left mouse click
    LeftClick,
}

impl Default for ParryInput {
    fn default() -> Self {
        Self::FKey
    }
}

/// Blade Ball auto-parry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BladeBallConfig {
    /// Enable auto-parry (master toggle)
    #[serde(default)]
    pub enabled: bool,
    /// Distance threshold in studs — max range at which parry can trigger.
    /// V5: Increased to 200. With close_enough removed, this is now a sanity
    /// cap, not a trigger. TTI-based logic handles actual timing.
    #[serde(default = "default_parry_distance")]
    pub parry_distance: f32,
    /// Which input to send when parrying
    #[serde(default)]
    pub parry_input: ParryInput,
    /// Show debug overlay with ball tracking info
    #[serde(default)]
    pub debug_overlay: bool,
}

fn default_parry_distance() -> f32 {
    200.0
}

impl Default for BladeBallConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            parry_distance: default_parry_distance(),
            parry_input: ParryInput::default(),
            debug_overlay: false,
        }
    }
}

/// System hotkeys configuration (not configurable via Hotkeys tab)
/// Feature toggles are now handled by HotkeyBindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// F1 - Toggle menu
    pub menu_toggle: u32,
    /// F9 - Reload player cache
    pub reload_data: u32,
    /// F12 - Exit application
    pub exit: u32,
    /// RMB - Aim activation key
    pub aim_key: u32,
    /// Insert - Autoclicker toggle
    #[serde(default = "default_autoclicker_toggle")]
    pub autoclicker_toggle: u32,
    /// Home - Full game instance refresh
    #[serde(default = "default_full_refresh")]
    pub full_refresh: u32,
    /// End - Save config to file
    #[serde(default = "default_save_config")]
    pub save_config: u32,
}

fn default_autoclicker_toggle() -> u32 {
    0x2D // Insert
}

fn default_full_refresh() -> u32 {
    0x24 // Home
}

fn default_save_config() -> u32 {
    0x23 // End
}

/// Features that can be bound to hotkeys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BindableFeature {
    #[default]
    None,
    // Visuals
    BoxEsp,
    NameTags,
    Tracers,
    HealthBars,
    ArmourBars,
    Chams,
    TeamCheck,
    HideDead,
    ShowBots,
    FreeCamera,
    // Aim
    AimAssist,
    Triggerbot,
    CameraAim,
    ViewportAim,
    SilentAim,
    AutoReload,
    // Movement
    Fly,
    Noclip,
    Spinbot,
    AntiSit,
    VoidHide,
    VehicleFly,
    Spiderman,
    NoFallDamage,
    ClickTeleport,
    AutoJump,
    HipHeight,
    // Hitbox
    HitboxMod,
    ShowHitboxVisual,
    // World
    Fullbright,
    // Camera
    CameraFov,
    // Cosmetics
    Korblox,
    Headless,
    // Misc
    AntiAfk,
    AutoClicker,
    BladeBall,
    Desync,
    WallCheck,
    Footprints,
    MovementTrails,
    Anchor,
    Waypoint,
}

impl BindableFeature {
    /// Get display name for the feature
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "-- None --",
            Self::BoxEsp => "Box ESP",
            Self::NameTags => "Name Tags",
            Self::Tracers => "Tracers",
            Self::HealthBars => "Health Bars",
            Self::ArmourBars => "Armour Bars",
            Self::Chams => "Chams",
            Self::TeamCheck => "Team Check",
            Self::HideDead => "Hide Dead",
            Self::ShowBots => "Show Bots",
            Self::FreeCamera => "Free Camera",
            Self::AimAssist => "Aim Assist",
            Self::Triggerbot => "Triggerbot",
            Self::CameraAim => "Camera Aim",
            Self::ViewportAim => "Viewport Aim",
            Self::SilentAim => "Silent Aim (Mouse)",
            Self::AutoReload => "Auto Reload",
            Self::Fly => "Fly",
            Self::Noclip => "Noclip",
            Self::Spinbot => "Spinbot",
            Self::AntiSit => "Anti-Sit",
            Self::VoidHide => "Void Hide",
            Self::VehicleFly => "Vehicle Fly",
            Self::Spiderman => "Spiderman",
            Self::NoFallDamage => "No Fall Damage",
            Self::ClickTeleport => "Click Teleport",
            Self::AutoJump => "Auto Jump",
            Self::HipHeight => "Hip Height",
            Self::HitboxMod => "Hitbox Mod",
            Self::ShowHitboxVisual => "Show Hitbox Visual",
            Self::Fullbright => "Fullbright",
            Self::CameraFov => "Camera FOV",
            Self::Korblox => "Korblox",
            Self::Headless => "Headless",
            Self::AntiAfk => "Anti-AFK",
            Self::AutoClicker => "Auto Clicker",
            Self::BladeBall => "Blade Ball",
            Self::Desync => "Desync",
            Self::WallCheck => "Wall Check",
            Self::Footprints => "Footprints",
            Self::MovementTrails => "Movement Trails",
            Self::Anchor => "Anchor",
            Self::Waypoint => "Waypoint",
        }
    }
    
    /// Get category for grouping in dropdown
    pub fn category(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::BoxEsp | Self::NameTags | Self::Tracers | Self::HealthBars | 
            Self::ArmourBars | Self::Chams | Self::TeamCheck | Self::HideDead | Self::ShowBots | Self::FreeCamera => "VISUAL",
            Self::AimAssist | Self::Triggerbot | Self::CameraAim | Self::ViewportAim | Self::SilentAim | Self::AutoReload => "AIM",
            Self::Fly | Self::Noclip | Self::Spinbot | Self::AntiSit | Self::VoidHide | Self::VehicleFly |
            Self::Spiderman | Self::NoFallDamage | Self::ClickTeleport | Self::AutoJump | Self::HipHeight => "MOVEMENT",
            Self::HitboxMod | Self::ShowHitboxVisual => "HITBOX",
            Self::Fullbright => "WORLD",
            Self::CameraFov => "CAMERA",
            Self::Korblox | Self::Headless => "COSMETICS",
            Self::AntiAfk | Self::AutoClicker | Self::BladeBall | Self::Desync => "MISC",
            Self::WallCheck | Self::Footprints | Self::MovementTrails => "VISUAL",
            Self::Anchor | Self::Waypoint => "MOVEMENT",
        }
    }
    
    /// Get all features in order for dropdown
    pub fn all_features() -> &'static [BindableFeature] {
        &[
            Self::None,
            // Visuals
            Self::BoxEsp,
            Self::NameTags,
            Self::Tracers,
            Self::HealthBars,
            Self::ArmourBars,
            Self::Chams,
            Self::TeamCheck,
            Self::HideDead,
            Self::ShowBots,
            Self::FreeCamera,
            // Aim
            Self::AimAssist,
            Self::Triggerbot,
            Self::CameraAim,
            Self::ViewportAim,
            Self::SilentAim,
            Self::AutoReload,
            // Movement
            Self::Fly,
            Self::Noclip,
            Self::Spinbot,
            Self::AntiSit,
            Self::VoidHide,
            Self::VehicleFly,
            Self::Spiderman,
            Self::NoFallDamage,
            Self::ClickTeleport,
            Self::AutoJump,
            Self::HipHeight,
            // Hitbox
            Self::HitboxMod,
            Self::ShowHitboxVisual,
            // World
            Self::Fullbright,
            // Camera
            Self::CameraFov,
            // Cosmetics
            Self::Korblox,
            Self::Headless,
            // Misc
            Self::AntiAfk,
            Self::AutoClicker,
            Self::BladeBall,
            Self::Desync,
            Self::WallCheck,
            Self::Footprints,
            Self::MovementTrails,
            Self::Anchor,
            Self::Waypoint,
        ]
    }
}

/// Virtual key codes for hotkey binding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HotkeyKey {
    #[default]
    None,
    // Mouse buttons
    MouseLeft, MouseRight, MouseMiddle, Mouse4, Mouse5,
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Insert, Delete, Home, End, PageUp, PageDown,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, 
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
}

impl HotkeyKey {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::MouseLeft => "LMB", Self::MouseRight => "RMB", Self::MouseMiddle => "MMB",
            Self::Mouse4 => "M4", Self::Mouse5 => "M5",
            Self::F1 => "F1", Self::F2 => "F2", Self::F3 => "F3", Self::F4 => "F4",
            Self::F5 => "F5", Self::F6 => "F6", Self::F7 => "F7", Self::F8 => "F8",
            Self::F9 => "F9", Self::F10 => "F10", Self::F11 => "F11", Self::F12 => "F12",
            Self::Insert => "Ins", Self::Delete => "Del", 
            Self::Home => "Home", Self::End => "End",
            Self::PageUp => "PgUp", Self::PageDown => "PgDn",
            Self::Numpad0 => "Num0", Self::Numpad1 => "Num1", Self::Numpad2 => "Num2",
            Self::Numpad3 => "Num3", Self::Numpad4 => "Num4", Self::Numpad5 => "Num5",
            Self::Numpad6 => "Num6", Self::Numpad7 => "Num7", Self::Numpad8 => "Num8",
            Self::Numpad9 => "Num9",
        }
    }
    
    pub fn to_vk_code(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::MouseLeft => 0x01, Self::MouseRight => 0x02, Self::MouseMiddle => 0x04,
            Self::Mouse4 => 0x05, Self::Mouse5 => 0x06,
            Self::F1 => 0x70, Self::F2 => 0x71, Self::F3 => 0x72, Self::F4 => 0x73,
            Self::F5 => 0x74, Self::F6 => 0x75, Self::F7 => 0x76, Self::F8 => 0x77,
            Self::F9 => 0x78, Self::F10 => 0x79, Self::F11 => 0x7A, Self::F12 => 0x7B,
            Self::Insert => 0x2D, Self::Delete => 0x2E,
            Self::Home => 0x24, Self::End => 0x23,
            Self::PageUp => 0x21, Self::PageDown => 0x22,
            Self::Numpad0 => 0x60, Self::Numpad1 => 0x61, Self::Numpad2 => 0x62,
            Self::Numpad3 => 0x63, Self::Numpad4 => 0x64, Self::Numpad5 => 0x65,
            Self::Numpad6 => 0x66, Self::Numpad7 => 0x67, Self::Numpad8 => 0x68,
            Self::Numpad9 => 0x69,
        }
    }
    
    pub fn all_keys() -> &'static [HotkeyKey] {
        &[
            Self::None,
            Self::MouseLeft, Self::MouseRight, Self::MouseMiddle, Self::Mouse4, Self::Mouse5,
            Self::F1, Self::F2, Self::F3, Self::F4, Self::F5, Self::F6,
            Self::F7, Self::F8, Self::F9, Self::F10, Self::F11, Self::F12,
            Self::Insert, Self::Delete, Self::Home, Self::End, Self::PageUp, Self::PageDown,
            Self::Numpad0, Self::Numpad1, Self::Numpad2, Self::Numpad3, Self::Numpad4,
            Self::Numpad5, Self::Numpad6, Self::Numpad7, Self::Numpad8, Self::Numpad9,
        ]
    }
}

/// A single hotkey binding slot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySlot {
    pub key: HotkeyKey,
    pub feature: BindableFeature,
}

impl Default for HotkeySlot {
    fn default() -> Self {
        Self {
            key: HotkeyKey::None,
            feature: BindableFeature::None,
        }
    }
}

/// Configurable hotkey bindings (10 slots)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBindings {
    pub slots: [HotkeySlot; 10],
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            slots: [
                HotkeySlot { key: HotkeyKey::F2, feature: BindableFeature::BoxEsp },
                HotkeySlot { key: HotkeyKey::F3, feature: BindableFeature::Chams },
                HotkeySlot { key: HotkeyKey::F4, feature: BindableFeature::AimAssist },
                HotkeySlot { key: HotkeyKey::F5, feature: BindableFeature::CameraAim },
                HotkeySlot { key: HotkeyKey::F6, feature: BindableFeature::Fly },
                HotkeySlot { key: HotkeyKey::F7, feature: BindableFeature::Tracers },
                HotkeySlot { key: HotkeyKey::F8, feature: BindableFeature::Noclip },
                HotkeySlot { key: HotkeyKey::F9, feature: BindableFeature::HitboxMod },
                HotkeySlot { key: HotkeyKey::Insert, feature: BindableFeature::Spinbot },
                HotkeySlot { key: HotkeyKey::None, feature: BindableFeature::None },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoClickerConfig {
    pub enabled: bool,
    pub delay_ms: f32,
    pub variance_percent: f32,
    pub hold_duration_ms: f32,
    #[serde(default)]
    pub turbo_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitboxConfig {
    /// Master enable (backwards compat)
    pub enabled: bool,
    
    // === ENEMY HITBOX ===
    /// Expand enemy hitboxes
    #[serde(default = "default_true")]
    pub enemy_enabled: bool,
    pub head_scale: f32,
    pub torso_scale: f32,
    pub arms_scale: f32,
    pub legs_scale: f32,
    
    // === SELF HITBOX ===
    /// Modify own hitbox (independent of enemy)
    #[serde(default)]
    pub self_enabled: bool,
    /// Scale for self hitbox (0.1-2.0, <1.0 = harder to hit)
    #[serde(default = "default_self_scale")]
    pub self_scale: f32,
    
    // === VISUALS ===
    #[serde(default = "default_hitbox_show_visual")]
    pub show_visual: bool,
    #[serde(default = "default_hitbox_color")]
    pub color: [f32; 4],
}

fn default_true() -> bool {
    true
}

fn default_self_scale() -> f32 {
    1.0
}

fn default_hitbox_show_visual() -> bool {
    true
}

fn default_hitbox_color() -> [f32; 4] {
    [0.0, 1.0, 0.5, 0.4] // Green with 40% alpha
}

// ============================================================================
// Default Implementations
// ============================================================================

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION,
            general: GeneralConfig::default(),
            visuals: VisualsConfig::default(),
            aimbot: AimbotConfig::default(),
            triggerbot: TriggerbotConfig::default(),
            camera_aim: CameraAimConfig::default(),
            silent_aim: SilentAimConfig::default(),
            viewport_aim: ViewportAimConfig::default(),
            movement: MovementConfig::default(),
            world: WorldConfig::default(),
            camera: CameraConfig::default(),
            interface: InterfaceConfig::default(),
            performance: PerformanceConfig::default(),
            hotkeys: HotkeyConfig::default(),
            hotkey_bindings: HotkeyBindings::default(),
            autoclicker: AutoClickerConfig::default(),
            hitbox: HitboxConfig::default(),
            anti_afk: AntiAfkConfig::default(),
            desync: DesyncConfig::default(),
            cosmetics: CosmeticsConfig::default(),
            blade_ball: BladeBallConfig::default(),
        }
    }
}

impl Config {
    /// Reset all active/toggleable features to disabled.
    /// Called on startup so every new session starts clean — settings like
    /// FOV values, smoothing, hotkeys, username etc. are preserved.
    pub fn reset_active_features(&mut self) {
        // Visuals
        self.visuals.box_esp = false;
        self.visuals.name_tags = false;
        self.visuals.health_bars = false;
        self.visuals.armor_bars = false;
        self.visuals.tracers = false;
        self.visuals.chams = false;
        self.visuals.mesh_chams = false;
        self.visuals.mesh_chams_fill = false;
        self.visuals.footprints = false;
        self.visuals.movement_trails = false;
        self.visuals.wall_check = false;
        self.visuals.crosshair_style = 0;

        // Aimbot
        self.aimbot.enabled = false;
        self.aimbot.auto_reload = false;

        // Triggerbot
        self.triggerbot.enabled = false;

        // Camera aim
        self.camera_aim.enabled = false;

        // Silent aim
        self.silent_aim.enabled = false;

        // Viewport aim
        self.viewport_aim.enabled = false;

        // Movement
        self.movement.fly_enabled = false;
        self.movement.noclip_enabled = false;
        self.movement.auto_jump = false;
        self.movement.spinbot_enabled = false;
        self.movement.anti_sit_enabled = false;
        self.movement.hip_height_enabled = false;
        self.movement.void_hide_enabled = false;
        self.movement.vehicle_fly_enabled = false;
        self.movement.no_fall_damage = false;
        self.movement.spiderman = false;
        self.movement.click_teleport = false;
        self.movement.anchor_enabled = false;
        self.movement.waypoint_enabled = false;
        self.movement.jump_power = 50.0;
        self.movement.walk_speed = 16.0;

        // World
        self.world.anti_fog = false;
        self.world.brightness_enabled = false;
        self.world.anti_flash = false;
        self.world.fullbright = false;
        self.world.force_lighting = false;
        self.world.no_shadows = false;
        self.world.terrain_enabled = false;
        self.world.atmosphere_enabled = false;
        self.world.bloom_enabled = false;
        self.world.dof_enabled = false;
        self.world.sunrays_enabled = false;

        // Camera
        self.camera.fov_enabled = false;
        self.camera.free_camera_enabled = false;

        // Hitbox
        self.hitbox.enabled = false;

        // Autoclicker
        self.autoclicker.enabled = false;

        // Anti-AFK
        self.anti_afk.enabled = false;

        // Desync
        self.desync.enabled = false;

        // Cosmetics
        self.cosmetics.korblox = false;
        self.cosmetics.headless = false;
        self.cosmetics.hide_face = false;
        self.cosmetics.arsenal_enabled = false;

        // Blade Ball
        self.blade_ball.enabled = false;
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            username: "YourUsernameHere".to_string(),
            process_name: "RobloxPlayerBeta.exe".to_string(),
            window_title: "Rust Project".to_string(),
            startup_delay_secs: 5,
        }
    }
}

impl Default for VisualsConfig {
    fn default() -> Self {
        Self {
            box_esp: false,
            name_tags: false,
            health_bars: false,
            armor_bars: false, // On by default - only shows when armor is available
            tracers: false,
            chams: false,
            mesh_chams: false,
            mesh_chams_fill: false,
            distance_colors: true,
            target_highlight: true,
            max_distance: 2000.0,
            team_check: false,
            hide_dead: true,
            show_bots: false,
            teammate_whitelist: Vec::new(),
            box_style: 0,         // 0 = Full box, 1 = Corners
            box_fill: false,
            box_color: [1.0, 0.0, 0.0], // Red
            box_fill_color: [1.0, 0.0, 0.0], // Red
            box_fill_opacity: 0.15,
            crosshair_style: 0,
            crosshair_color: default_crosshair_color(),
            crosshair_size: 6.0,
            crosshair_thickness: 1.5,
            crosshair_gap: 3.0,
            footprints: false,
            movement_trails: false,
            show_esp_preview: false,
            esp_preview_rotation: 0.0,
            esp_preview_health: 0.75,
            esp_preview_armor: 0.5,
            esp_preview_flipped: false,
            esp_preview_wall_occluded: false,
            wall_check: false,
            esp_intensity: default_esp_intensity(),
        }
    }
}

impl Default for AimbotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: 0,  // 0 = Mouse, 1 = Camera
            fov: 200.0,
            smoothing: 5.0,       // Lower smoothing for faster acquisition (was 12.0)
            show_fov: true,
            target_bone: "Head".to_string(),
            prediction_enabled: false,
            prediction_amount: 0.02,
            activation_mode: 0,  // 0 = Hold (recommended)
            hold_delay_ms: 50,   // Reduced delay for faster response (was 150ms)
            auto_reload: false,
        }
    }
}

impl Default for TriggerbotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: 0.0,  // Instant fire by default - delay adds latency
            trigger_radius: 12.0,  // Slightly larger radius for better hit registration
            wall_check: true,  // Wall check on by default - don't shoot through walls
        }
    }
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            jump_power: 50.0,
            walk_speed: 16.0,
            fly_enabled: false,
            fly_speed: 25.0,
            noclip_enabled: false,
            auto_jump: false,
            write_intensity: 3, // High by default for stability
            fly_mode: 1, // Mode 1 (position+velocity when not moving) by default
            spinbot_enabled: false,
            spinbot_speed: 6.0,
            anti_sit_enabled: false,
            hip_height_enabled: false,
            hip_height_value: 2.0,
            void_hide_enabled: false,
            vehicle_fly_enabled: false,
            vehicle_fly_speed: 50.0,
            no_fall_damage: false,
            spiderman: false,
            spiderman_speed: 30.0,
            click_teleport: false,
            click_teleport_key: 0x02, // VK_RBUTTON (right mouse button)
            anchor_enabled: false,
            waypoint_enabled: false,
            waypoint_save_key: 0x67,
            waypoint_tp_key: 0x68,
        }
    }
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            anti_fog: false,
            fog_start: 10000.0,
            fog_end: 100000.0,
            brightness_enabled: false,
            brightness_value: 2.0,
            anti_flash: false,
            max_brightness: 3.0,
            fullbright: false,
            force_lighting: false,
            ambient_color: [1.0, 1.0, 1.0],
            outdoor_ambient_color: [1.0, 1.0, 1.0],
            clock_time: 14.0,
            no_shadows: false,
            terrain_enabled: false,
            grass_length: 0.1,
            water_transparency: 0.5,
            water_color: [0.2, 0.5, 0.8],
            // Post-processing defaults
            atmosphere_enabled: false,
            atmosphere_density: default_atmosphere_density(),
            atmosphere_haze: default_atmosphere_haze(),
            atmosphere_glare: default_atmosphere_glare(),
            atmosphere_offset: 0.0,
            atmosphere_color: default_atmosphere_color(),
            bloom_enabled: false,
            bloom_active: true,
            bloom_intensity: default_bloom_intensity(),
            bloom_size: default_bloom_size(),
            bloom_threshold: default_bloom_threshold(),
            dof_enabled: false,
            dof_active: true,
            dof_far_intensity: default_dof_far_intensity(),
            dof_focus_distance: default_dof_focus_distance(),
            dof_in_focus_radius: default_dof_in_focus_radius(),
            dof_near_intensity: default_dof_near_intensity(),
            sunrays_enabled: false,
            sunrays_active: true,
            sunrays_intensity: default_sunrays_intensity(),
            sunrays_spread: default_sunrays_spread(),
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            fov_enabled: false,
            fov_value: 70.0,
            free_camera_enabled: false,
            free_camera_speed: 50.0,
        }
    }
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            show_hotkey_hints: true,
            hotkey_pos_x: 10.0,
            hotkey_pos_y: 10.0,
            hotkey_anchor: "top-left".to_string(),
            expanded_aim_section: 0, // 0 = all collapsed
            accent_r: 100,
            accent_g: 100,
            accent_b: 220,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            target_fps: 70,          // Balanced performance
            cache_update_ms: 8,      // ~120Hz base (high-priority = 1000Hz)
            idle_mode: false,
            idle_fps: 10,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            menu_toggle: 0x70,        // F1
            reload_data: 0x78,        // F9
            exit: 0x7B,               // F12
            aim_key: 0x02,            // RMB
            autoclicker_toggle: 0x2D, // Insert
            full_refresh: 0x24,       // Home
            save_config: 0x23,        // End
        }
    }
}

impl Default for AutoClickerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: 100.0,
            variance_percent: 15.0,
            hold_duration_ms: 30.0,
            turbo_mode: false,
        }
    }
}

impl Default for HitboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enemy_enabled: false,
            head_scale: 4.0,
            torso_scale: 3.5,
            arms_scale: 3.0,
            legs_scale: 3.0,
            self_enabled: false,
            self_scale: 1.0,
            show_visual: true,
            color: default_hitbox_color(),
        }
    }
}

// ============================================================================
// Config Manager
// ============================================================================

/// Thread-safe configuration manager.
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new config manager, loading from file if it exists.
    pub fn new() -> Self {
        let config_path = Self::get_config_path();
        let config = Self::load_or_default(&config_path);
        
        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
        }
    }
    
    /// Get the config file path.
    fn get_config_path() -> PathBuf {
        // Try to find config in current directory, then executable directory
        let local_config = PathBuf::from("config.toml");
        if local_config.exists() {
            return local_config;
        }
        
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_config = exe_dir.join("config.toml");
                if exe_config.exists() {
                    return exe_config;
                }
            }
        }
        
        // Default to current directory
        local_config
    }
    
    /// Create a timestamped backup of the config file.
    /// Returns the backup path on success.
    fn backup_config(path: &PathBuf) -> Option<PathBuf> {
        if !path.exists() {
            return None;
        }
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("config_backup_{}.toml", timestamp);
        let backup_path = path.with_file_name(&backup_name);
        match fs::copy(path, &backup_path) {
            Ok(_) => {
                tracing::info!("✓ Config backed up to {:?}", backup_path);
                Some(backup_path)
            }
            Err(e) => {
                tracing::warn!("Failed to backup config: {}", e);
                None
            }
        }
    }

    /// Load config from file or create default if not exists.
    /// 
    /// Migration strategy:
    /// 1. Try to parse the existing file. Because every field uses `#[serde(default)]`,
    ///    old configs with missing sections/fields will load successfully with defaults filled in.
    /// 2. After a successful load the config is re-saved so the file gains any new fields/sections
    ///    (users can see and edit them).
    /// 3. If parsing fails entirely (e.g. renamed/retyped field), the old file is **backed up**
    ///    before a fresh default is written — the user never silently loses their settings.
    fn load_or_default(path: &PathBuf) -> Config {
        match Self::load_from_file(path) {
            Ok(mut config) => {
                tracing::info!("Loaded configuration from {:?}", path);
                
                // Upgrade version stamp
                let was_old = config.config_version < CONFIG_VERSION;
                if was_old {
                    tracing::info!(
                        "Upgrading config from v{} → v{}",
                        config.config_version, CONFIG_VERSION
                    );
                    config.config_version = CONFIG_VERSION;
                }
                
                // Re-save so newly added fields/sections appear in the file.
                // This makes old configs self-heal without losing existing values.
                match toml::to_string_pretty(&config) {
                    Ok(content) => {
                        if let Err(e) = fs::write(path, &content) {
                            tracing::warn!("Failed to re-save upgraded config: {}", e);
                        } else if was_old {
                            tracing::info!("✓ Config upgraded and saved with new fields");
                        }
                    }
                    Err(e) => tracing::warn!("Failed to serialize config for upgrade: {}", e),
                }
                
                config
            }
            Err(e) => {
                tracing::warn!("Failed to load config: {}", e);
                
                // Backup the broken/old file before overwriting
                if path.exists() {
                    if let Some(backup) = Self::backup_config(path) {
                        tracing::info!(
                            "Old config preserved at {:?} — creating fresh config",
                            backup
                        );
                    }
                }
                
                let config = Config::default();
                
                // Save the default config to create a fresh config.toml
                match toml::to_string_pretty(&config) {
                    Ok(content) => {
                        if let Err(save_err) = fs::write(path, &content) {
                            tracing::error!("Failed to create config file: {}", save_err);
                        } else {
                            tracing::info!("✓ Created fresh config.toml at {:?}", path);
                            tracing::info!("  Edit config.toml and press F9 to reload settings");
                        }
                    }
                    Err(ser_err) => {
                        tracing::error!("Failed to serialize default config: {}", ser_err);
                    }
                }
                
                config
            }
        }
    }
    
    /// Load configuration from a TOML file.
    fn load_from_file(path: &PathBuf) -> Result<Config, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save current configuration to file.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config = self.config.read().unwrap();
        let content = toml::to_string_pretty(&*config)?;
        fs::write(&self.config_path, content)?;
        tracing::info!("Saved configuration to {:?}", self.config_path);
        Ok(())
    }
    
    /// Get a clone of the current configuration.
    pub fn get(&self) -> Config {
        self.config.read().unwrap().clone()
    }
    
    /// Read-only access without cloning. Use for quick reads of specific fields.
    /// The closure receives a reference to the current config. Avoid holding
    /// the lock for long (no I/O, no blocking calls inside the closure).
    #[inline]
    pub fn with_config<R, F: FnOnce(&Config) -> R>(&self, f: F) -> R {
        let guard = self.config.read().unwrap();
        f(&*guard)
    }
    
    /// Get thread-safe reference to config.
    pub fn get_arc(&self) -> Arc<RwLock<Config>> {
        Arc::clone(&self.config)
    }
    
    /// Update configuration with a closure.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Config),
    {
        let mut config = self.config.write().unwrap();
        f(&mut config);
    }

    /// Sync configuration from external source (used after menu modifications).
    pub fn sync(&self, new_config: Config) {
        let mut config = self.config.write().unwrap();
        *config = new_config;
    }
    
    /// Get the local player username.
    pub fn get_username(&self) -> String {
        self.config.read().unwrap().general.username.clone()
    }
    
    /// Get the process name.
    pub fn get_process_name(&self) -> String {
        self.config.read().unwrap().general.process_name.clone()
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.startup_delay_secs, 5);
        assert_eq!(config.aimbot.fov, 200.0);
        assert!(!config.visuals.box_esp);
    }
    
    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.general.username, config.general.username);
    }
}
