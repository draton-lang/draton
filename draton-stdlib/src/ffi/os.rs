use std::env;
use std::ffi::CStr;

/// Returns an environment variable when present.
pub fn env_var(key: impl AsRef<str>) -> Option<String> {
    env::var(key.as_ref()).ok()
}

/// Sets an environment variable.
pub fn set_env(key: impl AsRef<str>, value: impl AsRef<str>) {
    env::set_var(key.as_ref(), value.as_ref());
}

/// Returns the process arguments.
pub fn args() -> Vec<String> {
    env::args().collect()
}

/// Exits the process with the provided code.
pub fn exit(code: i64) -> ! {
    std::process::exit(code as i32)
}

/// Returns the current process id.
pub fn pid() -> i64 {
    // SAFETY: `getpid` has no preconditions and returns the current process id.
    unsafe { libc::getpid() as i64 }
}

/// Returns the current platform name.
pub fn platform() -> String {
    if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Returns the current architecture name.
pub fn arch() -> String {
    env::consts::ARCH.to_string()
}

/// Returns the system hostname, or an empty string on failure.
pub fn hostname() -> String {
    #[cfg(unix)]
    {
        let mut buffer = [0 as libc::c_char; 256];
        // SAFETY: `buffer` is valid for writes of its full length.
        let rc = unsafe { libc::gethostname(buffer.as_mut_ptr(), buffer.len()) };
        if rc != 0 {
            return String::new();
        }
        // SAFETY: `gethostname` writes a C string into `buffer` on success.
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .trim_end_matches('\0')
            .to_string()
    }
    #[cfg(not(unix))]
    {
        env::var("COMPUTERNAME").unwrap_or_default()
    }
}
