#![allow(dead_code)]

use eframe::egui::Color32;

pub const BG_DARK: Color32 = Color32::from_rgb(12, 12, 18);        // Main window background
pub const BG_MEDIUM: Color32 = Color32::from_rgb(18, 18, 26);      // Section/frame background  
pub const BG_LIGHT: Color32 = Color32::from_rgb(28, 28, 38);       // Input fields, buttons
pub const BG_HOVER: Color32 = Color32::from_rgb(35, 35, 48);       // Hover state
pub const BG_FRAME: Color32 = Color32::from_rgb(14, 14, 22);       // Inner frame background
pub const BG_FRAME_INNER: Color32 = Color32::from_rgb(10, 10, 16); // Inner double-border frame

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(100, 100, 220);   // Blue/purple main accent
pub const ACCENT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 255); // Brighter blue for highlights
pub const ACCENT_PRIMARY_DIM: Color32 = Color32::from_rgb(70, 70, 160); // Dimmed blue for inactive
pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(80, 200, 120);    // Green for enabled
pub const ACCENT_WARNING: Color32 = Color32::from_rgb(220, 180, 80);    // Gold/yellow
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(200, 70, 70);      // Red
pub const ACCENT_INFO: Color32 = Color32::from_rgb(80, 140, 220);       // Light blue

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 235);     // Main text (slightly off-white)
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 150, 165);   // Secondary text
pub const TEXT_MUTED: Color32 = Color32::from_rgb(85, 85, 100);         // Muted/disabled text
pub const TEXT_LABEL: Color32 = Color32::from_rgb(180, 180, 195);       // Labels
pub const TEXT_HEADER: Color32 = Color32::from_rgb(200, 200, 220);      // Section headers

pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(40, 40, 55);      // Default borders
pub const BORDER_FRAME: Color32 = Color32::from_rgb(50, 50, 70);        // Frame borders (outer)
pub const BORDER_FRAME_INNER: Color32 = Color32::from_rgb(35, 35, 50);  // Frame borders (inner)
pub const BORDER_FOCUS: Color32 = Color32::from_rgb(100, 100, 220);     // Focused element border
pub const BORDER_ACTIVE: Color32 = Color32::from_rgb(80, 200, 120);     // Active element border

pub fn accent_from_rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
