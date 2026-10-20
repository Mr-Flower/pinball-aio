//! Correzioni agli script dei tavoli per il motore su Linux ("standalone"), dal progetto
//! vpx-standalone-scripts: se lo script del tavolo è identico a uno già corretto, la
//! versione corretta si mette accanto al `.vpx` come `.vbs`, che il motore usa al suo posto.
//! Su Windows non servono: lì gli script girano con il VBScript di sistema.

use crate::{files, hash, net, vpx};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

const HASHES_URL: &str = "https://raw.githubusercontent.com/jsm174/vpx-standalone-scripts/refs/heads/master/hashes.json";

#[derive(Deserialize)]
struct Patched {
    sha256: String,
    url: String,
}

#[derive(Deserialize)]
pub struct Entry {
    sha256: String,
    patched: Patched,
}

pub fn load() -> Result<Vec<Entry>> {
    serde_json::from_str(&net::get_text(HASHES_URL)?).context("elenco delle patch non leggibile")
}

/// Applica la patch al tavolo se ce n'è una per il suo script. Un `.vbs` già presente
/// (messo dall'utente o da una patch precedente) non si tocca. `true` se l'ha applicata.
pub fn apply(entries: &[Entry], table: &Path, script_hash: &str) -> Result<bool> {
    let sidecar = table.with_extension("vbs");
    if sidecar.exists() {
        return Ok(false);
    }
    let Some(entry) = entries.iter().find(|e| e.sha256.eq_ignore_ascii_case(script_hash)) else {
        return Ok(false);
    };
    // GitHub serve i file con a capo LF; l'hash pubblicato è del testo con a capo CRLF,
    // come lo script dentro il tavolo.
    let script = vpx::script_text(&net::get_bytes(&entry.patched.url.replace(' ', "%20"))?);
    if !hash::sha256(script.as_bytes()).eq_ignore_ascii_case(&entry.patched.sha256) {
        bail!("la patch scaricata è diversa da quella pubblicata");
    }
    files::write_atomic(&sidecar, script.as_bytes())?;
    Ok(true)
}
