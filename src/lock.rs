use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Opens the Go-compatible advisory lock for a data/config directory. The
/// returned file keeps the lock until it is dropped, including across threads
/// and separate ttype processes on supported platforms.
pub fn lock_directory(directory: &Path) -> io::Result<File> {
    fs::create_dir_all(directory)?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))?;
    file.lock()?;
    Ok(file)
}
