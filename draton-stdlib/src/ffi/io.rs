use std::io::{self, BufRead, Write};

fn normalize_line(line: String) -> String {
    line.trim_end_matches(['\r', '\n']).to_string()
}

/// Prints a string to stdout without a newline.
pub fn print(s: impl AsRef<str>) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(s.as_ref().as_bytes());
    let _ = stdout.flush();
}

/// Prints a string to stdout with a newline.
pub fn println(s: impl AsRef<str>) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(s.as_ref().as_bytes());
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}

/// Prints a string to stderr without a newline.
pub fn eprint(s: impl AsRef<str>) {
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(s.as_ref().as_bytes());
    let _ = stderr.flush();
}

/// Prints a string to stderr with a newline.
pub fn eprintln(s: impl AsRef<str>) {
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(s.as_ref().as_bytes());
    let _ = stderr.write_all(b"\n");
    let _ = stderr.flush();
}

/// Reads a single line from stdin.
pub fn readline() -> String {
    let stdin = io::stdin();
    let mut line = String::new();
    match stdin.read_line(&mut line) {
        Ok(_) => normalize_line(line),
        Err(_) => String::new(),
    }
}

/// Reads every line from stdin until EOF.
pub fn readlines() -> Vec<String> {
    let stdin = io::stdin();
    stdin
        .lock()
        .lines()
        .map(|line| line.map(normalize_line).unwrap_or_default())
        .collect()
}
