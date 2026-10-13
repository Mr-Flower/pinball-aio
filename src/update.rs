//! Scarica e aggiorna i due programmi dalle loro release ufficiali su GitHub.
//! I binari non vengono ridistribuiti: arrivano non modificati dai repository originali.

use crate::paths::Paths;
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USER_AGENT: &str = concat!("pinball-aio/", env!("CARGO_PKG_VERSION"));

pub struct Component {
    pub id: &'static str,
    pub name: &'static str,
    pub repo: &'static str,
    /// Se c'è, si installa sempre e solo questa release invece della più recente.
    pin: Option<&'static str>,
    is_asset: fn(&str) -> bool,
    is_exe: fn(&str) -> bool,
}

// VPinballX: le build 10.8.1 sono pre-release, quindi si guarda l'elenco completo
// delle release e non solo "latest". "-windows-x64-" esclude la variante mingw.
// La versione è ferma su una build provata e si cambia qui a mano: le pre-release nuove
// possono rompere i tavoli. La 5436 non passa ai tavoli i solenoidi oltre il 32 delle
// Stern SAM (in Metallica la bocca del serpente resta chiusa); con la 3788 funzionano.
pub const VPINBALL: Component = Component {
    id: "vpinball",
    name: "VPinballX",
    repo: "vpinball/vpinball",
    pin: Some("v10.8.1-5436-af26b2d93"),
    #[cfg(target_os = "linux")]
    is_asset: |n| n.starts_with("VPinballX_BGFX-") && n.contains("-linux-x64-Release."),
    #[cfg(windows)]
    is_asset: |n| n.starts_with("VPinballX_BGFX-") && n.ends_with("-windows-x64-Release.zip"),
    #[cfg(target_os = "linux")]
    is_exe: |n| n == "VPinballX_BGFX",
    #[cfg(windows)]
    is_exe: |n| n.starts_with("VPinballX_BGFX") && n.ends_with(".exe"),
};

// VPinFE: si usa la build completa (con Chromium incluso), così non serve altro sul sistema.
pub const VPINFE: Component = Component {
    id: "vpinfe",
    name: "VPinFE",
    repo: "superhac/vpinfe",
    pin: None,
    #[cfg(target_os = "linux")]
    is_asset: |n| n.starts_with("vpinfe-") && n.ends_with("-linux-x64.zip"),
    #[cfg(windows)]
    is_asset: |n| n.starts_with("vpinfe-") && n.ends_with("-win-x64.zip"),
    #[cfg(target_os = "linux")]
    is_exe: |n| n == "vpinfe",
    #[cfg(windows)]
    is_exe: |n| n == "vpinfe.exe",
};

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    /// "sha256:<hex>", presente sulle release recenti.
    digest: Option<String>,
}

pub struct Progress {
    pub text: String,
    /// `None` quando la durata non è nota (controllo, estrazione).
    pub fraction: Option<f32>,
}

pub enum Outcome {
    UpToDate(String),
    Updated(String),
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
}

fn github<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    match agent().get(url).set("Accept", "application/vnd.github+json").call() {
        Ok(response) => response.into_json().context("risposta di GitHub non valida"),
        Err(ureq::Error::Status(403 | 429, _)) => bail!("GitHub ha limitato le richieste, riprova tra un'ora"),
        Err(e) => Err(anyhow!(e).context("impossibile contattare GitHub")),
    }
}

/// La release da installare: quella fissata dal componente, altrimenti la più recente.
fn wanted_asset(component: &Component) -> Result<(String, Asset)> {
    let api = format!("https://api.github.com/repos/{}/releases", component.repo);
    let releases = match component.pin {
        Some(tag) => vec![github::<Release>(&format!("{api}/tags/{tag}"))?],
        None => github(&format!("{api}?per_page=15"))?,
    };
    pick(releases, component)
        .ok_or_else(|| anyhow!("nessuna build di {} trovata per questo sistema", component.name))
}

/// La release più recente (GitHub le elenca dalla più nuova) con un pacchetto per questo sistema.
/// Le pre-release valgono solo per un componente fissato: lì la scelta è già stata fatta a mano.
fn pick(releases: Vec<Release>, component: &Component) -> Option<(String, Asset)> {
    releases
        .into_iter()
        .filter(|r| !r.draft && (component.pin.is_some() || !r.prerelease))
        .find_map(|r| {
            let asset = r.assets.into_iter().find(|a| (component.is_asset)(&a.name))?;
            Some((r.tag_name, asset))
        })
}

fn read_versions(paths: &Paths) -> BTreeMap<String, String> {
    fs::read(paths.versions_file())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn installed_version(paths: &Paths, component: &Component) -> Option<String> {
    read_versions(paths).remove(component.id)
}

fn find_file(dir: &Path, is_match: fn(&str) -> bool, depth: u32) -> Option<PathBuf> {
    let entries: Vec<_> = fs::read_dir(dir).ok()?.flatten().collect();
    for entry in &entries {
        if entry.path().is_file() && entry.file_name().to_str().is_some_and(is_match) {
            return Some(entry.path());
        }
    }
    if depth == 0 {
        return None;
    }
    entries
        .iter()
        .filter(|e| e.path().is_dir())
        .find_map(|e| find_file(&e.path(), is_match, depth - 1))
}

/// Percorso dell'eseguibile installato, se c'è.
pub fn exe_path(paths: &Paths, component: &Component) -> Option<PathBuf> {
    find_file(&paths.engine().join(component.id), component.is_exe, 3)
}

fn download(asset: &Asset, dest: &Path, name: &str, report: &dyn Fn(Progress)) -> Result<()> {
    let response = agent().get(&asset.browser_download_url).call().context("download non riuscito")?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(asset.size)
        .max(1);
    let mut reader = response.into_reader();
    let mut file = File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last_percent = u64::MAX;
    loop {
        let n = reader.read(&mut buffer).context("download interrotto")?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        hasher.update(&buffer[..n]);
        done += n as u64;
        let percent = done * 100 / total;
        if percent != last_percent {
            last_percent = percent;
            report(Progress {
                text: format!("Scarico {name}: {} di {} MB", done / 1_000_000, total / 1_000_000),
                fraction: Some(done as f32 / total as f32),
            });
        }
    }
    if done != asset.size {
        bail!("download incompleto ({done} byte su {})", asset.size);
    }
    if let Some(expected) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !actual.eq_ignore_ascii_case(expected) {
            bail!("il file scaricato è corrotto (sha256 diverso da quello pubblicato)");
        }
    }
    Ok(())
}

