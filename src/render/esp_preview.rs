use eframe::egui;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use crate::config::Config;
use super::theme;

/// State for avatar fetching and caching
#[derive(Default)]
pub struct AvatarState {
    /// Cached avatar image bytes by username
    cached_avatars: HashMap<String, AvatarData>,
    /// Currently loading username
    loading_username: Option<String>,
    /// Last error message
    last_error: Option<String>,
}

#[derive(Clone)]
pub struct AvatarData {
    /// The raw PNG bytes (Arc for zero-copy sharing)
    pub image_bytes: Arc<Vec<u8>>,
    /// When we fetched this
    #[allow(dead_code)]
    pub fetched_at: std::time::Instant,
}

lazy_static::lazy_static! {
    static ref AVATAR_STATE: Arc<Mutex<AvatarState>> = Arc::new(Mutex::new(AvatarState::default()));
    static ref AVATAR_FETCH_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
}

pub fn fetch_avatar_async(username: &str) {
    let username = username.to_string();
    
    {
        let state = AVATAR_STATE.lock().unwrap();
        if state.cached_avatars.contains_key(&username) {
            return;
        }
        if state.loading_username.as_ref() == Some(&username) {
            return;
        }
    }
    
    {
        let mut state = AVATAR_STATE.lock().unwrap();
        state.loading_username = Some(username.clone());
        state.last_error = None;
    }
    
    let username_clone = username.clone();
    AVATAR_FETCH_RUNTIME.spawn(async move {
        match fetch_avatar_internal(&username_clone).await {
            Ok(bytes) => {
                let mut state = AVATAR_STATE.lock().unwrap();
                state.cached_avatars.insert(username_clone.clone(), AvatarData {
                    image_bytes: Arc::new(bytes),
                    fetched_at: std::time::Instant::now(),
                });
                state.loading_username = None;
                tracing::info!("Avatar fetched for: {}", username_clone);
            }
            Err(e) => {
                let mut state = AVATAR_STATE.lock().unwrap();
                state.last_error = Some(e.to_string());
                state.loading_username = None;
                tracing::warn!("Failed to fetch avatar for {}: {}", username_clone, e);
            }
        }
    });
}

pub fn get_cached_avatar(username: &str) -> Option<Arc<Vec<u8>>> {
    let state = AVATAR_STATE.lock().unwrap();
    state.cached_avatars.get(username).map(|d| Arc::clone(&d.image_bytes))
}

pub fn is_loading(username: &str) -> bool {
    let state = AVATAR_STATE.lock().unwrap();
    state.loading_username.as_ref() == Some(&username.to_string())
}

pub fn get_last_error() -> Option<String> {
    let state = AVATAR_STATE.lock().unwrap();
    state.last_error.clone()
}

pub fn clear_avatar_cache(username: &str) {
    let mut state = AVATAR_STATE.lock().unwrap();
    state.cached_avatars.remove(username);
}

