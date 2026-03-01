use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{BindableFeature, Config, ConfigManager, HotkeyKey};
use crate::features::{AimAssist, AntiAfk, AutoClicker, AutoReload, CameraAim, Chams, Crosshair, Desync, Esp, EspRenderCache, FootprintTracker, HitboxExpander, MovementHacks, SilentAim, Triggerbot, WorldModifier};
use crate::features::aimbot::ViewportAim;
use crate::features::cosmetics;
use crate::sdk::{Instance, VisualEngine};
use crate::utils::cache::Cache;
use crate::utils::input::Input;
use crate::utils::map_parser::get_map_parser;
use crate::utils::game_support::blade_ball::BladeBallAutoParry;

use super::menu::{self, MenuTab, toggle_feature};
use super::dex_ui;

pub static PENDING_RELOAD: AtomicBool = AtomicBool::new(false);
pub static PENDING_REFRESH: AtomicBool = AtomicBool::new(false);

/// Main overlay application.
pub struct OverlayApp {
    // Core systems
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    config_manager: Arc<ConfigManager>,
    local_player_name: String,
    discord_username: String,
    players_instance: Arc<Instance>,
    workspace_instance: Arc<Instance>,
    memory: Arc<crate::core::Memory>,
    datamodel: Arc<Instance>,

    // Features
    aim_assist: AimAssist,
    camera_aim: CameraAim,
    viewport_aim: ViewportAim,
    silent_aim: SilentAim,
    triggerbot: Triggerbot,
    movement_hacks: MovementHacks,
    world_modifier: WorldModifier,
    autoclicker: AutoClicker,
    hitbox_expander: HitboxExpander,
    anti_afk: AntiAfk,
    auto_reload: AutoReload,
    desync: Desync,
    blade_ball: BladeBallAutoParry,

    // UI State
    menu_open: bool,
    menu_minimized: bool,
    current_tab: MenuTab,
    menu_pos: egui::Pos2,
    hotkey_pos: egui::Pos2,

    // Hotkey states
    key_states: [bool; 18],

    // Performance
    last_frame_time: Instant,
    last_reload_time: Instant,
    last_refresh_time: Instant,
    window_initialized: bool,
    win_click_through_style: bool,

    // Cached cosmetics model (to avoid repeated lookups)
    cached_cosmetics_model: Option<Arc<Instance>>,

    // ESP render cache (background thread for pre-computing ESP data)
    esp_render_cache: Arc<EspRenderCache>,
    
    // Footprint and movement trail tracker
    footprint_tracker: FootprintTracker,

    // Auto-refresh: tracks when cache last had valid entities
    last_valid_cache_time: Instant,
    consecutive_empty_frames: u32,
    
    // Whether egui visuals have been set (only needs to happen once)
    visuals_initialized: bool,
}

