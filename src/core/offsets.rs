#![allow(dead_code)]

use std::sync::Mutex;
use ahash::AHashSet;
use crate::core::offset_loader;

/// dedup set for offset-not-found warnings
static WARNED_OFFSETS: std::sync::LazyLock<Mutex<AHashSet<(String, String)>>> =
    std::sync::LazyLock::new(|| Mutex::new(AHashSet::new()));

pub fn client_version() -> String {
    offset_loader::get_version()
}

/// logs a warning once per key, returns 0 if not found
pub(crate) fn get_offset(namespace: &str, name: &str) -> u64 {
    match offset_loader::get_offset(namespace, name) {
        Some(v) => v as u64,
        None => {
            if let Ok(mut set) = WARNED_OFFSETS.lock() {
                let key = (namespace.to_owned(), name.to_owned());
                if set.insert(key) {
                    tracing::warn!("Offset not found: {}::{} (returning 0)", namespace, name);
                }
            }
            0
        }
    }
}

pub(crate) fn try_get_offset(namespace: &str, name: &str) -> Option<u64> {
    offset_loader::get_offset(namespace, name).map(|v| v as u64)
}

/// try multiple namespace:name pairs, return first match
pub(crate) fn try_get_offset_multi(attempts: &[(&str, &str)]) -> Option<u64> {
    for (namespace, name) in attempts {
        if let Some(v) = offset_loader::get_offset(namespace, name) {
            return Some(v as u64);
        }
    }
    None
}

pub mod animation_track {
    use super::get_offset;
    
    pub fn animation() -> u64 { get_offset("AnimationTrack", "Animation") }
    pub fn animator() -> u64 { get_offset("AnimationTrack", "Animator") }
    pub fn is_playing() -> u64 { get_offset("AnimationTrack", "IsPlaying") }
    pub fn looped() -> u64 { get_offset("AnimationTrack", "Looped") }
    pub fn speed() -> u64 { get_offset("AnimationTrack", "Speed") }
}

pub mod base_part {
    use super::{get_offset, try_get_offset, try_get_offset_multi};
    
    // instance-level offsets (applied to instance addr)
    pub fn color3() -> u64 { get_offset("BasePart", "Color3") }
    pub fn primitive() -> u64 { get_offset("BasePart", "Primitive") }
    // applied to primitive addr, not instance addr
    // falls back to CanCollide/Anchored offset (same byte)
    pub fn primitive_flags() -> u64 {
        try_get_offset_multi(&[
            ("BasePart", "PrimitiveFlags"),
            ("BasePart", "CanCollide"),
            ("BasePart", "Anchored"),
        ]).unwrap_or_else(|| {
            use std::sync::atomic::{AtomicBool, Ordering};
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!("Offset not found: BasePart::PrimitiveFlags (and no CanCollide/Anchored fallback)");
            }
            0
        })
    }
    pub fn primitive_owner() -> u64 { get_offset("BasePart", "PrimitiveOwner") }
    pub fn shape() -> u64 { get_offset("BasePart", "Shape") }
    pub fn transparency() -> u64 { get_offset("BasePart", "Transparency") }

    // primitive-level offsets (applied to prim addr)
    // imtheo uses "Primitive" namespace, some secondary sources use "BasePart"
    fn prim_offset(name: &str) -> u64 {
        try_get_offset_multi(&[("Primitive", name), ("BasePart", name)]).unwrap_or_else(|| {
            // Fall through to get_offset for the warning log
            get_offset("BasePart", name)
        })
    }

    pub fn assembly_angular_velocity() -> u64 { prim_offset("AssemblyAngularVelocity") }
    pub fn assembly_linear_velocity() -> u64 { prim_offset("AssemblyLinearVelocity") }
    pub fn material() -> u64 { prim_offset("Material") }
    pub fn position() -> u64 { prim_offset("Position") }
    pub fn rotation() -> u64 { prim_offset("Rotation") }
    pub fn size() -> u64 { prim_offset("Size") }
    pub fn validate_primitive() -> u64 {
        try_get_offset_multi(&[("Primitive", "Validate"), ("Primitive", "ValidatePrimitive"), ("BasePart", "ValidatePrimitive")])
            .unwrap_or_else(|| get_offset("BasePart", "ValidatePrimitive"))
    }
    
    // ntgetwritewatch alias for assembly_linear_velocity
    pub fn velocity() -> u64 { 
        try_get_offset("BasePart", "Velocity").unwrap_or_else(|| assembly_linear_velocity()) 
    }
    
    // falls back to rotation() offset
    pub fn cframe() -> u64 {
        try_get_offset_multi(&[("Primitive", "CFrame"), ("BasePart", "CFrame")])
            .unwrap_or_else(|| rotation())
    }
    
    // same byte as primitive_flags
    pub fn anchored() -> u64 {
        try_get_offset("BasePart", "Anchored").unwrap_or_else(|| primitive_flags())
    }
    
    pub fn can_collide() -> u64 {
        try_get_offset("BasePart", "CanCollide").unwrap_or_else(|| primitive_flags())
    }
    
    pub fn material_type() -> u64 {
        try_get_offset("BasePart", "MaterialType").unwrap_or(0x246)
    }
}

