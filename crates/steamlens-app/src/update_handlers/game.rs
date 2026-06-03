use std::time::Instant;

use iced::Task;

use crate::game_view::{self, GameViewMessage};
use crate::{App, Message, Screen, routing};

pub(crate) fn handle_game_view_message(app: &mut App, m: GameViewMessage) -> Task<Message> {
    let Screen::GameView(state) = &mut app.screen else {
        #[cfg(debug_assertions)]
        tracing::warn!("dropped stale GameView message: {m:?} (current screen: not GameView)");
        return Task::none();
    };

    let (task, event) = game_view::update(state, m, &mut app.context);
    let task = task.map(Message::GameView);
    routing::dispatch_game_event(app, task, event)
}

pub(crate) fn handle_animation_frame(app: &mut App, now: Instant) -> Task<Message> {
    const SKELETON_PER_SEC: f32 = 0.6;

    let delta_secs = now
        .saturating_duration_since(app.context.animation.last_tick)
        .as_secs_f32()
        .min(0.1);
    app.context.animation.last_tick = now;

    app.context.animation.skeleton_phase =
        (app.context.animation.skeleton_phase + SKELETON_PER_SEC * delta_secs).rem_euclid(1.0);

    let steam_running = app.context.connectivity.steam_running;
    match &mut app.screen {
        Screen::GameView(state) => state.tick_animations(delta_secs),
        Screen::ProfileView(state) => state.tick_animations(delta_secs, steam_running),
    }
    Task::none()
}

pub(crate) fn needs_animation_frame(app: &App) -> bool {
    if crate::splash::has_active_skeletons(app) {
        return true;
    }
    match &app.screen {
        Screen::GameView(state) => state.has_active_animations(),
        Screen::ProfileView(state) => {
            state.has_active_animations(app.context.connectivity.steam_running)
        }
    }
}
