use anyhow::{Context, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

/// Stage on the destination filesystem, flush, then atomically replace the file.
/// A failed write leaves the previous destination intact.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    if path.exists() {
        let mut backup = tempfile::NamedTempFile::new_in(parent)?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut backup)?;
        backup.as_file().sync_all()?;
        backup
            .persist(backup_path(path))
            .context("Could not preserve the previous file")?;
    }
    staged
        .persist(path)
        .context("Could not replace the saved file")?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