pub mod byte_code {
    use super::get_offset;
    
    pub fn pointer() -> u64 { get_offset("ByteCode", "Pointer") }
    pub fn size() -> u64 { get_offset("ByteCode", "Size") }
}

pub mod camera {
    use super::get_offset;
    
    pub fn camera_subject() -> u64 { get_offset("Camera", "CameraSubject") }
    pub fn camera_type() -> u64 { get_offset("Camera", "CameraType") }
    pub fn field_of_view() -> u64 { get_offset("Camera", "FieldOfView") }
    pub fn position() -> u64 { get_offset("Camera", "Position") }
    pub fn rotation() -> u64 { get_offset("Camera", "Rotation") }
    pub fn viewport() -> u64 { get_offset("Camera", "Viewport") }
}

pub mod click_detector {
    use super::get_offset;
    
    pub fn max_activation_distance() -> u64 { get_offset("ClickDetector", "MaxActivationDistance") }
    pub fn mouse_icon() -> u64 { get_offset("ClickDetector", "MouseIcon") }
}

pub mod datamodel {
    use super::get_offset;
    
    pub fn creator_id() -> u64 { get_offset("DataModel", "CreatorId") }
    pub fn game_id() -> u64 { get_offset("DataModel", "GameId") }
    pub fn game_loaded() -> u64 { get_offset("DataModel", "GameLoaded") }
    pub fn job_id() -> u64 { get_offset("DataModel", "JobId") }
    pub fn place_id() -> u64 { get_offset("DataModel", "PlaceId") }
    pub fn place_version() -> u64 { get_offset("DataModel", "PlaceVersion") }
    pub fn primitive_count() -> u64 { get_offset("DataModel", "PrimitiveCount") }
    pub fn script_context() -> u64 { get_offset("DataModel", "ScriptContext") }
    pub fn server_ip() -> u64 { get_offset("DataModel", "ServerIP") }
    pub fn workspace() -> u64 { get_offset("DataModel", "Workspace") }
}

pub mod fflags {
    use super::try_get_offset;
    
