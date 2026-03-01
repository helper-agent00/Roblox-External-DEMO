use std::sync::Arc;

use crate::config::Config;
use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{lighting, terrain as terrain_offsets, try_get_offset};
use crate::sdk::Instance;

/// World modifier for lighting, fog, terrain, and post-processing control.
pub struct WorldModifier {
    memory: Arc<Memory>,
    lighting_address: u64,
    terrain_address: u64,
    atmosphere_address: u64,
    bloom_address: u64,
    dof_address: u64,
    sunrays_address: u64,
    #[allow(dead_code)]
    workspace_address: u64,
    original_fog_start: Option<f32>,
    original_fog_end: Option<f32>,
    original_brightness: Option<f32>,
    original_ambient: Option<[f32; 3]>,
    original_outdoor_ambient: Option<[f32; 3]>,
    original_clock_time: Option<f32>,
    original_global_shadows: Option<bool>,
    original_grass_length: Option<f32>,
    original_water_transparency: Option<f32>,
    original_water_color: Option<[f32; 3]>,
    // Post-processing originals
    original_atmosphere_density: Option<f32>,
    original_atmosphere_haze: Option<f32>,
    original_atmosphere_glare: Option<f32>,
    original_atmosphere_offset: Option<f32>,
    original_atmosphere_color: Option<[f32; 3]>,
    original_bloom_enabled: Option<bool>,
    original_bloom_intensity: Option<f32>,
    original_bloom_size: Option<f32>,
    original_bloom_threshold: Option<f32>,
    original_dof_enabled: Option<bool>,
    original_dof_far_intensity: Option<f32>,
    original_dof_focus_distance: Option<f32>,
    original_dof_in_focus_radius: Option<f32>,
    original_dof_near_intensity: Option<f32>,
    original_sunrays_enabled: Option<bool>,
    original_sunrays_intensity: Option<f32>,
    original_sunrays_spread: Option<f32>,
}

impl WorldModifier {
    /// Create a new world modifier.
    pub fn new(memory: Arc<Memory>, datamodel: &Arc<Instance>) -> Self {
        let lighting_inst = datamodel.find_first_child("Lighting");
        let lighting_address = lighting_inst.as_ref().map(|l| l.address).unwrap_or(0);
        
        // Post-processing effects are children of Lighting
        let atmosphere_address = lighting_inst.as_ref()
            .and_then(|l| l.find_first_child_by_class("Atmosphere"))
            .map(|a| a.address).unwrap_or(0);
        let bloom_address = lighting_inst.as_ref()
            .and_then(|l| l.find_first_child_by_class("BloomEffect"))
            .map(|b| b.address).unwrap_or(0);
        let dof_address = lighting_inst.as_ref()
            .and_then(|l| l.find_first_child_by_class("DepthOfFieldEffect"))
            .map(|d| d.address).unwrap_or(0);
        let sunrays_address = lighting_inst.as_ref()
            .and_then(|l| l.find_first_child_by_class("SunRaysEffect"))
            .map(|s| s.address).unwrap_or(0);

        let workspace = datamodel
            .find_first_child("Workspace");
        
        let workspace_address = workspace.as_ref().map(|w| w.address).unwrap_or(0);
        
        let terrain_address = workspace
            .and_then(|w| w.find_first_child_by_class("Terrain"))
            .map(|t| t.address)
            .unwrap_or(0);

        if atmosphere_address != 0 { tracing::info!("[WORLD] Atmosphere found at 0x{:X}", atmosphere_address); }
        if bloom_address != 0 { tracing::info!("[WORLD] BloomEffect found at 0x{:X}", bloom_address); }
        if dof_address != 0 { tracing::info!("[WORLD] DepthOfFieldEffect found at 0x{:X}", dof_address); }
        if sunrays_address != 0 { tracing::info!("[WORLD] SunRaysEffect found at 0x{:X}", sunrays_address); }

        Self {
            memory,
            lighting_address,
            terrain_address,
            atmosphere_address,
            bloom_address,
            dof_address,
            sunrays_address,
            workspace_address,
            original_fog_start: None,
            original_fog_end: None,
            original_brightness: None,
            original_ambient: None,
            original_outdoor_ambient: None,
            original_clock_time: None,
            original_global_shadows: None,
            original_grass_length: None,
            original_water_transparency: None,
            original_water_color: None,
            original_atmosphere_density: None,
            original_atmosphere_haze: None,
            original_atmosphere_glare: None,
            original_atmosphere_offset: None,
            original_atmosphere_color: None,
            original_bloom_enabled: None,
            original_bloom_intensity: None,
            original_bloom_size: None,
            original_bloom_threshold: None,
            original_dof_enabled: None,
            original_dof_far_intensity: None,
            original_dof_focus_distance: None,
            original_dof_in_focus_radius: None,
            original_dof_near_intensity: None,
            original_sunrays_enabled: None,
            original_sunrays_intensity: None,
            original_sunrays_spread: None,
        }
    }

