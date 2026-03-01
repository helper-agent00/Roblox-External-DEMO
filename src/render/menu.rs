use eframe::egui;
use std::sync::Arc;

use crate::config::{Config, DesyncMode, DesyncStrength, ParryInput};
use crate::features::{AutoClicker, MovementHacks};
use crate::utils::game_support::blade_ball::BladeBallDebugState;
use super::dex_ui;
use crate::utils::cache::Cache;
use std::sync::Mutex;

use super::theme;
use super::widgets;

// Desync key options (VK code, display name)
// Grouped: easy-reach keyboard, mouse, modifiers
const DESYNC_KEY_OPTIONS: &[(u32, &str)] = &[
    // -- Easy reach while on WASD --
    (0x14, "CapsLock"),  // Left pinky, no finger gymnastics
    (0x09, "Tab"),       // Left pinky top
    (0xC0, "Tilde ~"),   // Top-left corner key
    (0x51, "Q"),         // Left ring finger
    (0x45, "E"),         // Left index
    (0x52, "R"),         // Left index
    (0x46, "F"),         // Left index
    (0x58, "X"),         // Classic
    (0x5A, "Z"),         // Left pinky bottom
    (0x43, "C"),         // Left middle bottom
    (0x56, "V"),         // Left index bottom
    (0x47, "G"),         // Left index stretch
    // -- Mouse buttons (thumb / wheel) --
    (0x04, "Mid Mouse"), // Wheel click
    (0x05, "Mouse4"),    // Thumb back
    (0x06, "Mouse5"),    // Thumb forward
    // -- Modifiers --
    (0x10, "Shift"),     // Left pinky hold
    (0x11, "Ctrl"),      // Left pinky curl
    (0x12, "Alt"),       // Left thumb
];

const DESYNC_MODE_OPTIONS: &[(DesyncMode, &str)] = &[
    (DesyncMode::Hold,   "Hold"),
    (DesyncMode::Toggle, "Toggle"),
    (DesyncMode::Timed,  "Timed"),
];

const DESYNC_STRENGTH_OPTIONS: &[(DesyncStrength, &str)] = &[
    (DesyncStrength::FullFreeze, "Full Freeze"),
    (DesyncStrength::Throttled,  "Throttled"),
];

fn vk_to_name(vk: u32) -> &'static str {
    for &(code, name) in DESYNC_KEY_OPTIONS {
        if code == vk {
            return name;
        }
    }
    "CapsLock" // Default
}

/// Menu tab selection.
#[derive(PartialEq, Clone, Copy)]
pub enum MenuTab {
    Visuals,
    Aimbot,
    Movement,
    World,
    AutoClicker,
    Hitbox,
    Skin,
    Hotkeys,
    Misc,
    Game,
    Performance,
    About,
    DexExplorer,
}

/// Render the main menu.
pub fn render_menu(
    ctx: &egui::Context,
    menu_pos: &mut egui::Pos2,
    menu_minimized: &mut bool,
    current_tab: &mut MenuTab,
    config: &mut Config,
    cache: &Arc<Cache>,
    autoclicker: &mut AutoClicker,
    _movement_hacks: &mut MovementHacks,
    discord_username: &str,
    bb_debug: &Arc<Mutex<BladeBallDebugState>>,
) {
    let menu_width = 420.0;

    let menu_response = egui::Area::new(egui::Id::new("nexus_menu"))
        .current_pos(*menu_pos)
        .movable(true)
        .constrain(true)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
            
            // Outer accent border
            egui::Frame::none()
                .fill(theme::BG_DARK)
                .rounding(6.0)
                .stroke(egui::Stroke::new(2.0, accent))
                .inner_margin(egui::Margin::same(2.0))
                .show(ui, |ui| {
                    // Inner main frame
                    egui::Frame::none()
                        .fill(theme::BG_DARK)
                        .rounding(4.0)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                        .show(ui, |ui| {
                    ui.set_width(menu_width);

                    render_header(ui, menu_minimized);

                    if !*menu_minimized {
                        render_tab_bar(ui, current_tab);

                        // Separator line
                        ui.add_space(1.0);
                        let sep_rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            sep_rect.left()..=sep_rect.right(),
                            sep_rect.top(),
                            egui::Stroke::new(1.0, theme::BORDER_DEFAULT),
                        );
                        ui.add_space(1.0);

                        egui::Frame::none()
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(480.0)
                                    .show(ui, |ui| match *current_tab {
                                        MenuTab::Visuals => render_visuals_tab(ui, config),
                                        MenuTab::Aimbot => render_aimbot_tab(ui, config),
                                        MenuTab::Movement => render_movement_tab(ui, config),
                                        MenuTab::World => render_world_tab(ui, config),
                                        MenuTab::AutoClicker => render_autoclicker_tab(ui, config, autoclicker),
                                        MenuTab::Hitbox => render_hitbox_tab(ui, config),
                                        MenuTab::Skin => render_skin_tab(ui, config),
                                        MenuTab::Hotkeys => render_hotkeys_tab(ui, config),
                                        MenuTab::Misc => render_misc_tab(ui, config, cache),
                                        MenuTab::Game => render_game_tab(ui, config, bb_debug),
                                        MenuTab::Performance => render_performance_tab(ui, config),
                                        MenuTab::About => render_about_tab(ui, config),
                                        MenuTab::DexExplorer => dex_ui::render_dex_tab(ui),
                                    });
                            });

                        render_footer(ui, cache, discord_username);
                    }
                        });
                });
        });

    if menu_response.response.dragged() {
        *menu_pos = ctx.input(|i| *menu_pos + i.pointer.delta());
    }

    // Premium feature popup (demo build)
    widgets::render_premium_popup(ctx);
}

use crate::config::BindableFeature;

/// Get the current state of a bindable feature
fn get_feature_state(config: &Config, feature: BindableFeature) -> bool {
    match feature {
        BindableFeature::None => false,
        BindableFeature::BoxEsp => config.visuals.box_esp,
        BindableFeature::NameTags => config.visuals.name_tags,
        BindableFeature::Tracers => config.visuals.tracers,
        BindableFeature::HealthBars => config.visuals.health_bars,
        BindableFeature::ArmourBars => config.visuals.armor_bars,
        BindableFeature::Chams => config.visuals.chams,
        BindableFeature::TeamCheck => config.visuals.team_check,
        BindableFeature::HideDead => config.visuals.hide_dead,
        BindableFeature::ShowBots => config.visuals.show_bots,
        BindableFeature::FreeCamera => config.camera.free_camera_enabled,
        BindableFeature::AimAssist => config.aimbot.enabled,
        BindableFeature::Triggerbot => config.triggerbot.enabled,
        BindableFeature::CameraAim => config.camera_aim.enabled,
        BindableFeature::ViewportAim => config.viewport_aim.enabled,
        BindableFeature::SilentAim => config.silent_aim.enabled,
        BindableFeature::AutoReload => config.aimbot.auto_reload,
        BindableFeature::Fly => config.movement.fly_enabled,
        BindableFeature::Noclip => config.movement.noclip_enabled,
        BindableFeature::Spinbot => config.movement.spinbot_enabled,
        BindableFeature::AntiSit => config.movement.anti_sit_enabled,
        BindableFeature::VoidHide => config.movement.void_hide_enabled,
        BindableFeature::VehicleFly => config.movement.vehicle_fly_enabled,
        BindableFeature::Spiderman => config.movement.spiderman,
        BindableFeature::NoFallDamage => config.movement.no_fall_damage,
        BindableFeature::ClickTeleport => config.movement.click_teleport,
        BindableFeature::AutoJump => config.movement.auto_jump,
        BindableFeature::HipHeight => config.movement.hip_height_enabled,
        BindableFeature::HitboxMod => config.hitbox.enabled,
        BindableFeature::ShowHitboxVisual => config.hitbox.show_visual,
        BindableFeature::Fullbright => config.world.fullbright,
        BindableFeature::CameraFov => config.camera.fov_enabled,
        BindableFeature::Korblox => config.cosmetics.korblox,
        BindableFeature::Headless => config.cosmetics.headless,
        BindableFeature::AntiAfk => config.anti_afk.enabled,
        BindableFeature::AutoClicker => config.autoclicker.enabled,
        BindableFeature::BladeBall => config.blade_ball.enabled,
        BindableFeature::Desync => config.desync.enabled,
        BindableFeature::WallCheck => config.visuals.wall_check,
        BindableFeature::Footprints => config.visuals.footprints,
        BindableFeature::MovementTrails => config.visuals.movement_trails,
        BindableFeature::Anchor => config.movement.anchor_enabled,
        BindableFeature::Waypoint => config.movement.waypoint_enabled,
    }
}