    pub fn debug_disable_timeout_disconnect() -> u64 { try_get_offset("FFlags", "DebugDisableTimeoutDisconnect").unwrap_or(0) }
    pub fn enable_load_module() -> u64 { try_get_offset("FFlags", "EnableLoadModule").unwrap_or(0) }
    pub fn party_player_inactivity_timeout_in_seconds() -> u64 { try_get_offset("FFlags", "PartyPlayerInactivityTimeoutInSeconds").unwrap_or(0) }
    pub fn physics_sender_max_bandwidth_bps() -> u64 { try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBps").unwrap_or(0) }
    pub fn physics_sender_max_bandwidth_bps_scaling() -> u64 { try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBpsScaling").unwrap_or(0) }
    pub fn task_scheduler_target_fps() -> u64 { try_get_offset("FFlags", "TaskSchedulerTargetFps").unwrap_or(0) }
    pub fn web_socket_service_enable_client_creation() -> u64 { try_get_offset("FFlags", "WebSocketServiceEnableClientCreation").unwrap_or(0) }
    pub fn world_step_max() -> u64 { try_get_offset("FFlags", "WorldStepMax").unwrap_or(0) }
    pub fn world_steps_offset_adjust_rate() -> u64 { try_get_offset("FFlags", "WorldStepsOffsetAdjustRate").unwrap_or(0) }

    /// Returns true if FFlag offsets were loaded
    pub fn is_available() -> bool {
        try_get_offset("FFlags", "PhysicsSenderMaxBandwidthBps").is_some()
    }
}

pub mod fake_datamodel {
    use super::get_offset;
    
    pub fn pointer() -> u64 { get_offset("FakeDataModel", "Pointer") }
    pub fn real_datamodel() -> u64 { get_offset("FakeDataModel", "RealDataModel") }
}

pub mod gui_object {
    use super::{get_offset, try_get_offset};
    
    pub fn background_color3() -> u64 { get_offset("GuiObject", "BackgroundColor3") }
    pub fn border_color3() -> u64 { get_offset("GuiObject", "BorderColor3") }
    pub fn image() -> u64 { get_offset("GuiObject", "Image") }
    pub fn layout_order() -> u64 { get_offset("GuiObject", "LayoutOrder") }
    pub fn position() -> u64 { get_offset("GuiObject", "Position") }
    pub fn rich_text() -> u64 { get_offset("GuiObject", "RichText") }
    pub fn rotation() -> u64 { get_offset("GuiObject", "Rotation") }
    pub fn screen_gui_enabled() -> u64 { get_offset("GuiObject", "ScreenGui_Enabled") }
    pub fn size() -> u64 { get_offset("GuiObject", "Size") }
    
    pub fn text() -> u64 { 
        try_get_offset("GuiObject", "Text")
            .or_else(|| try_get_offset("GuiObject", "TextLabelText"))
            .unwrap_or_else(|| {
                tracing::warn!("GuiObject.Text offset not found, using fallback 0xe40");
                0xe40  // Fallback
            })
    }
    
    pub fn text_color3() -> u64 { get_offset("GuiObject", "TextColor3") }
    pub fn visible() -> u64 { get_offset("GuiObject", "Visible") }
}

pub mod humanoid {
    use super::{get_offset, try_get_offset};
    
    pub fn auto_rotate() -> u64 { get_offset("Humanoid", "AutoRotate") }
    pub fn floor_material() -> u64 { get_offset("Humanoid", "FloorMaterial") }
    pub fn health() -> u64 { get_offset("Humanoid", "Health") }
    pub fn hip_height() -> u64 { get_offset("Humanoid", "HipHeight") }
    pub fn humanoid_state() -> u64 { get_offset("Humanoid", "HumanoidState") }
    pub fn humanoid_state_id() -> u64 { get_offset("Humanoid", "HumanoidStateID") }
    pub fn jump() -> u64 { get_offset("Humanoid", "Jump") }
    pub fn jump_height() -> u64 { get_offset("Humanoid", "JumpHeight") }
    pub fn jump_power() -> u64 { get_offset("Humanoid", "JumpPower") }
    pub fn max_health() -> u64 { get_offset("Humanoid", "MaxHealth") }
    pub fn max_slope_angle() -> u64 { get_offset("Humanoid", "MaxSlopeAngle") }
    pub fn move_direction() -> u64 { get_offset("Humanoid", "MoveDirection") }
    pub fn rig_type() -> u64 { get_offset("Humanoid", "RigType") }
    pub fn walkspeed() -> u64 { get_offset("Humanoid", "Walkspeed") }
    pub fn walkspeed_check() -> u64 { get_offset("Humanoid", "WalkspeedCheck") }
    
    // ntgetwritewatch fallbacks
    pub fn sit() -> u64 { 
        try_get_offset("Humanoid", "Sit").unwrap_or(0x1DC)
    }
    
    pub fn is_seat() -> u64 { 
        try_get_offset("Humanoid", "IsSeat").unwrap_or(0x120) 
    }
    
    pub fn platform_stand() -> u64 { 
        try_get_offset("Humanoid", "PlatformStand").unwrap_or(rig_type() + 0x60) 
    }
    
    pub fn ragdoll() -> u64 { 
        try_get_offset("Humanoid", "Ragdoll").unwrap_or(rig_type() + 0xF5) 
    }
    
    pub fn evaluate_state_machine() -> u64 {
        try_get_offset("Humanoid", "EvaluateStateMachine").unwrap_or(0x1DD)
    }
    
    pub fn auto_jump_enabled() -> u64 {
        try_get_offset("Humanoid", "AutoJumpEnabled").unwrap_or(0x1DB)
    }
    
    pub fn root_part_r6() -> u64 {
        try_get_offset("Humanoid", "RootPartR6").unwrap_or(0x4C0)
    }
    
    pub fn root_part_r15() -> u64 {
        try_get_offset("Humanoid", "RootPartR15").unwrap_or(0x620)
    }
    
    pub fn health_display_distance() -> u64 {
        try_get_offset("Humanoid", "HealthDisplayDistance").unwrap_or(0x338)
    }
    
    pub fn name_display_distance() -> u64 {
        try_get_offset("Humanoid", "NameDisplayDistance").unwrap_or(0x344)
    }
}

pub mod instance {
    use super::get_offset;
    
    pub fn attribute_container() -> u64 { get_offset("Instance", "AttributeContainer") }
    pub fn attribute_list() -> u64 { get_offset("Instance", "AttributeList") }
    pub fn attribute_to_next() -> u64 { get_offset("Instance", "AttributeToNext") }
    pub fn attribute_to_value() -> u64 { get_offset("Instance", "AttributeToValue") }
    pub fn children_end() -> u64 { get_offset("Instance", "ChildrenEnd") }
    pub fn children_start() -> u64 { get_offset("Instance", "ChildrenStart") }
    pub fn class_base() -> u64 { get_offset("Instance", "ClassBase") }
    pub fn class_descriptor() -> u64 { get_offset("Instance", "ClassDescriptor") }
    pub fn class_name() -> u64 { get_offset("Instance", "ClassName") }
    pub fn name() -> u64 { get_offset("Instance", "Name") }
    pub fn parent() -> u64 { get_offset("Instance", "Parent") }
}

pub mod lighting {
    use super::{get_offset, try_get_offset};
    
    pub fn ambient() -> u64 { get_offset("Lighting", "Ambient") }
    pub fn brightness() -> u64 { get_offset("Lighting", "Brightness") }
    pub fn clock_time() -> u64 { get_offset("Lighting", "ClockTime") }
    pub fn color_shift_bottom() -> u64 { get_offset("Lighting", "ColorShift_Bottom") }
    pub fn color_shift_top() -> u64 { get_offset("Lighting", "ColorShift_Top") }
    pub fn exposure_compensation() -> u64 { get_offset("Lighting", "ExposureCompensation") }
    pub fn fog_color() -> u64 { get_offset("Lighting", "FogColor") }
    pub fn fog_end() -> u64 { get_offset("Lighting", "FogEnd") }
    pub fn fog_start() -> u64 { get_offset("Lighting", "FogStart") }
    pub fn geographic_latitude() -> u64 { get_offset("Lighting", "GeographicLatitude") }
    pub fn outdoor_ambient() -> u64 { get_offset("Lighting", "OutdoorAmbient") }
    pub fn global_shadows() -> u64 { try_get_offset("Lighting", "GlobalShadows").unwrap_or(0x148) }
    pub fn shadow_color() -> u64 { try_get_offset("Lighting", "ShadowColor").unwrap_or(0x114) }
    pub fn environment_diffuse_scale() -> u64 { try_get_offset("Lighting", "EnvironmentDiffuseScale").unwrap_or(0x10c) }
    pub fn environment_specular_scale() -> u64 { try_get_offset("Lighting", "EnvironmentSpecularScale").unwrap_or(0x128) }
}

pub mod terrain {
    use super::get_offset;
    
    pub fn grass_length() -> u64 { get_offset("Terrain", "GrassLength") }
    pub fn water_color() -> u64 { get_offset("Terrain", "WaterColor") }
    pub fn water_reflectance() -> u64 { get_offset("Terrain", "WaterReflectance") }
    pub fn water_transparency() -> u64 { get_offset("Terrain", "WaterTransparency") }
    pub fn water_wave_size() -> u64 { get_offset("Terrain", "WaterWaveSize") }
    pub fn water_wave_speed() -> u64 { get_offset("Terrain", "WaterWaveSpeed") }
}

pub mod local_script {
    use super::get_offset;
    
    pub fn byte_code() -> u64 { get_offset("LocalScript", "ByteCode") }
}

pub mod mesh_part {
    use super::get_offset;
    
    pub fn mesh_id() -> u64 { get_offset("MeshPart", "MeshId") }
    pub fn texture() -> u64 { get_offset("MeshPart", "Texture") }
}

pub mod misc {
    use super::get_offset;
    
    pub fn adornee() -> u64 { get_offset("Misc", "Adornee") }
    pub fn animation_id() -> u64 { get_offset("Misc", "AnimationId") }
    pub fn require_lock() -> u64 { get_offset("Misc", "RequireLock") }
    pub fn string_length() -> u64 { get_offset("Misc", "StringLength") }
    pub fn value() -> u64 { get_offset("Misc", "Value") }
}

pub mod model {
    use super::get_offset;
    
    pub fn primary_part() -> u64 { get_offset("Model", "PrimaryPart") }
    pub fn scale() -> u64 { get_offset("Model", "Scale") }
}

pub mod module_script {
    use super::get_offset;
    
    pub fn byte_code() -> u64 { get_offset("ModuleScript", "ByteCode") }
}

pub mod mouse_service {
    use super::get_offset;
    
    pub fn input_object() -> u64 { get_offset("MouseService", "InputObject") }
    pub fn mouse_position() -> u64 { get_offset("MouseService", "MousePosition") }
    pub fn sensitivity_pointer() -> u64 { get_offset("MouseService", "SensitivityPointer") }
}

pub mod player {
    use super::{get_offset, try_get_offset};
    
    pub fn camera_mode() -> u64 { get_offset("Player", "CameraMode") }
    pub fn country() -> u64 { get_offset("Player", "Country") }
    pub fn display_name() -> u64 { get_offset("Player", "DisplayName") }
    pub fn gender() -> u64 { get_offset("Player", "Gender") }
    pub fn localplayer() -> u64 { get_offset("Player", "LocalPlayer") }
    pub fn max_zoom_distance() -> u64 { get_offset("Player", "MaxZoomDistance") }
    pub fn min_zoom_distance() -> u64 { get_offset("Player", "MinZoomDistance") }
    pub fn model_instance() -> u64 { get_offset("Player", "ModelInstance") }
    pub fn mouse() -> u64 { get_offset("Player", "Mouse") }
    
    pub fn team() -> u64 { 
        try_get_offset("Player", "Team").unwrap_or_else(|| {
            tracing::warn!("Player.Team offset not found, using despair fallback 0x270");
            0x270
        })
    }
    
    pub fn user_id() -> u64 { get_offset("Player", "UserId") }
    
    // usually near Team offset
    pub fn team_color() -> u64 { 
        try_get_offset("Player", "TeamColor").unwrap_or(team() + 0x8) 
    }
}

pub mod player_configurer {
    use super::get_offset;
    
    pub fn override_duration() -> u64 { get_offset("PlayerConfigurer", "OverrideDuration") }
    pub fn pointer() -> u64 { get_offset("PlayerConfigurer", "Pointer") }
}

pub mod player_mouse {
    use super::get_offset;
    
    pub fn icon() -> u64 { get_offset("PlayerMouse", "Icon") }
    pub fn workspace() -> u64 { get_offset("PlayerMouse", "Workspace") }
}

pub mod primitive_flags {
    use super::{get_offset, try_get_offset_multi};
    
    pub fn anchored() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "Anchored"),
            ("PrimitiveFlags", "AnchoredMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "Anchored"))
    }
    pub fn can_collide() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "CanCollide"),
            ("PrimitiveFlags", "CanCollideMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "CanCollide"))
    }
    pub fn can_touch() -> u64 {
        try_get_offset_multi(&[
            ("PrimitiveFlags", "CanTouch"),
            ("PrimitiveFlags", "CanTouchMask"),
        ]).unwrap_or_else(|| get_offset("PrimitiveFlags", "CanTouch"))
    }
}

