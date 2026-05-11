# Changelog
All notable changes to this project will be documented in this file.

## [1.0.0-alpha.8] - 2026-05-11
### ⚙️ Miscellaneous Tasks
- *(app)* Cleanup

### ⚡ Performance
- *(app)* Tighten card-only scan timeouts and add worker dispatch diagnostics
- *(app)* Replace per-app cache stat with single read_dir scan
- *(app)* Gate splash on classify

### 🐛 Bug Fixes
- *(app)* Initialize footer scan counter with cache hit count
- *(app)* Surface per-game scan failures as Error toasts with per-app retry
- *(app)* Logs during cache pipeline steps with invalidation reason enum
- *(app)* Warn on cache invalidation; info banner on schema bump
- *(app)* Hardskip Spacewar (480) and restrict log to steamlens crates only
- *(app)* Split host/worker logger init to eliminate log file race
- *(app)* Isolate worker stdin reads in a task to survive interval-tick cancellation
- *(app)* Persist GameSummaryCache during scan so subsequent runs hit cache
- *(ui)* Fixed wrong processing text in status bar
- *(app)* Use unique tmp filename in atomic_write to prevent concurrent-rename ENOENT
- *(ui)* Eliminate no-ach cache load race; tighten scan progress & ordering
- *(cache)* Add 30s grace window to last-played invalidation
- *(cache)* Invalidates period decresed to 10s
- *(genre)* Genre through worker payload
- *(scan)* Map Steam primary_genre ID to canonical name
- *(ui)* Clear-pill in filter strips; strict Hidden semantics in rarity filter
- *(ui)* Hidden pill behaves as 6th rarity tier; align clear button to right
- *(game-view)* Buffer icons and global percentages until achievements arrive
- *(game-card)* Preserve genre and tier breakdown on GoBack cache write
- *(game-view)* Preserve cache-seeded rarity_percent across AchievementsAndStats refresh
- *(game-view)* Start-align rarity pill on achievement card
- *(game-view)* Drop duplicate Hidden status filter
- *(game-view)* Drop duplicate Rarity sort option
- *(game-view)* Do not persist status filter across game opens
- *(profile-view)* Remove Rescan button and wiring
- *(game-view)* Polish Reload button as accent icon
- *(ui)* Add tooltips to header icon buttons (Reload / Settings / About)
- *(game-view)* Remove leftover loader banner and reveal-progress indicator
- *(ui)* Hide status footer when scan completes
- *(ui)* Replace SteamNotRunning full-screen with warning banner + reconnect
- *(profile-view)* Show remaining-to-100% count in "Closest to 100%" row

### 🚀 Features
- *(app)* Introduce crate::logging and migrate all eprintln! callsites
- *(app)* Implement full WorkerHandle API in worker_subprocess
- *(app)* Add GameSummaryCache and GameAchievementsCache for two-layer cache
- *(app)* Add path helpers for two-layer cache layout
- *(app)* Add two-layer summary/achievements cache I/O
- *(steam)* Activate BLoggedOn check on connect
- *(app)* Feat(app): distinguish "not logged in" from "steam not running" probe states
- *(app)* Add WorkerHandle::send_checked pre-flight API
- *(app)* Add SteamWorker::send_checked and migrate game_view callsites
- *(app)* Persist game summary on every achievements-loaded transition
- *(app)* Float loaded games above skeletons in library sort

### 🚜 Refactor
- *(app)* Consolidate app paths into ~/.steamlens via paths.rs
- *(app)* Swap hand-rolled file logger for tracing subscriber
- *(app)* Hoist write_command into ipc_pipe, remove local send helpers
- *(app)* Extract spawn_worker_child helper in progress_scan
- *(app)* Unify ConnectError and scan error envelope under WorkerProtocolError
- *(app)* Pipe and drain worker stderr; add kill_on_drop to spawn_worker_child
- *(app)* Wire WorkerHandle to progress_scan
- *(app)* Migrate bridge_loop to WorkerHandle
- *(app)* Tighten ipc_pipe visibility to pub(crate)
- *(app)* Switch cache classify to Layer 1 layout with change_number invalidation
- *(ui)* Make pill widget the single source for chips and clickable filters

## [1.0.0-alpha.7] - 2026-05-11
### ⚙️ Miscellaneous Tasks
- *(app)* Cleanup — remove dead code, prune comments
- *(ui)* Simplify build_filter_strip with iterator chain
- Release