    /// Apply all world modifications.
    pub fn apply_all(&mut self, config: &Config) {
        if is_valid_address(self.lighting_address) {
            self.apply_anti_fog(config);
            self.apply_fullbright(config);
            self.apply_force_lighting(config);
            self.apply_brightness(config);
            self.apply_anti_flash(config);
            self.apply_no_shadows(config);
        }
        
        if is_valid_address(self.terrain_address) {
            self.apply_terrain_control(config);
        }

        // Post-processing effects (only if offsets are available in dynamic map)
        if is_valid_address(self.atmosphere_address) {
            self.apply_atmosphere(config);
        }
        if is_valid_address(self.bloom_address) {
            self.apply_bloom(config);
        }
        if is_valid_address(self.dof_address) {
            self.apply_dof(config);
        }
        if is_valid_address(self.sunrays_address) {
            self.apply_sunrays(config);
        }
    }

    fn apply_anti_fog(&mut self, config: &Config) {
        if config.world.anti_fog {
            // Store original values on first enable
            if self.original_fog_start.is_none() {
                self.original_fog_start = Some(
                    self.memory.read::<f32>(self.lighting_address + lighting::fog_start()),
                );
                self.original_fog_end = Some(
                    self.memory.read::<f32>(self.lighting_address + lighting::fog_end()),
                );
            }

            self.memory.write::<f32>(self.lighting_address + lighting::fog_start(), config.world.fog_start);
            self.memory.write::<f32>(self.lighting_address + lighting::fog_end(), config.world.fog_end);
        } else if let (Some(orig_start), Some(orig_end)) = (self.original_fog_start, self.original_fog_end) {
            self.memory.write::<f32>(self.lighting_address + lighting::fog_start(), orig_start);
            self.memory.write::<f32>(self.lighting_address + lighting::fog_end(), orig_end);
            self.original_fog_start = None;
            self.original_fog_end = None;
        }
    }

    fn apply_fullbright(&mut self, config: &Config) {
        if config.world.fullbright {
            // Store originals if not already stored
            if self.original_ambient.is_none() {
                let amb: [f32; 3] = self.memory.read(self.lighting_address + lighting::ambient());
                self.original_ambient = Some(amb);
            }
            if self.original_outdoor_ambient.is_none() {
                let oamb: [f32; 3] = self.memory.read(self.lighting_address + lighting::outdoor_ambient());
                self.original_outdoor_ambient = Some(oamb);
            }
            if self.original_brightness.is_none() {
                self.original_brightness = Some(
                    self.memory.read::<f32>(self.lighting_address + lighting::brightness()),
                );
            }
            
            // Set fullbright: white ambient + high brightness
            self.memory.write::<[f32; 3]>(self.lighting_address + lighting::ambient(), [1.0, 1.0, 1.0]);
            self.memory.write::<[f32; 3]>(self.lighting_address + lighting::outdoor_ambient(), [1.0, 1.0, 1.0]);
            self.memory.write::<f32>(self.lighting_address + lighting::brightness(), 5.0);
        } else {
            // Fullbright disabled — restore brightness if we stored it and force_lighting isn't active
            if !config.world.force_lighting {
                if let Some(orig) = self.original_brightness.take() {
                    self.memory.write::<f32>(self.lighting_address + lighting::brightness(), orig);
                }
                if let Some(amb) = self.original_ambient.take() {
                    self.memory.write::<[f32; 3]>(self.lighting_address + lighting::ambient(), amb);
                }
                if let Some(oamb) = self.original_outdoor_ambient.take() {
                    self.memory.write::<[f32; 3]>(self.lighting_address + lighting::outdoor_ambient(), oamb);
                }
            }
        }
    }

