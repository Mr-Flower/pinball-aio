# Pinball AIO

Launcher per Windows e Linux che scarica e tiene aggiornato **VPinballX** (il motore) e mostra i tavoli in una libreria propria a tiles con le copertine, da cui si gioca. Modello di riferimento: Pocket Pinball / l'app VPX per Android, cioè "installa e gioca".

- Repository: https://github.com/Mr-Flower/pinball-aio (pubblico), ultima release `v0.1.4`; in sviluppo la 0.2.0 con la libreria propria.
- Il valore del progetto è solo la semplicità: nessun percorso da impostare. Multi-schermo, cabinet e personalizzazione spinta sono fuori scopo.

## Decisioni prese (2026-10-10)

- **Frontend proprio, niente più VPinFE** (deciso il 2026-10-20, al posto della scelta iniziale di VPinFE): libreria in egui nello stesso eseguibile, con le funzioni di VPinFE che servono (riconoscimento dei tavoli, schede, copertine, patch degli script) riscritte in Rust. La nuova versione cancella `engine/vpinfe` e `config/vpinfe`; le schede `.info` restano nello stesso formato.
- **VPinballX fermo su una build provata** (deciso il 2026-10-10). La build del motore è `pin` in `src/update.rs` (ora `v10.8.1-5436-af26b2d93`) e si cambia a mano dopo averla provata, con una nuova versione del launcher.
- **Niente binari di terzi nei pacchetti.** Il launcher scarica le release ufficiali da GitHub al primo avvio (serve internet). Deve restare gratuito: VPinball e PinMAME hanno parti sotto licenza "old MAME" non commerciale. ROM e tavoli non si includono né si scaricano mai.
- **Una cartella per tavolo** (formato ereditato da VPinFE), più una cartella `pinmame` condivisa accanto a `tables`.
- **Cartella dati scelta dall'utente al primo avvio**; tutto deve stare lì dentro, così cancellarla fa pulizia.
- **Uso previsto:** un solo monitor in orizzontale, vista desktop, DMD dentro la finestra del tavolo.
- **Icona:** la pallina di VPinballX, su richiesta dell'utente. Non è MIT: resta sotto la licenza di Visual Pinball (vedi `assets/README.md`).
- **Commit senza coautori:** niente righe di attribuzione a Claude nei messaggi di commit.

## Come funziona

