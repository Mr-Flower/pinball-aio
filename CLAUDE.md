# Launcher Visual Pinball "installa e gioca"

## Obiettivo

App desktop per Windows e Linux (AppImage) che permetta di giocare ai tavoli Visual Pinball X senza configurazione, sul modello dell'app VPX per Android: si importano i file (`.vpx`, `.directb2s`, ROM `.zip`), compaiono in una libreria con copertina, si clicca e si gioca.

Non è un motore di flipper: è un involucro attorno a **VPinballX standalone**, che fa tutta la simulazione. L'app importa i file, li mette in ordine e lancia il motore.

Il valore rispetto ai frontend esistenti è solo la semplicità: motore incluso, nessun percorso da impostare, import con un click. Ogni scelta va valutata su questo criterio; multi-schermo, cabinet e personalizzazione spinta sono fuori scopo.

## Cosa deve fare

1. Scaricare e aggiornare da sola VPinballX e VPinFE, così l'utente non installa altro.
2. Import con pulsante o drag & drop di `.vpx`, `.directb2s` e ROM `.zip`.
3. Creare da sola la struttura delle cartelle (vedi sotto).
4. Abbinare automaticamente ogni ROM al suo tavolo.
5. Applicare automaticamente le patch agli script dei tavoli che ne hanno bisogno.
6. Mostrare una griglia di tavoli con copertina e lanciare il tavolo scelto.

## Decisioni prese (2026-10-10)

- **Frontend: VPinFE**, non uno proprio. L'app è un piccolo launcher (`pinball-aio`) che scarica, aggiorna e avvia VPinballX e VPinFE già configurati. Condizione posta dall'utente: gli aggiornamenti dei due programmi li installa il launcher, in automatico a ogni avvio.
- **Motore non incluso nei pacchetti:** il launcher scarica le release ufficiali da GitHub al primo avvio (serve internet). Così non si ridistribuiscono binari di terzi; il launcher deve restare gratuito (licenze "old MAME" non commerciali di VPinball e PinMAME).
- **Una cartella per tavolo** (standard VPinball, richiesto da VPinFE), più una cartella `pinmame` condivisa accanto a `tables` per le ROM.
- **Uso previsto:** un solo monitor in orizzontale, backglass e DMD incorporati nella finestra del tavolo.

## Struttura delle cartelle

```
<cartella scelta al primo avvio>/  <- ricordata in ~/.config/pinball-aio/location.txt; se cancellata viene richiesta
  tables/<Tavolo (Marca Anno)>/ <- .vpx, .directb2s, .info, pupvideos/, pinmame/ (facoltativa)
  pinmame/roms|nvram/           <- ROM condivise: PinMAMEPath in [Plugin.PinMAME]
  config/VPinballX.ini          <- creato una volta sola, poi è dell'utente
  config/vpinfe/vpinfe.ini      <- percorsi riscritti a ogni avvio
  engine/vpinball|vpinfe/       <- più <nome>.old (versione precedente) e versions.json
```

Tutto deve stare in quella cartella, così cancellarla fa pulizia: su Linux anche i dati del motore (`vpxlaunchenv = XDG_DATA_HOME=…/config/vpx-data`); su Windows il motore scrive ancora in `%APPDATA%\VPinballX`. `PINBALL_AIO_DIR` e `PinballData` accanto all'eseguibile hanno la precedenza.

Il motore usa `PinMAMEPath` solo se il tavolo non ha una sua `pinmame/roms`.

## Stack

Rust + eframe (egui) per la finestra di avanzamento; niente Tauri né web. AppImage con `packaging/build-appimage.sh`, exe Windows da `cargo build`; `.github/workflows/release.yml` pubblica entrambi a ogni tag `v*`. `--cli --no-launch` per provare senza finestra.

## Parti del lavoro e difficoltà

| Parte | Difficoltà | Note |
|---|---|---|
| Interfaccia libreria + avvio tavolo | Facile | Lista, immagini, avvio di un processo |
| Import e organizzazione dei file | Facile | Copia e rinomina |
| Abbinare ROM al tavolo | Media | Leggere il nome ROM dallo script dentro il `.vpx` (`cGameName`) |
| Patch automatiche degli script | Media | Esiste un repository pubblico di patch per standalone; è quello che fa VPinFE |
| Copertine e metadati | Media | Serve una fonte online oppure uno screenshot generato |
| Pacchetti AppImage + exe | Media | Il grosso è impacchettare il motore con le sue librerie |
| Tavoli "difficili" (PUP pack, FlexDMD, script particolari) | Difficile | Alcuni non partiranno mai al primo colpo; accettarlo e segnalarlo bene all'utente |

## Primo passo

Prototipo **solo Linux** con i tavoli reali dell'utente: import, struttura cartelle, avvio. Misurare quanti tavoli partono senza interventi manuali: quel numero dice se l'idea regge. Windows e la rifinitura vengono dopo.

## Fatti utili

- **Motore:** VPinballX, https://github.com/vpinball/vpinball/releases
  - 10.8.0 è l'ultima stabile; su Windows si installa con `VPX8setup.exe` e usa VPinMAME esterno con le ROM in una cartella globale.
  - 10.8.1 è in pre-release, ha build Linux x64 (`VPinballX_BGFX`) e PinMAME integrato. È la base per questo progetto.
  - Esempio di avvio preso da un installer Lutris: `VPinballX_BGFX -EnableTrueFullscreen` con un `config.ini` e il percorso del `.vpx`. Le opzioni esatte vanno verificate con l'help del binario.
- **Perché servono le patch:** su Linux il VBScript di Windows è sostituito da un'implementazione Wine, e molti tavoli non partono senza uno script `.vbs` corretto.
- **VPinFE** (https://github.com/superhac/vpinfe): frontend esistente per Linux/Mac/Windows che fa già gran parte di questo (libreria, artwork, patch automatiche), ma richiede di impostare percorsi, abbinare i tavoli a file `.info` e mappare il gamepad. Utile come riferimento per patch, metadati e struttura cartelle.
- **vpxtool** (https://lib.rs/crates/vpxtool): utility Rust a riga di comando che indicizza e apre i file VPX; da qui si ricava come estrarre lo script da un tavolo.
- **Ambiente di sviluppo:** CachyOS (Arch). Per provare il motore senza impacchettarlo: pacchetto AUR `vpinball`.

## Da verificare

- Licenze di VPinball e PinMAME per la ridistribuzione dei binari dentro l'app. Le ROM non vanno mai incluse.
- Percorso ROM globale alternativo su Linux (forse `~/.pinmame/roms/`).
- Se la build Windows 10.8.1 funziona con lo stesso layout a cartelle e PinMAME integrato, senza registrare componenti COM.
- Nome e posizione del repository delle patch per standalone e come VPinFE sceglie la patch giusta per un tavolo.
- Fonte per copertine e metadati dei tavoli.
