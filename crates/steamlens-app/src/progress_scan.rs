use std::collections::VecDeque;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};

const MAX_CONCURRENT: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const COUNT_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-game achievement progress fetched by the background scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressData {
    pub earned: u32,
    pub total: u32,
}

/// Result of a single game's progress query, whether it succeeded or not.
#[derive(Debug, Clone)]
pub struct ProgressResult {
    pub app_id: u32,
    /// `None` when the child worker failed or timed out for this game.
    pub data: Option<ProgressData>,
}

/// Streams per-game achievement progress in a bounded concurrent manner.
///
/// Spawns up to `MAX_CONCURRENT` (5) child worker processes at once, each
/// requesting only the achievement count for one game.  As workers finish the
/// next game from the queue is started.  Results arrive via the tokio channel
/// returned from [`ProgressScanner::start`].
///
/// Drop the scanner to cancel all in-flight workers (their `Child` handles are
/// killed in `Drop`).
pub struct ProgressScanner {
    queue: VecDeque<u32>,
    in_flight: Vec<tokio::task::JoinHandle<ProgressResult>>,
    result_tx: tokio::sync::mpsc::UnboundedSender<ProgressResult>,
    result_rx: Option<tokio::sync::mpsc::UnboundedReceiver<ProgressResult>>,
}

impl ProgressScanner {
    /// Create a new scanner for the given list of app IDs.
    ///
    /// Call [`ProgressScanner::poll`] repeatedly (e.g. from an iced
    /// `Subscription` tick) to drive progress and collect results.
    pub fn new(app_ids: Vec<u32>) -> Self {
        let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            queue: VecDeque::from(app_ids),
            in_flight: Vec::new(),
            result_tx,
            result_rx: Some(result_rx),
        }
    }

    /// Take the receiver end of the result channel (call at most once).
    pub fn take_receiver(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<ProgressResult>> {
        self.result_rx.take()
    }

    /// Drive the scanner forward: retire finished tasks, spawn new ones from
    /// the queue.  Returns `true` while there is still work pending.
    ///
    /// Must be called from within a tokio runtime context (e.g. inside an iced
    /// `Subscription` or `Task`).
    pub fn poll(&mut self) -> bool {
        self.retire_finished();
        self.spawn_pending();
        !self.in_flight.is_empty() || !self.queue.is_empty()
    }

    /// Returns `true` if all queued games have been processed.
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
                let result = fetch_count_for_app(app_id).await;
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

async fn fetch_count_for_app(app_id: u32) -> ProgressResult {
    match try_fetch_count(app_id).await {
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

async fn try_fetch_count(app_id: u32) -> Result<ProgressData, Box<dyn std::error::Error + Send>> {
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

    let result = tokio::time::timeout(
        CONNECT_TIMEOUT + COUNT_TIMEOUT,
        run_count_protocol(&mut child),
    )
    .await;

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

async fn run_count_protocol(child: &mut Child) -> Result<ProgressData, std::io::Error> {
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

    match hello {
        WorkerResponse::Hello { .. } => {}
        WorkerResponse::Error { message, .. } => {
            return Err(std::io::Error::other(message));
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected first message from worker",
            ));
        }
    }

    send_command(&mut stdin, &WorkerCommand::QuickAchievementCount).await?;

    let response = tokio::time::timeout(COUNT_TIMEOUT, read_response(&mut stdout))
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for AchievementCount",
            )
        })?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "worker closed before AchievementCount",
            )
        })?;

    let (earned, total) = match response {
        WorkerResponse::AchievementCount { earned, total } => (earned, total),
        WorkerResponse::Error { message, .. } => {
            return Err(std::io::Error::other(message));
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response to QuickAchievementCount",
            ));
        }
    };

    send_command(&mut stdin, &WorkerCommand::Shutdown).await?;

    Ok(ProgressData { earned, total })
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

    #[test]
    fn progress_result_none_data_on_failure() {
        let result = ProgressResult {
            app_id: 99,
            data: None,
        };
        assert!(result.data.is_none(), "failure result must have None data");
    }

    #[test]
    fn scanner_max_concurrent_cap() {
        assert_eq!(
            MAX_CONCURRENT, 5,
            "scanner cap must be 5 to avoid Steam IPC exhaustion"
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
        // Abort spawned tasks so the test exits cleanly.
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
}