/// Toggle a bindable feature
pub fn toggle_feature(config: &mut Config, feature: BindableFeature) {
    match feature {
        BindableFeature::None => {},
        BindableFeature::BoxEsp => config.visuals.box_esp = !config.visuals.box_esp,
        BindableFeature::NameTags => config.visuals.name_tags = !config.visuals.name_tags,
        BindableFeature::Tracers => config.visuals.tracers = !config.visuals.tracers,
        BindableFeature::HealthBars => config.visuals.health_bars = !config.visuals.health_bars,
        BindableFeature::ArmourBars => config.visuals.armor_bars = !config.visuals.armor_bars,
        BindableFeature::Chams => config.visuals.chams = !config.visuals.chams,
        BindableFeature::TeamCheck => config.visuals.team_check = !config.visuals.team_check,
        BindableFeature::HideDead => config.visuals.hide_dead = !config.visuals.hide_dead,
        BindableFeature::ShowBots => config.visuals.show_bots = !config.visuals.show_bots,
        BindableFeature::FreeCamera => config.camera.free_camera_enabled = !config.camera.free_camera_enabled,
        BindableFeature::AimAssist => config.aimbot.enabled = !config.aimbot.enabled,
        BindableFeature::Triggerbot => {}, // Demo: premium feature
        BindableFeature::CameraAim => config.camera_aim.enabled = !config.camera_aim.enabled,
        BindableFeature::ViewportAim => config.viewport_aim.enabled = !config.viewport_aim.enabled,
        BindableFeature::SilentAim => {}, // Demo: premium feature
        BindableFeature::AutoReload => config.aimbot.auto_reload = !config.aimbot.auto_reload,
        BindableFeature::Fly => {}, // Demo: premium feature
        BindableFeature::Noclip => config.movement.noclip_enabled = !config.movement.noclip_enabled,
        BindableFeature::Spinbot => {}, // Demo: premium feature
        BindableFeature::AntiSit => config.movement.anti_sit_enabled = !config.movement.anti_sit_enabled,
        BindableFeature::VoidHide => {}, // Demo: premium feature
        BindableFeature::VehicleFly => {}, // Demo: premium feature
        BindableFeature::Spiderman => {}, // Demo: premium feature
        BindableFeature::NoFallDamage => {}, // Demo: premium feature
        BindableFeature::ClickTeleport => {}, // Demo: premium feature
        BindableFeature::AutoJump => config.movement.auto_jump = !config.movement.auto_jump,
        BindableFeature::HipHeight => config.movement.hip_height_enabled = !config.movement.hip_height_enabled,
        BindableFeature::HitboxMod => {}, // Demo: premium feature
        BindableFeature::ShowHitboxVisual => {}, // Demo: premium feature
        BindableFeature::Fullbright => config.world.fullbright = !config.world.fullbright,
        BindableFeature::CameraFov => config.camera.fov_enabled = !config.camera.fov_enabled,
        BindableFeature::Korblox => {}, // Demo: premium feature
        BindableFeature::Headless => {}, // Demo: premium feature
        BindableFeature::AntiAfk => config.anti_afk.enabled = !config.anti_afk.enabled,
        BindableFeature::AutoClicker => config.autoclicker.enabled = !config.autoclicker.enabled,
        BindableFeature::BladeBall => config.blade_ball.enabled = !config.blade_ball.enabled,
        BindableFeature::Desync => {}, // Demo: premium feature
        BindableFeature::WallCheck => config.visuals.wall_check = !config.visuals.wall_check,
        BindableFeature::Footprints => config.visuals.footprints = !config.visuals.footprints,
        BindableFeature::MovementTrails => config.visuals.movement_trails = !config.visuals.movement_trails,
        BindableFeature::Anchor => {}, // Demo: premium feature
        BindableFeature::Waypoint => {}, // Demo: premium feature
    }
}

/// Render hotkey hints panel.
pub fn render_hotkey_hints(ctx: &egui::Context, hotkey_pos: &mut egui::Pos2, menu_open: bool, config: &mut Config) {
    if !config.interface.show_hotkey_hints || menu_open {
        return;
    }

    let hotkey_response = egui::Area::new(egui::Id::new("hotkey_hints"))
        .current_pos(*hotkey_pos)
        .movable(true)
        .constrain(true)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 20, 240))
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .rounding(3.0)
                .show(ui, |ui| {
                    ui.set_width(110.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("hotkeys").size(10.0).color(theme::TEXT_SECONDARY));
                    });

                    ui.add_space(4.0);

                    // Always show Menu hotkey first
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("F1").size(8.0).color(theme::TEXT_MUTED));
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Menu").size(9.0).color(theme::TEXT_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let size = egui::vec2(8.0, 8.0);
                            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                            ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
                            let inner = rect.shrink(2.0);
                            ui.painter().rect_filled(inner, 0.0, theme::ACCENT_PRIMARY);
                        });
                    });

                    ui.spacing_mut().item_spacing.y = 1.0;
                    
                    // Show configured hotkeys from bindings
                    for slot in &config.hotkey_bindings.slots {
                        if slot.feature == BindableFeature::None || slot.key == crate::config::HotkeyKey::None {
                            continue;
                        }
                        
                        let key_name = slot.key.display_name();
                        let feature_name = slot.feature.display_name();
                        let active = get_feature_state(config, slot.feature);
                        
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(key_name).size(8.0).color(theme::TEXT_MUTED));
                            ui.add_space(4.0);
                            // Truncate long feature names
                            let display_name = if feature_name.len() > 10 {
                                &feature_name[..10]
                            } else {
                                feature_name
                            };
                            ui.label(egui::RichText::new(display_name).size(9.0).color(theme::TEXT_SECONDARY));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let size = egui::vec2(8.0, 8.0);
                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.0, theme::BORDER_DEFAULT));
                                if active {
                                    let inner = rect.shrink(2.0);
                                    ui.painter().rect_filled(inner, 0.0, theme::ACCENT_PRIMARY);
                                }
                            });
                        });

                        // Show fly mode when fly is enabled
                        if slot.feature == BindableFeature::Fly && config.movement.fly_enabled {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                let mode_text = match config.movement.fly_mode {
                                    0 => "vel",
                                    _ => "stable",
                                };
                                ui.label(egui::RichText::new(format!("└ {}", mode_text))
                                    .size(8.0)
                                    .color(theme::TEXT_MUTED));
                            });
                        }
                    }
                });
        });

    if hotkey_response.response.dragged() {
        *hotkey_pos = ctx.input(|i| *hotkey_pos + i.pointer.delta());
        config.interface.hotkey_pos_x = hotkey_pos.x;
        config.interface.hotkey_pos_y = hotkey_pos.y;
    }
}

fn render_header(ui: &mut egui::Ui, _menu_minimized: &mut bool) {
    egui::Frame::none()
        .fill(theme::BG_MEDIUM)
        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
        .inner_margin(egui::Margin::symmetric(10.0, 7.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Drag handle indicator
                ui.label(egui::RichText::new("⠿").size(12.0).color(theme::TEXT_MUTED));
                
                ui.add_space(4.0);
                
                // Pulsing glow NEXUS title
                let time = ui.ctx().input(|i| i.time);
                let pulse = ((time * 2.5 + (time * 7.3).sin() * 0.5).sin() * 0.5 + 0.5) as f32;
                let glow_alpha = (pulse * 200.0 + 55.0) as u8;
                let glow_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, glow_alpha);
                
                // Draw glow behind text
                let text_response = ui.label(egui::RichText::new("NEXUS").size(13.0).color(glow_color).strong());
                let glow_rect = text_response.rect.expand(2.0 + pulse * 2.0);
                ui.painter().rect_filled(glow_rect, 4.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, (pulse * 25.0) as u8));
                
                ui.label(egui::RichText::new("Underground").size(11.0).color(theme::ACCENT_PRIMARY).strong());
                
                // Request repaint for animation at ~30fps, not unlimited
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(window_button("×", theme::ACCENT_DANGER)).on_hover_text("Exit (F12)").clicked() {
                        std::process::exit(0);
                    }
                    
                    // Version badge
                    ui.label(egui::RichText::new("v3.2").size(9.0).color(theme::TEXT_MUTED));
                });
            });
        });
}

fn window_button(icon: &str, color: egui::Color32) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(icon).size(14.0).color(color))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .rounding(2.0)
        .min_size(egui::vec2(20.0, 20.0))
}

fn render_tab_bar(ui: &mut egui::Ui, current_tab: &mut MenuTab) {
    egui::Frame::none()
        .fill(theme::BG_DARK)
        .inner_margin(egui::Margin::symmetric(4.0, 3.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                ui.spacing_mut().item_spacing.y = 1.0;

                let tabs: &[(&str, MenuTab)] = &[
                    ("Visuals", MenuTab::Visuals),
                    ("Aim", MenuTab::Aimbot),
                    ("Move", MenuTab::Movement),
                    ("World", MenuTab::World),
                    ("Clicker", MenuTab::AutoClicker),
                    ("Hitbox", MenuTab::Hitbox),
                    ("Skins", MenuTab::Skin),
                    ("Keys", MenuTab::Hotkeys),
                    ("Game", MenuTab::Game),
                    ("Misc", MenuTab::Misc),
                    ("Config", MenuTab::Performance),
                    ("About", MenuTab::About),
                    ("DEX", MenuTab::DexExplorer),
                ];

                for &(label, tab) in tabs {
                    let active = *current_tab == tab;

                    let btn = egui::Button::new(
                        egui::RichText::new(label)
                            .size(10.0)
                            .color(if active { theme::TEXT_PRIMARY } else { theme::TEXT_MUTED }),
                    )
                    .fill(if active { theme::BG_LIGHT } else { egui::Color32::TRANSPARENT })
                    .stroke(if active { 
                        egui::Stroke::new(1.0, theme::ACCENT_PRIMARY) 
                    } else { 
                        egui::Stroke::NONE 
                    })
                    .rounding(3.0)
                    .min_size(egui::vec2(34.0, 20.0));

                    if ui.add(btn).clicked() {
                        *current_tab = tab;
                    }
                }
            });
        });
}

fn render_footer(ui: &mut egui::Ui, cache: &Arc<Cache>, discord_username: &str) {
    let sep_rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        sep_rect.left()..=sep_rect.right(),
        sep_rect.top(),
        egui::Stroke::new(1.0, theme::BORDER_DEFAULT),
    );

    egui::Frame::none()
        .fill(theme::BG_MEDIUM)
        .rounding(egui::Rounding { nw: 0.0, ne: 0.0, sw: 4.0, se: 4.0 })
        .inner_margin(egui::Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Player count
                let count = cache.count();
                let dot_color = if count > 0 { theme::ACCENT_SUCCESS } else { theme::ACCENT_DANGER };
                ui.label(egui::RichText::new("●").size(8.0).color(dot_color));
                ui.label(
                    egui::RichText::new(format!("{} players", count))
                        .size(9.0)
                        .color(if count > 0 { theme::TEXT_SECONDARY } else { theme::TEXT_MUTED }),
                );
                
                // Colorized hotkey hints
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !discord_username.is_empty() {
                        ui.label(egui::RichText::new(discord_username).size(9.0).color(theme::ACCENT_INFO));
                        ui.label(egui::RichText::new("|").size(9.0).color(theme::BORDER_DEFAULT));
                    }
                    ui.label(egui::RichText::new("Exit").size(9.0).color(theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("F12").size(10.0).color(egui::Color32::from_rgb(255, 80, 80)).strong());
                    ui.label(egui::RichText::new("Save").size(9.0).color(theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("END").size(10.0).color(egui::Color32::from_rgb(100, 255, 100)).strong());
                    ui.label(egui::RichText::new("Refresh").size(9.0).color(theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("HOME").size(10.0).color(egui::Color32::from_rgb(255, 180, 50)).strong());
                    ui.label(egui::RichText::new("Menu").size(9.0).color(theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("F1").size(10.0).color(egui::Color32::from_rgb(100, 150, 255)).strong());
                });
            });
        });
}

// ============================================================================
// Tab Content
// ============================================================================

