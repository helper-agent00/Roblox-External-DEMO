use std::sync::Arc;
use dashmap::DashMap;
use crate::core::Memory;
use crate::sdk::Instance;
use crate::utils::cache::{BodyPart, Entity};
use crate::utils::math::Vector3;

pub fn reset_dump() {}

pub fn part_name_to_body_part(_name: &str) -> Option<BodyPart> {
    None
}

pub fn scan_players(
    _cached_players: &Arc<DashMap<u64, Entity>>,
    _workspace: &Arc<Instance>,
    _players_service: &Arc<Instance>,
    _memory: &Arc<Memory>,
    _local_player_name: &str,
    _local_display_name: &str,
    _needs_rotation: bool,
    _show_gadgets: bool,
) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
    Vec::new()
}
