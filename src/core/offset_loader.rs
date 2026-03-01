#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use sysinfo::System;
use super::cloudflare_bypass;

lazy_static! {
    static ref OFFSETS: Arc<RwLock<OffsetDatabase>> = Arc::new(RwLock::new(OffsetDatabase::default()));
}

/// lock-free snapshot, set once after init
static FROZEN_DB: OnceLock<OffsetDatabase> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct OffsetDatabase {
    pub version: String,
    pub dumper_version: String,
    pub dumped_at: String,
    pub total_offsets: usize,
    
    pub animation_track: HashMap<String, usize>,
    pub base_part: HashMap<String, usize>,
    pub bytecode: HashMap<String, usize>,
    pub camera: HashMap<String, usize>,
    pub click_detector: HashMap<String, usize>,
    pub datamodel: HashMap<String, usize>,
    pub fflags: HashMap<String, usize>,
    pub fake_datamodel: HashMap<String, usize>,
    pub gui_object: HashMap<String, usize>,
    pub humanoid: HashMap<String, usize>,
    pub instance: HashMap<String, usize>,
    pub lighting: HashMap<String, usize>,
    pub local_script: HashMap<String, usize>,
    pub mesh_part: HashMap<String, usize>,
    pub misc: HashMap<String, usize>,
    pub model: HashMap<String, usize>,
    pub module_script: HashMap<String, usize>,
    pub mouse_service: HashMap<String, usize>,
    pub player: HashMap<String, usize>,
    pub player_configurer: HashMap<String, usize>,
    pub player_mouse: HashMap<String, usize>,
    pub primitive: HashMap<String, usize>,
    pub primitive_flags: HashMap<String, usize>,
    pub proximity_prompt: HashMap<String, usize>,
    pub render_view: HashMap<String, usize>,
    pub run_service: HashMap<String, usize>,
    pub sky: HashMap<String, usize>,
    pub special_mesh: HashMap<String, usize>,
    pub stats_item: HashMap<String, usize>,
    pub task_scheduler: HashMap<String, usize>,
    pub team: HashMap<String, usize>,
    pub textures: HashMap<String, usize>,
    pub value_base: HashMap<String, usize>,
    pub visual_engine: HashMap<String, usize>,
    pub workspace: HashMap<String, usize>,
    pub frame: HashMap<String, usize>,
    /// overflow map for dynamic/unknown namespaces
    pub extra: HashMap<String, HashMap<String, usize>>,
}

impl OffsetDatabase {
    pub fn get(&self, namespace: &str, name: &str) -> Option<usize> {
        // Match both PascalCase (from callers) and lowercase without allocating a String
        let map = match namespace {
            "AnimationTrack" | "animationtrack" => &self.animation_track,
            "BasePart" | "basepart" => &self.base_part,
            "ByteCode" | "bytecode" => &self.bytecode,
            "Camera" | "camera" => &self.camera,
            "ClickDetector" | "clickdetector" => &self.click_detector,
            "DataModel" | "datamodel" => &self.datamodel,
            "FFlags" | "fflags" => &self.fflags,
            "FakeDataModel" | "fakedatamodel" => &self.fake_datamodel,
            "GuiObject" | "guiobject" => &self.gui_object,
            "Humanoid" | "humanoid" => &self.humanoid,
            "Instance" | "instance" => &self.instance,
            "Lighting" | "lighting" => &self.lighting,
            "LocalScript" | "localscript" => &self.local_script,
            "MeshPart" | "meshpart" => &self.mesh_part,
            "Misc" | "misc" => &self.misc,
            "Model" | "model" => &self.model,
            "ModuleScript" | "modulescript" => &self.module_script,
            "MouseService" | "mouseservice" => &self.mouse_service,
            "Player" | "player" => &self.player,
            "PlayerConfigurer" | "playerconfigurer" => &self.player_configurer,
            "PlayerMouse" | "playermouse" => &self.player_mouse,
            "Primitive" | "primitive" => &self.primitive,
            "PrimitiveFlags" | "primitiveflags" => &self.primitive_flags,
            "ProximityPrompt" | "proximityprompt" => &self.proximity_prompt,
            "RenderView" | "renderview" => &self.render_view,
            "RunService" | "runservice" => &self.run_service,
            "Sky" | "sky" => &self.sky,
            "SpecialMesh" | "specialmesh" => &self.special_mesh,
            "StatsItem" | "statsitem" => &self.stats_item,
            "TaskScheduler" | "taskscheduler" => &self.task_scheduler,
            "Team" | "team" => &self.team,
            "Textures" | "textures" => &self.textures,
            "ValueBase" | "valuebase" => &self.value_base,
            "VisualEngine" | "visualengine" => &self.visual_engine,
            "Workspace" | "workspace" => &self.workspace,
            "Frame" | "frame" => &self.frame,
            _ => {
                // Fallback: only allocate for unknown/dynamic namespaces
                return self.extra
                    .get(&namespace.to_lowercase())
                    .and_then(|m| m.get(name).copied());
            }
        };
        
        map.get(name).copied()
    }
}

