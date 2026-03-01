# Nexus Underground — Demo

An external Roblox overlay built in Rust with a modern [egui](https://github.com/emilk/egui) interface.  
This is the **free demo** — premium features are locked. Join the Discord for full access.

[![Discord](https://img.shields.io/badge/Discord-Nexus%20Underground-5865F2?logo=discord&logoColor=white)](https://tr.ee/NexusD)

---

## Features

### Visuals
| Feature | Demo | Premium |
|---|---|---|
| Box ESP (2D bounding boxes, distance colours) | ✅ | ✅ |
| Name Tags + Distance | ✅ | ✅ |
| Tracers | ✅ | ✅ |
| Health & Armor Bars | ✅ | ✅ |
| Team / Target Highlighting | ✅ | ✅ |
| Dead-Player Filter | ✅ | ✅ |
| Wall Check (visibility) | ❌ | ✅ |
| Footprints & Movement Trails | ✅ | ✅ |
| Chams (3D player fill) | ✅ | ✅ |
| Mesh Chams | ✅ | ✅ |

### Aimbot
| Feature | Demo | Premium |
|---|---|---|
| Aim Assist (FOV, smoothing, prediction) | ✅ | ✅ |
| Camera Aim | ✅ | ✅ |
| Viewport Aim | ✅ | ✅ |
| Triggerbot | ❌ | ✅ |
| Auto Reload | ✅ | ✅ |
| Mouse Silent Aim | ❌ | ✅ |

### Movement
| Feature | Demo | Premium |
|---|---|---|
| Walk Speed | ✅ | ✅ |
| Jump Power | ✅ | ✅ |
| Auto-Jump | ✅ | ✅ |
| Spinbot | ❌ | ✅ |
| Noclip | ✅ | ✅ |
| Click Teleport | ❌ | ✅ |
| Waypoint | ❌ | ✅ |
| Anchor | ❌ | ✅ |
| No Fall Damage | ❌ | ✅ |
| Hip Height | ✅ | ✅ |
| Void Hide | ❌ | ✅ |
| Free Camera | ✅ | ✅ |
| Fly | ❌ | ✅ |
| Vehicle Fly | ❌ | ✅ |
| Spiderman | ❌ | ✅ |

### World
| Feature | Demo | Premium |
|---|---|---|
| FOV Changer | ✅ | ✅ |
| Fullbright | ✅ | ✅ |
| Anti-Fog | ✅ | ✅ |
| No Shadows | ✅ | ✅ |
| Brightness Control | ✅ | ✅ |
| Anti-Flash | ✅ | ✅ |
| Force Lighting | ✅ | ✅ |
| Ambient Control | ✅ | ✅ |

### Combat & Misc
| Feature | Demo | Premium |
|---|---|---|
| Anti-AFK | ✅ | ✅ |
| Auto-Clicker | ✅ | ✅ |
| Blade Ball Auto-Parry | ❌ | ✅ |
| Hitbox Expander | ❌ | ✅ |
| Desync | ❌ | ✅ |
| Cosmetics (Korblox / Headless / Hide Face) | ❌ | ✅ |
| DEX Explorer | ❌ | ✅ |

### Game Support
Nexus auto-detects the game and applies tuned settings for:
- **Phantom Forces** — dedicated entity scanner
- **Da Hood / Hood Modded** — mouse aim support
- **RIVALS** — viewport aim support
- **Blade Ball** — auto-parry system
- **Operation One** — game-specific aim tuning
- **Fallen / Aftermath** — dedicated entity scanner
- **Blox Strike** — custom support
- **Generic** — works on any Roblox experience

---

## Requirements

- **Windows 10 / 11**
- **~6 GB** free disk space (first build)
- Internet connection (for offsets & Cloudflare bypass)

---

## Quick Start

### 1. Install Rust

> Video guide: <https://youtu.be/z1r9JIRpepk>

1. Download **rustup-init.exe** from <https://rust-lang.org/tools/install/>
2. Run and press **Enter** to accept defaults
3. **Restart your PC**

Verify:
```powershell
rustc --version
```

### 2. Install VS Code *(optional, recommended)*

Download from <https://code.visualstudio.com/download> and install the **rust-analyzer** extension.

### 3. Download & Extract

Download the source from the releases page and extract to a folder, e.g. `C:\Nexus\`.

### 4. Build

Open a terminal in the extracted folder (the one containing `Cargo.toml`):

```powershell
cargo build --release
```

> First build takes **5–10 minutes** — this is normal.

### 5. Run

```powershell
cargo run --release
```

Or launch `target\release\nexus.exe` directly.

---

## NVIDIA / GPU Overlay Fixes

If you see a **black overlay** or click-through isn't working, try these flags:

| Flag | What it does | When to use |
|------|-------------|-------------|
| `-glow1` | Default transparent overlay | Most systems |
| `-glow2` | Non-transparent full-size overlay | Diagnose alpha issues |
| `-glow3` | Transparent + MSAA off + VSync off | **Fixes most NVIDIA issues** |
| `-glow4` | Adds `WS_EX_TRANSPARENT` style | Extra click-through reliability |
| `-novsync` | Disable VSync *(combinable)* | Overlay flickers |
| `-msaa0` | Disable MSAA *(combinable)* | Overlay is black |

**Recommended for NVIDIA:**
```powershell
cargo run --release -- -glow3
```

**Still black?**
```powershell
cargo run --release -- -glow3 -glow4
```

---

## Configuration

On first run Nexus creates `config.toml` next to the executable.  
**Set your Roblox username** so the overlay can identify your character — you'll be prompted on first launch.

Settings can be changed in-game via the menu (**F1**) or by editing `config.toml` directly.

---

## Default Hotkeys

| Key | Action |
|-----|--------|
| **F1** | Toggle menu |
| **F2** | Box ESP |
| **F3** | Chams |
| **F4** | Aim Assist |
| **F5** | Camera Aim |
| **F6** | Fly *(premium)* |
| **F7** | Tracers |
| **F8** | Noclip |
| **F9** | Hitbox Mod *(premium)* |
| **Insert** | Spinbot *(premium)* |
| **Home** | Refresh game data |
| **End** | Save config |
| **F12** | Exit |

All hotkeys are fully remappable in the menu.

---

## Get Premium

The full version includes every feature unlocked, priority support, and early access to updates.

**Discord:** <https://tr.ee/NexusD>

---

## License

For educational and research purposes only.

---

*"My crime is that of curiosity"* — Nexus Underground