impl OverlayApp {
    pub fn new(
        cache: Arc<Cache>,
        visengine: Arc<VisualEngine>,
        config_manager: Arc<ConfigManager>,
        memory: Arc<crate::core::Memory>,
        datamodel: Arc<Instance>,
        discord_username: String,
        win_click_through_style: bool,
    ) -> Self {
        let config = config_manager.get();
        let local_player_name = config.general.username.clone();

        // Find players instance for hitbox expander
        let players_instance = datamodel.find_first_child_by_class("Players")
            .map(|p| Arc::new(p))
            .unwrap_or_else(|| Arc::new(Instance::new(0, Arc::clone(&memory))));

        // Find workspace instance for map parser
        let workspace_instance = datamodel.find_first_child_by_class("Workspace")
            .map(|w| Arc::new(w))
            .unwrap_or_else(|| Arc::new(Instance::new(0, Arc::clone(&memory))));

        // Initialize DEX Explorer
        dex_ui::init_dex_explorer(Arc::clone(&memory));

        // Initialize ESP render cache and start background thread
        let esp_render_cache = Arc::new(EspRenderCache::new());
        esp_render_cache.set_local_player_name(&local_player_name);
        esp_render_cache.update_config(&config);
        esp_render_cache.start(Arc::clone(&cache), Arc::clone(&visengine));

        Self {
            cache: Arc::clone(&cache),
            visengine: Arc::clone(&visengine),
            config_manager,
            local_player_name: local_player_name.clone(),
            discord_username,
            players_instance: Arc::clone(&players_instance),
            workspace_instance: Arc::clone(&workspace_instance),
            memory: Arc::clone(&memory),
            datamodel: Arc::clone(&datamodel),
            aim_assist: AimAssist::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            camera_aim: CameraAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            viewport_aim: ViewportAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            silent_aim: SilentAim::new(Arc::clone(&memory), Arc::clone(&cache), Arc::clone(&visengine)),
            triggerbot: Triggerbot::new(),
            movement_hacks: MovementHacks::new(Arc::clone(&memory), Arc::clone(&cache), local_player_name.clone()),
            world_modifier: WorldModifier::new(Arc::clone(&memory), &datamodel),
            autoclicker: AutoClicker::new(),
            hitbox_expander: HitboxExpander::new(Arc::clone(&memory), Arc::clone(&cache), local_player_name.clone()),
            anti_afk: AntiAfk::new(),
            auto_reload: {
                let mut ar = AutoReload::new();
                ar.init(Arc::clone(&memory), Arc::clone(&players_instance), local_player_name.clone());
                ar
            },
            desync: Desync::new(Arc::clone(&memory), memory.base_address()),
            blade_ball: BladeBallAutoParry::new(Arc::clone(&memory)),
            menu_open: true,
            menu_minimized: false,
            current_tab: MenuTab::Visuals,
            menu_pos: egui::pos2(20.0, 20.0),
            hotkey_pos: egui::pos2(config.interface.hotkey_pos_x, config.interface.hotkey_pos_y),
            key_states: [false; 18],
            last_frame_time: Instant::now(),
            last_reload_time: Instant::now(),
            last_refresh_time: Instant::now(),
            window_initialized: false,
            win_click_through_style,
            cached_cosmetics_model: None,
            esp_render_cache,
            footprint_tracker: FootprintTracker::new(),
            last_valid_cache_time: Instant::now(),
            consecutive_empty_frames: 0,
            visuals_initialized: false,
        }
    }

