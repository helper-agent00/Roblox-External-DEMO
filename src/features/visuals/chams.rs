use ahash::AHashSet;
use eframe::egui;
use rayon::prelude::*;
use std::sync::Arc;

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::cache::{Cache, BodyPart, PartData};
use crate::utils::map_parser::get_map_parser;
use crate::utils::math::{Matrix4, Vector2, Vector3};
use crate::utils::velocity::{is_teammate, INTERPOLATION_TIME};

/// The 8 corners of a unit cube centered at origin
const OBB_CORNERS: [Vector3; 8] = [
    Vector3 { x: -1.0, y: -1.0, z: -1.0 },
    Vector3 { x:  1.0, y: -1.0, z: -1.0 },
    Vector3 { x: -1.0, y:  1.0, z: -1.0 },
    Vector3 { x:  1.0, y:  1.0, z: -1.0 },
    Vector3 { x: -1.0, y: -1.0, z:  1.0 },
    Vector3 { x:  1.0, y: -1.0, z:  1.0 },
    Vector3 { x: -1.0, y:  1.0, z:  1.0 },
    Vector3 { x:  1.0, y:  1.0, z:  1.0 },
];

/// Convex hull polygon for a single body part
struct PartHull {
    /// Screen-space vertices of the convex hull
    vertices: Vec<Vector2>,
    /// Whether this body part is behind a wall (occluded)
    is_occluded: bool,
}

/// All rendering data for one entity
struct ChamsEntityData {
    /// Convex hulls for each visible body part
    part_hulls: Vec<PartHull>,
    /// Distance from local player (for LOD decisions)
    distance: f32,
    /// Colors for visible parts
    fill_color: egui::Color32,
    glow_color: egui::Color32,
    outline_color: egui::Color32,
    /// Colors for occluded parts (behind walls)
    occluded_fill_color: egui::Color32,
    occluded_glow_color: egui::Color32,
    occluded_outline_color: egui::Color32,
}

