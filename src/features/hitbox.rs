use std::sync::Arc;
use crate::config::HitboxConfig;
use crate::core::Memory;
use crate::sdk::Instance;
use crate::utils::cache::Cache;

pub struct HitboxExpander {
    _memory: Arc<Memory>,
}

impl HitboxExpander {
    pub fn new(memory: Arc<Memory>, _cache: Arc<Cache>, _local_player_name: String) -> Self {
        Self { _memory: memory }
    }

    pub fn update(&mut self, _config: &HitboxConfig, _players_instance: &Instance) {}
    pub fn restore_all_hitboxes(&mut self) {}

    #[allow(dead_code)]
    pub fn get_stats(&self) -> (usize, bool) { (0, false) }
}

impl Drop for HitboxExpander {
    fn drop(&mut self) {}
}
