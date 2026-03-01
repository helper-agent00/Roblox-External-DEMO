#![allow(dead_code)]

use std::sync::Arc;

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{camera, visual_engine};
use crate::utils::math::{Matrix4, Vector2, Vector3, Vector4};

/// Represents the Visual Engine (camera/rendering).
pub struct VisualEngine {
    pub address: u64,
    memory: Arc<Memory>,
}

impl VisualEngine {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self { address, memory }
    }

    pub fn get_dimensions(&self) -> Vector2 {
        self.memory.read::<Vector2>(self.address + visual_engine::dimensions())
    }

    #[inline]
    pub fn screen_center(&self) -> Vector2 {
        let d = self.get_dimensions();
        Vector2::new(d.x / 2.0, d.y / 2.0)
    }

    pub fn get_view_matrix(&self) -> Matrix4 {
        self.memory.read::<Matrix4>(self.address + visual_engine::view_matrix())
    }

    /// Resolves camera address, then reads Camera.Position.
    pub fn get_camera_position(&self) -> Option<Vector3> {
        let cam_addr = self.memory.resolve_camera_address()?;
        let pos_addr = cam_addr + camera::position();
        if !is_valid_address(pos_addr) {
            return None;
        }
        let pos = self.memory.read::<Vector3>(pos_addr);
        if pos.is_valid() && pos.y.abs() < 1_000_000.0 {
            Some(pos)
        } else {
            None
        }
    }

    /// Client area offset for correct windowed ESP positioning.
    #[cfg(target_os = "windows")]
    pub fn get_window_offset(&self) -> Vector2 {
        use windows::Win32::Foundation::{POINT, RECT};
        use windows::Win32::Graphics::Gdi::ClientToScreen;
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect};

        unsafe {
            if let Ok(hwnd) = FindWindowW(None, windows::core::w!("Roblox")) {
                let mut client_rect = RECT::default();
                let mut top_left = POINT { x: 0, y: 0 };

                if GetClientRect(hwnd, &mut client_rect).is_ok() {
                    if ClientToScreen(hwnd, &mut top_left).as_bool() {
                        return Vector2::new(top_left.x as f32, top_left.y as f32);
                    }
                }
            }
        }

        Vector2::ZERO
    }

 
    /// World → screen. Returns None if behind camera or outside frustum.
    pub fn world_to_screen(
        &self,
        world: Vector3,
        dims: Vector2,
        view: &Matrix4,
    ) -> Option<Vector2> {
        let clip = view.transform(Vector4::new(world.x, world.y, world.z, 1.0));

        if clip.w < 0.001 {
            return None;
        }

        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;

        if ndc_x.abs() > 2.0 || ndc_y.abs() > 2.0 {
            return None;
        }

        let screen_x = (dims.x * 0.5) * ndc_x + (dims.x * 0.5);
        let screen_y = -(dims.y * 0.5) * ndc_y + (dims.y * 0.5);

        Some(Vector2::new(screen_x, screen_y))
    }

    /// Like world_to_screen but without frustum cull — for tracers.
    /// Off-screen coords are fine since egui clips lines automatically.
    pub fn world_to_screen_wide(
        &self,
        world: Vector3,
        dims: Vector2,
        view: &Matrix4,
    ) -> Option<Vector2> {
        let clip = view.transform(Vector4::new(world.x, world.y, world.z, 1.0));

        // Behind camera — no tracer
        if clip.w < 0.001 {
            return None;
        }

        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;

        let screen_x = (dims.x * 0.5) * ndc_x + (dims.x * 0.5);
        let screen_y = -(dims.y * 0.5) * ndc_y + (dims.y * 0.5);

        Some(Vector2::new(screen_x, screen_y))
    }

    pub fn get_depth(&self, world: Vector3, view: &Matrix4) -> f32 {
        let clip = view.transform(Vector4::new(world.x, world.y, world.z, 1.0));
        clip.w
    }
}
