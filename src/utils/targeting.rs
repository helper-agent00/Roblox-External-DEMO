use ahash::AHashSet;
use std::sync::Arc;

use crate::config::Config;
use crate::sdk::VisualEngine;
use crate::utils::cache::{BodyPart, Cache, Entity};
use crate::utils::game_support::GameId;
use crate::utils::math::{Vector2, Vector3};
use crate::utils::velocity::is_teammate;

pub struct TargetContext {
    pub snapshot: Arc<Vec<Entity>>,
    pub local_team: u64,
    pub local_team_id: String,
    pub teammate_addresses: AHashSet<u64>,
    pub screen_center: Vector2,
    pub dimensions: Vector2,
    pub game_id: GameId,
}

impl TargetContext {
    pub fn build(
        cache: &Cache,
        visengine: &VisualEngine,
        config: &Config,
        _local_player_name: &str,
    ) -> Option<Self> {
        let dimensions = visengine.get_dimensions();
        if dimensions.x <= 0.0 || dimensions.y <= 0.0 {
            return None;
        }

        let snapshot = cache.get_snapshot();
        let screen_center = Vector2::new(dimensions.x / 2.0, dimensions.y / 2.0);

        let local_team = cache.get_local_team_addr();
        let local_team_id = cache.get_local_team_id();
        let game_id = cache.get_game_id();

        let teammate_whitelist = &config.visuals.teammate_whitelist;
        let teammate_addresses: AHashSet<u64> =
            if config.visuals.team_check && !teammate_whitelist.is_empty() {
                snapshot
                    .iter()
                    .filter(|e| {
                        teammate_whitelist
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(&e.name))
                    })
                    .map(|e| e.model_address)
                    .collect()
            } else {
                AHashSet::new()
            };

        Some(Self {
            snapshot,
            local_team,
            local_team_id,
            teammate_addresses,
            screen_center,
            dimensions,
            game_id,
        })
    }

    /// Check if an entity should be skipped (self, team, dead, no parts).
    #[inline]
    pub fn should_skip(
        &self,
        entity: &Entity,
        local_player_name: &str,
        team_check: bool,
        hide_dead: bool,
    ) -> bool {
        if entity.name.eq_ignore_ascii_case(local_player_name) {
            return true;
        }
        if is_teammate(
            entity,
            team_check,
            self.local_team,
            &self.teammate_addresses,
            &self.local_team_id,
            self.game_id,
        ) {
            return true;
        }
        if hide_dead && entity.is_dead() {
            return true;
        }
        // Skip entities without humanoid UNLESS they're game-specific or have valid body parts
        if entity.humanoid_address == 0 && !entity.is_game_specific && entity.root_part().is_none()
        {
            return true;
        }
        false
    }
}

/// Get a bone position with standard fallback chain.
#[inline]
pub fn get_bone_with_fallback(entity: &Entity, preferred: &str) -> Option<Vector3> {
    let primary = match preferred {
        "Head" => entity.parts.get(&BodyPart::Head),
        "Torso" | "UpperTorso" => entity
            .parts
            .get(&BodyPart::UpperTorso)
            .or_else(|| entity.parts.get(&BodyPart::Torso)),
        "LowerTorso" => entity.parts.get(&BodyPart::LowerTorso),
        "HumanoidRootPart" => entity.parts.get(&BodyPart::HumanoidRootPart),
        other => BodyPart::from_name(other)
            .and_then(|bp| entity.parts.get(&bp)),
    };

    if let Some(part) = primary {
        if part.position.is_valid() && !part.position.is_near_origin(1.0) {
            return Some(part.position);
        }
    }

    // Standard fallback chain
    const FALLBACKS: [BodyPart; 4] = [
        BodyPart::Head,
        BodyPart::UpperTorso,
        BodyPart::Torso,
        BodyPart::HumanoidRootPart,
    ];
    for bp in FALLBACKS {
        if let Some(part) = entity.parts.get(&bp) {
            if part.position.is_valid() && !part.position.is_near_origin(1.0) {
                return Some(part.position);
            }
        }
    }
    None
}

/// Compute target priority score (lower = better).
#[inline]
pub fn compute_priority(entity: &Entity, screen_dist: f32, world_dist: f32) -> f32 {
    let mut priority = screen_dist;

    // Low health = higher priority (finish the kill)
    if entity.max_health > 0.0 {
        let health_pct = entity.health / entity.max_health;
        priority *= 0.3 + health_pct * 0.7;
    }

    // Close targets are more threatening
    if world_dist < 50.0 {
        let threat_bonus = 1.0 - (world_dist / 50.0) * 0.2;
        priority *= threat_bonus;
    }

    priority
}
