use steamlens_core::ipc::FrameError;

#[derive(Debug)]
pub(super) enum WorkerError {
    Io(std::io::Error),
    Frame(FrameError),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerError::Io(e) => write!(f, "io: {e}"),
            WorkerError::Frame(e) => write!(f, "frame: {e}"),
        }
    }
}

impl From<std::io::Error> for WorkerError {
    fn from(e: std::io::Error) -> Self {
        WorkerError::Io(e)
    }
}

impl From<FrameError> for WorkerError {
    fn from(e: FrameError) -> Self {
        WorkerError::Frame(e)
    }
}

pub(super) fn error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut cur = err.source();
    while let Some(e) = cur {
        out.push_str(": ");
        out.push_str(&e.to_string());
        cur = e.source();
    }
    out
}
