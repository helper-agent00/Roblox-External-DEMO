use std::sync::Arc;
use crate::core::Memory;
use crate::sdk::Instance;
use super::{GameSupport, GamePlayerModel};

pub struct PhantomForcesSupport;

impl PhantomForcesSupport {
    pub fn new() -> Self { Self }
}

impl GameSupport for PhantomForcesSupport {
    fn get_player_models(&self, _workspace: &Instance, _memory: &Arc<Memory>, _local_player_name: &str) -> Vec<GamePlayerModel> {
        Vec::new()
    }

    fn is_same_team(&self, _player: &GamePlayerModel, _local_team: &str) -> bool {
        false
    }

    fn game_name(&self) -> &'static str {
        "Phantom Forces"
    }
}
