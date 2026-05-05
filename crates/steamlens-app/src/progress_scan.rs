use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};
use steamlens_core::{AchievementData, StatData};

const MAX_CONCURRENT: usize = 3;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
const PERCENTAGES_TIMEOUT: Duration = Duration::from_secs(15);

/// Per-game progress counts derived from a scan, used by the UI to render
/// counters before the full cache entry is materialised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressData {
    pub earned: u32,
    pub total: u32,
}

/// Full per-game payload returned by a successful scan.
///
/// Combines the achievement+stat list (from `LoadAchievementsAndStats`) with
/// the global rarity percentages (from `RequestGlobalPercentages`) plus the
/// game's display name from the worker's `Hello` frame.
#[derive(Debug, Clone)]
pub struct ScannedGameData {
    pub app_name: Option<String>,
    pub achievements: Vec<AchievementData>,
    pub stats: Vec<StatData>,
    /// Map of `api_name` → percentage of all owners who unlocked this
    /// achievement. Empty when Steam declined to provide percentages
    /// (treat as missing rarity data; the cache entry will have an empty
    /// `tier_breakdown`).
    pub global_percentages: HashMap<String, f32>,
}

impl ScannedGameData {
    pub fn earned_count(&self) -> u32 {
        self.achievements.iter().filter(|a| a.is_achieved).count() as u32
    }

    pub fn total_count(&self) -> u32 {
        self.achievements.len() as u32
    }
}

/// Result of a single game's scan, success or failure.
#[derive(Debug, Clone)]
pub struct ProgressResult {
    pub app_id: u32,
    /// `None` when the worker child failed, timed out, or returned an
    /// `Error` frame for any of the three protocol stages.
    pub data: Option<ScannedGameData>,
}

/// Streams full per-game scan results in a bounded concurrent manner.
///
/// Spawns up to `MAX_CONCURRENT` (3) child worker processes at once. Each
/// child connects to Steam, fetches achievements + stats + global rarity
/// percentages, then exits. As workers finish, the next game from the queue
/// is started. Results arrive via the tokio channel returned from
/// [`ProgressScanner::take_receiver`].
///
/// Drop the scanner to cancel all in-flight workers (their `Child` handles
/// are killed in `Drop`).
pub struct ProgressScanner {
    queue: VecDeque<u32>,
    in_flight: Vec<tokio::task::JoinHandle<ProgressResult>>,
    result_tx: tokio::sync::mpsc::UnboundedSender<ProgressResult>,
    result_rx: Option<tokio::sync::mpsc::UnboundedReceiver<ProgressResult>>,
}

impl ProgressScanner {
    pub fn new(app_ids: Vec<u32>) -> Self {
        let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            queue: VecDeque::from(app_ids),
            in_flight: Vec::new(),
            result_tx,
            result_rx: Some(result_rx),
        }
    }

    pub fn take_receiver(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<ProgressResult>> {
        self.result_rx.take()
    }

    /// Drive the scanner forward: retire finished tasks, spawn new ones from
    /// the queue. Returns `true` while there is still work pending.
    pub fn poll(&mut self) -> bool {
        self.retire_finished();
        self.spawn_pending();
        !self.in_flight.is_empty() || !self.queue.is_empty()
    }

    #[allow(dead_code)]
    pub fn is_done(&self) -> bool {
        self.queue.is_empty() && self.in_flight.is_empty()
    }

    fn retire_finished(&mut self) {
        self.in_flight.retain(|h| !h.is_finished());
    }

    fn spawn_pending(&mut self) {
        while self.in_flight.len() < MAX_CONCURRENT {
            let Some(app_id) = self.queue.pop_front() else {
                break;
            };
            let tx = self.result_tx.clone();
            let handle = tokio::spawn(async move {
                let result = scan_one_app(app_id).await;
                let _ = tx.send(result);
                ProgressResult { app_id, data: None }
            });
            self.in_flight.push(handle);
        }
    }
}

impl Drop for ProgressScanner {
    fn drop(&mut self) {
        for handle in self.in_flight.drain(..) {
            handle.abort();
        }
    }
}

async fn scan_one_app(app_id: u32) -> ProgressResult {
    match try_full_scan(app_id).await {
        Ok(data) => ProgressResult {
            app_id,
            data: Some(data),
        },
        Err(e) => {
            eprintln!("[steamlens] progress_scan: app_id={app_id} failed: {e}");
            ProgressResult { app_id, data: None }
        }
    }
}

async fn try_full_scan(app_id: u32) -> Result<ScannedGameData, Box<dyn std::error::Error + Send>> {
    let exe =
        std::env::current_exe().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

    let mut child = Command::new(&exe)
        .arg("--worker")
        .arg(app_id.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

    let total_timeout = CONNECT_TIMEOUT + LOAD_TIMEOUT + PERCENTAGES_TIMEOUT;
    let result =
        tokio::time::timeout(total_timeout, run_full_scan_protocol(&mut child)).await;

    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;

    match result {
        Err(_) => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "worker timed out",
        )) as Box<dyn std::error::Error + Send>),
        Ok(Err(e)) => Err(Box::new(e) as Box<dyn std::error::Error + Send>),
        Ok(Ok(data)) => Ok(data),
    }
}

