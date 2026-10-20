//! Avvia un tavolo nel motore con le impostazioni della cartella dati, come faceva VPinFE:
//! `VPinballX -ini <config/VPinballX.ini> -play <tavolo.vpx>`.

use crate::paths::Paths;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub fn start(paths: &Paths, engine: &Path, table: &Path) -> Result<Child> {
    let mut command = Command::new(engine);
    command.arg("-ini").arg(paths.vpx_ini()).arg("-play").arg(table).stdin(Stdio::null());
    if let Some(dir) = engine.parent() {
        command.current_dir(dir);
    }
    // Su Linux il motore scrive log e dati in $XDG_DATA_HOME/VPinballX: li si tiene
    // dentro la cartella dati.
    #[cfg(target_os = "linux")]
    command.env("XDG_DATA_HOME", paths.vpx_data());
    command.spawn().with_context(|| format!("impossibile avviare {}", engine.display()))
}
