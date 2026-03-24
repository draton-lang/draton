use std::fs as stdfs;

use draton_stdlib::fs;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn read_write_append_delete_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file = dir.path().join("note.txt");

    fs::write(&file, "hello").expect("write");
    fs::append(&file, " world").expect("append");

    assert_eq!(fs::read(&file).expect("read"), "hello world");
    assert!(fs::exists(&file));

    fs::delete(&file).expect("delete");
    assert!(!fs::exists(&file));
}

#[test]
fn mkdir_readdir_copy_and_move_work() {
    let dir = tempdir().expect("tempdir");
    let nested = dir.path().join("nested/child");
    fs::mkdir(&nested).expect("mkdir");

    let src = nested.join("src.txt");
    let copy = nested.join("copy.txt");
    let moved = nested.join("moved.txt");

    fs::write(&src, "data").expect("write");
    fs::copy(&src, &copy).expect("copy");
    fs::move_path(&copy, &moved).expect("move");

    let entries = fs::readdir(&nested).expect("readdir");
    assert_eq!(
        entries,
        vec!["moved.txt".to_string(), "src.txt".to_string()]
    );
}

#[test]
fn read_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("missing.txt");

    let error = fs::read(&missing).expect_err("missing file should error");
    assert!(error.message().contains("No such file") || error.message().contains("cannot find"));
}

#[test]
fn delete_missing_path_returns_error() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("gone.txt");

    let error = fs::delete(&missing).expect_err("delete should error");
    assert!(error.message().contains("No such file") || error.message().contains("cannot find"));
}

#[test]
fn move_falls_back_cleanly_for_regular_rename() {
    let dir = tempdir().expect("tempdir");
    let src = dir.path().join("before.txt");
    let dst = dir.path().join("after.txt");
    fs::write(&src, "payload").expect("write");

    fs::move_path(&src, &dst).expect("move");

    assert_eq!(std::fs::read_to_string(&dst).expect("read dst"), "payload");
    assert!(!src.exists());
    let _ = stdfs::remove_file(dst);
}
