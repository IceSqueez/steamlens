use iced::Task;

use crate::game_view::{self, GameViewEvent, GameViewMessage, GameViewState};
use crate::profile_view::types::ProfileViewState;
use crate::steam_worker::{SteamRequest, SteamWorker};
use crate::{App, Message, Screen, cache, capsule_cache};

pub(crate) fn current_pv_state_mut<'a>(
    screen: &'a mut Screen,
    preserved: &'a mut Option<Box<ProfileViewState>>,
) -> &'a mut ProfileViewState {
    match screen {
        Screen::ProfileView(state) => state.as_mut(),
        Screen::GameView(_) => preserved
            .as_mut()
            .expect("GameView screen must have preserved profile state")
            .as_mut(),
    }
}

pub(crate) fn dispatch_game_event(
    app: &mut App,
    task: Task<Message>,
    event: GameViewEvent,
) -> Task<Message> {
    match event {
        GameViewEvent::None => task,
        GameViewEvent::AchievementsFullyLoaded { app_id } => {
            Task::batch([task, Task::done(Message::PersistGameSummary(app_id))])
        }
        GameViewEvent::GoBack => Task::batch([task, go_back_to_profile(app)]),
        GameViewEvent::InvalidateCache { app_id } => {
            Task::batch([task, Task::done(Message::InvalidateGameCache(app_id))])
        }
    }
}

pub(crate) fn go_back_to_profile(app: &mut App) -> Task<Message> {
    let disconnect_task = crate::worker_drain::disconnect_worker(app);
    if let Some(mut prev) = app.preserved_profile_state.take() {
        prev.search.clear();
        app.screen = Screen::ProfileView(prev);
    }
    disconnect_task
}

pub(crate) fn open_game_view(app: &mut App, app_id: u32) -> Task<Message> {
    let prev = if let Screen::ProfileView(pv_state) = std::mem::replace(
        &mut app.screen,
        Screen::ProfileView(Box::new(ProfileViewState::new())),
    ) {
        pv_state
    } else {
        Box::new(ProfileViewState::new())
    };
    app.preserved_profile_state = Some(prev);

    let steam_off = app.context.connectivity.steam_running == Some(false);

    let mut state = GameViewState::new(app_id);
    state.achievement_sort = app.context.settings.manager.sort;
    state.rarity_tier_set = app
        .context
        .settings
        .manager
        .rarity_tiers
        .iter()
        .copied()
        .collect();
    state.include_hidden = app.context.settings.manager.include_hidden;
    state.unlocked_at_top = app.context.settings.manager.unlocked_at_top;

    let mut tasks: Vec<Task<Message>> = Vec::new();

    if !app.context.user.steam_root.as_os_str().is_empty() {
        tasks.push(crate::boot::spawn_steam_state_refresh(
            app.context.user.steam_root.clone(),
            app.context.user.steamid3,
            app.context.steam.app_state_mtime,
        ));
    }

    if let Some(cached) = app.context.game_cache.entries.get(&app_id) {
        state.expected_total = cached.progress.total;
        state.genre = cached.genre.clone();
        state.playtime_minutes = cached.playtime_minutes;
        if !cached.achievements.is_empty() {
            tasks.push(crate::game_cache_builder::spawn_seed_task(
                app_id,
                cached.clone(),
            ));
        }
    }
    if state.playtime_minutes.is_none() {
        state.playtime_minutes = app
            .context
            .steam
            .app_state
            .get(&app_id)
            .and_then(|s| s.playtime_minutes);
    }

    if steam_off {
        if state.achievements.is_empty() && !app.context.game_cache.entries.contains_key(&app_id) {
            let steamid3 = app.context.user.steamid3;
            tasks.push(Task::perform(
                cache::store::load_game_cache(steamid3, app_id),
                move |entry| {
                    Message::Cache(cache::CacheEvent::OfflineLoaded {
                        app_id,
                        entry: entry.map(Box::new),
                    })
                },
            ));
        }
        tasks.push(crate::worker_drain::disconnect_worker(app));
        state.cache_only = true;
        state.phase = game_view::GameViewPhase::Ready;
    } else {
        tasks.push(crate::worker_drain::disconnect_worker(app));
        let worker = SteamWorker::spawn(app.context.worker.reply_tx.clone());
        tasks.push(worker.dispatch(SteamRequest::ConnectWithApp(app_id), Message::DiscardReply));
        app.context.worker.current = Some(worker);
    }

    let portrait_assets = app
        .context
        .steam
        .library_assets
        .get(&app_id)
        .cloned()
        .unwrap_or_default();
    tasks.push(Task::perform(
        capsule_cache::fetch_capsule(
            app_id,
            capsule_cache::CapsuleSize::Portrait,
            portrait_assets,
        ),
        move |result| match result {
            Ok((size, pixels)) => {
                let handle = iced::widget::image::Handle::from_rgba(
                    pixels.width,
                    pixels.height,
                    pixels.rgba,
                );
                Message::GameView(GameViewMessage::CapsuleLoaded {
                    app_id,
                    size,
                    handle,
                    width: pixels.width,
                    height: pixels.height,
                })
            }
            Err((size, _)) => Message::GameView(GameViewMessage::CapsuleFailed { app_id, size }),
        },
    ));

    app.screen = Screen::GameView(Box::new(state));

    Task::batch(tasks)
}
