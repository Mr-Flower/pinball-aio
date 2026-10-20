//! I tavoli della libreria: una cartella per tavolo con il `.vpx` e, se il tavolo è stato
//! riconosciuto, la scheda `<cartella>.info`. La scheda è un JSON nello stesso formato di
//! VPinFE (sezioni Info, User, VPXFile, VPinFE, Medias), così quelle già create restano
//! valide; le sezioni e le voci che non usiamo si conservano come sono.

use crate::files::write_atomic;
use crate::paths::Paths;
use crate::vps::{self, Game};
use crate::vpx::VpxInfo;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub dir: PathBuf,
    /// Nome della cartella, unico nella libreria.
    pub folder: String,
    pub vpx: PathBuf,
    pub title: String,
    pub manufacturer: String,
    pub year: String,
    pub authors: Vec<String>,
    pub themes: Vec<String>,
    pub rom: String,
    pub vps_id: String,
    pub ipdb_id: String,
    pub tutorial: Option<String>,
    pub favorite: bool,
    pub last_run: Option<i64>,
    pub start_count: i64,
    /// Minuti di gioco.
    pub run_time: i64,
    /// C'è la scheda, cioè il tavolo è stato riconosciuto.
    pub has_info: bool,
    /// Lo script usa PinMAME (vedi `vpx::uses_pinmame`); `None` se non è ancora noto.
    pub uses_pinmame: Option<bool>,
    /// Immagine della tile: il backglass, altrimenti la prima che c'è tra le altre.
    pub cover: Option<PathBuf>,
}

impl Table {
    pub fn media(&self, file: &str) -> PathBuf {
        self.dir.join("medias").join(file)
    }

