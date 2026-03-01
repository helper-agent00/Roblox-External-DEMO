#![allow(dead_code)]

use ahash::AHashSet;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use rayon::prelude::*;
use smallvec::SmallVec;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::humanoid;
use crate::sdk::{Humanoid, Instance, Part, Player};
use crate::utils::math::{Matrix3, Vector3};
use crate::utils::game_support::GameId;

const DEFAULT_UPDATE_INTERVAL_MS: u64 = 12; // ~83Hz base rate (smooth enough for ESP)
const HIGH_PRIORITY_INTERVAL_MS: u64 = 4;   // ~250Hz when aim key held - responsive without hammering CPU
const VELOCITY_SAMPLES: usize = 8;          // More samples for ultra-smooth velocity
const VELOCITY_SMOOTHING: f32 = 0.85;       // Higher = more responsive to velocity changes

pub const DEAD_HEALTH_THRESHOLD: f32 = 1.0;
const STALE_MODEL_THRESHOLD: usize = 5;
const CACHE_CLEAR_COOLDOWN_SECS: u64 = 3;
const POSITION_STALE_CHECK_CYCLES: u32 = 10;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPart {
    Head,
    Torso,
    HumanoidRootPart,
    UpperTorso,
    LowerTorso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
}

impl BodyPart {
    #[inline]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Head" => Some(Self::Head),
            "Torso" => Some(Self::Torso),
            "HumanoidRootPart" => Some(Self::HumanoidRootPart),
            "UpperTorso" => Some(Self::UpperTorso),
            "LowerTorso" => Some(Self::LowerTorso),
            "Left Arm" => Some(Self::LeftArm),
            "Right Arm" => Some(Self::RightArm),
            "Left Leg" => Some(Self::LeftLeg),
            "Right Leg" => Some(Self::RightLeg),
            "LeftUpperArm" => Some(Self::LeftUpperArm),
            "LeftLowerArm" => Some(Self::LeftLowerArm),
            "LeftHand" => Some(Self::LeftHand),
            "RightUpperArm" => Some(Self::RightUpperArm),
            "RightLowerArm" => Some(Self::RightLowerArm),
            "RightHand" => Some(Self::RightHand),
            "LeftUpperLeg" => Some(Self::LeftUpperLeg),
            "LeftLowerLeg" => Some(Self::LeftLowerLeg),
            "LeftFoot" => Some(Self::LeftFoot),
            "RightUpperLeg" => Some(Self::RightUpperLeg),
            "RightLowerLeg" => Some(Self::RightLowerLeg),
            "RightFoot" => Some(Self::RightFoot),
            // Operation One collision parts are handled by the dedicated
            // operation_one::part_name_to_body_part mapper — NOT here,
            // so the standard scan / NPC scanner don't accidentally
            // pick up Op1-style collision models.
            _ => None,
        }
    }
    
    #[inline]
    pub const fn to_name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Torso => "Torso",
            Self::HumanoidRootPart => "HumanoidRootPart",
            Self::UpperTorso => "UpperTorso",
            Self::LowerTorso => "LowerTorso",
            Self::LeftArm => "Left Arm",
            Self::RightArm => "Right Arm",
            Self::LeftLeg => "Left Leg",
            Self::RightLeg => "Right Leg",
            Self::LeftUpperArm => "LeftUpperArm",
            Self::LeftLowerArm => "LeftLowerArm",
            Self::LeftHand => "LeftHand",
            Self::RightUpperArm => "RightUpperArm",
            Self::RightLowerArm => "RightLowerArm",
            Self::RightHand => "RightHand",
            Self::LeftUpperLeg => "LeftUpperLeg",
            Self::LeftLowerLeg => "LeftLowerLeg",
            Self::LeftFoot => "LeftFoot",
            Self::RightUpperLeg => "RightUpperLeg",
            Self::RightLowerLeg => "RightLowerLeg",
            Self::RightFoot => "RightFoot",
        }
    }
    
    pub const ALL: [BodyPart; 21] = [
        Self::Head, Self::Torso, Self::HumanoidRootPart,
        Self::UpperTorso, Self::LowerTorso,
        Self::LeftArm, Self::RightArm, Self::LeftLeg, Self::RightLeg,
        Self::LeftUpperArm, Self::LeftLowerArm, Self::LeftHand,
        Self::RightUpperArm, Self::RightLowerArm, Self::RightHand,
        Self::LeftUpperLeg, Self::LeftLowerLeg, Self::LeftFoot,
        Self::RightUpperLeg, Self::RightLowerLeg, Self::RightFoot,
    ];

    /// Visible body parts for R15 rigs (excludes HumanoidRootPart which is invisible).
    pub const VISIBLE_R15: &'static [BodyPart] = &[
        Self::Head,
        Self::UpperTorso, Self::LowerTorso,
        Self::LeftUpperArm, Self::RightUpperArm,
        Self::LeftLowerArm, Self::RightLowerArm,
        Self::LeftHand, Self::RightHand,
        Self::LeftUpperLeg, Self::RightUpperLeg,
        Self::LeftLowerLeg, Self::RightLowerLeg,
        Self::LeftFoot, Self::RightFoot,
    ];

    /// Visible body parts for R6 rigs.
    pub const VISIBLE_R6: &'static [BodyPart] = &[
        Self::Head, Self::Torso,
        Self::LeftArm, Self::RightArm,
        Self::LeftLeg, Self::RightLeg,
    ];
}

#[derive(Clone)]
pub struct Entity {
    pub model_address: u64,
    pub name: String,
    pub rig_type: u8,
    pub humanoid_address: u64,
    pub team_address: u64,  // Team object address - same address = same team
    pub team_identifier: String, // Game-specific team identifier (e.g., folder name in PF)
    pub body_effects_address: u64, // BodyEffects folder address (for armor, K.O, etc.)
    pub armor_value_address: u64,   // Cached Armor NumberValue address (avoids find_first_child per tick)
    pub parts: ahash::AHashMap<BodyPart, PartData>,
    pub velocity: Vector3,
    pub health: f32,
    pub max_health: f32,
    pub armor: f32,     // Current armor value (0 if not available)
    pub max_armor: f32, // Max armor value (0 if not available)
    pub has_teammate_label: bool, // Rivals-style team check: HumanoidRootPart has TeammateLabel child
    pub is_game_specific: bool, // True if detected via game-specific logic (e.g., PF workspace players)
}

impl Entity {
    #[inline]
    pub fn is_dead(&self) -> bool {
        self.health < DEAD_HEALTH_THRESHOLD || self.health.is_nan() || self.health.is_infinite()
    }

    /// Fallback chain: HRP → UpperTorso → Torso (works for R15 and R6).
    #[inline]
    pub fn root_part(&self) -> Option<&PartData> {
        self.parts.get(&BodyPart::HumanoidRootPart)
            .or_else(|| self.parts.get(&BodyPart::UpperTorso))
            .or_else(|| self.parts.get(&BodyPart::Torso))
    }

    #[inline]
    pub fn root_position(&self) -> Option<Vector3> {
        self.root_part().map(|p| p.position)
    }
}

#[derive(Clone)]
pub struct PartData {
    pub address: u64,
    pub primitive_address: u64,
    pub size: Vector3,
    pub position: Vector3,
    pub rotation: Matrix3,
}

#[derive(Clone)]
struct PositionSample {
    position: Vector3,
    timestamp: Instant,
}


type PositionHistoryVec = SmallVec<[PositionSample; VELOCITY_SAMPLES]>;

pub struct Cache {
    cached_players: Arc<DashMap<u64, Entity>>,
    position_history: Arc<DashMap<u64, PositionHistoryVec>>,
    smoothed_velocities: Arc<DashMap<u64, Vector3>>,
    update_interval_ms: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    show_bots: Arc<AtomicBool>,
    high_priority: Arc<AtomicBool>,
    game_id: Arc<AtomicU64>,
    /// Local player's team identifier for game-specific team checking (e.g., PF team folder name)
    local_team_identifier: Arc<std::sync::RwLock<String>>,
    /// Pre-built read-only snapshot for consumers (avoids per-call DashMap cloning).
    /// Uses ArcSwap for lock-free reads — consumers never block, even when the
    /// cache thread is publishing a new snapshot.
    snapshot_cache: Arc<ArcSwap<Vec<Entity>>>,
    /// Whether any consumer needs rotation data (e.g. Chams).
    needs_rotation: Arc<AtomicBool>,
    /// When true, the cache thread sleeps instead of reading game memory.
    paused: Arc<AtomicBool>,
    /// Local player's Roblox Teams team_address, read directly from Players.LocalPlayer.
    /// This avoids the fragile snapshot-by-name lookup that fails when config username != entity name.
    local_team_address: Arc<AtomicU64>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cached_players: Arc::new(DashMap::new()),
            position_history: Arc::new(DashMap::new()),
            smoothed_velocities: Arc::new(DashMap::new()),
            update_interval_ms: Arc::new(AtomicU64::new(DEFAULT_UPDATE_INTERVAL_MS)),
            running: Arc::new(AtomicBool::new(false)),
            show_bots: Arc::new(AtomicBool::new(false)),
            high_priority: Arc::new(AtomicBool::new(false)),
            game_id: Arc::new(AtomicU64::new(0)),
            local_team_identifier: Arc::new(std::sync::RwLock::new(String::new())),
            snapshot_cache: Arc::new(ArcSwap::from_pointee(Vec::new())),
            needs_rotation: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            local_team_address: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Toggle rotation data collection.
    pub fn set_needs_rotation(&self, needs: bool) {
        self.needs_rotation.store(needs, Ordering::Relaxed);
    }