async fn fetch_avatar_internal(username: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    let user_id = get_user_id(&client, username).await?;
    
    let thumbnail_url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar?userIds={}&size=420x420&format=Png&isCircular=false",
        user_id
    );
    
    let resp = client.get(&thumbnail_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?;
    
    let json: serde_json::Value = resp.json().await?;
    
    let image_url = json["data"]
        .get(0)
        .and_then(|d| d["imageUrl"].as_str())
        .ok_or("No avatar image URL found")?;
    
    let image_resp = client.get(image_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?;
    
    let bytes = image_resp.bytes().await?.to_vec();
    
    Ok(bytes)
}

async fn get_user_id(client: &reqwest::Client, username: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://users.roblox.com/v1/usernames/users";
    
    let body = serde_json::json!({
        "usernames": [username],
        "excludeBannedUsers": false
    });
    
    let resp = client.post(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    
    let json: serde_json::Value = resp.json().await?;
    
    let user_id = json["data"]
        .get(0)
        .and_then(|d| d["id"].as_u64())
        .ok_or_else(|| format!("User '{}' not found", username))?;
    
    Ok(user_id)
}

pub fn render_esp_preview_window(
    ctx: &egui::Context,
    config: &mut Config,
    menu_pos: egui::Pos2,
    menu_width: f32,
) {
    if !config.visuals.show_esp_preview {
        return;
    }
    
    let accent = theme::accent_from_rgb(
        config.interface.accent_r,
        config.interface.accent_g,
        config.interface.accent_b,
    );
    
    let username = config.general.username.clone();
    
    if !username.is_empty() {
        fetch_avatar_async(&username);
    }
    
    let preview_pos = egui::pos2(menu_pos.x + menu_width + 10.0, menu_pos.y);
    let preview_width = 220.0;
    
    let mut should_close = false;
    let mut should_refresh = false;
    
    egui::Area::new(egui::Id::new("esp_preview_window"))
        .default_pos(preview_pos)
        .movable(true)
        .constrain(true)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(theme::BG_DARK)
                .rounding(6.0)
                .stroke(egui::Stroke::new(2.0, accent))
                .inner_margin(egui::Margin::same(2.0))
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(theme::BG_DARK)
                        .rounding(4.0)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                        .show(ui, |ui| {
                            ui.set_width(preview_width);

                            egui::Frame::none()
                                .fill(theme::BG_MEDIUM)
                                .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("ESP & CHAMS PREVIEW")
                                            .size(10.0)
                                            .color(theme::TEXT_HEADER)
                                            .strong());
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.add(
                                                egui::Button::new(egui::RichText::new("×").size(14.0).color(theme::ACCENT_DANGER))
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .stroke(egui::Stroke::NONE)
                                                    .min_size(egui::vec2(18.0, 18.0))
                                            ).clicked() {
                                                should_close = true;
                                            }
                                            
                                            if ui.add(
                                                egui::Button::new(egui::RichText::new("↻").size(12.0).color(theme::ACCENT_INFO))
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .stroke(egui::Stroke::NONE)
                                                    .min_size(egui::vec2(18.0, 18.0))
                                            ).on_hover_text("Refresh avatar").clicked() {
                                                should_refresh = true;
                                            }
                                        });
                                    });
                                });

                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(8, 8, 12))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    render_interactive_preview(ui, config, accent, &username);
                                });

                            egui::Frame::none()
                                .fill(theme::BG_MEDIUM)
                                .rounding(egui::Rounding { nw: 0.0, ne: 0.0, sw: 4.0, se: 4.0 })
                                .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                                .show(ui, |ui| {
                                    render_preview_controls(ui, config, accent);
                                });
                        });
                });
        });
    
    if should_close {
        config.visuals.show_esp_preview = false;
    }
    if should_refresh && !username.is_empty() {
        clear_avatar_cache(&username);
        fetch_avatar_async(&username);
    }
}