    fn handle_hotkeys(&mut self, config: &mut Config) {
        // F1 - Menu toggle (always active, not configurable)
        if Input::is_key_pressed(config.hotkeys.menu_toggle as i32, &mut self.key_states[0]) {
            self.menu_open = !self.menu_open;
        }

        // DEX Explorer is toggled from the menu UI only (no hotkey)

        // F9 - Reload (system key)
        if Input::is_key_pressed(config.hotkeys.reload_data as i32, &mut self.key_states[8]) {
            PENDING_RELOAD.store(true, Ordering::SeqCst);
        }

        // Home - Full refresh (system key)
        if Input::is_key_pressed(config.hotkeys.full_refresh as i32, &mut self.key_states[12]) {
            PENDING_REFRESH.store(true, Ordering::SeqCst);
        }

        // End - Save config (system key)
        if Input::is_key_pressed(config.hotkeys.save_config as i32, &mut self.key_states[13]) {
            self.config_manager.save().ok();
            tracing::info!("Config saved to file");
        }

        // F12 - Exit (system key, always active)
        if Input::is_key_pressed(config.hotkeys.exit as i32, &mut self.key_states[9]) {
            std::process::exit(0);
        }

        // Insert - Autoclicker toggle (special handling for autoclicker)
        if Input::is_key_pressed(config.hotkeys.autoclicker_toggle as i32, &mut self.key_states[14]) {
            self.autoclicker.toggle(&config.autoclicker);
            tracing::info!("AutoClicker: {}", if self.autoclicker.is_running() { "[enabled]" } else { "[disabled]" });
        }

        // Process configurable hotkey bindings (slots 1-10 use key_states 1-10)
        // First, collect the data we need to avoid borrow conflict
        let bindings: Vec<(usize, HotkeyKey, BindableFeature)> = config.hotkey_bindings.slots.iter()
            .enumerate()
            .filter(|(_, slot)| slot.key != HotkeyKey::None && slot.feature != BindableFeature::None)
            .map(|(idx, slot)| (idx, slot.key, slot.feature))
            .collect();
        
        // Reserved system keys
        let reserved_keys = [
            config.hotkeys.menu_toggle,
            config.hotkeys.exit,
            config.hotkeys.reload_data,
            config.hotkeys.full_refresh,
            config.hotkeys.save_config,
        ];
        
        for (slot_idx, key, feature) in bindings {
            let vk_code = key.to_vk_code() as i32;
            let state_idx = slot_idx + 1; // Use slots 1-10
            
            // Skip if this key is a reserved system key
            if reserved_keys.contains(&(vk_code as u32)) {
                continue;
            }
            
            if state_idx < 15 && Input::is_key_pressed(vk_code, &mut self.key_states[state_idx]) {
                let feature_name = feature.display_name();
                toggle_feature(config, feature);
                
                // Log the toggle
                let is_enabled = match feature {
                    BindableFeature::BoxEsp => config.visuals.box_esp,
                    BindableFeature::NameTags => config.visuals.name_tags,
                    BindableFeature::Tracers => config.visuals.tracers,
                    BindableFeature::HealthBars => config.visuals.health_bars,
                    BindableFeature::ArmourBars => config.visuals.armor_bars,
                    BindableFeature::Chams => config.visuals.chams,
                    BindableFeature::TeamCheck => config.visuals.team_check,
                    BindableFeature::HideDead => config.visuals.hide_dead,
                    BindableFeature::ShowBots => config.visuals.show_bots,
                    BindableFeature::AimAssist => config.aimbot.enabled,
                    BindableFeature::Triggerbot => config.triggerbot.enabled,
                    BindableFeature::CameraAim => config.camera_aim.enabled,
                    BindableFeature::AutoReload => config.aimbot.auto_reload,
                    BindableFeature::Fly => config.movement.fly_enabled,
                    BindableFeature::Noclip => config.movement.noclip_enabled,
                    BindableFeature::Spinbot => config.movement.spinbot_enabled,
                    BindableFeature::AntiSit => config.movement.anti_sit_enabled,
                    BindableFeature::VoidHide => config.movement.void_hide_enabled,
                    BindableFeature::HitboxMod => config.hitbox.enabled,
                    BindableFeature::ShowHitboxVisual => config.hitbox.show_visual,
                    BindableFeature::Fullbright => config.world.fullbright,
                    BindableFeature::CameraFov => config.camera.fov_enabled,
                    BindableFeature::Korblox => config.cosmetics.korblox,
                    BindableFeature::Headless => config.cosmetics.headless,
                    BindableFeature::AntiAfk => config.anti_afk.enabled,
                    BindableFeature::AutoClicker => config.autoclicker.enabled,
                    BindableFeature::BladeBall => config.blade_ball.enabled,
                    BindableFeature::Desync => config.desync.enabled,
                    BindableFeature::WallCheck => config.visuals.wall_check,
                    BindableFeature::Footprints => config.visuals.footprints,
                    BindableFeature::MovementTrails => config.visuals.movement_trails,
                    BindableFeature::Anchor => config.movement.anchor_enabled,
                    BindableFeature::Waypoint => config.movement.waypoint_enabled,
                    _ => false,
                };
                tracing::info!("{}: {}", feature_name, if is_enabled { "[enabled]" } else { "[disabled]" });
            }
        }
    }

