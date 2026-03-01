use ahash::{AHashMap, AHashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache, Entity};
use crate::utils::map_parser::get_map_parser;
use crate::utils::math::{Matrix4, Vector2, Vector3};
use crate::utils::game_support::GameId;
use crate::utils::velocity::{is_teammate, INTERPOLATION_TIME};

const LOD_HIGH_INTERVAL_US: u64 = 4000;
const LOD_MEDIUM_INTERVAL_US: u64 = 8000;
const LOD_LOW_INTERVAL_US: u64 = 16000;

const WALL_CACHE_HIGH_MS: u64 = 100;
const WALL_CACHE_MEDIUM_MS: u64 = 300;
const WALL_CACHE_LOW_DISTANCE: f32 = 150.0;

const LOD_HIGH_DISTANCE: f32 = 50.0;
const LOD_MEDIUM_DISTANCE: f32 = 150.0;

const MAX_RENDER_ENTITIES_BASE: usize = 64;

#[inline]
fn intensity_lod_multiplier(intensity: f32) -> f32 {
    1.0 + 4.0 * (1.0 - intensity.clamp(0.0, 1.0))
}

#[inline]
fn max_render_entities(intensity: f32) -> usize {
    let scale = 0.25 + 0.75 * intensity.clamp(0.0, 1.0);
    (MAX_RENDER_ENTITIES_BASE as f32 * scale).round() as usize
}

#[inline]
fn entity_persist_time_ms(intensity: f32) -> u64 {
    let base = 500.0 + 300.0 * (1.0 - intensity.clamp(0.0, 1.0));
    base as u64
}

#[inline]
fn stale_max_age_ms(intensity: f32) -> u128 {
    (200.0 + 300.0 * (1.0 - intensity.clamp(0.0, 1.0))) as u128
}

#[inline]
fn wall_cache_multiplier(intensity: f32) -> f32 {
    1.0 + 2.0 * (1.0 - intensity.clamp(0.0, 1.0))
}

#[derive(Clone)]
pub struct EspRenderData {
    #[allow(dead_code)]
    pub entity_key: u64,
    /// `Arc<str>` for near-free cloning (atomic inc vs String heap alloc).
    pub name: Arc<str>,
    pub distance: f32,
    pub is_aim_target: bool,
    pub health_percent: f32,
    pub armor_percent: f32,
    pub has_armor: bool,
    pub is_teammate: bool,
    pub is_visible: bool,
    #[allow(dead_code)]
    pub lod_level: LodLevel,
    pub world_pos: Vector3,
    pub world_bottom: Vector3,
    pub world_top: Vector3,
    pub box_3d_corners_world: Option<[Vector3; 8]>,
    pub computed_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LodLevel {
    High,
    Medium,
    Low,
}

impl LodLevel {
    pub fn from_distance(distance: f32) -> Self {
        if distance < LOD_HIGH_DISTANCE {
            LodLevel::High
        } else if distance < LOD_MEDIUM_DISTANCE {
            LodLevel::Medium
        } else {
            LodLevel::Low
        }
    }

    #[allow(dead_code)]
    pub fn interval_us(&self) -> u64 {
        self.interval_us_scaled(1.0)
    }