/// Compute 2D convex hull using Andrew's monotone chain algorithm.
/// Returns vertices in counter-clockwise order.
fn compute_convex_hull(mut points: Vec<Vector2>) -> Vec<Vector2> {
    if points.len() < 3 {
        return points;
    }

    // Sort by x, then by y
    points.sort_by(|a, b| {
        if (a.x - b.x).abs() < 0.001 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // Remove duplicates
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5);

    if points.len() < 3 {
        return points;
    }

    let mut hull: Vec<Vector2> = Vec::with_capacity(points.len() * 2);

    // Build lower hull
    for p in &points {
        while hull.len() >= 2 {
            let o = &hull[hull.len() - 2];
            let a = &hull[hull.len() - 1];
            // Cross product: (a - o) × (p - o)
            let cross = (a.x - o.x) * (p.y - o.y) - (a.y - o.y) * (p.x - o.x);
            if cross > 0.0 {
                break;
            }
            hull.pop();
        }
        hull.push(*p);
    }

    // Build upper hull
    let lower_len = hull.len() + 1;
    for p in points.iter().rev() {
        while hull.len() >= lower_len {
            let o = &hull[hull.len() - 2];
            let a = &hull[hull.len() - 1];
            let cross = (a.x - o.x) * (p.y - o.y) - (a.y - o.y) * (p.x - o.x);
            if cross > 0.0 {
                break;
            }
            hull.pop();
        }
        hull.push(*p);
    }

    hull.pop(); // Remove duplicate last point
    hull
}

pub struct Chams;

impl Chams {
    /// Project OBB corners of a body part to screen space and compute convex hull
    fn compute_part_hull(
        part: &PartData,
        velocity: Vector3,
        visengine: &Arc<VisualEngine>,
        dimensions: Vector2,
        view_matrix: &Matrix4,
        window_offset: &Vector2,
    ) -> Option<PartHull> {
        let pos = part.position;
        let rot = part.rotation;

        // Skip invalid parts
        if part.size.x < 0.01 || part.size.y < 0.01 || part.size.z < 0.01 {
            return None;
        }

        // Clamp minimum OBB size — prevents degenerate hulls from custom rigs
        // whose collision parts may be very small in one dimension.
        const MIN_CHAMS_DIM: f32 = 0.3;
        let size = Vector3::new(
            part.size.x.max(MIN_CHAMS_DIM),
            part.size.y.max(MIN_CHAMS_DIM),
            part.size.z.max(MIN_CHAMS_DIM),
        );

        // Velocity interpolation for smoother tracking
        let interp_pos = Vector3::new(
            pos.x + velocity.x * INTERPOLATION_TIME,
            pos.y + velocity.y * INTERPOLATION_TIME,
            pos.z + velocity.z * INTERPOLATION_TIME,
        );

        // Project all 8 OBB corners
        let mut screen_points: Vec<Vector2> = Vec::with_capacity(8);

        for corner in &OBB_CORNERS {
            // Scale corner by half-size (OBB extents)
            let local = Vector3::new(
                corner.x * size.x * 0.5,
                corner.y * size.y * 0.5,
                corner.z * size.z * 0.5,
            );

            // Transform by rotation matrix and add position
            let world = Vector3::new(
                interp_pos.x + rot.m[0] * local.x + rot.m[1] * local.y + rot.m[2] * local.z,
                interp_pos.y + rot.m[3] * local.x + rot.m[4] * local.y + rot.m[5] * local.z,
                interp_pos.z + rot.m[6] * local.x + rot.m[7] * local.y + rot.m[8] * local.z,
            );

            // Project to screen
            if let Some(screen) = visengine.world_to_screen(world, dimensions, view_matrix) {
                screen_points.push(Vector2::new(
                    screen.x + window_offset.x,
                    screen.y + window_offset.y,
                ));
            }
        }

        // Need at least 3 points for a hull
        if screen_points.len() < 3 {
            return None;
        }

        // Compute convex hull
        let hull = compute_convex_hull(screen_points);
        
        if hull.len() < 3 {
            return None;
        }

        Some(PartHull { vertices: hull, is_occluded: false })
    }

    /// Render chams for all visible players.
    pub fn render(
        ctx: &egui::Context,
        cache: &Arc<Cache>,
        visengine: &Arc<VisualEngine>,
        config: &Config,
        aim_target_name: Option<&str>,
        local_player_name: &str,
    ) {
        if !config.visuals.chams {
            return;
        }
        let snapshot = cache.get_snapshot();
        if snapshot.is_empty() {
            return;
        }

        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let max_distance = config.visuals.max_distance;
        let target_highlight = config.visuals.target_highlight;
        let team_check = config.visuals.team_check;
        let hide_dead = config.visuals.hide_dead;
        let wall_check = config.visuals.wall_check;

        // Get local player info
        let local_entity = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name));
        
        // Use Head position for wall checks — it's ~1.5 studs above HRP,
        // which lifts the ray origin above floor surfaces and avoids
        // false-positive blocks from floors the player is standing on.
        // When dead/spectating, the local entity may be gone — fall back
        // to the Camera position so distance/wall checks still work.
        let local_pos = local_entity
            .and_then(|e| {
                if e.is_dead() { return None; }
                e.parts.get(&BodyPart::Head)
                    .or_else(|| e.parts.get(&BodyPart::HumanoidRootPart))
                    .map(|p| p.position)
            })
            .or_else(|| visengine.get_camera_position())
            .unwrap_or(Vector3::ZERO);
        
        let local_team = cache.get_local_team_addr();
        
        // Get game-specific team identifier (e.g., Phantom Forces)
        let local_team_identifier = cache.get_local_team_id();
        let game_id = cache.get_game_id();
        
        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> = if team_check && !teammate_whitelist.is_empty() {
            snapshot.iter()
                .filter(|e| teammate_whitelist.iter().any(|name| name.eq_ignore_ascii_case(&e.name)))
                .map(|e| e.model_address)
                .collect()
        } else {
            AHashSet::new()
        };

        // Parallel computation for larger player counts
        let use_parallel = snapshot.len() > 4;

        let compute_entity = |entity: &crate::utils::cache::Entity| -> Option<ChamsEntityData> {
            // Skip local player
            if entity.name.eq_ignore_ascii_case(local_player_name) {
                return None;
            }

            // Hide dead players
            if hide_dead && entity.is_dead() {
                return None;
            }

            // Team check — game-aware dispatch (Rivals=TeammateLabel, PF=tag colour, default=Teams)
            let is_team = is_teammate(
                entity, team_check, local_team, &teammate_addresses, &local_team_identifier, game_id,
            );

            // Get root part for distance check
            let root_part = entity.root_part()?;

            let root_pos = root_part.position;
            
            if !root_pos.is_valid() || root_pos.is_near_origin(1.0) {
                return None;
            }

            let distance = root_pos.distance_to(local_pos);
            if distance > max_distance {
                return None;
            }

            let is_aim_target = target_highlight
                && aim_target_name
                    .map(|name| entity.name.eq_ignore_ascii_case(name))
                    .unwrap_or(false);

            // Determine colors
            let (fill_color, glow_color, outline_color) = if is_team {
                // Blue for teammates
                (
                    egui::Color32::from_rgba_unmultiplied(59, 130, 246, 100),
                    egui::Color32::from_rgba_unmultiplied(96, 165, 250, 60),
                    egui::Color32::from_rgba_unmultiplied(147, 197, 253, 220),
                )
            } else if is_aim_target {
                // Purple for aim target
                (
                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 120),
                    egui::Color32::from_rgba_unmultiplied(192, 132, 252, 80),
                    egui::Color32::from_rgba_unmultiplied(216, 180, 254, 240),
                )
            } else {
                Self::get_distance_colors(distance)
            };

            // Occluded colors (dimmed red tint for parts behind walls)
            let (occluded_fill, occluded_glow, occluded_outline) = if is_team {
                (
                    egui::Color32::from_rgba_unmultiplied(30, 65, 123, 60),
                    egui::Color32::from_rgba_unmultiplied(48, 82, 125, 30),
                    egui::Color32::from_rgba_unmultiplied(73, 98, 126, 130),
                )
            } else {
                // Red-tinted dimmed version for enemies behind walls
                (
                    egui::Color32::from_rgba_unmultiplied(180, 50, 50, 70),
                    egui::Color32::from_rgba_unmultiplied(200, 60, 60, 35),
                    egui::Color32::from_rgba_unmultiplied(220, 80, 80, 150),
                )
            };

            let velocity = entity.velocity;

            // Get map parser for per-part wall check
            let map_parser = if wall_check { Some(get_map_parser()) } else { None };

            // Compute convex hull for each cached body part (skip invisible HumanoidRootPart).
            // Iterating all parts instead of a fixed rig list ensures custom rigs
            // (e.g., Operation One's collision/hip/legs) render correctly.
            let part_hulls: Vec<PartHull> = {
                entity.parts.iter()
                .filter(|(bp, _)| **bp != BodyPart::HumanoidRootPart)
                .filter_map(|(_, part)| {
                    // Skip tiny parts at distance — projected size < ~5px
                    let max_dim = part.size.x.max(part.size.y).max(part.size.z);
                    if distance > 50.0 && max_dim / distance < 0.005 {
                        return None;
                    }

                    let mut hull = Self::compute_part_hull(
                        part,
                        velocity,
                        visengine,
                        dimensions,
                        &view_matrix,
                        &window_offset,
                    )?;

                    // Per-part wall check: check if this specific body part is behind a wall
                    if let Some(ref mp) = map_parser {
                        let part_pos = part.position;
                        if !mp.is_visible(local_pos, part_pos) {
                            hull.is_occluded = true;
                        }
                    }

                    Some(hull)
                })
                .collect()
            };

            if part_hulls.is_empty() {
                return None;
            }

            Some(ChamsEntityData {
                part_hulls,
                distance,
                fill_color,
                glow_color,
                outline_color,
                occluded_fill_color: occluded_fill,
                occluded_glow_color: occluded_glow,
                occluded_outline_color: occluded_outline,
            })
        };

        let chams_data: Vec<ChamsEntityData> = if use_parallel {
            snapshot.par_iter().filter_map(compute_entity).collect()
        } else {
            snapshot.iter().filter_map(compute_entity).collect()
        };

        // Render all chams
        egui::Area::new(egui::Id::new("chams_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();

                for entity_data in &chams_data {
                    // Distance-based glow LOD: skip glow layers for far entities
                    let glow_layers = if entity_data.distance > 120.0 { 0u8 }
                                      else if entity_data.distance > 80.0 { 1 }
                                      else { 2 };

                    for hull in &entity_data.part_hulls {
                        if hull.vertices.len() < 3 {
                            continue;
                        }

                        // Select colors based on whether this part is behind a wall
                        let (fill_c, glow_c, outline_c) = if hull.is_occluded {
                            (entity_data.occluded_fill_color, entity_data.occluded_glow_color, entity_data.occluded_outline_color)
                        } else {
                            (entity_data.fill_color, entity_data.glow_color, entity_data.outline_color)
                        };

                        // Convert to egui points
                        let points: Vec<egui::Pos2> = hull.vertices
                            .iter()
                            .map(|v| egui::pos2(v.x, v.y))
                            .collect();

                        // Glow layers (skip for distant entities to reduce shape count)
                        if glow_layers >= 2 {
                            Self::draw_expanded_polygon(painter, &points, 4.0, 
                                egui::Color32::from_rgba_unmultiplied(
                                    glow_c.r(), glow_c.g(), glow_c.b(), 20,
                                )
                            );
                        }
                        if glow_layers >= 1 {
                            Self::draw_expanded_polygon(painter, &points, 2.5, 
                                egui::Color32::from_rgba_unmultiplied(
                                    glow_c.r(), glow_c.g(), glow_c.b(), 40,
                                )
                            );
                        }

                        // Fill + outline combined into single shape (avoids clone)
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            fill_c,
                            egui::Stroke::new(1.2, outline_c),
                        ));
                    }
                }
            });
    }

    /// Draw an expanded version of a polygon for glow effect
    fn draw_expanded_polygon(painter: &egui::Painter, points: &[egui::Pos2], expand: f32, color: egui::Color32) {
        if points.len() < 3 {
            return;
        }

        // Calculate centroid
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        for p in points {
            cx += p.x;
            cy += p.y;
        }
        cx /= points.len() as f32;
        cy /= points.len() as f32;

        // Expand each point away from centroid
        let expanded: Vec<egui::Pos2> = points
            .iter()
            .map(|p| {
                let dx = p.x - cx;
                let dy = p.y - cy;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                egui::pos2(
                    p.x + dx / len * expand,
                    p.y + dy / len * expand,
                )
            })
            .collect();

        painter.add(egui::Shape::convex_polygon(
            expanded,
            color,
            egui::Stroke::NONE,
        ));
    }

    /// Get colors based on distance (thresholds match ESP: 30/80/150)
    fn get_distance_colors(distance: f32) -> (egui::Color32, egui::Color32, egui::Color32) {
        if distance < 30.0 {
            // Green - close
            (
                egui::Color32::from_rgba_unmultiplied(34, 197, 94, 110),
                egui::Color32::from_rgba_unmultiplied(74, 222, 128, 60),
                egui::Color32::from_rgba_unmultiplied(134, 239, 172, 230),
            )
        } else if distance < 80.0 {
            // Teal - medium
            (
                egui::Color32::from_rgba_unmultiplied(20, 184, 166, 100),
                egui::Color32::from_rgba_unmultiplied(45, 212, 191, 50),
                egui::Color32::from_rgba_unmultiplied(94, 234, 212, 210),
            )
        } else if distance < 150.0 {
            // Yellow - far
            (
                egui::Color32::from_rgba_unmultiplied(251, 191, 36, 90),
                egui::Color32::from_rgba_unmultiplied(252, 211, 77, 45),
                egui::Color32::from_rgba_unmultiplied(253, 224, 113, 190),
            )
        } else {
            // Red - very far
            (
                egui::Color32::from_rgba_unmultiplied(239, 68, 68, 80),
                egui::Color32::from_rgba_unmultiplied(248, 113, 113, 40),
                egui::Color32::from_rgba_unmultiplied(252, 165, 165, 170),
            )
        }
    }

    /// Get mesh chams outline + subtle fill colors based on distance.
    /// Returns (outline_color, faint_fill_color).
    fn get_mesh_distance_colors(distance: f32) -> (egui::Color32, egui::Color32) {
        if distance < 30.0 {
            // Bright green
            (
                egui::Color32::from_rgba_unmultiplied(34, 255, 94, 240),
                egui::Color32::from_rgba_unmultiplied(34, 197, 94, 18),
            )
        } else if distance < 80.0 {
            // Cyan/teal
            (
                egui::Color32::from_rgba_unmultiplied(20, 230, 200, 230),
                egui::Color32::from_rgba_unmultiplied(20, 184, 166, 15),
            )
        } else if distance < 150.0 {
            // Yellow/amber
            (
                egui::Color32::from_rgba_unmultiplied(255, 210, 50, 220),
                egui::Color32::from_rgba_unmultiplied(251, 191, 36, 12),
            )
        } else {
            // Red
            (
                egui::Color32::from_rgba_unmultiplied(255, 80, 80, 200),
                egui::Color32::from_rgba_unmultiplied(239, 68, 68, 10),
            )
        }
    }

    /// Render mesh chams — lightweight outline-only mode for all visible players.
    ///
    /// Draws a crisp bright outline tracing each body part's silhouette,
    /// with an optional faint colour fill controlled by `mesh_chams_fill`.
    /// No glow layers — this is a clean wireframe look distinct from filled chams.
    /// Fully compatible with wall_check — occluded parts get dimmed colors.
    pub fn render_mesh(
        ctx: &egui::Context,
        cache: &Arc<Cache>,
        visengine: &Arc<VisualEngine>,
        config: &Config,
        aim_target_name: Option<&str>,
        local_player_name: &str,
    ) {
        if !config.visuals.mesh_chams {
            return;
        }
        let snapshot = cache.get_snapshot();
        if snapshot.is_empty() {
            return;
        }

        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let max_distance = config.visuals.max_distance;
        let target_highlight = config.visuals.target_highlight;
        let team_check = config.visuals.team_check;
        let hide_dead = config.visuals.hide_dead;
        let wall_check = config.visuals.wall_check;

        let local_entity = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name));

        // When dead/spectating, fall back to Camera position
        let local_pos = local_entity
            .and_then(|e| {
                if e.is_dead() { return None; }
                e.parts.get(&BodyPart::Head)
                    .or_else(|| e.parts.get(&BodyPart::HumanoidRootPart))
                    .map(|p| p.position)
            })
            .or_else(|| visengine.get_camera_position())
            .unwrap_or(Vector3::ZERO);

        let local_team = cache.get_local_team_addr();
        let local_team_identifier = cache.get_local_team_id();
        let game_id = cache.get_game_id();

        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> = if team_check && !teammate_whitelist.is_empty() {
            snapshot.iter()
                .filter(|e| teammate_whitelist.iter().any(|name| name.eq_ignore_ascii_case(&e.name)))
                .map(|e| e.model_address)
                .collect()
        } else {
            AHashSet::new()
        };

        let use_parallel = snapshot.len() > 4;
        let show_fill = config.visuals.mesh_chams_fill;

        /// Per-entity mesh chams data
        struct MeshEntityData {
            part_hulls: Vec<PartHull>,
            outline_color: egui::Color32,
            fill_color: egui::Color32,
            occluded_outline: egui::Color32,
            occluded_fill: egui::Color32,
        }

        let compute_entity = |entity: &crate::utils::cache::Entity| -> Option<MeshEntityData> {
            if entity.name.eq_ignore_ascii_case(local_player_name) {
                return None;
            }
            if hide_dead && entity.is_dead() {
                return None;
            }

            let is_team = is_teammate(
                entity, team_check, local_team, &teammate_addresses, &local_team_identifier, game_id,
            );

            let root_part = entity.root_part()?;
            let root_pos = root_part.position;
            if !root_pos.is_valid() || root_pos.is_near_origin(1.0) {
                return None;
            }

            let distance = root_pos.distance_to(local_pos);
            if distance > max_distance {
                return None;
            }

            let is_aim_target = target_highlight
                && aim_target_name
                    .map(|name| entity.name.eq_ignore_ascii_case(name))
                    .unwrap_or(false);

            // Pick colors (outline + optional fill only — no glow in mesh mode)
            let (outline_c, fill_c) = if is_team {
                // Blue for teammates
                (
                    egui::Color32::from_rgba_unmultiplied(100, 160, 255, 220),
                    egui::Color32::from_rgba_unmultiplied(59, 130, 246, 15),
                )
            } else if is_aim_target {
                // Purple for aim target
                (
                    egui::Color32::from_rgba_unmultiplied(200, 120, 255, 245),
                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 22),
                )
            } else {
                Chams::get_mesh_distance_colors(distance)
            };

            // Occluded (behind wall) — dimmed version
            let (occ_outline, occ_fill) = if is_team {
                (
                    egui::Color32::from_rgba_unmultiplied(60, 90, 150, 120),
                    egui::Color32::from_rgba_unmultiplied(40, 70, 130, 8),
                )
            } else {
                // Dim red for occluded enemies
                (
                    egui::Color32::from_rgba_unmultiplied(200, 60, 60, 130),
                    egui::Color32::from_rgba_unmultiplied(160, 40, 40, 6),
                )
            };

            let velocity = entity.velocity;
            let map_parser = if wall_check { Some(get_map_parser()) } else { None };

            let part_hulls: Vec<PartHull> = {
                entity.parts.iter()
                .filter(|(bp, _)| **bp != BodyPart::HumanoidRootPart)
                .filter_map(|(_, part)| {
                    // Skip tiny parts at distance — projected size < ~5px
                    let max_dim = part.size.x.max(part.size.y).max(part.size.z);
                    if distance > 50.0 && max_dim / distance < 0.005 {
                        return None;
                    }

                    let mut hull = Chams::compute_part_hull(
                        part,
                        velocity,
                        visengine,
                        dimensions,
                        &view_matrix,
                        &window_offset,
                    )?;

                    if let Some(ref mp) = map_parser {
                        if !mp.is_visible(local_pos, part.position) {
                            hull.is_occluded = true;
                        }
                    }

                    Some(hull)
                })
                .collect()
            };

            if part_hulls.is_empty() {
                return None;
            }

            Some(MeshEntityData {
                part_hulls,
                outline_color: outline_c,
                fill_color: fill_c,
                occluded_outline: occ_outline,
                occluded_fill: occ_fill,
            })
        };

        let mesh_data: Vec<MeshEntityData> = if use_parallel {
            snapshot.par_iter().filter_map(compute_entity).collect()
        } else {
            snapshot.iter().filter_map(compute_entity).collect()
        };

        // Draw — clean wireframe outlines (fill optional)
        egui::Area::new(egui::Id::new("mesh_chams_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();

                for entity_data in &mesh_data {
                    for hull in &entity_data.part_hulls {
                        if hull.vertices.len() < 3 {
                            continue;
                        }

                        let (outline_c, fill_c) = if hull.is_occluded {
                            (entity_data.occluded_outline, entity_data.occluded_fill)
                        } else {
                            (entity_data.outline_color, entity_data.fill_color)
                        };

                        let points: Vec<egui::Pos2> = hull.vertices
                            .iter()
                            .map(|v| egui::pos2(v.x, v.y))
                            .collect();

                        // Optional colour fill inside the outline
                        if show_fill {
                            painter.add(egui::Shape::convex_polygon(
                                points.clone(),
                                fill_c,
                                egui::Stroke::NONE,
                            ));
                        }

                        // Crisp outline only — clean wireframe look
                        painter.add(egui::Shape::closed_line(
                            points,
                            egui::Stroke::new(1.8, outline_c),
                        ));
                    }
                }
            });
    }
}