### 🐛 Bug Fixes
- *(ui)* Align skeleton padding, spacing, and alignment with real card layouts
- *(ui)* Match skeleton avatar radius, capsule centering, and tags Fill spacer to real layouts
- *(game)* Missing icons on first game_view loading
- *(app)* Message::GoBack on GameView routes through RequestGoBack to save cache
- *(platform)* Fixes correct separator per platform
- *(core)* Correct vdf_path typo blocking compilation
- *(game)* Preserve achievement revealed state across AchievementsAndStats refresh
- *(ui)* Stretch app header full-width on all views
- *(ui)* Stop persisting search query across restarts

### 📚 Documentation
- *(release)* Release v1.0.0-alpha.7

### 🚀 Features
- *(widgets)* Add leaf primfeat(theme): introduce data-driven ThemePalette with Dark and Light variants
- *(widgets)* Add card builder and skeleton primitive
- *(widgets)* Add segmented_bar widget; migrate game card tier bar
- *(widgets)* Unify game card and profile widget bars on segmented_bar
- *(widgets)* Add pill widget
- *(widgets)* Add pill and tooltip_box widgets
- *(ui)* Adds shell module
- *(app)* Adds AppContext
- *(ui)* Unified slot-based App-level header for ProfileView and GameView
- *(ui)* Unify app header across views
- *(ui)* Add status and genre filter chips to profile view header

### 🚜 Refactor
- *(views)* Migrate game card and achievement card chrome to widgets::card
- *(ui)* Migrate game_view loaded states to ShellContent
- *(app)* Absorb ui::shell into screen module
- *(views)* Introduce ProfileViewProps and GameViewProps
- *(views)* Adopt local message types with .map() at boundary
- *(views)* Per-screen update functions with Out-Events
- *(app)* Extract inline Task::perform into command builder modules
- *(views)* Delegate per-screen subscriptions to screen modules
- *(ui)* Consolidate skeleton; eliminate magic numbers in card placeholders
- *(ui)* Align skeleton sizes with real widget dimensions
- *(ui)* Build search input in shell from SearchConfig
- *(ui)* Migrate game_view filter chrome into shell FilterStrip slots
- *(ui)* Unify segmented controls into shell SegmentedControlConfig slot
- *(ui)* Rename FilterChip to FilterButton to disambiguate from pill widget

## [1.0.0-alpha.6] - 2026-05-07
### ⚙️ Miscellaneous Tasks
- *(settings)* Remove dead default_steam_root helper
- *(cache)* Drop Default derive on NoAchievementsCache
- *(steam_state)* Drop unused exports
- *(cache)* Drop vestigial steam_last_updated field
- *(genre)* Drop stale dead_code annotation on genre field
- *(game_view)* Remove dead placeholders noise
- *(app)* Gate is_done with cfg(test)
- *(app)* Drop unused AchievementFilter::ALL const
- *(steam_worker)* Drop dead annotations and unused field
- *(game_view)* Drop 11 dead message variants superseded by SteamReply
- *(steam_worker)* Drop dead SteamReply variants and fields hidden by wholesale allow
- *(app)* Refine wholesale dead_code allows on UI severity enums
- Release

### ⚡ Performance
- *(ffi)* Reduced FFI call during cold-scan per achievement
- *(settings)* Gate SettingsFlushTick subscription on dirty state
- *(capsule_cache)* Use shared reqwest::Client with timeouts

### 🎨 Styling
- *(fmt)* Fixes fmt erros

### 🐛 Bug Fixes
- *(worker)* Async-aware sleep in worker, typed Steam ID parse, named EResult constants
- *(app)* Stop SteamNotRunning placeholder leak and centralize settings load
- *(progress_scan)* Kill_on_drop(true) so aborted scans do not leak worker subprocess

### 📚 Documentation
- *(readme)* Refresh features, friendlier disclaimer, regrouped badges
- *(release)* Release v1.0.0-alpha.6
- *(readme)* Updates badges
- *(readme)* Fixed badges spacing
- *(readme)* Section reorder

### 🚀 Features
- *(app)* Unified Ctrl+F focuses search across Profile and Game views
- *(ui)* Per-game scan failure UX via banner + footer retry button

### 🚜 Refactor
- *(ipc)* Introduce typed WorkerErrorKind instead of string
- *(client)* Split into 7 cohesive sub-types
- *(vdf)* VDF helpers
- *(core)* Centralize Steam paths and FFI string decode
- *(app)* Replace inline tempdir helpers with tempfile::TempDir
- *(timeouts)* Centralize operation timeouts into crate::timeouts
- *(worker)* Extract disconnect_worker and return_to_profile_view helpers
- *(ipc)* Extract shared read_response into crate::ipc_pipe
- *(cache)* Introduce generic Cached trait to unify load/write logic
- *(core)* Replace magic Steam community ID base with named constant
- *(vdf)* Replace magic VDF tag bytes with named TAG_* constants
- *(user_stats)* Drop Result wrapping from num_achievements
- *(game_view)* Convert from_data inherents to From<X> impls
- *(progress)* Unify ProgressData/CachedProgress under core::AchievementCountPayload
- *(cache)* Typed CachedStatValue enum replaces value_int/value_float Options

