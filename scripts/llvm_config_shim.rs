use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn prefix_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .expect("llvm-config shim must live in <prefix>/bin")
}

fn real_llvm_config() -> Option<PathBuf> {
    let path = prefix_dir().join("bin").join("llvm-config-real.exe");
    path.exists().then_some(path)
}

fn print_and_exit(message: &str) -> ExitCode {
    println!("{message}");
    ExitCode::SUCCESS
}

fn libnames() -> io::Result<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(prefix_dir().join("lib"))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("lib") {
            continue;
        }
        let stem = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some(stem) => stem,
            None => continue,
        };
        if stem.starts_with("LLVM") || stem == "LTO" || stem == "Remarks" {
            names.push(format!("{stem}.lib"));
        }
    }
    names.sort();
    names.dedup();
    Ok(names.join(" "))
}

fn delegate(args: &[String]) -> ExitCode {
    let Some(real) = real_llvm_config() else {
        eprintln!("unsupported llvm-config arguments without bundled llvm-config-real.exe: {args:?}");
        return ExitCode::from(1);
    };
    let output = match Command::new(real).args(args).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("failed to execute llvm-config-real.exe: {error}");
            return ExitCode::from(1);
        }
    };
    let _ = io::stdout().write_all(&output.stdout);
    let _ = io::stderr().write_all(&output.stderr);
    ExitCode::from(output.status.code().unwrap_or(1) as u8)
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--version" => print_and_exit("14.0.6"),
        [flag] if flag == "--libdir" => print_and_exit(&prefix_dir().join("lib").display().to_string()),
        [flag] if flag == "--cflags" => {
            print_and_exit(&format!("-I{}", prefix_dir().join("include").display()))
        }
        [flag] if flag == "--build-mode" => print_and_exit("Release"),
        [flag, link] if flag == "--libnames" && link == "--link-static" => match libnames() {
            Ok(value) => print_and_exit(&value),
            Err(error) => {
                eprintln!("failed to enumerate LLVM static libraries: {error}");
                ExitCode::from(1)
            }
        },
        [flag, link] if flag == "--system-libs" && link == "--link-static" => delegate(&args),
        _ => delegate(&args),
    }
}