    /// Pause/resume memory scanning.
    pub fn set_paused(&self, paused: bool) {
        let was_paused = self.paused.swap(paused, Ordering::SeqCst);
        if paused != was_paused {
            if paused {
                tracing::info!("[Cache] Paused - no memory reads");
            } else {
                tracing::info!("[Cache] Resumed - scanning active");
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_local_team_identifier(&self, team: String) {
        if let Ok(mut guard) = self.local_team_identifier.write() {
            if *guard != team {
                tracing::info!("Local team changed: '{}' -> '{}'", *guard, team);
                *guard = team;
            }
        }
    }

    pub fn get_local_team_id(&self) -> String {
        self.local_team_identifier.read().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn set_game_id(&self, game_id: GameId) {
        self.game_id.store(game_id as u64, Ordering::Relaxed);
    }

    pub fn get_game_id(&self) -> GameId {
        GameId::from_place_id(self.game_id.load(Ordering::Relaxed))
    }

    pub fn set_high_priority(&self, enabled: bool) {
        self.high_priority.store(enabled, Ordering::Relaxed);
    }
    pub fn set_show_bots(&self, enabled: bool) {
        self.show_bots.store(enabled, Ordering::Relaxed);
    }
    pub fn set_update_rate(&self, ms: u64) {
        self.update_interval_ms.store(ms.max(16).min(200), Ordering::Relaxed);
    }
    pub fn clear(&self) {
        self.cached_players.clear();
        self.position_history.clear();
        self.smoothed_velocities.clear();
        self.snapshot_cache.store(Arc::new(Vec::new()));
    }
    pub fn count(&self) -> usize {
        self.cached_players.len()
    }

    pub fn get_snapshot(&self) -> Arc<Vec<Entity>> {
        self.snapshot_cache.load_full()
    }

    pub fn get_local_team_addr(&self) -> u64 {
        self.local_team_address.load(Ordering::Relaxed)
    }

    pub fn get_local_team_address(&self, _local_player_name: &str) -> u64 {
        // Use the stored value from Players.LocalPlayer instead of fragile name-based lookup
        self.local_team_address.load(Ordering::Relaxed)
    }

    /// Uses stored value since local player is excluded from cache.
    pub fn get_local_team_identifier(&self, _local_player_name: &str) -> String {
        self.get_local_team_id()
    }

    /// Convenience wrapper — delegates to `velocity::is_teammate()`.
    pub fn is_same_team(&self, entity: &Entity, _local_player_name: &str) -> bool {
        let game_id = self.get_game_id();
        let local_team_addr = self.get_local_team_addr();
        // Delegate to the same logic that features use
        crate::utils::velocity::is_teammate(
            entity,
            true, // team_check_enabled
            local_team_addr,
            &AHashSet::new(), // no whitelist in this context
            &self.get_local_team_id(),
            game_id,
        )
    }

    pub fn start(&self, players_instance: Arc<Instance>, workspace_instance: Arc<Instance>, memory: Arc<Memory>, update_rate_ms: u64) {
        if self.running.swap(true, Ordering::SeqCst) {
            tracing::warn!("Cache thread already running");
            return;
        }

        self.update_interval_ms.store(update_rate_ms.max(4).min(50), Ordering::Relaxed);

        let cached_players = Arc::clone(&self.cached_players);
        let position_history = Arc::clone(&self.position_history);
        let smoothed_velocities = Arc::clone(&self.smoothed_velocities);
        let update_interval_ms = Arc::clone(&self.update_interval_ms);
        let running = Arc::clone(&self.running);
        let show_bots = Arc::clone(&self.show_bots);
        let high_priority = Arc::clone(&self.high_priority);
        let game_id = Arc::clone(&self.game_id);
        let local_team_identifier = Arc::clone(&self.local_team_identifier);
        let snapshot_cache = Arc::clone(&self.snapshot_cache);
        let needs_rotation = Arc::clone(&self.needs_rotation);
        let paused = Arc::clone(&self.paused);
        let local_team_address = Arc::clone(&self.local_team_address);

        thread::spawn(move || {
            let mut first_run = true;
            while running.load(Ordering::Relaxed) {
                // When paused, sleep without doing any memory reads.
                if paused.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }

                let bots_enabled = show_bots.load(Ordering::Relaxed);
                let current_game_id = GameId::from_place_id(game_id.load(Ordering::Relaxed));
                
                if first_run {
                    first_run = false;
                }
                
                let rot_needed = needs_rotation.load(Ordering::Relaxed);
                
                Self::update_cache(&cached_players, &position_history, &smoothed_velocities, &players_instance, &workspace_instance, &memory, bots_enabled, current_game_id, &local_team_identifier, rot_needed, &local_team_address);
                
                // Publish snapshot for consumers (lock-free swap via ArcSwap)
                {
                    let snap: Vec<Entity> = cached_players.iter().map(|r| r.value().clone()).collect();
                    snapshot_cache.store(Arc::new(snap));
                }
                
                // Use high-priority interval when aim key is held for tighter tracking
                let interval = if high_priority.load(Ordering::Relaxed) {
                    HIGH_PRIORITY_INTERVAL_MS
                } else {
                    update_interval_ms.load(Ordering::Relaxed)
                };
                thread::sleep(Duration::from_millis(interval));
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn restart(&self, players_instance: Arc<Instance>, workspace_instance: Arc<Instance>, memory: Arc<Memory>, update_rate_ms: u64) {
        self.stop();
        
        std::thread::sleep(Duration::from_millis(100));
        
        self.clear();
        
        self.running.store(false, Ordering::SeqCst);
        self.start(players_instance, workspace_instance, memory, update_rate_ms);
    }

    fn update_cache(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
        players_instance: &Arc<Instance>,
        workspace_instance: &Arc<Instance>,
        memory: &Arc<Memory>,
        show_bots: bool,
        game_id: GameId,
        local_team_identifier: &Arc<std::sync::RwLock<String>>,
        needs_rotation: bool,
        local_team_address: &Arc<AtomicU64>,
    ) {
        let children = players_instance.get_children();
        let now = Instant::now();

        // Thread-local cache of confirmed "Player" addresses.
        thread_local! {
            static KNOWN_PLAYER_ADDRS: std::cell::RefCell<AHashSet<u64>> = std::cell::RefCell::new(AHashSet::new());
        }

        let player_children: Vec<_> = {
            KNOWN_PLAYER_ADDRS.with(|known| {
                let mut known = known.borrow_mut();
                let child_addrs: AHashSet<u64> = children.iter().map(|c| c.address).collect();
                // Prune addresses no longer in children (player left)
                known.retain(|addr| child_addrs.contains(addr));
                
                children
                    .into_iter()
                    .filter(|child| {
                        // Fast path: already confirmed as Player
                        if known.contains(&child.address) {
                            return true;
                        }
                        // Slow path: check class name
                        if child.get_class_name() == "Player" {
                            known.insert(child.address);
                            true
                        } else {
                            false
                        }
                    })
                    .collect()
            })
        };

        // Read local player's team address directly from Players.LocalPlayer (works for ALL games).
        // This replaces the broken pattern of searching the entity snapshot by config username.
        let lp_addr = memory.read::<u64>(players_instance.address + crate::core::offsets::player::localplayer());
        if lp_addr != 0 && is_valid_address(lp_addr) {
            let lp = Player::new(lp_addr, Arc::clone(memory));
            let team_addr = lp.get_team_address();
            let prev = local_team_address.swap(team_addr, Ordering::Relaxed);
            if prev != team_addr && team_addr != 0 {
                tracing::info!("[Cache] Local player team address updated: {:#x} -> {:#x}", prev, team_addr);
            }
        }

        let mut current_addresses: AHashSet<u64> = player_children.iter().map(|c| c.address).collect();

        let player_model_info: Vec<(u64, String)> = player_children
            .iter()
            .filter_map(|child| {
                let player = Player::new(child.address, Arc::clone(memory));
                let model = player.get_model_instance();
                let model_addr = model.address();
                let name = player.get_name();
                if is_valid_address(model_addr) && !name.is_empty() {
                    Some((model_addr, name))
                } else {
                    None
                }
            })
            .collect();
        
        // Build lookup sets — single pass, avoid double-clone of names
        let mut player_model_addresses = AHashSet::with_capacity(player_model_info.len());
        let mut player_names = AHashSet::with_capacity(player_model_info.len());
        for (addr, name) in &player_model_info {
            player_model_addresses.insert(*addr);
            player_names.insert(name.clone());
        }

        let updates: Vec<(u64, Option<Entity>, Option<(u64, Vector3)>, bool)> = {
            player_children
            .par_iter()
            .filter_map(|child| {
                // Games with workspace-based player scanning (PF, Op1, Blox Strike, Fallen)
                // build properly-tagged entities through their game-specific scanner.
                // Skip standard entity creation to prevent duplicates with missing/wrong
                // team data that overwrite the correct game-specific team colours.
                if game_id.uses_workspace_players() {
                    return None;
                }

                let player = Player::new(child.address, Arc::clone(memory));
                let model = player.get_model_instance();
                let model_addr = model.address();

                if !is_valid_address(model_addr) {
                    let was_cached = cached_players.contains_key(&child.address);
                    return Some((child.address, None, None, was_cached));
                }

                let existing_check = cached_players
                    .get(&child.address)
                    .map(|r| (r.model_address == model_addr, r.clone()));

                if let Some((same_model, mut existing)) = existing_check {
                    if same_model {
                        let is_valid = Self::update_entity_positions(&mut existing, &model, memory, needs_rotation);
                        if !is_valid {
                            // Humanoid destroyed = character died/despawned.
                            // Mark entity as dead so ESP hides it immediately
                            // instead of showing stale last-known-alive state.
                            existing.health = 0.0;
                            return Some((child.address, Some(existing), None, true));
                        }
                        // Update team address (basic Roblox Teams check)
                        existing.team_address = player.get_team_address();
                        let root_pos = existing.root_position();
                        return Some((child.address, Some(existing), root_pos.map(|p| (child.address, p)), false));
                    } else {
                        if let Some(entity) = Self::build_entity(&player, &model, memory) {
                            let root_pos = entity.root_position();
                            return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p)), false));
                        } else {
                            return Some((child.address, None, None, true));
                        }
                    }
                }

                // Build new entity (no existing cache entry)
                if let Some(entity) = Self::build_entity(&player, &model, memory) {
                    let root_pos = entity.root_position();
                    return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p)), false));
                }

                None
            })
            .collect()
        };

