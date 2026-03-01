use eframe::egui;
use std::sync::Arc;

use crate::config::Config;
use crate::sdk::VisualEngine;

pub struct Crosshair;

impl Crosshair {
    pub fn render(ctx: &egui::Context, config: &Config, visengine: &Arc<VisualEngine>) {
        let style = config.visuals.crosshair_style;
        if style == 0 {
            return;
        }

        let dimensions = visengine.get_dimensions();
        let window_offset = visengine.get_window_offset();
        let center = egui::pos2(
            window_offset.x + dimensions.x / 2.0,
            window_offset.y + dimensions.y / 2.0,
        );

        let color = egui::Color32::from_rgb(
            (config.visuals.crosshair_color[0] * 255.0) as u8,
            (config.visuals.crosshair_color[1] * 255.0) as u8,
            (config.visuals.crosshair_color[2] * 255.0) as u8,
        );
        let outline = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200);
        let size = config.visuals.crosshair_size;
        let thickness = config.visuals.crosshair_thickness;
        let gap = config.visuals.crosshair_gap;

        egui::Area::new(egui::Id::new("crosshair_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                let painter = ui.painter();

                match style {
                    1 => {
                        // Cross with gap
                        Self::draw_cross(painter, center, size, gap, thickness, color, outline);
                    }
                    2 => {
                        // Dot only
                        Self::draw_dot(painter, center, thickness + 0.5, color, outline);
                    }
                    3 => {
                        // Circle + dot
                        painter.circle_stroke(center, size, egui::Stroke::new(1.0, outline));
                        painter.circle_stroke(center, size, egui::Stroke::new(thickness, color));
                        Self::draw_dot(painter, center, thickness, color, outline);
                    }
                    4 => {
                        // Cross + dot
                        Self::draw_cross(painter, center, size, gap, thickness, color, outline);
                        Self::draw_dot(painter, center, thickness, color, outline);
                    }
                    _ => {}
                }
            });
    }

    fn draw_cross(
        painter: &egui::Painter,
        center: egui::Pos2,
        size: f32,
        gap: f32,
        thickness: f32,
        color: egui::Color32,
        outline: egui::Color32,
    ) {
        let outline_stroke = egui::Stroke::new(thickness + 1.0, outline);
        let stroke = egui::Stroke::new(thickness, color);

        // Top
        let top_start = egui::pos2(center.x, center.y - gap - size);
        let top_end = egui::pos2(center.x, center.y - gap);
        // Bottom
        let bot_start = egui::pos2(center.x, center.y + gap);
        let bot_end = egui::pos2(center.x, center.y + gap + size);
        // Left
        let left_start = egui::pos2(center.x - gap - size, center.y);
        let left_end = egui::pos2(center.x - gap, center.y);
        // Right
        let right_start = egui::pos2(center.x + gap, center.y);
        let right_end = egui::pos2(center.x + gap + size, center.y);

        // Outline pass
        painter.line_segment([top_start, top_end], outline_stroke);
        painter.line_segment([bot_start, bot_end], outline_stroke);
        painter.line_segment([left_start, left_end], outline_stroke);
        painter.line_segment([right_start, right_end], outline_stroke);

        // Color pass
        painter.line_segment([top_start, top_end], stroke);
        painter.line_segment([bot_start, bot_end], stroke);
        painter.line_segment([left_start, left_end], stroke);
        painter.line_segment([right_start, right_end], stroke);
    }

    fn draw_dot(
        painter: &egui::Painter,
        center: egui::Pos2,
        size: f32,
        color: egui::Color32,
        outline: egui::Color32,
    ) {
        painter.circle_filled(center, size + 0.5, outline);
        painter.circle_filled(center, size, color);
    }
}