### 🧪 Testing
- *(callback)* Drop tautological tests for derived traits
- *(progress_scan)* Pin MAX_CONCURRENT at 1 with explanatory tripwire
- *(probe)* Replace subprocess-spawning flake with deterministic Read-impl tests
- *(app)* Introduce App::default() for test fixtures
- *(app)* Integration test for OpenGameView ↔ GoBack round-trip
- *(app)* Cover DrainProgressResults success/empty/failed branches
- *(ipc_pipe)* IPC framing round-trip via tokio duplex

## [1.0.0-alpha.5] - 2026-05-06
### ⚙️ Miscellaneous Tasks
- *(app)* Remove Open by App ID footer and related code paths
- Release

### 🐛 Bug Fixes
- *(profile)* Changes color for label to muted
- *(app)* Add tick marks, brighten dim ticks, raise tick row height
- *(app)* Rename UNLOCKED BREAKDOWN to ACHIEVEMENTS BREAKDOWN
- *(app)* Shrink rarity card width to free horizontal space
- *(app)* Use rule::horizontal for rarity-cards separator to match game cards
- *(app)* Pin profile-header info to top and earnings to bottom
- *(app)* Reorder rarity card content and widen card gap
- *(app)* Unify tick lit color via overall_tier_color advancing through rarity ladder
- *(app)* Use current capsule_size when looking up closest-to-100% thumbnails
- *(app)* Show earned-of-total and vertically center closest-row content
- *(app)* Round outer corners of game-card progress bar segments
- *(game-tags)* Bottom-align tags row in game card
- *(app)* Match achievement card background with game card (C_SURFACE)
- *(app)* Wrap achievement card separator with same padding as game card
- *(app)* Boost achievement card glow blur and saturation
- *(app)* Grow tags row height and card total to fit padded pills
- *(app)* Tighten pin button top and right padding

### 📚 Documentation
- *(release)* Release v1.0.0-alpha.5

### 🚀 Features
- *(app)* Introduce MessagingCenter with Banner/Footer/Toast surfaces
- *(messaging)* Autoclosable toasts
- *(profile)* Redesign profile widget with rarity-segmented progress bar
- *(app)* Redesign closest to complete widget
- *(app)* Redesign game card with rarity-segmented bar and inline counter
- *(app)* Add rarity glow to unlocked achievement cards and brighten locked text
- *(app)* Keep pin button visible on pinned game cards
- *(profile)* Wire ISteamUser023::GetPlayerSteamLevel into Profile lvl widget

## [1.0.0-alpha.4] - 2026-05-06
### ⚙️ Miscellaneous Tasks
- Release

### 🎨 Styling
- *(fmt)* Fixes formatting issues

### 🐛 Bug Fixes
- *(core)* Cfg-gate Path import + impl Send/Sync for windows ChildLifetimeGuard

### 📚 Documentation
- *(release)* Release v1.0.0-alpha.4

### 🚀 Features
- *(core)* Add cross-platform path discovery + lift linux-only gate
- *(core)* Kill-on-parent-exit job objects

### 🛠️ Build
- *(build)* Cross-platform compile matrix
- *(workflows)* Cost-optimize cross-platform matrix
- *(release)* Fail-fast on broken matrix entry

## [1.0.0-alpha.3] - 2026-05-06
### Deps
- *(libs)* Bump postcard 1 -> 1.1 version
- *(libs)* Bump libaries version and moved to workspace

### ⚙️ Miscellaneous Tasks
- *(core)* Drop all live-Steam integration tests
- *(workspace)* Comment cleanup and simplification pass
- Release

### ⚡ Performance
- *(app)* Replace progress-gated card visibility with skeleton-hydrated model
- *(app)* Cache profile avatar handle and parse localconfig once
- *(app)* Stream cache hits into state via 16ms tick instead of bulk dispatch

### 🎨 Styling
- *(profile)* Small changes to profile alignmnent
- *(app)* Cleanup code and fixes styling warnings

