# SteamLens

A modern desktop tool to inspect and modify Steam achievements and stats.

[![CI](https://github.com/IceSqueez/steamlens/actions/workflows/ci.yml/badge.svg)](https://github.com/IceSqueez/steamlens/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/IceSqueez/steamlens?include_prereleases)](https://github.com/IceSqueez/steamlens/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/IceSqueez/steamlens/total)](https://github.com/IceSqueez/steamlens/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)

[![Rust 2024](https://img.shields.io/badge/rust-2024-B7410E?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?logo=linux&logoColor=black)](https://github.com/IceSqueez/steamlens/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-000000?logo=apple)](https://github.com/IceSqueez/steamlens/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-0078D6)](https://github.com/IceSqueez/steamlens/releases/latest)

<!-- TODO: add screenshot here -->

## Notes & Caveats

A few things worth knowing before you start:

- **Single-player and personal use.** SteamLens is for inspecting and tweaking achievements on games you own. Think of it as a tool for tinkerers and curious players — not a competitive cheat utility.
- **Not for VAC or server-side anti-cheat games.** Multiplayer titles protected by VAC or server-side validation are out of scope. The server has the final say there, and your changes may simply be ignored.
- **Steam Cloud can overwrite your changes.** If the game was running or Cloud is syncing, the cloud copy can win. Quit the game first, give Cloud a moment to settle, then edit.
- **Some games store progression in stats.** Editing stats can affect quest progress, item unlocks, or save state. The consent checkbox before a stats edit is there for that reason.
- **No warranty.** SteamLens is provided as-is. The author and contributors aren't liable for lost progress, data corruption, or other consequences of running the tool.
- **Not affiliated with Valve.** SteamLens is an independent open-source project — no endorsement by Valve Corporation or Steam.

## Features

### Library (Profile view)

- **Card grid** with capsule artwork — switch between three preset image sizes (Small / Medium / Large) and the layout reflows to fit.
- **Live search** filters games by name as you type.
- **Three sort modes** — Last Played, Name (A→Z), Completion %.
- **Per-card rarity breakdown** — a small bar shows the tiers of achievements you've unlocked for each game; hover for counts.
- **Steam level** is shown next to your persona name in the header.
- **Failed-scan recovery** — if individual games fail to scan, a _Retry_ button appears in the status footer that re-queues just the failed apps.

### Game view

- **Achievement cards** with rarity tiers (Common / Uncommon / Rare / Very Rare / Ultra Rare), rounded segments, and unlock-time stamps.
- **Rare-glow animation** — achievements unlocked by fewer than 10% of players get a soft animated gold border.
- **Search** with inline highlighting on names and descriptions.
- **Filters** — by status (All / Unlocked / Locked / Hidden) and by rarity tier; combine them freely.
- **Sort** — by unlock chance, by rarity tier, or alphabetically.
- **Toggle achievements** with a dirty-state Apply flow; bulk operations (unlock all / lock all / invert).
- **Stats editor** behind a consent gate. Increment-only stats are detected and validated.
- **Reset with typed-name confirmation** so you don't reset Civilization VI by accident when you meant Vampire Survivors.
- **Reactive icons** — achievement icons fade in as Steam delivers them, with a smooth animation.

### General

- **Modern dark theme** (Dracula by default) for comfortable extended use.
- **Smooth animations** throughout — card reveals on load, hover states, skeleton placeholders during hydration.

## Known Limitations (Alpha)

- **Linux is the primary test platform.** macOS and Windows binaries build green on CI; runtime testing on those platforms is lighter during alpha.

## Installation

### From source

```bash
git clone https://github.com/IceSqueez/steamlens.git
cd steamlens
cargo build --release
./target/release/steamlens-app
```

### System dependencies (Linux)

On Ubuntu / Debian:

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

Make sure you have [Xcode Command Line Tools](https://developer.apple.com/download/) installed; [Homebrew](https://brew.sh/) is optional. Cargo will pick up dependencies via the macOS SDK.

### Windows

No special dependencies — the MSVC toolchain plus Cargo handle the rest.

## Usage

1. **Start SteamLens.** Launch the binary; it splashes and connects to your running Steam client.
2. **Browse your library.** Search by name, change sort order, or switch capsule sizes from the top-right card menu.
3. **Open a game.** Click a card to enter the game view. Skeleton placeholders fade out as cached data is hydrated, then refreshed once Steam responds.
4. **Edit achievements.** Click any achievement to toggle it. Changes are tracked locally until you Apply.
5. **Edit stats** by enabling the consent checkbox, then editing values inline. Increment-only stats reject decreases.
6. **Apply or discard.** When there are dirty changes, an Apply button appears at the bottom. Click it to write to Steam, or Cancel to drop the changes.
7. **Go back.** The back arrow returns you to the library; the game-view state is cached for next time.

## Architecture

SteamLens is a single-binary desktop app built with **Rust 2024 edition** and the **iced** GUI framework.

The codebase is split into three crates:

- **`steamlens-core`** — Low-level Steam interop (FFI to `steamclient.so` / `.dll`, callback dispatch, schema parsing). No external types leak into its public API.
- **`steamlens-vdf`** — Binary KeyValue parser for Steam's cache format (achievements, stats schema, game metadata). Standalone — can be reused elsewhere.
- **`steamlens-app`** — iced GUI, app state machine, async tasks and subscriptions, theming, and animations.

The runtime uses **tokio** for async work under the hood, driven by iced's task system. Steam interop runs in a worker subprocess (`--worker $APP_ID`) so the main process never has to outlive a Steam pipe handshake.

## Contributing

Found a bug? Have an idea for a feature? Open an issue or PR at [github.com/IceSqueez/steamlens](https://github.com/IceSqueez/steamlens).

**Commit format.** We use [Conventional Commits](https://www.conventionalcommits.org/). Format your commit message as `type(scope): short description`. Example:

```
feat(ui): add keyboard shortcut for Apply
fix(core): handle missing stat values in schema
docs(readme): update README with troubleshooting section
```

Commits are processed by `git-cliff` to generate the [CHANGELOG](CHANGELOG.md) automatically.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
