#![allow(dead_code)]

pub mod phantom_forces;
pub mod blade_ball;
pub mod fallen;
pub mod aftermath;
pub mod operation_one;
pub mod blox_strike;

use std::sync::Arc;
use crate::core::Memory;
use crate::sdk::Instance;

/// Known game IDs for automatic detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum GameId {
    Unknown = 0,
    PhantomForces = 292439477,
    BadBusiness = 3233893879,
    Rivals = 6035872082,
    ApocalypseRising2 = 358276974,
    Criminality = 1494262959,
    DaHood = 2788229376,
    Arsenal = 286090429,
    BladeBall = 13772394625,
    Fallen = 10228136016,
    Aftermath = 15327728308,
    OperationOne = 8307114974,
    BloxStrike = 7633926880,
}

impl GameId {
    /// Detect game ID from place ID
    pub fn from_place_id(place_id: u64) -> Self {
        match place_id {
            292439477 => Self::PhantomForces,
            3233893879 => Self::BadBusiness,
            6035872082 => Self::Rivals,
            358276974 => Self::ApocalypseRising2,
            1494262959 => Self::Criminality,
            2788229376 => Self::DaHood,
            286090429 => Self::Arsenal,
            13772394625 | 15234596844 | 16281300371 => Self::BladeBall,
            10228136016 | 13800717766 => Self::Fallen,
            15327728308 | 112237800564065 => Self::Aftermath,
            8307114974 | 72920620366355 => Self::OperationOne,
            7633926880 | 114234929420007 => Self::BloxStrike,
            _ => Self::Unknown,
        }
    }

    /// Check if this game uses workspace-based player models
    pub fn uses_workspace_players(&self) -> bool {
        matches!(self, Self::PhantomForces | Self::Fallen | Self::OperationOne | Self::BloxStrike)
    }

    /// Check if this game stores NPCs/entities in a workspace folder (e.g., game_assets > Entities)
    pub fn uses_entity_folder(&self) -> bool {
        matches!(self, Self::Aftermath)
    }

    /// Check if this game uses armor system
    pub fn uses_armor(&self) -> bool {
        matches!(self, Self::PhantomForces)
    }

    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown Game",
            Self::PhantomForces => "Phantom Forces",
            Self::BadBusiness => "Bad Business",
            Self::Rivals => "Rivals",
            Self::ApocalypseRising2 => "Apocalypse Rising 2",
            Self::Criminality => "Criminality",
            Self::DaHood => "Da Hood",
            Self::Arsenal => "Arsenal",
            Self::BladeBall => "Blade Ball",
            Self::Fallen => "Fallen",
            Self::Aftermath => "Aftermath",
            Self::OperationOne => "Operation One",
            Self::BloxStrike => "Blox Strike",
        }
    }
}

/// Player model info extracted from game-specific structures
#[derive(Debug, Clone)]
pub struct GamePlayerModel {
    pub model_address: u64,
    pub name: String,
    pub team_identifier: String,  // Team folder name or team color
    pub parts: Vec<GamePartInfo>,
}

/// Part info with game-specific identifiers
#[derive(Debug, Clone)]
pub struct GamePartInfo {
    pub address: u64,
    pub name: String,
    pub is_head: bool,
    pub is_torso: bool,
}

/// Trait for game-specific player detection
pub trait GameSupport: Send + Sync {
    /// Get all player models from the game
    fn get_player_models(
        &self,
        workspace: &Instance,
        memory: &Arc<Memory>,
        local_player_name: &str,
    ) -> Vec<GamePlayerModel>;

    /// Check if a player is on the same team as local player
    fn is_same_team(&self, player: &GamePlayerModel, local_team: &str) -> bool;

    /// Get the game name
    fn game_name(&self) -> &'static str;
}

/// Universal player detection (standard Roblox Players service)
pub struct UniversalGameSupport;

impl GameSupport for UniversalGameSupport {
    fn get_player_models(
        &self,
        _workspace: &Instance,
        _memory: &Arc<Memory>,
        _local_player_name: &str,
    ) -> Vec<GamePlayerModel> {
        // Universal uses standard Players service - handled by main cache
        Vec::new()
    }

    fn is_same_team(&self, _player: &GamePlayerModel, _local_team: &str) -> bool {
        false
    }

    fn game_name(&self) -> &'static str {
        "Universal"
    }
}

/// Get the appropriate game support implementation
pub fn get_game_support(game_id: GameId) -> Box<dyn GameSupport> {
    match game_id {
        GameId::PhantomForces => Box::new(phantom_forces::PhantomForcesSupport::new()),
        GameId::Fallen => Box::new(fallen::FallenSupport::new()),
        GameId::Aftermath => Box::new(aftermath::AftermathSupport::new()),
        _ => Box::new(UniversalGameSupport),
    }
}
