use std::process::ExitStatus;

use steamlens_core::ipc::{WorkerErrorKind, WorkerResponse};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy)]
pub enum WorkerMode {
    Interactive,
    OneShot,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerSpawnError {
    #[error("could not resolve current executable: {0}")]
    ExeNotFound(std::io::Error),

    #[error("worker child spawn failed: {0}")]
    SpawnFailed(std::io::Error),

    #[error("spawned worker has no pid")]
    NoChildPid,

    #[error("kill-on-parent-exit guard failed: {0}")]
    LifetimeGuardFailed(std::io::Error),

    #[error("child stdin pipe unavailable after spawn")]
    StdinUnavailable,

    #[error("child stdout pipe unavailable after spawn")]
    StdoutUnavailable,

    #[error("child stderr pipe unavailable after spawn")]
    StderrUnavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectivityError {
    #[error("steam is not running")]
    SteamNotRunning,
    #[error("user is not signed in to steam")]
    NotLoggedIn,
}

#[derive(Debug, thiserror::Error)]
pub enum SendCheckedError {
    #[error(transparent)]
    Connectivity(#[from] ConnectivityError),
    #[error(transparent)]
    Protocol(#[from] WorkerProtocolError),
}

pub(crate) fn preflight(
    steam_running: bool,
    user_logged_in: bool,
) -> Result<(), ConnectivityError> {
    if !steam_running {
        return Err(ConnectivityError::SteamNotRunning);
    }
    if !user_logged_in {
        return Err(ConnectivityError::NotLoggedIn);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerProtocolError {
    #[error("worker error: {kind:?}: {message}")]
    WorkerError {
        kind: WorkerErrorKind,
        message: String,
    },

    #[error("unexpected response variant")]
    UnexpectedMessage,

    #[error("operation timed out")]
    Timeout,

    #[error("child stdout closed before protocol completion")]
    UnexpectedEof,

    #[error("frame decode failed: {0}")]
    Decode(std::io::Error),

    #[error("write to child stdin failed: {0}")]
    Write(std::io::Error),
}

/// Call `finish` exactly once to release the child; `Drop` alone will kill abruptly.
pub struct WorkerHandle {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::UnboundedReceiver<WorkerResponse>,
    _reader_task: JoinHandle<()>,
    stderr_task: Option<JoinHandle<Vec<u8>>>,
    _guard: steamlens_core::ChildLifetimeGuard,
    mode: WorkerMode,
}

impl WorkerHandle {
    pub async fn spawn(app_id: u32, mode: WorkerMode) -> Result<Self, WorkerSpawnError> {
        let exe = steamlens_core::current_exe_resilient().map_err(WorkerSpawnError::ExeNotFound)?;
        let mut child = tokio::process::Command::new(exe)
            .arg("--worker")
            .arg(app_id.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(WorkerSpawnError::SpawnFailed)?;

        let pid = child.id().ok_or(WorkerSpawnError::NoChildPid)?;
        let guard = steamlens_core::associate_kill_on_parent_exit(pid)
            .inspect_err(|_| {
                let _ = child.start_kill();
            })
            .map_err(WorkerSpawnError::LifetimeGuardFailed)?;

        let stdin = child
            .stdin
            .take()
            .ok_or(WorkerSpawnError::StdinUnavailable)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(WorkerSpawnError::StdoutUnavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(WorkerSpawnError::StderrUnavailable)?;

        let capture = matches!(mode, WorkerMode::OneShot);
        let stderr_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut buf = Vec::new();
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                forward_worker_line(app_id, &line);
                if capture {
                    buf.extend_from_slice(line.as_bytes());
                    buf.push(b'\n');
                }
            }
            buf
        });

        let (resp_tx, responses) = mpsc::unbounded_channel::<WorkerResponse>();
        let reader_task = tokio::spawn(async move {
            let mut stdout = stdout;
            while let Some(resp) = crate::ipc_pipe::read_response(&mut stdout).await {
                if resp_tx.send(resp).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            responses,
            _reader_task: reader_task,
            stderr_task: Some(stderr_task),
            _guard: guard,
            mode,
        })
    }

    pub async fn send(
        &mut self,
        cmd: &steamlens_core::ipc::WorkerCommand,
    ) -> Result<(), WorkerProtocolError> {
        crate::ipc_pipe::write_command(&mut self.stdin, cmd)
            .await
            .map_err(WorkerProtocolError::Write)
    }

    #[allow(dead_code, reason = "callsites migrate in next chunk")]
    pub async fn send_checked(
        &mut self,
        cmd: &steamlens_core::ipc::WorkerCommand,
        steam_running: bool,
        user_logged_in: bool,
    ) -> Result<(), SendCheckedError> {
        preflight(steam_running, user_logged_in).map_err(SendCheckedError::Connectivity)?;
        self.send(cmd).await.map_err(SendCheckedError::Protocol)
    }

    pub async fn recv(
        &mut self,
    ) -> Result<Option<steamlens_core::ipc::WorkerResponse>, WorkerProtocolError> {
        Ok(self.responses.recv().await)
    }

    /// Interactive: graceful Shutdown command, then kill on `CHILD_DRAIN` timeout. Returns `(status, None)`.
    /// OneShot: immediate kill, drains stderr capture buffer. Returns `(status, Some(bytes))`.
    pub async fn finish(mut self) -> (Option<ExitStatus>, Option<Vec<u8>>) {
        match self.mode {
            WorkerMode::Interactive => {
                let _ = crate::ipc_pipe::write_command(
                    &mut self.stdin,
                    &steamlens_core::ipc::WorkerCommand::Shutdown,
                )
                .await;
                if let Ok(Ok(status)) =
                    tokio::time::timeout(crate::timeouts::CHILD_DRAIN, self.child.wait()).await
                {
                    abort_task(self.stderr_task).await;
                    return (Some(status), None);
                }
                let _ = self.child.start_kill();
                let status = tokio::time::timeout(crate::timeouts::CHILD_KILL, self.child.wait())
                    .await
                    .ok()
                    .and_then(|r| r.ok());
                abort_task(self.stderr_task).await;
                (status, None)
            }
            WorkerMode::OneShot => {
                let _ = self.child.start_kill();
                let status = tokio::time::timeout(crate::timeouts::CHILD_KILL, self.child.wait())
                    .await
                    .ok()
                    .and_then(|r| r.ok());
                let bytes = match self.stderr_task {
                    Some(task) => tokio::time::timeout(crate::timeouts::STDERR_DRAIN, task)
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => None,
                };
                (status, bytes)
            }
        }
    }
}

async fn abort_task(task: Option<JoinHandle<Vec<u8>>>) {
    if let Some(t) = task {
        t.abort();
        let _ = t.await;
    }
}

fn forward_worker_line(app_id: u32, line: &str) {
    let (level, message) = parse_worker_line(line);
    match level {
        tracing::Level::ERROR => {
            tracing::error!(target: "worker", app_id, "{message}")
        }
        tracing::Level::WARN => tracing::warn!(target: "worker", app_id, "{message}"),
        tracing::Level::DEBUG => {
            tracing::debug!(target: "worker", app_id, "{message}")
        }
        tracing::Level::TRACE => {
            tracing::trace!(target: "worker", app_id, "{message}")
        }
        _ => tracing::info!(target: "worker", app_id, "{message}"),
    }
}

fn parse_worker_line(line: &str) -> (tracing::Level, &str) {
    let trimmed = line.trim_start();
    for (prefix, level) in [
        ("ERROR ", tracing::Level::ERROR),
        ("WARN ", tracing::Level::WARN),
        ("INFO ", tracing::Level::INFO),
        ("DEBUG ", tracing::Level::DEBUG),
        ("TRACE ", tracing::Level::TRACE),
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return (level, rest);
        }
    }
    (tracing::Level::INFO, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_returns_steam_not_running_when_flag_false() {
        let err = preflight(false, true).unwrap_err();
        assert!(matches!(err, ConnectivityError::SteamNotRunning));
    }

    #[test]
    fn preflight_returns_not_logged_in_when_flag_false() {
        let err = preflight(true, false).unwrap_err();
        assert!(matches!(err, ConnectivityError::NotLoggedIn));
    }

    #[test]
    fn preflight_passes_when_both_true() {
        assert!(preflight(true, true).is_ok());
    }

    #[test]
    fn preflight_steam_not_running_takes_precedence_over_login() {
        let err = preflight(false, false).unwrap_err();
        assert!(matches!(err, ConnectivityError::SteamNotRunning));
    }

    #[test]
    fn worker_protocol_error_display_includes_kind_and_message() {
        let err = WorkerProtocolError::WorkerError {
            kind: WorkerErrorKind::Connect,
            message: "no pipe".to_owned(),
        };
        let s = format!("{err}");
        assert!(s.contains("Connect"));
        assert!(s.contains("no pipe"));
    }

    #[test]
    fn worker_protocol_error_variants_constructible() {
        let _: WorkerProtocolError = WorkerProtocolError::UnexpectedMessage;
        let _: WorkerProtocolError = WorkerProtocolError::Timeout;
        let _: WorkerProtocolError = WorkerProtocolError::UnexpectedEof;
        let _: WorkerProtocolError = WorkerProtocolError::Decode(std::io::Error::other("x"));
        let _: WorkerProtocolError = WorkerProtocolError::Write(std::io::Error::other("y"));
    }

    #[test]
    fn parse_worker_line_strips_known_levels() {
        let cases = [
            (
                "INFO worker connected",
                tracing::Level::INFO,
                "worker connected",
            ),
            (
                "DEBUG cmd dispatched",
                tracing::Level::DEBUG,
                "cmd dispatched",
            ),
            ("WARN cache miss", tracing::Level::WARN, "cache miss"),
            ("ERROR pipe broken", tracing::Level::ERROR, "pipe broken"),
            ("TRACE tick", tracing::Level::TRACE, "tick"),
        ];
        for (line, expected_level, expected_msg) in cases {
            let (level, msg) = parse_worker_line(line);
            assert_eq!(level, expected_level, "level for {line:?}");
            assert_eq!(msg, expected_msg, "message for {line:?}");
        }
    }

    #[test]
    fn parse_worker_line_falls_back_to_info_when_no_prefix() {
        let (level, msg) = parse_worker_line("steamlens: BLoggedOn = true");
        assert_eq!(level, tracing::Level::INFO);
        assert_eq!(msg, "steamlens: BLoggedOn = true");
    }

    #[test]
    fn worker_mode_variants_copy() {
        let a = WorkerMode::OneShot;
        let _a2 = a;
        let _ = a;
        let b = WorkerMode::Interactive;
        let _b2 = b;
        let _ = b;
    }

    #[test]
    fn worker_spawn_error_display() {
        let variants: &[(&str, WorkerSpawnError)] = &[
            (
                "executable",
                WorkerSpawnError::ExeNotFound(std::io::Error::other("no exe")),
            ),
            (
                "spawn failed",
                WorkerSpawnError::SpawnFailed(std::io::Error::other("denied")),
            ),
            ("no pid", WorkerSpawnError::NoChildPid),
            (
                "guard failed",
                WorkerSpawnError::LifetimeGuardFailed(std::io::Error::other("guard")),
            ),
            ("stdin", WorkerSpawnError::StdinUnavailable),
            ("stdout", WorkerSpawnError::StdoutUnavailable),
            ("stderr", WorkerSpawnError::StderrUnavailable),
        ];
        for (needle, err) in variants {
            let s = format!("{err}");
            assert!(
                s.to_lowercase().contains(needle),
                "expected {needle:?} in {s:?}"
            );
        }
    }
}
