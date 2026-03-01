use std::sync::Arc;
use eframe::egui;
use crate::core::Memory;
use super::theme;

pub fn init_dex_explorer(_memory: Arc<Memory>) {}

pub fn toggle_window() {}

pub fn is_window_open() -> bool { false }

pub fn render_dex_window(_ctx: &egui::Context) {}

pub fn render_dex_tab(ui: &mut egui::Ui) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("DEX Explorer")
                .size(14.0)
                .color(theme::TEXT_PRIMARY),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("This is a premium feature.")
                .size(11.0)
                .color(theme::ACCENT_WARNING),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Join the Discord for full access.")
                .size(10.0)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(6.0);
        ui.hyperlink_to(
            egui::RichText::new("Nexus Underground - Discord")
                .size(11.0)
                .color(theme::ACCENT_PRIMARY)
                .underline(),
            "https://tr.ee/NexusD",
        );
    });
}
