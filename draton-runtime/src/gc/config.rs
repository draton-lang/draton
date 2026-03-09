/// Runtime GC configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GcConfig {
    pub heap_size: usize,
    pub max_heap: usize,
    pub gc_threshold: f64,
    pub pause_target_ns: u64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            heap_size: 512 * 1024 * 1024,
            max_heap: 512 * 1024 * 1024,
            gc_threshold: 0.75,
            pause_target_ns: 1_000_000,
        }
    }
}

impl GcConfig {
    /// Returns a sanitized config with valid bounds.
    pub fn normalized(self) -> Self {
        let heap_size = self.heap_size.max(64 * 1024 * 1024);
        let max_heap = self.max_heap.max(heap_size);
        let gc_threshold = self.gc_threshold.clamp(0.1, 0.95);
        let pause_target_ns = self.pause_target_ns.max(1_000);
        Self {
            heap_size,
            max_heap,
            gc_threshold,
            pause_target_ns,
        }
    }
}