### 🐛 Bug Fixes
- *(app)* Sort by rarity tier within group → name
- *(app)* Show Pending badge on hidden card when toggled
- *(app)* Keep dirty toggles in their persisted filter group until Apply
- *(app)* Seal hidden achievement leak via rarity/locked filters
- *(app)* Cache entry steam_last_updated propagation; remove scanner race
- *(app)* Clear-cache UI sync, schema=0 bump, library view setting
- *(app)* Remove dead bottom loader; decouple card visibility from capsule reveal
- *(app)* Restore splash overlay; log worker failures for diagnosis
- *(app)* Splash overlay enforces 750ms minimum then waits for scan
- *(app)* Align game card skeleton with achievement skeleton style
- *(app)* Remove redundant hover tooltip from game card
- *(app)* Remove duplicate Reset button from GameView footer bar
- *(app)* Render Steam profile avatar at 2x size with PNG image fallback to initials
- *(app)* Profile widget 3:1 width ratio + responsive height + min window 896x504
- *(app)* Unify search style across views, replace GameView sort dropdown with segment toggle
- *(app)* Lock profile widget columns to fixed 290px height
- *(app)* Auto-recompute tier_breakdown for cache entries that lack it
- *(app)* Auto-dispatch RequestGlobalPercentages so achievement rarity tags populate
- *(profile)* Removed wrong name wrapping symbols
- *(app)* Scanner uses lite IPC variant to skip oversized icons

### 📚 Documentation
- *(release)* Release v1.0.0-alpha.3

### 🚀 Features
- *(app)* Subprocess worker entrypoint (--worker <app_id>)
- *(app)* 5-tier achievement rarity system (Common → Legendary) with percent badge
- *(app)* Per-game percentile tier distribution + UnlockChance default sort + visible Common/Uncommon glow
- *(app)* Subprocess-per-game cutover
- *(app)* Extend Legendary tier through ties at 3rd position
- *(core)* User profile loader + per-game achievement progress backend
- *(core)* Manifest_path + last_updated on GameSummary; steam_state readers
- *(app)* Persistent settings + atomic write + toast infrastructure
- *(app)* Per-game cache types + JSON load/write helpers
- *(app)* Cache invalidation + boot rewire
- *(app)* Profile widget + per-card progress overlay
- *(app)* Introduce SteamLens custom theme; drop Dracula default
- *(app)* Add skeleton-box primitive with shimmer gradient
- *(app)* Redesign Library top bar with segments, primary rescan, settings/about
- *(app)* Redesign GameView header with Back/title/unlocked/Reload
- *(app)* Add shortcut to focus library search
- *(app)* Redesign profile widget with rarity-stacked bar, rarity cards, closest-to-complete sifeat(app): redesign profile widget with rarity-stacked bar, rarity cards, closest-to-complete sidebar
- *(app)* Redesign GameView filter row with tabs, segments, tier chips, and action footer
- *(app)* Switch game and achievement grids to fixed-size cards with gap-responsive spacing
- *(app)* Achievement card skeleton + visual polish
- *(app)* Redesign game card with hover overlay, tier-stacked progress bar, and tags row
- *(app)* Add Completion sort + replace verbose sort labels with A–Z / LP / C + tooltips
- *(core)* Add Steam probe FFI for live profile fetch
- *(app)* Probe Steam liveness at splash, override profile with live data
- *(app)* Persistent cache fallback + Steam-off banner
- *(app)* Full per-game IPC scan replaces count-only progress fetch
- *(app)* Failed-games tracking + Retry buttons in loader strip
- *(library)* Pipe-first enumeration via packageinfo.vdf
- *(library)* Cache no-achievements app_ids by package change_number
- *(app)* Show real game genre as a card tag
- *(core)* Add shared-memory IPC primitive
- *(app)* Route large achievement payloads through shared memory
- *(app)* Route large icon updates through shared memory
- *(core)* Sweep orphan shm steamlens-* regions at startup

### 🚜 Refactor
- *(core)* IPC foundation — types, framing codec, deps for subprocess refactor
- *(app)* Replace SteamWorker thread-actor with subprocess bridge
- *(app)* Rename ProfileView, GameView; drop Splash
- *(app)* Drop RarityFilter enum; persist more details settings
- *(ipc)* Rename WorkerResponse::Hello → SteamConnected, drop dead command
- *(app)* Simplified access to game summary data
- *(ipc)* Unify data plane through shm; pipe carries only signals

### 🛠️ Build
- *(build)* Migrate ipc framing from bincode to postcard; bump reqwest+bytes for advisories

## [1.0.0-alpha.2] - 2026-05-04
### ⚙️ Miscellaneous Tasks
- Release

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
- *(release)* Release v1.0.0-alpha.2

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

