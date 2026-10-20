# Pinball AIO

Launcher per Windows e Linux che scarica e tiene aggiornato
[VPinballX](https://github.com/vpinball/vpinball) (il motore) e mostra i tavoli in una
libreria a tiles, con le copertine, da cui si gioca con un clic.

## Uso

1. Scarica `PinballAIO-x86_64.AppImage` (Linux) o `PinballAIO-x64.exe` (Windows) dalle release.
2. Avvialo. Al primo avvio chiede dove mettere la cartella dei dati, poi scarica il motore
   (circa 350 MB): serve internet.
3. Con **Aggiungi tavoli** (o trascinando i file nella finestra) aggiungi i tavoli `.vpx`, i
   loro backglass `.directb2s` e le ROM `.zip`. Il launcher copia i file al loro posto, crea
   una cartella per tavolo, riconosce il tavolo, scarica scheda e copertine e, su Linux,
   applica le correzioni agli script.
4. Scegli un tavolo e premi **Gioca** (o Invio, o doppio clic). Esc esce dal tavolo e riporta
   alla libreria.

Nella libreria si cerca scrivendo, si filtra per preferiti, si ordina per nome, anno, marca o
partite, e si naviga con le frecce. F11 passa allo schermo intero. I tavoli non riconosciuti
dal nome e quelli senza ROM sono segnati e raccolti in **Da sistemare**; un tavolo non
riconosciuto si abbina a mano cercandolo nell'elenco, e resta comunque giocabile.

A ogni avvio il launcher controlla il motore (fermo su una versione provata) e prepara i
tavoli copiati nel frattempo in `tables/`. Senza rete parte con ciò che è già installato.

## Cartelle

Tutto sta nella cartella scelta al primo avvio (proposta: `Pinball` nella cartella utente):

```
Pinball/
  tables/                         una cartella per tavolo
    Hurricane (Williams 1991)/
      Hurricane.vpx
      Hurricane.directb2s
      pupvideos/                  PUP pack di questo tavolo
      pinmame/roms/               ROM di questo tavolo (facoltativa)
  pinmame/roms/                   ROM condivise: puoi metterle tutte qui
  config/                         VPinballX.ini, cache del database dei tavoli e miniature
  engine/                         il motore scaricato
```

Le ROM si possono mettere tutte in `Pinball/pinmame/roms`, come zip non estratti: ogni tavolo
cerca la sua per nome. Un tavolo che ha una propria `pinmame/roms` usa solo quella.

Per fare pulizia basta cancellare quella cartella: al prossimo avvio il launcher chiede di
nuovo dove metterla. Fuori dalla cartella restano solo:

- un file di testo con il percorso scelto (`~/.config/pinball-aio/location.txt`, su Windows
  `%APPDATA%\pinball-aio\location.txt`);
- su Windows, le impostazioni e i log che VPinballX scrive in `%APPDATA%\VPinballX`.

La variabile `PINBALL_AIO_DIR` o una cartella `PinballData` accanto all'eseguibile (modalità
portatile) hanno la precedenza sulla scelta fatta.

Dopo un aggiornamento la versione precedente del motore resta in `engine/vpinball.old`.

Le versioni fino alla 0.1.4 usavano VPinFE come libreria: alla prima esecuzione della nuova
versione VPinFE viene cancellato (`engine/vpinfe`, `config/vpinfe`). Le schede dei tavoli
(`.info`, nello stesso formato) e le copertine restano e vengono usate così come sono.

## Compilare

```
cargo build --release
packaging/build-appimage.sh .     # solo Linux
```

`pinball-aio --cli` aggiorna, configura e prepara i tavoli senza finestra.

## Licenze e note legali

- **Questo launcher** è sotto licenza MIT (vedi `LICENSE`), è gratuito e non contiene codice
  né programmi di VPinballX o PinMAME: scarica il motore non modificato dalle sue release
  ufficiali su GitHub, sul computer dell'utente, e lo avvia come programma separato. La
  preparazione dei tavoli (riconoscimento, schede, copertine, patch) segue il funzionamento
  di [VPinFE](https://github.com/superhac/vpinfe) (MIT, © 2024 superhac), riscritto in Rust.
- **VPinballX** è distribuito con licenza GPLv3+ e, per le parti più vecchie, con una licenza
  di tipo "old MAME" che ne vieta la vendita e l'uso commerciale. **PinMAME** (incluso nel
  motore) ha licenza BSD-3-Clause e "old MAME", anch'essa non commerciale.
  Chi usa questo launcher usa quei programmi alle condizioni delle loro licenze: in
  particolare non va venduto né inserito in prodotti commerciali insieme a loro.
- **L'icona** è quella di Visual Pinball X, usata per indicare il motore che il launcher
  avvia. Non è sotto licenza MIT: resta del team di Visual Pinball, alle condizioni della
  loro licenza (vedi `assets/README.md`).
- **Tavoli e ROM** non sono inclusi e non vengono scaricati. Sono opere protette da diritto
  d'autore: usa solo file che hai il diritto di usare.
- **Schede, copertine e patch degli script** sono scaricate al momento, sul computer
  dell'utente, dagli archivi pubblici su GitHub della
  [Virtual Pinball Spreadsheet](https://github.com/VirtualPinballSpreadsheet/vps-db),
  di [VPinMediaDB](https://github.com/superhac/vpinmediadb) e di
  [vpx-standalone-scripts](https://github.com/jsm174/vpx-standalone-scripts). Non sono incluse
  nel launcher e appartengono ai rispettivi autori.
- **Librerie incluse negli eseguibili:** crate Rust open source con licenze permissive (MIT,
  Apache-2.0, BSD, ISC, Zlib, MPL-2.0 e simili; elenco con `cargo metadata`) i font di egui
  (OFL-1.1 e Ubuntu Font Licence 1.0) e i font Barlow e Bebas Neue (OFL-1.1, vedi
  `assets/README.md`).
- Progetto indipendente, non affiliato né approvato da Visual Pinball, PinMAME, VPinFE o dagli
  archivi citati; i loro nomi e l'icona sono usati solo per indicare i programmi e i dati che
  il launcher scarica.
