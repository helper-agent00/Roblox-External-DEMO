// NEXUS - Roblox External Overlay
//! Edit `config.toml` to customize settings, or use the in-game menu.

mod app_init;
mod config;
mod core;
mod features;
mod render;
mod sdk;
mod utils;

use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::env;
use std::io::Write;
use std::sync::Arc;

use crate::app_init::OverlayDebugFlags;
use crate::config::ConfigManager;
use crate::core::offset_loader;
use crate::utils::Cache;

const CURRENT_VERSION: &str = "1.0.0";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let has_arg = |flags: &[&str]| args.iter().any(|a| flags.iter().any(|f| a.eq_ignore_ascii_case(f)));

    let use_syscalls = has_arg(&["-syscall", "--syscall"]);
    let glow_mode1 = has_arg(&["-glow1"]);
    let glow_mode2 = has_arg(&["-glow2"]);
    let glow_mode3 = has_arg(&["-glow3"]);
    let glow_mode4 = has_arg(&["-glow4"]);
    let disable_vsync = has_arg(&["-novsync"]);
    let msaa_off = has_arg(&["-nomsa", "-msaa0"]);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nexus=info".parse().unwrap())
                .add_directive("chromiumoxide=off".parse().unwrap())
                .add_directive("tungstenite=off".parse().unwrap()),
        )
        .with_target(false)
        .init();

    println!("╔════════════════════════════════════════════════╗");
    println!("║  ███╗   ██╗███████╗██╗  ██╗██╗   ██╗███████╗   ║");
    println!("║  ████╗  ██║██╔════╝╚██╗██╔╝██║   ██║██╔════╝   ║");
    println!("║  ██╔██╗ ██║█████╗   ╚███╔╝ ██║   ██║███████╗   ║");
    println!("║  ██║╚██╗██║██╔══╝   ██╔██╗ ██║   ██║╚════██║   ║");
    println!("║  ██║ ╚████║███████╗██╔╝ ██╗╚██████╔╝███████║   ║");
    println!("║  ╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ║");
    println!("║             DEMO - v{}                       ║", CURRENT_VERSION);
    println!("║          Developed by NexusUnderground         ║");
    println!("║         'My crime is that of curiosity'        ║");
    println!("╚════════════════════════════════════════════════╝\n");

    if use_syscalls {
        crate::core::memory::enable_syscalls();
    }

    tracing::info!("Loading dynamic offsets...");
    if let Err(e) = offset_loader::initialize_offsets().await {
        eprintln!("\n❌ Failed to load offsets: {}", e);
        eprintln!("\nMake sure:");
        eprintln!("  1. Roblox is running");
        eprintln!("  2. You have internet connection");
        eprintln!("  3. The detected version has offsets available");
        eprintln!("\nPress Enter to exit...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        std::process::exit(1);
    }

    let discord_username = "Nexus-User".to_string();

    let config_manager = Arc::new(ConfigManager::new());
    let config = config_manager.get();

    tracing::info!("Configuration loaded");
    tracing::info!("Username: {}", config.general.username);

    if config.general.username == "YourUsernameHere" || config.general.username.trim().is_empty() {
        println!("\n╔══════════════════════════════════════╗");
        println!("║     ⚠  USERNAME NOT SET  ⚠           ║");
        println!("╚══════════════════════════════════════╝");
        println!("\n  Nexus needs your ROBLOX USERNAME (not display name)");
        println!("  to identify your character in-game.\n");
        println!("  You can find it at: https://www.roblox.com/users/profile");
        println!("  It's the name in the URL or under your display name.\n");
        
        loop {
            print!("  Enter your Roblox username: ");
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let username = input.trim().to_string();
            
            if username.is_empty() {
                println!("  ❌ Username cannot be empty. Please try again.");
                continue;
            }
            if username.eq_ignore_ascii_case("YourUsernameHere") {
                println!("  ❌ Please enter your actual Roblox username.");
                continue;
            }
            if username.len() < 3 || username.len() > 20 {
                println!("  ❌ Roblox usernames are 3-20 characters. Please try again.");
                continue;
            }
            
            config_manager.update(|c| {
                c.general.username = username.clone();
            });
            if let Err(e) = config_manager.save() {
                tracing::error!("Failed to save config: {}", e);
                eprintln!("  ⚠ Could not save to config.toml: {}", e);
            } else {
                println!("\n  ✓ Username set to '{}' and saved to config.toml", username);
            }
            break;
        }
        
        let config = config_manager.get();
        println!("  ✓ Continuing with username: {}\n", config.general.username);
    }

    let memory = match app_init::init_memory(&config.general.process_name) {
        Some(m) => Arc::new(m),
        None => return,
    };

    let base = memory.base_address();

    let (datamodel, visengine, players, workspace) =
        match app_init::init_game_instances(&memory, base) {
            Some(instances) => instances,
            None => return,
        };

    let (game_id, raw_place_id) = app_init::detect_game_id(&datamodel, &memory);
    tracing::info!("Detected game: {} (raw PlaceId: {})", game_id.name(), raw_place_id);

    let cache = Arc::new(Cache::new());
    cache.set_game_id(game_id);
    cache.start(
        Arc::clone(&players),
        Arc::clone(&workspace),
        Arc::clone(&memory),
        config.performance.cache_update_ms,
    );

    tracing::info!("Cache thread started");
    tracing::info!("Starting overlay...\n");

    app_init::run_overlay(
        cache,
        visengine,
        config_manager,
        memory,
        datamodel,
        discord_username,
        OverlayDebugFlags {
            glow_mode1,
            glow_mode2,
            glow_mode3,
            glow_mode4,
            disable_vsync,
            msaa_off,
        },
    );
}
