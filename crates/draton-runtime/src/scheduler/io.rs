//! Async I/O integration surface. The current implementation provides a small,
//! portable facade and a Linux `io_uring` holder when available.

/// Minimal runtime I/O driver.
#[derive(Default)]
pub struct IoDriver {
    #[cfg(target_os = "linux")]
    ring: Option<io_uring::IoUring>,
}

impl std::fmt::Debug for IoDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IoDriver")
    }
}

impl IoDriver {
    /// Creates a new best-effort I/O driver.
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                ring: io_uring::IoUring::new(8).ok(),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::default()
        }
    }

    /// Polls the platform I/O driver once.
    pub fn poll_once(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let _ = self.ring.as_mut();
        }
    }
}