        // Count stale entities (model changed but couldn't rebuild)
        let stale_count = updates.iter().filter(|(_, _, _, is_stale)| *is_stale).count();
        
        static LAST_CACHE_CLEAR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let current_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last_clear = LAST_CACHE_CLEAR.load(std::sync::atomic::Ordering::Relaxed);
        
        if stale_count >= STALE_MODEL_THRESHOLD && (current_secs - last_clear) >= CACHE_CLEAR_COOLDOWN_SECS {
            tracing::info!("Round transition detected ({} stale entities) - selective cache cleanup", stale_count);
            // Selective cleanup: only remove entities that are truly stale (no valid player anymore)
            // Keep position_history and smoothed_velocities for smoother re-acquisition
            let stale_addresses: Vec<u64> = cached_players
                .iter()
                .filter(|entry| {
                    let entity = entry.value();
                    // Game-specific entities (like PF) have humanoid now but use position-based
                    // staleness checks since their models may be re-created differently across rounds
                    if entity.is_game_specific {
                        // For PF entities, only remove if all parts are at origin
                        entity.parts.values().all(|p| p.position.is_near_origin(5.0))
                    } else {
                        // Entity is stale if humanoid is invalid or all parts are at origin
                        !is_valid_address(entity.humanoid_address) || 
                        entity.parts.values().all(|p| p.position.is_near_origin(5.0))
                    }
                })
                .map(|entry| *entry.key())
                .collect();
            
            for addr in stale_addresses {
                cached_players.remove(&addr);
            }
            LAST_CACHE_CLEAR.store(current_secs, std::sync::atomic::Ordering::Relaxed);
            // Don't return early - continue to rebuild entities
        }

        let npc_updates: Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> = if show_bots || game_id.uses_entity_folder() {
            // Entity-folder games (Aftermath) ALWAYS scan workspace — players AND NPCs
            // live here since Player.Character is not used
            Self::scan_workspace_for_npcs(cached_players, workspace_instance, memory, &player_model_addresses, &player_names, game_id, needs_rotation)
        } else {
            Vec::new()
        };

        // Game-specific player scanning (e.g., Phantom Forces uses Workspace.Players instead of Players service)
        let game_specific_updates: Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> = 
            if game_id.uses_workspace_players() {
                // Get local player info using LocalPlayer property from Players service
                // This is more reliable than picking the first child
                let local_player_addr = memory.read::<u64>(players_instance.address + crate::core::offsets::player::localplayer());
                
                let (local_player_name, local_display_name, local_team_addr) = if local_player_addr != 0 && is_valid_address(local_player_addr) {
                    let player = Player::new(local_player_addr, Arc::clone(memory));
                    (player.get_name(), player.get_display_name(), player.get_team_address())
                } else {
                    // Fallback to first child if LocalPlayer not accessible
                    player_children
                        .first()
                        .map(|p| {
                            let player = Player::new(p.address, Arc::clone(memory));
                            (player.get_name(), player.get_display_name(), player.get_team_address())
                        })
                        .unwrap_or_default()
                };
                
                // Log once at startup
                static LOGGED_LOCAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !LOGGED_LOCAL.load(std::sync::atomic::Ordering::Relaxed) {
                    LOGGED_LOCAL.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                
                Self::scan_game_specific_players(
                    cached_players, 
                    workspace_instance,
                    players_instance,
                    memory, 
                    game_id, 
                    &local_player_name,
                    &local_display_name,
                    local_team_addr,
                    &player_model_addresses,
                    local_team_identifier,
                    needs_rotation,
                    show_bots
                )
            } else {
                Vec::new()
            };

        for (addr, entity_opt, _) in &npc_updates {
            if entity_opt.is_some() {
                current_addresses.insert(*addr);
            }
        }
        
        for (addr, entity_opt, _) in &game_specific_updates {
            if entity_opt.is_some() {
                current_addresses.insert(*addr);
            }
        }

        for (address, entity_opt, _, _was_stale) in &updates {
            if let Some(entity) = entity_opt {
                cached_players.insert(*address, entity.clone());
            }
            // Note: do NOT remove stale entities here. When a character dies/respawns,
            // the model is briefly invalid. Removing the entity loses teammate data
            // (has_teammate_label, team_address) that takes time for the server to
            // re-apply on the new model. cleanup_disconnected handles true departures.
        }
        for (address, entity_opt, _) in &npc_updates {
            if let Some(entity) = entity_opt {
                cached_players.insert(*address, entity.clone());
            }
        }
        for (address, entity_opt, _) in &game_specific_updates {
            if let Some(entity) = entity_opt {
                cached_players.insert(*address, entity.clone());
            }
        }

        // For workspace-player games: remove any leftover non-game-specific
        // (standard Players-service) entities.  The standard par_iter now skips
        // building them, but stale entries from before game detection can linger.
        //
        // NOTE: Do NOT mark game-specific entities as dead here when they're
        // missing from one scan — workspace children lists can be briefly
        // inconsistent, and marking dead on every miss causes ESP flickering.
        // Instead, death is handled naturally:
        //   1. Health read in update_entity_positions → is_dead() when humanoid HP=0
        //   2. cleanup_disconnected removes entities whose address leaves current_addresses
        if game_id.uses_workspace_players() {
            cached_players.retain(|_key, entity| entity.is_game_specific);
        }

        // Collect addresses that had velocity updates this tick (for targeted processing)
        let mut velocity_updated: AHashSet<u64> = AHashSet::new();

        for (address, entity_opt, root_pos_opt, _) in &updates {
            if let (Some(_), Some((_, root_pos))) = (entity_opt, root_pos_opt) {
                Self::update_velocity_history(position_history, *address, *root_pos, now);
                velocity_updated.insert(*address);
            }
        }
        
        for (address, entity_opt, root_pos_opt) in &npc_updates {
            if let (Some(_), Some((_, root_pos))) = (entity_opt, root_pos_opt) {
                Self::update_velocity_history(position_history, *address, *root_pos, now);
                velocity_updated.insert(*address);
            }
        }
        
        for (address, entity_opt, root_pos_opt) in &game_specific_updates {
            if let (Some(_), Some((_, root_pos))) = (entity_opt, root_pos_opt) {
                Self::update_velocity_history(position_history, *address, *root_pos, now);
                velocity_updated.insert(*address);
            }
        }

        {
        // Only compute velocities for entities that had position updates this tick.
        // Targeted DashMap gets (O(1) per entity) instead of full iteration (all shards).
        Self::apply_velocities_from_history(cached_players, position_history, smoothed_velocities, &velocity_updated);

        // Merge workspace collision parts for Op1-style games where Player.Character
        // has only HumanoidRootPart and the visible model with body parts is a
        // separate Model in workspace with the same player name.
        // Rate-limited: only runs every 5th tick to avoid excessive workspace scanning.
        static WS_MERGE_TICKER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if WS_MERGE_TICKER.fetch_add(1, Ordering::Relaxed) % 5 == 0 {
            Self::merge_workspace_body_parts(cached_players, workspace_instance, memory);
        }

        // Rate-limit cleanup: players don't join/leave frequently.
        // Running every 10th tick is sufficient and avoids 3x DashMap retain() per tick.
        static CLEANUP_TICKER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if CLEANUP_TICKER.fetch_add(1, Ordering::Relaxed) % 10 == 0 {
            Self::cleanup_disconnected(cached_players, position_history, smoothed_velocities, &current_addresses);
        }
        } // end cache_velocity_cleanup scope
    }

