//! Runtime panic surface.

use std::ffi::CStr;

/// Aborts the process with a Draton-style panic report.
pub fn draton_panic(msg: *const libc::c_char, file: *const libc::c_char, line: u32) -> ! {
    let message = c_string(msg);
    let file_name = c_string(file);
    crate::platform().panic_halt(&format!("Draton panic at {file_name}:{line}: {message}"));
}

fn c_string(ptr: *const libc::c_char) -> String {
    if ptr.is_null() {
        return "<null>".to_string();
    }
    // SAFETY: The runtime ABI passes valid null-terminated strings here.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}
