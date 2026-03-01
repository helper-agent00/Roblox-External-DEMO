use eframe::egui;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::math::{Matrix4, Vector2, Vector3};

/// Max trail points per entity
const MAX_TRAIL_POINTS: usize = 40;
/// Max footprint dots per entity  
const MAX_FOOTPRINTS: usize = 60;
/// Footprint lifetime in seconds
const FOOTPRINT_LIFETIME_SECS: f32 = 8.0;
/// Minimum distance between footprint samples (studs)
const FOOTPRINT_MIN_DISTANCE: f32 = 2.0;
/// Minimum distance between trail samples (studs)
const TRAIL_MIN_DISTANCE: f32 = 0.5;
/// Trail point lifetime in seconds
const TRAIL_LIFETIME_SECS: f32 = 2.0;
/// Maximum entities to track
const MAX_TRACKED_ENTITIES: usize = 32;

#[derive(Clone)]
struct TimedPoint {
    world_pos: Vector3,
    time: Instant,
}

/// Per-entity tracking data
struct TrackedEntity {
    footprints: VecDeque<TimedPoint>,
    trail_points: VecDeque<TimedPoint>,
    last_footprint_pos: Vector3,
    last_trail_pos: Vector3,
    is_teammate: bool,
}

impl TrackedEntity {
    fn new() -> Self {
        Self {
            footprints: VecDeque::with_capacity(MAX_FOOTPRINTS),
            trail_points: VecDeque::with_capacity(MAX_TRAIL_POINTS),
            last_footprint_pos: Vector3::ZERO,
            last_trail_pos: Vector3::ZERO,
            is_teammate: false,
        }
    }
}

/// Footprint and trail tracking system.
pub struct FootprintTracker {
    entities: ahash::AHashMap<u64, TrackedEntity>,
}

impl FootprintTracker {
    pub fn new() -> Self {
        Self {
            entities: ahash::AHashMap::new(),
        }
    }

    /// Feed new entity data from ESP cache. Call each frame.
    pub fn update(&mut self, render_data: &[super::esp_cache::EspRenderData]) {
        let now = Instant::now();

        // Track which entities are still present
        let mut seen: ahash::AHashSet<u64> = ahash::AHashSet::new();

        for data in render_data {
            seen.insert(data.entity_key);

            let pos = data.world_pos;
            if pos.x == 0.0 && pos.y == 0.0 && pos.z == 0.0 {
                continue;
            }

            let entry = self.entities.entry(data.entity_key).or_insert_with(TrackedEntity::new);
            entry.is_teammate = data.is_teammate;

            // Footprints - sample at intervals on the ground plane
            let fp_dist = (pos.x - entry.last_footprint_pos.x).abs()
                + (pos.z - entry.last_footprint_pos.z).abs();
            if fp_dist > FOOTPRINT_MIN_DISTANCE || entry.footprints.is_empty() {
                // Use feet position (lower Y)
                let feet = Vector3::new(pos.x, pos.y - 2.5, pos.z);
                entry.footprints.push_back(TimedPoint { world_pos: feet, time: now });
                entry.last_footprint_pos = pos;
                if entry.footprints.len() > MAX_FOOTPRINTS {
                    entry.footprints.pop_front();
                }
            }

            // Trails - sample more frequently
            let trail_dist = pos.distance_to(entry.last_trail_pos);
            if trail_dist > TRAIL_MIN_DISTANCE || entry.trail_points.is_empty() {
                entry.trail_points.push_back(TimedPoint { world_pos: pos, time: now });
                entry.last_trail_pos = pos;
                if entry.trail_points.len() > MAX_TRAIL_POINTS {
                    entry.trail_points.pop_front();
                }
            }
        }

        // Cleanup old footprints and trails
        for entry in self.entities.values_mut() {
            entry.footprints.retain(|p| p.time.elapsed().as_secs_f32() < FOOTPRINT_LIFETIME_SECS);
            entry.trail_points.retain(|p| p.time.elapsed().as_secs_f32() < TRAIL_LIFETIME_SECS);
        }

        // Remove entities no longer tracked (keep for a few seconds for trails to fade)
        self.entities.retain(|key, entry| {
            seen.contains(key) || !entry.trail_points.is_empty() || !entry.footprints.is_empty()
        });

        // Limit total tracked entities
        if self.entities.len() > MAX_TRACKED_ENTITIES {
            // Remove entries by lowest address (deterministic, no allocation)
            while self.entities.len() > MAX_TRACKED_ENTITIES {
                let mut min_key = u64::MAX;
                for &k in self.entities.keys() {
                    if k < min_key {
                        min_key = k;
                    }
                }
                if min_key == u64::MAX { break; }
                self.entities.remove(&min_key);
            }
        }
    }

