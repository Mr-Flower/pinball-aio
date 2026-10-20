//! Mette in ordine la cartella `tables/`: l'utente può buttarci i file così come sono,
//! il launcher crea una cartella per tavolo come richiede VPinballX.

use crate::files;
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

/// Estensioni dei file presenti in una cartella, in minuscolo.
fn extensions(dir: &Path) -> Vec<String> {
    let Ok(files) = fs::read_dir(dir) else { return Vec::new() };
    files.flatten().filter_map(|f| Some(f.path().extension()?.to_string_lossy().to_lowercase())).collect()
}

fn has_ext(dir: &Path, ext: &str) -> bool {
    extensions(dir).iter().any(|e| e == ext)
}

/// Nomi delle cartelle dei tavoli (quelle con un `.vpx`) ancora senza scheda (`.info`),
/// in ordine alfabetico.
pub fn without_info(paths: &Paths) -> Vec<String> {
    let Ok(folders) = fs::read_dir(paths.tables()) else { return Vec::new() };
    let mut names: Vec<String> = folders
        .flatten()
        .filter(|f| {
            let found = extensions(&f.path());
            found.iter().any(|e| e == "vpx") && !found.iter().any(|e| e == "info")
        })
        .filter_map(|f| f.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

/// Tavoli rimasti senza scheda dopo l'ultima preparazione (vedi `set_tried`).
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

/// Cosa è successo ai file aggiunti con `import`.
#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub tables: usize,
    pub roms: usize,
    /// Nomi dei file non copiati, con il motivo.
    pub skipped: Vec<String>,
}

/// File che vanno accanto al tavolo: il tavolo, il backglass, lo script e le impostazioni.
pub const TABLE_FILES: [&str; 4] = ["vpx", "directb2s", "vbs", "ini"];

/// Copia nella cartella dati i file scelti dall'utente: tavoli e loro file in `tables/`
/// (poi sistemati in cartelle da `organize`), ROM `.zip` in `pinmame/roms`, cartelle che
/// contengono un `.vpx` così come sono. Gli originali restano dove sono e un file già
/// presente non si sovrascrive mai.
pub fn import(paths: &Paths, files: &[PathBuf]) -> io::Result<Imported> {
    let mut result = Imported::default();
    fs::create_dir_all(paths.tables())?;
    fs::create_dir_all(paths.pinmame().join("roms"))?;
    for file in files {
        let Some(name) = file.file_name() else { continue };
        let shown = name.to_string_lossy().into_owned();
        let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let (target, is_table, is_rom) = if file.is_dir() {
            if !has_ext(file, "vpx") {
                result.skipped.push(format!("{shown}: la cartella non contiene un tavolo (.vpx)"));
                continue;
            }
            (paths.tables().join(name), true, false)
        } else if ext == "zip" {
            (paths.pinmame().join("roms").join(name), false, true)
        } else if TABLE_FILES.contains(&ext.as_str()) {
            (paths.tables().join(name), ext == "vpx", false)
        } else {
            result.skipped.push(format!("{shown}: tipo di file non riconosciuto"));
            continue;
        };
        // Un file di tavolo già sistemato nella sua cartella da un'importazione precedente.
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let in_folder = !file.is_dir() && !is_rom && paths.tables().join(folder_name(&stem)).join(name).exists();
        if target.exists() || in_folder {
            result.skipped.push(format!("{shown}: c'è già"));
            continue;
        }
        if file.is_dir() {
            files::copy_dir(file, &target)?;
        } else {
            files::copy_atomic(file, &target)?;
        }
        result.tables += usize::from(is_table);
        result.roms += usize::from(is_rom);
    }
    organize(paths)?;
    Ok(result)
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

    fn table_count(paths: &Paths) -> usize {
        fs::read_dir(paths.tables()).unwrap().flatten().filter(|f| has_ext(&f.path(), "vpx")).count()
    }

    #[test]
    fn import_copies_tables_roms_and_table_folders() {
        let root = std::env::temp_dir().join(format!("pinball-aio-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let folder = source.join("Medieval Madness (Williams 1997)");
        fs::create_dir_all(folder.join("medias")).unwrap();
        fs::write(folder.join("mm.vpx"), b"x").unwrap();
        fs::write(folder.join("medias").join("bg.png"), b"x").unwrap();
        fs::create_dir_all(source.join("not a table")).unwrap();
        for name in ["Fish Tales (Williams 1992) v2.vpx", "Fish Tales (Williams 1992) v2.directb2s", "ft_l5.zip", "notes.pdf"] {
            fs::write(source.join(name), b"x").unwrap();
        }
        let paths = Paths { root: root.join("data") };
        let files: Vec<PathBuf> = ["Medieval Madness (Williams 1997)", "not a table", "Fish Tales (Williams 1992) v2.vpx",
            "Fish Tales (Williams 1992) v2.directb2s", "ft_l5.zip", "notes.pdf"]
            .iter()
            .map(|n| source.join(n))
            .collect();
        let imported = import(&paths, &files).unwrap();
        assert_eq!((imported.tables, imported.roms, imported.skipped.len()), (2, 1, 2));
        assert!(paths.tables().join("Medieval Madness (Williams 1997)").join("medias").join("bg.png").is_file());
        let fish = paths.tables().join("Fish Tales (Williams 1992)");
        assert!(fish.join("Fish Tales (Williams 1992) v2.vpx").is_file());
        assert!(fish.join("Fish Tales (Williams 1992) v2.directb2s").is_file());
        assert!(paths.pinmame().join("roms").join("ft_l5.zip").is_file());
        assert!(source.join("ft_l5.zip").is_file());

        // La seconda volta non si copia né si sovrascrive niente.
        let again = import(&paths, &files).unwrap();
        assert_eq!((again.tables, again.roms, again.skipped.len()), (0, 0, 6));
        fs::remove_dir_all(root).unwrap();
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
        assert_eq!(table_count(&paths), 1);

        assert_eq!(organize(&paths).unwrap(), 1);
        let folder = tables.join("Hurricane (Williams 1991)");
        assert!(folder.join("Hurricane (Williams 1991) v2.vpx").is_file());
        assert!(folder.join("Hurricane (Williams 1991) v2.directb2s").is_file());
        assert!(tables.join("orphan.directb2s").is_file());
        assert_eq!(table_count(&paths), 2);
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
