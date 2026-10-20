//! Prepara i tavoli nuovi: li riconosce nel database VPS, ne scrive la scheda, scarica le
//! copertine e, su Linux, applica le correzioni agli script. Prende il posto di
//! `vpinfe --buildmeta` e `vpinfe --vpxpatch`. Un errore su un tavolo non ferma gli altri,
//! e un tavolo non riconosciuto resta comunque giocabile.

use crate::catalog::{self, Table};
use crate::library;
use crate::media;
use crate::paths::Paths;
use crate::update::Progress;
use crate::vps::{Database, Game};
use crate::{patches, vpx};
use anyhow::{anyhow, Result};
use std::path::Path;

/// Tavoli ancora senza scheda e mai provati prima (vedi `library::tried`).
pub fn pending(paths: &Paths) -> Vec<String> {
    let tried = library::tried(paths);
    library::without_info(paths).into_iter().filter(|name| !tried.contains(name)).collect()
}

/// Prepara i tavoli di `pending`. Restituisce quanti ne ha riconosciuti.
pub fn prepare_new(paths: &Paths, report: &dyn Fn(Progress)) -> usize {
    let folders = pending(paths);
    if folders.is_empty() {
        return 0;
    }
    report(Progress { text: "Scarico l'elenco dei tavoli…".into(), fraction: None });
    let db = match Database::load(paths) {
        Ok(db) => db,
        Err(e) => {
            // Senza database si riproverà al prossimo avvio: non si annota nulla.
            eprintln!("schede dei tavoli non create: {e:#}");
            return 0;
        }
    };
    let index = media::Index::load(paths).map_err(|e| eprintln!("copertine non scaricate: {e:#}")).ok();
    let fixes = if cfg!(target_os = "linux") {
        patches::load().map_err(|e| eprintln!("patch degli script non controllate: {e:#}")).ok()
    } else {
        None
    };
    let mut found = 0;
    for (i, folder) in folders.iter().enumerate() {
        report(Progress {
            text: format!("Preparo {folder} ({} di {})…", i + 1, folders.len()),
            fraction: Some(i as f32 / folders.len() as f32),
        });
        let dir = paths.tables().join(folder);
        let game = db.find(folder);
        match prepare_one(&dir, game, index.as_ref(), fixes.as_deref()) {
            Ok(()) => found += usize::from(game.is_some()),
            Err(e) => eprintln!("{folder}: {e:#}"),
        }
    }
    // I tavoli non riconosciuti si annotano, così non si rifà il giro a ogni avvio: si
    // possono abbinare a mano dalla libreria.
    if let Err(e) = library::set_tried(paths, &library::without_info(paths)) {
        eprintln!("elenco dei tavoli senza scheda non salvato: {e}");
    }
    found
}

fn prepare_one(dir: &Path, game: Option<&Game>, index: Option<&media::Index>, fixes: Option<&[patches::Entry]>) -> Result<()> {
    let table = catalog::load(dir).ok_or_else(|| anyhow!("nessun .vpx"))?;
    let mut info = vpx::read(&table.vpx)?;
    let mut patched = false;
    if let Some(fixes) = fixes {
        match patches::apply(fixes, &table.vpx, &info.embedded_script_hash) {
            Ok(applied) => patched = applied,
            Err(e) => eprintln!("{}: patch dello script non applicata: {e:#}", table.folder),
        }
    }
    let Some(game) = game else { return Ok(()) };
    if patched {
        // Ora il motore esegue lo script corretto: la scheda descrive quello.
        info.refresh_script(&table.vpx);
    }
    catalog::write_info(dir, game, &table.vpx, &info)?;
    if patched {
        catalog::set_patch_applied(dir)?;
    }
    if let Some(index) = index {
        if let Err(e) = media::download(index, dir, &game.id) {
            eprintln!("{}: {e:#}", table.folder);
        }
    }
    Ok(())
}

/// Annota nelle schede scritte da VPinFE se il tavolo usa PinMAME, che serve per avvisare
/// quando manca la ROM. Si fa una volta per tavolo: lo script va letto dal `.vpx`.
pub fn mark_pinmame(paths: &Paths, report: &dyn Fn(Progress)) {
    let unknown: Vec<Table> =
        catalog::load_all(paths).into_iter().filter(|t| t.has_info && t.uses_pinmame.is_none()).collect();
    for (i, table) in unknown.iter().enumerate() {
        report(Progress {
            text: format!("Controllo i tavoli ({} di {})…", i + 1, unknown.len()),
            fraction: Some(i as f32 / unknown.len() as f32),
        });
        match vpx::read_script(&table.vpx) {
            Ok(script) => {
                if let Err(e) = catalog::set_uses_pinmame(&table.dir, vpx::uses_pinmame(&script)) {
                    eprintln!("{}: {e}", table.folder);
                }
            }
            Err(e) => eprintln!("{}: script non leggibile: {e}", table.folder),
        }
    }
}

/// Abbina a mano un tavolo a una voce del database e ne scarica le copertine.
pub fn assign(paths: &Paths, table: &Table, game: &Game) -> Result<()> {
    let info = vpx::read(&table.vpx)?;
    // Le copertine scaricate per la voce di prima non valgono più.
    if table.vps_id != game.id {
        media::forget(&table.dir)?;
    }
    catalog::write_info(&table.dir, game, &table.vpx, &info)?;
    match media::Index::load(paths) {
        Ok(index) => {
            if let Err(e) = media::download(&index, &table.dir, &game.id) {
                eprintln!("{}: {e:#}", table.folder);
            }
        }
        Err(e) => eprintln!("copertine non scaricate: {e:#}"),
    }
    library::set_tried(paths, &library::without_info(paths))?;
    Ok(())
}