pub fn detect_roblox_version() -> Result<String> {
    let mut system = System::new_all();
    system.refresh_all();

    let version_regex = Regex::new(r"version-([a-f0-9]+)").unwrap();

    for (_, process) in system.processes() {
        let process_name = process.name().to_lowercase();
        
        if process_name.contains("robloxplayerbeta") || process_name.contains("roblox") {
            if let Some(exe_path) = process.exe() {
                if let Some(path_str) = exe_path.to_str() {
                    tracing::info!("Found Roblox process: {}", path_str);
                    
                    if let Some(captures) = version_regex.captures(path_str) {
                        let version = format!("version-{}", &captures[1]);
                        tracing::info!("Detected Roblox version: {}", version);
                        return Ok(version);
                    }
                }
            }
        }
    }

    Err(anyhow!("Roblox process not found. Please ensure Roblox is running."))
}

fn is_cloudflare_challenge(content: &str) -> bool {
    content.contains("challenge-platform") || 
    content.contains("cf-browser-verification") ||
    content.contains("Just a moment") ||
    content.contains("Checking your browser") ||
    (content.contains("cloudflare") && content.contains("challenge")) ||
    // Also check if it looks like HTML when we expect C++ headers
    (content.contains("<!DOCTYPE") || content.contains("<html")) && !content.contains("namespace")
}

pub async fn download_fflags(version: &str) -> Result<String> {
    let url = format!("https://imtheo.lol/Offsets/{}/FFlags.hpp", version);
    
    tracing::info!("Downloading FFlags from: {}", url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36")
        .build()?;
    
    if let Some(cache) = cloudflare_bypass::load_cached_cookie("imtheo.lol") {
        tracing::info!("Trying FFlags with cached Cloudflare cookie...");
        
        let response = client
            .get(&url)
            .header("Cookie", format!("cf_clearance={}", cache.cf_clearance))
            .header("Accept", "text/html,application/xhtml+xml,*/*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .context("Failed to download FFlags")?;
        
        if response.status().is_success() {
            let content = response.text().await?;
            
            if !is_cloudflare_challenge(&content) && 
               (content.contains("namespace") || content.contains("constexpr") || content.contains("uintptr_t")) {
                tracing::info!("Successfully downloaded FFlags using cached cookie ({} bytes)", content.len());
                return Ok(content);
            }
        }
        
        tracing::info!("Cached cookie didn't work for FFlags, trying direct request...");
    }
    
    let response = client
        .get(&url)
        .header("Accept", "text/html,application/xhtml+xml,*/*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Failed to download FFlags")?;
    
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download FFlags: HTTP {}. This version may not be available yet.",
            response.status()
        ));
    }
    
    let content = response.text().await?;
    
    if is_cloudflare_challenge(&content) {
        tracing::warn!("[FFLAGS] Cloudflare protection detected! Starting browser bypass...");
        
        match cloudflare_bypass::download_with_cloudflare_bypass(&url).await {
            Ok(content) => {
                tracing::info!("Successfully downloaded FFlags via browser ({} bytes)", content.len());
                return Ok(content);
            }
            Err(e) => {
                tracing::warn!("[FFLAGS] Browser bypass failed: {}", e);
                return Err(anyhow!("Cloudflare bypass failed for FFlags: {}", e));
            }
        }
    }
    
    if !content.contains("namespace") && !content.contains("constexpr") && !content.contains("uintptr_t") {
        tracing::warn!("[FFLAGS] Response doesn't appear to be valid FFlag data");
        return Err(anyhow!("Invalid response from imtheo.lol FFlags - not valid offset data"));
    }
    
    tracing::info!("Successfully downloaded FFlags ({} bytes)", content.len());
    
    Ok(content)
}

pub async fn download_offsets(version: &str) -> Result<String> {
    let url = format!("https://imtheo.lol/Offsets/{}/Offsets.hpp", version);
    
    tracing::info!("Downloading offsets from primary source: {}", url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36")
        .build()?;
    
    if let Some(cache) = cloudflare_bypass::load_cached_cookie("imtheo.lol") {
        tracing::info!("Trying with cached Cloudflare cookie...");
        
        let response = client
            .get(&url)
            .header("Cookie", format!("cf_clearance={}", cache.cf_clearance))
            .header("Accept", "text/html,application/xhtml+xml,*/*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .context("Failed to download offsets")?;
        
        if response.status().is_success() {
            let content = response.text().await?;
            
            if !is_cloudflare_challenge(&content) && 
               (content.contains("namespace") || content.contains("constexpr") || content.contains("uintptr_t")) {
                tracing::info!("Successfully downloaded offsets using cached cookie ({} bytes)", content.len());
                return Ok(content);
            }
        }
        
        tracing::info!("Cached cookie didn't work, trying direct request...");
    }
    
    let response = client
        .get(&url)
        .header("Accept", "text/html,application/xhtml+xml,*/*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Failed to download offsets")?;
    
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download offsets: HTTP {}. This version may not be available yet.",
            response.status()
        ));
    }
    
    let content = response.text().await?;
    
    if is_cloudflare_challenge(&content) {
        tracing::warn!("[SOURCE 1] Cloudflare protection detected! Starting browser bypass...");
        
        match cloudflare_bypass::download_with_cloudflare_bypass(&url).await {
            Ok(content) => {
                tracing::info!("Successfully downloaded offsets via browser ({} bytes)", content.len());
                return Ok(content);
            }
            Err(e) => {
                tracing::warn!("[SOURCE 1] Browser bypass failed: {}", e);
                return Err(anyhow!("Cloudflare bypass failed: {}", e));
            }
        }
    }
    
    if !content.contains("namespace") && !content.contains("constexpr") && !content.contains("uintptr_t") {
        tracing::warn!("[SOURCE 1] Response doesn't appear to be valid offset data");
        return Err(anyhow!("Invalid response from imtheo.lol - not valid offset data"));
    }
    
    tracing::info!("Successfully downloaded offsets from primary source ({} bytes)", content.len());
    
    Ok(content)
}

