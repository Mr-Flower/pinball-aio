//! Scritture e copie che non lasciano mai un file a metà: si passa da un `.part` accanto
//! alla destinazione e lo si rinomina solo a lavoro finito.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn part(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = part(path);
    fs::write(&temp, bytes)?;
    fs::rename(&temp, path)
}

pub fn copy_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let temp = part(to);
    fs::copy(from, &temp)?;
    fs::rename(&temp, to)
}

/// Copia una cartella con tutto il suo contenuto.
pub fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            copy_atomic(&entry.path(), &target)?;
        }
    }
    Ok(())
}
