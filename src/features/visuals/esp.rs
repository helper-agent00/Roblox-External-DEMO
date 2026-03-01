use eframe::egui;
use std::sync::Arc;

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::cache::{Cache, BodyPart};
use crate::utils::math::{Vector2, Vector3};

pub struct Esp;

impl Esp {
    pub fn render_fov_circle(ctx: &egui::Context, config: &Config, visengine: &Arc<VisualEngine>) {
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();
        let center = egui::pos2(
            window_offset.x + dimensions.x / 2.0,
            window_offset.y + dimensions.y / 2.0,
        );

        // Aimbot FOV circle (indigo)
        if config.aimbot.enabled && config.aimbot.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_aimbot"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.aimbot.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(99, 102, 241, 100)),
                    );
                });
        }

        // Camera aim FOV circle (cyan/green)
        if config.camera_aim.enabled && config.camera_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_camera"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.camera_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(34, 211, 153, 100)),
                    );
                });
        }

        // Viewport aim FOV circle (orange)
        if config.viewport_aim.enabled && config.viewport_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_viewport"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.viewport_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(251, 146, 60, 120)),
                    );
                });
        }

        // Silent aim (MouseService) FOV circle (red/pink)
        if config.silent_aim.enabled && config.silent_aim.show_fov {
            egui::Area::new(egui::Id::new("fov_circle_silent"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().circle_stroke(
                        center,
                        config.silent_aim.fov,
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(244, 63, 94, 120)),
                    );
                });
        }

    }

    /// Render ESP using pre-computed cached data (high-performance path).
    /// 
    /// Metadata (health, teams, visibility, etc.) comes from the background ESP data thread.
    /// Screen positions are re-projected from world-space each frame using the CURRENT
    /// view matrix to eliminate lag when the camera moves.
    pub fn render_cached(
        ctx: &egui::Context,
        esp_cache: &Arc<super::esp_cache::EspRenderCache>,
        config: &Config,
        visengine: &Arc<VisualEngine>,
    ) {
        let render_data = esp_cache.get_render_data();
        if render_data.is_empty() {
            return;
        }

        // Use FRESH view matrix from visengine for frame-accurate screen projection.
        // This is the key fix: the background thread caches entity metadata, but screen
        // positions are always computed with the current camera to prevent ESP "lag".
        let view_matrix = visengine.get_view_matrix();
        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();

        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return;
        }

        let box_enabled = config.visuals.box_esp;
        let name_enabled = config.visuals.name_tags;
        let tracers_enabled = config.visuals.tracers;
        let distance_colors = config.visuals.distance_colors;
        let target_highlight = config.visuals.target_highlight;
        let team_check = config.visuals.team_check;
        let wall_check = config.visuals.wall_check;

        if !box_enabled && !name_enabled && !tracers_enabled {
            return;
        }

        // Single Area for all ESP rendering - minimal overhead
        egui::Area::new(egui::Id::new("esp_overlay_cached"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(egui::Rect::EVERYTHING);
                let painter = ui.painter();

                for data in render_data.iter() {
                    // ── Determine ESP color (independent of projection) ──
                    let esp_color = if data.is_teammate && team_check {
                        egui::Color32::from_rgb(59, 130, 246) // Blue for teammates
                    } else if data.is_aim_target && target_highlight {
                        egui::Color32::from_rgb(255, 0, 255) // Magenta for aim target
                    } else if wall_check && !data.is_visible {
                        egui::Color32::from_rgb(255, 80, 80) // Red-ish for hidden
                    } else if distance_colors {
                        Self::get_distance_color(data.distance)
                    } else {
                        egui::Color32::from_rgb(
                            (config.visuals.box_color[0] * 255.0) as u8,
                            (config.visuals.box_color[1] * 255.0) as u8,
                            (config.visuals.box_color[2] * 255.0) as u8,
                        )
                    };

                    // ── Tracers — drawn BEFORE box projection so they work
                    //    even when the entity is outside the tight box-ESP
                    //    frustum.  Uses a wide projection (no NDC clamp) so
                    //    tracers always point toward any entity that is in
                    //    front of the camera, regardless of viewing angle.
                    if tracers_enabled {
                        if let Some(target) = visengine.world_to_screen_wide(data.world_pos, dimensions, &view_matrix) {
                            let screen_bottom = egui::pos2(
                                window_offset.x + dimensions.x / 2.0,
                                window_offset.y + dimensions.y,
                            );
                            let target_pos = egui::pos2(
                                target.x + window_offset.x,
                                target.y + window_offset.y,
                            );
                            painter.line_segment([screen_bottom, target_pos], egui::Stroke::new(1.0, esp_color));
                        }
                    }

                    // ── Box / Name / Health / Armor — require both top and
                    //    bottom world points to project within the normal
                    //    frustum so bounding-box geometry is valid. ──
                    let bottom_screen = match visengine.world_to_screen(data.world_bottom, dimensions, &view_matrix) {
                        Some(v) => Vector2::new(v.x + window_offset.x, v.y + window_offset.y),
                        None => continue,
                    };
                    let top_screen = match visengine.world_to_screen(data.world_top, dimensions, &view_matrix) {
                        Some(v) => Vector2::new(v.x + window_offset.x, v.y + window_offset.y),
                        None => continue,
                    };

                    // Compute box bounds from fresh screen projections
                    let base_height = (bottom_screen.y - top_screen.y).abs();
                    let center_x = (top_screen.x + bottom_screen.x) / 2.0;
                    let base_top = top_screen.y.min(bottom_screen.y);
                    let base_bottom = top_screen.y.max(bottom_screen.y);

                    let box_width = base_height * 0.65;
                    let v_padding_top = base_height * 0.06;
                    let v_padding_bottom = base_height * 0.08;

                    let box_left = center_x - box_width / 2.0;
                    let box_right = center_x + box_width / 2.0;
                    let box_top = base_top - v_padding_top;
                    let box_bottom = base_bottom + v_padding_bottom;

                    // Skip if box is too small
                    if (box_bottom - box_top) < 5.0 {
                        continue;
                    }

                    // Box
                    if box_enabled {
                        if config.visuals.box_style == 2 {
                            // 3D box - re-project world corners to screen using current view matrix
                            let corners_3d = data.box_3d_corners_world.as_ref().and_then(|world_corners| {
                                let mut screen_pts = [Vector2::ZERO; 8];
                                for (i, wc) in world_corners.iter().enumerate() {
                                    match visengine.world_to_screen(*wc, dimensions, &view_matrix) {
                                        Some(sp) => screen_pts[i] = Vector2::new(sp.x + window_offset.x, sp.y + window_offset.y),
                                        None => return None,
                                    }
                                }
                                Some(screen_pts)
                            });
                            if let Some(corners) = corners_3d {
                                // Box fill for 3D mode (approximate with 2D bounding rect)
                                if config.visuals.box_fill {
                                    let fill_alpha = (config.visuals.box_fill_opacity * 255.0) as u8;
                                    let fill_color = egui::Color32::from_rgba_unmultiplied(
                                        (config.visuals.box_fill_color[0] * 255.0) as u8,
                                        (config.visuals.box_fill_color[1] * 255.0) as u8,
                                        (config.visuals.box_fill_color[2] * 255.0) as u8,
                                        fill_alpha,
                                    );
                                    let rect = egui::Rect::from_min_max(
                                        egui::pos2(box_left, box_top),
                                        egui::pos2(box_right, box_bottom),
                                    );
                                    painter.rect_filled(rect, 0.0, fill_color);
                                }
                                Self::draw_3d_box(painter, &corners, esp_color);
                            }
                        } else {
                            let rect = egui::Rect::from_min_max(
                                egui::pos2(box_left, box_top),
                                egui::pos2(box_right, box_bottom),
                            );

                            // Box fill
                            if config.visuals.box_fill {
                                let fill_alpha = (config.visuals.box_fill_opacity * 255.0) as u8;
                                let fill_color = egui::Color32::from_rgba_unmultiplied(
                                    (config.visuals.box_fill_color[0] * 255.0) as u8,
                                    (config.visuals.box_fill_color[1] * 255.0) as u8,
                                    (config.visuals.box_fill_color[2] * 255.0) as u8,
                                    fill_alpha,
                                );
                                painter.rect_filled(rect, 0.0, fill_color);
                            }

                            // Box outline
                            if config.visuals.box_style == 1 {
                                Self::draw_corner_box(painter, box_left, box_top, box_right, box_bottom, esp_color);
                            } else {
                                painter.rect_stroke(rect, 0.0, egui::Stroke::new(2.0, esp_color));
                            }
                        }
                    }

                    // Health bar
                    if config.visuals.health_bars {
                        Self::draw_health_bar(painter, box_left, box_top, box_bottom, data.health_percent);
                    }

                    // Armor bar
                    if config.visuals.armor_bars && data.has_armor {
                        let armor_offset = if config.visuals.health_bars { -12.0 } else { -6.0 };
                        Self::draw_armor_bar(painter, box_left + armor_offset, box_top, box_bottom, data.armor_percent);
                    }

                    // Name and distance
                    if name_enabled {
                        static NAME_FONT: std::sync::LazyLock<egui::FontId> = std::sync::LazyLock::new(|| egui::FontId::proportional(14.0));
                        static DIST_FONT: std::sync::LazyLock<egui::FontId> = std::sync::LazyLock::new(|| egui::FontId::proportional(12.0));

                        painter.text(
                            egui::pos2(box_left, box_top - 15.0),
                            egui::Align2::LEFT_TOP,
                            &data.name,
                            NAME_FONT.clone(),
                            esp_color,
                        );

                        painter.text(
                            egui::pos2(box_left, box_bottom + 2.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}m", data.distance as u32),
                            DIST_FONT.clone(),
                            esp_color,
                        );
                    }
                }
            });
    }

    fn get_distance_color(distance: f32) -> egui::Color32 {
        if distance < 30.0 {
            egui::Color32::from_rgb(0, 255, 0)
        } else if distance < 80.0 {
            egui::Color32::from_rgb(0, 255, 128)
        } else if distance < 150.0 {
            egui::Color32::from_rgb(255, 255, 0)
        } else {
            egui::Color32::from_rgb(255, 100, 100)
        }
    }

    /// Draw corner-only box style (4 L-shaped corners)
    /// More performant than full box - only 8 line segments instead of 4 rect sides
    #[inline]
    fn draw_corner_box(painter: &egui::Painter, left: f32, top: f32, right: f32, bottom: f32, color: egui::Color32) {
        let width = right - left;
        let height = bottom - top;
        // Corner length is 20% of the smaller dimension, clamped for small boxes
        let corner_len = (width.min(height) * 0.20).max(4.0).min(20.0);
        let stroke = egui::Stroke::new(2.0, color);
        
        // Top-left corner
        painter.line_segment([egui::pos2(left, top), egui::pos2(left + corner_len, top)], stroke);
        painter.line_segment([egui::pos2(left, top), egui::pos2(left, top + corner_len)], stroke);
        
        // Top-right corner
        painter.line_segment([egui::pos2(right - corner_len, top), egui::pos2(right, top)], stroke);
        painter.line_segment([egui::pos2(right, top), egui::pos2(right, top + corner_len)], stroke);
        
        // Bottom-left corner
        painter.line_segment([egui::pos2(left, bottom - corner_len), egui::pos2(left, bottom)], stroke);
        painter.line_segment([egui::pos2(left, bottom), egui::pos2(left + corner_len, bottom)], stroke);
        
        // Bottom-right corner
        painter.line_segment([egui::pos2(right - corner_len, bottom), egui::pos2(right, bottom)], stroke);
        painter.line_segment([egui::pos2(right, bottom - corner_len), egui::pos2(right, bottom)], stroke);
    }

    /// Draw a 3D wireframe box from 8 projected screen-space corners.
    /// Corner layout:
    ///   Top face: 0-1-2-3  (front-left, front-right, back-right, back-left)
    ///   Bottom face: 4-5-6-7 (same order)
    #[inline]
    fn draw_3d_box(painter: &egui::Painter, corners: &[Vector2; 8], color: egui::Color32) {
        let outline = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);
        let outline_stroke = egui::Stroke::new(3.0, outline);
        let stroke = egui::Stroke::new(1.5, color);

        // 12 edges of a cuboid
        const EDGES: [(usize, usize); 12] = [
            // Top face
            (0, 1), (1, 2), (2, 3), (3, 0),
            // Bottom face
            (4, 5), (5, 6), (6, 7), (7, 4),
            // Vertical pillars
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];

        // Outline pass
        for &(a, b) in &EDGES {
            painter.line_segment(
                [egui::pos2(corners[a].x, corners[a].y), egui::pos2(corners[b].x, corners[b].y)],
                outline_stroke,
            );
        }
        // Color pass
        for &(a, b) in &EDGES {
            painter.line_segment(
                [egui::pos2(corners[a].x, corners[a].y), egui::pos2(corners[b].x, corners[b].y)],
                stroke,
            );
        }
    }

    fn draw_health_bar(painter: &egui::Painter, box_left: f32, box_top: f32, box_bottom: f32, health_percent: f32) {
        let bar_width = 4.0;
        let bar_height = box_bottom - box_top;
        let bar_x = box_left - bar_width - 2.0;

        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));

        let fill_height = bar_height * health_percent;
        let fill_top = box_bottom - fill_height;

        let health_color = if health_percent > 0.6 {
            egui::Color32::GREEN
        } else if health_percent > 0.3 {
            egui::Color32::YELLOW
        } else {
            egui::Color32::RED
        };

        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, fill_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(fill_rect, 0.0, health_color);
        painter.rect_stroke(bg_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }

    fn draw_armor_bar(painter: &egui::Painter, bar_x: f32, box_top: f32, box_bottom: f32, armor_percent: f32) {
        let bar_width = 4.0;
        let bar_height = box_bottom - box_top;

        // Background
        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));

        let fill_height = bar_height * armor_percent;
        let fill_top = box_bottom - fill_height;

        // Armor is typically displayed as blue/cyan
        let armor_color = egui::Color32::from_rgb(59, 130, 246); // Blue color

        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, fill_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(fill_rect, 0.0, armor_color);
        painter.rect_stroke(bg_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }

    /// Render hitbox expansion visualization.
    pub fn render_hitbox_visual(
        ctx: &egui::Context,
        cache: &Arc<Cache>,
        visengine: &Arc<VisualEngine>,
        config: &crate::config::Config,
        local_player_name: &str,
    ) {
        // Only render if hitbox expander is enabled AND visual is enabled
        if !config.hitbox.enabled || !config.hitbox.show_visual {
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

        // Get color from config
        let hitbox_color = egui::Color32::from_rgba_unmultiplied(
            (config.hitbox.color[0] * 255.0) as u8,
            (config.hitbox.color[1] * 255.0) as u8,
            (config.hitbox.color[2] * 255.0) as u8,
            (config.hitbox.color[3] * 255.0) as u8,
        );
        let hitbox_fill = egui::Color32::from_rgba_unmultiplied(
            (config.hitbox.color[0] * 255.0) as u8,
            (config.hitbox.color[1] * 255.0) as u8,
            (config.hitbox.color[2] * 255.0) as u8,
            ((config.hitbox.color[3] * 0.3) * 255.0) as u8, // 30% of alpha for fill
        );

        let hitbox_size_x = config.hitbox.head_scale * 2.0;
        let hitbox_size_y = config.hitbox.torso_scale * 2.0;
        let _hitbox_size_z = config.hitbox.arms_scale * 2.0; // Reserved for future 3D visualization

        let local_pos = snapshot
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(local_player_name))
            .and_then(|e| e.parts.get(&BodyPart::HumanoidRootPart))
            .map(|p| p.position)
            .unwrap_or(Vector3::ZERO);

        // Collect hitbox data
        let hitbox_data: Vec<_> = snapshot
            .iter()
            .filter(|entity| {
                // Skip local player unless self_enabled
                if entity.name.eq_ignore_ascii_case(local_player_name) {
                    config.hitbox.self_enabled
                } else {
                    config.hitbox.enemy_enabled
                }
            })
            .filter_map(|entity| {
                let hrp = entity.parts.get(&BodyPart::HumanoidRootPart)?;
                let pos = hrp.position;

                if (pos.x == 0.0 && pos.y == 0.0 && pos.z == 0.0)
                    || pos.x.is_nan() || pos.y.is_nan() || pos.z.is_nan()
                {
                    return None;
                }

                let distance = pos.distance_to(local_pos);
                if distance > config.visuals.max_distance {
                    return None;
                }

                let screen_center = visengine.world_to_screen(pos, dimensions, &view_matrix)?;
                let screen_center = Vector2::new(
                    screen_center.x + window_offset.x,
                    screen_center.y + window_offset.y,
                );

                let base_scale = 1200.0 / distance.max(10.0);
                let visual_width = base_scale * hitbox_size_x * 0.8;
                let visual_height = base_scale * hitbox_size_y * 0.8;

                if visual_width < 5.0 || visual_height < 5.0 {
                    return None;
                }

                Some((screen_center, visual_width, visual_height))
            })
            .collect();

        // Render
        egui::Area::new(egui::Id::new("hitbox_visual_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();

                for (screen_center, width, height) in &hitbox_data {
                    let rect = egui::Rect::from_center_size(
                        egui::pos2(screen_center.x, screen_center.y),
                        egui::vec2(*width, *height),
                    );

                    painter.rect_filled(rect, 4.0, hitbox_fill);
                    painter.rect_stroke(rect, 4.0, egui::Stroke::new(2.0, hitbox_color));
                }
            });
    }
}
