//! Database della Virtual Pinball Spreadsheet: l'elenco dei tavoli esistenti con nome,
//! marca, anno e identificativo, usato per riconoscere i tavoli dal nome della cartella.
//! Si scarica da GitHub e si tiene in cache; lo si riscarica solo quando cambia.

use crate::{files, net};
use crate::paths::Paths;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

const DB_URL: &str = "https://github.com/VirtualPinballSpreadsheet/vps-db/raw/refs/heads/main/db/vpsdb.json";
const VERSION_URL: &str =
    "https://raw.githubusercontent.com/VirtualPinballSpreadsheet/vps-db/refs/heads/main/lastUpdated.json";

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub year: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub theme: Vec<String>,
    pub ipdb_url: Option<String>,
    pub tutorial_files: Vec<serde_json::Value>,
}

impl Game {
    pub fn year_text(&self) -> String {
        self.year.map(|y| y.to_string()).unwrap_or_default()
    }

    /// Identificativo IPDB, ricavato dall'indirizzo `…?id=NNNN`.
    pub fn ipdb_id(&self) -> String {
        let url = self.ipdb_url.as_deref().unwrap_or_default();
        url.split(['?', '&'])
            .find_map(|part| part.strip_prefix("id="))
            .unwrap_or_default()
            .to_string()
    }

    /// Link al tutorial di Pinball Primer, se il database ne indica uno.
    pub fn pinball_primer(&self) -> Option<String> {
        const PREFIX: &str = "https://pinballprimer.github.io/";
        let url_of = |v: &serde_json::Value| v.get("url")?.as_str().map(str::to_string);
        self.tutorial_files.iter().find_map(|t| {
            let direct = url_of(t).filter(|u| u.starts_with(PREFIX));
            let listed = || {
                t.get("urls")?.as_array()?.iter().filter_map(url_of).find(|u| u.starts_with(PREFIX))
            };
            direct.or_else(listed)
        })
    }
}

pub struct Database {
    games: Vec<Game>,
    /// Per ogni voce: nome normalizzato e testo per la ricerca libera, calcolati una volta.
    names: Vec<String>,
    haystacks: Vec<String>,
}

impl Database {
    pub fn new(games: Vec<Game>) -> Self {
        let names: Vec<String> = games.iter().map(|g| normalize(&g.name)).collect();
        let haystacks = games
            .iter()
            .zip(&names)
            .map(|(g, name)| format!("{name} {} {}", normalize(&g.manufacturer), g.year_text()))
            .collect();
        Self { games, names, haystacks }
    }

    /// Carica il database, scaricandolo se in cache manca o è più vecchio di quello pubblicato.
    /// Senza rete si usa la copia in cache, se c'è.
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.cache().join("vpsdb.json");
        let version_file = paths.cache().join("vpsdb.version");
        let local_version = fs::read_to_string(&version_file).unwrap_or_default();
        match net::get_text(VERSION_URL) {
            Ok(remote) if remote.trim() != local_version.trim() || !file.exists() => {
                let bytes = net::get_bytes(DB_URL).context("download del database VPS non riuscito");
                match bytes {
                    Ok(bytes) => {
                        fs::create_dir_all(paths.cache())?;
                        files::write_atomic(&file, &bytes)?;
                        fs::write(&version_file, remote.trim())?;
                    }
                    Err(e) if !file.exists() => return Err(e),
                    Err(e) => eprintln!("{e:#}: uso la copia in cache"),
                }
            }
            Ok(_) => {}
            Err(e) if !file.exists() => return Err(e.context("database VPS non raggiungibile")),
            Err(e) => eprintln!("database VPS non controllato ({e:#}): uso la copia in cache"),
        }
        let games: Vec<Game> =
            serde_json::from_slice(&fs::read(&file)?).context("database VPS non leggibile")?;
        Ok(Self::new(games))
    }

    /// Il tavolo che corrisponde meglio al nome della cartella, "Titolo (Marca Anno)".
    pub fn find(&self, folder: &str) -> Option<&Game> {
        let wanted = parse_folder(folder);
        let name = normalize(&wanted.name);
        let mut best: Option<(f64, &Game)> = None;
        for (game, game_name) in self.games.iter().zip(&self.names) {
            if wanted.year.is_some() && game.year != wanted.year {
                continue;
            }
            let name_score = ratio(&name, game_name);
            if name_score < 0.8 {
                continue;
            }
            let maker_score = match &wanted.manufacturer {
                Some(maker) => maker_similarity(maker, &game.manufacturer),
                None => 1.0,
            };
            if maker_score < 0.8 {
                continue;
            }
            // A parità conta il nome scritto esattamente com'è.
            let score = name_score * 2.0 + maker_score + ratio(&wanted.name, &game.name) * 0.01;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, game));
            }
        }
        best.map(|(_, game)| game)
    }

    /// Ricerca libera per l'abbinamento a mano: tutte le parole devono comparire nel nome,
    /// nella marca o nell'anno. I nomi che iniziano con la ricerca vengono prima.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Game> {
        let query = normalize(query);
        let words: Vec<&str> = query.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(bool, &Game)> = (0..self.games.len())
            .filter(|&i| words.iter().all(|w| self.haystacks[i].contains(w)))
            .map(|i| (!self.names[i].starts_with(&query), &self.games[i]))
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
        found.into_iter().take(limit).map(|(_, g)| g).collect()
    }
}

pub struct FolderName {
    pub name: String,
    pub manufacturer: Option<String>,
    pub year: Option<i64>,
}

