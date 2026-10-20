//! Copertine dei tavoli da VPinMediaDB: un indice JSON per identificativo VPS con
//! l'indirizzo e l'md5 di ogni immagine. Si scaricano solo le immagini che la libreria
//! mostra, nella versione "1k"; niente video né audio.

use crate::catalog;
use crate::{files, hash, net};
use crate::paths::Paths;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

const INDEX_URL: &str = "https://github.com/superhac/vpinmediadb/raw/refs/heads/main/vpinmdb.json";

/// Chiave nell'indice, sottogruppo di risoluzione (se c'è) e nome del file in `medias/`.
/// I nomi sono quelli di VPinFE.
const WANTED: [(&str, Option<&str>, &str); 6] = [
    ("bg", Some("1k"), "bg.png"),
    ("table", Some("1k"), "table.png"),
    ("dmd", Some("1k"), "dmd.png"),
    ("wheel", None, "wheel.png"),
    ("cab", None, "cab.png"),
    ("flyer", None, "flyer.png"),
];

pub struct Index {
    data: Value,
}

impl Index {
    /// Scarica l'indice; senza rete usa l'ultima copia scaricata.
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.cache().join("vpinmdb.json");
        let bytes = match net::get_bytes(INDEX_URL) {
            Ok(bytes) => {
                fs::create_dir_all(paths.cache())?;
                files::write_atomic(&file, &bytes)?;
                bytes
            }
            Err(e) => fs::read(&file).map_err(|_| e).context("indice delle copertine non raggiungibile")?,
        };
        Ok(Self { data: serde_json::from_slice(&bytes).context("indice delle copertine non leggibile")? })
    }

    fn entry(&self, vps_id: &str, key: &str, group: Option<&str>) -> Option<(String, String)> {
        let mut node = self.data.get(vps_id)?;
        if let Some(group) = group {
            node = node.get(group)?;
        }
        let url = node.get(key)?.as_str()?.to_string();
        let md5 = node.get(format!("{key}_md5"))?.as_str().unwrap_or_default().to_string();
        Some((url, md5))
    }
}

/// Scarica le copertine mancanti o cambiate di un tavolo riconosciuto. Non tocca le
/// immagini messe dall'utente: quelle già presenti e non registrate come scaricate.
/// Restituisce quante ne ha scaricate.
pub fn download(index: &Index, dir: &Path, vps_id: &str) -> Result<usize> {
    let medias = dir.join("medias");
    let mut count = 0;
    let mut errors = Vec::new();
    for (key, group, file) in WANTED {
        let Some((url, md5)) = index.entry(vps_id, key, group) else { continue };
        let target = medias.join(file);
        if target.exists() {
            match catalog::media_source(dir, key) {
                Some((source, old)) if source == "vpinmediadb" && !old.eq_ignore_ascii_case(&md5) => {}
                _ => continue,
            }
        }
        let result = net::get_bytes(&url).and_then(|bytes| {
            if !md5.is_empty() && !hash::md5(&bytes).eq_ignore_ascii_case(&md5) {
                bail!("{file}: file scaricato diverso da quello pubblicato");
            }
            fs::create_dir_all(&medias)?;
            files::write_atomic(&target, &bytes)?;
            catalog::record_media(dir, key, file, &md5)?;
            Ok(())
        });
        match result {
            Ok(()) => count += 1,
            Err(e) => errors.push(format!("{e:#}")),
        }
    }
    if count == 0 && !errors.is_empty() {
        bail!("copertine non scaricate: {}", errors.join("; "));
    }
    Ok(count)
}

/// Cancella le copertine scaricate per un tavolo, lasciando quelle messe dall'utente.
pub fn forget(dir: &Path) -> std::io::Result<()> {
    for file in catalog::forget_downloaded_media(dir)? {
        // Solo nomi di file semplici: la scheda potrebbe essere stata scritta da altri.
        if !file.is_empty() && Path::new(&file).file_name() == Some(std::ffi::OsStr::new(&file)) {
            let _ = fs::remove_file(dir.join("medias").join(file));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_read_by_resolution_group() {
        let index = Index {
            data: serde_json::json!({
                "abc": {"1k": {"bg": "u1", "bg_md5": "m1"}, "wheel": "u2", "wheel_md5": "m2"}
            }),
        };
        assert_eq!(index.entry("abc", "bg", Some("1k")), Some(("u1".into(), "m1".into())));
        assert_eq!(index.entry("abc", "wheel", None), Some(("u2".into(), "m2".into())));
        assert_eq!(index.entry("abc", "table", Some("1k")), None);
        assert_eq!(index.entry("zzz", "wheel", None), None);
    }
}
