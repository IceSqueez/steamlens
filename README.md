# SteamLens

A modern desktop tool to inspect and modify Steam achievements and stats.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)
[![Language: Rust 2024](https://img.shields.io/badge/language-Rust%202024-orange)](https://www.rust-lang.org/)
[![Platform: Linux / Windows / macOS](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-brightgreen)](https://github.com/IceSqueez/steamlens)
[![Release](https://img.shields.io/github/v/release/IceSqueez/steamlens?include_prereleases&label=release&color=yellow)](https://github.com/IceSqueez/steamlens/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/IceSqueez/steamlens/total?label=downloads&color=brightgreen)](https://github.com/IceSqueez/steamlens/releases)
[![CI](https://github.com/IceSqueez/steamlens/actions/workflows/ci.yml/badge.svg)](https://github.com/IceSqueez/steamlens/actions/workflows/ci.yml)

<!-- TODO: add screenshot here -->

## ⚠️ DISCLAIMER

**SteamLens is a debug and development tool, not a casual achievement booster.** Before you use it, understand the risks:

- **Corruption risk:** Modifying achievements and stats can corrupt your Steam game progress in unpredictable ways. Some games store critical state in achievement/stat data; changes may break game logic or save integrity.
- **Steam Cloud sync:** Steam Cloud may silently overwrite your changes when the game next runs, especially if the game was in-flight or the Steam client hasn't synced yet.
- **Server-side validation:** Many games re-validate stats from server-side data or anti-cheat systems. Your changes may be reverted without warning.
- **VAC and anti-cheat:** VAC-protected games and games with server-side anti-cheat are **explicitly out of scope** for this tool. Do not attempt to use SteamLens on them.
- **No warranty:** The author and contributors are **not liable** for lost progress, banned accounts, data corruption, or any other consequences of using this tool.
- **Only on games you own:** Use SteamLens **only on games you own** and **at your own risk**. Do not use it to compromise other users' accounts or game progress.
- **Not affiliated with Valve:** SteamLens is **not affiliated with, endorsed by, or sponsored by Valve Corporation** or Steam.

**Use this tool only if you understand and accept these risks.**

## Features

- **Library view** — Browse your Steam library as a card grid. Filter by name, sort by Last Played or Name, and switch between three preset image sizes (Small, Medium, Large).
- **Manager view** — Open a game to see its achievements and stats. Toggle achievements on and off (with dirty/Apply flow), edit stats with a consent gate, bulk operations, and reset with typed-name confirmation.
- **Reactive icons** — Achievement icons auto-refresh after you click Apply, with a smooth fade-in animation.
- **Rare achievement glow** — Achievements unlocked by fewer than 10% of players get an animated gold pulsing border.
- **Inline search** — Type to search, with highlight on matching achievement names and descriptions.
- **Responsive cards** — Cards stretch to fill available width and stay legible at any size.
- **Smooth animations** — Stream-based card reveal on load, hover states on cards and buttons, and consistent animations throughout the UI.
- **Dracula theme** — Modern dark mode by default for comfortable extended use.

## Known Limitations (Alpha)

- **One game per session:** Switching games requires restarting the app (a Steamworks SDK limitation we have not yet worked around).
- **Steam Cloud may overwrite changes:** If the game was running or Steam Cloud is syncing, your changes may be silently overwritten.
- **No VAC-protected games:** This tool is explicitly not compatible with VAC-protected titles or games with server-side anti-cheat.
- **Stats edits are powerful:** A consent checkbox is shown before you can edit stats for a reason. Don't use this lightly.
- **Linux is primary:** Linux is the platform we test on most thoroughly. Windows and macOS are buildable but receive less testing in alpha.

## Installation

### From Source

```bash
git clone https://github.com/IceSqueez/steamlens.git
cd steamlens
cargo build --release
./target/release/steamlens-app
```

### System Dependencies (Linux)

On Ubuntu/Debian:

```bash
sudo apt install libwayland-dev libxkbcommon-dev libgl1-mesa-dev libx11-dev pkg-config
```

On Fedora:

```bash
sudo dnf install wayland-devel libxkbcommon-devel mesa-libGL-devel libX11-devel pkg-config
```

On Arch:

```bash
sudo pacman -S wayland libxkbcommon mesa libx11 pkg-config
```

### macOS

Ensure you have [Xcode Command Line Tools](https://developer.apple.com/download/) and optionally [Homebrew](https://brew.sh/). SteamLens will find its dependencies via Homebrew or the macOS SDK.

### Windows

No special dependencies — the MSVC toolchain and Cargo handle everything.

## Usage

1. **Start SteamLens** — Launch the app. It will splash and connect to your running Steam client.
2. **Browse your library** — See your installed games as cards. Use the search box to filter by name, and click the sort button to change the order (Last Played or Name). Press the button in the top-right of any card to switch image sizes.
3. **Open a game** — Click a game card to enter Manager view.
4. **Edit achievements and stats** — Toggle achievements by clicking on them. To edit stats, click the "Edit Stats" button, confirm the consent dialog, then modify the values. All changes are tracked in real-time.
5. **Apply or discard** — Once you've made changes, the **Apply** button appears at the bottom. Click it to write changes back to Steam, or click **Cancel** to discard them without saving.
6. **Go back** — Click the back arrow in Manager view to return to your library.

## Architecture

SteamLens is a single-binary desktop app built with **Rust 2024 edition** and the **iced** GUI framework.

The codebase is split into three crates:

- **`steamlens-core`** — Low-level Steam interop (FFI to `steamclient.so` or `.dll`, callback dispatch, schema parsing). No external types leak into its public API.
- **`steamlens-vdf`** — Binary KeyValue parser for Steam's cache format (achievements, stats schema, game metadata). Standalone crate, can be reused elsewhere.
- **`steamlens-app`** — iced GUI, app state machine (Library → Manager screens), async Tasks and Subscriptions for I/O, theming, and animations.

The runtime uses **tokio** for async work under the hood, driven by iced's built-in task system.

## Contributing

Found a bug? Have an idea for a feature? Open an issue or PR at [github.com/IceSqueez/steamlens](https://github.com/IceSqueez/steamlens).

**Commit format:** We use [Conventional Commits](https://www.conventionalcommits.org/). Format your commit message as `type(scope): short description`. Example:

```
feat(ui): add keyboard shortcut for Apply
fix(core): handle missing stat values in schema
docs(readme): update README with troubleshooting section
```

Commits are automatically processed by `git-cliff` to generate the [CHANGELOG](CHANGELOG.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