/// expected format: "// Roblox Version: version-f8734e043e1e40a2"
fn extract_ntgetwritewatch_version(content: &str) -> Option<String> {
    let version_regex = Regex::new(r"//\s*Roblox\s*Version:\s*(version-[a-f0-9]+)").ok()?;
    
    // Check only the first few lines for the version header
    for line in content.lines().take(10) {
        if let Some(captures) = version_regex.captures(line) {
            return Some(captures[1].to_string());
        }
    }
    
    None
}

pub async fn download_offsets_secondary(expected_version: &str) -> Result<String> {
    let url = "https://offsets.ntgetwritewatch.workers.dev/offsets.hpp";
    
    tracing::info!("Downloading offsets from secondary source: {}", url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to download secondary offsets")?;
    
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download secondary offsets: HTTP {}",
            response.status()
        ));
    }
    
    let content = response.text().await?;
    
    // Extract and validate version from ntgetwritewatch
    match extract_ntgetwritewatch_version(&content) {
        Some(source_version) => {
            if source_version != expected_version {
                tracing::warn!(
                    "[SOURCE 2] Version mismatch! Expected: {}, Got: {}",
                    expected_version,
                    source_version
                );
                return Err(anyhow!(
                    "Version mismatch: ntgetwritewatch has {} but current Roblox is {}",
                    source_version,
                    expected_version
                ));
            }
            tracing::info!(
                "[SOURCE 2] Version match confirmed: {}",
                source_version
            );
        }
        None => {
            tracing::warn!("[SOURCE 2] Could not extract version from ntgetwritewatch response");
            return Err(anyhow!(
                "Could not verify version from ntgetwritewatch - no version header found"
            ));
        }
    }
    
    tracing::info!("Successfully downloaded offsets from secondary source ({} bytes)", content.len());
    
    Ok(content)
}

