//! Mette in ordine la cartella `tables/`: l'utente può buttarci i file così come sono,
//! il launcher crea una cartella per tavolo come richiedono VPinballX e VPinFE.

use crate::paths::Paths;
use std::fs;
use std::io;
use std::path::Path;

fn is_vpx(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx"))
}

/// Nome della cartella a partire dal nome del file: si tiene fino a "(Marca Anno)",
/// togliendo autore e versione che di solito seguono.
/// "Attack from Mars (Bally 1995)_Bigus(MOD)4.1" -> "Attack from Mars (Bally 1995)".
fn folder_name(stem: &str) -> &str {
    let mut from = 0;
    while let Some(open) = stem[from..].find('(').map(|i| i + from) {
        let Some(close) = stem[open..].find(')').map(|i| i + open) else { break };
        let inside = stem[open + 1..close].trim_end();
        let year = inside.len() >= 4 && inside.bytes().rev().take(4).all(|b| b.is_ascii_digit());
        if year && open > 0 {
            return stem[..=close].trim();
        }
        from = close + 1;
    }
    stem.trim()
}

/// Sposta ogni `.vpx` sciolto in `tables/` nella sua cartella, insieme ai file con lo
/// stesso nome (backglass, .ini, .vbs…). Restituisce quanti tavoli ha spostato.
pub fn organize(paths: &Paths) -> io::Result<usize> {
    let tables = paths.tables();
    let loose: Vec<_> = fs::read_dir(&tables)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.is_dir())
        .collect();
    let mut moved = 0;
    for vpx in loose.iter().filter(|p| is_vpx(p)) {
        let Some(stem) = vpx.file_stem().and_then(|s| s.to_str()) else { continue };
        let folder = tables.join(folder_name(stem));
        fs::create_dir_all(&folder)?;
        for file in loose.iter().filter(|p| p.file_stem().and_then(|s| s.to_str()) == Some(stem)) {
            let Some(name) = file.file_name() else { continue };
            let target = folder.join(name);
            // Mai sovrascrivere: un file già presente nella cartella resta com'è.
            if !target.exists() {
                fs::rename(file, target)?;
                moved += usize::from(is_vpx(file));
            }
        }
    }
    Ok(moved)
}

/// Cartelle dei tavoli: quante sono e quante hanno già il file `.info` che VPinFE
/// richiede per mostrarle.
pub fn count(paths: &Paths) -> (usize, usize) {
    let has_ext = |dir: &Path, ext: &str| {
        fs::read_dir(dir).is_ok_and(|files| {
            files.flatten().any(|f| f.path().extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
        })
    };
    let Ok(folders) = fs::read_dir(paths.tables()) else { return (0, 0) };
    let tables: Vec<_> = folders.flatten().map(|f| f.path()).filter(|p| has_ext(p, "vpx")).collect();
    let with_info = tables.iter().filter(|p| has_ext(p, "info")).count();
    (tables.len(), with_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_name_stops_at_manufacturer_and_year() {
        assert_eq!(folder_name("Attack from Mars (Bally 1995)_Bigus(MOD)4.1"), "Attack from Mars (Bally 1995)");
        assert_eq!(folder_name("Simpsons Pinball Party (Stern 2003)4.0"), "Simpsons Pinball Party (Stern 2003)");
        assert_eq!(folder_name("RollerCoaster Tycoon (Stern 2002) JPSalas 6.0.0"), "RollerCoaster Tycoon (Stern 2002)");
        assert_eq!(folder_name("Space Cadet (Windows) nFozzy LW"), "Space Cadet (Windows) nFozzy LW");
        assert_eq!(folder_name("Hurricane"), "Hurricane");
        assert_eq!(folder_name("Broken (Name 1990"), "Broken (Name 1990");
    }

    #[test]
    fn organize_moves_tables_with_their_companion_files() {
        let root = std::env::temp_dir().join(format!("pinball-aio-test-{}", std::process::id()));
        let paths = Paths { root: root.clone() };
        let tables = paths.tables();
        fs::create_dir_all(tables.join("Already (Stern 2000)")).unwrap();
        for name in ["Hurricane (Williams 1991) v2.vpx", "Hurricane (Williams 1991) v2.directb2s", "orphan.directb2s"] {
            fs::write(tables.join(name), b"x").unwrap();
        }
        fs::write(tables.join("Already (Stern 2000)").join("Already.vpx"), b"x").unwrap();
        assert_eq!(count(&paths), (1, 0));

        assert_eq!(organize(&paths).unwrap(), 1);
        let folder = tables.join("Hurricane (Williams 1991)");
        assert!(folder.join("Hurricane (Williams 1991) v2.vpx").is_file());
        assert!(folder.join("Hurricane (Williams 1991) v2.directb2s").is_file());
        assert!(tables.join("orphan.directb2s").is_file());
        assert_eq!(count(&paths), (2, 0));
        assert_eq!(organize(&paths).unwrap(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