fn render_visuals_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    // ESP intensity — at the very top so users see it first
    widgets::double_border_frame(ui, "PERFORMANCE", accent, |ui| {
        widgets::styled_slider(ui, "esp intensity", &mut config.visuals.esp_intensity, 0.0..=1.0, "");
        config.visuals.esp_intensity = config.visuals.esp_intensity.clamp(0.0, 1.0);
        let label = if config.visuals.esp_intensity >= 0.85 {
            ("MAX", "Fastest updates · highest CPU", theme::ACCENT_DANGER)
        } else if config.visuals.esp_intensity >= 0.55 {
            ("HIGH", "Smooth updates · moderate CPU", theme::ACCENT_WARNING)
        } else if config.visuals.esp_intensity >= 0.25 {
            ("MED", "Balanced updates · low CPU", theme::ACCENT_SUCCESS)
        } else {
            ("LOW", "Minimal updates · lowest CPU", theme::ACCENT_INFO)
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label.0).size(9.0).strong().color(label.2));
            ui.label(egui::RichText::new(label.1).size(8.0).color(theme::TEXT_MUTED));
        });
    });

    ui.add_space(3.0);

    // ESP section with double border
    widgets::double_border_frame(ui, "esp", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.box_esp, "box esp", Some("F2"));
        if config.visuals.box_esp {
            // Box style selector (Full / Corners / 3D)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("box style:").size(10.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                if ui.selectable_label(config.visuals.box_style == 0, "full").clicked() {
                    config.visuals.box_style = 0;
                }
                if ui.selectable_label(config.visuals.box_style == 1, "corners").clicked() {
                    config.visuals.box_style = 1;
                }
                if ui.selectable_label(config.visuals.box_style == 2, "3D").clicked() {
                    config.visuals.box_style = 2;
                }
            });
            
            // Box fill toggle and opacity
            widgets::styled_toggle(ui, &mut config.visuals.box_fill, "box fill", None);
            if config.visuals.box_fill {
                widgets::styled_slider(ui, "fill opacity", &mut config.visuals.box_fill_opacity, 0.05..=0.5, "");
                // Fill color picker (always available - independent from outline)
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("fill color:").size(10.0).color(theme::TEXT_MUTED));
                    ui.add_space(4.0);
                    let mut fill_color = egui::Color32::from_rgb(
                        (config.visuals.box_fill_color[0] * 255.0) as u8,
                        (config.visuals.box_fill_color[1] * 255.0) as u8,
                        (config.visuals.box_fill_color[2] * 255.0) as u8,
                    );
                    if ui.color_edit_button_srgba(&mut fill_color).changed() {
                        config.visuals.box_fill_color[0] = fill_color.r() as f32 / 255.0;
                        config.visuals.box_fill_color[1] = fill_color.g() as f32 / 255.0;
                        config.visuals.box_fill_color[2] = fill_color.b() as f32 / 255.0;
                    }
                });
            }
            
            // Outline color picker (only when distance_colors is OFF)
            if !config.visuals.distance_colors {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("box color:").size(10.0).color(theme::TEXT_MUTED));
                    ui.add_space(4.0);
                    let mut color = egui::Color32::from_rgb(
                        (config.visuals.box_color[0] * 255.0) as u8,
                        (config.visuals.box_color[1] * 255.0) as u8,
                        (config.visuals.box_color[2] * 255.0) as u8,
                    );
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        config.visuals.box_color[0] = color.r() as f32 / 255.0;
                        config.visuals.box_color[1] = color.g() as f32 / 255.0;
                        config.visuals.box_color[2] = color.b() as f32 / 255.0;
                    }
                });
            }
        }
        widgets::styled_toggle(ui, &mut config.visuals.name_tags, "name tags", None);
        widgets::styled_toggle(ui, &mut config.visuals.tracers, "tracers", Some("F8"));
        widgets::styled_toggle(ui, &mut config.visuals.health_bars, "health bars", None);
        widgets::styled_toggle(ui, &mut config.visuals.armor_bars, "armor bars", None);
    });

    ui.add_space(3.0);
    
    // Effects section
    widgets::double_border_frame(ui, "effects", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.chams, "chams glow", Some("F3"));
        widgets::styled_toggle(ui, &mut config.visuals.mesh_chams, "mesh chams", None);
        if config.visuals.mesh_chams {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                widgets::styled_toggle(ui, &mut config.visuals.mesh_chams_fill, "colour fill", None);
            });
        }
        if (config.visuals.chams || config.visuals.mesh_chams) && config.visuals.wall_check {
            ui.label(egui::RichText::new("TIP: Wall check adds per-part visibility").size(8.0).color(theme::ACCENT_INFO));
        }
    });

    ui.add_space(3.0);

    // Crosshair section
    widgets::double_border_frame(ui, "CROSSHAIR", accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("style:").size(10.0).color(theme::TEXT_MUTED));
            ui.add_space(4.0);
            if ui.selectable_label(config.visuals.crosshair_style == 0, "off").clicked() {
                config.visuals.crosshair_style = 0;
            }
            if ui.selectable_label(config.visuals.crosshair_style == 1, "cross").clicked() {
                config.visuals.crosshair_style = 1;
            }
            if ui.selectable_label(config.visuals.crosshair_style == 2, "dot").clicked() {
                config.visuals.crosshair_style = 2;
            }
            if ui.selectable_label(config.visuals.crosshair_style == 3, "circle").clicked() {
                config.visuals.crosshair_style = 3;
            }
            if ui.selectable_label(config.visuals.crosshair_style == 4, "cross+dot").clicked() {
                config.visuals.crosshair_style = 4;
            }
        });
        if config.visuals.crosshair_style > 0 {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("color:").size(10.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                let mut color = egui::Color32::from_rgb(
                    (config.visuals.crosshair_color[0] * 255.0) as u8,
                    (config.visuals.crosshair_color[1] * 255.0) as u8,
                    (config.visuals.crosshair_color[2] * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    config.visuals.crosshair_color[0] = color.r() as f32 / 255.0;
                    config.visuals.crosshair_color[1] = color.g() as f32 / 255.0;
                    config.visuals.crosshair_color[2] = color.b() as f32 / 255.0;
                }
            });
            widgets::styled_slider(ui, "size", &mut config.visuals.crosshair_size, 2.0..=20.0, "px");
            widgets::styled_slider(ui, "thickness", &mut config.visuals.crosshair_thickness, 0.5..=5.0, "px");
            widgets::styled_slider(ui, "gap", &mut config.visuals.crosshair_gap, 0.0..=10.0, "px");
        }
    });

    ui.add_space(3.0);

    // Tracking section (footprints & trails)
    widgets::double_border_frame(ui, "TRACKING", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.footprints, "footprints", None);
        if config.visuals.footprints {
            ui.label(egui::RichText::new("  Shows where players have walked").size(8.0).color(theme::ACCENT_INFO));
        }
        widgets::styled_toggle(ui, &mut config.visuals.movement_trails, "movement trails", None);
        if config.visuals.movement_trails {
            ui.label(egui::RichText::new("  Fading line behind moving players").size(8.0).color(theme::ACCENT_INFO));
        }
    });

    ui.add_space(3.0);
    
    // Camera section  
    widgets::double_border_frame(ui, "camera", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.camera.fov_enabled, "fov changer", None);
        if config.camera.fov_enabled {
            widgets::styled_slider(ui, "fov", &mut config.camera.fov_value, 1.0..=120.0, "°");
            config.camera.fov_value = config.camera.fov_value.clamp(1.0, 120.0);
        }
        widgets::styled_toggle(ui, &mut config.camera.free_camera_enabled, "free camera", None);
        if config.camera.free_camera_enabled {
            widgets::styled_slider(ui, "speed", &mut config.camera.free_camera_speed, 10.0..=200.0, "");
            ui.label(egui::RichText::new("WASD = move | SPACE/CTRL = up/down").size(8.0).color(theme::ACCENT_INFO));
        }
    });

    ui.add_space(3.0);
    
    // Filters section
    widgets::double_border_frame(ui, "FILTERS", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.team_check, "team check", None);
        
        // Teammate whitelist input when team check is enabled
        if config.visuals.team_check {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("add teammate name:").size(9.0).color(theme::TEXT_MUTED));
            
            // Single line input with add button
            ui.horizontal(|ui| {
                // Get or create persistent input buffer
                let input_id = ui.id().with("team_input");
                let mut input_text = ui.ctx().data_mut(|d| {
                    d.get_persisted::<String>(input_id).unwrap_or_default()
                });
                
                let text_edit = egui::TextEdit::singleline(&mut input_text)
                    .desired_width(ui.available_width() - 50.0)
                    .font(egui::TextStyle::Small)
                    .hint_text("Player name...");
                
                let response = ui.add(text_edit);
                
                // Add button or Enter key
                let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let add_clicked = ui.add(
                    egui::Button::new(egui::RichText::new("+").size(12.0).color(theme::ACCENT_SUCCESS))
                        .fill(theme::BG_DARK)
                        .min_size(egui::vec2(30.0, 20.0))
                ).clicked();
                
                if (enter_pressed || add_clicked) && !input_text.trim().is_empty() {
                    let name = input_text.trim().to_string();
                    if !config.visuals.teammate_whitelist.contains(&name) {
                        config.visuals.teammate_whitelist.push(name);
                    }
                    input_text.clear();
                }
                
                // Store input buffer
                ui.ctx().data_mut(|d| d.insert_persisted(input_id, input_text));
            });
            
            // Show current teammates with remove buttons
            if !config.visuals.teammate_whitelist.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("teammates ({}):", config.visuals.teammate_whitelist.len())).size(8.0).color(theme::ACCENT_INFO));
                
                let mut to_remove: Option<usize> = None;
                for (idx, name) in config.visuals.teammate_whitelist.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("• {}", name)).size(9.0).color(theme::TEXT_PRIMARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(egui::RichText::new("×").size(10.0).color(theme::ACCENT_DANGER))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .min_size(egui::vec2(16.0, 16.0))
                            ).clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    });
                }
                if let Some(idx) = to_remove {
                    config.visuals.teammate_whitelist.remove(idx);
                }
            }
        }
        
        widgets::styled_toggle(ui, &mut config.visuals.hide_dead, "hide dead", None);
        widgets::styled_toggle(ui, &mut config.visuals.show_bots, "show bots/npcs/gadgets", None);
        widgets::styled_toggle(ui, &mut config.visuals.wall_check, "wall check", None);
        if config.visuals.wall_check {
            // Show part count to verify scanning is working
            let part_count = crate::utils::map_parser::get_map_parser().part_count();
            if part_count > 0 {
                ui.label(egui::RichText::new(format!("✓ {} parts cached", part_count)).size(8.0).color(theme::ACCENT_SUCCESS));
            } else {
                ui.label(egui::RichText::new("⏳ Scanning map...").size(8.0).color(theme::ACCENT_WARNING));
            }
        }
    });

    ui.add_space(3.0);
    
    // Display section
    widgets::double_border_frame(ui, "display", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.visuals.distance_colors, "distance colors", None);
        widgets::styled_toggle(ui, &mut config.visuals.target_highlight, "target highlight", None);
        widgets::styled_slider(ui, "max distance", &mut config.visuals.max_distance, 50.0..=5000.0, "m");
    });

    ui.add_space(3.0);
    
    // ESP & Chams Preview - Interactive preview with drag rotation, flip, wall check
    widgets::double_border_frame(ui, "ESP & CHAMS PREVIEW", accent, |ui| {
        ui.horizontal(|ui| {
            widgets::styled_toggle(ui, &mut config.visuals.show_esp_preview, "show preview window", None);
        });
        
        if config.visuals.show_esp_preview {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("drag to rotate · double-click to reset")
                .size(9.0)
                .color(theme::TEXT_MUTED));
            
            ui.add_space(4.0);
            
            // Flip + Wall check toggles
            ui.horizontal(|ui| {
                widgets::styled_toggle(ui, &mut config.visuals.esp_preview_flipped, "flip", None);
                if config.visuals.wall_check || config.visuals.chams {
                    widgets::styled_toggle(ui, &mut config.visuals.esp_preview_wall_occluded, "show occluded", None);
                }
            });
            
            ui.add_space(4.0);
            
            // Demo value sliders
            ui.label(egui::RichText::new("demo values:").size(9.0).color(theme::TEXT_MUTED));
            widgets::styled_slider(ui, "health", &mut config.visuals.esp_preview_health, 0.0..=1.0, "");
            widgets::styled_slider(ui, "armor", &mut config.visuals.esp_preview_armor, 0.0..=1.0, "");
        }
    });
}

