use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use steamlens_core::ipc::{
    WorkerCommand, WorkerResponse, decode_frame, encode_frame, parse_header,
};
use steamlens_core::{CardOnlyAchievement, CardOnlyPayload, StatData};

use crate::timeouts;

const MAX_CONCURRENT: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressData {
    pub earned: u32,
    pub total: u32,
}

#[derive(Debug, Clone)]
pub struct ScannedGameData {
    pub app_name: Option<String>,
    pub achievements: Vec<CardOnlyAchievement>,
    pub stats: Vec<StatData>,
    pub global_percentages: HashMap<String, f32>,
    #[allow(dead_code)]
    pub genre: Option<String>,
}

impl ScannedGameData {
    pub fn earned_count(&self) -> u32 {
        self.achievements.iter().filter(|a| a.is_achieved).count() as u32
    }

    pub fn total_count(&self) -> u32 {
        self.achievements.len() as u32
    }
}

#[derive(Debug, Clone)]
pub struct ProgressResult {
    pub app_id: u32,
    pub data: Option<ScannedGameData>,
}

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
        Err((err, diag)) => {
            if diag.is_empty() {
                eprintln!("[steamlens] progress_scan: app_id={app_id} failed: {err}");
            } else {
                eprintln!(
                    "[steamlens] progress_scan: app_id={app_id} failed: {err}\n--- worker diagnostics ---\n{}--- end diagnostics ---",
                    diag
                );
            }
            ProgressResult { app_id, data: None }
        }
    }
}

type ScanError = (Box<dyn std::error::Error + Send>, String);

async fn try_full_scan(app_id: u32) -> Result<ScannedGameData, ScanError> {
    let exe = std::env::current_exe().map_err(|e| {
        (
            Box::new(e) as Box<dyn std::error::Error + Send>,
            String::new(),
        )
    })?;

    let mut child = Command::new(&exe)
        .arg("--worker")
        .arg(app_id.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            (
                Box::new(e) as Box<dyn std::error::Error + Send>,
                String::new(),
            )
        })?;

    let _job_guard = match child.id() {
        Some(pid) => steamlens_core::associate_kill_on_parent_exit(pid).map_err(|e| {
            let _ = child.start_kill();
            (
                Box::new(e) as Box<dyn std::error::Error + Send>,
                String::new(),
            )
        })?,
        None => {
            let _ = child.start_kill();
            return Err((
                Box::new(std::io::Error::other("spawned worker has no pid"))
                    as Box<dyn std::error::Error + Send>,
                String::new(),
            ));
        }
    };

    let stderr_pipe = child.stderr.take();
    let stderr_task = tokio::spawn(async move {
        let Some(mut stderr) = stderr_pipe else {
            return Vec::new();
        };
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf).await;
        buf
    });

    let total_timeout =
        timeouts::STEAM_CONNECT + timeouts::COLD_SCAN_LOAD + timeouts::GLOBAL_PERCENTAGES;
    let result = tokio::time::timeout(total_timeout, run_full_scan_protocol(&mut child)).await;

    let _ = child.start_kill();
    let exit_status = tokio::time::timeout(timeouts::CHILD_KILL, child.wait())
        .await
        .ok()
        .and_then(|r| r.ok());

    let stderr_bytes = tokio::time::timeout(timeouts::STDERR_DRAIN, stderr_task)
        .await
        .ok()
        .and_then(|res| res.ok())
        .unwrap_or_default();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).into_owned();

    let mut diag = format!("worker {}\n", format_exit_status(exit_status.as_ref()));
    if !stderr_str.is_empty() {
        diag.push_str(&stderr_str);
    }

    match result {
        Err(_) => Err((
            Box::new(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "worker timed out",
            )) as Box<dyn std::error::Error + Send>,
            diag,
        )),
        Ok(Err(e)) => Err((Box::new(e) as Box<dyn std::error::Error + Send>, diag)),
        Ok(Ok(data)) => Ok(data),
    }
}

#[cfg(unix)]
fn format_exit_status(status: Option<&std::process::ExitStatus>) -> String {
    use std::os::unix::process::ExitStatusExt;
    let Some(s) = status else {
        return "exit status unavailable".to_owned();
    };
    if let Some(code) = s.code() {
        format!("exited with code {code}")
    } else if let Some(sig) = s.signal() {
        format!("killed by signal {} ({})", sig, signal_name(sig))
    } else {
        format!("{s:?}")
    }
}

#[cfg(not(unix))]
fn format_exit_status(status: Option<&std::process::ExitStatus>) -> String {
    let Some(s) = status else {
        return "exit status unavailable".to_owned();
    };
    if let Some(code) = s.code() {
        format!("exited with code {code}")
    } else {
        format!("{s:?}")
    }
}