/// Interactive preview with drag-to-rotate
fn render_interactive_preview(
    ui: &mut egui::Ui,
    config: &mut Config,
    accent: egui::Color32,
    username: &str,
) {
    let avatar_size = 160.0;
    let content_height = 24.0 + avatar_size + 24.0;
    let content_width = ui.available_width();
    let content_size = egui::vec2(content_width, content_height);
    
    // Allocate interactive area for drag + hover
    let (response, painter) = ui.allocate_painter(content_size, egui::Sense::click_and_drag());
    let rect = response.rect;
    let center_x = rect.center().x;
    let avatar_top = rect.top() + 22.0;
    
    // Handle mouse drag for rotation
    if response.dragged() {
        let delta = response.drag_delta();
        config.visuals.esp_preview_rotation += delta.x * 1.5;
        // Wrap to -180..180
        while config.visuals.esp_preview_rotation > 180.0 {
            config.visuals.esp_preview_rotation -= 360.0;
        }
        while config.visuals.esp_preview_rotation < -180.0 {
            config.visuals.esp_preview_rotation += 360.0;
        }
    }
    
    // Double-click to reset rotation
    if response.double_clicked() {
        config.visuals.esp_preview_rotation = 0.0;
    }
    
    // Show hover hint
    if response.hovered() && !response.dragged() {
        response.on_hover_text("Drag to rotate · Double-click to reset");
    }
    
    let rotation = config.visuals.esp_preview_rotation;
    let is_flipped = config.visuals.esp_preview_flipped;
    let is_occluded = config.visuals.esp_preview_wall_occluded;
    
    // Calculate body proportions for ESP box (adjusted by rotation)
    let rotation_rad = rotation.to_radians();
    let cos_r = rotation_rad.cos().abs();
    let sin_r = rotation_rad.sin().abs();
    
    // Body width narrows as we rotate (3D perspective simulation)
    let front_width = avatar_size * 0.42;
    let side_width = avatar_size * 0.22;
    let body_width = front_width * cos_r + side_width * sin_r;
    let body_height = avatar_size * 0.78;
    let body_top_offset = avatar_size * 0.10;
    
    let box_left = center_x - body_width / 2.0;
    let box_right = center_x + body_width / 2.0;
    let box_top = avatar_top + body_top_offset;
    let box_bottom = box_top + body_height;
    
    // Determine ESP colors based on wall check/occluded state
    let (esp_color, chams_fill, chams_glow, chams_outline) = if is_occluded {
        // Occluded/behind wall colors
        let esp_c = egui::Color32::from_rgb(220, 80, 80);
        let fill = egui::Color32::from_rgba_unmultiplied(180, 50, 50, 70);
        let glow = egui::Color32::from_rgba_unmultiplied(200, 60, 60, 35);
        let outline = egui::Color32::from_rgba_unmultiplied(220, 80, 80, 150);
        (esp_c, fill, glow, outline)
    } else if config.visuals.distance_colors {
        let esp_c = egui::Color32::from_rgb(0, 255, 0);
        let fill = egui::Color32::from_rgba_unmultiplied(0, 255, 80, 90);
        let glow = egui::Color32::from_rgba_unmultiplied(0, 220, 60, 50);
        let outline = egui::Color32::from_rgba_unmultiplied(100, 255, 150, 200);
        (esp_c, fill, glow, outline)
    } else {
        let r = (config.visuals.box_color[0] * 255.0) as u8;
        let g = (config.visuals.box_color[1] * 255.0) as u8;
        let b = (config.visuals.box_color[2] * 255.0) as u8;
        let esp_c = egui::Color32::from_rgb(r, g, b);
        let fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 90);
        let glow = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 50);
        let outline = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 200);
        (esp_c, fill, glow, outline)
    };
    
    // Draw chams glow (behind avatar)
    if config.visuals.chams {
        draw_chams_effect(
            &painter,
            center_x,
            avatar_top + avatar_size / 2.0,
            body_width,
            body_height,
            chams_fill,
            chams_glow,
            chams_outline,
        );
    }
    
    // Wall check indicator - dashed line / wall visual
    if is_occluded && config.visuals.wall_check {
        draw_wall_indicator(&painter, rect, accent);
    }
    
    // Draw avatar image
    let is_loading_now = is_loading(username);
    let avatar_bytes = get_cached_avatar(username);
    let error = get_last_error();
    
    if username.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No username set\n\nSet in Misc tab",
            egui::FontId::proportional(11.0),
            theme::TEXT_MUTED,
        );
        return;
    }
    
    if is_loading_now {
        let time = ui.ctx().input(|i| i.time);
        let dots = ".".repeat(((time * 3.0) as usize % 4) + 1);
        painter.text(
            egui::pos2(center_x, avatar_top + avatar_size / 2.0),
            egui::Align2::CENTER_CENTER,
            format!("Loading{}", dots),
            egui::FontId::proportional(12.0),
            theme::ACCENT_INFO,
        );
        ui.ctx().request_repaint();
        return;
    }
    
    // Draw avatar with tint when occluded
    if let Some(bytes) = &avatar_bytes {
        let texture_key = format!("avatar_{}", username);
        
        // Avatar is always displayed at full size — rotation only affects the ESP box,
        // not the avatar image itself. This prevents the "shrinking" effect.
        let display_width = avatar_size;
        
        let image_rect = egui::Rect::from_center_size(
            egui::pos2(center_x, avatar_top + avatar_size / 2.0),
            egui::vec2(display_width, avatar_size),
        );
        
        // Tint overlay for occluded state
        let tint = if is_occluded {
            egui::Color32::from_rgba_unmultiplied(180, 80, 80, 200)
        } else {
            egui::Color32::WHITE
        };
        
        // Load or retrieve texture via egui's memory. This caches the decoded texture
        // by URI so it's only decoded once no matter how many frames we render.
        let texture_uri = format!("bytes://{}", texture_key);
        let texture_id = ui.ctx().try_load_texture(
            &texture_uri,
            egui::TextureOptions::LINEAR,
            egui::SizeHint::default(),
        );
        
        // If the texture isn't loaded yet, register the bytes and let egui decode it
        match texture_id {
            Ok(egui::load::TexturePoll::Ready { texture }) => {
                // UV rect: flip horizontally if flipped
                let uv = if is_flipped {
                    egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
                } else {
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
                };
                
                // Paint directly with the Painter — this properly supports UV flipping
                painter.image(texture.id, image_rect, uv, tint);
            }
            Ok(egui::load::TexturePoll::Pending { .. }) => {
                // Still loading, show spinner text
                painter.text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Loading...",
                    egui::FontId::proportional(12.0),
                    theme::ACCENT_INFO,
                );
                ui.ctx().request_repaint();
            }
            Err(_) => {
                // Texture not yet registered — insert bytes so egui can load it next frame
                ui.ctx().include_bytes(texture_uri.clone(), bytes.as_ref().clone());
                ui.ctx().request_repaint();
            }
        }
    } else if let Some(err) = &error {
        painter.text(
            egui::pos2(center_x, avatar_top + avatar_size / 2.0),
            egui::Align2::CENTER_CENTER,
            format!("Error:\n{}", if err.len() > 30 { &err[..30] } else { err }),
            egui::FontId::proportional(10.0),
            theme::ACCENT_DANGER,
        );
    } else {
        // Placeholder silhouette
        let placeholder_rect = egui::Rect::from_center_size(
            egui::pos2(center_x, avatar_top + avatar_size / 2.0),
            egui::vec2(avatar_size * 0.7, avatar_size),
        );
        painter.rect_filled(placeholder_rect, 8.0, egui::Color32::from_rgb(30, 30, 40));
        painter.text(
            placeholder_rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(48.0),
            theme::TEXT_MUTED,
        );
    }

    if config.visuals.box_esp {
        let esp_rect = egui::Rect::from_min_max(
            egui::pos2(box_left, box_top),
            egui::pos2(box_right, box_bottom),
        );
        
        if config.visuals.box_fill {
            let fill_alpha = (config.visuals.box_fill_opacity * 255.0) as u8;
            let fill_color = egui::Color32::from_rgba_unmultiplied(
                (config.visuals.box_fill_color[0] * 255.0) as u8,
                (config.visuals.box_fill_color[1] * 255.0) as u8,
                (config.visuals.box_fill_color[2] * 255.0) as u8,
                fill_alpha,
            );
            painter.rect_filled(esp_rect, 0.0, fill_color);
        }
        
        if config.visuals.box_style == 1 {
            draw_corner_box(&painter, box_left, box_top, box_right, box_bottom, esp_color);
        } else {
            painter.rect_stroke(esp_rect, 0.0, egui::Stroke::new(2.0, esp_color));
        }
    }
    
    // health bar
    if config.visuals.health_bars {
        let bar_width = 4.0;
        let bar_x = box_left - bar_width - 2.0;
        let bar_height = box_bottom - box_top;
        
        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        
        let health = config.visuals.esp_preview_health;
        let fill_height = bar_height * health;
        let fill_top = box_bottom - fill_height;
        
        let health_color = if health > 0.6 {
            egui::Color32::GREEN
        } else if health > 0.3 {
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
    
    // armor bar
    if config.visuals.armor_bars {
        let bar_width = 4.0;
        let armor_offset = if config.visuals.health_bars { -12.0 } else { -6.0 };
        let bar_x = box_left + armor_offset;
        let bar_height = box_bottom - box_top;
        
        let bg_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, box_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(bg_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        
        let armor = config.visuals.esp_preview_armor;
        let fill_height = bar_height * armor;
        let fill_top = box_bottom - fill_height;
        
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(bar_x, fill_top),
            egui::pos2(bar_x + bar_width, box_bottom),
        );
        painter.rect_filled(fill_rect, 0.0, egui::Color32::from_rgb(59, 130, 246));
        painter.rect_stroke(bg_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));
    }
    
    // name tag
    if config.visuals.name_tags {
        painter.text(
            egui::pos2(box_left, box_top - 16.0),
            egui::Align2::LEFT_TOP,
            username,
            egui::FontId::proportional(12.0),
            esp_color,
        );
        
        painter.text(
            egui::pos2(box_left, box_bottom + 3.0),
            egui::Align2::LEFT_TOP,
            "25m",
            egui::FontId::proportional(10.0),
            esp_color,
        );
    }
    
    // tracers
    if config.visuals.tracers {
        let tracer_start = egui::pos2(rect.center().x, rect.bottom() - 5.0);
        let tracer_end = egui::pos2(center_x, box_bottom);
        painter.line_segment([tracer_start, tracer_end], egui::Stroke::new(1.0, esp_color));
    }
    
    // rotation indicator
    if rotation.abs() > 1.0 {
        let indicator_y = rect.bottom() - 4.0;
        let indicator_width = (rotation / 180.0) * (content_width / 2.0 - 10.0);
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(center_x.min(center_x + indicator_width), indicator_y - 2.0),
            egui::pos2(center_x.max(center_x + indicator_width), indicator_y),
        );
        painter.rect_filled(bar_rect, 1.0, accent.linear_multiply(0.6));
    }
}

