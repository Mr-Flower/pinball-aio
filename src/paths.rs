use std::path::PathBuf;

/// Cartelle usate dal launcher, tutte sotto un'unica radice.
pub struct Paths {
    pub root: PathBuf,
}

/// Unico file fuori dalla cartella dati: ricorda dove l'utente l'ha messa.
fn location_file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("pinball-aio").join("location.txt"))
}

impl Paths {
    /// Ordine: variabile `PINBALL_AIO_DIR`, poi una cartella `PinballData` accanto
    /// all'eseguibile (modalità portatile), poi la cartella scelta al primo avvio.
    /// `None` se non è ancora stata scelta o se è stata cancellata: va richiesta.
    pub fn detect() -> Option<Self> {
        if let Some(dir) = std::env::var_os("PINBALL_AIO_DIR").filter(|d| !d.is_empty()) {
            return Some(Self { root: PathBuf::from(dir) });
        }
        // Dentro un AppImage current_exe punta al mount temporaneo: conta il file .AppImage.
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        if let Some(portable) = exe.as_deref().and_then(|e| e.parent()).map(|d| d.join("PinballData")) {
            if portable.is_dir() {
                return Some(Self { root: portable });
            }
        }
        let saved = std::fs::read_to_string(location_file()?).ok()?;
        let root = PathBuf::from(saved.trim());
        (root.is_absolute() && root.is_dir()).then_some(Self { root })
    }

    pub fn default_root() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Pinball")
    }

    /// Dalla cartella indicata nel selettore ricava quella da usare: una cartella vuota
    /// o già nostra va bene così com'è, altrimenti si crea dentro una sottocartella
    /// `Pinball`, così cancellarla non tocca gli altri file dell'utente.
    pub fn root_for_picked(picked: PathBuf) -> PathBuf {
        let ours = picked.join("engine").is_dir();
        let empty = std::fs::read_dir(&picked).map(|mut d| d.next().is_none()).unwrap_or(true);
        if ours || empty {
            picked
        } else {
            picked.join("Pinball")
        }
    }

    /// Crea la cartella scelta e la ricorda per i prossimi avvii.
    pub fn choose(root: PathBuf) -> std::io::Result<Self> {
        if !root.is_absolute() {
            return Err(std::io::Error::other("serve un percorso completo"));
        }
        std::fs::create_dir_all(&root)?;
        if let Some(file) = location_file() {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(file, root.display().to_string())?;
        }
        Ok(Self { root })
    }

    pub fn tables(&self) -> PathBuf {
        self.root.join("tables")
    }

    /// ROM condivise, usate dai tavoli che non hanno una loro cartella `pinmame`.
    pub fn pinmame(&self) -> PathBuf {
        self.root.join("pinmame")
    }

    pub fn engine(&self) -> PathBuf {
        self.root.join("engine")
    }

    pub fn downloads(&self) -> PathBuf {
        self.engine().join(".download")
    }

    pub fn versions_file(&self) -> PathBuf {
        self.engine().join("versions.json")
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn vpx_ini(&self) -> PathBuf {
        self.config().join("VPinballX.ini")
    }

    /// Cartella in cui il motore tiene log e dati propri (solo Linux, vedi `config.rs`).
    pub fn vpx_data(&self) -> PathBuf {
        self.config().join("vpx-data")
    }

    /// Configurazione di VPinFE, usato fino alla versione 0.1.4: si cancella al primo avvio.
    pub fn retired_vpinfe_config(&self) -> PathBuf {
        self.config().join("vpinfe")
    }

    /// Database dei tavoli, indice delle copertine e miniature: si può cancellare, si rifà da sé.
    pub fn cache(&self) -> PathBuf {
        self.config().join("cache")
    }

    pub fn thumbs(&self) -> PathBuf {
        self.cache().join("thumbs")
    }

    /// Tavoli non riconosciuti nel database, per non riprovarci a ogni avvio.
    pub fn tried_file(&self) -> PathBuf {
        self.config().join("tavoli-senza-scheda.txt")
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [
            self.tables(),
            self.pinmame().join("roms"),
            self.pinmame().join("nvram"),
            self.engine(),
            self.config(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