#[cfg(unix)]
fn signal_name(sig: i32) -> &'static str {
    match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        6 => "SIGABRT",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => "unknown",
    }
}

async fn run_full_scan_protocol(child: &mut Child) -> Result<ScannedGameData, std::io::Error> {
    let mut stdin = child.stdin.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdin missing")
    })?;
    let mut stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "child stdout missing")
    })?;

    let connected = tokio::time::timeout(timeouts::STEAM_CONNECT, read_response(&mut stdout))
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for SteamConnected",
            )
        })?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "worker closed before SteamConnected",
            )
        })?;

    let app_name = match connected {
        WorkerResponse::SteamConnected { app_name, .. } => app_name,
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

    send_command(&mut stdin, &WorkerCommand::LoadAchievementsAndStatsCardOnly).await?;
    let (achievements, stats, genre) =
        read_card_only_skipping_async(&mut stdout, timeouts::COLD_SCAN_LOAD).await?;

    let global_percentages = if achievements.is_empty() {
        HashMap::new()
    } else {
        send_command(&mut stdin, &WorkerCommand::RequestGlobalPercentages).await?;
        read_percentages_skipping_async(&mut stdout, timeouts::GLOBAL_PERCENTAGES).await
    };

    let _ = send_command(&mut stdin, &WorkerCommand::Shutdown).await;

    Ok(ScannedGameData {
        app_name,
        achievements,
        stats,
        global_percentages,
        genre,
    })
}

async fn read_card_only_skipping_async(
    stdout: &mut ChildStdout,
    total_timeout: Duration,
) -> Result<(Vec<CardOnlyAchievement>, Vec<StatData>, Option<String>), std::io::Error> {
    let deadline = tokio::time::Instant::now() + total_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for CardOnlyAchievements",
            ));
        }
        let frame = tokio::time::timeout(remaining, read_response(stdout))
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "timed out waiting for CardOnlyAchievements",
                )
            })?
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "worker closed before CardOnlyAchievements",
                )
            })?;
        match frame {
            WorkerResponse::CardOnlyAchievements {
                shm_path,
                region_bytes,
            } => {
                let path = std::path::PathBuf::from(&shm_path);
                let payload: CardOnlyPayload = steamlens_core::read_payload(&path, region_bytes)
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("CardOnlyAchievements shm read: {e}"),
                        )
                    })?;
                return Ok((payload.achievements, Vec::new(), None));
            }
            WorkerResponse::IconUpdated { shm_path, .. } => {
                steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::AchievementsAndStats { shm_path, .. }
            | WorkerResponse::AchievementCount { shm_path, .. }
            | WorkerResponse::ProbeResult { shm_path, .. }
            | WorkerResponse::GlobalPercentagesReady { shm_path, .. } => {
                steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::Error { message, .. } => {
                return Err(std::io::Error::other(message));
            }
            _ => continue,
        }
    }
}

async fn read_percentages_skipping_async(
    stdout: &mut ChildStdout,
    total_timeout: Duration,
) -> HashMap<String, f32> {
    let deadline = tokio::time::Instant::now() + total_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return HashMap::new();
        }
        let frame = match tokio::time::timeout(remaining, read_response(stdout)).await {
            Ok(Some(f)) => f,
            _ => return HashMap::new(),
        };
        match frame {
            WorkerResponse::GlobalPercentagesReady {
                shm_path,
                region_bytes,
            } => {
                let path = std::path::PathBuf::from(&shm_path);
                return steamlens_core::read_payload::<HashMap<String, f32>>(&path, region_bytes)
                    .unwrap_or_default();
            }
            WorkerResponse::AchievementsAndStats { shm_path, .. }
            | WorkerResponse::IconUpdated { shm_path, .. }
            | WorkerResponse::AchievementCount { shm_path, .. }
            | WorkerResponse::ProbeResult { shm_path, .. } => {
                steamlens_core::unlink_at(&std::path::PathBuf::from(shm_path));
                continue;
            }
            WorkerResponse::Error { .. } => return HashMap::new(),
            _ => continue,
        }
    }
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

    fn make_ach(id: &str, achieved: bool) -> CardOnlyAchievement {
        CardOnlyAchievement {
            id: id.to_owned(),
            is_achieved: achieved,
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
            genre: None,
        };
        assert_eq!(data.earned_count(), 2);
        assert_eq!(data.total_count(), 3);
    }

    #[test]
    fn scanner_max_concurrent_cap() {
        assert_eq!(
            MAX_CONCURRENT, 1,
            "scanner cap must be 1; sequential scan avoids Steam IPC contention on cold start"
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
