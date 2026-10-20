//! Legge da un `.vpx` i dati che servono alla scheda del tavolo. Il file è un archivio
//! OLE (Compound File): i testi stanno in `TableInfo/…`, lo script in `GameStg/GameData`.
//! I valori sono calcolati come fa VPinFE, così le schede scritte da lui e da noi coincidono
//! e gli hash degli script si confrontano con quelli delle patch pubblicate.

use crate::hash;
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct VpxInfo {
    pub version: String,
    pub author: String,
    pub release_date: String,
    pub save_date: String,
    pub save_rev: String,
    pub company_name: String,
    pub company_year: String,
    pub table_type: String,
    /// Script eseguito dal motore (il `.vbs` accanto al tavolo, se c'è), con a capo CRLF.
    pub script: String,
    pub script_hash: String,
    /// Hash dello script interno al tavolo: è quello da confrontare con l'elenco delle patch.
    pub embedded_script_hash: String,
    pub file_hash: String,
    pub rom: String,
}

impl VpxInfo {
    /// Riprende lo script dal `.vbs` accanto al tavolo, per esempio dopo averci messo una patch.
    pub fn refresh_script(&mut self, vpx: &Path) {
        if let Some(script) = sidecar_script(vpx) {
            self.set_script(script);
        }
    }

    fn set_script(&mut self, script: String) {
        self.script_hash = hash::sha256(script.as_bytes());
        self.rom = rom_name(&script);
        self.script = script;
    }
}

/// I testi sono UTF-16: come VPinFE si leggono byte per byte togliendo gli zeri, il che
/// basta per i caratteri latini.
fn latin1_without_nul(bytes: &[u8]) -> String {
    bytes.iter().filter(|&&b| b != 0).map(|&b| b as char).collect()
}

/// UTF-8 che salta le sequenze non valide, come `decode("utf-8", errors="ignore")`:
/// serve per ottenere lo stesso hash dello script.
fn utf8_ignoring_errors(mut bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    loop {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                text.push_str(valid);
                return text;
            }
            Err(e) => {
                let (valid, rest) = bytes.split_at(e.valid_up_to());
                text.push_str(std::str::from_utf8(valid).unwrap_or_default());
                match e.error_len() {
                    Some(len) => bytes = &rest[len..],
                    None => return text,
                }
            }
        }
    }
}

fn crlf(text: &str) -> String {
    if text.contains("\r\n") && !text.replace("\r\n", "").contains('\n') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Testo di uno script come lo si confronta con gli hash: UTF-8 e a capo CRLF.
pub fn script_text(bytes: &[u8]) -> String {
    crlf(&utf8_ignoring_errors(bytes))
}

/// Lo script sta dopo il marcatore `CODE`, preceduto dalla sua lunghezza in 4 byte.
fn script_from_game_data(data: &[u8]) -> String {
    let Some(at) = data.windows(4).position(|w| w == b"CODE") else { return String::new() };
    let start = at + 4;
    let Some(len) = data.get(start..start + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])) else {
        return String::new();
    };
    let end = (start + 4).saturating_add(len.max(0) as usize).min(data.len());
    script_text(&data[start + 4..end])
}

/// Toglie i commenti VBScript (`'` fuori dalle stringhe).
fn strip_comments(script: &str) -> String {
    let mut code = String::with_capacity(script.len());
    for line in script.lines() {
        let mut in_string = false;
        for c in line.chars() {
            if c == '"' {
                in_string = !in_string;
            } else if c == '\'' && !in_string {
                break;
            }
            code.push(c);
        }
        code.push('\n');
    }
    code
}

/// Valore tra virgolette dopo `<key> =` (spazi facoltativi), cercando `key` senza
/// distinzione di maiuscole. `whole_run`: il valore non contiene spazi e finisce
/// all'ultima virgoletta della parola, come `"([^\s]+)"`.
fn quoted_after(text: &str, key: &str, whole_run: bool) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(key).map(|i| i + from) {
        from = i + key.len();
        let rest = text[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('"') else { continue };
        let value = if whole_run {
            let run = &rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())];
            run.rfind('"').map(|end| &run[..end])
        } else {
            rest.find('"').map(|end| &rest[..end])
        };
        if let Some(value) = value.filter(|v| !v.is_empty() && !v.contains('\n')) {
            return Some(value.to_string());
        }
    }
    None
}