pub fn parse_flat_offsets(content: &str, db: &mut OffsetDatabase) -> Result<usize> {
    let offset_regex = Regex::new(r"inline\s+constexpr\s+uintptr_t\s+(\w+)\s*=\s*0x([0-9a-fA-F]+);").unwrap();
    let version_regex = Regex::new(r"Roblox Version:\s*(version-[a-f0-9]+)").unwrap();
    
    if let Some(captures) = version_regex.captures(content) {
        if db.version.is_empty() {
            db.version = captures[1].to_string();
        }
    }
    
    let mut count = 0;
    
    let name_mappings: HashMap<&str, (&str, &str)> = [
        // MouseService
        ("InputObject", ("mouseservice", "InputObject")),
        ("MousePosition", ("mouseservice", "MousePosition")),
        ("MouseSensitivity", ("mouseservice", "SensitivityPointer")),
        
        // Frame/GUI
        ("FramePositionOffsetX", ("frame", "PositionX")),
        ("FramePositionOffsetY", ("frame", "PositionY")),
        ("FramePositionX", ("guiobject", "PositionX")),
        ("FramePositionY", ("guiobject", "PositionY")),
        ("FrameSizeX", ("guiobject", "SizeX")),
        ("FrameSizeY", ("guiobject", "SizeY")),
        ("FrameSizeOffsetX", ("guiobject", "SizeOffsetX")),
        ("FrameSizeOffsetY", ("guiobject", "SizeOffsetY")),
        ("FrameRotation", ("guiobject", "Rotation")),
        ("FrameVisible", ("guiobject", "Visible")),
        ("ScreenGuiEnabled", ("guiobject", "ScreenGuiEnabled")),
        ("TextLabelText", ("guiobject", "TextLabelText")),
        ("Text", ("guiobject", "Text")),  // Also map directly for primary source
        ("TextLabelVisible", ("guiobject", "TextLabelVisible")),
        
        // Player
        ("PlayerMouse", ("player", "Mouse")),
        ("LocalPlayer", ("player", "LocalPlayer")),
        ("UserId", ("player", "UserId")),
        ("Team", ("player", "Team")),
        ("TeamColor", ("player", "TeamColor")),
        ("DisplayName", ("player", "DisplayName")),
        ("ModelInstance", ("player", "ModelInstance")),
        ("CharacterAppearanceId", ("player", "CharacterAppearanceId")),
        ("CameraMaxZoomDistance", ("player", "MaxZoomDistance")),
        ("CameraMinZoomDistance", ("player", "MinZoomDistance")),
        ("CameraMode", ("player", "CameraMode")),
        
        // Camera
        ("Camera", ("workspace", "CurrentCamera")),
        ("CameraPos", ("camera", "Position")),
        ("CameraRotation", ("camera", "Rotation")),
        ("CameraSubject", ("camera", "CameraSubject")),
        ("CameraType", ("camera", "CameraType")),
        ("FOV", ("camera", "FieldOfView")),
        ("ViewportSize", ("camera", "ViewportSize")),
        
        // Instance
        ("Name", ("instance", "Name")),
        ("NameSize", ("instance", "NameSize")),
        ("Parent", ("instance", "Parent")),
        ("Children", ("instance", "ChildrenStart")),
        ("ChildrenEnd", ("instance", "ChildrenEnd")),
        ("ClassDescriptor", ("instance", "ClassDescriptor")),
        ("ClassDescriptorToClassName", ("instance", "ClassName")),
        ("InstanceAttributePointer1", ("instance", "AttributeContainer")),
        ("InstanceAttributePointer2", ("instance", "AttributeList")),
        ("AttributeToNext", ("instance", "AttributeToNext")),
        ("AttributeToValue", ("instance", "AttributeToValue")),
        ("OnDemandInstance", ("instance", "OnDemandInstance")),
        ("InstanceCapabilities", ("instance", "Capabilities")),
        
        // FakeDataModel/DataModel
        ("FakeDataModelPointer", ("fakedatamodel", "Pointer")),
        ("FakeDataModelToDataModel", ("fakedatamodel", "RealDataModel")),
        ("Workspace", ("datamodel", "Workspace")),
        ("PlaceId", ("datamodel", "PlaceId")),
        ("GameId", ("datamodel", "GameId")),
        ("CreatorId", ("datamodel", "CreatorId")),
        ("JobId", ("datamodel", "JobId")),
        ("ScriptContext", ("datamodel", "ScriptContext")),
        ("GameLoaded", ("datamodel", "GameLoaded")),
        ("DataModelPrimitiveCount", ("datamodel", "PrimitiveCount")),
        ("DataModelDeleterPointer", ("datamodel", "DeleterPointer")),
        ("DataModelToRenderView1", ("datamodel", "ToRenderView1")),
        ("DataModelToRenderView2", ("datamodel", "ToRenderView2")),
        ("DataModelToRenderView3", ("datamodel", "ToRenderView3")),
        
        // VisualEngine
        ("VisualEnginePointer", ("visualengine", "Pointer")),
        ("Dimensions", ("visualengine", "Dimensions")),
        ("viewmatrix", ("visualengine", "ViewMatrix")),
        ("VisualEngineToDataModel1", ("visualengine", "ToDataModel1")),
        ("VisualEngineToDataModel2", ("visualengine", "ToDataModel2")),
        ("VisualEngine", ("visualengine", "VisualEngine")),
        
        // Humanoid
        ("Health", ("humanoid", "Health")),
        ("MaxHealth", ("humanoid", "MaxHealth")),
        ("WalkSpeed", ("humanoid", "Walkspeed")),
        ("WalkSpeedCheck", ("humanoid", "WalkspeedCheck")),
        ("JumpPower", ("humanoid", "JumpPower")),
        ("HipHeight", ("humanoid", "HipHeight")),
        ("RigType", ("humanoid", "RigType")),
        ("MoveDirection", ("humanoid", "MoveDirection")),
        ("RootPartR15", ("humanoid", "RootPartR15")),
        ("RootPartR6", ("humanoid", "RootPartR6")),
        ("MaxSlopeAngle", ("humanoid", "MaxSlopeAngle")),
        ("HumanoidDisplayName", ("humanoid", "DisplayName")),
        ("HumanoidState", ("humanoid", "HumanoidState")),
        ("HumanoidStateId", ("humanoid", "HumanoidStateID")),
        ("HealthDisplayDistance", ("humanoid", "HealthDisplayDistance")),
        ("NameDisplayDistance", ("humanoid", "NameDisplayDistance")),
        ("Sit", ("humanoid", "Sit")),
        ("EvaluateStateMachine", ("humanoid", "EvaluateStateMachine")),
        ("AutoJumpEnabled", ("humanoid", "AutoJumpEnabled")),
        
        // BasePart
        ("CFrame", ("basepart", "CFrame")),
        ("Position", ("basepart", "Position")),
        ("Rotation", ("basepart", "Rotation")),
        ("PartSize", ("basepart", "Size")),
        ("Velocity", ("basepart", "Velocity")),
        ("Transparency", ("basepart", "Transparency")),
        ("Primitive", ("basepart", "Primitive")),
        ("Anchored", ("basepart", "Anchored")),
        ("AnchoredMask", ("basepart", "AnchoredMask")),
        ("CanCollide", ("basepart", "CanCollide")),
        ("CanCollideMask", ("basepart", "CanCollideMask")),
        ("CanTouch", ("basepart", "CanTouch")),
        ("CanTouchMask", ("basepart", "CanTouchMask")),
        ("MaterialType", ("basepart", "MaterialType")),
        
        // Primitive
        ("PrimitivesPointer1", ("primitive", "Pointer1")),
        ("PrimitivesPointer2", ("primitive", "Pointer2")),
        ("PrimitiveValidateValue", ("primitive", "ValidateValue")),
        
        // TaskScheduler
        ("TaskSchedulerPointer", ("taskscheduler", "Pointer")),
        ("TaskSchedulerMaxFPS", ("taskscheduler", "MaxFPS")),
        ("JobStart", ("taskscheduler", "JobStart")),
        ("JobEnd", ("taskscheduler", "JobEnd")),
        ("JobsPointer", ("taskscheduler", "JobsPointer")),
        ("Job_Name", ("taskscheduler", "JobName")),
        
        // Workspace
        ("Gravity", ("workspace", "Gravity")),
        ("WorkspaceToWorld", ("workspace", "ToWorld")),
        
        // ValueBase
        ("Value", ("valuebase", "Value")),
        ("ValueGetSetToValue", ("valuebase", "GetSetToValue")),
        
        // Lighting
        ("ClockTime", ("lighting", "ClockTime")),
        ("FogColor", ("lighting", "FogColor")),
        ("FogStart", ("lighting", "FogStart")),
        ("FogEnd", ("lighting", "FogEnd")),
        ("OutdoorAmbient", ("lighting", "OutdoorAmbient")),
        
        // Sky
        ("SkyboxBk", ("sky", "SkyboxBk")),
        ("SkyboxDn", ("sky", "SkyboxDn")),
        ("SkyboxFt", ("sky", "SkyboxFt")),
        ("SkyboxLf", ("sky", "SkyboxLf")),
        ("SkyboxRt", ("sky", "SkyboxRt")),
        ("SkyboxUp", ("sky", "SkyboxUp")),
        ("MoonTextureId", ("sky", "MoonTextureId")),
        ("SunTextureId", ("sky", "SunTextureId")),
        ("StarCount", ("sky", "StarCount")),
        
        // ClickDetector
        ("ClickDetectorMaxActivationDistance", ("clickdetector", "MaxActivationDistance")),
        
        // ProximityPrompt
        ("ProximityPromptEnabled", ("proximityprompt", "Enabled")),
        ("ProximityPromptMaxActivationDistance", ("proximityprompt", "MaxActivationDistance")),
        ("ProximityPromptHoldDuraction", ("proximityprompt", "HoldDuration")),
        ("ProximityPromptActionText", ("proximityprompt", "ActionText")),
        ("ProximityPromptMaxObjectText", ("proximityprompt", "ObjectText")),
        ("ProximityPromptGamepadKeyCode", ("proximityprompt", "GamepadKeyCode")),
        
        // LocalScript/ModuleScript (Bytecode)
        ("LocalScriptByteCode", ("localscript", "ByteCode")),
        ("LocalScriptBytecodePointer", ("bytecode", "LocalScriptPointer")),
        ("LocalScriptHash", ("localscript", "Hash")),
        ("ModuleScriptByteCode", ("modulescript", "ByteCode")),
        ("ModuleScriptBytecodePointer", ("bytecode", "ModuleScriptPointer")),
        ("ModuleScriptHash", ("modulescript", "Hash")),
        ("RunContext", ("localscript", "RunContext")),
        ("Sandboxed", ("localscript", "Sandboxed")),
        
        // MeshPart
        ("MeshPartColor3", ("meshpart", "Color3")),
        ("MeshPartTexture", ("meshpart", "Texture")),
        
        // Misc
        ("Adornee", ("misc", "Adornee")),
        ("AnimationId", ("animationtrack", "AnimationId")),
        ("DecalTexture", ("textures", "DecalTexture")),
        ("SoundId", ("misc", "SoundId")),
        ("Ping", ("misc", "Ping")),
        ("Tool_Grip_Position", ("misc", "ToolGripPosition")),
        ("Deleter", ("misc", "Deleter")),
        ("DeleterBack", ("misc", "DeleterBack")),
        ("StringLength", ("misc", "StringLength")),
        ("RequireBypass", ("misc", "RequireBypass")),
        ("TagList", ("misc", "TagList")),
        
        // FFlags
        ("FFlagList", ("fflags", "List")),
        ("FFlagToValueGetSet", ("fflags", "ToValueGetSet")),
        
        // PlayerConfigurer
        ("PlayerConfigurerPointer", ("playerconfigurer", "Pointer")),
        ("BanningEnabled", ("playerconfigurer", "BanningEnabled")),
        ("ForceNewAFKDuration", ("playerconfigurer", "ForceNewAFKDuration")),
        
        // RenderView
        ("RenderJobToDataModel", ("renderview", "ToDataModel")),
        ("RenderJobToFakeDataModel", ("renderview", "ToFakeDataModel")),
        ("RenderJobToRenderView", ("renderview", "ToRenderView")),
        ("InsetMinX", ("renderview", "InsetMinX")),
        ("InsetMinY", ("renderview", "InsetMinY")),
        ("InsetMaxX", ("renderview", "InsetMaxX")),
        ("InsetMaxY", ("renderview", "InsetMaxY")),
        
        // Beam effects
        ("BeamBrightness", ("misc", "BeamBrightness")),
        ("BeamColor", ("misc", "BeamColor")),
        ("BeamLightEmission", ("misc", "BeamLightEmission")),
        ("BeamLightInfuence", ("misc", "BeamLightInfluence")),
        
        // Team (from Team namespace in ntgetwritewatch)
        ("TeamColor", ("team", "BrickColor")),
        
        // PrimitiveFlags (explicit masks from ntgetwritewatch)
        ("AnchoredMask", ("primitiveflags", "AnchoredMask")),
        ("CanCollideMask", ("primitiveflags", "CanCollideMask")),
        ("CanTouchMask", ("primitiveflags", "CanTouchMask")),
    ].iter().cloned().collect();
    
    // Additional mappings where one source name maps to multiple destinations
    // Velocity is used on both BasePart and Primitive (same offset value)
    let multi_mappings: HashMap<&str, Vec<(&str, &str)>> = [
        ("Velocity", vec![("primitive", "Velocity"), ("basepart", "AssemblyLinearVelocity")]),
        ("Position", vec![("primitive", "Position")]),
    ].iter().cloned().collect();
    
    for line in content.lines() {
        if let Some(captures) = offset_regex.captures(line) {
            let name = &captures[1];
            let value = usize::from_str_radix(&captures[2], 16)
                .context(format!("Failed to parse hex value: {}", &captures[2]))?;
            
            if let Some((namespace, mapped_name)) = name_mappings.get(name) {
                let map = match *namespace {
                    "mouseservice" => &mut db.mouse_service,
                    "player" => &mut db.player,
                    "camera" => &mut db.camera,
                    "instance" => &mut db.instance,
                    "fakedatamodel" => &mut db.fake_datamodel,
                    "datamodel" => &mut db.datamodel,
                    "visualengine" => &mut db.visual_engine,
                    "humanoid" => &mut db.humanoid,
                    "basepart" => &mut db.base_part,
                    "taskscheduler" => &mut db.task_scheduler,
                    "valuebase" => &mut db.value_base,
                    "workspace" => &mut db.workspace,
                    "frame" => &mut db.frame,
                    "guiobject" => &mut db.gui_object,
                    "lighting" => &mut db.lighting,
                    "sky" => &mut db.sky,
                    "clickdetector" => &mut db.click_detector,
                    "proximityprompt" => &mut db.proximity_prompt,
                    "localscript" => &mut db.local_script,
                    "modulescript" => &mut db.module_script,
                    "bytecode" => &mut db.bytecode,
                    "meshpart" => &mut db.mesh_part,
                    "misc" => &mut db.misc,
                    "animationtrack" => &mut db.animation_track,
                    "textures" => &mut db.textures,
                    "fflags" => &mut db.fflags,
                    "playerconfigurer" => &mut db.player_configurer,
                    "renderview" => &mut db.render_view,
                    "primitive" => &mut db.primitive,
                    "primitiveflags" => &mut db.primitive_flags,
                    "team" => &mut db.team,
                    _ => continue,
                };
                
                let existing = map.get(*mapped_name).copied();
                let should_insert = match existing {
                    None => true,
                    Some(0) => value != 0,
                    Some(_) => false,
                };
                
                if should_insert && value != 0 {
                    map.insert(mapped_name.to_string(), value);
                    count += 1;
                }
            }
            
            // Handle multi-mappings (same source offset to multiple destinations)
            if let Some(mappings) = multi_mappings.get(name) {
                for (namespace, mapped_name) in mappings {
                    let map = match *namespace {
                        "primitive" => &mut db.primitive,
                        "mouseservice" => &mut db.mouse_service,
                        "player" => &mut db.player,
                        "camera" => &mut db.camera,
                        "instance" => &mut db.instance,
                        "fakedatamodel" => &mut db.fake_datamodel,
                        "datamodel" => &mut db.datamodel,
                        "visualengine" => &mut db.visual_engine,
                        "humanoid" => &mut db.humanoid,
                        "basepart" => &mut db.base_part,
                        "taskscheduler" => &mut db.task_scheduler,
                        "valuebase" => &mut db.value_base,
                        "workspace" => &mut db.workspace,
                        "frame" => &mut db.frame,
                        "guiobject" => &mut db.gui_object,
                        "lighting" => &mut db.lighting,
                        "sky" => &mut db.sky,
                        "team" => &mut db.team,
                        "primitiveflags" => &mut db.primitive_flags,
                        _ => continue,
                    };
                    
                    let existing = map.get(*mapped_name).copied();
                    let should_insert = match existing {
                        None => true,
                        Some(0) => value != 0,
                        Some(_) => false,
                    };
                    
                    if should_insert && value != 0 {
                        map.insert(mapped_name.to_string(), value);
                        count += 1;
                    }
                }
            }
        }
    }
    
    Ok(count)
}