    pub fn interval_us_scaled(&self, intensity: f32) -> u64 {
        let base = match self {
            LodLevel::High => LOD_HIGH_INTERVAL_US,
            LodLevel::Medium => LOD_MEDIUM_INTERVAL_US,
            LodLevel::Low => LOD_LOW_INTERVAL_US,
        };
        (base as f32 * intensity_lod_multiplier(intensity)) as u64
    }
}

#[derive(Clone)]
#[allow(dead_code)] // Fields read via snapshot pattern - all populated, some consumed internally
struct ConfigSnapshot {
    box_enabled: bool,
    name_enabled: bool,
    tracers_enabled: bool,
    distance_colors: bool,
    target_highlight: bool,
    max_distance: f32,
    team_check: bool,
    hide_dead: bool,
    wall_check: bool,
    box_color: [f32; 3],
    teammate_whitelist: Vec<String>,
    esp_intensity: f32,
}

impl ConfigSnapshot {
    fn from_config(config: &Config) -> Self {
        Self {
            box_enabled: config.visuals.box_esp,
            name_enabled: config.visuals.name_tags,
            tracers_enabled: config.visuals.tracers,
            distance_colors: config.visuals.distance_colors,
            target_highlight: config.visuals.target_highlight,
            max_distance: config.visuals.max_distance,
            team_check: config.visuals.team_check,
            hide_dead: config.visuals.hide_dead,
            wall_check: config.visuals.wall_check,
            box_color: config.visuals.box_color,
            teammate_whitelist: config.visuals.teammate_whitelist.clone(),
            esp_intensity: config.visuals.esp_intensity.clamp(0.0, 1.0),
        }
    }
}

/// Uses double buffering to prevent flickering - writer fills back buffer,
/// then atomically swaps with front buffer. Reader always gets complete frame.
pub struct EspRenderCache {
    buffer_a: RwLock<Arc<Vec<EspRenderData>>>,
    buffer_b: RwLock<Arc<Vec<EspRenderData>>>,
    front_buffer_idx: AtomicU64,
    dimensions: RwLock<Vector2>,
    window_offset: RwLock<Vector2>,
    config_snapshot: RwLock<ConfigSnapshot>,
    local_player_name: RwLock<String>,
    /// Aim target name for highlighting (name-based, survives respawns)
    aim_target_name: RwLock<String>,
    running: AtomicBool,
    last_update_us: AtomicU64,
    entity_count: AtomicU64,
}

impl EspRenderCache {
    pub fn new() -> Self {
        Self {
            buffer_a: RwLock::new(Arc::new(Vec::with_capacity(MAX_RENDER_ENTITIES_BASE))),
            buffer_b: RwLock::new(Arc::new(Vec::with_capacity(MAX_RENDER_ENTITIES_BASE))),
            front_buffer_idx: AtomicU64::new(0),
            dimensions: RwLock::new(Vector2::ZERO),
            window_offset: RwLock::new(Vector2::ZERO),
            config_snapshot: RwLock::new(ConfigSnapshot {
                box_enabled: false,
                name_enabled: false,
                tracers_enabled: false,
                distance_colors: false,
                target_highlight: false,
                max_distance: 2000.0,
                team_check: true,
                hide_dead: true,
                wall_check: false,
                box_color: [1.0, 1.0, 1.0],
                teammate_whitelist: Vec::new(),
                esp_intensity: 0.75,
            }),
            local_player_name: RwLock::new(String::new()),
            aim_target_name: RwLock::new(String::new()),
            running: AtomicBool::new(false),
            last_update_us: AtomicU64::new(0),
            entity_count: AtomicU64::new(0),
        }
    }

    pub fn update_config(&self, config: &Config) {
        if let Ok(mut snapshot) = self.config_snapshot.write() {
            *snapshot = ConfigSnapshot::from_config(config);
        }
    }

    pub fn set_local_player_name(&self, name: &str) {
        if let Ok(mut lpn) = self.local_player_name.write() {
            if *lpn != name {
                *lpn = name.to_string();
            }
        }
    }

    pub fn set_aim_target_name(&self, name: &str) {
        if let Ok(mut n) = self.aim_target_name.write() {
            if *n != name {
                *n = name.to_string();
            }
        }
    }

    pub fn clear_aim_target(&self) {
        if let Ok(mut n) = self.aim_target_name.write() {
            if !n.is_empty() {
                n.clear();
            }
        }
    }

    pub fn get_render_data(&self) -> Arc<Vec<EspRenderData>> {
        let front_idx = self.front_buffer_idx.load(Ordering::Acquire);
        let buffer = if front_idx == 0 { &self.buffer_a } else { &self.buffer_b };
        buffer.read().map(|r| Arc::clone(&r)).unwrap_or_else(|_| Arc::new(Vec::new()))
    }

