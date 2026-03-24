//! Runtime panic surface.

/// Aborts the process with a Draton-style panic report.
pub fn draton_panic(msg: *const libc::c_char, file: *const libc::c_char, line: u32) -> ! {
    let message = crate::scheduler::coreroutine::c_string(msg);
    let file_name = crate::scheduler::coreroutine::c_string(file);
    eprintln!("Draton panic at {file_name}:{line}: {message}");
    std::process::abort();
}