/// "Attack from Mars (Bally 1995) (v2)" -> nome, marca e anno. Senza parentesi finali con
/// l'anno tutto il testo è il nome.
pub fn parse_folder(folder: &str) -> FolderName {
    let plain = FolderName { name: folder.trim().to_string(), manufacturer: None, year: None };
    let Some(open) = folder.find(" (") else { return plain };
    let Some(close) = folder[open..].find(')').map(|i| i + open) else { return plain };
    let inside = &folder[open + 2..close];
    let Some((maker, year)) = inside.rsplit_once(' ') else { return plain };
    let Ok(year) = year.parse::<i64>() else { return plain };
    if year < 1000 || maker.trim().is_empty() || maker.contains('(') {
        return plain;
    }
    FolderName {
        name: folder[..open].trim().to_string(),
        manufacturer: Some(maker.trim().to_string()),
        year: Some(year),
    }
}

/// Minuscole, senza punteggiatura e senza "the" iniziale, così "The Simpsons Pinball
/// Party" e "Simpsons Pinball Party" o "Guns N' Roses" e "Guns N Roses" coincidono.
fn normalize(text: &str) -> String {
    let lower: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else if c == '\'' { '\0' } else { ' ' })
        .filter(|&c| c != '\0')
        .collect();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let skip = usize::from(words.len() > 1 && words[0] == "the");
    words[skip..].join(" ")
}

fn maker_similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (normalize(a), normalize(b));
    if !a.is_empty() && !b.is_empty() && (a.starts_with(&b) || b.starts_with(&a)) {
        return 1.0;
    }
    ratio(&a, &b)
}

/// Somiglianza tra due testi come `difflib.SequenceMatcher.ratio` (Ratcliff/Obershelp):
/// 2 × caratteri in comune / caratteri totali.
pub fn ratio(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    2.0 * matching(&a, &b) as f64 / (a.len() + b.len()) as f64
}

fn matching(a: &[char], b: &[char]) -> usize {
    let (mut best, mut at_a, mut at_b) = (0, 0, 0);
    let mut lengths = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        for j in (0..b.len()).rev() {
            lengths[j + 1] = if *ca == b[j] { lengths[j] + 1 } else { 0 };
            if lengths[j + 1] > best {
                (best, at_a, at_b) = (lengths[j + 1], i + 1 - lengths[j + 1], j + 1 - lengths[j + 1]);
            }
        }
    }
    if best == 0 {
        return 0;
    }
    best + matching(&a[..at_a], &b[..at_b]) + matching(&a[at_a + best..], &b[at_b + best..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: &str, name: &str, maker: &str, year: i64) -> Game {
        Game { id: id.into(), name: name.into(), manufacturer: maker.into(), year: Some(year), ..Game::default() }
    }

    #[test]
    fn ratio_matches_difflib() {
        assert_eq!(ratio("abcd", "bcde"), 0.75);
        assert_eq!(ratio("stern", "stern"), 1.0);
        assert_eq!(ratio("", "abc"), 0.0);
        assert!((ratio("guns n roses", "guns n' roses") - 24.0 / 25.0).abs() < 1e-9);
    }

    #[test]
    fn folder_names_are_split_into_name_maker_and_year() {
        let f = parse_folder("Attack from Mars (Bally 1995) (v2)");
        assert_eq!((f.name.as_str(), f.manufacturer.as_deref(), f.year), ("Attack from Mars", Some("Bally"), Some(1995)));
        let f = parse_folder("Tales (Data East 1993)");
        assert_eq!((f.manufacturer.as_deref(), f.year), (Some("Data East"), Some(1993)));
        let f = parse_folder("Space Cadet (Windows) nFozzy");
        assert_eq!((f.name.as_str(), f.year), ("Space Cadet (Windows) nFozzy", None));
    }

    #[test]
    fn the_best_match_wins_over_the_first() {
        let db = Database::new(vec![
                game("jp", "JP's Indiana Jones", "Stern", 2008),
                game("ij", "Indiana Jones", "Stern", 2008),
                game("wms", "Indiana Jones - The Pinball Adventure", "Williams", 1993),
                game("simp", "The Simpsons Pinball Party", "Stern Electronics", 2003),
                game("gnr", "Guns N' Roses", "Data East", 1994),
        ]);
        assert_eq!(db.find("Indiana Jones (Stern 2008)").unwrap().id, "ij");
        assert_eq!(db.find("Simpsons Pinball Party (Stern 2003)").unwrap().id, "simp");
        assert_eq!(db.find("Guns N Roses (Data East 1994)").unwrap().id, "gnr");
        assert!(db.find("Indiana Jones (Stern 2009)").is_none());
        assert!(db.find("Something Else (Stern 2008)").is_none());
        assert_eq!(db.search("indiana", 10).len(), 3);
        assert_eq!(db.search("indiana 1993", 10)[0].id, "wms");
    }

    #[test]
    fn ipdb_and_tutorial_links_come_from_the_entry() {
        let mut g = game("x", "X", "Y", 2000);
        g.ipdb_url = Some("https://www.ipdb.org/machine.cgi?id=3781".into());
        g.tutorial_files = vec![serde_json::json!({"urls": [{"url": "https://pinballprimer.github.io/afm.html"}]})];
        assert_eq!(g.ipdb_id(), "3781");
        assert_eq!(g.pinball_primer().as_deref(), Some("https://pinballprimer.github.io/afm.html"));
    }
}