pub mod proximity_prompt {
    use super::get_offset;
    
    pub fn action_text() -> u64 { get_offset("ProximityPrompt", "ActionText") }
    pub fn enabled() -> u64 { get_offset("ProximityPrompt", "Enabled") }
    pub fn gamepad_key_code() -> u64 { get_offset("ProximityPrompt", "GamepadKeyCode") }
    pub fn hold_duration() -> u64 { get_offset("ProximityPrompt", "HoldDuration") }
    pub fn key_code() -> u64 { get_offset("ProximityPrompt", "KeyCode") }
    pub fn max_activation_distance() -> u64 { get_offset("ProximityPrompt", "MaxActivationDistance") }
    pub fn object_text() -> u64 { get_offset("ProximityPrompt", "ObjectText") }
    pub fn requires_line_of_sight() -> u64 { get_offset("ProximityPrompt", "RequiresLineOfSight") }
}

pub mod render_view {
    use super::get_offset;
    
    pub fn device_d3d11() -> u64 { get_offset("RenderView", "DeviceD3D11") }
    pub fn visual_engine() -> u64 { get_offset("RenderView", "VisualEngine") }
}

pub mod run_service {
    use super::get_offset;
    
    pub fn heartbeat_fps() -> u64 { get_offset("RunService", "HeartbeatFPS") }
    pub fn heartbeat_task() -> u64 { get_offset("RunService", "HeartbeatTask") }
}

