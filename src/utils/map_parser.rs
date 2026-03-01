use std::sync::atomic::{AtomicBool, Ordering};
use crate::utils::math::Vector3;
use crate::sdk::Instance;

#[derive(Clone)]
pub struct ParsedPart {
    pub position: Vector3,
    pub half_size: Vector3,
}

impl ParsedPart {
    pub fn new(position: Vector3, size: Vector3) -> Self {
        Self {
            position,
            half_size: Vector3 {
                x: size.x / 2.0,
                y: size.y / 2.0,
                z: size.z / 2.0,
            },
        }
    }
}

pub struct MapParser {
    enabled: AtomicBool,
}

impl MapParser {
    pub fn new() -> Self {
        Self { enabled: AtomicBool::new(false) }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn is_scanning(&self) -> bool { false }
    pub fn should_rescan(&self) -> bool { false }
    pub fn scan(&self, _workspace: &Instance) {}
    pub fn clear(&self) {}
    pub fn part_count(&self) -> usize { 0 }
    pub fn log_debug_stats(&self) {}

    pub fn is_visible(&self, _start: Vector3, _end: Vector3) -> bool {
        true
    }
}

impl Default for MapParser {
    fn default() -> Self { Self::new() }
}

lazy_static::lazy_static! {
    pub static ref MAP_PARSER: MapParser = MapParser::new();
}

pub fn get_map_parser() -> &'static MapParser {
    &MAP_PARSER
}
