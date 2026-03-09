use super::heap::{GcRuntime, GC_MARKED};

impl GcRuntime {
    pub fn write_barrier(&self, obj: *mut u8, _field: *mut u8, new_val: *mut u8) {
        let Ok(mut heap) = self.heap.lock() else {
            return;
        };
        if let Some(parent) = heap.objects.iter_mut().find(|record| record.contains(obj)) {
            let mut header = parent.header();
            header.gc_flags |= GC_MARKED;
            parent.set_header(header);
        }
        if !new_val.is_null() {
            heap.protect(new_val);
            if let Some(child) = heap
                .objects
                .iter_mut()
                .find(|record| record.contains(new_val))
            {
                let mut header = child.header();
                header.gc_flags |= GC_MARKED;
                child.set_header(header);
            }
        }
    }
}
