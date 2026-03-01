use std::sync::Arc;
use dashmap::DashMap;
use crate::core::Memory;
use crate::sdk::Instance;
use crate::utils::cache::Entity;
use crate::utils::math::Vector3;

pub fn reset_dump() {}

pub fn scan_players(
    _cached_players: &Arc<DashMap<u64, Entity>>,
    _workspace: &Arc<Instance>,
    _players_service: &Arc<Instance>,
    _memory: &Arc<Memory>,
    _local_player_name: &str,
    _local_display_name: &str,
    _local_team_identifier: &Arc<std::sync::RwLock<String>>,
    _needs_rotation: bool,
) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
    Vec::new()
}
