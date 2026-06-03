use iced::Task;

use crate::game_view;
use crate::routing;
use crate::steam_connectivity;
use crate::steam_worker::{SteamReply, SteamRequest};
use crate::{App, Message, Screen};

pub(crate) fn handle_worker_reply(app: &mut App, reply: SteamReply) -> Task<Message> {
    if let SteamReply::ConnectFailed(reason) = &reply {
        tracing::error!("worker: connect failed: {reason}");
        let go_back_task = if matches!(app.screen, Screen::GameView(_)) {
            routing::go_back_to_profile(app)
        } else {
            Task::none()
        };
        let disconnect_task = disconnect_worker(app);
        steam_connectivity::mark_steam_offline_and_warn(app);
        return Task::batch([go_back_task, disconnect_task]);
    }

    let mut tasks: Vec<Task<Message>> = Vec::new();

    if let SteamReply::Connected { .. } = &reply
        && let Some(w) = &app.context.worker.current
    {
        tasks.push(w.dispatch(SteamRequest::RequestUserStats, Message::DiscardReply));
        tasks.push(w.dispatch(
            SteamRequest::RequestGlobalPercentages,
            Message::DiscardReply,
        ));
    }

    let Screen::GameView(state) = &mut app.screen else {
        return Task::batch(tasks);
    };

    tasks
        .push(game_view::handle_steam_reply(state, reply, &mut app.context).map(Message::GameView));
    Task::batch(tasks)
}

pub(crate) fn disconnect_worker(app: &mut App) -> Task<Message> {
    let task = match &app.context.worker.current {
        Some(w) => w.dispatch(SteamRequest::Disconnect, Message::DiscardReply),
        None => Task::none(),
    };
    app.context.worker.current = None;
    task
}