pub fn parse_cpp_offsets(content: &str) -> Result<OffsetDatabase> {
    let mut db = OffsetDatabase::default();
    
    let version_regex = Regex::new(r#"ClientVersion = "(.+?)""#).unwrap();
    let namespace_regex = Regex::new(r"namespace\s+(\w+)\s*\{").unwrap();
    let offset_regex = Regex::new(r"inline\s+constexpr\s+uintptr_t\s+(\w+)\s*=\s*0x([0-9a-fA-F]+);").unwrap();
    
    if let Some(captures) = version_regex.captures(content) {
        db.version = captures[1].to_string();
    }
    
    let mut current_namespace = String::new();
    let mut brace_count = 0;
    
    for line in content.lines() {
        let trimmed = line.trim();
        
        if let Some(captures) = namespace_regex.captures(trimmed) {
            current_namespace = captures[1].to_string();
            brace_count = 1;
            continue;
        }
        
        brace_count += trimmed.matches('{').count() as i32;
        brace_count -= trimmed.matches('}').count() as i32;
        
        if brace_count <= 0 {
            current_namespace.clear();
        }
        
        if !current_namespace.is_empty() {
            if let Some(captures) = offset_regex.captures(trimmed) {
                let name = captures[1].to_string();
                let value = usize::from_str_radix(&captures[2], 16)
                    .context(format!("Failed to parse hex value: {}", &captures[2]))?;
                
                db.total_offsets += 1;
                
                let map = match current_namespace.to_lowercase().as_str() {
                    "animationtrack" => &mut db.animation_track,
                    "basepart" => &mut db.base_part,
                    "bytecode" => &mut db.bytecode,
                    "camera" => &mut db.camera,
                    "clickdetector" => &mut db.click_detector,
                    "datamodel" => &mut db.datamodel,
                    "fflags" => &mut db.fflags,
                    "fakedatamodel" => &mut db.fake_datamodel,
                    "guiobject" => &mut db.gui_object,
                    "humanoid" => &mut db.humanoid,
                    "instance" => &mut db.instance,
                    "lighting" => &mut db.lighting,
                    "localscript" => &mut db.local_script,
                    "meshpart" => &mut db.mesh_part,
                    "misc" => &mut db.misc,
                    "model" => &mut db.model,
                    "modulescript" => &mut db.module_script,
                    "mouseservice" => &mut db.mouse_service,
                    "player" => &mut db.player,
                    "playerconfigurer" => &mut db.player_configurer,
                    "playermouse" => &mut db.player_mouse,
                    "primitive" => &mut db.primitive,
                    "primitiveflags" => &mut db.primitive_flags,
                    "proximityprompt" => &mut db.proximity_prompt,
                    "renderview" => &mut db.render_view,
                    "runservice" => &mut db.run_service,
                    "sky" => &mut db.sky,
                    "specialmesh" => &mut db.special_mesh,
                    "statsitem" => &mut db.stats_item,
                    "taskscheduler" => &mut db.task_scheduler,
                    "team" => &mut db.team,
                    "textures" => &mut db.textures,
                    "valuebase" => &mut db.value_base,
                    "visualengine" => &mut db.visual_engine,
                    "workspace" => &mut db.workspace,
                    _ => {
                        // Store in dynamic overflow map — auto-accept any new namespace
                        db.extra
                            .entry(current_namespace.to_lowercase())
                            .or_default()
                            .insert(name, value);
                        continue;
                    }
                };
                
                map.insert(name, value);
            }
        }
    }
    
    let known_ns_count = [
        &db.animation_track, &db.base_part, &db.camera, &db.datamodel,
        &db.humanoid, &db.instance, &db.player, &db.workspace,
        &db.bytecode, &db.click_detector, &db.fflags, &db.fake_datamodel,
        &db.gui_object, &db.lighting, &db.local_script, &db.mesh_part,
        &db.misc, &db.model, &db.module_script, &db.mouse_service,
        &db.player_configurer, &db.player_mouse, &db.primitive,
        &db.primitive_flags, &db.proximity_prompt, &db.render_view,
        &db.run_service, &db.sky, &db.special_mesh, &db.stats_item,
        &db.task_scheduler, &db.team, &db.textures, &db.value_base,
        &db.visual_engine,
    ].iter().filter(|m| !m.is_empty()).count();
    let extra_ns_count = db.extra.len();
    let extra_offset_count: usize = db.extra.values().map(|m| m.len()).sum();
    db.total_offsets += extra_offset_count;
    tracing::info!("Parsed {} offsets across {} namespaces ({} known + {} dynamic)", 
        db.total_offsets, known_ns_count + extra_ns_count, known_ns_count, extra_ns_count
    );
    if extra_ns_count > 0 {
        let extra_names: Vec<&String> = db.extra.keys().collect();
        tracing::info!("Dynamic namespaces loaded: {:?}", extra_names);
    }
    
    Ok(db)
}

pub async fn initialize_offsets() -> Result<()> {
    tracing::info!("╔════════════════════════════════════════════════════════════╗");
    tracing::info!("║           DYNAMIC OFFSET LOADER - DUAL SOURCE              ║");
    tracing::info!("║   *Appreciation goes to imtheo.lol and ntgetwritewatch*    ║");
    tracing::info!("╚════════════════════════════════════════════════════════════╝");
    
    let version = detect_roblox_version()?;
    
    tracing::info!("[SOURCE 1] Downloading from imtheo.lol...");
    let mut db = match download_offsets(&version).await {
        Ok(content) => {
            match parse_cpp_offsets(&content) {
                Ok(parsed) => {
                    tracing::info!("[SOURCE 1] ✓ Loaded {} offsets from imtheo", parsed.total_offsets);
                    parsed
                }
                Err(e) => {
                    tracing::warn!("[SOURCE 1] ✗ Failed to parse: {}", e);
                    OffsetDatabase::default()
                }
            }
        }
        Err(e) => {
            tracing::warn!("[SOURCE 1] ✗ Failed to download: {}", e);
            OffsetDatabase::default()
        }
    };
    
    // Download FFlags from separate endpoint (dumper update moved FFlags out of Offsets.hpp)
    tracing::info!("[FFLAGS] Downloading FFlags from imtheo.lol...");
    match download_fflags(&version).await {
        Ok(content) => {
            match parse_cpp_offsets(&content) {
                Ok(parsed) => {
                    // Merge FFlags into the main database
                    let fflag_count = parsed.fflags.len();
                    for (key, value) in &parsed.fflags {
                        db.fflags.insert(key.clone(), *value);
                    }
                    // Also merge any extra namespaces from the FFlags file
                    for (ns, map) in &parsed.extra {
                        let target = db.extra.entry(ns.clone()).or_default();
                        for (key, value) in map {
                            target.insert(key.clone(), *value);
                        }
                    }
                    db.total_offsets += fflag_count;
                    tracing::info!("[FFLAGS] ✓ Loaded {} FFlag offsets", fflag_count);
                }
                Err(e) => {
                    tracing::warn!("[FFLAGS] ✗ Failed to parse: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("[FFLAGS] ✗ Failed to download: {} (FFlag features will be unavailable)", e);
        }
    }
    
    tracing::info!("[SOURCE 2] Downloading from ntgetwritewatch...");
    match download_offsets_secondary(&version).await {
        Ok(content) => {
            match parse_flat_offsets(&content, &mut db) {
                Ok(count) => {
                    db.total_offsets += count;  // Update total count with secondary source offsets
                    tracing::info!("[SOURCE 2] ✓ Added {} additional offsets from ntgetwritewatch", count);
                }
                Err(e) => {
                    tracing::warn!("[SOURCE 2] ✗ Failed to parse: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("[SOURCE 2] ✗ Skipped: {}", e);
        }
    }
    
    if db.version.is_empty() {
        db.version = version.clone();
    }
    
    let mut placeholder_count = 0;
    let check_map = |map: &HashMap<String, usize>| -> usize {
        map.iter().filter(|(_, val)| **val == 0).count()
    };
    
    placeholder_count += check_map(&db.mouse_service);
    placeholder_count += check_map(&db.player);
    placeholder_count += check_map(&db.player_mouse);
    placeholder_count += check_map(&db.camera);
    placeholder_count += check_map(&db.humanoid);
    
    if placeholder_count > 0 {
        tracing::warn!("⚠ {} offsets have placeholder values", placeholder_count);
    }
    
    {
        let mut global = OFFSETS.write().unwrap();
        *global = db;
    }
    
    // Freeze offsets into lock-free OnceLock for hot-path access
    // After this, get_offset() is zero-cost: no RwLock, no String allocation
    let frozen = OFFSETS.read().unwrap().clone();
    let _ = FROZEN_DB.set(frozen);
    
    tracing::info!("✓ Offsets loaded: {} total for {}", OFFSETS.read().unwrap().total_offsets, version);
    
    Ok(())
}

pub fn get_offsets() -> Arc<RwLock<OffsetDatabase>> {
    OFFSETS.clone()
}

pub fn get_offset(namespace: &str, name: &str) -> Option<usize> {
    // Fast path: read from frozen (lock-free) database after init
    if let Some(db) = FROZEN_DB.get() {
        return db.get(namespace, name);
    }
    // Slow path: only during init before offsets are frozen
    let db = OFFSETS.read().unwrap();
    db.get(namespace, name)
}

pub fn dump_namespace(namespace: &str) {
    let db = OFFSETS.read().unwrap();
    let map = match namespace.to_lowercase().as_str() {
        "guiobject" => Some(&db.gui_object),
        "instance" => Some(&db.instance),
        "humanoid" => Some(&db.humanoid),
        "player" => Some(&db.player),
        "basepart" => Some(&db.base_part),
        "camera" => Some(&db.camera),
        "datamodel" => Some(&db.datamodel),
        _ => db.extra.get(&namespace.to_lowercase()),
    };
    
    if let Some(map) = map {
        tracing::info!("=== {} Offsets ({} total) ===", namespace, map.len());
        let mut sorted: Vec<_> = map.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());
        for (name, value) in sorted {
            tracing::info!("  {}: {:#x}", name, value);
        }
    } else {
        tracing::warn!("Unknown namespace: {}", namespace);
    }
}

pub fn dump_phantom_forces_offsets() {
    tracing::info!("============ PHANTOM FORCES OFFSET DEBUG ============");
    
    let db = OFFSETS.read().unwrap();
    
    // Instance offsets (for traversing the tree)
    tracing::info!("--- Instance ---");
    for name in ["Name", "ChildrenStart", "ChildrenEnd", "ClassDescriptor", "ClassName"] {
        if let Some(v) = db.instance.get(name) {
            tracing::info!("  {}: {:#x}", name, v);
        } else {
            tracing::warn!("  {}: NOT FOUND", name);
        }
    }
    
    // GuiObject offsets (for reading TextLabel.Text)
    tracing::info!("--- GuiObject ---");
    for name in ["Text", "TextLabelText", "Visible", "Position", "Size"] {
        if let Some(v) = db.gui_object.get(name) {
            tracing::info!("  {}: {:#x}", name, v);
        } else {
            tracing::warn!("  {}: NOT FOUND", name);
        }
    }
    
    // BasePart offsets (for getting positions)
    tracing::info!("--- BasePart ---");
    for name in ["Primitive", "Position", "CFrame", "Size"] {
        if let Some(v) = db.base_part.get(name) {
            tracing::info!("  {}: {:#x}", name, v);
        } else {
            tracing::warn!("  {}: NOT FOUND", name);
        }
    }
    
    // DataModel offsets
    tracing::info!("--- DataModel ---");
    for name in ["PlaceId", "Workspace"] {
        if let Some(v) = db.datamodel.get(name) {
            tracing::info!("  {}: {:#x}", name, v);
        } else {
            tracing::warn!("  {}: NOT FOUND", name);
        }
    }
    
    // Player offsets (for Team detection)
    tracing::info!("--- Player ---");
    for name in ["Team", "TeamColor", "LocalPlayer", "UserId", "DisplayName"] {
        if let Some(v) = db.player.get(name) {
            tracing::info!("  {}: {:#x}", name, v);
        } else {
            tracing::warn!("  {}: NOT FOUND", name);
        }
    }
    
    tracing::info!("=====================================================");
}

pub fn get_version() -> String {
    if let Some(db) = FROZEN_DB.get() {
        return db.version.clone();
    }
    let db = OFFSETS.read().unwrap();
    db.version.clone()
}