/// Render the control buttons below the preview
fn render_preview_controls(ui: &mut egui::Ui, config: &mut Config, _accent: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let flip_text = if config.visuals.esp_preview_flipped { "⟲ Unflip" } else { "⟳ Flip" };
        if ui.add(
            egui::Button::new(egui::RichText::new(flip_text).size(9.5).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_DARK)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .min_size(egui::vec2(52.0, 20.0))
        ).clicked() {
            config.visuals.esp_preview_flipped = !config.visuals.esp_preview_flipped;
        }

        if ui.add(
            egui::Button::new(egui::RichText::new("⊙ Reset").size(9.5).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_DARK)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .min_size(egui::vec2(52.0, 20.0))
        ).clicked() {
            config.visuals.esp_preview_rotation = 0.0;
        }

        if config.visuals.wall_check || config.visuals.chams {
            let wall_label = if config.visuals.esp_preview_wall_occluded { "🧱 Behind" } else { "👁 Visible" };
            let wall_color = if config.visuals.esp_preview_wall_occluded { 
                egui::Color32::from_rgb(220, 80, 80) 
            } else { 
                egui::Color32::from_rgb(80, 220, 80) 
            };
            if ui.add(
                egui::Button::new(egui::RichText::new(wall_label).size(9.5).color(wall_color))
                    .fill(theme::BG_DARK)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                    .min_size(egui::vec2(60.0, 20.0))
            ).on_hover_text("Toggle visible / behind wall preview").clicked() {
                config.visuals.esp_preview_wall_occluded = !config.visuals.esp_preview_wall_occluded;
            }
        }
    });
    
    ui.add_space(3.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new("HP").size(9.0).color(theme::TEXT_MUTED));
        ui.add(egui::Slider::new(&mut config.visuals.esp_preview_health, 0.0..=1.0)
            .show_value(false)
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
        
        ui.label(egui::RichText::new("AR").size(9.0).color(theme::TEXT_MUTED));
        ui.add(egui::Slider::new(&mut config.visuals.esp_preview_armor, 0.0..=1.0)
            .show_value(false)
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
    });

    if config.visuals.esp_preview_rotation.abs() > 0.5 {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("rotation: {:.0}°", config.visuals.esp_preview_rotation))
                .size(8.0)
                .color(theme::TEXT_MUTED));
        });
    }
}