    #[allow(dead_code)]
    pub fn get_entity_count(&self) -> u64 {
        self.entity_count.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn get_last_update_us(&self) -> u64 {
        self.last_update_us.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn start(
        self: &Arc<Self>,
        cache: Arc<Cache>,
        visengine: Arc<VisualEngine>,
    ) {
        if self.running.swap(true, Ordering::SeqCst) {
            tracing::warn!("ESP cache thread already running");
            return;
        }

        let esp_cache = Arc::clone(self);
        let cache = Arc::clone(&cache);
        let visengine = Arc::clone(&visengine);

        thread::Builder::new()
            .name("esp-data-thread".into())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                unsafe {
                    use windows::Win32::System::Threading::{
                        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL,
                    };
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
                }

                tracing::info!("ESP data thread started");

                // Prevents flickering by keeping entities visible between LOD updates
                let mut entity_cache: ahash::AHashMap<u64, EspRenderData> = ahash::AHashMap::new();
                let mut entity_last_seen: ahash::AHashMap<u64, Instant> = ahash::AHashMap::new();

                let mut wall_check_cache: AHashMap<u64, (bool, Instant)> = AHashMap::new();

                let mut last_local_pos = Vector3::ZERO;
                const FAST_MOVEMENT_THRESHOLD_SQ: f32 = 2500.0; // ~50 studs/frame — teleport/spawn only

                let mut last_snapshot_ptr: usize = 0;

                while esp_cache.running.load(Ordering::Relaxed) {
                    let frame_start = Instant::now();

                    let config = {
                        match esp_cache.config_snapshot.read() {
                            Ok(c) => c.clone(),
                            Err(_) => continue,
                        }
                    };
                    let local_name = match esp_cache.local_player_name.read() {
                        Ok(n) => n.clone(),
                        Err(_) => continue,
                    };

                    if !config.box_enabled && !config.name_enabled && !config.tracers_enabled {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }

                    let dimensions = visengine.get_dimensions();
                    let window_offset = visengine.get_window_offset();

                    if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    if let Ok(mut d) = esp_cache.dimensions.write() {
                        *d = dimensions;
                    }
                    if let Ok(mut w) = esp_cache.window_offset.write() {
                        *w = window_offset;
                    }

                    let (view_matrix, snapshot, aim_target_name, snapshot_changed) = {
                        let vm = visengine.get_view_matrix();
                        let snap = cache.get_snapshot();
                        let aim = esp_cache.aim_target_name.read()
                            .map(|n| n.clone())
                            .unwrap_or_default();
                        // Cheap pointer identity check: if the Arc points to the same
                        // allocation as last tick, the cache thread hasn't published
                        // new data — we can skip the expensive entity loop entirely.
                        let snap_ptr = Arc::as_ptr(&snap) as usize;
                        let changed = snap_ptr != last_snapshot_ptr;
                        last_snapshot_ptr = snap_ptr;
                        (vm, snap, aim, changed)
                    };

                    // If not found (e.g., Aftermath where entities have GUID names),
                    // fall back to camera position which tracks the player.
                    let local_entity = snapshot.iter().find(|e| e.name.eq_ignore_ascii_case(&local_name));
                    let local_pos = local_entity
                        .and_then(|e| e.root_position())
                        .or_else(|| visengine.get_camera_position())
                        .unwrap_or(Vector3::ZERO);

                    let local_team = cache.get_local_team_addr();
                    let local_team_identifier = cache.get_local_team_id();
                    let game_id = cache.get_game_id();

                    let local_velocity_vec = local_pos - last_local_pos;
                    let local_velocity_sq = local_velocity_vec.x * local_velocity_vec.x 
                        + local_velocity_vec.y * local_velocity_vec.y 
                        + local_velocity_vec.z * local_velocity_vec.z;
                    last_local_pos = local_pos;

                    let is_moving_fast = local_velocity_sq > FAST_MOVEMENT_THRESHOLD_SQ;

                    // Invalidate distant entity caches when moving fast (they'd be stale/stuck)
                    if is_moving_fast {
                        entity_cache.retain(|_, data| data.distance < LOD_HIGH_DISTANCE);
                    }

                    let teammate_addresses: AHashSet<u64> = if config.team_check && !config.teammate_whitelist.is_empty() {
                        snapshot.iter()
                            .filter(|e| config.teammate_whitelist.iter().any(|n| n.eq_ignore_ascii_case(&e.name)))
                            .map(|e| e.model_address)
                            .collect()
                    } else {
                        AHashSet::new()
                    };

                    let now = Instant::now();

                    let mut seen_entities: AHashSet<u64> = AHashSet::new();

                    // The cache thread publishes new snapshots at ~80Hz. Between publishes
                    // the entity data is identical, so re-iterating wastes CPU cycles on
                    // weaker machines. When unchanged, we jump straight to the sleep.
                    // The render thread still re-projects cached world positions each frame
                    // so visual smoothness is unaffected.
                    if !snapshot_changed && !entity_cache.is_empty() {
                        // Still need to sleep to avoid busy-looping
                        let elapsed = frame_start.elapsed();
                        let min_sleep = Duration::from_micros(LOD_HIGH_INTERVAL_US);
                        if elapsed < min_sleep {
                            thread::sleep(min_sleep - elapsed);
                        }
                        continue;
                    }

                    {
                    for entity in snapshot.iter() {
                        if entity.name.eq_ignore_ascii_case(&local_name) {
                            continue;
                        }
                        // For games where entity names are GUIDs (entity-folder games),
                        // skip the entity closest to camera position (likely the local player)
                        if local_entity.is_none() {
                            if let Some(rp) = entity.root_position() {
                                if rp.distance_to(local_pos) < 15.0 {
                                    continue;
                                }
                            }
                        }

                        let entity_key = entity.model_address;
                        seen_entities.insert(entity_key);
                        entity_last_seen.insert(entity_key, now);

                        let root_part = entity.root_part();

                        let root_pos = match root_part {
                            Some(p) => Vector3::new(
                                p.position.x + entity.velocity.x * INTERPOLATION_TIME,
                                p.position.y + entity.velocity.y * INTERPOLATION_TIME,
                                p.position.z + entity.velocity.z * INTERPOLATION_TIME,
                            ),
                            None => continue,
                        };

                        if !root_pos.is_valid() || root_pos.is_near_origin(1.0) {
                            continue;
                        }

                        let distance = root_pos.distance_to(local_pos);
                        if distance > config.max_distance {
                            entity_cache.remove(&entity_key);
                            continue;
                        }

                        if config.hide_dead && entity.is_dead() {
                            entity_cache.remove(&entity_key);
                            wall_check_cache.remove(&entity_key);
                            continue;
                        }

                        let lod = LodLevel::from_distance(distance);

                        if let Some(cached) = entity_cache.get(&entity_key) {
                            let elapsed_us = cached.computed_at.elapsed().as_micros() as u64;
                            if elapsed_us < lod.interval_us_scaled(config.esp_intensity) {
                                // Prevents stale screen-space coords from "sticking"
                                // when the camera rotates.
                                let still_on_screen = visengine.world_to_screen(
                                    cached.world_pos, dimensions, &view_matrix,
                                ).is_some();
                                if still_on_screen {
                                    continue;
                                }
                            }
                        }

                        // Avoids heap allocation (just atomic inc) when name is unchanged.
                        let cached_name: Option<Arc<str>> = entity_cache.get(&entity_key)
                            .filter(|c| c.name.as_ref() == entity.name.as_str())
                            .map(|c| Arc::clone(&c.name));

                        if let Some(data) = Self::compute_entity_data(
                            entity,
                            cached_name,
                            &local_pos,
                            &teammate_addresses,
                            &config,
                            &aim_target_name,
                            &visengine,
                            dimensions,
                            &view_matrix,
                            &window_offset,
                            &local_team_identifier,
                            local_team,
                            lod,
                            distance,
                            &mut wall_check_cache,
                            game_id,
                        ) {
                            entity_cache.insert(entity_key, data);
                        }
                        // NOTE: Do NOT remove from cache on None — keep stale data.
                        // The render thread re-projects world positions each frame, so stale
                        // metadata (health/team) is acceptable. Removing causes flicker when
                        // entities are momentarily off-screen or at screen edges.
                        // The max_age and persist-time filters handle true cleanup.
                    }
                    }

                    let persist_ms = entity_persist_time_ms(config.esp_intensity) as u128;
                    entity_cache.retain(|key, _| {
                        entity_last_seen.get(key)
                            .map(|t| t.elapsed().as_millis() < persist_ms)
                            .unwrap_or(false)
                    });
                    entity_last_seen.retain(|_, v| v.elapsed().as_millis() < persist_ms);

                    wall_check_cache.retain(|_, (_, timestamp)| timestamp.elapsed().as_millis() < 1000);

                    let (render_list, entity_count) = {
                        let max_age_ms = stale_max_age_ms(config.esp_intensity);
                        let max_entities = max_render_entities(config.esp_intensity);
                        let mut render_list: Vec<EspRenderData> = entity_cache.values()
                            .filter(|d| d.computed_at.elapsed().as_millis() < max_age_ms)
                            .cloned()
                            .collect();

                        // Use select_nth_unstable (O(n)) to partition the nearest N,
                        // then sort only those N. Avoids O(n log n) full sort.
                        if render_list.len() > max_entities {
                            render_list.select_nth_unstable_by(max_entities - 1, |a, b| {
                                a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
                            });
                            render_list.truncate(max_entities);
                        }
                        render_list.sort_unstable_by(|a, b| {
                            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        let count = render_list.len() as u64;
                        (render_list, count)
                    };

                    let front_idx = esp_cache.front_buffer_idx.load(Ordering::Acquire);
                    let back_buffer = if front_idx == 0 { &esp_cache.buffer_b } else { &esp_cache.buffer_a };

                    if let Ok(mut data) = back_buffer.write() {
                        *data = Arc::new(render_list);
                    }

                    esp_cache.front_buffer_idx.store(if front_idx == 0 { 1 } else { 0 }, Ordering::Release);

                    esp_cache.entity_count.store(entity_count, Ordering::Relaxed);
                    esp_cache.last_update_us.store(frame_start.elapsed().as_micros() as u64, Ordering::Relaxed);

                    // Use plain thread::sleep — the ~1ms OS jitter is invisible at these
                    // update rates, and spin-waiting was burning an entire CPU core on
                    // weaker machines (the #1 reported lag source).
                    let elapsed = frame_start.elapsed();
                    let min_sleep = Duration::from_micros(LOD_HIGH_INTERVAL_US);
                    if elapsed < min_sleep {
                        thread::sleep(min_sleep - elapsed);
                    }
                }

                tracing::info!("ESP data thread stopped");
            })
            .expect("Failed to spawn ESP data thread");
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn compute_entity_data(
        entity: &Entity,
        cached_name: Option<Arc<str>>,
        local_pos: &Vector3,
        teammate_addresses: &AHashSet<u64>,
        config: &ConfigSnapshot,
        aim_target_name: &str,
        visengine: &Arc<VisualEngine>,
        dimensions: Vector2,
        view_matrix: &Matrix4,
        window_offset: &Vector2,
        local_team_identifier: &str,
        local_team: u64,
        lod: LodLevel,
        distance: f32,
        wall_check_cache: &mut AHashMap<u64, (bool, Instant)>,
        game_id: GameId,
    ) -> Option<EspRenderData> {
        if config.hide_dead && entity.is_dead() {
            return None;
        }

        let is_teammate = is_teammate(
            entity, config.team_check, local_team, teammate_addresses, local_team_identifier, game_id,
        );

        let velocity = entity.velocity;

        let root_part = entity.root_part()?;

        let root_pos = Vector3::new(
            root_part.position.x + velocity.x * INTERPOLATION_TIME,
            root_part.position.y + velocity.y * INTERPOLATION_TIME,
            root_part.position.z + velocity.z * INTERPOLATION_TIME,
        );

        let head_y = entity.parts.get(&BodyPart::Head)
            .map(|h| h.position.y + velocity.y * INTERPOLATION_TIME)
            .unwrap_or(root_pos.y + 1.5);

        let feet_offset = if entity.rig_type == 1 { 2.35 } else { 2.5 };
        let head_offset = 0.6;

        let bottom_world = Vector3::new(root_pos.x, root_pos.y - feet_offset, root_pos.z);
        let top_y = if entity.parts.contains_key(&BodyPart::Head) {
            head_y + head_offset
        } else {
            root_pos.y + if entity.rig_type == 1 { 2.5 } else { 2.0 }
        };
        let top_world = Vector3::new(root_pos.x, top_y, root_pos.z);

        // Screen projection is for size validation only — do NOT fail if
        // the entity is currently off-screen.  World positions are cached
        // regardless so the render thread can still draw tracers toward
        // entities that are in front of the camera but outside the tight
        // NDC frustum used for box ESP.
        let too_small_on_screen = match (
            visengine.world_to_screen(bottom_world, dimensions, view_matrix),
            visengine.world_to_screen(top_world, dimensions, view_matrix),
        ) {
            (Some(bs), Some(ts)) => {
                let bsy = bs.y + window_offset.y;
                let tsy = ts.y + window_offset.y;
                let base_height = (bsy - tsy).abs();
                let height = base_height * 1.14;
                height < 5.0
            }
            // Off-screen: not "too small", just out of the box-ESP frustum.
            // Allow through so tracers and name tags work when re-projected
            // with the render thread's current view matrix.
            _ => false,
        };
        if too_small_on_screen {
            return None;
        }

        let is_aim_target = !aim_target_name.is_empty() && entity.name.eq_ignore_ascii_case(aim_target_name);

        let health_percent = if entity.max_health > 0.0 {
            (entity.health / entity.max_health).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let has_armor = entity.max_armor > 0.0;
        let armor_percent = if has_armor {
            (entity.armor / entity.max_armor).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let is_visible = if config.wall_check && distance < WALL_CACHE_LOW_DISTANCE {
            let entity_key = entity.model_address;
            let wcm = wall_cache_multiplier(config.esp_intensity);
            let cache_lifetime_ms = if distance < LOD_HIGH_DISTANCE {
                (WALL_CACHE_HIGH_MS as f32 * wcm) as u64
            } else {
                (WALL_CACHE_MEDIUM_MS as f32 * wcm) as u64
            };

            let cached = wall_check_cache.get(&entity_key)
                .filter(|(_, ts)| ts.elapsed().as_millis() < cache_lifetime_ms as u128)
                .map(|(v, _)| *v);

            if let Some(cached_val) = cached {
                cached_val
            } else {
                let local_valid = local_pos.x.abs() > 10.0 || local_pos.y.abs() > 10.0 || local_pos.z.abs() > 10.0;
                let result = if !local_valid {
                    true
                } else {
                    let head_pos = entity.parts.get(&BodyPart::Head)
                        .map(|p| p.position)
                        .unwrap_or(root_pos);
                    get_map_parser().is_visible(*local_pos, head_pos)
                };
                wall_check_cache.insert(entity_key, (result, Instant::now()));
                result
            }
        } else {
            true
        };

        get_map_parser().log_debug_stats();

        let feet_w = if entity.rig_type == 1 { 1.2 } else { 1.5 };
        let feet_d = feet_w * 0.7;

        let corners_world = [
            Vector3::new(root_pos.x - feet_w, top_y, root_pos.z - feet_d),
            Vector3::new(root_pos.x + feet_w, top_y, root_pos.z - feet_d),
            Vector3::new(root_pos.x + feet_w, top_y, root_pos.z + feet_d),
            Vector3::new(root_pos.x - feet_w, top_y, root_pos.z + feet_d),
            Vector3::new(root_pos.x - feet_w, bottom_world.y, root_pos.z - feet_d),
            Vector3::new(root_pos.x + feet_w, bottom_world.y, root_pos.z - feet_d),
            Vector3::new(root_pos.x + feet_w, bottom_world.y, root_pos.z + feet_d),
            Vector3::new(root_pos.x - feet_w, bottom_world.y, root_pos.z + feet_d),
        ];

        Some(EspRenderData {
            entity_key: entity.model_address,
            name: cached_name.unwrap_or_else(|| Arc::from(entity.name.as_str())),
            distance,
            is_aim_target,
            health_percent,
            armor_percent,
            has_armor,
            is_teammate,
            is_visible,
            lod_level: lod,
            world_pos: root_pos,
            world_bottom: bottom_world,
            world_top: top_world,
            box_3d_corners_world: Some(corners_world),
            computed_at: Instant::now(),
        })
    }
}

impl Default for EspRenderCache {
    fn default() -> Self {
        Self::new()
    }
}
