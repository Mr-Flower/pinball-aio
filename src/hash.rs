//! Impronte usate per verificare i file scaricati e riconoscere tavoli e script.

use md5::Md5;
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut text, b| {
        let _ = write!(text, "{b:02x}");
        text
    })
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn md5(bytes: &[u8]) -> String {
    hex(&Md5::digest(bytes))
}

/// sha256 di un file letto a pezzi: i tavoli arrivano a centinaia di MB.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..n]);
    }
}

/// FNV-1a a 64 bit: stabile tra versioni del compilatore, a differenza dell'hasher della
/// libreria standard, quindi adatto ai nomi dei file in cache.
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    parts.iter().flat_map(|part| part.iter()).fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digests() {
        assert_eq!(md5(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(fnv1a(&[b"a"]), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(&[b"ab", b"c"]), fnv1a(&[b"abc"]));
    }
}