fn draw_chams_effect(
    painter: &egui::Painter,
    center_x: f32,
    center_y: f32,
    body_width: f32,
    body_height: f32,
    fill_color: egui::Color32,
    glow_color: egui::Color32,
    outline_color: egui::Color32,
) {
    let glow_layers: [(f32, f32); 5] = [
        (22.0, 0.3),
        (16.0, 0.45),
        (11.0, 0.6),
        (7.0, 0.75),
        (3.0, 0.9),
    ];
    
    for (expand, alpha_mult) in glow_layers {
        let a = (glow_color.a() as f32 * alpha_mult) as u8;
        let c = egui::Color32::from_rgba_unmultiplied(glow_color.r(), glow_color.g(), glow_color.b(), a);
        
        let glow_rect = egui::Rect::from_center_size(
            egui::pos2(center_x, center_y),
            egui::vec2(body_width + expand * 2.0, body_height + expand * 2.0),
        );
        painter.rect_filled(glow_rect, 10.0 + expand / 2.0, c);
    }

    let fill_rect = egui::Rect::from_center_size(
        egui::pos2(center_x, center_y),
        egui::vec2(body_width * 0.85, body_height * 0.92),
    );
    painter.rect_filled(fill_rect, 6.0, fill_color);

    let outline_rect = egui::Rect::from_center_size(
        egui::pos2(center_x, center_y),
        egui::vec2(body_width * 0.9, body_height * 0.95),
    );
    painter.rect_stroke(outline_rect, 6.0, egui::Stroke::new(1.5, outline_color));
}