fn render_aimbot_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    // Global Settings (shared across all aim features)
    widgets::double_border_frame(ui, "GLOBAL SETTINGS", accent, |ui| {
        ui.label(egui::RichText::new("(applies to all aim systems)").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        
        // Prediction
        widgets::styled_toggle(ui, &mut config.aimbot.prediction_enabled, "prediction", None);
        if config.aimbot.prediction_enabled {
            widgets::styled_slider(ui, "lead time", &mut config.aimbot.prediction_amount, 0.01..=0.2, "s");
        }
    });
    
    ui.add_space(6.0);

    // Section 1: Aim Assist (spring physics aimbot)
    if widgets::aim_section_header(
        ui,
        "aim assist",
        Some("humanized guidance for locking onto targets"),
        &mut config.aimbot.enabled,
        1,
        &mut config.interface.expanded_aim_section,
        Some("F4"),
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new("hold [RMB] to aim").size(9.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);
                
                widgets::styled_slider(ui, "fov radius", &mut config.aimbot.fov, 20.0..=500.0, "px");
                widgets::styled_slider(ui, "smoothing", &mut config.aimbot.smoothing, 1.0..=40.0, "");
                widgets::styled_toggle(ui, &mut config.aimbot.show_fov, "show fov", None);
                
                ui.add_space(4.0);
                widgets::bone_selector(ui, "target", &mut config.aimbot.target_bone, "bone_select");
                
                ui.add_space(4.0);
                widgets::activation_mode_selector(ui, "activation", &mut config.aimbot.activation_mode, "activation_mode_select");
                
                // Hold delay only relevant for Hold mode
                if config.aimbot.activation_mode == 0 {
                    ui.add_space(2.0);
                    let mut hold_delay_f = config.aimbot.hold_delay_ms as f32;
                    widgets::styled_slider(ui, "hold delay", &mut hold_delay_f, 0.0..=500.0, "ms");
                    config.aimbot.hold_delay_ms = hold_delay_f as u32;
                }
            });
    }

    // Section 2: Triggerbot (premium)
    let _tb_was_off = !config.triggerbot.enabled;
    if widgets::aim_section_header(
        ui,
        "triggerbot",
        Some("auto-fire when aim assist locks on"),
        &mut config.triggerbot.enabled,
        2,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                widgets::styled_slider(ui, "delay", &mut config.triggerbot.delay_ms, 0.0..=500.0, "ms");
                
                ui.add_space(4.0);
                ui.label(egui::RichText::new("fires when aim assist has a locked target").size(8.0).color(theme::TEXT_MUTED));
                ui.label(egui::RichText::new("requires aim assist to be enabled").size(8.0).color(theme::TEXT_MUTED));
            });
    }
    widgets::guard_premium_feature(ui.ctx(), &mut config.triggerbot.enabled, _tb_was_off);

    // Section 3: Camera Aim
    if widgets::aim_section_header(
        ui,
        "camera aim",
        Some("spoofs camera CFrame rotation"),
        &mut config.camera_aim.enabled,
        3,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                widgets::styled_slider(ui, "fov", &mut config.camera_aim.fov, 10.0..=400.0, "px");
                widgets::styled_toggle(ui, &mut config.camera_aim.show_fov, "show fov", None);
                ui.add_space(4.0);
                widgets::bone_selector(ui, "target", &mut config.camera_aim.target_bone, "cam_bone_select");
            });
    }

    // Section 4: Silent Aim
    if widgets::aim_section_header(
        ui,
        "silent aim",
        None,
        &mut config.viewport_aim.enabled,
        4,
        &mut config.interface.expanded_aim_section,
        None,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                // FOV settings
                widgets::styled_slider(ui, "fov", &mut config.viewport_aim.fov, 20.0..=500.0, "px");
                widgets::styled_toggle(ui, &mut config.viewport_aim.show_fov, "show fov", None);
                widgets::styled_toggle(ui, &mut config.viewport_aim.use_fov, "restrict to fov", None);
                
                ui.add_space(4.0);
                
                // Target bone
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("target bone").size(10.0).color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_source("viewport_target_bone")
                            .width(100.0)
                            .selected_text(match config.viewport_aim.target_bone {
                                0 => "Head",
                                1 => "UpperTorso",
                                2 => "LowerTorso",
                                _ => "HRP",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut config.viewport_aim.target_bone, 0, "Head");
                                ui.selectable_value(&mut config.viewport_aim.target_bone, 1, "UpperTorso");
                                ui.selectable_value(&mut config.viewport_aim.target_bone, 2, "LowerTorso");
                                ui.selectable_value(&mut config.viewport_aim.target_bone, 3, "HRP");
                            });
                    });
                });
                
                ui.add_space(4.0);
                ui.label(egui::RichText::new("uses global prediction & team check").size(8.0).color(theme::TEXT_MUTED));
            });
    }

    // Section 5: Mouse Silent Aim
    let _sa_was_off = !config.silent_aim.enabled;
    let _sa_expanded = widgets::aim_section_header(
        ui,
        "mouse silent aim",
        None,
        &mut config.silent_aim.enabled,
        5,
        &mut config.interface.expanded_aim_section,
        None,
    );
    widgets::guard_premium_feature(ui.ctx(), &mut config.silent_aim.enabled, _sa_was_off);
    if _sa_expanded {
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 12.0, right: 4.0, top: 4.0, bottom: 6.0 })
            .show(ui, |ui| {
                // FOV settings
                widgets::styled_slider(ui, "fov", &mut config.silent_aim.fov, 20.0..=500.0, "px");
                widgets::styled_toggle(ui, &mut config.silent_aim.show_fov, "show fov", None);
                widgets::styled_toggle(ui, &mut config.silent_aim.use_fov, "restrict to fov", None);

                ui.add_space(4.0);

                // Target bone
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("target bone").size(10.0).color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_source("silent_aim_target_bone")
                            .width(100.0)
                            .selected_text(match config.silent_aim.target_bone {
                                0 => "Head",
                                1 => "UpperTorso",
                                2 => "LowerTorso",
                                3 => "HRP",
                                4 => "LeftHand",
                                5 => "RightHand",
                                6 => "LeftFoot",
                                7 => "RightFoot",
                                _ => "Head",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut config.silent_aim.target_bone, 0, "Head");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 1, "UpperTorso");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 2, "LowerTorso");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 3, "HRP");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 4, "LeftHand");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 5, "RightHand");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 6, "LeftFoot");
                                ui.selectable_value(&mut config.silent_aim.target_bone, 7, "RightFoot");
                            });
                    });
                });

                ui.add_space(4.0);

                // Body part mode
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("body part mode").size(10.0).color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_source("silent_aim_body_mode")
                            .width(100.0)
                            .selected_text(match config.silent_aim.body_part_mode {
                                0 => "Fixed",
                                1 => "Closest Part",
                                2 => "Closest Point",
                                _ => "Random",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut config.silent_aim.body_part_mode, 0, "Fixed");
                                ui.selectable_value(&mut config.silent_aim.body_part_mode, 1, "Closest Part");
                                ui.selectable_value(&mut config.silent_aim.body_part_mode, 2, "Closest Point");
                                ui.selectable_value(&mut config.silent_aim.body_part_mode, 3, "Random");
                            });
                    });
                });

                ui.add_space(4.0);

                // Activation mode
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("activation").size(10.0).color(theme::TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_source("silent_aim_activation")
                            .width(100.0)
                            .selected_text(match config.silent_aim.activation_mode {
                                0 => "Hold RMB",
                                1 => "Toggle RMB",
                                _ => "Always On",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut config.silent_aim.activation_mode, 0, "Hold RMB");
                                ui.selectable_value(&mut config.silent_aim.activation_mode, 1, "Toggle RMB");
                                ui.selectable_value(&mut config.silent_aim.activation_mode, 2, "Always On");
                            });
                    });
                });

                ui.add_space(4.0);

                // Extra options
                widgets::styled_toggle(ui, &mut config.silent_aim.sticky_aim, "sticky aim (lock on target)", None);
                widgets::styled_toggle(ui, &mut config.silent_aim.team_check, "team check", None);
                widgets::styled_toggle(ui, &mut config.silent_aim.prediction_enabled, "velocity prediction", None);
                if config.silent_aim.prediction_enabled {
                    widgets::styled_slider(ui, "prediction", &mut config.silent_aim.prediction_amount, 10.0..=200.0, "ms");
                }

                widgets::styled_toggle(ui, &mut config.silent_aim.show_debug, "show debug overlay", None);

                ui.add_space(4.0);
                ui.label(egui::RichText::new("spoofs MouseService InputObject position").size(8.0).color(theme::TEXT_MUTED));
                ui.label(egui::RichText::new("for Da Hood, Hood Modded & Mouse.Hit games").size(8.0).color(theme::ACCENT_WARNING));
                ui.label(egui::RichText::new("NOT for PF/Rivals (use camera aim or viewport aim)").size(8.0).color(theme::ACCENT_WARNING));
            });
    }

    // Section 6: Auto Reload
    ui.add_space(4.0);
    widgets::section_header(ui, "auto reload");
    widgets::styled_toggle(ui, &mut config.aimbot.auto_reload, "auto reload", None);
}

