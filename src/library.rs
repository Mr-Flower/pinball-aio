//! Mette in ordine la cartella `tables/`: l'utente può buttarci i file così come sono,
//! il launcher crea una cartella per tavolo come richiedono VPinballX e VPinFE.

use crate::paths::Paths;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

fn has_ext(dir: &Path, ext: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|files| {
        files.flatten().any(|f| f.path().extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
    })
}

/// Cartelle dentro `tables/` che contengono un `.vpx`.
fn table_folders(paths: &Paths) -> Vec<PathBuf> {
    let Ok(folders) = fs::read_dir(paths.tables()) else { return Vec::new() };
    folders.flatten().map(|f| f.path()).filter(|p| has_ext(p, "vpx")).collect()
}

/// Quante cartelle di tavoli ci sono.
pub fn count(paths: &Paths) -> usize {
    table_folders(paths).len()
}

/// Nomi delle cartelle dei tavoli ancora senza il file `.info` che VPinFE richiede per
/// mostrarle, in ordine alfabetico.
pub fn without_info(paths: &Paths) -> Vec<String> {
    let mut names: Vec<String> = table_folders(paths)
        .iter()
        .filter(|p| !has_ext(p, "info"))
        .filter_map(|p| Some(p.file_name()?.to_str()?.to_string()))
        .collect();
    names.sort();
    names
}

/// Tavoli rimasti senza scheda dopo l'ultimo passaggio di VPinFE (vedi `set_tried`).
pub fn tried(paths: &Paths) -> Vec<String> {
    fs::read_to_string(paths.tried_file())
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn set_tried(paths: &Paths, names: &[String]) -> io::Result<()> {
    if names.is_empty() {
        return match fs::remove_file(paths.tried_file()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    fs::write(paths.tried_file(), names.join("\n") + "\n")
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
        assert_eq!(count(&paths), 1);

        assert_eq!(organize(&paths).unwrap(), 1);
        let folder = tables.join("Hurricane (Williams 1991)");
        assert!(folder.join("Hurricane (Williams 1991) v2.vpx").is_file());
        assert!(folder.join("Hurricane (Williams 1991) v2.directb2s").is_file());
        assert!(tables.join("orphan.directb2s").is_file());
        assert_eq!(count(&paths), 2);
        assert_eq!(organize(&paths).unwrap(), 0);

        // Senza scheda ci sono tutti e due; una volta annotati, resta da fare solo chi arriva dopo.
        assert_eq!(without_info(&paths), ["Already (Stern 2000)", "Hurricane (Williams 1991)"]);
        assert!(tried(&paths).is_empty());
        fs::write(folder.join("Hurricane (Williams 1991).info"), b"{}").unwrap();
        fs::create_dir_all(paths.config()).unwrap();
        set_tried(&paths, &without_info(&paths)).unwrap();
        assert_eq!(tried(&paths), ["Already (Stern 2000)"]);
        set_tried(&paths, &[]).unwrap();
        assert!(tried(&paths).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
