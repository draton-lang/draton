/// Runtime GC configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GcConfig {
    /// Young generation bump-pointer arena size in bytes (default: 4 MB).
    pub young_size: usize,
    /// Old generation heap budget in bytes; triggers major GC when exceeded (default: 64 MB).
    pub old_size: usize,
    /// Maximum total heap size in bytes (default: 512 MB).
    pub max_heap: usize,
    /// Fraction of `old_size` at which a major GC cycle is triggered (default: 0.75).
    pub gc_threshold: f64,
    /// Soft upper bound on a single GC pause in nanoseconds (default: 1 ms).
    /// The incremental major GC adapts its slice size to stay within this budget.
    pub pause_target_ns: u64,
    /// Objects larger than this bypass the young gen and go to the large-object space
    /// (default: 32 KB).
    pub large_threshold: usize,
    /// Whether the runtime may tune GC thresholds and cache budgets from live telemetry.
    pub autotune: bool,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            young_size:      4  * 1024 * 1024,
            old_size:        64 * 1024 * 1024,
            max_heap:       512 * 1024 * 1024,
            gc_threshold:   0.75,
            pause_target_ns: 1_000_000,
            large_threshold: 32 * 1024,
            autotune: true,
        }
    }
}

impl GcConfig {
    /// Returns a sanitized config with all values within sensible bounds.
    pub fn normalized(self) -> Self {
        let young_size      = self.young_size.max(256 * 1024);
        let old_size        = self.old_size.max(young_size * 4);
        let max_heap        = self.max_heap.max(old_size + young_size);
        let gc_threshold    = self.gc_threshold.clamp(0.1, 0.95);
        let pause_target_ns = self.pause_target_ns.max(1_000);
        let large_threshold = self.large_threshold.max(4 * 1024);
        Self {
            young_size,
            old_size,
            max_heap,
            gc_threshold,
            pause_target_ns,
            large_threshold,
            autotune: self.autotune,
        }
    }
}