pub mod sky {
    use super::get_offset;
    
    pub fn moon_angular_size() -> u64 { get_offset("Sky", "MoonAngularSize") }
    pub fn moon_texture_id() -> u64 { get_offset("Sky", "MoonTextureId") }
    pub fn skybox_bk() -> u64 { get_offset("Sky", "SkyboxBk") }
    pub fn skybox_dn() -> u64 { get_offset("Sky", "SkyboxDn") }
    pub fn skybox_ft() -> u64 { get_offset("Sky", "SkyboxFt") }
    pub fn skybox_lf() -> u64 { get_offset("Sky", "SkyboxLf") }
    pub fn skybox_orientation() -> u64 { get_offset("Sky", "SkyboxOrientation") }
    pub fn skybox_rt() -> u64 { get_offset("Sky", "SkyboxRt") }
    pub fn skybox_up() -> u64 { get_offset("Sky", "SkyboxUp") }
    pub fn star_count() -> u64 { get_offset("Sky", "StarCount") }
    pub fn sun_angular_size() -> u64 { get_offset("Sky", "SunAngularSize") }
    pub fn sun_texture_id() -> u64 { get_offset("Sky", "SunTextureId") }
}

pub mod special_mesh {
    use super::get_offset;
    
    pub fn mesh_id() -> u64 { get_offset("SpecialMesh", "MeshId") }
    pub fn scale() -> u64 { get_offset("SpecialMesh", "Scale") }
}

