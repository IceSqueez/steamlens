# Changelog
All notable changes to this project will be documented in this file.

## [1.0.0-alpha.2] - 2026-05-04
### 🎨 Styling
- *(app)* Add subtle card shadow for 3D depth
- *(app)* Card image shadows + Library card separator

### 🐛 Bug Fixes
- *(app)* Sort uses persisted is_achieved, not dirty-aware effective_achieved
- *(app)* Wrap long card names and pin status rows to card bottom
- *(app)* Tighten Library card name block (smaller gap, bottom-left aligned)
- *(app)* Library card top padding + capsule URL fallback chain
- *(app)* Tighten achievement card height + add separator above status row
- *(card)* Slightly increase padding for separator
- *(app)* Preserve Library state across Manager navigation
- *(app)* Keep revealed hidden achievements in Hidden group
- *(core)* Replace with slice fixes analyzer warning

### 📚 Documentation
- *(readme)* Updates with disclaimer, badges, features, and known limitations

### 🚀 Features
- *(app)* Achievement view as responsive card grid with width slider
- *(app)* Uniform card sizing + achievement card horizontal layout
- *(app)* Library preset image sizes instead of slider
- *(app)* Fade-in animation for Library capsules on load
- *(app)* Stream-based card reveal (queue + tick)
- *(app)* Loading indicator with spinner + counter for stream reveal
- *(app)* Hover accent border on Library and Achievement cards
- *(app)* Full-card hover state — brighter bg, lifted shadow, accent border
- *(app)* Hover/focus styles for control buttons in Manager view
- *(app)* Lock other Library cards once a game is opened
- *(app)* In-memory capsule handle cache (instant size-switch restore)
- *(app)* Typed game-name confirmation gate on reset stats
- *(app)* Responsive cards stretch to fill width
- *(app)* Inline search highlight on achievement card name + description
- *(app)* Rare achievement glow with pulse animation

## [1.0.0-alpha.1] - 2026-05-04
### Bump
- Version 0.1.0 → 0.2.0
- Version 0.2.0 → 0.3.0-alpha.0+build.1

### ⚙️ Miscellaneous Tasks
- *(deps)* Bump iced dependency version
- *(license)* Switch to MIT OR Apache-2.0 dual license
- *(core)* Drop probe examples
- *(vdf)* Small polish
- *(core)* Apply Round 2 QA polish (non_exhaustive, SAFETY wording, boundary tests)
- *(achiev)* Remove sleeps and logs
- *(proj)* Centralize all dependencies in workspace.dependencies
- *(app)* Moves method before tests to dismiss diagnostics warnings
- *(release)* Adds "main" as allowed branch for releases
- Release
- *(release)* Single workspace tag - v{version}
- *(cliff)* Handle Initial commit and remote-tracking merges

### 🐛 Bug Fixes
- *(ci)* Updates github actions
- *(app)* Add connect timeout and inflight Retry guard
- *(core)* Use ResetAllStats for full reset (achievements + underlying stats)
- *(app)* Worker polls callbacks internally after RequestUserStats
- *(app)* Defer Steam library load until app picker
- *(app)* Cancel button only discards changes
- *(app)* Refresh icons after Apply + hide hidden achievements until revealed

### 📚 Documentation
- *(core)* Expand SAFETY invariants on UserStats unsafe blocks
- *(release)* Release v1.0.0-alpha.1

### 🚀 Features
- Init commit
- *(git)* Added cz.toml and pre-commit hook
- *(ui)* Added basic ui for
- *(ci)* Adds github actions
- *(sln)* Updates gitignore
- *(cliff)* Adds git cliff and release configs
- *(core)* Add ISteamClient018 + ISteamUser012 FFI baseline
- *(core)* Add Steam callback poller infrastructure
- *(vdf)* Add binary KeyValue parser
- *(core)* Add ISteamUserStats013 sync wrappers
- *(core)* Add RequestUserStats and typed callback decoding
- *(app)* Wire steamlens-core into iced with splash → connect → status flow
- *(core)* Add Client::connect(app_id) for SteamAppId env injection
- *(app)* Add Manager view with SteamWorker actor for persistent Client
- *(app)* Add splash screen and reset scope selection modal
- *(core)* Add Client::stat_descriptors for VDF schema parsing
- *(app)* Wire stat_descriptors into worker and Stats tab
- *(core)* Wrap ISteamApps001::GetAppData and add Client::app_name()
- *(core)* Wrap ISteamUtils005 and add Client::get_image for achievement icons
- *(app)* Render achievement icons via Client::get_image
- *(app)* Reactive achievement icon refresh via UserAchievementIconFetched
- *(app)* Sort achievements unlocked → locked → hidden by display name
- *(core)* Library scan backend (text VDF parser + GameSummary + ScanLibrary worker request)
- *(app)* Library screen with card grid, capsule pipeline

### 🛠️ Build
- *(github)* Switch CI branches to main

### 🧪 Testing
- *(vdf)* Switch to synthetic test bytes
- *(core)* Add !Send compile-time assertion and SteamError display unit tests
- *(core)* Add reset_achievements integration tests with destructive opt-in
- *(app)* Add update() state-machine unit tests

