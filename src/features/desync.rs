use std::sync::Arc;
use crate::config::DesyncConfig;
use crate::core::Memory;

pub struct Desync {
    _memory: Arc<Memory>,
}

impl Desync {
    pub fn new(memory: Arc<Memory>, _base_address: u64) -> Self {
        Self { _memory: memory }
    }

    pub fn update(&mut self, _config: &DesyncConfig, _keybind_held: bool) {}
    pub fn cleanup(&mut self) {}

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool { false }

    #[allow(dead_code)]
    pub fn is_restoring(&self) -> bool { false }

    #[allow(dead_code)]
    pub fn read_current_bandwidth(&self) -> i32 { 0 }

    #[allow(dead_code)]
    pub fn get_original_bandwidth(&self) -> Option<i32> { None }
}

impl Drop for Desync {
    fn drop(&mut self) {}
}
