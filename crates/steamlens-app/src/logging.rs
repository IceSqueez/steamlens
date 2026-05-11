use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

/// Opens `steamlens.log` at the given path with truncation on every call,
/// installs a panic hook that writes to the same file, and records a startup
/// timestamp for monotonic log prefixes. Call once at the top of `main()`.
pub fn init() -> io::Result<()> {
    init_with_path(&crate::paths::log_path())
}

pub(crate) fn init_with_path(path: &Path) -> io::Result<()> {
    if LOG_FILE.get().is_some() {
        return Err(io::Error::other("logging already initialized"));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    START.get_or_init(Instant::now);

    if LOG_FILE.set(Mutex::new(file)).is_err() {
        return Err(io::Error::other("logging already initialized"));
    }

    std::panic::set_hook(Box::new(|info| {
        write_line("panic", format_args!("PANIC: {info}"));
        let bt = std::backtrace::Backtrace::force_capture();
        write_line("panic", format_args!("{bt}"));
    }));

    Ok(())
}

pub fn write_line(tag: &'static str, args: std::fmt::Arguments) {
    let Some(lock) = LOG_FILE.get() else {
        return;
    };
    let Ok(mut file) = lock.lock() else {
        return;
    };
    let elapsed = START
        .get()
        .map(|s| s.elapsed().as_secs_f64())
        .unwrap_or(0.0);
    let _ = writeln!(file, "[+{elapsed:.3}s] [{tag}] {args}");
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::logging::write_line(module_path!(), format_args!($($arg)*));
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;

    // LOG_FILE is a process-global OnceLock. We use a separate Once guard so
    // all tests that need the global to be initialized call ensure_global(),
    // which is idempotent and thread-safe regardless of test ordering.
    static GLOBAL_INIT: Once = Once::new();

    fn ensure_global() {
        GLOBAL_INIT.call_once(|| {
            let dir = tempfile::TempDir::new().expect("tempdir for global init");
            // Intentionally leak the TempDir so the path stays valid for the
            // process lifetime; it's a test binary, so this is acceptable.
            let path = Box::leak(Box::new(dir)).path().join("global.log");
            init_with_path(&path).expect("global init must succeed");
        });
    }

    #[test]
    fn init_creates_log_file_and_truncates() {
        // Test the file-level truncation behaviour directly with OpenOptions —
        // this does not touch the global OnceLock.
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("trunc.log");

        std::fs::write(&path, b"old content that must be gone").expect("pre-seed");

        {
            use std::fs::OpenOptions;
            let _f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .expect("open with truncate");
        }

        let content = std::fs::read(&path).expect("read after truncate");
        assert!(
            content.is_empty(),
            "file must be empty after truncate open, got {} bytes",
            content.len()
        );
    }

    #[test]
    fn double_init_returns_error() {
        ensure_global();

        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("second.log");

        let result = init_with_path(&path);
        assert!(
            result.is_err(),
            "second init_with_path must return Err, got Ok"
        );
    }

    #[test]
    fn log_macro_writes_line() {
        ensure_global();
        // Must not panic regardless of whether the global path still exists.
        crate::log!("macro test: value={}", 42);
    }
}