fn render_movement_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    // Movement section
    widgets::double_border_frame(ui, "MOVEMENT", accent, |ui| {
        widgets::editable_slider(ui, "jump power", &mut config.movement.jump_power, 50.0..=300.0, "", "jump_power");
        widgets::styled_toggle(ui, &mut config.movement.auto_jump, "auto jump", None);
        widgets::premium_feature_toggle(ui, &mut config.movement.spiderman, "spiderman", None);
        if config.movement.spiderman {
            widgets::styled_slider(ui, "climb speed", &mut config.movement.spiderman_speed, 10.0..=100.0, "");
            ui.label(egui::RichText::new("TIP: Walk into walls to climb").size(8.0).color(theme::ACCENT_INFO));
        }
        widgets::editable_slider(ui, "walk speed", &mut config.movement.walk_speed, 16.0..=500.0, "", "walk_speed");
    });

    ui.add_space(3.0);
    
    // Fly section
    widgets::double_border_frame(ui, "FLY", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.movement.fly_enabled, "fly", Some("F6"));
        if config.movement.fly_enabled {
            widgets::editable_slider(ui, "speed", &mut config.movement.fly_speed, 10.0..=500.0, "", "fly_speed");
            widgets::fly_mode_selector(ui, "mode", &mut config.movement.fly_mode, "fly_mode");
            ui.add_space(4.0);
            ui.label(egui::RichText::new("writes per second:").size(9.0).color(theme::TEXT_MUTED));
            widgets::write_intensity_selector(ui, &mut config.movement.write_intensity);
            ui.add_space(4.0);
            ui.label(egui::RichText::new("TIP: Active once menu is closed").size(8.0).color(theme::ACCENT_INFO));
            ui.label(egui::RichText::new("SPACE = ascend | CTRL = descend").size(8.0).color(theme::ACCENT_INFO));
        }
        widgets::premium_feature_toggle(ui, &mut config.movement.vehicle_fly_enabled, "vehicle fly", None);
        if config.movement.vehicle_fly_enabled {
            widgets::styled_slider(ui, "vehicle speed", &mut config.movement.vehicle_fly_speed, 10.0..=500.0, "");
            ui.label(egui::RichText::new("TIP: Sit in a vehicle to fly it").size(8.0).color(theme::ACCENT_INFO));
        }
    });

    ui.add_space(3.0);
    
    // Other features section
    widgets::double_border_frame(ui, "OTHER", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.movement.noclip_enabled, "noclip", Some("F10"));
        widgets::premium_feature_toggle(ui, &mut config.movement.spinbot_enabled, "spinbot", None);
        widgets::styled_toggle(ui, &mut config.movement.anti_sit_enabled, "anti-sit", None);
        widgets::styled_toggle(ui, &mut config.movement.hip_height_enabled, "hip height", None);
        if config.movement.hip_height_enabled {
            widgets::editable_slider(ui, "height", &mut config.movement.hip_height_value, 0.0..=1300.0, "", "hip_height");
        }
        widgets::premium_feature_toggle(ui, &mut config.movement.no_fall_damage, "no fall damage", None);
    });

    ui.add_space(3.0);
    
    // Teleport section
    widgets::double_border_frame(ui, "TELEPORT", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.movement.void_hide_enabled, "void hide", None);
        widgets::premium_feature_toggle(ui, &mut config.movement.click_teleport, "click teleport", None);
        widgets::premium_feature_toggle(ui, &mut config.movement.waypoint_enabled, "waypoint", None);
        widgets::premium_feature_toggle(ui, &mut config.movement.anchor_enabled, "anchor", None);
    });
}

fn render_world_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    // Ban warning
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(200, 70, 70, 30))
        .stroke(egui::Stroke::new(1.0, theme::ACCENT_DANGER))
        .rounding(4.0)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{26A0}").size(13.0).color(theme::ACCENT_WARNING));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("BAN RISK").size(10.0).color(theme::ACCENT_DANGER).strong());
                    ui.label(egui::RichText::new("World hacks write to game memory and may trigger server-side detection.").size(9.0).color(theme::ACCENT_WARNING));
                    ui.label(egui::RichText::new("Games with anti-cheat (Fallen, Aftermath, etc.) can detect and ban for these.").size(9.0).color(theme::ACCENT_WARNING));
                });
            });
        });

    ui.add_space(3.0);

    // Fog section
    widgets::double_border_frame(ui, "FOG", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.anti_fog, "anti-fog", None);
        if config.world.anti_fog {
            widgets::styled_slider(ui, "start", &mut config.world.fog_start, 0.0..=100000.0, "");
            widgets::styled_slider(ui, "end", &mut config.world.fog_end, 0.0..=100000.0, "");
        }
    });

    ui.add_space(3.0);
    
    // Lighting section
    widgets::double_border_frame(ui, "LIGHTING", accent, |ui| {
        // Fullbright - maximum lighting
        widgets::styled_toggle(ui, &mut config.world.fullbright, "fullbright", None);
        if config.world.fullbright {
            ui.label(egui::RichText::new("  ⚡ Max brightness active").size(9.0).color(theme::ACCENT_WARNING));
        }
        
        // Manual brightness (disabled if fullbright is on)
        if !config.world.fullbright {
            widgets::styled_toggle(ui, &mut config.world.brightness_enabled, "brightness", None);
            if config.world.brightness_enabled {
                widgets::styled_slider(ui, "value", &mut config.world.brightness_value, 0.0..=10.0, "");
            }
        }
        
        widgets::styled_toggle(ui, &mut config.world.anti_flash, "anti-flash", None);
        if config.world.anti_flash {
            widgets::styled_slider(ui, "max", &mut config.world.max_brightness, 0.5..=5.0, "");
        }
        
        // No shadows
        widgets::styled_toggle(ui, &mut config.world.no_shadows, "no shadows", None);
    });
    
    ui.add_space(3.0);
    
    // Force Lighting section
    widgets::double_border_frame(ui, "FORCE LIGHTING", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.force_lighting, "enabled", None);
        if config.world.force_lighting && !config.world.fullbright {
            ui.add_space(2.0);
            
            // Ambient color picker
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ambient").size(10.0).color(theme::TEXT_PRIMARY));
                let mut color = egui::Color32::from_rgb(
                    (config.world.ambient_color[0] * 255.0) as u8,
                    (config.world.ambient_color[1] * 255.0) as u8,
                    (config.world.ambient_color[2] * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    config.world.ambient_color = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    ];
                }
            });
            
            // Outdoor ambient color picker
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("outdoor ambient").size(10.0).color(theme::TEXT_PRIMARY));
                let mut color = egui::Color32::from_rgb(
                    (config.world.outdoor_ambient_color[0] * 255.0) as u8,
                    (config.world.outdoor_ambient_color[1] * 255.0) as u8,
                    (config.world.outdoor_ambient_color[2] * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    config.world.outdoor_ambient_color = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    ];
                }
            });
            
            // Clock time slider
            widgets::styled_slider(ui, "time of day", &mut config.world.clock_time, 0.0..=24.0, "h");
        } else if config.world.fullbright {
            ui.label(egui::RichText::new("  (disabled while fullbright is on)").size(9.0).color(theme::TEXT_MUTED));
        }
    });
    
    ui.add_space(3.0);
    
    // Terrain Control section
    widgets::double_border_frame(ui, "TERRAIN", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.terrain_enabled, "enabled", None);
        if config.world.terrain_enabled {
            ui.add_space(2.0);
            widgets::styled_slider(ui, "grass length", &mut config.world.grass_length, 0.0..=1.0, "");
            widgets::styled_slider(ui, "water transparency", &mut config.world.water_transparency, 0.0..=1.0, "");
            
            // Water color picker
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("water color").size(10.0).color(theme::TEXT_PRIMARY));
                let mut color = egui::Color32::from_rgb(
                    (config.world.water_color[0] * 255.0) as u8,
                    (config.world.water_color[1] * 255.0) as u8,
                    (config.world.water_color[2] * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    config.world.water_color = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    ];
                }
            });
        }
    });

    ui.add_space(3.0);

    // Atmosphere section
    widgets::double_border_frame(ui, "ATMOSPHERE", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.atmosphere_enabled, "enabled", None);
        if config.world.atmosphere_enabled {
            ui.add_space(2.0);
            widgets::styled_slider(ui, "density", &mut config.world.atmosphere_density, 0.0..=1.0, "");
            widgets::styled_slider(ui, "haze", &mut config.world.atmosphere_haze, 0.0..=10.0, "");
            widgets::styled_slider(ui, "glare", &mut config.world.atmosphere_glare, 0.0..=10.0, "");
            widgets::styled_slider(ui, "offset", &mut config.world.atmosphere_offset, 0.0..=1.0, "");

            // Atmosphere color picker
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("color").size(10.0).color(theme::TEXT_PRIMARY));
                let mut color = egui::Color32::from_rgb(
                    (config.world.atmosphere_color[0] * 255.0) as u8,
                    (config.world.atmosphere_color[1] * 255.0) as u8,
                    (config.world.atmosphere_color[2] * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    config.world.atmosphere_color = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    ];
                }
            });
        }
    });

    ui.add_space(3.0);

    // Bloom Effect section
    widgets::double_border_frame(ui, "BLOOM EFFECT", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.bloom_enabled, "enabled", None);
        if config.world.bloom_enabled {
            ui.add_space(2.0);
            widgets::styled_toggle(ui, &mut config.world.bloom_active, "bloom active", None);
            widgets::styled_slider(ui, "intensity", &mut config.world.bloom_intensity, 0.0..=3.0, "");
            widgets::styled_slider(ui, "size", &mut config.world.bloom_size, 0.0..=100.0, "");
            widgets::styled_slider(ui, "threshold", &mut config.world.bloom_threshold, 0.0..=5.0, "");
        }
    });

    ui.add_space(3.0);

    // Depth of Field section
    widgets::double_border_frame(ui, "DEPTH OF FIELD", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.dof_enabled, "enabled", None);
        if config.world.dof_enabled {
            ui.add_space(2.0);
            widgets::styled_toggle(ui, &mut config.world.dof_active, "effect active", None);
            widgets::styled_slider(ui, "far intensity", &mut config.world.dof_far_intensity, 0.0..=1.0, "");
            widgets::styled_slider(ui, "focus distance", &mut config.world.dof_focus_distance, 0.0..=500.0, "");
            widgets::styled_slider(ui, "in-focus radius", &mut config.world.dof_in_focus_radius, 0.0..=500.0, "");
            widgets::styled_slider(ui, "near intensity", &mut config.world.dof_near_intensity, 0.0..=1.0, "");
        }
    });

    ui.add_space(3.0);

    // Sun Rays section
    widgets::double_border_frame(ui, "SUN RAYS", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.world.sunrays_enabled, "enabled", None);
        if config.world.sunrays_enabled {
            ui.add_space(2.0);
            widgets::styled_toggle(ui, &mut config.world.sunrays_active, "rays active", None);
            widgets::styled_slider(ui, "intensity", &mut config.world.sunrays_intensity, 0.0..=1.0, "");
            widgets::styled_slider(ui, "spread", &mut config.world.sunrays_spread, 0.0..=1.0, "");
        }
    });
}