/// Nome della ROM dichiarato nello script (`cGameName = "…"`, altrimenti `OptRom = "…"`).
pub fn rom_name(script: &str) -> String {
    let code = strip_comments(script);
    quoted_after(&code, "gamename", false)
        .or_else(|| quoted_after(&code, "optrom", true))
        .unwrap_or_default()
}

/// Script nel `.vbs` con lo stesso nome del tavolo, che il motore esegue al posto di
/// quello interno.
fn sidecar_script(vpx: &Path) -> Option<String> {
    let bytes = std::fs::read(vpx.with_extension("vbs")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(crlf(text.strip_prefix('\u{feff}').unwrap_or(&text)))
}

fn embedded_script<F: Read + io::Seek>(ole: &mut cfb::CompoundFile<F>) -> io::Result<String> {
    let mut data = Vec::new();
    ole.open_stream("/GameStg/GameData")?.read_to_end(&mut data)?;
    Ok(script_from_game_data(&data))
}

/// Legge il tavolo: testi della scheda, script e impronte.
pub fn read(path: &Path) -> io::Result<VpxInfo> {
    let mut ole = cfb::open(path)?;
    let mut text = |name: &str| -> String {
        let mut bytes = Vec::new();
        match ole.open_stream(format!("/TableInfo/{name}")).and_then(|mut s| s.read_to_end(&mut bytes)) {
            Ok(_) => latin1_without_nul(&bytes),
            Err(_) => String::new(),
        }
    };
    let mut info = VpxInfo {
        version: text("TableVersion"),
        author: text("AuthorName"),
        release_date: text("ReleaseDate"),
        save_date: text("TableSaveDate"),
        save_rev: text("TableSaveRev"),
        company_name: text("CompanyName"),
        company_year: text("CompanyYear"),
        table_type: text("TableType"),
        file_hash: hash::sha256_file(path)?,
        ..VpxInfo::default()
    };
    let embedded = embedded_script(&mut ole)?;
    info.embedded_script_hash = hash::sha256(embedded.as_bytes());
    info.set_script(sidecar_script(path).unwrap_or(embedded));
    Ok(info)
}

/// Script eseguito dal motore, senza calcolare le impronte: basta per `uses_pinmame`.
pub fn read_script(path: &Path) -> io::Result<String> {
    match sidecar_script(path) {
        Some(script) => Ok(script),
        None => embedded_script(&mut cfb::open(path)?),
    }
}

/// Il tavolo usa PinMAME, e quindi una ROM: lo script carica `core.vbs` con `LoadVPM`.
/// Altri tavoli dichiarano un `cGameName` solo per dare un nome ai record.
pub fn uses_pinmame(script: &str) -> bool {
    strip_comments(script).to_lowercase().contains("loadvpm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_follows_the_code_marker() {
        let mut data = b"xxxxCODE".to_vec();
        data.extend_from_slice(&5i32.to_le_bytes());
        data.extend_from_slice(b"a\nb\xffcTAIL");
        assert_eq!(script_from_game_data(&data), "a\r\nbc");
        assert_eq!(script_from_game_data(b"nothing"), "");
    }

    #[test]
    fn line_endings_become_crlf() {
        assert_eq!(crlf("a\r\nb"), "a\r\nb");
        assert_eq!(crlf("a\nb\r\nc"), "a\r\nb\r\nc");
    }

    #[test]
    fn rom_name_skips_comments_and_falls_back_to_optrom() {
        let script = "' Const cGameName = \"old\"\r\nConst cGameName = \"afm_113b\" ' comment\r\n";
        assert_eq!(rom_name(script), "afm_113b");
        assert_eq!(rom_name("Const cGameName=\"tafg_lx3\""), "tafg_lx3");
        assert_eq!(rom_name("Const OptRom = \"mm_109c\")"), "mm_109c");
        assert_eq!(rom_name("Dim x = \"no rom\""), "");
        assert_eq!(rom_name("GameName = \"\"\r\nOptRom = \"ok\""), "ok");
    }

    #[test]
    fn pinmame_tables_load_the_vpm_core() {
        assert!(uses_pinmame("On Error Resume Next\r\nLoadVPM \"01560000\", \"WPC.VBS\", 3.26"));
        assert!(!uses_pinmame("' LoadVPM \"x\"\r\nConst cGameName = \"jpspacecadet\""));
    }
}
