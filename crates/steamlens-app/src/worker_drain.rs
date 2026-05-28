use iced::Task;

use crate::game_view;
use crate::routing;
use crate::steam_connectivity;
use crate::steam_worker::{SteamReply, SteamRequest};
use crate::{App, Message, Screen};

pub(crate) fn drain_worker_replies(app: &mut App) -> Task<Message> {
    let Some(rx) = &app.context.worker_rx else {
        return Task::none();
    };

    let replies: Vec<SteamReply> = rx.try_iter().collect();
    let mut tasks: Vec<Task<Message>> = Vec::new();

    for reply in replies {
        if let SteamReply::ConnectFailed(reason) = &reply {
            tracing::error!("worker: connect failed: {reason}");
            if matches!(app.screen, Screen::GameView(_)) {
                routing::go_back_to_profile(app);
            }
            disconnect_worker(app);
            steam_connectivity::mark_steam_offline_and_warn(app);
            return Task::none();
        }

        if let SteamReply::Connected { .. } = &reply
            && let Some(w) = &app.context.worker
        {
            w.send(SteamRequest::RequestUserStats);
            w.send(SteamRequest::RequestGlobalPercentages);
        }

        let Screen::GameView(state) = &mut app.screen else {
            continue;
        };

        let t =
            game_view::handle_steam_reply(state, reply, &mut app.context).map(Message::GameView);
        tasks.push(t);
    }

    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

pub(crate) fn disconnect_worker(app: &mut App) {
    if let Some(w) = &app.context.worker {
        w.send(SteamRequest::Disconnect);
    }
    app.context.worker = None;
    app.context.worker_rx = None;
}
