# Pinball AIO

Launcher per Windows e Linux che scarica, tiene aggiornati e avvia **VPinballX** (il motore) e **VPinFE** (la libreria dei tavoli), già configurati tra loro. Modello di riferimento: Pocket Pinball / l'app VPX per Android, cioè "installa e gioca".

- Repository: https://github.com/Mr-Flower/pinball-aio (pubblico), ultima release `v0.1.4`.
- Il valore del progetto è solo la semplicità: nessun percorso da impostare. Multi-schermo, cabinet e personalizzazione spinta sono fuori scopo.

## Decisioni prese (2026-10-10)

- **Frontend: VPinFE**, non uno proprio. Condizione dell'utente: gli aggiornamenti li installa il launcher, in automatico a ogni avvio.
- **VPinballX fermo su una build provata** (deciso dopo, stesso giorno): solo VPinFE segue l'ultima release. La build del motore è `pin` in `src/update.rs` (ora `v10.8.1-5436-af26b2d93`) e si cambia a mano dopo averla provata, con una nuova versione del launcher.
- **Niente binari di terzi nei pacchetti.** Il launcher scarica le release ufficiali da GitHub al primo avvio (serve internet). Deve restare gratuito: VPinball e PinMAME hanno parti sotto licenza "old MAME" non commerciale. ROM e tavoli non si includono né si scaricano mai.
- **Una cartella per tavolo** (lo richiede VPinFE), più una cartella `pinmame` condivisa accanto a `tables`.
- **Cartella dati scelta dall'utente al primo avvio**; tutto deve stare lì dentro, così cancellarla fa pulizia.
- **Uso previsto:** un solo monitor in orizzontale, vista desktop, DMD dentro la finestra del tavolo.
- **Icona:** la pallina di VPinballX, su richiesta dell'utente. Non è MIT: resta sotto la licenza di Visual Pinball (vedi `assets/README.md`).
- **Commit senza coautori:** niente righe di attribuzione a Claude nei messaggi di commit.

## Come funziona

A ogni avvio (`src/main.rs`, funzione `prepare`):