    /// Render footprint dots
    pub fn render_footprints(
        &self,
        ctx: &egui::Context,
        config: &Config,
        visengine: &Arc<VisualEngine>,
    ) {
        if !config.visuals.footprints {
            return;
        }

        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        egui::Area::new(egui::Id::new("footprint_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();
                for entry in self.entities.values() {
                    let base_color = if entry.is_teammate {
                        egui::Color32::from_rgb(59, 130, 246) // Blue
                    } else {
                        egui::Color32::from_rgb(255, 165, 0) // Orange
                    };

                    for fp in &entry.footprints {
                        let age = fp.time.elapsed().as_secs_f32();
                        let alpha = ((1.0 - age / FOOTPRINT_LIFETIME_SECS) * 120.0) as u8;
                        if alpha < 5 { continue; }

                        if let Some(sp) = Self::w2s(fp.world_pos, &view_matrix, dimensions, &window_offset, visengine) {
                            let color = egui::Color32::from_rgba_unmultiplied(
                                base_color.r(), base_color.g(), base_color.b(), alpha,
                            );
                            painter.circle_filled(egui::pos2(sp.x, sp.y), 2.5, color);
                        }
                    }
                }
            });
    }

    /// Render movement trails
    pub fn render_trails(
        &self,
        ctx: &egui::Context,
        config: &Config,
        visengine: &Arc<VisualEngine>,
    ) {
        if !config.visuals.movement_trails {
            return;
        }

        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        egui::Area::new(egui::Id::new("trail_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();
                for entry in self.entities.values() {
                    if entry.trail_points.len() < 2 {
                        continue;
                    }

                    let base_color = if entry.is_teammate {
                        egui::Color32::from_rgb(59, 130, 246)
                    } else {
                        egui::Color32::from_rgb(139, 92, 246) // Purple
                    };

                    let mut prev_sp: Option<Vector2> = None;
                    for tp in &entry.trail_points {
                        let age = tp.time.elapsed().as_secs_f32();
                        let alpha = ((1.0 - age / TRAIL_LIFETIME_SECS) * 180.0) as u8;
                        if alpha < 5 { continue; }

                        if let Some(sp) = Self::w2s(tp.world_pos, &view_matrix, dimensions, &window_offset, visengine) {
                            if let Some(prev) = prev_sp {
                                let color = egui::Color32::from_rgba_unmultiplied(
                                    base_color.r(), base_color.g(), base_color.b(), alpha,
                                );
                                let thickness = 1.0 + (1.0 - age / TRAIL_LIFETIME_SECS) * 1.5;
                                painter.line_segment(
                                    [egui::pos2(prev.x, prev.y), egui::pos2(sp.x, sp.y)],
                                    egui::Stroke::new(thickness, color),
                                );
                            }
                            prev_sp = Some(sp);
                        } else {
                            prev_sp = None;
                        }
                    }
                }
            });
    }

    fn w2s(
        world: Vector3,
        view_matrix: &Matrix4,
        dimensions: Vector2,
        window_offset: &Vector2,
        visengine: &Arc<VisualEngine>,
    ) -> Option<Vector2> {
        let sp = visengine.world_to_screen(world, dimensions, view_matrix)?;
        Some(Vector2::new(sp.x + window_offset.x, sp.y + window_offset.y))
    }
}

impl Default for FootprintTracker {
    fn default() -> Self {
        Self::new()
    }
}
