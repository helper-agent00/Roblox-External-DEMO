//! Shared initialization logic used by both the binary and DLL entry points.

use std::io::Write;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::ConfigManager;
use crate::core::offsets::{datamodel, fake_datamodel, visual_engine};
use crate::core::Memory;
use crate::sdk::{Instance, VisualEngine};
use crate::utils::{Cache, GameId};

// ============================================================================
// Overlay Debug Flags
// ============================================================================

pub struct OverlayDebugFlags {
    #[allow(dead_code)]
    pub glow_mode1: bool,
    pub glow_mode2: bool,
    pub glow_mode3: bool,
    pub glow_mode4: bool,
    pub disable_vsync: bool,
    pub msaa_off: bool,
}

impl Default for OverlayDebugFlags {
    fn default() -> Self {
        Self {
            glow_mode1: false,
            glow_mode2: false,
            glow_mode3: false,
            glow_mode4: false,
            disable_vsync: false,
            msaa_off: false,
        }
    }
}

// ============================================================================
// Memory & Instance Initialization
// ============================================================================

pub fn init_memory(process_name: &str) -> Option<Memory> {
    let mut memory = Memory::new();

    if let Err(e) = memory.attach(process_name) {
        tracing::error!("Failed to attach to {}: {}", process_name, e);
        wait_and_exit();
        return None;
    }

    tracing::info!("Attached to {}", process_name);
    Some(memory)
}

pub fn init_game_instances(
    memory: &Arc<Memory>,
    base: u64,
) -> Option<(Arc<Instance>, Arc<VisualEngine>, Arc<Instance>, Arc<Instance>)> {
    let fake_dm = memory.read::<u64>(base + fake_datamodel::pointer());
    if fake_dm == 0 {
        tracing::error!("FakeDataModel is null");
        wait_and_exit();
        return None;
    }

    let dm_addr = memory.read::<u64>(fake_dm + fake_datamodel::real_datamodel());
    let datamodel = Arc::new(Instance::new(dm_addr, Arc::clone(memory)));

    let ve_addr = memory.read::<u64>(base + visual_engine::pointer());
    let visengine = Arc::new(VisualEngine::new(ve_addr, Arc::clone(memory)));

    let players = datamodel.find_first_child_by_class("Players")?;
    let players = Arc::new(players);

    let workspace = datamodel.find_first_child_by_class("Workspace")?;
    let workspace = Arc::new(workspace);

    Some((datamodel, visengine, players, workspace))
}

// ============================================================================
// Game Detection
// ============================================================================

/// Returns (GameId, raw_place_id).  The raw value is always logged so
/// production builds can diagnose "Unknown Game" issues.
pub fn detect_game_id(dm: &Instance, memory: &Arc<Memory>) -> (GameId, u64) {
    let offset = datamodel::place_id();
    let mut place_id = memory.read::<u64>(dm.address + offset);

    // PlaceId can momentarily read as 0 during teleport / game-load.
    // Retry once after 150 ms to handle the race.
    if place_id == 0 {
        tracing::warn!("PlaceId read as 0 (offset {:#x}) — retrying in 150ms...", offset);
        std::thread::sleep(std::time::Duration::from_millis(150));
        place_id = memory.read::<u64>(dm.address + offset);
    }

    // Always log raw value — this is the MOST important diagnostic.
    tracing::info!("DataModel PlaceId: {} (raw, offset {:#x})", place_id, offset);

    let game_id = GameId::from_place_id(place_id);

    (game_id, place_id)
}

// ============================================================================
// User Prompts
// ============================================================================

pub fn wait_and_exit() {
    println!("\nExiting in 5 seconds...");
    let _ = std::io::stdout().flush();
    thread::sleep(Duration::from_secs(5));
}

#[allow(dead_code)] // Used by lib.rs (DLL target), not by main.rs (bin target)
pub fn wait_for_input() {
    println!("\nPress Enter to continue...");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}

// ============================================================================
// Overlay Runner
// ============================================================================

pub fn run_overlay(
    cache: Arc<Cache>,
    visengine: Arc<VisualEngine>,
    config_manager: Arc<ConfigManager>,
    memory: Arc<Memory>,
    datamodel: Arc<Instance>,
    discord_username: String,
    flags: OverlayDebugFlags,
) {
    let config = config_manager.get();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&config.general.window_title)
        .with_decorations(false);

    if flags.glow_mode2 {
        viewport = viewport
            .with_transparent(false)
            .with_always_on_top()
            .with_maximized(true);
    } else {
        viewport = viewport
            .with_transparent(true)
            .with_always_on_top()
            .with_maximized(true)
            .with_mouse_passthrough(true);
    }

    let (msaa, vsync) = if flags.glow_mode3 {
        (0, false)
    } else {
        (
            if flags.msaa_off { 0 } else { 1 },
            !flags.disable_vsync,
        )
    };

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        multisampling: msaa,
        vsync,
        ..Default::default()
    };

    let glow_mode4 = flags.glow_mode4;

    match eframe::run_native(
        &config.general.window_title,
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            if let Some(gl) = cc.gl.as_ref() {
                use eframe::glow::HasContext as _;
                unsafe {
                    let vendor = gl.get_parameter_string(eframe::glow::VENDOR);
                    let renderer = gl.get_parameter_string(eframe::glow::RENDERER);
                    let version = gl.get_parameter_string(eframe::glow::VERSION);
                    tracing::info!(
                        "OpenGL Vendor: {} | Renderer: {} | Version: {}",
                        vendor,
                        renderer,
                        version
                    );
                }
            } else {
                tracing::warn!("No GL context available at creation");
            }

            Ok(Box::new(crate::render::OverlayApp::new(
                cache,
                visengine,
                config_manager,
                memory,
                datamodel,
                discord_username,
                glow_mode4,
            )))
        }),
    ) {
        Ok(_) => tracing::info!("Overlay closed normally"),
        Err(e) => {
            tracing::error!("Overlay failed to start: {}", e);
            eprintln!("\n❌ Overlay failed to start: {}", e);
            eprintln!("\nTry running with a different glow mode:");
            eprintln!("  -glow2  Non-transparent (fixes alpha issues)");
            eprintln!("  -glow3  MSAA off + VSync off (fixes NVIDIA issues)");
            eprintln!("  -glow4  WS_EX_TRANSPARENT style");
            eprintln!("\nPress Enter to exit...");
            let mut input = String::new();
            let _ = std::io::stdin().read_line(&mut input);
        }
    }
}