    fn scan_workspace_for_npcs(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        memory: &Arc<Memory>,
        player_model_addresses: &AHashSet<u64>,
        player_names: &AHashSet<String>,
        game_id: GameId,
        needs_rotation: bool,
    ) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
        let mut models_to_check: Vec<Instance> = Vec::new();
        Self::collect_models_recursive(workspace, memory, &mut models_to_check, 3); // Max depth of 3
        
        static LAST_NPC_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let last = LAST_NPC_LOG.load(std::sync::atomic::Ordering::Relaxed);
        if current_time - last >= 5 {

            LAST_NPC_LOG.store(current_time, std::sync::atomic::Ordering::Relaxed);
        }

        let is_entity_folder_game = game_id.uses_entity_folder();

        models_to_check
            .par_iter()
            .filter_map(|child| {
                if player_model_addresses.contains(&child.address) {
                    return None;
                }

                let model_name = child.get_name();
                if player_names.contains(&model_name) {
                    return None;
                }

                let model = crate::sdk::ModelInstance::new(child.address, Arc::clone(memory));

                // For entity-folder games (Aftermath), don't require a "Humanoid" child.
                // Player entities may use AnimationController with a different name,
                // or have no animation controller at all. Just check for body parts.
                if !is_entity_folder_game {
                    let humanoid = match model.find_first_child("Humanoid") {
                        Some(h) => h,
                        None => return None,
                    };
                    let humanoid_class = humanoid.get_class_name();
                    if humanoid_class != "Humanoid" {
                        return None;
                    }
                }

                let model_addr = model.address();
                if !is_valid_address(model_addr) {
                    return None;
                }

                // Check if existing NPC needs position update only (lock-free with DashMap)
                let existing_check = cached_players
                    .get(&child.address)
                    .map(|r| (r.model_address == model_addr, r.clone()));

                if let Some((same_model, mut existing)) = existing_check {
                    if same_model {
                        let is_valid = Self::update_entity_positions(&mut existing, &model, memory, needs_rotation);
                        if !is_valid {
                            // Humanoid destroyed = NPC died/despawned.
                            // Mark as dead so ESP hides it immediately instead
                            // of dropping the entity (which causes flicker when
                            // it gets re-built next tick with stale health).
                            existing.health = 0.0;
                            return Some((child.address, Some(existing), None));
                        }
                        let root_pos = existing.root_position();
                        // Filter dead/despawned entities (sent to Y=1B)
                        if let Some(pos) = root_pos {
                            if pos.y.abs() > 100_000.0 {
                                return None;
                            }
                        }
                        return Some((child.address, Some(existing), root_pos.map(|p| (child.address, p))));
                    }
                }

                // Build new NPC entity
                if let Some(mut entity) = Self::build_npc_entity(&model, memory) {
                    // For entity-folder games, label entities based on rig type:
                    // R15 (full skeleton) = Player, R6 (partial) = Zombie
                    if is_entity_folder_game && entity.name.starts_with('{') {
                        let short_id = entity.name.trim_start_matches('{')
                            .split('-').next().unwrap_or("???")
                            .chars().take(4).collect::<String>();
                        entity.name = if entity.rig_type == 1 {
                            format!("Player {}", short_id)
                        } else {
                            format!("Zombie {}", short_id)
                        };
                    }
                    let root_pos = entity.root_position();
                    // Filter dead/despawned entities (sent to Y=1B)
                    if let Some(pos) = root_pos {
                        if pos.y.abs() > 100_000.0 {
                            return None;
                        }
                    }
                    return Some((child.address, Some(entity), root_pos.map(|p| (child.address, p))));
                }

                None
            })
            .collect()
    }

    /// Scan for game-specific player models (e.g., Phantom Forces workspace-based players)
    fn scan_game_specific_players(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        players_service: &Arc<Instance>,
        memory: &Arc<Memory>,
        game_id: GameId,
        local_player_name: &str,
        local_display_name: &str,
        local_team_addr: u64,
        player_model_addresses: &AHashSet<u64>,
        local_team_identifier: &Arc<std::sync::RwLock<String>>,
        needs_rotation: bool,
        show_bots: bool,
    ) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
        match game_id {
            GameId::PhantomForces => {
                Self::scan_phantom_forces_players(
                    cached_players, workspace, players_service, memory, 
                    local_player_name, local_display_name, local_team_addr,
                    player_model_addresses, local_team_identifier, needs_rotation
                )
            }
            GameId::Fallen => {
                Self::scan_fallen_players(
                    cached_players, workspace, memory,
                    local_player_name, local_display_name,
                    player_model_addresses, needs_rotation
                )
            }
            GameId::OperationOne => {
                crate::utils::game_support::operation_one::scan_players(
                    cached_players, workspace, players_service, memory,
                    local_player_name, local_display_name, needs_rotation, show_bots
                )
            }
            GameId::BloxStrike => {
                crate::utils::game_support::blox_strike::scan_players(
                    cached_players, workspace, players_service, memory,
                    local_player_name, local_display_name, local_team_identifier, needs_rotation
                )
            }
            _ => Vec::new(),
        }
    }

    /// Scan for Phantom Forces players in Workspace.Players.{team}.{player_model}
    fn scan_phantom_forces_players(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        _players_service: &Arc<Instance>,
        memory: &Arc<Memory>,
        local_player_name: &str,
        local_display_name: &str,
        _local_team_addr: u64,
        player_model_addresses: &AHashSet<u64>,
        _local_team_identifier: &Arc<std::sync::RwLock<String>>,
        needs_rotation: bool,
    ) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
        let players_folder = match workspace.find_first_child("Players") {
            Some(f) => f,
            None => {
                return Vec::new();
            }
        };

        let mut results = Vec::new();
        let team_folders = players_folder.get_children();

        // Build entities for all players in Workspace.Players folders
        // Teammate detection: Check PlayerTag.TextColor3 - BLUE = teammate, RED = enemy
        for team_folder in team_folders {
            let player_models = team_folder.get_children();

            for player_model in player_models {
                let model_class = player_model.get_class_name();
                if model_class != "Model" {
                    continue;
                }

                // Skip if already tracked as a regular player
                if player_model_addresses.contains(&player_model.address) {
                    continue;
                }

                let model = crate::sdk::ModelInstance::new(player_model.address, Arc::clone(memory));
                let model_addr = model.address();
                
                if !is_valid_address(model_addr) {
                    continue;
                }

                // Check if existing entity needs position update only
                let existing_check = cached_players
                    .get(&player_model.address)
                    .map(|r| (r.model_address == model_addr, r.clone()));

                if let Some((same_model, mut existing)) = existing_check {
                    if same_model {
                        let is_valid = Self::update_entity_positions(&mut existing, &model, memory, needs_rotation);
                        if !is_valid {
                            // Humanoid destroyed = character died/despawned.
                            // Mark entity as dead so ESP hides it immediately
                            // instead of showing stale last-known-alive state.
                            existing.health = 0.0;
                            results.push((player_model.address, Some(existing), None));
                            continue;
                        }
                        let root_pos = existing.parts.get(&BodyPart::HumanoidRootPart)
                            .or_else(|| existing.parts.get(&BodyPart::Head))
                            .or_else(|| existing.parts.get(&BodyPart::Torso))
                            .map(|p| p.position);
                        results.push((player_model.address, Some(existing), root_pos.map(|p| (player_model.address, p))));
                        continue;
                    }
                }

                // Build new PF entity - uses color-based team detection
                if let Some(entity) = Self::build_phantom_forces_entity(&model, memory, local_player_name, local_display_name) {
                    let root_pos = entity.parts.get(&BodyPart::HumanoidRootPart)
                        .or_else(|| entity.parts.get(&BodyPart::Head))
                        .or_else(|| entity.parts.get(&BodyPart::Torso))
                        .map(|p| p.position);
                    results.push((player_model.address, Some(entity), root_pos.map(|p| (player_model.address, p))));
                }
            }
        }

        results
    }

    /// Build PF entity. Detects teammates via PlayerTag.TextColor3 (blue = teammate, red = enemy).
    fn build_phantom_forces_entity(
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
        local_player_name: &str,
        local_display_name: &str,
    ) -> Option<Entity> {
        let model_children = model.get_children();
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();
        let mut player_name = String::new();
        let mut found_head = false;
        let mut found_torso = false;
        let mut is_teammate = false;
        let mut humanoid_address: u64 = 0;

        // Sequential limb slots for unmapped SpecialMesh parts (PF obfuscates names)
        const LIMB_SEQUENCE: &[BodyPart] = &[
            BodyPart::LeftArm, BodyPart::RightArm,
            BodyPart::LeftLeg, BodyPart::RightLeg,
            BodyPart::LeftUpperArm, BodyPart::RightUpperArm,
        ];
        let mut limb_index: usize = 0;

        for child in model_children {
            let child_class = child.get_class_name();
            let child_name = child.get_name();

            // Capture Humanoid for health reads (PF models do have one)
            if child_class == "Humanoid" {
                humanoid_address = child.address;
                continue;
            }
            
            if !child_class.contains("Part") {
                continue;
            }

            let special_mesh = child.find_first_child_by_class("SpecialMesh");
            let has_special_mesh = special_mesh.is_some();
            let has_billboard = child.find_first_child_by_class("BillboardGui");
            let has_spotlight = child.find_first_child_by_class("SpotLight").is_some();

            if has_special_mesh && has_billboard.is_some() {
                // HEAD - has BillboardGui with PlayerTag
                if let Some(billboard) = has_billboard {
                    if let Some(player_tag) = billboard.find_first_child("PlayerTag") {
                        // Get player name
                        let text_offset = crate::core::offsets::gui_object::text();
                        let name = memory.read_string(player_tag.address + text_offset);
                        if !name.is_empty() {
                            player_name = name.clone();
                        }
                        
                        // COLOR-BASED TEAM DETECTION:
                        // Read TextColor3 (RGB floats) from PlayerTag
                        // BLUE/CYAN tag = teammate, RED tag = enemy
                        let text_color_offset = crate::core::offsets::gui_object::text_color3();
                        let r: f32 = memory.read(player_tag.address + text_color_offset);
                        let b: f32 = memory.read(player_tag.address + text_color_offset + 8);
                        
                        // Teammate: high blue/green, low red (cyan/blue tags)
                        // Enemy: high red, low blue (red tags)
                        if b > 0.5 && r < 0.5 {
                            is_teammate = true;
                        }
                    }
                }
                
                let part = Part::new(child.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();
                
                if position.is_valid() {
                    let mut size = primitive.get_size();
                    // PF parts may have tiny physics sizes — use SpecialMesh.Scale as fallback
                    if size.x < 0.1 || size.y < 0.1 || size.z < 0.1 {
                        if let Some(ref mesh) = special_mesh {
                            let scale: Vector3 = memory.read(mesh.address + crate::core::offsets::special_mesh::scale());
                            if scale.x > 0.1 && scale.y > 0.1 && scale.z > 0.1 {
                                size = scale;
                            } else {
                                size = Vector3::new(1.2, 1.2, 1.2); // default head size
                            }
                        }
                    }
                    parts.insert(BodyPart::Head, PartData {
                        address: child.address,
                        primitive_address: primitive.address,
                        size,
                        position,
                        rotation: primitive.get_rotation(),
                    });
                    found_head = true;
                }
            } else if has_special_mesh && has_spotlight {
                // TORSO - has SpotLight
                let part = Part::new(child.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();
                
                if position.is_valid() {
                    let mut size = primitive.get_size();
                    if size.x < 0.1 || size.y < 0.1 || size.z < 0.1 {
                        if let Some(ref mesh) = special_mesh {
                            let scale: Vector3 = memory.read(mesh.address + crate::core::offsets::special_mesh::scale());
                            if scale.x > 0.1 && scale.y > 0.1 && scale.z > 0.1 {
                                size = scale;
                            } else {
                                size = Vector3::new(2.0, 2.0, 1.0); // default torso size
                            }
                        }
                    }
                    parts.insert(BodyPart::Torso, PartData {
                        address: child.address,
                        primitive_address: primitive.address,
                        size,
                        position,
                        rotation: primitive.get_rotation(),
                    });
                    found_torso = true;
                }
            } else if has_special_mesh {
                // Other body parts — PF names are obfuscated so BodyPart::from_name fails.
                // Assign to sequential limb slots like the reference C++ impl ("Other_N").
                let body_part = BodyPart::from_name(&child_name).unwrap_or_else(|| {
                    let idx = limb_index.min(LIMB_SEQUENCE.len() - 1);
                    limb_index += 1;
                    LIMB_SEQUENCE[idx]
                });
                // Don't overwrite head/torso if a collision happens
                if !parts.contains_key(&body_part) {
                    let part = Part::new(child.address, Arc::clone(memory));
                    let primitive = part.get_primitive();
                    let position = primitive.get_position();
                    
                    if position.is_valid() {
                        let mut size = primitive.get_size();
                        if size.x < 0.1 || size.y < 0.1 || size.z < 0.1 {
                            if let Some(ref mesh) = special_mesh {
                                let scale: Vector3 = memory.read(mesh.address + crate::core::offsets::special_mesh::scale());
                                if scale.x > 0.1 && scale.y > 0.1 && scale.z > 0.1 {
                                    size = scale;
                                } else {
                                    size = Vector3::new(1.0, 1.5, 1.0); // default limb size
                                }
                            }
                        }
                        parts.insert(body_part, PartData {
                            address: child.address,
                            primitive_address: primitive.address,
                            size,
                            position,
                            rotation: primitive.get_rotation(),
                        });
                    }
                }
            }
        }

        // Need head or torso
        if !found_head && !found_torso && parts.is_empty() {
            return None;
        }

        // Synthesize HumanoidRootPart from Torso/Head for root_part() consistency
        if !parts.contains_key(&BodyPart::HumanoidRootPart) {
            if let Some(fallback) = parts.get(&BodyPart::Torso).or_else(|| parts.get(&BodyPart::Head)) {
                let fb = fallback.clone();
                parts.insert(BodyPart::HumanoidRootPart, fb);
            }
        }

        // Skip local player
        if !player_name.is_empty() {
            if player_name.eq_ignore_ascii_case(local_player_name) || 
               player_name.eq_ignore_ascii_case(local_display_name) {
                return None;
            }
        }

        // Fallback name from model
        if player_name.is_empty() {
            let model_instance = Instance::new(model.address(), Arc::clone(memory));
            player_name = model_instance.get_name();
        }

        // Read health from Humanoid if found
        let (health, max_health) = if humanoid_address != 0 && is_valid_address(humanoid_address) {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        Some(Entity {
            model_address: model.address(),
            name: player_name,
            rig_type: 1,
            humanoid_address,
            team_address: 0,
            team_identifier: String::new(),
            body_effects_address: 0,
            armor_value_address: 0,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor: 0.0,
            max_armor: 0.0,
            has_teammate_label: is_teammate, // BLUE tag = true (teammate)
            is_game_specific: true,
        })
    }

    fn scan_fallen_players(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        memory: &Arc<Memory>,
        local_player_name: &str,
        local_display_name: &str,
        player_model_addresses: &AHashSet<u64>,
        needs_rotation: bool,
    ) -> Vec<(u64, Option<Entity>, Option<(u64, Vector3)>)> {
        let mut results = Vec::new();
        let workspace_children = workspace.get_children();

        for child in workspace_children {
            if child.get_class_name() != "Model" {
                continue;
            }

            // Skip if already tracked as a regular Players-service player
            if player_model_addresses.contains(&child.address) {
                continue;
            }

            // Quick check: does the model look like a player?
            // Must have Humanoid + NameTag + ≥3 body-part MeshParts
            let model_children = child.get_children();
            let mut has_humanoid = false;
            let mut has_nametag = false;
            let mut body_part_count: u8 = 0;
            for mc in &model_children {
                match mc.get_class_name().as_str() {
                    "Humanoid" => has_humanoid = true,
                    "BillboardGui" if mc.get_name() == "NameTag" => has_nametag = true,
                    "MeshPart" | "Part" => {
                        if BodyPart::from_name(&mc.get_name()).is_some() {
                            body_part_count += 1;
                        }
                    }
                    _ => {}
                }
            }
            if !has_humanoid || !has_nametag || body_part_count < 3 {
                continue;
            }

            let model = crate::sdk::ModelInstance::new(child.address, Arc::clone(memory));
            let model_addr = model.address();
            if !is_valid_address(model_addr) {
                continue;
            }

            // Check if existing entity just needs a position update
            let existing_check = cached_players
                .get(&child.address)
                .map(|r| (r.model_address == model_addr, r.clone()));

            if let Some((same_model, mut existing)) = existing_check {
                if same_model {
                    let is_valid = Self::update_entity_positions(&mut existing, &model, memory, needs_rotation);
                    if !is_valid {
                        // Humanoid destroyed = character died/despawned.
                        // Mark entity as dead so ESP hides it immediately
                        // instead of showing stale last-known-alive state.
                        existing.health = 0.0;
                        results.push((child.address, Some(existing), None));
                        continue;
                    }
                    let root_pos = existing.parts.get(&BodyPart::HumanoidRootPart)
                        .or_else(|| existing.parts.get(&BodyPart::UpperTorso))
                        .or_else(|| existing.parts.get(&BodyPart::Head))
                        .map(|p| p.position);
                    results.push((child.address, Some(existing), root_pos.map(|p| (child.address, p))));
                    continue;
                }
            }

            // Build new Fallen entity — reuse model_children from validation above
            // to avoid a second get_children() call (expensive memory reads)
            if let Some(entity) = Self::build_fallen_entity_from_children(&model_children, model.address(), memory, local_player_name, local_display_name) {
                let root_pos = entity.parts.get(&BodyPart::HumanoidRootPart)
                    .or_else(|| entity.parts.get(&BodyPart::UpperTorso))
                    .or_else(|| entity.parts.get(&BodyPart::Head))
                    .map(|p| p.position);
                results.push((child.address, Some(entity), root_pos.map(|p| (child.address, p))));
            }
        }

        results
    }

    /// Build Fallen entity from pre-fetched children (avoids second get_children() call).
    fn build_fallen_entity_from_children(
        model_children: &[Instance],
        model_addr: u64,
        memory: &Arc<Memory>,
        local_player_name: &str,
        local_display_name: &str,
    ) -> Option<Entity> {
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();
        let mut humanoid_address: u64 = 0;
        let mut rig_type: u8 = 0;

        for child in model_children {
            let class = child.get_class_name();

            // Grab the Humanoid
            if class == "Humanoid" {
                humanoid_address = child.address;
                let hum = Humanoid::new(child.address, Arc::clone(memory));
                rig_type = hum.get_rig_type();
                continue;
            }

            // Collect body-part primitives
            if class != "MeshPart" && class != "Part" {
                continue;
            }

            let part_name = child.get_name();
            if let Some(body_part) = BodyPart::from_name(&part_name) {
                let part = Part::new(child.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();
                if position.is_valid() {
                    parts.insert(body_part, PartData {
                        address: child.address,
                        primitive_address: primitive.address,
                        size: primitive.get_size(),
                        position,
                        rotation: primitive.get_rotation(),
                    });
                }
            }
        }

        if parts.is_empty() {
            return None;
        }

        // Player name = model name (e.g., "any1an")
        let model_instance = Instance::new(model_addr, Arc::clone(memory));
        let player_name = model_instance.get_name();

        // Skip local player
        if player_name.eq_ignore_ascii_case(local_player_name)
            || player_name.eq_ignore_ascii_case(local_display_name)
        {
            return None;
        }

        // Read health from Humanoid
        let (health, max_health) = if is_valid_address(humanoid_address) {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        Some(Entity {
            model_address: model_addr,
            name: player_name,
            rig_type,
            humanoid_address,
            team_address: 0,
            team_identifier: String::new(),
            body_effects_address: 0,
            armor_value_address: 0,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor: 0.0,
            max_armor: 0.0,
            has_teammate_label: false,
            is_game_specific: true,
        })
    }

    fn collect_models_recursive(
        parent: &Instance,
        memory: &Arc<Memory>,
        models: &mut Vec<Instance>,
        max_depth: u32,
    ) {
        if max_depth == 0 {
            return;
        }

        let children = parent.get_children();
        
        for child in children {
            let class_name = child.get_class_name();
            
            match class_name.as_str() {
                "Model" => {
                    models.push(Instance::new(child.address, Arc::clone(memory)));
                }
                "Folder" | "Configuration" => {
                    Self::collect_models_recursive(&child, memory, models, max_depth - 1);
                }
                _ => {
                }
            }
        }
    }

    fn build_npc_entity(
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
    ) -> Option<Entity> {
        let model_children = model.get_children();
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();

        for part_instance in model_children {
            if let Some(body_part) = BodyPart::from_name(&part_instance.get_name()) {
                let part = Part::new(part_instance.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();

                if !position.is_valid() {
                    continue;
                }

                parts.insert(
                    body_part,
                    PartData {
                        address: part_instance.address,
                        primitive_address: primitive.address,
                        size: primitive.get_size(),
                        position,
                        rotation: primitive.get_rotation(),
                    },
                );
            }
        }

        if parts.is_empty() {
            return None;
        }

        // Synthesize HumanoidRootPart from Torso/Head if missing.
        // Ground zombies and partial-body entities often lack HRP but still
        // have a Torso or Head we can use as the root reference point.
        if !parts.contains_key(&BodyPart::HumanoidRootPart) {
            if let Some(fallback) = parts.get(&BodyPart::Torso)
                .or_else(|| parts.get(&BodyPart::UpperTorso))
                .or_else(|| parts.get(&BodyPart::LowerTorso))
                .or_else(|| parts.get(&BodyPart::Head))
            {
                parts.insert(BodyPart::HumanoidRootPart, fallback.clone());
            }
        }

        let (rig_type, humanoid_address) = model
            .find_first_child("Humanoid")
            .and_then(|h| {
                if h.get_class_name() == "Humanoid" {
                    let hum = Humanoid::new(h.address, Arc::clone(memory));
                    Some((hum.get_rig_type(), h.address))
                } else {
                    // AnimationController named "Humanoid" — detect rig from parts
                    let rig = if parts.contains_key(&BodyPart::UpperTorso) { 1 } else { 0 };
                    Some((rig, 0))
                }
            })
            .unwrap_or_else(|| {
                // No "Humanoid" child at all (entity-folder games) — detect rig from parts
                let rig = if parts.contains_key(&BodyPart::UpperTorso) { 1 } else { 0 };
                (rig, 0)
            });

        let (health, max_health) = if humanoid_address != 0 {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            // Validate health values
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        let model_instance = Instance::new(model.address(), Arc::clone(memory));
        let name = model_instance.get_name();

        let (body_effects_address, armor_value_address, armor, max_armor) = model
            .find_first_child("BodyEffects")
            .map(|be| {
                let be_addr = be.address;
                let armor_instance = be.find_first_child("Armor");
                let (armor_addr, armor_val, max_armor_val) = if let Some(armor_inst) = &armor_instance {
                    let value = memory.read::<f64>(armor_inst.address + crate::core::offsets::value_base::value());
                    let armor = if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                        value as f32
                    } else {
                        0.0
                    };
                    let max_armor = be.find_first_child("MaxArmor")
                        .map(|ma| {
                            let mv = memory.read::<f64>(ma.address + crate::core::offsets::value_base::value());
                            if !mv.is_nan() && !mv.is_infinite() && mv > 0.0 { mv as f32 } else { 100.0 }
                        })
                        .unwrap_or(100.0);
                    (armor_inst.address, armor, max_armor)
                } else {
                    (0, 0.0, 0.0)
                };
                (be_addr, armor_addr, armor_val, max_armor_val)
            })
            .unwrap_or((0, 0, 0.0, 0.0));

        Some(Entity {
            model_address: model.address(),
            name,
            rig_type,
            humanoid_address,
            team_address: 0, // NPCs don't have teams
            team_identifier: String::new(),
            body_effects_address,
            armor_value_address,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor,
            max_armor,
            has_teammate_label: false, // NPCs are never teammates
            is_game_specific: false,
        })
    }

    fn update_velocity_history(
        history: &Arc<DashMap<u64, PositionHistoryVec>>,
        address: u64,
        root_pos: Vector3,
        now: Instant,
    ) {
        let mut samples = history.entry(address).or_insert_with(SmallVec::new);
        samples.push(PositionSample {
            position: root_pos,
            timestamp: now,
        });

        if samples.len() > VELOCITY_SAMPLES {
            let excess = samples.len() - VELOCITY_SAMPLES;
            samples.drain(0..excess);
        }
    }

    fn apply_velocities_from_history(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
        updated_addresses: &AHashSet<u64>,
    ) {
        // Only compute velocities for entities that had position updates this tick.
        // Targeted DashMap::get() per address is O(1) per shard vs iterating all shards.
        for address in updated_addresses {
            let address = *address;
            let entry = match position_history.get(&address) {
                Some(e) => e,
                None => continue,
            };
            let samples = entry.value();
            
            if samples.len() >= 2 {
                let mut total_velocity = Vector3::ZERO;
                let mut total_weight = 0.0;
                
                for i in 1..samples.len() {
                    let prev = &samples[i - 1];
                    let curr = &samples[i];
                    let dt = curr.timestamp.duration_since(prev.timestamp).as_secs_f32();
                    
                    if dt > 0.001 && dt < 0.5 {
                        // Weight more recent samples higher
                        let weight = i as f32;
                        let instant_vel = Vector3::new(
                            (curr.position.x - prev.position.x) / dt,
                            (curr.position.y - prev.position.y) / dt,
                            (curr.position.z - prev.position.z) / dt,
                        );
                        total_velocity.x += instant_vel.x * weight;
                        total_velocity.y += instant_vel.y * weight;
                        total_velocity.z += instant_vel.z * weight;
                        total_weight += weight;
                    }
                }
                
                if total_weight > 0.0 {
                    let raw_velocity = Vector3::new(
                        total_velocity.x / total_weight,
                        total_velocity.y / total_weight,
                        total_velocity.z / total_weight,
                    );
                    
                    let prev_smooth = smoothed_velocities
                        .get(&address)
                        .map(|r| *r.value())
                        .unwrap_or(Vector3::ZERO);
                    let smoothed = Vector3::new(
                        VELOCITY_SMOOTHING * raw_velocity.x + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.x,
                        VELOCITY_SMOOTHING * raw_velocity.y + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.y,
                        VELOCITY_SMOOTHING * raw_velocity.z + (1.0 - VELOCITY_SMOOTHING) * prev_smooth.z,
                    );
                    
                    smoothed_velocities.insert(address, smoothed);
                    
                    if let Some(mut entity) = cached_players.get_mut(&address) {
                        entity.velocity = smoothed;
                    }
                }
            }
        }
    }

    fn update_entity_positions(
        entity: &mut Entity,
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
        needs_rotation: bool,
    ) -> bool {
        // Read and validate health
        if entity.humanoid_address != 0 {
            if !is_valid_address(entity.humanoid_address) {
                return false;
            }
            let health = memory.read::<f32>(entity.humanoid_address + humanoid::health());
            let max_health = memory.read::<f32>(entity.humanoid_address + humanoid::max_health());
            
            if health.is_nan() || max_health.is_nan() 
                || max_health <= 0.0 || max_health > 100000.0 
            {
                return false; 
            }
            if !health.is_infinite() && health >= 0.0 {
                entity.health = health;
            }
            if !max_health.is_infinite() {
                entity.max_health = max_health;
            }
        }

        // Re-discover missing parts (Roblox loads character parts progressively, so initial
        // scan may only find 2-3 parts). Rate-limited via a static counter so we don't
        // call the expensive get_children() on every tick for incomplete entities.
        //
        // For non-standard character models (e.g., Operation One with only HumanoidRootPart),
        // we skip re-discovery entirely when the entity has only 1 part — those models will
        // never gain standard named parts no matter how many times we scan.
        let expected_parts = if entity.rig_type == 1 { 16 } else { 7 }; // R15=16(15+HRP), R6=7(6+HRP)
        if entity.parts.len() < expected_parts && entity.parts.len() > 1 {
            // Per-entity rate limiting using Knuth hash — each entity hashes to a
            // different slot in the cycle window, so re-discovery runs at a steady
            // per-entity cadence regardless of total entity count.
            static REDISCOVERY_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let tick = REDISCOVERY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let entity_slot = (entity.model_address as u32).wrapping_mul(2654435761);
            if (tick.wrapping_add(entity_slot)) % POSITION_STALE_CHECK_CYCLES == 0 {
                let model_children = model.get_children();
                for part_instance in model_children {
                    if let Some(body_part) = BodyPart::from_name(&part_instance.get_name()) {
                        // Only add parts we don't already have
                        if !entity.parts.contains_key(&body_part) {
                            let part = Part::new(part_instance.address, Arc::clone(memory));
                            let primitive = part.get_primitive();
                            let position = primitive.get_position();
                            if position.is_valid() {
                                entity.parts.insert(body_part, PartData {
                                    address: part_instance.address,
                                    primitive_address: primitive.address,
                                    size: primitive.get_size(),
                                    position,
                                    rotation: primitive.get_rotation(),
                                });
                            }
                        }
                    }
                }

                // Synthesize HumanoidRootPart if still missing after re-discovery
                if !entity.parts.contains_key(&BodyPart::HumanoidRootPart) {
                    if let Some(fallback) = entity.parts.get(&BodyPart::Torso)
                        .or_else(|| entity.parts.get(&BodyPart::UpperTorso))
                        .or_else(|| entity.parts.get(&BodyPart::LowerTorso))
                        .or_else(|| entity.parts.get(&BodyPart::Head))
                    {
                        let fb = fallback.clone();
                        entity.parts.insert(BodyPart::HumanoidRootPart, fb);
                    }
                }
            } // end rate-limited re-discovery
        }

        for part_data in entity.parts.values_mut() {
            if is_valid_address(part_data.primitive_address) {
                let pos = memory.read::<Vector3>(part_data.primitive_address + crate::core::offsets::base_part::position());
                if pos.is_valid() {
                    part_data.position = pos;
                }
                if needs_rotation {
                    let rot = memory.read::<Matrix3>(part_data.primitive_address + crate::core::offsets::base_part::rotation());
                    part_data.rotation = rot;
                }
            }
        }
        
        // Update armor value directly from cached address (no find_first_child per tick)
        if is_valid_address(entity.armor_value_address) {
            let value = memory.read::<f64>(entity.armor_value_address + crate::core::offsets::value_base::value());
            if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                entity.armor = value as f32;
            }
        }
        
        // Periodically re-check TeammateLabel on HumanoidRootPart.
        // Teams can change between rounds (e.g. Rivals reshuffles teams), and TeammateLabel
        // may be added/removed by the server after the character model is created.
        //
        // IMPORTANT: Only run this for standard (non-game-specific) entities.
        // Game-specific entities (e.g. Phantom Forces) use `has_teammate_label` for
        // colour-based team detection set during entity build — overwriting it with
        // a generic TeammateLabel child check would destroy the PF team data.
        //
        // Rate limiting: We use the entity's model_address hash to distribute checks
        // across ticks. This avoids the old bug where a single static counter was shared
        // across ALL entities, causing checks to fire far less often than intended.
        if !entity.is_game_specific {
            let entity_tick = (entity.model_address as u32).wrapping_mul(2654435761); // Knuth hash
            static TEAMMATE_GLOBAL_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let global_tick = TEAMMATE_GLOBAL_TICK.fetch_add(1, Ordering::Relaxed);
            // Each entity hashes to a different slot in the 20-tick window
            if (global_tick.wrapping_add(entity_tick)) % 20 == 0 {
                if let Some(hrp) = entity.parts.get(&BodyPart::HumanoidRootPart) {
                    let hrp_instance = Instance::new(hrp.address, Arc::clone(memory));
                    let new_label = hrp_instance.find_first_child("TeammateLabel").is_some();
                    if new_label != entity.has_teammate_label {
                        tracing::debug!("[Cache] '{}' teammate label changed: {} -> {}", entity.name, entity.has_teammate_label, new_label);
                        entity.has_teammate_label = new_label;
                    }
                }
            }
        } else {
            // Game-specific entities (e.g. PF): periodically re-read tag colour.
            // Auto-balance can swap teams mid-round; without re-reading, the entity
            // keeps stale team data until a full rebuild (which only happens on respawn).
            static PF_TAG_RECHECK_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let tick = PF_TAG_RECHECK_TICK.fetch_add(1, Ordering::Relaxed);
            let entity_slot = (entity.model_address as u32).wrapping_mul(2654435761);
            if (tick.wrapping_add(entity_slot)) % 40 == 0 {
                // Re-check PlayerTag colour on the Head part
                if let Some(head) = entity.parts.get(&BodyPart::Head) {
                    let head_instance = Instance::new(head.address, Arc::clone(memory));
                    if let Some(billboard) = head_instance.find_first_child_by_class("BillboardGui") {
                        if let Some(player_tag) = billboard.find_first_child("PlayerTag") {
                            let text_color_offset = crate::core::offsets::gui_object::text_color3();
                            let r: f32 = memory.read(player_tag.address + text_color_offset);
                            let b: f32 = memory.read(player_tag.address + text_color_offset + 8);
                            let new_teammate = b > 0.5 && r < 0.5;
                            if new_teammate != entity.has_teammate_label {
                                tracing::info!("[Cache] PF '{}' tag colour changed: teammate {} -> {} (r={:.2}, b={:.2})",
                                    entity.name, entity.has_teammate_label, new_teammate, r, b);
                                entity.has_teammate_label = new_teammate;
                            }
                        }
                    }
                }
            }
        }
        
        true  // Entity is still valid
    }

    fn build_entity(
        player: &Player,
        model: &crate::sdk::ModelInstance,
        memory: &Arc<Memory>,
    ) -> Option<Entity> {
        let model_children = model.get_children();
        let player_name = player.get_name();
        
        if model_children.is_empty() {
            return None;
        }

        let mut parts = ahash::AHashMap::new();
        let mut unmatched_parts: Vec<String> = Vec::new();

        for part_instance in model_children {
            let name = part_instance.get_name();
            // Use BodyPart enum for fast hashing
            if let Some(body_part) = BodyPart::from_name(&name) {
                let part = Part::new(part_instance.address, Arc::clone(memory));
                let primitive = part.get_primitive();
                let position = primitive.get_position();

                if !position.is_valid() {
                    continue;
                }

                // Cache primitive address for fast position reads (avoids find_first_child every frame)
                parts.insert(
                    body_part,
                    PartData {
                        address: part_instance.address,
                        primitive_address: primitive.address,
                        size: primitive.get_size(),
                        position,
                        rotation: primitive.get_rotation(),
                    },
                );
            } else {
                let class = part_instance.get_class_name();
                // Track unmatched parts for diagnostics (only if they're BasePart/MeshPart)
                if class == "Part" || class == "MeshPart" || class == "UnionOperation" {
                    unmatched_parts.push(name);
                }
            }
        }

        if parts.is_empty() {
            // Log once per player for diagnostics when no standard body parts found
            // but unmatched parts exist (non-standard character models)
            if !unmatched_parts.is_empty() {
                tracing::debug!("[Cache] '{}' has no standard body parts but has: {:?}", player_name, unmatched_parts);
            }
            return None;
        }

        // Log entity construction details (rate-limited, once per entity name)
        if !unmatched_parts.is_empty() {
            tracing::debug!("[Cache] '{}' built with {} standard parts, {} unmatched parts: {:?}",
                player_name, parts.len(), unmatched_parts.len(), unmatched_parts);
        }

        let (rig_type, humanoid_address) = model
            .find_first_child("Humanoid")
            .map(|h| {
                let hum = Humanoid::new(h.address, Arc::clone(memory));
                (hum.get_rig_type(), h.address)
            })
            .unwrap_or((0, 0));

        let (health, max_health) = if humanoid_address != 0 {
            let h = memory.read::<f32>(humanoid_address + humanoid::health());
            let mh = memory.read::<f32>(humanoid_address + humanoid::max_health());
            // Validate health values
            let health = if !h.is_nan() && !h.is_infinite() && h >= 0.0 && h <= 100000.0 { h } else { 100.0 };
            let max_health = if !mh.is_nan() && !mh.is_infinite() && mh > 0.0 && mh <= 100000.0 { mh } else { 100.0 };
            (health, max_health)
        } else {
            (100.0, 100.0)
        };

        let team_address = player.get_team_address();

        // Rivals-style team check: Check if HumanoidRootPart has a TeammateLabel child
        // This is more reliable than the whitelist system for games like Rivals
        let has_teammate_label = parts
            .get(&BodyPart::HumanoidRootPart)
            .map(|hrp| {
                let hrp_instance = Instance::new(hrp.address, Arc::clone(memory));
                hrp_instance.find_first_child("TeammateLabel").is_some()
            })
            .unwrap_or(false);

        let (body_effects_address, armor_value_address, armor, max_armor) = model
            .find_first_child("BodyEffects")
            .map(|be| {
                let be_addr = be.address;
                let armor_instance = be.find_first_child("Armor");
                let (armor_addr, armor_val, max_armor_val) = if let Some(armor_inst) = &armor_instance {
                    let value = memory.read::<f64>(armor_inst.address + crate::core::offsets::value_base::value());
                    let armor = if !value.is_nan() && !value.is_infinite() && value >= 0.0 && value <= 10000.0 {
                        value as f32
                    } else {
                        0.0
                    };
                    let max_armor = be.find_first_child("MaxArmor")
                        .map(|ma| {
                            let mv = memory.read::<f64>(ma.address + crate::core::offsets::value_base::value());
                            if !mv.is_nan() && !mv.is_infinite() && mv > 0.0 { mv as f32 } else { 100.0 }
                        })
                        .unwrap_or(100.0);
                    (armor_inst.address, armor, max_armor)
                } else {
                    (0, 0.0, 0.0)
                };
                (be_addr, armor_addr, armor_val, max_armor_val)
            })
            .unwrap_or((0, 0, 0.0, 0.0));

        let entity = Entity {
            model_address: model.address(),
            name: player_name.clone(),
            rig_type,
            humanoid_address,
            team_address,
            team_identifier: String::new(), // Standard players use team_address
            body_effects_address,
            armor_value_address,
            parts,
            velocity: Vector3::ZERO,
            health,
            max_health,
            armor,
            max_armor,
            has_teammate_label,
            is_game_specific: false,
        };

        // Diagnostic: log entity details on first build (helps debug game-specific issues)
        tracing::debug!(
            "[Cache] Entity built: '{}' | parts: {} | rig: {} | hp: {:.0}/{:.0} | team_addr: {:#x} | teammate_label: {} | model: {:#x}",
            player_name, entity.parts.len(), rig_type, health, max_health, team_address, has_teammate_label, model.address()
        );

        Some(entity)
    }

    /// Merge body parts from matching workspace models into entities that only have
    /// HumanoidRootPart. Handles Operation One-style games where Player.Character
    /// contains a minimal model (just HumanoidRootPart for network positioning)
    /// and a SEPARATE Model in Workspace with the same player name holds the actual
    /// renderable body (collision, collision2, collision3, hip, legs).
    fn merge_workspace_body_parts(
        cached_players: &Arc<DashMap<u64, Entity>>,
        workspace: &Arc<Instance>,
        memory: &Arc<Memory>,
    ) {
        // Collect entities that need merging — only those with ≤1 standard body part
        let incomplete: Vec<(u64, String, u64)> = cached_players.iter()
            .filter(|r| r.value().parts.len() <= 1 && !r.value().is_game_specific)
            .map(|r| (*r.key(), r.value().name.clone(), r.value().model_address))
            .collect();

        if incomplete.is_empty() {
            return;
        }

        // Scan workspace direct children once for all incomplete entities
        let workspace_children = workspace.get_children();

        for (entity_key, entity_name, entity_model_addr) in &incomplete {
            for ws_child in &workspace_children {
                // Skip the Character model itself (same address = same object)
                if ws_child.address == *entity_model_addr {
                    continue;
                }

                // Must match player name
                if !ws_child.get_name().eq_ignore_ascii_case(entity_name) {
                    continue;
                }

                // Must be a Model
                if ws_child.get_class_name() != "Model" {
                    continue;
                }

                // Scan this workspace model's children for body parts we can merge
                let model = crate::sdk::ModelInstance::new(ws_child.address, Arc::clone(memory));
                let model_children = model.get_children();

                let mut merged = 0u32;
                if let Some(mut entry) = cached_players.get_mut(entity_key) {
                    for part_instance in &model_children {
                        let name = part_instance.get_name();
                        if let Some(body_part) = BodyPart::from_name(&name) {
                            // Skip HumanoidRootPart — keep the one from Player.Character
                            if body_part == BodyPart::HumanoidRootPart {
                                continue;
                            }
                            // Skip parts we already have
                            if entry.parts.contains_key(&body_part) {
                                continue;
                            }
                            let part = Part::new(part_instance.address, Arc::clone(memory));
                            let primitive = part.get_primitive();
                            let position = primitive.get_position();
                            if position.is_valid() {
                                entry.parts.insert(body_part, PartData {
                                    address: part_instance.address,
                                    primitive_address: primitive.address,
                                    size: primitive.get_size(),
                                    position,
                                    rotation: primitive.get_rotation(),
                                });
                                merged += 1;
                            }
                        }
                    }
                }

                if merged > 0 {
                    tracing::info!(
                        "[Cache] Merged {} workspace collision parts for '{}' (ws_model: {:#x}, total parts: {})",
                        merged, entity_name, ws_child.address, merged + 1
                    );
                    break; // Found the right workspace model
                }
            }
        }
    }

    fn cleanup_disconnected(
        cached_players: &Arc<DashMap<u64, Entity>>,
        position_history: &Arc<DashMap<u64, PositionHistoryVec>>,
        smoothed_velocities: &Arc<DashMap<u64, Vector3>>,
        current_addresses: &AHashSet<u64>,
    ) {
        // Lock-free cleanup with DashMap's retain
        // Only remove entities that are truly disconnected (not in player list)
        // Don't remove entities at origin - they may be respawning (grace period handled elsewhere)
        cached_players.retain(|addr, _entity| {
            current_addresses.contains(addr)
        });
        // Keep velocity history a bit longer for smoother re-acquisition after respawn
        // Only clean up entries that have been gone for multiple cycles
        position_history.retain(|addr, samples| {
            if current_addresses.contains(addr) {
                return true;
            }
            // Keep history for 500ms after disconnect (for respawn grace period)
            if let Some(last) = samples.last() {
                last.timestamp.elapsed().as_millis() < 500
            } else {
                false
            }
        });
        smoothed_velocities.retain(|addr, _| current_addresses.contains(addr));
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        self.stop();
    }
}
