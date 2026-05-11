use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::Mutex;

/// Opens `steamlens.log` at the given path with truncation on every call,
/// installs a panic hook that routes panic info through the tracing subscriber,
/// and sets the process-global default subscriber. Call once at the top of `main()`.
pub fn init() -> io::Result<()> {
    init_with_path(&crate::paths::log_path())
}

pub(crate) fn init_with_path(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    let writer = Mutex::new(file);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| io::Error::other("logging already initialized"))?;

    std::panic::set_hook(Box::new(|info| {
        tracing::error!(target: "panic", "PANIC: {info}");
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "{bt}");
    }));

    Ok(())
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        ::tracing::info!($($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_and_log_lifecycle() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("steamlens.log");

        std::fs::write(&path, b"stale content that must be gone").expect("pre-seed");

        let first = init_with_path(&path);

        let content_after_init = std::fs::read(&path).expect("read after init");
        assert!(
            content_after_init.len() < 1024,
            "file must be empty or near-empty after init, got {} bytes",
            content_after_init.len()
        );

        if first.is_ok() {
            crate::log!("test marker abc123");

            let content_after_log = std::fs::read(&path).expect("read after log");
            assert!(
                content_after_log
                    .windows(b"test marker abc123".len())
                    .any(|w| w == b"test marker abc123"),
                "log output must contain the test marker"
            );

            let dir2 = tempfile::TempDir::new().expect("tempdir2");
            let path2 = dir2.path().join("second.log");
            let second = init_with_path(&path2);
            assert!(
                second.is_err(),
                "second init_with_path must return Err, got Ok"
            );
        } else {
            assert!(
                first.is_err(),
                "expected Err on already-initialized subscriber"
            );
        }
    }
}