    fn apply_force_lighting(&mut self, config: &Config) {
        if config.world.force_lighting && !config.world.fullbright {
            // Store originals if not already stored
            if self.original_ambient.is_none() {
                let amb: [f32; 3] = self.memory.read(self.lighting_address + lighting::ambient());
                self.original_ambient = Some(amb);
            }
            if self.original_outdoor_ambient.is_none() {
                let oamb: [f32; 3] = self.memory.read(self.lighting_address + lighting::outdoor_ambient());
                self.original_outdoor_ambient = Some(oamb);
            }
            if self.original_clock_time.is_none() {
                self.original_clock_time = Some(
                    self.memory.read::<f32>(self.lighting_address + lighting::clock_time()),
                );
            }
            
            // Apply custom lighting
            self.memory.write::<[f32; 3]>(self.lighting_address + lighting::ambient(), config.world.ambient_color);
            self.memory.write::<[f32; 3]>(self.lighting_address + lighting::outdoor_ambient(), config.world.outdoor_ambient_color);
            self.memory.write::<f32>(self.lighting_address + lighting::clock_time(), config.world.clock_time);
        } else if !config.world.fullbright && !config.world.force_lighting {
            // Restore if both are off
            if let Some(amb) = self.original_ambient.take() {
                self.memory.write::<[f32; 3]>(self.lighting_address + lighting::ambient(), amb);
            }
            if let Some(oamb) = self.original_outdoor_ambient.take() {
                self.memory.write::<[f32; 3]>(self.lighting_address + lighting::outdoor_ambient(), oamb);
            }
            if let Some(ct) = self.original_clock_time.take() {
                self.memory.write::<f32>(self.lighting_address + lighting::clock_time(), ct);
            }
        }
    }

    fn apply_no_shadows(&mut self, config: &Config) {
        if config.world.no_shadows {
            if self.original_global_shadows.is_none() {
                self.original_global_shadows = Some(
                    self.memory.read::<bool>(self.lighting_address + lighting::global_shadows()),
                );
            }
            self.memory.write::<bool>(self.lighting_address + lighting::global_shadows(), false);
        } else if let Some(orig) = self.original_global_shadows.take() {
            self.memory.write::<bool>(self.lighting_address + lighting::global_shadows(), orig);
        }
    }

    fn apply_brightness(&mut self, config: &Config) {
        // Skip if fullbright is on (it handles brightness)
        if config.world.fullbright {
            return;
        }
        
        if config.world.brightness_enabled {
            if self.original_brightness.is_none() {
                self.original_brightness = Some(
                    self.memory.read::<f32>(self.lighting_address + lighting::brightness()),
                );
            }

            self.memory.write::<f32>(self.lighting_address + lighting::brightness(), config.world.brightness_value);
        } else if let Some(orig) = self.original_brightness.take() {
            self.memory.write::<f32>(self.lighting_address + lighting::brightness(), orig);
        }
    }

    fn apply_anti_flash(&self, config: &Config) {
        // Skip if fullbright is on
        if config.world.fullbright {
            return;
        }
        
        if config.world.anti_flash {
            let current = self.memory.read::<f32>(self.lighting_address + lighting::brightness());

            if current > config.world.max_brightness {
                self.memory.write::<f32>(self.lighting_address + lighting::brightness(), config.world.max_brightness);
            }
        }
    }

    fn apply_terrain_control(&mut self, config: &Config) {
        if config.world.terrain_enabled {
            // Store originals
            if self.original_grass_length.is_none() {
                self.original_grass_length = Some(
                    self.memory.read::<f32>(self.terrain_address + terrain_offsets::grass_length()),
                );
            }
            if self.original_water_transparency.is_none() {
                self.original_water_transparency = Some(
                    self.memory.read::<f32>(self.terrain_address + terrain_offsets::water_transparency()),
                );
            }
            if self.original_water_color.is_none() {
                let wc: [f32; 3] = self.memory.read(self.terrain_address + terrain_offsets::water_color());
                self.original_water_color = Some(wc);
            }
            
            // Apply terrain settings
            self.memory.write::<f32>(self.terrain_address + terrain_offsets::grass_length(), config.world.grass_length);
            self.memory.write::<f32>(self.terrain_address + terrain_offsets::water_transparency(), config.world.water_transparency);
            self.memory.write::<[f32; 3]>(self.terrain_address + terrain_offsets::water_color(), config.world.water_color);
        } else {
            // Restore originals
            if let Some(gl) = self.original_grass_length.take() {
                self.memory.write::<f32>(self.terrain_address + terrain_offsets::grass_length(), gl);
            }
            if let Some(wt) = self.original_water_transparency.take() {
                self.memory.write::<f32>(self.terrain_address + terrain_offsets::water_transparency(), wt);
            }
            if let Some(wc) = self.original_water_color.take() {
                self.memory.write::<[f32; 3]>(self.terrain_address + terrain_offsets::water_color(), wc);
            }
        }
    }

