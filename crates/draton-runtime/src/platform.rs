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
        write_fd(libc::STDOUT_FILENO, bytes);
    }

    fn write_stderr(&self, bytes: &[u8]) {
        write_fd(libc::STDERR_FILENO, bytes);
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
    // SAFETY: `bytes` is a valid buffer for the duration of the syscall.
    unsafe {
        let _ = libc::write(fd, bytes.as_ptr().cast::<libc::c_void>(), bytes.len());
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