fn render_autoclicker_tab(ui: &mut egui::Ui, config: &mut Config, autoclicker: &mut AutoClicker) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    let is_running = autoclicker.is_running();
    let state = autoclicker.state().lock().unwrap();
    let sequence_len = state.sequence.len();
    let is_recording = state.recording;
    let current_index = state.current_index;
    let total_clicks = state.total_clicks;
    let sequence_display: Vec<String> = state.sequence.iter().map(|b| b.display_name()).collect();
    drop(state);

    // How to use section
    widgets::double_border_frame(ui, "HOW TO USE", accent, |ui| {
        ui.label(egui::RichText::new("1. Click 'Record Buttons' to start").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("2. Press mouse buttons to record sequence").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("3. Click 'Stop Recording' when done").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("4. Press [Insert] to start/stop clicking").size(9.0).color(theme::ACCENT_PRIMARY));
    });

    ui.add_space(3.0);
    
    // Status indicator
    widgets::double_border_frame(ui, "STATUS", accent, |ui| {
        ui.horizontal(|ui| {
            let (status_text, status_color) = if is_running {
                ("● Running", theme::ACCENT_SUCCESS)
            } else if is_recording {
                ("● Recording", theme::ACCENT_WARNING)
            } else {
                ("○ Stopped", theme::TEXT_MUTED)
            };
            ui.label(egui::RichText::new(status_text).size(11.0).color(status_color).strong());
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("Clicks: {}", total_clicks)).size(9.0).color(theme::TEXT_SECONDARY));
            });
        });
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "TIMING", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.autoclicker.turbo_mode, "Turbo Mode", Some("Maximum speed, no delays"));
        
        if !config.autoclicker.turbo_mode {
            widgets::styled_slider(ui, "Delay", &mut config.autoclicker.delay_ms, 10.0..=1000.0, "ms");
            widgets::styled_slider(ui, "Variance", &mut config.autoclicker.variance_percent, 0.0..=50.0, "%");
            widgets::styled_slider(ui, "Hold", &mut config.autoclicker.hold_duration_ms, 10.0..=200.0, "ms");
        }
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "SEQUENCE", accent, |ui| {
        // Display current sequence
        if sequence_display.is_empty() {
            ui.label(egui::RichText::new("No buttons recorded").size(9.0).color(theme::TEXT_MUTED).italics());
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (idx, name) in sequence_display.iter().enumerate() {
                    let is_current = is_running && idx == current_index;
                    let bg = if is_current { theme::ACCENT_PRIMARY } else { theme::BG_LIGHT };
                    let text_color = if is_current { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY };
                    
                    egui::Frame::none()
                        .fill(bg)
                        .rounding(3.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(name).size(9.0).color(text_color));
                        });
                }
            });
        }
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "CONTROLS", accent, |ui| {
        // Record button
        ui.horizontal(|ui| {
            let record_text = if is_recording { "⏹ Stop Recording" } else { "⏺ Record Buttons" };
            let record_color = if is_recording { theme::ACCENT_WARNING } else { theme::TEXT_PRIMARY };
            
            let record_btn = egui::Button::new(egui::RichText::new(record_text).size(10.0).color(record_color))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, if is_recording { theme::ACCENT_WARNING } else { theme::BORDER_DEFAULT }))
                .rounding(4.0)
                .min_size(egui::vec2(ui.available_width() * 0.65, 18.0));
            
            if ui.add_enabled(!is_running, record_btn).clicked() {
                if is_recording {
                    autoclicker.stop_recording();
                } else {
                    autoclicker.start_recording();
                }
            }
            
            // Clear button
            let clear_btn = egui::Button::new(egui::RichText::new("Clear").size(10.0).color(theme::TEXT_SECONDARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0);
            
            if ui.add_enabled(!is_running && !is_recording && sequence_len > 0, clear_btn).clicked() {
                autoclicker.clear_sequence();
            }
        });

        // Undo last button
        if !is_running && !is_recording && sequence_len > 0 {
            if ui.add(
                egui::Button::new(egui::RichText::new("↩ Remove Last").size(9.0).color(theme::TEXT_MUTED))
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
            ).clicked() {
                autoclicker.remove_last();
            }
        }

        ui.add_space(4.0);
        
        // On/Off toggle button
        ui.horizontal(|ui| {
            let (toggle_text, toggle_color, toggle_stroke) = if is_running {
                ("Stop Clicker", theme::ACCENT_DANGER, theme::ACCENT_DANGER)
            } else {
                ("Start Clicker", theme::ACCENT_SUCCESS, theme::ACCENT_SUCCESS)
            };
            
            let toggle_btn = egui::Button::new(egui::RichText::new(toggle_text).size(10.0).color(toggle_color).strong())
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, toggle_stroke))
                .rounding(4.0)
                .min_size(egui::vec2(ui.available_width(), 22.0));
            
            if ui.add_enabled(!is_recording && sequence_len > 0, toggle_btn).clicked() {
                autoclicker.toggle(&config.autoclicker);
            }
        });
        
        ui.label(egui::RichText::new("or press [Insert] to toggle").size(8.0).color(theme::TEXT_MUTED));
    });
}

fn render_hitbox_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "HITBOX MODIFIER", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.hitbox.enabled, "Enable Hitbox Mod", Some("F11"));
        ui.label(egui::RichText::new("Modifies primitive sizes").size(9.0).color(theme::ACCENT_WARNING));
        
        ui.add_space(4.0);
        widgets::premium_feature_toggle(ui, &mut config.hitbox.show_visual, "Show Hitbox Visual", None);
        if config.hitbox.show_visual {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Color").size(10.0).color(theme::TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut color = egui::Color32::from_rgba_unmultiplied(
                        (config.hitbox.color[0] * 255.0) as u8,
                        (config.hitbox.color[1] * 255.0) as u8,
                        (config.hitbox.color[2] * 255.0) as u8,
                        (config.hitbox.color[3] * 255.0) as u8,
                    );
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        config.hitbox.color = [
                            color.r() as f32 / 255.0,
                            color.g() as f32 / 255.0,
                            color.b() as f32 / 255.0,
                            color.a() as f32 / 255.0,
                        ];
                    }
                });
            });
        }
    });

    ui.add_space(3.0);
    
    widgets::double_border_frame(ui, "ENEMY HITBOXES", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.hitbox.enemy_enabled, "expand enemy hitboxes", None);
        ui.label(egui::RichText::new("easier to hit them").size(9.0).color(theme::TEXT_MUTED));
        
        if config.hitbox.enemy_enabled {
            ui.add_space(4.0);
            widgets::styled_slider(ui, "Head", &mut config.hitbox.head_scale, 1.0..=5.0, "x");
            widgets::styled_slider(ui, "Torso", &mut config.hitbox.torso_scale, 1.0..=4.0, "x");
            widgets::styled_slider(ui, "Arms", &mut config.hitbox.arms_scale, 1.0..=3.0, "x");
            widgets::styled_slider(ui, "Legs", &mut config.hitbox.legs_scale, 1.0..=3.0, "x");
        }
    });
    
    ui.add_space(3.0);
    
    widgets::double_border_frame(ui, "YOUR HITBOX", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.hitbox.self_enabled, "shrink your hitbox", None);
        ui.label(egui::RichText::new("harder to hit you").size(9.0).color(theme::TEXT_MUTED));
        
        if config.hitbox.self_enabled {
            ui.add_space(4.0);
            widgets::styled_slider(ui, "Self Scale", &mut config.hitbox.self_scale, 0.1..=2.0, "x");
        }
    });


}

