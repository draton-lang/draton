use std::fs;
use std::io::Write;
use std::path::Path;

use crate::FsError;

fn fs_error(error: std::io::Error) -> FsError {
    FsError::new(error.to_string())
}

/// Reads a UTF-8 file into a string.
pub fn read(path: impl AsRef<Path>) -> Result<String, FsError> {
    fs::read_to_string(path).map_err(fs_error)
}

/// Writes a UTF-8 string to a file, replacing any existing contents.
pub fn write(path: impl AsRef<Path>, content: impl AsRef<str>) -> Result<(), FsError> {
    fs::write(path, content.as_ref()).map_err(fs_error)
}

/// Appends a UTF-8 string to a file, creating it when needed.
pub fn append(path: impl AsRef<Path>, content: impl AsRef<str>) -> Result<(), FsError> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(fs_error)?;
    file.write_all(content.as_ref().as_bytes())
        .map_err(fs_error)
}

/// Deletes a file or empty directory.
pub fn delete(path: impl AsRef<Path>) -> Result<(), FsError> {
    let path = path.as_ref();
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir(path).map_err(fs_error),
        Ok(_) => fs::remove_file(path).map_err(fs_error),
        Err(error) => Err(fs_error(error)),
    }
}

/// Returns whether the path exists.
pub fn exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}

/// Creates a directory and all missing parents.
pub fn mkdir(path: impl AsRef<Path>) -> Result<(), FsError> {
    fs::create_dir_all(path).map_err(fs_error)
}

/// Reads directory entries as sorted file names.
pub fn readdir(path: impl AsRef<Path>) -> Result<Vec<String>, FsError> {
    let entries = fs::read_dir(path).map_err(fs_error)?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(fs_error)?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
}

/// Copies a file.
pub fn copy(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), FsError> {
    fs::copy(src, dst).map(|_| ()).map_err(fs_error)
}

/// Moves or renames a path.
pub fn move_path(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), FsError> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::EXDEV) => {
            fs::copy(src, dst).map_err(fs_error)?;
            delete(src)
        }
        Err(error) => Err(fs_error(error)),
    }
}