fn extract(archive: &Path, dest: &Path) -> Result<()> {
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest)?;
    let file = File::open(archive)?;
    if archive.extension().is_some_and(|e| e == "zip") {
        zip::ZipArchive::new(file)?.extract(dest)?;
        // Le build Linux di VPinballX meno recenti sono un .tar.gz chiuso dentro lo zip.
        for entry in fs::read_dir(dest)?.flatten() {
            let inner = entry.path();
            if inner.is_file() && entry.file_name().to_str().is_some_and(|n| n.ends_with(".tar.gz")) {
                tar::Archive::new(flate2::read::GzDecoder::new(File::open(&inner)?)).unpack(dest)?;
                fs::remove_file(&inner)?;
            }
        }
    } else {
        tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(dest)?;
    }
    Ok(())
}

/// Se un aggiornamento precedente si è interrotto a metà dello scambio, rimette la versione vecchia.
fn recover(current: &Path, previous: &Path) {
    if !current.exists() && previous.exists() {
        let _ = fs::rename(previous, current);
    }
}

/// Controlla la release voluta e, se è diversa da quella installata, la scarica e la installa.
/// La versione precedente resta in `<id>.old` per poter tornare indietro a mano.
pub fn update(paths: &Paths, component: &Component, report: &dyn Fn(Progress)) -> Result<Outcome> {
    let name = component.name;
    let current = paths.engine().join(component.id);
    let previous = paths.engine().join(format!("{}.old", component.id));
    let staging = paths.engine().join(format!("{}.new", component.id));
    recover(&current, &previous);

    report(Progress { text: format!("Controllo aggiornamenti di {name}…"), fraction: None });
    let (tag, asset) = wanted_asset(component)?;
    if installed_version(paths, component).as_deref() == Some(&tag) && exe_path(paths, component).is_some() {
        return Ok(Outcome::UpToDate(tag));
    }

    fs::create_dir_all(paths.downloads())?;
    let archive = paths.downloads().join(&asset.name);
    download(&asset, &archive, name, report)?;

    report(Progress { text: format!("Installo {name} {tag}…"), fraction: None });
    extract(&archive, &staging).with_context(|| format!("estrazione di {} non riuscita", asset.name))?;
    if find_file(&staging, component.is_exe, 3).is_none() {
        bail!("l'archivio di {name} non contiene l'eseguibile atteso");
    }
    if current.exists() {
        let _ = fs::remove_dir_all(&previous);
        fs::rename(&current, &previous)
            .with_context(|| format!("impossibile sostituire {name}: è ancora in esecuzione?"))?;
    }
    if let Err(e) = fs::rename(&staging, &current) {
        recover(&current, &previous);
        return Err(e.into());
    }

    let mut versions = read_versions(paths);
    versions.insert(component.id.to_string(), tag.clone());
    fs::write(paths.versions_file(), serde_json::to_vec_pretty(&versions)?)?;
    let _ = fs::remove_dir_all(paths.downloads());
    Ok(Outcome::Updated(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, asset: &str) -> Release {
        let asset = Asset { name: asset.to_string(), browser_download_url: String::new(), size: 0, digest: None };
        Release { tag_name: tag.to_string(), draft, prerelease: false, assets: vec![asset] }
    }

    #[test]
    fn picks_the_newest_release_with_a_package_for_this_system() {
        #[cfg(target_os = "linux")]
        let package = "vpinfe-v2.6.6-linux-x64.zip";
        #[cfg(windows)]
        let package = "vpinfe-v2.6.6-win-x64.zip";
        let beta = Release { prerelease: true, ..release("v2.7.0-beta", false, package) };
        let releases = vec![
            release("v2.7.0", true, package),
            beta,
            release("v2.6.7", false, "vpinfe-v2.6.7-macos-arm64.zip"),
            release("v2.6.6", false, package),
            release("v2.6.5", false, package),
        ];
        assert_eq!(pick(releases, &VPINFE).unwrap().0, "v2.6.6");
    }

    #[test]
    fn the_engine_is_pinned_and_the_frontend_is_not() {
        assert!(VPINBALL.pin.is_some());
        assert!(VPINFE.pin.is_none());
    }

    #[test]
    fn a_pinned_release_can_be_a_prerelease() {
        #[cfg(target_os = "linux")]
        let package = "VPinballX_BGFX-10.8.1-5436-af26b2d93-linux-x64-Release.tar.gz";
        #[cfg(windows)]
        let package = "VPinballX_BGFX-10.8.1-5436-af26b2d93-windows-x64-Release.zip";
        let pinned = Release { prerelease: true, ..release("v10.8.1-5436-af26b2d93", false, package) };
        assert!(pick(vec![pinned], &VPINBALL).is_some());
    }
}