A ogni avvio (`src/main.rs`, funzione `prepare`, in background mentre la finestra mostra l'avanzamento):

1. `paths.rs`: trova la cartella dati. Ordine: variabile `PINBALL_AIO_DIR`, cartella `PinballData` accanto all'eseguibile, percorso salvato in `~/.config/pinball-aio/location.txt`. Se manca o è stata cancellata, la finestra la chiede.
2. `update.rs`: legge da GitHub la release fissata del motore e, se il tag è diverso da quello in `engine/versions.json`, scarica, verifica l'sha256, estrae e scambia le cartelle (la precedente resta in `engine/vpinball.old`). Senza rete parte con ciò che è installato. Cancella i resti di VPinFE (`remove_vpinfe`).
3. `config.rs`: crea `config/VPinballX.ini` solo se manca; su Windows accende i plugin; se il motore cambia tra build con numerazione diversa di `BackglassOutput` / `ScoreViewOutput`, scambia 1 e 2.
4. `library.rs`: sposta i `.vpx` sciolti in `tables/` in una cartella per tavolo. `metadata.rs`: per le cartelle senza `.info` e non già provate (`config/tavoli-senza-scheda.txt`) riconosce il tavolo nel database VPS (`vps.rs`), legge il `.vpx` (`vpx.rs`), scrive la scheda (`catalog.rs`), scarica le copertine (`media.rs`) e su Linux applica le patch degli script (`patches.rs`). Poi annota nelle schede se il tavolo usa PinMAME (`usesPinMAME`, una volta per tavolo).
5. La finestra diventa la libreria (`frontend/`): tiles con copertina, ricerca, filtri, ordinamento, pannello di dettaglio, abbinamento a mano, aggiunta di file (pulsante o trascinamento), avvio dei tavoli (`play.rs`). `mod.rs` tiene stato e logica, `view.rs` disegna e raccoglie le scelte come `Action` eseguite dopo il disegno, `widgets.rs` ha tile e pulsanti, `theme.rs` colori e font (Barlow e Bebas Neue, OFL, in `assets/fonts`).

`pinball-aio --cli` fa i passi 1-4 senza finestra ed elenca i tavoli non riconosciuti: utile per provare.

## Struttura della cartella dati

```
<cartella scelta>/            (sul PC dell'utente: ~/Pinball)
  tables/<Tavolo (Marca Anno)>/   .vpx, .directb2s, .info, medias/, pupvideos/, pinmame/ (facoltativa)
  pinmame/roms|nvram/             ROM condivise (zip non estratti)
  config/VPinballX.ini            impostazioni del motore, dell'utente dopo la creazione
  config/cache/                   vpsdb.json, vpinmdb.json, thumbs/ (miniature): si rifà da sé
  config/vpx-data/                log e dati del motore (solo Linux)
  config/tavoli-senza-scheda.txt  tavoli non riconosciuti nel database VPS (c'è solo se ce ne sono)
  engine/vpinball/                più vpinball.old e versions.json
```

## Fatti verificati

**VPinballX 10.8.1** (build `VPinballX_BGFX`; i fatti sotto sono stati verificati sulla `v10.8.1-5436-af26b2d93`, tranne dove è indicata la 3788)

- Le 10.8.1 sono pre-release, non compaiono come "latest". Asset: `…-linux-x64-Release.tar.gz` (con symlink) e `…-windows-x64-Release.zip` (esiste anche una variante `mingw`, da escludere). Nella 3788 l'asset Linux è un `.zip` con dentro il `.tar.gz`.
- **Difetto della 5436:** i solenoidi oltre il 32 delle Stern SAM (51–58, scheda ausiliaria) non arrivano allo script del tavolo, né con `SolCallback` né leggendo `Controller.Solenoid`. In Metallica la bocca del serpente non si apre mai; fermi anche martello, bara, bersagli in fila, paletto. Succede ai tavoli che non accendono i solenoidi modulati (Metallica dichiara `UseVPMModSol = 1` ma non chiama `vpmInit`): con `.SolMask(2) = 1` prima di `.Run`, in uno script `.vbs` accanto al tavolo, i solenoidi arrivano. Con la 3788 funzionano senza modifiche. Non segnalato a monte. Il passaggio dei solenoidi è stato riscritto a luglio 2026 (build 5359) e dopo la 5436 ci sono altre correzioni non ancora rilasciate.
- `BackglassOutput` / `ScoreViewOutput`: nella 5436 "dentro la finestra del tavolo" è 2; nella 3788 è 1, e 2 apre una finestra a parte grande quanto lo schermo.
- La 3788 scrive il log sull'uscita standard, non in `vpinball.log`. Provata e scartata sul PC dell'utente: glitch grafici sul tavolo, DMD incorporato sbiadito e in basso a sinistra con le stesse coordinate, e alla chiusura riempie `VPinballX.ini` dei suoi valori predefiniti.
- Per provare uno script modificato: una copia vera del `.vpx` con accanto un `.vbs` dallo stesso nome (un collegamento simbolico viene risolto e lo script ignorato). `debug.print` finisce nel log come `Script.Print`.
- PinMAME: usa `pinmame/roms` accanto al tavolo se esiste, altrimenti `PinMAMEPath` in `[Plugin.PinMAME]`. La ROM è cercata per nome.
- Plugin: su Linux ("standalone") PinMAME, ScoreView, FlexDMD, B2SLegacy, PUP, AltSound, Serum, WMP e VNI sono accesi se `Enable` manca; su Windows sono spenti e lo script si ferma con "Necessario oggetto: 'Controller'" (letto in `Settings_properties.inl`, non provato dal vivo). Il launcher su Windows scrive `Enable = 1` dove la voce manca o è vuota.
- Opzioni usate: `-ini <file>`, `-play <tavolo>`, `-PrefPath`, `-ExtractVBS`. Su Linux log e dati vanno in `$XDG_DATA_HOME/VPinballX/10.8/`.
- Riquadri incorporati (`BackglassOutput` / `ScoreViewOutput = 2`): senza posizione e dimensioni esplicite occupano tutta la finestra. `…WndX/WndY/Width/Height` sono in pixel dell'immagine disegnata, cioè schermo × `AAFactor`.
- Sincronizzazione: il predefinito `SyncMode = 3` (frame pacing) sul PC dell'utente dava 52 fps medi con cali a 25. `SyncMode = 1` (vsync) tiene la frequenza del monitor. `MaxFramerate` viene ignorato con il vsync acceso; vale con `SyncMode = 0`.
- Tasti predefiniti: F12 apre il menu interno (si naviga con Ctrl sinistro/destro e Shift sinistro/destro), ESC esce dal tavolo, F11 mostra gli fps, Shift = flipper, Invio = lancio, 1 = start, 5 = moneta.
- Vista per tavolo: file `.ini` con lo stesso nome del `.vpx`, sezione `[TableOverride]`, voci `ViewDT…`. Il menu F12 lo crea da solo quando si salva "per questo tavolo".
- Alcuni tavoli disegnano un proprio DMD nella vista desktop (Metallica: verde, in alto a sinistra), altri no (Addams Family). Il riquadro ScoreView è in più e non lo sostituisce.

**Libreria propria (rifà VPinFE 2.6.6, letto dal suo bytecode e dai sorgenti MIT)**

- Database VPS: `vpsdb.json` dal repository VirtualPinballSpreadsheet/vps-db (2645 tavoli al 2026-10-20), riscaricato solo se cambia `lastUpdated.json`. Copertine: indice `vpinmdb.json` di superhac/vpinmediadb per id VPS (1156 tavoli), file con md5; si scaricano bg, table, dmd in versione 1k più wheel, cab, flyer (niente video né audio). Patch: `hashes.json` di jsm174/vpx-standalone-scripts, sha256 dello script con a capo CRLF (GitHub serve i file con LF). Nessuno dei tre archivi ha una licenza dichiarata.
- VPinFE abbina il primo tavolo con somiglianza ≥ 0,8 su nome, marca e anno; il nostro prende il punteggio migliore (anno uguale, marca per prefisso). Sui 21 tavoli dell'utente coincide su 20; Indiana Jones (Stern 2008) VPinFE l'aveva abbinato per errore a "JP's Indiana Jones", noi a "Indiana Jones" (`NqaGRv8k`).
- Lettura del `.vpx` (OLE, crate `cfb`): testi in `TableInfo/…` come latin-1 senza zeri; script dopo il marcatore `CODE` in `GameStg/GameData`, UTF-8 saltando i byte non validi, a capo CRLF. Hash del file e dello script identici a quelli di VPinFE su tutti i 21 tavoli.
- Usa PinMAME chi chiama `LoadVPM` nello script; `cGameName` da solo non basta (JP's Space Cadet lo usa per i record).
- Avvio del tavolo: `VPinballX_BGFX -ini config/VPinballX.ini -play <tavolo.vpx>`, cartella di lavoro quella del motore, su Linux `XDG_DATA_HOME=config/vpx-data`. Statistiche nel `.info` come VPinFE: `StartCount` +1 e `LastRun` all'avvio, `RunTime` in minuti per eccesso all'uscita. La finestra non si minimizza: il motore va a schermo intero sopra e all'uscita la libreria è di nuovo visibile.
- Prove senza disturbare lo schermo dell'utente: `kwin_wayland --virtual --socket <nome>` dentro `dbus-run-session`, con `XDG_RUNTIME_DIR` corto (il percorso del socket ha un limite di 108 byte) e `spectacle -b -n -f -o` per la foto; per il motore `SDL_AUDIODRIVER=dummy`. Il motore vero è partito così con la configurazione del launcher (log "Startup done", ROM trovata in `pinmame/roms`).
- Test dell'interfaccia con `egui_kittest` (solo dipendenza di sviluppo), senza GPU; il test `visual` (ignorato) disegna con wgpu le schermate su dati veri: `VISUAL_DIR=<cartella dati> VISUAL_OUT=<cartella> cargo test --release visual -- --ignored`.

**Build e pubblicazione**

- Rust + eframe (egui) per la finestra, `rfd` per il selettore di cartelle; niente Tauri né web.
- eframe: dopo `ViewportCommand::Close` viene disegnato ancora un fotogramma, e `monitor_size` è vuoto nei primi due (su KDE Wayland arriva dal terzo, circa 0,1 s).
- `packaging/build-appimage.sh [cartella]` crea l'AppImage. Fallisce con "Text file busy" se l'AppImage di destinazione è in esecuzione.
- `.github/workflows/release.yml`: un tag `v*` costruisce AppImage ed exe e crea la release; avviato a mano costruisce soltanto.
- egui 0.31: alcuni simboli mancano nei font (escono come quadratini): "＋" e "✕" no, "+", "🗙", "🗁", "★", "☆", "▶", "⚠", "🔍" sì (vengono dai font di egui, tenuti come riserva dopo Barlow).
- egui 0.31: `ctx.set_fonts` vale dal fotogramma successivo, e usare una famiglia non ancora installata manda in panico il programma. Per questo il tema si installa nella creazione dell'app, prima del primo fotogramma (nei test: primo fotogramma senza disegno). `ui.allocate_rect` e `ui.put` non fanno avanzare il cursore come ci si aspetta (con `put` torna sotto il widget messo): per disegnare sopra un'immagine si usa `ui.interact` con il painter. `ScrollArea` ritaglia tutto ciò che esce dal suo rettangolo: le tiles hanno un margine interno (`GRID_PADDING`) perché sollevamento, ombra e bordo non vengano tagliati. I tasti consumati dalla libreria (Esc, frecce) non arrivano più a menu e tendine: con un menu o una tendina aperti la libreria non li legge.
- I test girano anche su Windows: nei confronti non scrivere percorsi con `/` fissi.

## Punti aperti

- **Motore fermo sulla 5436:** quando esce una release successiva va provata prima di spostare `pin` (Metallica senza lo script accanto: tiro al serpente). Altri tavoli Stern SAM con meccanismi sui solenoidi 51–58 possono avere lo stesso difetto; il launcher non li corregge.
- **Lancio della pallina diverso da Pocket Pinball** su Metallica (arriva in un altro punto): non indagato.
- **Windows mai provato dal vivo:** l'exe della 0.1.4 compila e passa i test su GitHub, ma nessuno l'ha avviato (controllato solo il nome dentro lo zip del motore: `VPinballX_BGFX64.exe`). Lì il motore scrive ancora impostazioni e log in `%APPDATA%\VPinballX`, fuori dalla cartella dati.
- **Libreria propria (0.2.0) non ancora provata dall'utente.** Provati: test automatici, `--cli` da zero in una cartella temporanea, finestra e motore dentro un KWin virtuale. Non provati: il selettore di file "Aggiungi tavoli" (portale di sistema) e una partita avviata cliccando nella finestra vera, con ritorno alla libreria.
- **Windows:** la libreria non è mai stata compilata per Windows in locale (manca il target); lo fa la CI. Niente installatore, solo l'exe.
- **DMD doppio:** sui tavoli con un DMD proprio, quello ScoreView è un duplicato. Spostando l'arancione, quello verde di Metallica risulta tagliato: causa non trovata (sospettato il riquadro del backglass a tutta finestra, `BackglassOutput = 0` per escluderlo).
- **Sfondo di Metallica:** l'utente ricorda uno sfondo dietro il tavolo che poi è sparito; con le impostazioni originali non ricompare e non si è capito da dove venisse.
- **Default grafici tarati su un solo PC**, con grafica integrata (Radeon 680M, 2560×1440). Il DMD in alto a sinistra copre il DMD proprio di Metallica, ma altri tavoli possono averlo altrove.
- **PUP pack, FlexDMD, backglass:** non provati (l'utente non ha ancora file di questo tipo).
- **Licenze dei dati** (VPS, vpinmediadb, patch): nessuna dichiarata; si scaricano sul PC dell'utente come faceva VPinFE. Da valutare un avviso agli autori prima di pubblicare la 0.2.0.
- **PUP pack e altri `.zip`:** "Aggiungi tavoli" mette ogni `.zip` tra le ROM.