pub mod stats_item {
    use super::get_offset;
    
    pub fn value() -> u64 { get_offset("StatsItem", "Value") }
}

pub mod task_scheduler {
    use super::get_offset;
    
    pub fn fake_datamodel_to_datamodel() -> u64 { get_offset("TaskScheduler", "FakeDataModelToDataModel") }
    pub fn job_end() -> u64 { get_offset("TaskScheduler", "JobEnd") }
    pub fn job_name() -> u64 { get_offset("TaskScheduler", "JobName") }
    pub fn job_start() -> u64 { get_offset("TaskScheduler", "JobStart") }
    pub fn max_fps() -> u64 { get_offset("TaskScheduler", "MaxFPS") }
    pub fn pointer() -> u64 { get_offset("TaskScheduler", "Pointer") }
    pub fn render_job_to_fake_datamodel() -> u64 { get_offset("TaskScheduler", "RenderJobToFakeDataModel") }
    pub fn render_job_to_render_view() -> u64 { get_offset("TaskScheduler", "RenderJobToRenderView") }
}

pub mod team {
    use super::get_offset;
    
    pub fn brick_color() -> u64 { get_offset("Team", "BrickColor") }
}

pub mod textures {
    use super::get_offset;
    
    pub fn decal_texture() -> u64 { get_offset("Textures", "Decal_Texture") }
    pub fn texture_texture() -> u64 { get_offset("Textures", "Texture_Texture") }
}

pub mod visual_engine {
    use super::get_offset;
    
    pub fn dimensions() -> u64 { get_offset("VisualEngine", "Dimensions") }
    pub fn pointer() -> u64 { get_offset("VisualEngine", "Pointer") }
    pub fn to_datamodel1() -> u64 { get_offset("VisualEngine", "ToDataModel1") }
    pub fn to_datamodel2() -> u64 { get_offset("VisualEngine", "ToDataModel2") }
    pub fn view_matrix() -> u64 { get_offset("VisualEngine", "ViewMatrix") }
}

pub mod workspace {
    use super::get_offset;
    
    pub fn current_camera() -> u64 { get_offset("Workspace", "CurrentCamera") }
    pub fn distributed_game_time() -> u64 { get_offset("Workspace", "DistributedGameTime") }
    pub fn gravity() -> u64 { get_offset("Workspace", "Gravity") }
    pub fn gravity_container() -> u64 { get_offset("Workspace", "GravityContainer") }
    pub fn primitives_pointer1() -> u64 { get_offset("Workspace", "PrimitivesPointer1") }
    pub fn primitives_pointer2() -> u64 { get_offset("Workspace", "PrimitivesPointer2") }
    pub fn read_only_gravity() -> u64 { get_offset("Workspace", "ReadOnlyGravity") }
}

pub mod primitive {
    use super::base_part;
    
    pub fn velocity() -> u64 { base_part::assembly_linear_velocity() }
    
    pub fn position() -> u64 { base_part::position() }
}

pub mod value_base {
    use super::misc;
    
    pub fn value() -> u64 { misc::value() }
}

pub mod frame {
    use super::gui_object;
    
    pub fn position_x() -> u64 { gui_object::position() }
    
    // +0x8 from position (UDim2 layout)
    pub fn position_y() -> u64 { gui_object::position() + 0x8 }
}
