//! Richieste HTTP condivise: aggiornamento dei programmi, database dei tavoli, copertine.

use anyhow::{anyhow, bail, Context, Result};
use std::io::Read;
use std::time::Duration;

const USER_AGENT: &str = concat!("pinball-aio/", env!("CARGO_PKG_VERSION"));

pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
}

fn call(url: &str) -> Result<ureq::Response> {
    match agent().get(url).call() {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(403 | 429, _)) => bail!("GitHub ha limitato le richieste, riprova tra un'ora"),
        Err(ureq::Error::Status(code, _)) => bail!("{url}: risposta {code}"),
        Err(e) => Err(anyhow!(e).context(format!("impossibile contattare {url}"))),
    }
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    call(url)?.into_reader().read_to_end(&mut bytes).context("download interrotto")?;
    Ok(bytes)
}

pub fn get_text(url: &str) -> Result<String> {
    String::from_utf8(get_bytes(url)?).context("risposta non valida")
}
