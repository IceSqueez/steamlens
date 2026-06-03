use std::io;
use std::path::PathBuf;

pub struct ChildLifetimeGuard {
    #[cfg(target_os = "windows")]
    job_handle: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: the Windows variant holds a Job Object kernel handle (raw `*mut
// c_void`). Win32 Job Object handles are thread-safe for the only operation
// we perform on them — `CloseHandle` in `Drop` — per
// https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle.
// We never read the handle's contents or mutate via `&` reference, so Send
// (and Sync, by extension) is sound.
#[cfg(target_os = "windows")]
unsafe impl Send for ChildLifetimeGuard {}
#[cfg(target_os = "windows")]
unsafe impl Sync for ChildLifetimeGuard {}

#[cfg(target_os = "windows")]
impl Drop for ChildLifetimeGuard {
    fn drop(&mut self) {
        if !self.job_handle.is_null() {
            // SAFETY: handle minted by `CreateJobObjectW`; we own it
            // exclusively for the lifetime of this guard.
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.job_handle);
            }
        }
    }
}

pub fn associate_kill_on_parent_exit(pid: u32) -> io::Result<ChildLifetimeGuard> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        use std::mem::size_of;
        use std::ptr;
        use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        // SAFETY: pid is a valid u32; OpenProcess returns null on failure
        // which we explicitly check.
        let proc_handle = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, FALSE, pid) };
        if proc_handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: nulls are valid for both lpJobAttributes and lpName per MSDN
        // (anonymous job, default security descriptor).
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            let err = io::Error::last_os_error();
            // SAFETY: proc_handle was successfully opened above.
            unsafe { CloseHandle(proc_handle) };
            return Err(err);
        }

        // SAFETY: zeroing a `#[repr(C)]` struct of integer fields is a
        // well-defined valid value per Win32 ABI. We only set
        // BasicLimitInformation.LimitFlags before the syscall.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // SAFETY: `info` is a fully-initialized struct of the size we pass
        // in `cbJobObjectInformationLength`.
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            // SAFETY: both handles were opened above.
            unsafe {
                CloseHandle(job);
                CloseHandle(proc_handle);
            }
            return Err(err);
        }

        // SAFETY: both handles are live; AssignProcessToJobObject requires
        // PROCESS_TERMINATE + PROCESS_SET_QUOTA which we requested.
        let assigned = unsafe { AssignProcessToJobObject(job, proc_handle) };

        // SAFETY: proc_handle is no longer needed regardless of assignment
        // outcome; the job retains its own reference to the process.
        unsafe { CloseHandle(proc_handle) };

        if assigned == 0 {
            let err = io::Error::last_os_error();
            // SAFETY: job handle still owned by us.
            unsafe { CloseHandle(job) };
            return Err(err);
        }

        Ok(ChildLifetimeGuard { job_handle: job })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid;
        Ok(ChildLifetimeGuard {})
    }
}

pub fn current_exe_resilient() -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Ok(PathBuf::from("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe()
    }
}