/// Draw wall indicator (brick pattern showing obstruction)
fn draw_wall_indicator(painter: &egui::Painter, rect: egui::Rect, _accent: egui::Color32) {
    let wall_color = egui::Color32::from_rgba_unmultiplied(120, 80, 60, 45);
    let line_color = egui::Color32::from_rgba_unmultiplied(90, 60, 40, 60);

    let wall_width = 30.0;
    let wall_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + 15.0),
        egui::pos2(rect.left() + wall_width, rect.bottom() - 15.0),
    );
    painter.rect_filled(wall_rect, 2.0, wall_color);
    
    // Brick line pattern
    let row_height = 12.0;
    let mut y = wall_rect.top();
    let mut row = 0;
    while y < wall_rect.bottom() {
        painter.line_segment(
            [egui::pos2(wall_rect.left(), y), egui::pos2(wall_rect.right(), y)],
            egui::Stroke::new(0.5, line_color),
        );

        let offset = if row % 2 == 0 { 0.0 } else { wall_width / 2.0 };
        let mut x = wall_rect.left() + offset;
        while x < wall_rect.right() {
            painter.line_segment(
                [egui::pos2(x, y), egui::pos2(x, (y + row_height).min(wall_rect.bottom()))],
                egui::Stroke::new(0.5, line_color),
            );
            x += wall_width;
        }
        
        y += row_height;
        row += 1;
    }
    
    painter.rect_stroke(wall_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 70, 50, 80)));

    painter.text(
        egui::pos2(wall_rect.center().x, wall_rect.top() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "WALL",
        egui::FontId::proportional(7.0),
        egui::Color32::from_rgba_unmultiplied(160, 120, 90, 150),
    );
}

/// Corner-only box style for ESP
fn draw_corner_box(
    painter: &egui::Painter,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    color: egui::Color32,
) {
    let width = right - left;
    let height = bottom - top;
    let corner_len = (width.min(height) * 0.20).max(4.0).min(20.0);
    let stroke = egui::Stroke::new(2.0, color);
    
    // Top-left
    painter.line_segment([egui::pos2(left, top), egui::pos2(left + corner_len, top)], stroke);
    painter.line_segment([egui::pos2(left, top), egui::pos2(left, top + corner_len)], stroke);
    // Top-right
    painter.line_segment([egui::pos2(right - corner_len, top), egui::pos2(right, top)], stroke);
    painter.line_segment([egui::pos2(right, top), egui::pos2(right, top + corner_len)], stroke);
    // Bottom-left
    painter.line_segment([egui::pos2(left, bottom - corner_len), egui::pos2(left, bottom)], stroke);
    painter.line_segment([egui::pos2(left, bottom), egui::pos2(left + corner_len, bottom)], stroke);
    // Bottom-right
    painter.line_segment([egui::pos2(right - corner_len, bottom), egui::pos2(right, bottom)], stroke);
    painter.line_segment([egui::pos2(right, bottom - corner_len), egui::pos2(right, bottom)], stroke);
}