    // Post-processing effects (Atmosphere, Bloom, DOF, SunRays).
    // Uses dynamic offsets from the dump's extra namespaces.
    fn apply_atmosphere(&mut self, config: &Config) {
        let addr = self.atmosphere_address;
        if config.world.atmosphere_enabled {
            if let (Some(density_off), Some(haze_off), Some(glare_off), Some(offset_off), Some(color_off)) = (
                try_get_offset("Atmosphere", "Density"),
                try_get_offset("Atmosphere", "Haze"),
                try_get_offset("Atmosphere", "Glare"),
                try_get_offset("Atmosphere", "Offset"),
                try_get_offset("Atmosphere", "Color"),
            ) {
                // Store originals
                if self.original_atmosphere_density.is_none() {
                    self.original_atmosphere_density = Some(self.memory.read::<f32>(addr + density_off));
                    self.original_atmosphere_haze = Some(self.memory.read::<f32>(addr + haze_off));
                    self.original_atmosphere_glare = Some(self.memory.read::<f32>(addr + glare_off));
                    self.original_atmosphere_offset = Some(self.memory.read::<f32>(addr + offset_off));
                    self.original_atmosphere_color = Some(self.memory.read::<[f32; 3]>(addr + color_off));
                }

                self.memory.write::<f32>(addr + density_off, config.world.atmosphere_density);
                self.memory.write::<f32>(addr + haze_off, config.world.atmosphere_haze);
                self.memory.write::<f32>(addr + glare_off, config.world.atmosphere_glare);
                self.memory.write::<f32>(addr + offset_off, config.world.atmosphere_offset);
                self.memory.write::<[f32; 3]>(addr + color_off, config.world.atmosphere_color);
            }
        } else {
            // Restore originals
            if let Some(orig) = self.original_atmosphere_density.take() {
                if let Some(off) = try_get_offset("Atmosphere", "Density") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_atmosphere_haze.take() {
                if let Some(off) = try_get_offset("Atmosphere", "Haze") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_atmosphere_glare.take() {
                if let Some(off) = try_get_offset("Atmosphere", "Glare") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_atmosphere_offset.take() {
                if let Some(off) = try_get_offset("Atmosphere", "Offset") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_atmosphere_color.take() {
                if let Some(off) = try_get_offset("Atmosphere", "Color") {
                    self.memory.write::<[f32; 3]>(addr + off, orig);
                }
            }
        }
    }

    fn apply_bloom(&mut self, config: &Config) {
        let addr = self.bloom_address;
        if config.world.bloom_enabled {
            if let (Some(enabled_off), Some(intensity_off), Some(size_off), Some(threshold_off)) = (
                try_get_offset("BloomEffect", "Enabled"),
                try_get_offset("BloomEffect", "Intensity"),
                try_get_offset("BloomEffect", "Size"),
                try_get_offset("BloomEffect", "Threshold"),
            ) {
                if self.original_bloom_enabled.is_none() {
                    self.original_bloom_enabled = Some(self.memory.read::<bool>(addr + enabled_off));
                    self.original_bloom_intensity = Some(self.memory.read::<f32>(addr + intensity_off));
                    self.original_bloom_size = Some(self.memory.read::<f32>(addr + size_off));
                    self.original_bloom_threshold = Some(self.memory.read::<f32>(addr + threshold_off));
                }

                self.memory.write::<bool>(addr + enabled_off, config.world.bloom_active);
                self.memory.write::<f32>(addr + intensity_off, config.world.bloom_intensity);
                self.memory.write::<f32>(addr + size_off, config.world.bloom_size);
                self.memory.write::<f32>(addr + threshold_off, config.world.bloom_threshold);
            }
        } else {
            if let Some(orig) = self.original_bloom_enabled.take() {
                if let Some(off) = try_get_offset("BloomEffect", "Enabled") {
                    self.memory.write::<bool>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_bloom_intensity.take() {
                if let Some(off) = try_get_offset("BloomEffect", "Intensity") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_bloom_size.take() {
                if let Some(off) = try_get_offset("BloomEffect", "Size") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_bloom_threshold.take() {
                if let Some(off) = try_get_offset("BloomEffect", "Threshold") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
        }
    }

    fn apply_dof(&mut self, config: &Config) {
        let addr = self.dof_address;
        if config.world.dof_enabled {
            if let (Some(enabled_off), Some(far_off), Some(focus_off), Some(radius_off), Some(near_off)) = (
                try_get_offset("DepthOfFieldEffect", "Enabled"),
                try_get_offset("DepthOfFieldEffect", "FarIntensity"),
                try_get_offset("DepthOfFieldEffect", "FocusDistance"),
                try_get_offset("DepthOfFieldEffect", "InFocusRadius"),
                try_get_offset("DepthOfFieldEffect", "NearIntensity"),
            ) {
                if self.original_dof_enabled.is_none() {
                    self.original_dof_enabled = Some(self.memory.read::<bool>(addr + enabled_off));
                    self.original_dof_far_intensity = Some(self.memory.read::<f32>(addr + far_off));
                    self.original_dof_focus_distance = Some(self.memory.read::<f32>(addr + focus_off));
                    self.original_dof_in_focus_radius = Some(self.memory.read::<f32>(addr + radius_off));
                    self.original_dof_near_intensity = Some(self.memory.read::<f32>(addr + near_off));
                }

                self.memory.write::<bool>(addr + enabled_off, config.world.dof_active);
                self.memory.write::<f32>(addr + far_off, config.world.dof_far_intensity);
                self.memory.write::<f32>(addr + focus_off, config.world.dof_focus_distance);
                self.memory.write::<f32>(addr + radius_off, config.world.dof_in_focus_radius);
                self.memory.write::<f32>(addr + near_off, config.world.dof_near_intensity);
            }
        } else {
            if let Some(orig) = self.original_dof_enabled.take() {
                if let Some(off) = try_get_offset("DepthOfFieldEffect", "Enabled") {
                    self.memory.write::<bool>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_dof_far_intensity.take() {
                if let Some(off) = try_get_offset("DepthOfFieldEffect", "FarIntensity") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_dof_focus_distance.take() {
                if let Some(off) = try_get_offset("DepthOfFieldEffect", "FocusDistance") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_dof_in_focus_radius.take() {
                if let Some(off) = try_get_offset("DepthOfFieldEffect", "InFocusRadius") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_dof_near_intensity.take() {
                if let Some(off) = try_get_offset("DepthOfFieldEffect", "NearIntensity") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
        }
    }

    fn apply_sunrays(&mut self, config: &Config) {
        let addr = self.sunrays_address;
        if config.world.sunrays_enabled {
            // Note: SunRaysEffect has Enabled and Intensity at the same offset (0xd0) in the dump.
            // Enabled is likely a bool at 0xd0, and Intensity is a float at a nearby offset.
            // We use separate offsets as provided by the dump.
            if let (Some(enabled_off), Some(intensity_off), Some(spread_off)) = (
                try_get_offset("SunRaysEffect", "Enabled"),
                try_get_offset("SunRaysEffect", "Intensity"),
                try_get_offset("SunRaysEffect", "Spread"),
            ) {
                if self.original_sunrays_enabled.is_none() {
                    self.original_sunrays_enabled = Some(self.memory.read::<bool>(addr + enabled_off));
                    self.original_sunrays_intensity = Some(self.memory.read::<f32>(addr + intensity_off));
                    self.original_sunrays_spread = Some(self.memory.read::<f32>(addr + spread_off));
                }

                self.memory.write::<bool>(addr + enabled_off, config.world.sunrays_active);
                self.memory.write::<f32>(addr + intensity_off, config.world.sunrays_intensity);
                self.memory.write::<f32>(addr + spread_off, config.world.sunrays_spread);
            }
        } else {
            if let Some(orig) = self.original_sunrays_enabled.take() {
                if let Some(off) = try_get_offset("SunRaysEffect", "Enabled") {
                    self.memory.write::<bool>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_sunrays_intensity.take() {
                if let Some(off) = try_get_offset("SunRaysEffect", "Intensity") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
            if let Some(orig) = self.original_sunrays_spread.take() {
                if let Some(off) = try_get_offset("SunRaysEffect", "Spread") {
                    self.memory.write::<f32>(addr + off, orig);
                }
            }
        }
    }
}