1. `paths.rs`: trova la cartella dati. Ordine: variabile `PINBALL_AIO_DIR`, cartella `PinballData` accanto all'eseguibile, percorso salvato in `~/.config/pinball-aio/location.txt`. Se manca o è stata cancellata, la finestra la chiede.
2. `update.rs`: per ciascuno dei due programmi legge da GitHub la release voluta (quella fissata per VPinballX, l'ultima per VPinFE), e se il tag è diverso da quello in `engine/versions.json` scarica, verifica l'sha256, estrae e scambia le cartelle. La versione precedente resta in `engine/<nome>.old`. Senza rete parte con ciò che è installato.
3. `config.rs`: crea `config/VPinballX.ini` solo se manca; riscrive sempre in `vpinfe.ini` i quattro percorsi `vpxbinpath`, `tablerootdir`, `vpxinipath`, `globalinioverride`. Se il motore cambia tra build con numerazione diversa di `BackglassOutput` / `ScoreViewOutput`, scambia 1 e 2 in `VPinballX.ini`.
4. `library.rs`: sposta i `.vpx` sciolti in `tables/` in una cartella per tavolo (nome tagliato a "Titolo (Marca Anno)"), con i file dallo stesso nome. Se ci sono tavoli nuovi (appena spostati, oppure cartelle senza `.info` non ancora elencate in `config/tavoli-senza-scheda.txt`) lancia `vpinfe --buildmeta` (schede `.info` e copertine) e `vpinfe --vpxpatch` (patch degli script).
5. Avvia VPinFE. Se non c'è nessun tavolo lo avvia `--headless` e apre nel browser la sua pagina di gestione, con un pulsante per passare poi alla libreria.

`pinball-aio --cli --no-launch` fa i passi 1-4 senza finestra: utile per provare.

## Struttura della cartella dati

```
<cartella scelta>/            (sul PC dell'utente: ~/Pinball)
  tables/<Tavolo (Marca Anno)>/   .vpx, .directb2s, .info, medias/, pupvideos/, pinmame/ (facoltativa)
  pinmame/roms|nvram/             ROM condivise (zip non estratti)
  config/VPinballX.ini            impostazioni del motore, dell'utente dopo la creazione
  config/vpinfe/                  vpinfe.ini, temi, log di VPinFE
  config/vpx-data/                log e dati del motore (solo Linux)
  config/tavoli-senza-scheda.txt  tavoli che VPinFE non ha riconosciuto (c'è solo se ce ne sono)
  engine/vpinball|vpinfe/         più <nome>.old e versions.json
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

**VPinFE 2.6.6**

- Pacchetto completo `vpinfe-<tag>-linux-x64.zip` / `-win-x64.zip` (con Chromium). Su Windows lo zip contiene `vpinfe/vpinfe.exe` e un `vpinfe.bat` che fa solo `cd` nella sua cartella e lo avvia: il launcher avvia direttamente l'exe da quella cartella (con `cmd /C` i percorsi con spazi si rompono).
- Vede solo cartelle con un `.vpx` e un `.info`. I tavoli non riconosciuti dal nome vanno abbinati a mano in "Unmatched Tables".
- `--configdir DIR`, `--headless`, `--buildmeta`, `--vpxpatch`, `--gamepadtest`. Pagina di gestione su `http://localhost:8001`.
- `globalinioverride` è il file passato al motore con `-ini`; `vpxlaunchenv` imposta variabili d'ambiente per il motore (usato per `XDG_DATA_HOME`).
- Con SIGTERM chiude anche il processo figlio; con un kill secco il figlio resta vivo e tiene occupate le porte.

**Build e pubblicazione**

- Rust + eframe (egui) per la finestra, `rfd` per il selettore di cartelle; niente Tauri né web.
- eframe: dopo `ViewportCommand::Close` viene disegnato ancora un fotogramma, e `monitor_size` è vuoto nei primi due (su KDE Wayland arriva dal terzo, circa 0,1 s).
- `packaging/build-appimage.sh [cartella]` crea l'AppImage. Fallisce con "Text file busy" se l'AppImage di destinazione è in esecuzione.
- `.github/workflows/release.yml`: un tag `v*` costruisce AppImage ed exe e crea la release; avviato a mano costruisce soltanto.
- I test girano anche su Windows: nei confronti non scrivere percorsi con `/` fissi.

## Punti aperti

- **Motore fermo sulla 5436:** quando esce una release successiva va provata prima di spostare `pin` (Metallica senza lo script accanto: tiro al serpente). Altri tavoli Stern SAM con meccanismi sui solenoidi 51–58 possono avere lo stesso difetto; il launcher non li corregge.
- **Lancio della pallina diverso da Pocket Pinball** su Metallica (arriva in un altro punto): non indagato.
- **Windows mai provato dal vivo:** l'exe compila e passa i test su GitHub, ma nessuno l'ha avviato (controllati solo i nomi dentro gli zip: `VPinballX_BGFX64.exe`, `vpinfe/vpinfe.exe`). Lì il motore scrive ancora impostazioni e log in `%APPDATA%\VPinballX`, fuori dalla cartella dati.
- **Installazione da zero** provata il 2026-10-13 solo con `--cli` in una cartella temporanea (scarica, configura, prepara un tavolo; VPinFE `--headless` risponde sulla 8001). Dalla finestra, il percorso "nessun tavolo → pagina di gestione → apri la libreria" non è stato visto dall'inizio alla fine.
- **DMD doppio:** sui tavoli con un DMD proprio, quello ScoreView è un duplicato. Spostando l'arancione, quello verde di Metallica risulta tagliato: causa non trovata (sospettato il riquadro del backglass a tutta finestra, `BackglassOutput = 0` per escluderlo).
- **Sfondo di Metallica:** l'utente ricorda uno sfondo dietro il tavolo che poi è sparito; con le impostazioni originali non ricompare e non si è capito da dove venisse.
- **Default grafici tarati su un solo PC**, con grafica integrata (Radeon 680M, 2560×1440). Il DMD in alto a sinistra copre il DMD proprio di Metallica, ma altri tavoli possono averlo altrove.
- **PUP pack, FlexDMD, backglass:** non provati (l'utente non ha ancora file di questo tipo).
- **VPinFE che si aggiorna da solo** dalla sua pagina: il launcher non se ne accorge e riscarica la stessa versione una volta.
- Attesa durante `--buildmeta` senza percentuale (circa 15 secondi a tavolo per le copertine).
