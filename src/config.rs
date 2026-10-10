//! Scrive la configurazione dei due programmi, così l'utente non imposta nessun percorso.

use crate::ini;
use crate::paths::Paths;
use std::fs;
use std::io;
use std::path::Path;

/// Impostazioni iniziali del motore per un solo monitor in orizzontale, messe a punto su
/// una grafica integrata (Radeon 680M) a 2560x1440:
/// - vista desktop a schermo intero con vsync. Il predefinito del motore, "frame pacing",
///   dava 52 fps medi con cali a 25. `MaxFramerate = 60` vale solo se si spegne il vsync
///   (`SyncMode = 0`): con il vsync acceso comanda la frequenza del monitor;
/// - bloom e occlusione ambientale spenti, riflessi solo statici;
/// - backglass dentro la finestra del tavolo, dove lo mette il motore;
/// - DMD in alto a sinistra, largo circa un terzo dello schermo: lì copre il DMD che
///   molti tavoli disegnano da sé nella vista desktop, che altrimenti si vedrebbe doppio.
///
/// Le misure del DMD sono in pixel dell'immagine disegnata (schermo × `AAFactor`): chi
/// cambia `AAFactor` deve riscalarle.
/// Il file viene creato una volta sola: dopo, le modifiche dell'utente restano.
fn default_vpx_ini(paths: &Paths, (width, _height): (u32, u32)) -> String {
    let dmd_width = width * 11 / 32;
    format!(
        "[Player]\n\
         BGSet = 0\n\
         PlayfieldFullScreen = 1\n\
         SyncMode = 1\n\
         MaxFramerate = 60\n\
         ForceBloomOff = 1\n\
         DisableAO = 1\n\
         PFReflection = 2\n\
         \n\
         [Backglass]\n\
         BackglassOutput = 2\n\
         \n\
         [ScoreView]\n\
         ScoreViewOutput = 2\n\
         ScoreViewWndX = 0\n\
         ScoreViewWndY = 0\n\
         ScoreViewWidth = {dmd_width}\n\
         ScoreViewHeight = {}\n\
         \n\
         [Plugin.PinMAME]\n\
         PinMAMEPath = {}\n",
        dmd_width / 4,
        paths.pinmame().display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn default_ini_scales_the_dmd_with_the_screen() {
        let paths = Paths { root: PathBuf::from("/data") };
        let ini = default_vpx_ini(&paths, (2560, 1440));
        assert_eq!(ini::get(&ini, "ScoreView", "ScoreViewWidth").as_deref(), Some("880"));
        assert_eq!(ini::get(&ini, "ScoreView", "ScoreViewHeight").as_deref(), Some("220"));
        assert_eq!(ini::get(&ini, "Player", "SyncMode").as_deref(), Some("1"));
        assert_eq!(ini::get(&ini, "Player", "MaxFramerate").as_deref(), Some("60"));
        assert_eq!(ini::get(&ini, "Player", "ForceBloomOff").as_deref(), Some("1"));
        assert_eq!(ini::get(&ini, "Plugin.PinMAME", "PinMAMEPath").as_deref(), Some("/data/pinmame"));
        let small = default_vpx_ini(&paths, (1920, 1080));
        assert_eq!(ini::get(&small, "ScoreView", "ScoreViewWidth").as_deref(), Some("660"));
    }
}

/// `screen` è la risoluzione del monitor in pixel, usata solo per creare `VPinballX.ini`.
pub fn write(paths: &Paths, vpx_exe: &Path, screen: (u32, u32)) -> io::Result<()> {
    if !paths.vpx_ini().exists() {
        fs::write(paths.vpx_ini(), default_vpx_ini(paths, screen))?;
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
