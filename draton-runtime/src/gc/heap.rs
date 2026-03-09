use std::collections::HashMap;
use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::sync::Mutex;

use super::config::GcConfig;

pub const LARGE_OBJECT_THRESHOLD: usize = 4 * 1024;
pub const GC_MARKED: u8 = 1 << 0;
pub const GC_OLD: u8 = 1 << 1;
pub const GC_PINNED: u8 = 1 << 2;
pub const GC_FINALIZED: u8 = 1 << 3;

/// The GC-managed header stored before each payload.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjHeader {
    pub size: u32,
    pub type_id: u16,
    pub gc_flags: u8,
    pub ref_count: u8,
    pub forwarding: *mut u8,
}

/// Logical heap space used by the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapSpace {
    YoungEden,
    YoungSurvivor,
    Old,
    Large,
}

#[derive(Debug)]
pub struct ObjectRecord {
    pub bytes: Box<[u8]>,
    pub payload: usize,
    pub space: HeapSpace,
    pub minor_survivals: u8,
    pub protected: bool,
}

impl ObjectRecord {
    fn new(size: usize, type_id: u16, space: HeapSpace) -> Option<Self> {
        let total = size.checked_add(size_of::<ObjHeader>())?;
        let mut bytes = vec![0u8; total].into_boxed_slice();
        let base = NonNull::new(bytes.as_mut_ptr())?;
        let payload = NonNull::new(base.as_ptr().wrapping_add(size_of::<ObjHeader>()))?;
        let header = ObjHeader {
            size: size as u32,
            type_id,
            gc_flags: match space {
                HeapSpace::Old => GC_OLD,
                _ => 0,
            },
            ref_count: 0,
            forwarding: ptr::null_mut(),
        };
        // SAFETY: `base` points to the beginning of the boxed allocation and has
        // enough space for `ObjHeader` followed by `size` payload bytes.
        unsafe {
            ptr::write(base.as_ptr().cast::<ObjHeader>(), header);
        }
        Some(Self {
            bytes,
            payload: payload.as_ptr() as usize,
            space,
            minor_survivals: 0,
            protected: true,
        })
    }

    pub fn header(&self) -> ObjHeader {
        // SAFETY: The allocation always begins with a valid `ObjHeader`.
        unsafe { ptr::read(self.bytes.as_ptr().cast::<ObjHeader>()) }
    }

    pub fn set_header(&mut self, header: ObjHeader) {
        // SAFETY: The allocation always begins with a valid `ObjHeader`.
        unsafe {
            ptr::write(self.bytes.as_mut_ptr().cast::<ObjHeader>(), header);
        }
    }

    pub fn contains(&self, payload: *mut u8) -> bool {
        self.payload == payload as usize
    }
}

#[derive(Debug)]
pub struct HeapState {
    pub config: GcConfig,
    pub objects: Vec<ObjectRecord>,
    pub roots: HashMap<usize, usize>,
    pub minor_cycles: u64,
    pub major_cycles: u64,
}

impl HeapState {
    pub fn new(config: GcConfig) -> Self {
        Self {
            config: config.normalized(),
            objects: Vec::new(),
            roots: HashMap::new(),
            minor_cycles: 0,
            major_cycles: 0,
        }
    }

    pub fn alloc(&mut self, size: usize, type_id: u16) -> *mut u8 {
        let space = if size > LARGE_OBJECT_THRESHOLD {
            HeapSpace::Large
        } else {
            HeapSpace::YoungEden
        };
        let Some(record) = ObjectRecord::new(size, type_id, space) else {
            return ptr::null_mut();
        };
        let payload = record.payload as *mut u8;
        self.objects.push(record);
        self.roots.insert(payload as usize, 1);
        payload
    }

    pub fn alloc_array(&mut self, elem_size: usize, len: usize, type_id: u16) -> *mut u8 {
        let Some(size) = elem_size.checked_mul(len) else {
            return ptr::null_mut();
        };
        self.alloc(size, type_id)
    }

    pub fn header_of(&self, payload: *mut u8) -> Option<ObjHeader> {
        self.objects
            .iter()
            .find(|record| record.contains(payload))
            .map(ObjectRecord::header)
    }

    pub fn space_of(&self, payload: *mut u8) -> Option<HeapSpace> {
        self.objects
            .iter()
            .find(|record| record.contains(payload))
            .map(|record| record.space)
    }

    pub fn pin(&mut self, payload: *mut u8) {
        if let Some(record) = self
            .objects
            .iter_mut()
            .find(|record| record.contains(payload))
        {
            let mut header = record.header();
            header.gc_flags |= GC_PINNED;
            record.set_header(header);
        }
    }

    pub fn unpin(&mut self, payload: *mut u8) {
        if let Some(record) = self
            .objects
            .iter_mut()
            .find(|record| record.contains(payload))
        {
            let mut header = record.header();
            header.gc_flags &= !GC_PINNED;
            record.set_header(header);
        }
    }

    pub fn protect(&mut self, payload: *mut u8) {
        if let Some(record) = self
            .objects
            .iter_mut()
            .find(|record| record.contains(payload))
        {
            record.protected = true;
            let counter = self.roots.entry(payload as usize).or_insert(0);
            *counter += 1;
        }
    }

    pub fn release(&mut self, payload: *mut u8) {
        if let Some(record) = self
            .objects
            .iter_mut()
            .find(|record| record.contains(payload))
        {
            record.protected = false;
        }
        if let Some(counter) = self.roots.get_mut(&(payload as usize)) {
            if *counter > 1 {
                *counter -= 1;
            } else {
                self.roots.remove(&(payload as usize));
            }
        }
    }

    pub fn current_usage(&self) -> usize {
        self.objects.iter().map(|record| record.bytes.len()).sum()
    }
}

/// The global GC runtime handle.
#[derive(Debug)]
pub struct GcRuntime {
    pub heap: Mutex<HeapState>,
}

impl GcRuntime {
    pub fn new(config: GcConfig) -> Self {
        Self {
            heap: Mutex::new(HeapState::new(config)),
        }
    }
}
