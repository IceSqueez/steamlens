use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::Mutex;

use tracing::Level;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;

/// Host process only. Truncates `steamlens.log` and opens it as the writer.
pub fn init() -> io::Result<()> {
    init_with_path(&crate::paths::log_path())
}

/// Subprocess only. Writes to stderr with a minimal `<LEVEL> <message>` format —
/// the host parses the level prefix and re-emits each line into its own writer
/// with proper timestamp + target, so worker output is not double-wrapped.
pub fn init_worker() -> io::Result<()> {
    install_worker_subscriber(std::io::stderr)?;
    #[cfg(target_os = "windows")]
    install_seh_handler();
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_seh_handler() {
    use windows_sys::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter;
    // SAFETY: `seh_handler` is a valid `LPTOP_LEVEL_EXCEPTION_FILTER`
    // function pointer.  `SetUnhandledExceptionFilter` is always safe to
    // call; it installs a process-wide last-resort filter and returns the
    // previous one (which we discard — we do not need to chain).
    unsafe {
        SetUnhandledExceptionFilter(Some(seh_handler));
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn seh_handler(
    info: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};

    // SAFETY: OS guarantees `info` and `info.ExceptionRecord` are non-null
    // and valid for the duration of the call. We read only plain integer
    // and pointer fields. We deliberately do NOT use `eprintln!`,
    // `tracing`, or `std::io::Stderr::lock()` — all three acquire user-space
    // mutexes (std stderr uses a non-reentrant `Mutex`) which the faulting
    // thread may already hold, causing same-thread deadlock. `WriteFile`
    // on a Win32 handle goes straight to the kernel and is the only
    // lock-free path. No heap allocation, no panic-machinery re-entry.
    let (code, addr) = unsafe {
        let rec = (*info).ExceptionRecord;
        ((*rec).ExceptionCode as u32, (*rec).ExceptionAddress)
    };

    let mut buf = [0u8; 64];
    let msg = format_seh_line(&mut buf, code, addr as usize);

    // SAFETY: `STD_ERROR_HANDLE` is a process-wide pseudo-handle; `WriteFile`
    // accepts a borrowed handle and a borrowed buffer, writes synchronously,
    // and does not retain either pointer after returning.
    let stderr = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
    // SAFETY: same as above.
    unsafe {
        let mut written: u32 = 0;
        WriteFile(
            stderr,
            msg.as_ptr(),
            msg.len() as u32,
            &mut written,
            core::ptr::null_mut(),
        );
    }

    // SAFETY: PSAPI's `EnumProcessModules` / `GetModuleInformation` /
    // `GetModuleBaseNameW` are documented as safe to call from an SEH
    // top-level filter — they take only kernel locks, not the process
    // heap or any user-space mutex. We use stack-only buffers, no
    // allocations. A failure of any PSAPI call is silently swallowed —
    // we are dying anyway, the module list is best-effort diagnostic.
    write_stack_trace(stderr);
    write_module_list(stderr);

    windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_EXECUTE_HANDLER
}

#[cfg(target_os = "windows")]
fn write_stack_trace(stderr: windows_sys::Win32::Foundation::HANDLE) {
    use std::io::Write;
    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;

    const MAX_FRAMES: usize = 32;
    let mut frames: [*mut core::ffi::c_void; MAX_FRAMES] = [core::ptr::null_mut(); MAX_FRAMES];
    // SAFETY: `frames` is a stack array we own; `RtlCaptureStackBackTrace`
    // walks the current thread's stack and writes return addresses into it.
    // No allocation, no locks. Documented safe in any context including
    // top-level SEH filters.
    let captured = unsafe {
        RtlCaptureStackBackTrace(
            0,
            MAX_FRAMES as u32,
            frames.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    } as usize;

    let mut line = [0u8; 96];
    for (idx, frame) in frames.iter().take(captured).enumerate() {
        let mut cursor = std::io::Cursor::new(line.as_mut_slice());
        let _ = write!(
            cursor,
            "ERROR seh frame[{idx:02}]: 0x{:016X}\n",
            *frame as usize
        );
        let pos = cursor.position() as usize;
        // SAFETY: see SEH-handler comment — WriteFile is the lock-free path.
        unsafe {
            let mut written: u32 = 0;
            WriteFile(
                stderr,
                line.as_ptr(),
                pos as u32,
                &mut written,
                core::ptr::null_mut(),
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn format_seh_line<'a>(buf: &'a mut [u8; 64], code: u32, addr: usize) -> &'a [u8] {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(buf.as_mut_slice());
    let _ = write!(cursor, "ERROR seh: code=0x{code:08X} addr=0x{addr:016X}\n");
    let pos = cursor.position() as usize;
    &buf[..pos]
}

#[cfg(target_os = "windows")]
fn write_module_list(stderr: windows_sys::Win32::Foundation::HANDLE) {
    use std::io::Write;
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::ProcessStatus::{
        EnumProcessModules, GetModuleBaseNameW, GetModuleInformation, MODULEINFO,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    const MAX_MODULES: usize = 256;
    let mut handles: [HMODULE; MAX_MODULES] = [core::ptr::null_mut(); MAX_MODULES];
    let mut needed: u32 = 0;
    // SAFETY: `handles` is a stack array of `HMODULE`; we pass its length
    // in bytes; `needed` is a stack-owned `u32`.
    let proc_h = unsafe { GetCurrentProcess() };
    // SAFETY: same; the call writes into `handles` and `needed` only.
    let enum_ok = unsafe {
        EnumProcessModules(
            proc_h,
            handles.as_mut_ptr(),
            (handles.len() * core::mem::size_of::<HMODULE>()) as u32,
            &mut needed,
        )
    };
    if enum_ok == 0 {
        return;
    }
    let count = ((needed as usize) / core::mem::size_of::<HMODULE>()).min(MAX_MODULES);

    let mut line = [0u8; 256];
    let mut name_w = [0u16; 96];
    for handle in handles.iter().take(count) {
        let mut info: MODULEINFO = MODULEINFO {
            lpBaseOfDll: core::ptr::null_mut(),
            SizeOfImage: 0,
            EntryPoint: core::ptr::null_mut(),
        };
        // SAFETY: stack-owned `info`; `*handle` is a live HMODULE in our process.
        let info_ok = unsafe {
            GetModuleInformation(
                proc_h,
                *handle,
                &mut info,
                core::mem::size_of::<MODULEINFO>() as u32,
            )
        };
        if info_ok == 0 {
            continue;
        }
        // SAFETY: `name_w` is a stack buffer of u16; passing length in chars.
        let name_len = unsafe {
            GetModuleBaseNameW(proc_h, *handle, name_w.as_mut_ptr(), name_w.len() as u32)
        };
        let mut cursor = std::io::Cursor::new(line.as_mut_slice());
        let _ = write!(
            cursor,
            "ERROR seh module: base=0x{:016X} size=0x{:08X} name=",
            info.lpBaseOfDll as usize, info.SizeOfImage
        );
        for ch in name_w.iter().take(name_len as usize) {
            let b = if (0x20..0x7F).contains(ch) {
                *ch as u8
            } else {
                b'?'
            };
            let _ = cursor.write_all(&[b]);
        }
        let _ = cursor.write_all(b"\n");
        let pos = cursor.position() as usize;
        // SAFETY: see SEH-handler comment — WriteFile is the lock-free path.
        unsafe {
            let mut written: u32 = 0;
            WriteFile(
                stderr,
                line.as_ptr(),
                pos as u32,
                &mut written,
                core::ptr::null_mut(),
            );
        }
    }
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

    install_subscriber(Mutex::new(file))
}

fn install_subscriber<W>(writer: W) -> io::Result<()>
where
    W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + Send + Sync + 'static,
{
    let filter = host_filter();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);

    install(fmt_layer, filter)
}

fn install_worker_subscriber<W>(writer: W) -> io::Result<()>
where
    W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + Send + Sync + 'static,
{
    let filter = Targets::new()
        .with_target("steamlens_app", Level::TRACE)
        .with_target("steamlens_core", Level::TRACE)
        .with_target("steamlens_vdf", Level::TRACE)
        .with_target("panic", Level::ERROR)
        .with_default(LevelFilter::OFF);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .with_level(true)
        .without_time();

    install(fmt_layer, filter)
}

fn host_filter() -> Targets {
    let default_level = std::env::var("STEAMLENS_LOG")
        .ok()
        .and_then(|s| s.parse::<Level>().ok())
        .unwrap_or(Level::INFO);
    Targets::new()
        .with_target("steamlens_app", default_level)
        .with_target("steamlens_core", default_level)
        .with_target("steamlens_vdf", default_level)
        .with_target("worker", default_level)
        .with_target("probe", default_level)
        .with_target("panic", Level::ERROR)
        .with_default(LevelFilter::OFF)
}

fn install<L>(fmt_layer: L, filter: Targets) -> io::Result<()>
where
    L: tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync + 'static,
{
    let subscriber = tracing_subscriber::registry().with(fmt_layer).with(filter);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| io::Error::other("logging already initialized"))?;

    std::panic::set_hook(Box::new(|info| {
        tracing::error!(target: "panic", "PANIC: {info}");
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "{bt}");
    }));

    Ok(())
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
            tracing::info!("test marker abc123");

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
