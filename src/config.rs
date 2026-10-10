//! Scrive la configurazione dei due programmi, così l'utente non imposta nessun percorso.

use crate::ini;
use crate::paths::Paths;
use std::fs;
use std::io;
use std::path::Path;

/// Impostazioni iniziali del motore per un solo monitor in orizzontale:
/// vista desktop a schermo intero, con backglass e DMD dentro la finestra del tavolo.
/// Il file viene creato una volta sola: dopo, le modifiche dell'utente restano.
fn default_vpx_ini(paths: &Paths) -> String {
    format!(
        "[Player]\n\
         BGSet = 0\n\
         PlayfieldFullScreen = 1\n\
         \n\
         [Backglass]\n\
         BackglassOutput = 2\n\
         \n\
         [ScoreView]\n\
         ScoreViewOutput = 2\n\
         \n\
         [Plugin.PinMAME]\n\
         PinMAMEPath = {}\n",
        paths.pinmame().display()
    )
}

pub fn write(paths: &Paths, vpx_exe: &Path) -> io::Result<()> {
    if !paths.vpx_ini().exists() {
        fs::write(paths.vpx_ini(), default_vpx_ini(paths))?;
    }

    // I percorsi vengono riscritti a ogni avvio: restano giusti anche se la cartella
    // dati viene spostata o un aggiornamento cambia posizione all'eseguibile.
    let mut text = fs::read_to_string(paths.vpinfe_ini()).unwrap_or_default();
    let vpx_ini = paths.vpx_ini();
    for (key, value) in [
        ("vpxbinpath", vpx_exe),
        ("tablerootdir", &paths.tables()),
        ("vpxinipath", &vpx_ini),
        ("globalinioverride", &vpx_ini),
    ] {
        text = ini::set(&text, "Settings", key, &value.display().to_string());
    }
    // Il motore su Linux scrive log e dati in $XDG_DATA_HOME/VPinballX: lo si porta dentro
    // la cartella dati, senza toccare un valore che l'utente ha impostato da sé.
    #[cfg(target_os = "linux")]
    {
        let current = ini::get(&text, "Settings", "vpxlaunchenv").unwrap_or_default();
        if current.is_empty() || current.starts_with("XDG_DATA_HOME=") {
            let dir = paths.vpx_data().display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
            text = ini::set(&text, "Settings", "vpxlaunchenv", &format!("XDG_DATA_HOME=\"{dir}\""));
        }
    }
    fs::write(paths.vpinfe_ini(), text)
}
