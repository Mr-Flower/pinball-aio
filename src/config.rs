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
fn default_vpx_ini(paths: &Paths, (width, _height): (u32, u32), embedded: u32) -> String {
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
         BackglassOutput = {embedded}\n\
         \n\
         [ScoreView]\n\
         ScoreViewOutput = {embedded}\n\
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

/// Valore di `BackglassOutput` / `ScoreViewOutput` che vuol dire "dentro la finestra del
/// tavolo". Fino alla build 3788 era 1 e 2 apriva una finestra a parte; nelle build
/// successive i due numeri sono scambiati.
pub fn embedded_output(vpx_tag: &str) -> u32 {
    let build = vpx_tag.split('-').nth(1).and_then(|n| n.parse::<u32>().ok());
    if build.is_some_and(|n| n <= 3788) { 1 } else { 2 }
}

fn swap_outputs(text: &str) -> String {
    let mut text = text.to_string();
    for (section, key) in [("Backglass", "BackglassOutput"), ("ScoreView", "ScoreViewOutput")] {
        let swapped = match ini::get(&text, section, key).as_deref() {
            Some("1") => "2",
            Some("2") => "1",
            _ => continue,
        };
        text = ini::set(&text, section, key, swapped);
    }
    text
}

/// Quando il motore passa da una numerazione all'altra, scambia 1 e 2 nelle due voci di
/// `VPinballX.ini`, così backglass e DMD restano dove l'utente li aveva.
pub fn follow_engine(paths: &Paths, old_tag: &str, new_tag: &str) -> io::Result<()> {
    if embedded_output(old_tag) == embedded_output(new_tag) {
        return Ok(());
    }
    match fs::read_to_string(paths.vpx_ini()) {
        Ok(text) => fs::write(paths.vpx_ini(), swap_outputs(&text)),
        Err(_) => Ok(()),
    }
}

/// Plugin che il motore accende da solo su Linux ("standalone") e lascia spenti su Windows,
/// dove si aspetta VPinMAME, B2S e gli altri installati a parte come oggetti COM: senza,
/// lo script del tavolo si ferma con "Necessario oggetto: 'Controller'".
const STANDALONE_PLUGINS: [&str; 9] =
    ["AltSound", "B2SLegacy", "FlexDMD", "PinMAME", "PUP", "ScoreView", "Serum", "WMP", "VNI"];

/// Accende i plugin di `STANDALONE_PLUGINS` che non hanno un valore: uno spento a mano
/// dall'utente (`Enable = 0`) resta spento.
fn enable_plugins(text: &str) -> String {
    let mut text = text.to_string();
    for plugin in STANDALONE_PLUGINS {
        let section = format!("Plugin.{plugin}");
        if ini::get(&text, &section, "Enable").unwrap_or_default().is_empty() {
            text = ini::set(&text, &section, "Enable", "1");
        }
    }
    text
}

/// `screen` è la risoluzione del monitor in pixel e `vpx_tag` la versione del motore
/// installata: servono solo per creare `VPinballX.ini`.
pub fn write(paths: &Paths, vpx_exe: &Path, vpx_tag: &str, screen: (u32, u32)) -> io::Result<()> {
    if !paths.vpx_ini().exists() {
        fs::write(paths.vpx_ini(), default_vpx_ini(paths, screen, embedded_output(vpx_tag)))?;
    }
    if cfg!(windows) {
        let text = fs::read_to_string(paths.vpx_ini())?;
        let enabled = enable_plugins(&text);
        if enabled != text {
            fs::write(paths.vpx_ini(), enabled)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn default_ini_scales_the_dmd_with_the_screen() {
        let paths = Paths { root: PathBuf::from("/data") };
        let ini = default_vpx_ini(&paths, (2560, 1440), embedded_output("v10.8.1-5436-af26b2d93"));
        assert_eq!(ini::get(&ini, "ScoreView", "ScoreViewOutput").as_deref(), Some("2"));
        assert_eq!(ini::get(&ini, "Backglass", "BackglassOutput").as_deref(), Some("2"));
        assert_eq!(ini::get(&ini, "ScoreView", "ScoreViewWidth").as_deref(), Some("880"));
        assert_eq!(ini::get(&ini, "ScoreView", "ScoreViewHeight").as_deref(), Some("220"));
        assert_eq!(ini::get(&ini, "Player", "SyncMode").as_deref(), Some("1"));
        assert_eq!(ini::get(&ini, "Player", "MaxFramerate").as_deref(), Some("60"));
        assert_eq!(ini::get(&ini, "Player", "ForceBloomOff").as_deref(), Some("1"));
        let pinmame = paths.pinmame().display().to_string();
        assert_eq!(ini::get(&ini, "Plugin.PinMAME", "PinMAMEPath"), Some(pinmame));
        let small = default_vpx_ini(&paths, (1920, 1080), embedded_output("v10.8.1-3788-2151290"));
        assert_eq!(ini::get(&small, "ScoreView", "ScoreViewOutput").as_deref(), Some("1"));
        assert_eq!(ini::get(&small, "ScoreView", "ScoreViewWidth").as_deref(), Some("660"));
    }

    #[test]
    fn plugins_without_a_value_are_enabled() {
        let ini = "[Plugin.PinMAME]\nPinMAMEPath = x\nEnable = \n\n[Plugin.PUP]\nEnable = 0\n";
        let enabled = enable_plugins(ini);
        assert_eq!(ini::get(&enabled, "Plugin.PinMAME", "Enable").as_deref(), Some("1"));
        assert_eq!(ini::get(&enabled, "Plugin.PinMAME", "PinMAMEPath").as_deref(), Some("x"));
        assert_eq!(ini::get(&enabled, "Plugin.PUP", "Enable").as_deref(), Some("0"));
        assert_eq!(ini::get(&enabled, "Plugin.ScoreView", "Enable").as_deref(), Some("1"));
        assert_eq!(enable_plugins(&enabled), enabled);
    }

    #[test]
    fn outputs_follow_the_engine_across_the_renumbering() {
        let ini = "[Backglass]\nBackglassOutput = 2\n\n[ScoreView]\nScoreViewOutput = 2\nScoreViewWidth = 848\n";
        let old = swap_outputs(ini);
        assert_eq!(ini::get(&old, "Backglass", "BackglassOutput").as_deref(), Some("1"));
        assert_eq!(ini::get(&old, "ScoreView", "ScoreViewOutput").as_deref(), Some("1"));
        assert_eq!(ini::get(&old, "ScoreView", "ScoreViewWidth").as_deref(), Some("848"));
        assert_eq!(swap_outputs(&old), ini);
        let off = "[ScoreView]\nScoreViewOutput = 0\n";
        assert_eq!(swap_outputs(off), off);
    }
}