    fn reload_data(&mut self) {
        if self.last_reload_time.elapsed().as_secs() >= 2 {
            self.cache.clear();
            self.last_reload_time = Instant::now();
        }
    }

    /// Full game instance refresh - re-reads DataModel, VisualEngine, Players from memory.
    /// Use this when switching games/rounds causes the base addresses to become invalid.
    fn refresh_game_instances(&mut self) {
        if self.last_refresh_time.elapsed().as_secs() < 3 {
            tracing::warn!("Refresh on cooldown, please wait...");
            return;
        }

        tracing::info!("Refreshing game instances...");
        
        let config = self.config_manager.get();
        let base = self.memory.base_address();

        // Re-read DataModel
        let fake_dm = self.memory.read::<u64>(base + crate::core::offsets::fake_datamodel::pointer());
        if fake_dm == 0 {
            tracing::error!("FakeDataModel is null - game may not be running");
            return;
        }

        let dm_addr = self.memory.read::<u64>(fake_dm + crate::core::offsets::fake_datamodel::real_datamodel());
        if dm_addr == 0 {
            tracing::error!("DataModel is null - game may be loading");
            return;
        }

        let new_datamodel = Arc::new(Instance::new(dm_addr, Arc::clone(&self.memory)));

        // Re-read VisualEngine
        let ve_addr = self.memory.read::<u64>(base + crate::core::offsets::visual_engine::pointer());
        let new_visengine = Arc::new(crate::sdk::VisualEngine::new(ve_addr, Arc::clone(&self.memory)));

        // Find Players service
        let new_players = match new_datamodel.find_first_child_by_class("Players") {
            Some(p) => Arc::new(p),
            None => {
                tracing::error!("Players service not found - game may be loading");
                return;
            }
        };

        // Update instance references
        self.datamodel = Arc::clone(&new_datamodel);
        self.visengine = Arc::clone(&new_visengine);
        self.players_instance = Arc::clone(&new_players);

        // Recreate features that depend on these instances
        self.aim_assist = AimAssist::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.camera_aim = CameraAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.viewport_aim = ViewportAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.silent_aim = SilentAim::new(Arc::clone(&self.memory), Arc::clone(&self.cache), Arc::clone(&new_visengine));
        self.triggerbot = Triggerbot::new();
        self.world_modifier = WorldModifier::new(Arc::clone(&self.memory), &self.datamodel);
        self.hitbox_expander = HitboxExpander::new(
            Arc::clone(&self.memory),
            Arc::clone(&self.cache),
            self.local_player_name.clone(),
        );

        // Find Workspace
        let new_workspace = match new_datamodel.find_first_child_by_class("Workspace") {
            Some(w) => Arc::new(w),
            None => {
                tracing::error!("Workspace not found - game may be loading");
                return;
            }
        };

        // Re-detect game ID from PlaceId (handles game switches between rounds)
        let (game_id, raw_place_id) = crate::app_init::detect_game_id(&new_datamodel, &self.memory);
        self.cache.set_game_id(game_id);
        tracing::info!("Refreshed game ID: {} (raw PlaceId: {})", game_id.name(), raw_place_id);

        // Reset game-specific state so it re-initializes on next scan
        crate::utils::game_support::operation_one::reset_dump();

        // Clear blade ball cached state so it re-discovers folders/ball in the new workspace
        self.blade_ball.clear_cache();

        // Update workspace instance for map parser
        self.workspace_instance = Arc::clone(&new_workspace);

        // Clear map parser cache on refresh (will rescan on next frame if wall_check enabled)
        get_map_parser().clear();

        // Restart cache with new Players and Workspace instances
        self.cache.restart(
            new_players,
            new_workspace,
            Arc::clone(&self.memory),
            config.performance.cache_update_ms,
        );

        // Restart ESP render cache with updated instances
        self.esp_render_cache.stop();
        self.esp_render_cache = Arc::new(EspRenderCache::new());
        self.esp_render_cache.set_local_player_name(&self.local_player_name);
        self.esp_render_cache.update_config(&config);
        self.esp_render_cache.start(Arc::clone(&self.cache), Arc::clone(&self.visengine));

        // Clear cosmetics cache on refresh
        self.cached_cosmetics_model = None;

        self.last_refresh_time = Instant::now();
        self.last_valid_cache_time = Instant::now();
        self.consecutive_empty_frames = 0;
        tracing::info!("Game instances refreshed successfully");
    }

