use std::sync::OnceLock;

/// Platform I/O interface for bare-metal targets.
pub trait DratonPlatform: Send + Sync {
    /// Write bytes to standard output.
    fn write_stdout(&self, bytes: &[u8]);
    /// Write bytes to standard error.
    fn write_stderr(&self, bytes: &[u8]);
    /// Read a line from standard input. Returns empty string if unavailable.
    fn read_line(&self) -> Vec<u8>;
    /// Halt with a panic message. Must not return.
    fn panic_halt(&self, msg: &str) -> !;
}

#[cfg(feature = "std-io")]
pub struct HostedPlatform;

#[cfg(feature = "std-io")]
impl DratonPlatform for HostedPlatform {
    fn write_stdout(&self, bytes: &[u8]) {
        #[cfg(unix)]
        {
            write_fd(libc::STDOUT_FILENO, bytes);
        }
        #[cfg(windows)]
        {
            write_fd(1, bytes);
        }
    }

    fn write_stderr(&self, bytes: &[u8]) {
        #[cfg(unix)]
        {
            write_fd(libc::STDERR_FILENO, bytes);
        }
        #[cfg(windows)]
        {
            write_fd(2, bytes);
        }
    }

    fn read_line(&self) -> Vec<u8> {
        use std::io::BufRead;

        let stdin = std::io::stdin();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(_) => normalize_line(line).into_bytes(),
            Err(_) => Vec::new(),
        }
    }

    fn panic_halt(&self, msg: &str) -> ! {
        self.write_stderr(msg.as_bytes());
        self.write_stderr(b"\n");
        std::process::abort();
    }
}

#[cfg(feature = "std-io")]
fn normalize_line(line: String) -> String {
    line.trim_end_matches(['\r', '\n']).to_string()
}

#[cfg(feature = "std-io")]
fn write_fd(fd: libc::c_int, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    #[cfg(unix)]
    {
        // SAFETY: `bytes` is a valid buffer for the duration of the syscall.
        unsafe {
            let _ = libc::write(fd, bytes.as_ptr().cast::<libc::c_void>(), bytes.len());
        }
    }
    #[cfg(windows)]
    {
        unsafe extern "C" {
            fn _write(
                fd: libc::c_int,
                buffer: *const libc::c_void,
                count: libc::c_uint,
            ) -> libc::c_int;
        }

        let mut written = 0usize;
        while written < bytes.len() {
            let remaining = bytes.len() - written;
            let chunk = remaining.min(libc::c_uint::MAX as usize);
            let ptr = unsafe { bytes.as_ptr().add(written) }.cast::<libc::c_void>();
            // SAFETY: `ptr` points into `bytes`, and `chunk` is clamped to the CRT API width.
            let result = unsafe { _write(fd, ptr, chunk as libc::c_uint) };
            if result <= 0 {
                break;
            }
            written += result as usize;
        }
    }
}

pub(crate) fn default_platform() -> Option<Box<dyn DratonPlatform>> {
    #[cfg(feature = "std-io")]
    {
        return Some(Box::new(HostedPlatform));
    }
    #[cfg(not(feature = "std-io"))]
    {
        None
    }
}

pub(crate) fn registry() -> &'static OnceLock<Box<dyn DratonPlatform>> {
    static PLATFORM: OnceLock<Box<dyn DratonPlatform>> = OnceLock::new();
    &PLATFORM
}