fn render_hotkeys_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    // Instructions section
    widgets::double_border_frame(ui, "HOTKEY BINDINGS", accent, |ui| {
        ui.label(egui::RichText::new("Configure custom hotkeys for quick feature toggles").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("Select a key and the feature it should toggle").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("⚠ F1 = Menu, F12 = Exit, Home = Refresh, End = Save").size(8.0).color(theme::ACCENT_WARNING));
    });
    
    ui.add_space(3.0);
    
    // Visual features section
    widgets::double_border_frame(ui, "HOTKEY SLOTS", accent, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Key").size(9.0).color(theme::TEXT_MUTED));
            ui.add_space(50.0);
            ui.label(egui::RichText::new("Feature").size(9.0).color(theme::TEXT_MUTED));
        });
        ui.add_space(6.0);
        
        // Render all 10 hotkey slots
        for i in 0..10 {
            widgets::hotkey_slot(ui, i, &mut config.hotkey_bindings.slots[i]);
            ui.add_space(2.0);
        }
    });
    
    ui.add_space(3.0);
    
    // Quick category info
    widgets::double_border_frame(ui, "AVAILABLE FEATURES", accent, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            
            // Visual category
            ui.label(egui::RichText::new("VISUAL:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("ESP, Tags, Tracers, Health, Armour, Chams, Team, Dead, Bots").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("AIM:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Aim Assist, Triggerbot, Camera Aim, Auto Reload").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("MOVEMENT:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Fly, Noclip, Spinbot, Anti-Sit, Void Hide").size(10.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(3.0);
        
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("HITBOX:").size(10.0).color(theme::ACCENT_INFO).strong());
            ui.label(egui::RichText::new("Hitbox Mod, Show Visual").size(10.0).color(theme::TEXT_MUTED));
        });
    });
    
    ui.add_space(3.0);
    
    // Hotkey panel settings (moved from MISC)
    widgets::double_border_frame(ui, "HOTKEY PANEL", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.interface.show_hotkey_hints, "Show Panel", None);
    
        if config.interface.show_hotkey_hints {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Drag panel to move, or use presets:").size(9.0).color(theme::TEXT_MUTED));
            ui.horizontal(|ui| {
                if ui.add(
                    egui::Button::new(egui::RichText::new("↖ TL").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Top-Left").clicked() {
                    config.interface.hotkey_pos_x = 10.0;
                    config.interface.hotkey_pos_y = 10.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↗ TR").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Top-Right").clicked() {
                    config.interface.hotkey_pos_x = 1800.0;
                    config.interface.hotkey_pos_y = 10.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↙ BL").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Bottom-Left").clicked() {
                    config.interface.hotkey_pos_x = 10.0;
                    config.interface.hotkey_pos_y = 900.0;
                }
                if ui.add(
                    egui::Button::new(egui::RichText::new("↘ BR").size(9.0))
                        .fill(theme::BG_LIGHT)
                        .rounding(3.0)
                        .min_size(egui::vec2(35.0, 16.0))
                ).on_hover_text("Bottom-Right").clicked() {
                    config.interface.hotkey_pos_x = 1800.0;
                    config.interface.hotkey_pos_y = 900.0;
                }
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("X:").size(9.0).color(theme::TEXT_MUTED));
                ui.add(egui::DragValue::new(&mut config.interface.hotkey_pos_x)
                    .speed(1.0)
                    .range(0.0..=3000.0));
                ui.label(egui::RichText::new("Y:").size(9.0).color(theme::TEXT_MUTED));
                ui.add(egui::DragValue::new(&mut config.interface.hotkey_pos_y)
                    .speed(1.0)
                    .range(0.0..=2000.0));
            });
        }
    });
}

/// Arsenal melee skin options
const ARSENAL_MELEE_OPTIONS: &[&str] = &[
    "Default",
    "Knife",
    "Taser",
    "Frying Pan",
    "Crowbar",
    "Katana",
    "Bat",
    "Stop Sign",
    "Shovel",
    "Icicle",
    "Candy Cane",
    "Karambit",
    "Butterfly Knife",
    "Machete",
    "Cleaver",
    "Hammer",
    "Axe",
    "Pipe",
    "Baguette",
    "Golf Club",
];

fn render_skin_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "COSMETICS", accent, |ui| {
        ui.label(egui::RichText::new("Visual character modifications").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(2.0);
        
        widgets::premium_feature_toggle(ui, &mut config.cosmetics.korblox, "Fake Korblox", None);
        if config.cosmetics.korblox {
            ui.label(egui::RichText::new("  └ Makes right leg invisible + Korblox mesh").size(8.0).color(theme::TEXT_MUTED));
        }
        
        widgets::premium_feature_toggle(ui, &mut config.cosmetics.headless, "Headless", None);
        if config.cosmetics.headless {
            ui.label(egui::RichText::new("  └ Makes head invisible").size(8.0).color(theme::TEXT_MUTED));
        }
        
        widgets::premium_feature_toggle(ui, &mut config.cosmetics.hide_face, "Hide Face", None);
        if config.cosmetics.hide_face {
            ui.label(egui::RichText::new("  └ Makes face decal invisible").size(8.0).color(theme::TEXT_MUTED));
        }
    });

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Game-specific skins (Arsenal) moved to Game tab").size(8.0).color(theme::TEXT_MUTED).italics());
}

fn render_misc_tab(ui: &mut egui::Ui, config: &mut Config, cache: &Arc<Cache>) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "DESYNC", accent, |ui| {
        widgets::premium_feature_toggle(ui, &mut config.desync.enabled, "Desync", None);
        ui.label(egui::RichText::new("Freezes outbound physics replication").size(9.0).color(theme::TEXT_MUTED));
        
        if config.desync.enabled {
            ui.add_space(4.0);

            // -- Strength selector (FullFreeze vs Throttled) --
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Strength").size(10.0).color(theme::TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let strength_name = match config.desync.strength {
                        DesyncStrength::FullFreeze => "Full Freeze",
                        DesyncStrength::Throttled  => "Throttled",
                    };
                    egui::ComboBox::from_id_source("desync_strength")
                        .selected_text(strength_name)
                        .width(90.0)
                        .show_ui(ui, |ui| {
                            for &(strength, name) in DESYNC_STRENGTH_OPTIONS {
                                ui.selectable_value(&mut config.desync.strength, strength, name);
                            }
                        });
                });
            });
            match config.desync.strength {
                DesyncStrength::FullFreeze => {
                    ui.label(egui::RichText::new("Bandwidth → 0 — full replication stop (client-auth games)").size(8.0).color(theme::TEXT_MUTED));
                }
                DesyncStrength::Throttled => {
                    ui.label(egui::RichText::new("Low bandwidth — laggy updates, works on more games").size(8.0).color(theme::TEXT_MUTED));
                    ui.add_space(2.0);
                    let mut bps_f32 = config.desync.throttle_bps as f32;
                    widgets::styled_slider(ui, "Bandwidth", &mut bps_f32, 50.0..=2000.0, "bps");
                    config.desync.throttle_bps = bps_f32 as i32;
                }
            }

            ui.add_space(4.0);
            widgets::premium_feature_toggle(ui, &mut config.desync.strong_mode, "Strong Mode", None);
            ui.label(egui::RichText::new("Also freezes WorldStepMax (stronger lag)").size(8.0).color(theme::TEXT_MUTED));

            ui.add_space(6.0);
            // -- Activation mode selector --
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Mode").size(10.0).color(theme::TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_name = match config.desync.mode {
                        DesyncMode::Hold   => "Hold",
                        DesyncMode::Toggle => "Toggle",
                        DesyncMode::Timed  => "Timed",
                    };
                    egui::ComboBox::from_id_source("desync_mode")
                        .selected_text(mode_name)
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for &(mode, name) in DESYNC_MODE_OPTIONS {
                                ui.selectable_value(&mut config.desync.mode, mode, name);
                            }
                        });
                });
            });
            match config.desync.mode {
                DesyncMode::Hold => {
                    ui.label(egui::RichText::new("Hold key to freeze, release to unfreeze").size(8.0).color(theme::TEXT_MUTED));
                }
                DesyncMode::Toggle => {
                    ui.label(egui::RichText::new("Press once to activate, press again to stop").size(8.0).color(theme::TEXT_MUTED));
                }
                DesyncMode::Timed => {
                    ui.label(egui::RichText::new("Press to activate — auto-releases after timer").size(8.0).color(theme::TEXT_MUTED));
                    ui.add_space(2.0);
                    widgets::styled_slider(ui, "Duration", &mut config.desync.auto_release_secs, 1.0..=6.0, "s");
                }
            }

            ui.add_space(4.0);
            // -- Key selector --
            ui.horizontal(|ui| {
                let label = match config.desync.mode {
                    DesyncMode::Hold => "Hold Key",
                    _ => "Activate Key",
                };
                ui.label(egui::RichText::new(label).size(10.0).color(theme::TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let key_name = vk_to_name(config.desync.keybind);
                    egui::ComboBox::from_id_source("desync_key")
                        .selected_text(key_name)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for &(vk, name) in DESYNC_KEY_OPTIONS {
                                ui.selectable_value(&mut config.desync.keybind, vk, name);
                            }
                        });
                });
            });
            ui.label(egui::RichText::new("⚠ Other players see you lagged while active").size(8.0).color(theme::ACCENT_WARNING));
        }
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "ANTI-AFK", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.anti_afk.enabled, "Anti-AFK", None);
        ui.label(egui::RichText::new("Prevents idle kick").size(9.0).color(theme::TEXT_MUTED));
        if config.anti_afk.enabled {
            let mut interval_f32 = config.anti_afk.interval_secs as f32;
            widgets::styled_slider(ui, "Interval", &mut interval_f32, 30.0..=180.0, "s");
            config.anti_afk.interval_secs = interval_f32 as u32;
        }
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "GAME DATA", accent, |ui| {
        let cache_count = cache.count();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Players Cached").size(10.0).color(theme::TEXT_PRIMARY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let color = if cache_count > 0 { theme::ACCENT_SUCCESS } else { theme::TEXT_MUTED };
            ui.label(egui::RichText::new(format!("{}", cache_count)).size(10.0).color(color).strong());
        });
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new(egui::RichText::new("↻ Reload").size(9.0).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0)
                .min_size(egui::vec2(80.0, 18.0))
        ).on_hover_text("F9 - Refresh player cache").clicked() {
            super::app::PENDING_RELOAD.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        ui.add_space(4.0);

        if ui.add(
            egui::Button::new(egui::RichText::new("🔄 Full Sync").size(9.0).color(theme::TEXT_PRIMARY))
                .fill(theme::BG_LIGHT)
                .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
                .rounding(4.0)
                .min_size(egui::vec2(80.0, 18.0))
        ).on_hover_text("Home - Re-read game instances").clicked() {
            super::app::PENDING_REFRESH.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    });
}