    /// "Bally · 1995", oppure solo ciò che si conosce.
    pub fn subtitle(&self) -> String {
        [self.manufacturer.as_str(), self.year.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Nomi dei file `.zip` di una cartella, in minuscolo.
fn zip_names(dir: &Path) -> HashSet<String> {
    let Ok(entries) = fs::read_dir(dir) else { return HashSet::new() };
    entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .filter(|name| name.ends_with(".zip"))
        .collect()
}

/// ROM della cartella condivisa, lette una volta per tutta la libreria.
pub struct RomIndex {
    shared: HashSet<String>,
}

impl RomIndex {
    pub fn read(paths: &Paths) -> Self {
        Self { shared: zip_names(&paths.pinmame().join("roms")) }
    }

    /// Nome del file della ROM, se il tavolo ne usa una e non si trova né nella cartella
    /// condivisa né in quella `pinmame` del tavolo.
    pub fn missing(&self, table: &Table) -> Option<String> {
        if table.rom.is_empty() || table.uses_pinmame != Some(true) {
            return None;
        }
        let file = format!("{}.zip", table.rom);
        let key = file.to_lowercase();
        let found = self.shared.contains(&key) || zip_names(&table.dir.join("pinmame").join("roms")).contains(&key);
        (!found).then_some(file)
    }
}

pub fn info_path(dir: &Path) -> PathBuf {
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dir.join(format!("{folder}.info"))
}

fn first_vpx(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vpx")))
        .collect();
    found.sort();
    found.into_iter().next()
}

fn read_json(path: &Path) -> Option<Map<String, Value>> {
    let bytes = fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    }
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn number(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => 0,
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(|v| text(Some(v))).filter(|s| !s.is_empty()).collect(),
        _ => Vec::new(),
    }
}

pub fn load(dir: &Path) -> Option<Table> {
    let vpx = first_vpx(dir)?;
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let parsed = vps::parse_folder(&folder);
    let mut table = Table {
        dir: dir.to_path_buf(),
        title: parsed.name,
        manufacturer: parsed.manufacturer.unwrap_or_default(),
        year: parsed.year.map(|y| y.to_string()).unwrap_or_default(),
        folder,
        vpx,
        ..Table::default()
    };
    table.cover = ["bg.png", "flyer.png", "cab.png", "table.png", "wheel.png"]
        .iter()
        .map(|file| table.media(file))
        .find(|path| path.is_file());
    let Some(data) = read_json(&info_path(dir)) else { return Some(table) };
    let empty = Map::new();
    let section = |name: &str| data.get(name).and_then(Value::as_object).unwrap_or(&empty);
    let (info, user, file) = (section("Info"), section("User"), section("VPXFile"));
    table.has_info = true;
    let title = text(info.get("Title"));
    if !title.is_empty() {
        table.title = title;
        table.manufacturer = text(info.get("Manufacturer"));
        table.year = text(info.get("Year"));
    }
    // Un nome scelto a mano in VPinFE vale più di quello del database.
    let alt = data.get("VPinFE").and_then(|v| v.get("alttitle"));
    if !text(alt).is_empty() {
        table.title = text(alt);
    }
    table.authors = strings(info.get("Authors"));
    table.themes = strings(info.get("Themes"));
    table.rom = text(info.get("Rom"));
    if table.rom.is_empty() {
        table.rom = text(file.get("rom"));
    }
    table.uses_pinmame = file.get("usesPinMAME").and_then(Value::as_bool);
    table.vps_id = text(info.get("VPSId"));
    table.ipdb_id = text(info.get("IPDBId"));
    table.tutorial = Some(text(info.get("PinballPrimerTut"))).filter(|t| !t.is_empty());
    table.favorite = number(user.get("Favorite")) != 0;
    table.last_run = Some(number(user.get("LastRun"))).filter(|&t| t > 0);
    table.start_count = number(user.get("StartCount"));
    table.run_time = number(user.get("RunTime"));
    Some(table)
}

/// Tutti i tavoli in `tables/`, cioè le cartelle che contengono un `.vpx`.
pub fn load_all(paths: &Paths) -> Vec<Table> {
    let Ok(entries) = fs::read_dir(paths.tables()) else { return Vec::new() };
    let mut tables: Vec<Table> = entries.flatten().filter(|e| e.path().is_dir()).filter_map(|e| load(&e.path())).collect();
    tables.sort_by_key(|t| t.title.to_lowercase());
    tables
}

fn write(path: &Path, data: &Map<String, Value>) -> io::Result<()> {
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    serde::Serialize::serialize(data, &mut serializer).map_err(io::Error::other)?;
    write_atomic(path, &bytes)
}

/// Secondi dal 1970, il formato di `LastRun`.
pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Crea o rifà la scheda di un tavolo riconosciuto. Le statistiche, le copertine già
/// registrate e le sezioni di altri programmi restano.
pub fn write_info(dir: &Path, game: &Game, vpx_file: &Path, vpx: &VpxInfo) -> io::Result<()> {
    let path = info_path(dir);
    let mut data = read_json(&path).unwrap_or_default();
    let authors: Vec<String> = vpx.author.split(',').map(str::trim).filter(|a| !a.is_empty()).map(str::to_string).collect();
    let mut info = json!({
        "IPDBId": game.ipdb_id(),
        "Title": game.name,
        "Manufacturer": game.manufacturer,
        "Year": game.year,
        "Type": game.kind.clone().unwrap_or_default(),
        "Themes": game.theme,
        "VPSId": game.id,
        "Authors": authors,
        "Rom": vpx.rom,
    });
    if let Some(tutorial) = game.pinball_primer() {
        info["PinballPrimerTut"] = json!(tutorial);
    }
    let previous = data.get("VPXFile").cloned().unwrap_or(json!({}));
    let mut vpx_file_section = json!({
        "filename": vpx_file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        "filehash": vpx.file_hash,
        "version": vpx.version,
        "releaseDate": vpx.release_date,
        "saveDate": vpx.save_date,
        "saveRev": vpx.save_rev,
        "manufacturer": vpx.company_name,
        "year": vpx.company_year,
        "type": vpx.table_type,
        "vbsHash": vpx.script_hash,
        "rom": vpx.rom,
        "usesPinMAME": crate::vpx::uses_pinmame(&vpx.script),
    });
    let script = vpx.script.to_lowercase();
    for (key, needle) in [
        ("detectnfozzy", "class flipperpolarity"),
        ("detectfleep", "rubberstrongsoundfactor"),
        ("detectssf", "playsoundat"),
        ("detectlut", "lut"),
        ("detectscorebit", "scorebit"),
        ("detectfastflips", "fastflips"),
        ("detectflex", "flexdmd"),
    ] {
        vpx_file_section[key] = json!(script.contains(needle));
    }
    if previous.get("filehash") == vpx_file_section.get("filehash") {
        if let Some(applied) = previous.get("patch_applied") {
            vpx_file_section["patch_applied"] = applied.clone();
        }
    }
    data.insert("Info".into(), info);
    let user = data.entry("User").or_insert_with(|| json!({}));
    if let Some(user) = user.as_object_mut() {
        for (key, value) in [
            ("Rating", json!(0)),
            ("Favorite", json!(0)),
            ("LastRun", Value::Null),
            ("StartCount", json!(0)),
            ("RunTime", json!(0)),
            ("Tags", json!([])),
            ("FrontendDOFEvent", json!("")),
        ] {
            user.entry(key).or_insert(value);
        }
    }
    data.insert("VPXFile".into(), vpx_file_section);
    let vpinfe = data.entry("VPinFE").or_insert_with(|| json!({}));
    if let Some(vpinfe) = vpinfe.as_object_mut() {
        for key in ["altlauncher", "pluginprofile", "alttitle", "altvpsid"] {
            vpinfe.entry(key).or_insert(json!(""));
        }
        vpinfe.entry("deletedNVRamOnClose").or_insert(json!(false));
    }
    data.entry("Medias").or_insert_with(|| json!({}));
    write(&path, &data)
}

/// Modifica una sezione della scheda, se la scheda esiste.
fn update_section(dir: &Path, name: &str, change: impl FnOnce(&mut Map<String, Value>)) -> io::Result<()> {
    let path = info_path(dir);
    let Some(mut data) = read_json(&path) else { return Ok(()) };
    let section = data.entry(name).or_insert_with(|| json!({}));
    if !section.is_object() {
        *section = json!({});
    }
    if let Some(map) = section.as_object_mut() {
        change(map);
    }
    write(&path, &data)
}

pub fn set_favorite(table: &Table, favorite: bool) -> io::Result<()> {
    update_section(&table.dir, "User", |user| {
        user.insert("Favorite".into(), json!(i64::from(favorite)));
    })
}

/// Una partita che inizia: come VPinFE, conta l'avvio e ne segna l'ora.
pub fn record_start(table: &Table) -> io::Result<()> {
    update_section(&table.dir, "User", |user| {
        let count = number(user.get("StartCount")) + 1;
        user.insert("StartCount".into(), json!(count));
        user.insert("LastRun".into(), json!(unix_now()));
    })
}

/// Una partita finita: aggiunge i minuti giocati, arrotondati per eccesso.
pub fn record_time(table: &Table, seconds: u64) -> io::Result<()> {
    update_section(&table.dir, "User", |user| {
        let total = number(user.get("RunTime")) + seconds.div_ceil(60) as i64;
        user.insert("RunTime".into(), json!(total));
    })
}

/// Copertina scaricata, annotata come fa VPinFE: con l'md5 si capisce se è cambiata.
pub fn record_media(dir: &Path, key: &str, file: &str, md5: &str) -> io::Result<()> {
    update_section(dir, "Medias", |medias| {
        medias.insert(key.into(), json!({ "Source": "vpinmediadb", "Path": file, "MD5Hash": md5 }));
    })
}

/// Toglie dalla scheda le copertine scaricate, restituendo i loro file.
pub fn forget_downloaded_media(dir: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    update_section(dir, "Medias", |medias| {
        medias.retain(|_, entry| {
            let downloaded = text(entry.get("Source")) == "vpinmediadb";
            if downloaded {
                files.push(text(entry.get("Path")));
            }
            !downloaded
        });
    })?;
    Ok(files)
}

/// Copertine che l'utente ha messo da sé (o registrate da altri): non vanno sostituite.
pub fn media_source(dir: &Path, key: &str) -> Option<(String, String)> {
    let data = read_json(&info_path(dir))?;
    let entry = data.get("Medias")?.get(key)?;
    Some((text(entry.get("Source")), text(entry.get("MD5Hash"))))
}

pub fn set_uses_pinmame(dir: &Path, uses: bool) -> io::Result<()> {
    update_section(dir, "VPXFile", |file| {
        file.insert("usesPinMAME".into(), json!(uses));
    })
}

pub fn set_patch_applied(dir: &Path) -> io::Result<()> {
    update_section(dir, "VPXFile", |file| {
        file.insert("patch_applied".into(), json!("true"));
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pinball-aio-catalog-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tables_without_a_card_take_the_name_from_the_folder() {
        let root = temp_dir("plain");
        let paths = Paths { root: root.clone() };
        let dir = paths.tables().join("Fish Tales (Williams 1992)");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Fish Tales (Williams 1992) v1.vpx"), b"x").unwrap();
        fs::create_dir_all(paths.tables().join("empty folder")).unwrap();
        let tables = load_all(&paths);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!((t.title.as_str(), t.subtitle().as_str(), t.has_info), ("Fish Tales", "Williams · 1992", false));
        // Senza scheda le statistiche non si salvano e non è un errore.
        record_start(t).unwrap();
        assert!(!info_path(&t.dir).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_card_keeps_stats_and_unknown_sections_when_rewritten() {
        let root = temp_dir("card");
        let dir = root.join("Indiana Jones (Stern 2008)");
        fs::create_dir_all(&dir).unwrap();
        let vpx_path = dir.join("Indiana Jones (Stern 2008).vpx");
        fs::write(&vpx_path, b"x").unwrap();
        fs::write(
            info_path(&dir),
            r#"{"Info":{"Title":"JP's Indiana Jones"},"User":{"StartCount":4,"Favorite":1},"Other":{"k":1},"VPinFE":{"alttitle":""}}"#,
        )
        .unwrap();
        let game = Game {
            id: "NqaGRv8k".into(),
            name: "Indiana Jones".into(),
            manufacturer: "Stern".into(),
            year: Some(2008),
            ipdb_url: Some("https://www.ipdb.org/machine.cgi?id=5306".into()),
            ..Game::default()
        };
        let vpx = VpxInfo { author: "Bigus1, Hanibal".into(), rom: "ij4_210".into(), script: "LoadVPM\r\nSub LUT".into(), ..VpxInfo::default() };
        write_info(&dir, &game, &vpx_path, &vpx).unwrap();
        let t = load(&dir).unwrap();
        assert_eq!((t.title.as_str(), t.vps_id.as_str(), t.ipdb_id.as_str()), ("Indiana Jones", "NqaGRv8k", "5306"));
        assert_eq!((t.start_count, t.favorite, t.rom.as_str()), (4, true, "ij4_210"));
        assert_eq!(t.authors, ["Bigus1", "Hanibal"]);
        assert_eq!(t.uses_pinmame, Some(true));
        let data = read_json(&info_path(&dir)).unwrap();
        assert_eq!(data["Other"]["k"], 1);
        assert_eq!(data["VPXFile"]["detectlut"], true);
        assert_eq!(data["Info"]["Year"], 2008);

        record_start(&t).unwrap();
        record_time(&t, 61).unwrap();
        set_favorite(&t, false).unwrap();
        record_media(&dir, "bg", "bg.png", "abc").unwrap();
        let t = load(&dir).unwrap();
        assert_eq!((t.start_count, t.run_time, t.favorite), (5, 2, false));
        assert!(t.last_run.is_some());
        assert_eq!(media_source(&dir, "bg"), Some(("vpinmediadb".into(), "abc".into())));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_rom_is_looked_up_where_the_engine_looks() {
        let root = temp_dir("rom");
        let paths = Paths { root: root.clone() };
        let dir = paths.tables().join("T (Bally 1990)");
        fs::create_dir_all(dir.join("pinmame").join("roms")).unwrap();
        fs::create_dir_all(paths.pinmame().join("roms")).unwrap();
        let mut t = Table { dir: dir.clone(), rom: "afm_113b".into(), ..Table::default() };
        assert_eq!(RomIndex::read(&paths).missing(&t), None);
        t.uses_pinmame = Some(true);
        assert_eq!(RomIndex::read(&paths).missing(&t).as_deref(), Some("afm_113b.zip"));
        fs::write(dir.join("pinmame").join("roms").join("afm_113b.zip"), b"x").unwrap();
        assert_eq!(RomIndex::read(&paths).missing(&t), None);
        fs::remove_file(dir.join("pinmame").join("roms").join("afm_113b.zip")).unwrap();
        fs::write(paths.pinmame().join("roms").join("AFM_113B.zip"), b"x").unwrap();
        assert_eq!(RomIndex::read(&paths).missing(&t), None);
        t.rom.clear();
        assert_eq!(RomIndex::read(&paths).missing(&t), None);
        fs::remove_dir_all(root).unwrap();
    }
}