async fn run_full_scan_protocol(child: &mut Child) -> Result<ScannedGameData, std::io::Error> {
    let mut stdin = child.stdin.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdin missing")
    })?;
    let mut stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdout missing")
    })?;

    let hello = tokio::time::timeout(CONNECT_TIMEOUT, read_response(&mut stdout))
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out waiting for Hello")
        })?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "worker closed before Hello",
            )
        })?;

    let app_name = match hello {
        WorkerResponse::Hello { app_name, .. } => app_name,
        WorkerResponse::Error { message, .. } => {
            return Err(std::io::Error::other(message));
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected first message from worker",
            ));
        }
    };

    send_command(&mut stdin, &WorkerCommand::LoadAchievementsAndStats).await?;
    let load_response = tokio::time::timeout(LOAD_TIMEOUT, read_response(&mut stdout))
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for AchievementsAndStats",
            )
        })?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "worker closed before AchievementsAndStats",
            )
        })?;

    let (achievements, stats) = match load_response {
        WorkerResponse::AchievementsAndStats {
            achievements,
            stats,
        } => (achievements, stats),
        WorkerResponse::Error { message, .. } => {
            return Err(std::io::Error::other(message));
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response to LoadAchievementsAndStats",
            ));
        }
    };

    send_command(&mut stdin, &WorkerCommand::RequestGlobalPercentages).await?;
    let global_percentages =
        match tokio::time::timeout(PERCENTAGES_TIMEOUT, read_response(&mut stdout)).await {
            Ok(Some(WorkerResponse::GlobalPercentagesReady(map))) => map,
            _ => HashMap::new(),
        };

    let _ = send_command(&mut stdin, &WorkerCommand::Shutdown).await;

    Ok(ScannedGameData {
        app_name,
        achievements,
        stats,
        global_percentages,
    })
}

async fn read_response(stdout: &mut ChildStdout) -> Option<WorkerResponse> {
    let mut header = [0u8; 4];
    stdout.read_exact(&mut header).await.ok()?;
    let len = parse_header(header).ok()?;
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await.ok()?;
    decode_frame::<WorkerResponse>(&buf).ok()
}

async fn send_command(stdin: &mut ChildStdin, cmd: &WorkerCommand) -> Result<(), std::io::Error> {
    let framed = encode_frame(cmd)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    stdin.write_all(&framed).await?;
    stdin.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ach(id: &str, achieved: bool) -> AchievementData {
        AchievementData {
            id: id.to_owned(),
            display_name: id.to_owned(),
            description: String::new(),
            is_hidden: false,
            is_achieved: achieved,
            unlock_time: None,
            permission: 0,
            icon: None,
        }
    }

    #[test]
    fn scanner_new_empty_is_done() {
        let scanner = ProgressScanner::new(vec![]);
        assert!(scanner.is_done(), "empty scanner must be done immediately");
    }

    #[test]
    fn scanner_new_with_ids_not_done() {
        let scanner = ProgressScanner::new(vec![105600, 570]);
        assert!(
            !scanner.is_done(),
            "scanner with queued games must not be done"
        );
        assert_eq!(scanner.queue.len(), 2, "queue must hold both ids");
        assert!(scanner.in_flight.is_empty(), "no tasks spawned before poll");
    }

    #[test]
    fn scanned_data_count_helpers() {
        let data = ScannedGameData {
            app_name: None,
            achievements: vec![
                make_ach("A", true),
                make_ach("B", false),
                make_ach("C", true),
            ],
            stats: Vec::new(),
            global_percentages: HashMap::new(),
        };
        assert_eq!(data.earned_count(), 2);
        assert_eq!(data.total_count(), 3);
    }

    #[test]
    fn scanner_max_concurrent_cap() {
        assert_eq!(
            MAX_CONCURRENT, 3,
            "scanner cap must be 3 to avoid overloading Steam IPC during cold start"
        );
    }

    #[tokio::test]
    async fn scanner_poll_spawns_up_to_max_concurrent() {
        let ids: Vec<u32> = (1..=10).collect();
        let mut scanner = ProgressScanner::new(ids);
        let has_more = scanner.poll();
        assert!(has_more, "poll on 10 games must return true");
        assert!(
            scanner.in_flight.len() <= MAX_CONCURRENT,
            "must not exceed MAX_CONCURRENT in-flight tasks"
        );
        assert_eq!(
            scanner.queue.len(),
            10 - scanner.in_flight.len(),
            "queue must shrink by the number of spawned tasks"
        );
        for h in scanner.in_flight.drain(..) {
            h.abort();
        }
        scanner.queue.clear();
    }

    #[test]
    fn take_receiver_gives_channel_once() {
        let mut scanner = ProgressScanner::new(vec![1]);
        let rx1 = scanner.take_receiver();
        let rx2 = scanner.take_receiver();
        assert!(rx1.is_some(), "first take must return Some");
        assert!(rx2.is_none(), "second take must return None");
    }

    #[test]
    fn progress_result_none_data_on_failure() {
        let result = ProgressResult {
            app_id: 99,
            data: None,
        };
        assert!(result.data.is_none(), "failure result must have None data");
    }

    #[test]
    fn progress_data_equality() {
        let a = ProgressData {
            earned: 5,
            total: 10,
        };
        let b = ProgressData {
            earned: 5,
            total: 10,
        };
        let c = ProgressData {
            earned: 3,
            total: 10,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