    fn setup_window(&mut self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                FindWindowW, GetWindowLongW, SetWindowLongW, SetWindowPos,
                GWL_EXSTYLE, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE,
                WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            };

            let config = self.config_manager.get();
            let title: Vec<u16> = config.general.window_title.encode_utf16().chain(std::iter::once(0)).collect();

            if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR::from_raw(title.as_ptr())) {
                if !self.window_initialized {
                    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    let mut new_style = ex_style | WS_EX_LAYERED.0 as i32 | WS_EX_TOOLWINDOW.0 as i32;
                    if self.win_click_through_style {
                        // Enable Windows-level click-through (in addition to egui mouse passthrough)
                        new_style |= WS_EX_TRANSPARENT.0 as i32;
                    }
                    SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
                    
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    
                    self.window_initialized = true;
                }
            }
        }
    }

    /// Get camera address for viewport aim
    #[inline]
    fn get_camera_address(&self) -> u64 {
        self.memory.resolve_camera_address().unwrap_or(0)
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Use Rgba::TRANSPARENT as shown in egui's custom_window_frame example
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Get mutable config
        let mut config = self.config_manager.get();

        // Check for pending reload (soft - cache clear only)
        if PENDING_RELOAD.swap(false, Ordering::SeqCst) {
            self.reload_data();
        }

        // Check for pending refresh (full - re-read game instances)
        if PENDING_REFRESH.swap(false, Ordering::SeqCst) {
            self.refresh_game_instances();
        }

        // Auto-refresh: detect stale game instances after round transitions
        // When the cache has no valid entities for too long, the game addresses are likely stale.
        {
            let cache_count = self.cache.count();
            if cache_count > 0 {
                self.last_valid_cache_time = Instant::now();
                self.consecutive_empty_frames = 0;
            } else {
                self.consecutive_empty_frames += 1;
            }

            // Skip auto-refresh when cache is intentionally paused (no features need data).
            if !self.cache.is_paused() {
            // Auto-refresh if cache has been empty long enough
            let empty_duration = self.last_valid_cache_time.elapsed();
            if self.consecutive_empty_frames > 150
                && empty_duration.as_secs() >= 4
                && self.last_refresh_time.elapsed().as_secs() >= 5
            {
                // Validate off the render thread to avoid hitches.
                let players = Arc::clone(&self.players_instance);
                let empty_frames = self.consecutive_empty_frames;
                // Use a simple flag to communicate the result back
                static AUTO_REFRESH_PENDING: AtomicBool = AtomicBool::new(false);
                static AUTO_REFRESH_SOFT_CLEAR: AtomicBool = AtomicBool::new(false);

                // Check if a previous background check signaled we need to refresh
                if AUTO_REFRESH_PENDING.swap(false, Ordering::SeqCst) {
                    tracing::info!("[AUTO-REFRESH] Background check confirmed stale - refreshing game instances");
                    self.refresh_game_instances();
                } else if AUTO_REFRESH_SOFT_CLEAR.swap(false, Ordering::SeqCst) {
                    tracing::debug!("[AUTO-REFRESH] Background check: cache empty but Players valid — forcing re-scan");
                    self.cache.clear();
                } else {
                    // Launch background validation (only if not already in-flight)
                    static CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
                    if !CHECK_IN_FLIGHT.swap(true, Ordering::SeqCst) {
                        std::thread::Builder::new()
                            .name("auto-refresh-check".into())
                            .spawn(move || {
                                let players_children = players.get_children();
                                let has_valid_players = players_children.iter().any(|c| {
                                    let name = c.get_name();
                                    !name.is_empty() && name.len() < 50
                                });
                                if !has_valid_players {
                                    AUTO_REFRESH_PENDING.store(true, Ordering::SeqCst);
                                } else if empty_frames % 300 == 0 {
                                    AUTO_REFRESH_SOFT_CLEAR.store(true, Ordering::SeqCst);
                                }
                                CHECK_IN_FLIGHT.store(false, Ordering::SeqCst);
                            })
                            .ok();
                    }
                }
            }
            } // end !is_paused
        }

        // Set transparent overlay visuals only once (not every frame)
        if !self.visuals_initialized {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::TRANSPARENT;
            visuals.window_fill = egui::Color32::TRANSPARENT;
            visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
            visuals.faint_bg_color = egui::Color32::TRANSPARENT;
            visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            ctx.set_visuals(visuals);
            self.visuals_initialized = true;
        }

  
        {
        self.handle_hotkeys(&mut config);
        }
        self.setup_window();
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!self.menu_open));

        // Frame rate limiting
        let target_fps = if config.blade_ball.enabled {
            config.performance.target_fps.max(100) // Fast discovery for parry thread
        } else if self.menu_open || config.visuals.box_esp || config.visuals.chams || config.visuals.mesh_chams || config.aimbot.enabled {
            config.performance.target_fps
        } else if config.performance.idle_mode {
            config.performance.idle_fps
        } else {
            config.performance.target_fps
        };

        let frame_delay = Duration::from_secs_f64(1.0 / target_fps.clamp(5, 144) as f64);
        ctx.request_repaint_after(frame_delay);
        self.last_frame_time = Instant::now();

        // Apply features (pass menu_open to disable fly when GUI is active)
        {
            self.movement_hacks.apply_all(&config, self.menu_open);
            self.world_modifier.apply_all(&config);
        self.anti_afk.update(&config.anti_afk);
        
        // Update desync (handles Hold / Toggle / Timed modes internally)
        {
            let desync_keybind_held = Input::is_key_down(config.desync.keybind as i32);
            self.desync.update(&config.desync, desync_keybind_held);
        }

        // Update Blade Ball auto-parry (only in Blade Ball game)
        if config.blade_ball.enabled
            && self.cache.get_game_id() == crate::utils::game_support::GameId::BladeBall
        {
            let local_entity = self.cache.get_snapshot().iter()
                .find(|e| e.name.eq_ignore_ascii_case(&self.local_player_name))
                .cloned();
            let local_root_pos = local_entity.as_ref().and_then(|e| e.root_position());
            let local_model_addr = local_entity.as_ref().map(|e| e.model_address).unwrap_or(0);
            self.blade_ball.update(&config.blade_ball, &self.workspace_instance, local_root_pos, &self.local_player_name, local_model_addr);
        }
        
        // Update auto reload (only when menu is closed to avoid interference)
        if !self.menu_open {
            self.auto_reload.update(config.aimbot.auto_reload);
        }
        
        // Update cache settings
        self.cache.set_show_bots(config.visuals.show_bots);
        self.cache.set_needs_rotation(config.visuals.chams || config.visuals.mesh_chams);

        // Pause cache when no features need entity data.
        {
            let any_visual = config.visuals.box_esp
                || config.visuals.name_tags
                || config.visuals.tracers
                || config.visuals.health_bars
                || config.visuals.armor_bars
                || config.visuals.chams
                || config.visuals.mesh_chams
                || config.visuals.footprints
                || config.visuals.movement_trails
                || config.visuals.show_bots;
            let any_aim = config.aimbot.enabled
                || config.camera_aim.enabled
                || config.silent_aim.enabled
                || config.triggerbot.enabled
                || config.viewport_aim.enabled;
            let any_combat = config.hitbox.enabled
                || config.blade_ball.enabled;
            let any_movement = config.movement.fly_enabled
                || config.movement.noclip_enabled
                || config.movement.spinbot_enabled
                || config.movement.hip_height_enabled
                || config.movement.void_hide_enabled
                || config.movement.vehicle_fly_enabled
                || config.movement.anchor_enabled;
            let cache_needed = any_visual || any_aim || any_combat || any_movement;
            self.cache.set_paused(!cache_needed);
        }

        // Update map parser for wall check (scan workspace periodically)
        {
            let map_parser = get_map_parser();
            // Enable/disable based on config
            map_parser.set_enabled(config.visuals.wall_check);
            
            // Scan on a background thread to avoid blocking the render loop
            if map_parser.should_rescan() {
                tracing::debug!(
                    "[WALL_DEBUG] app: triggering background rescan (enabled={}, current_parts={})",
                    map_parser.is_enabled(), map_parser.part_count()
                );
                let workspace = self.workspace_instance.clone();
                std::thread::spawn(move || {
                    let start = std::time::Instant::now();
                    get_map_parser().scan(&workspace);
                    tracing::debug!(
                        "[WALL_DEBUG] app: background scan finished in {:.1}ms, parts={}",
                        start.elapsed().as_secs_f64() * 1000.0,
                        get_map_parser().part_count()
                    );
                });
            }
        }

        // Update hitbox expander
        self.hitbox_expander.update(&config.hitbox, &self.players_instance);
        }

        // Apply cosmetics (Korblox, Headless, Hide Face)
        if config.cosmetics.korblox || config.cosmetics.headless || config.cosmetics.hide_face {
            // Use cached model or fetch it
            let model = self.cached_cosmetics_model.clone().filter(|m| {
                // Validate cached model is still valid
                crate::core::memory::is_valid_address(m.address) && m.address != 0
            }).or_else(|| {
                // Fetch fresh model
                if let Some(local_player) = self.players_instance.find_first_child(&self.local_player_name) {
                    use crate::core::offsets::player;
                    let model_addr = self.memory.read::<u64>(local_player.address + player::model_instance());
                    if crate::core::memory::is_valid_address(model_addr) {
                        Some(Arc::new(Instance::new(model_addr, Arc::clone(&self.memory))))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            
            if let Some(model) = model {
                // Update cache
                self.cached_cosmetics_model = Some(Arc::clone(&model));
                cosmetics::apply_cosmetics_from_config(
                    &config.cosmetics,
                    &model,
                );
            }
        } else {
            // Clear cache when cosmetics disabled
            self.cached_cosmetics_model = None;
        }

        if config.aimbot.enabled {
            self.aim_assist.apply(&config, &self.local_player_name);
        }

        // Camera Aim
        if config.camera_aim.enabled {
            self.camera_aim.update(&config, &self.local_player_name);
        }

        // Viewport Aim
        if config.viewport_aim.enabled {
            // Get camera address (same method as camera_aim)
            let camera_addr = self.get_camera_address();
            self.viewport_aim.update(&config, &self.local_player_name, camera_addr);
        }

        // Silent Aim
        if config.silent_aim.enabled {
            self.silent_aim.update(&config, &self.local_player_name);
        }

        // Triggerbot - auto-fire when aim assist is locked onto a target
        if config.triggerbot.enabled && config.aimbot.enabled {
            let locked_name = self.aim_assist.get_locked_target_name().map(|s| s.to_string());
            self.triggerbot.apply(&config, locked_name.as_deref());
        }

        // Update ESP render cache with current config and aim target
        self.esp_render_cache.update_config(&config);
        self.esp_render_cache.set_local_player_name(&self.local_player_name);

        // Collect target name from whichever aim system is active (single source of truth)
        let aim_target_name: Option<String> = if config.visuals.target_highlight {
            if config.aimbot.enabled {
                self.aim_assist.get_locked_target_name().map(|s| s.to_string())
            } else if config.camera_aim.enabled {
                self.camera_aim.get_current_target_name().map(|s| s.to_string())
            } else if config.viewport_aim.enabled {
                self.viewport_aim.get_current_target_name().map(|s| s.to_string())
            } else if config.silent_aim.enabled {
                self.silent_aim.get_current_target_name().map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ref name) = aim_target_name {
            self.esp_render_cache.set_aim_target_name(name);
        } else {
            self.esp_render_cache.clear_aim_target();
        }

        // Render visuals using cached ESP data (high-performance path)
        {
        if config.visuals.box_esp || config.visuals.name_tags || config.visuals.tracers {
            Esp::render_cached(ctx, &self.esp_render_cache, &config, &self.visengine);
        }

        // Update and render footprints/trails
        if config.visuals.footprints || config.visuals.movement_trails {
            let render_data = self.esp_render_cache.get_render_data();
            self.footprint_tracker.update(&render_data);
            self.footprint_tracker.render_footprints(ctx, &config, &self.visengine);
            self.footprint_tracker.render_trails(ctx, &config, &self.visengine);
        }

        if config.visuals.chams {
            Chams::render(
                ctx,
                &self.cache,
                &self.visengine,
                &config,
                aim_target_name.as_deref(),
                &self.local_player_name,
            );
        }

        if config.visuals.mesh_chams {
            Chams::render_mesh(
                ctx,
                &self.cache,
                &self.visengine,
                &config,
                aim_target_name.as_deref(),
                &self.local_player_name,
            );
        }

        // Render hitbox visual overlay
        if config.hitbox.enabled && config.hitbox.show_visual {
            Esp::render_hitbox_visual(
                ctx,
                &self.cache,
                &self.visengine,
                &config,
                &self.local_player_name,
            );
        }

        // Render FOV circle for aimbot, camera aim, viewport aim, or silent aim
        if config.aimbot.enabled || config.camera_aim.enabled || config.viewport_aim.enabled || config.silent_aim.enabled {
            Esp::render_fov_circle(ctx, &config, &self.visengine);
        }

        // Render silent aim debug overlay
        if config.silent_aim.enabled && config.silent_aim.show_debug {
            egui::Area::new(egui::Id::new("silent_aim_debug"))
                .fixed_pos(egui::pos2(10.0, 300.0))
                .interactable(false)
                .show(ctx, |ui| {
                    self.silent_aim.render_debug(ui);
                });
        }

        // Render crosshair overlay
        if config.visuals.crosshair_style > 0 {
            Crosshair::render(ctx, &config, &self.visengine);
        }
        }

        // Sync hotkey position from config (in case user changed it via UI presets)
        if self.menu_open {
            self.hotkey_pos = egui::pos2(config.interface.hotkey_pos_x, config.interface.hotkey_pos_y);
        }

        menu::render_hotkey_hints(ctx, &mut self.hotkey_pos, self.menu_open, &mut config);

        // Menu width constant for positioning
        let menu_width = 340.0;

        if self.menu_open {
            let bb_debug = self.blade_ball.get_debug_state();
            menu::render_menu(
                ctx,
                &mut self.menu_pos,
                &mut self.menu_minimized,
                &mut self.current_tab,
                &mut config,
                &self.cache,
                &mut self.autoclicker,
                &mut self.movement_hacks,
                &self.discord_username,
                &bb_debug,
            );
            
            // Render ESP preview window (to the right of menu)
            super::esp_preview::render_esp_preview_window(
                ctx,
                &mut config,
                self.menu_pos,
                menu_width,
            );
        }

        // Render DEX Explorer window
        dex_ui::render_dex_window(ctx);

        self.autoclicker.update_recording();
        // Single write-back (previously sync+clone then update — two write locks per frame)
        self.config_manager.update(|c| *c = config);
    }
}