fn render_game_tab(ui: &mut egui::Ui, config: &mut Config, _bb_debug: &Arc<Mutex<BladeBallDebugState>>) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);

    // Supported games with custom adaptations
    widgets::double_border_frame(ui, "SUPPORTED GAMES", accent, |ui| {
        ui.label(egui::RichText::new("Games with custom player detection & team support:").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);

        let games: &[(&str, &str)] = &[
            ("Phantom Forces",  "Workspace players, tag-colour teams, armor"),
            ("Operation One",   "Workspace models, Electronic:Config alive check, Highlight teams"),
            ("Blox Strike",     "Folder-based CT/T teams, Characters workspace scan"),
            ("Blade Ball",      "Auto-parry, ball tracking, TTI prediction"),
            ("Rivals",          "TeammateLabel detection, all aim modes"),
            ("Fallen",          "Workspace player models, FFA (no teams)"),
            ("Aftermath",       "Entity-folder workspace scan, GUID entities"),
            ("Arsenal",         "Melee skin changer"),
        ];

        for (name, desc) in games {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(*name).size(9.5).color(theme::ACCENT_PRIMARY).strong());
                ui.label(egui::RichText::new("—").size(9.0).color(theme::TEXT_MUTED));
                ui.label(egui::RichText::new(*desc).size(8.5).color(theme::TEXT_SECONDARY));
            });
        }

        ui.add_space(4.0);
        ui.label(egui::RichText::new("All other Roblox games use standard Players-service detection.").size(8.5).color(theme::TEXT_MUTED).italics());
    });

    ui.add_space(3.0);

    // Blade Ball section
    widgets::double_border_frame(ui, "BLADE BALL", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.blade_ball.enabled, "Auto Parry", None);
        ui.label(egui::RichText::new("Instantly parries when targeted + ball is close").size(9.0).color(theme::TEXT_MUTED));

        if config.blade_ball.enabled {
            ui.add_space(4.0);
            widgets::styled_slider(ui, "Distance", &mut config.blade_ball.parry_distance, 3.0..=50.0, " studs");
            ui.label(egui::RichText::new("Fallback distance trigger (TTI also used)").size(8.0).color(theme::TEXT_MUTED));

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Parry Input").size(10.0).color(theme::TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let input_name = match config.blade_ball.parry_input {
                        ParryInput::FKey => "F Key",
                        ParryInput::LeftClick => "Left Click",
                    };
                    egui::ComboBox::from_id_source("bb_parry_input")
                        .selected_text(input_name)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut config.blade_ball.parry_input, ParryInput::FKey, "F Key");
                            ui.selectable_value(&mut config.blade_ball.parry_input, ParryInput::LeftClick, "Left Click");
                        });
                });
            });

            ui.add_space(4.0);
        }
    });

    ui.add_space(3.0);

    // Arsenal section
    widgets::double_border_frame(ui, "ARSENAL", accent, |ui| {
        ui.label(egui::RichText::new("Melee skin modification for Arsenal").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(2.0);

        widgets::styled_toggle(ui, &mut config.cosmetics.arsenal_enabled, "Enable Skin Changer", None);

        if config.cosmetics.arsenal_enabled {
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Melee Skin").size(10.0).color(theme::TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_source("game_arsenal_melee_skin")
                        .selected_text(&config.cosmetics.arsenal_melee_skin)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for &skin in ARSENAL_MELEE_OPTIONS {
                                ui.selectable_value(&mut config.cosmetics.arsenal_melee_skin, skin.to_string(), skin);
                            }
                        });
                });
            });

            ui.add_space(4.0);
            widgets::styled_toggle(ui, &mut config.cosmetics.arsenal_swing_fix, "Swing Fix", None);
            ui.label(egui::RichText::new("  Reverts to default knife when attacking").size(8.0).color(theme::TEXT_MUTED));

            ui.add_space(4.0);
            ui.label(egui::RichText::new("Arsenal game only").size(8.0).color(theme::ACCENT_WARNING));
        }
    });

    ui.add_space(3.0);

    // Rivals section - tips/info
    widgets::double_border_frame(ui, "RIVALS", accent, |ui| {
        ui.label(egui::RichText::new("Rivals features are spread across tabs:").size(9.0).color(theme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Aim").size(9.0).color(theme::ACCENT_PRIMARY).strong());
            ui.label(egui::RichText::new("Silent Aim, Viewport Aim, Camera Aim").size(9.0).color(theme::TEXT_SECONDARY));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Visuals").size(9.0).color(theme::ACCENT_PRIMARY).strong());
            ui.label(egui::RichText::new("ESP, Chams, Tracers").size(9.0).color(theme::TEXT_SECONDARY));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Move").size(9.0).color(theme::ACCENT_PRIMARY).strong());
            ui.label(egui::RichText::new("Speed, Fly, Noclip").size(9.0).color(theme::TEXT_SECONDARY));
        });
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Use Viewport Aim for best Rivals results").size(9.0).color(theme::ACCENT_INFO));
    });
}

fn render_performance_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    widgets::double_border_frame(ui, "FPS", accent, |ui| {
        let mut fps_f32 = config.performance.target_fps as f32;
        widgets::styled_slider(ui, "Target", &mut fps_f32, 10.0..=144.0, "");
        config.performance.target_fps = fps_f32 as u32;
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "IDLE", accent, |ui| {
        widgets::styled_toggle(ui, &mut config.performance.idle_mode, "Idle Mode", None);
        if config.performance.idle_mode {
            let mut idle_f32 = config.performance.idle_fps as f32;
            widgets::styled_slider(ui, "Idle FPS", &mut idle_f32, 1.0..=30.0, "");
            config.performance.idle_fps = idle_f32 as u32;
        }
    });

    ui.add_space(3.0);
    widgets::double_border_frame(ui, "CACHE", accent, |ui| {
        let mut cache_f32 = config.performance.cache_update_ms as f32;
        widgets::styled_slider(ui, "Rate", &mut cache_f32, 16.0..=200.0, "ms");
        config.performance.cache_update_ms = cache_f32 as u64;
    });
}

fn render_about_tab(ui: &mut egui::Ui, config: &mut Config) {
    let accent = theme::accent_from_rgb(config.interface.accent_r, config.interface.accent_g, config.interface.accent_b);
    
    ui.add_space(6.0);
    
    // Accent Color Picker
    widgets::double_border_frame(ui, "ACCENT COLOR", accent, |ui| {
        ui.horizontal(|ui| {
            let preview_size = egui::vec2(24.0, 16.0);
            let (preview_rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
            ui.painter().rect_filled(preview_rect, 2.0, accent);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("({}, {}, {})", config.interface.accent_r, config.interface.accent_g, config.interface.accent_b)).size(9.0).color(theme::TEXT_MUTED));
        });
        
        ui.add_space(4.0);
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("R").size(9.0).color(egui::Color32::from_rgb(255, 100, 100)));
            let mut r = config.interface.accent_r as f32;
            ui.add(egui::Slider::new(&mut r, 0.0..=255.0).show_value(false));
            config.interface.accent_r = r as u8;
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("G").size(9.0).color(egui::Color32::from_rgb(100, 255, 100)));
            let mut g = config.interface.accent_g as f32;
            ui.add(egui::Slider::new(&mut g, 0.0..=255.0).show_value(false));
            config.interface.accent_g = g as u8;
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("B").size(9.0).color(egui::Color32::from_rgb(100, 100, 255)));
            let mut b = config.interface.accent_b as f32;
            ui.add(egui::Slider::new(&mut b, 0.0..=255.0).show_value(false));
            config.interface.accent_b = b as u8;
        });
        
        ui.add_space(3.0);
        
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Presets").size(8.0).color(theme::TEXT_MUTED));
            ui.add_space(4.0);
            let presets = [
                ("Blue", 100u8, 100u8, 220u8),
                ("Purple", 150, 80, 200),
                ("Pink", 200, 80, 120),
                ("Cyan", 80, 180, 200),
                ("Green", 80, 200, 120),
                ("Orange", 220, 150, 80),
                ("Red", 220, 60, 60),
                ("White", 200, 200, 210),
            ];
            for (name, r, g, b) in presets {
                let color = egui::Color32::from_rgb(r, g, b);
                let btn_size = egui::vec2(14.0, 14.0);
                let (btn_rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                ui.painter().rect_filled(btn_rect, 2.0, color);
                if response.on_hover_text(name).clicked() {
                    config.interface.accent_r = r;
                    config.interface.accent_g = g;
                    config.interface.accent_b = b;
                }
            }
        });
    });
    
    ui.add_space(8.0);
    
    // Logo/Title - compact
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(accent)
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(14.0, 6.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("N").size(28.0).color(theme::BG_DARK).strong());
            });
        ui.add_space(8.0);
        ui.label(egui::RichText::new("NEXUS UNDERGROUND").size(16.0).color(theme::TEXT_PRIMARY).strong());
        ui.label(egui::RichText::new("v3.2.0").size(10.0).color(theme::TEXT_MUTED));
    });
    
    ui.add_space(10.0);
    
    // Made by - compact
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(theme::BG_LIGHT)
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Made by").size(10.0).color(theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("NexusUnderground").size(11.0).color(theme::ACCENT_PRIMARY).strong());
                });
            });
    });
    
    ui.add_space(8.0);
    
    // Credits/Info
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("“My crime is that of curiosity”").size(10.0).color(theme::TEXT_MUTED).italics());
    });
    
    ui.add_space(16.0);    
    // Discord link - corrected URL
    ui.vertical_centered(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(88, 101, 242)) // Discord blurple
            .rounding(6.0)
            .inner_margin(egui::Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                if ui.add(
                    egui::Label::new(egui::RichText::new("Join Discord").size(11.0).color(egui::Color32::WHITE).strong())
                        .sense(egui::Sense::click())
                 ).on_hover_text("https://tr.ee/NexusD").clicked() {
                    let _ = open::that("https://tr.ee/NexusD");
                }
            });
    });
    
    ui.add_space(8.0);    
    // Quick tips - compact
    widgets::section_header(ui, "QUICK TIPS");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(egui::RichText::new("[F1]").size(9.0).color(egui::Color32::from_rgb(100, 150, 255)).strong());
        ui.label(egui::RichText::new("Menu").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[END]").size(9.0).color(egui::Color32::from_rgb(100, 220, 100)).strong());
        ui.label(egui::RichText::new("Save").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[HOME]").size(9.0).color(egui::Color32::from_rgb(255, 180, 50)).strong());
        ui.label(egui::RichText::new("Refresh").size(9.0).color(theme::TEXT_MUTED));
        ui.label(egui::RichText::new("[F12]").size(9.0).color(egui::Color32::from_rgb(255, 80, 80)).strong());
        ui.label(egui::RichText::new("Exit").size(9.0).color(theme::TEXT_MUTED));
    });
}
