#![expect(dead_code, reason = "WorkerHandle API has no callers until M2")]

use std::process::ExitStatus;
use std::time::Duration;

use steamlens_core::ipc::WorkerErrorKind;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;

/// Lifecycle mode for a spawned worker child.
#[derive(Debug, Clone, Copy)]
pub enum WorkerMode {
    /// Long-lived interactive session. No whole-operation timeout.
    /// Drainer task forwards each stderr line through `crate::log!`.
    Interactive,

    /// Short-lived single-operation scan, kill-on-timeout.
    /// Drainer task forwards each line AND captures into a buffer for `drain_stderr()`.
    OneShot { timeout: Duration },
}

/// Error variants covering all failure modes during worker spawn.
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

    /// Not in RFC-006 §Public API Surface; added because stderr is always piped
    /// and a missing pipe is the same class of failure as missing stdin/stdout.
    #[error("child stderr pipe unavailable after spawn")]
    StderrUnavailable,
}

/// Typed error for in-session protocol failures across both interactive and one-shot scan paths.
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

/// Unified handle to a spawned `--worker <app_id>` child process.
///
/// Both `drain_stderr` and `shutdown` consume `self` — callers may call exactly
/// one of them per handle. `shutdown` is the normal teardown path; `drain_stderr`
/// is for `OneShot` error-inspection paths where the caller wants the raw stderr
/// bytes before discarding the handle.
pub struct WorkerHandle {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr_task: Option<JoinHandle<Vec<u8>>>,
    _guard: steamlens_core::ChildLifetimeGuard,
    mode: WorkerMode,
    app_id: u32,
}

impl WorkerHandle {
    /// Spawn a `--worker <app_id>` child in the given mode.
    pub async fn spawn(app_id: u32, mode: WorkerMode) -> Result<Self, WorkerSpawnError> {
        let exe = std::env::current_exe().map_err(WorkerSpawnError::ExeNotFound)?;
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

        let capture = matches!(mode, WorkerMode::OneShot { .. });
        let stderr_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut buf = Vec::new();
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                crate::log!("worker[app_id={app_id}] stderr: {line}");
                if capture {
                    buf.extend_from_slice(line.as_bytes());
                    buf.push(b'\n');
                }
            }
            buf
        });

        Ok(Self {
            child,
            stdin,
            stdout,
            stderr_task: Some(stderr_task),
            _guard: guard,
            mode,
            app_id,
        })
    }

    /// Send a command frame to the worker's stdin.
    pub async fn send(
        &mut self,
        cmd: &steamlens_core::ipc::WorkerCommand,
    ) -> Result<(), WorkerProtocolError> {
        crate::ipc_pipe::write_command(&mut self.stdin, cmd)
            .await
            .map_err(WorkerProtocolError::Write)
    }

    /// Read one response frame from the worker's stdout.
    ///
    /// Returns `None` on EOF or decode failure; callers that treat EOF as an error
    /// should map `None` to `WorkerProtocolError::UnexpectedEof`.
    pub async fn recv(
        &mut self,
    ) -> Result<Option<steamlens_core::ipc::WorkerResponse>, WorkerProtocolError> {
        Ok(crate::ipc_pipe::read_response(&mut self.stdout).await)
    }

    /// Drain the stderr capture buffer (`OneShot` mode only).
    ///
    /// Kills the child so the drainer sees EOF, then waits up to `STDERR_DRAIN`.
    /// Returns `None` in `Interactive` mode or if the drain times out.
    pub async fn drain_stderr(mut self) -> Option<Vec<u8>> {
        match self.mode {
            WorkerMode::OneShot { .. } => {
                let _ = self.child.start_kill();
                let _ = tokio::time::timeout(crate::timeouts::CHILD_KILL, self.child.wait()).await;
                let task = self.stderr_task.take()?;
                tokio::time::timeout(crate::timeouts::STDERR_DRAIN, task)
                    .await
                    .ok()
                    .and_then(|r| r.ok())
            }
            WorkerMode::Interactive => None,
        }
    }

    /// Mode-aware shutdown. Consumes `self`.
    ///
    /// `Interactive`: sends `Shutdown`, waits `CHILD_DRAIN`, kills on timeout.
    /// `OneShot`: kills immediately, waits `CHILD_KILL`.
    pub async fn shutdown(mut self) -> Option<ExitStatus> {
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
                    self.abort_stderr_task().await;
                    return Some(status);
                }
            }
            WorkerMode::OneShot { .. } => {}
        }

        let _ = self.child.start_kill();
        let status = tokio::time::timeout(crate::timeouts::CHILD_KILL, self.child.wait())
            .await
            .ok()
            .and_then(|r| r.ok());
        self.abort_stderr_task().await;
        status
    }

    async fn abort_stderr_task(&mut self) {
        if let Some(task) = self.stderr_task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn worker_mode_variants_copy() {
        let a = WorkerMode::Interactive;
        let b = WorkerMode::OneShot {
            timeout: Duration::from_secs(30),
        };
        let _a2 = a;
        let _b2 = b;
        let _ = (a, b);
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
