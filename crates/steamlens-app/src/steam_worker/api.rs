use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iced::Task;
use tokio::sync::mpsc;

use steamlens_core::AchievementIcon;

use super::bridge::bridge_loop;

pub enum SteamRequest {
    ConnectWithApp(u32),
    RequestUserStats,
    RequestGlobalPercentages,
    ApplyChanges {
        achievements_to_set: Vec<String>,
        achievements_to_clear: Vec<String>,
        stats_int: HashMap<String, i32>,
        stats_float: HashMap<String, f32>,
    },
}

#[derive(Debug, Clone)]
pub enum SteamReply {
    Connected {
        app_name: Option<String>,
    },
    ConnectFailed(String),
    AchievementsFull {
        achievements: Vec<steamlens_core::AchievementData>,
        stats: Vec<steamlens_core::StatData>,
    },
    LoadFailed(String),
    ChangesSaved,
    SaveFailed(String),
    IconUpdated {
        name: String,
        icon: AchievementIcon,
    },
    GlobalPercentagesReady(HashMap<String, f32>),
    GlobalPercentagesFailed,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct WorkerReply {
    pub app_id: u32,
    pub reply: SteamReply,
}

pub type SharedWorkerReplyReceiver = Arc<Mutex<Option<mpsc::UnboundedReceiver<WorkerReply>>>>;

static NEXT_WORKER_GENERATION: AtomicU64 = AtomicU64::new(1);

pub struct SteamWorker {
    request_sender: mpsc::UnboundedSender<SteamRequest>,
    reply_receiver: SharedWorkerReplyReceiver,
    generation: u64,
}

impl SteamWorker {
    pub fn spawn() -> Self {
        let (request_sender, request_receiver) = mpsc::unbounded_channel::<SteamRequest>();
        let (reply_sender, reply_receiver) = mpsc::unbounded_channel::<WorkerReply>();
        let generation = NEXT_WORKER_GENERATION.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(bridge_loop(request_receiver, reply_sender));
        SteamWorker {
            request_sender,
            reply_receiver: Arc::new(Mutex::new(Some(reply_receiver))),
            generation,
        }
    }

    pub fn reply_receiver(&self) -> SharedWorkerReplyReceiver {
        Arc::clone(&self.reply_receiver)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn dispatch<M: 'static + Send>(&self, request: SteamRequest, message: M) -> Task<M> {
        let weak_sender = self.request_sender.downgrade();
        Task::future(async move {
            if let Some(sender) = weak_sender.upgrade() {
                let _ = sender.send(request);
            }
            message
        })
    }

    pub fn dispatch_checked<M: 'static + Send>(
        &self,
        request: SteamRequest,
        steam_running: bool,
        user_logged_in: bool,
        message: M,
    ) -> Result<Task<M>, crate::worker_subprocess::ConnectivityError> {
        crate::worker_subprocess::preflight(steam_running, user_logged_in)?;
        Ok(self.dispatch(request, message))
    }
}
