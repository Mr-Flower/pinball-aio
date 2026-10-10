# Pinball AIO

Launcher per Windows e Linux che scarica, tiene aggiornati e avvia
[VPinballX](https://github.com/vpinball/vpinball) (il motore) e
[VPinFE](https://github.com/superhac/vpinfe) (la libreria dei tavoli), già configurati tra loro.

## Uso

1. Scarica `PinballAIO-x86_64.AppImage` (Linux) o `PinballAIO-x64.exe` (Windows) dalle release.
2. Avvialo. Al primo avvio chiede dove mettere la cartella dei dati, poi scarica circa
   500 MB: serve internet.
3. Copia i file `.vpx` (con l'eventuale `.directb2s` dallo stesso nome) in `tables/` e le ROM
   in `pinmame/roms/`, poi riavvia il launcher: crea una cartella per tavolo, scarica schede e
   copertine, applica le patch agli script e apre la libreria di VPinFE.

Se non c'è nessun tavolo, al posto della libreria si apre nel browser la pagina di gestione di
VPinFE, da cui si possono importare i file trascinandoli. La stessa pagina
(`http://localhost:8001` mentre VPinFE è aperto) serve ad abbinare a mano i tavoli che VPinFE
non riconosce dal nome: finché non sono abbinati non compaiono nella libreria.

A ogni avvio il launcher controlla se ci sono versioni nuove dei due programmi e le installa.
Senza rete parte con quelle già installate.

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
  config/                         VPinballX.ini e configurazione di VPinFE
  engine/                         i due programmi scaricati
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

Dopo un aggiornamento la versione precedente resta in `engine/vpinball.old` e `engine/vpinfe.old`.

## Compilare

```
cargo build --release
packaging/build-appimage.sh .     # solo Linux
```

`pinball-aio --cli --no-launch` aggiorna e configura senza finestra e senza avviare VPinFE.

## Licenze e note legali

- **Questo launcher** è sotto licenza MIT (vedi `LICENSE`), è gratuito e non contiene codice
  né programmi di VPinballX, PinMAME o VPinFE: li scarica non modificati dalle loro release
  ufficiali su GitHub, sul computer dell'utente, e li avvia come programmi separati.
- **VPinballX** è distribuito con licenza GPLv3+ e, per le parti più vecchie, con una licenza
  di tipo "old MAME" che ne vieta la vendita e l'uso commerciale. **PinMAME** (incluso nel
  motore) ha licenza BSD-3-Clause e "old MAME", anch'essa non commerciale. **VPinFE** è MIT.
  Chi usa questo launcher usa quei programmi alle condizioni delle loro licenze: in
  particolare non va venduto né inserito in prodotti commerciali insieme a loro.
- **L'icona** è quella di Visual Pinball X, usata per indicare il motore che il launcher
  avvia. Non è sotto licenza MIT: resta del team di Visual Pinball, alle condizioni della
  loro licenza (vedi `assets/README.md`).
- **Tavoli e ROM** non sono inclusi e non vengono scaricati. Sono opere protette da diritto
  d'autore: usa solo file che hai il diritto di usare.
- **Schede e copertine** dei tavoli sono scaricate da VPinFE dai suoi archivi pubblici (VPS
  database e vpinmediadb), non da questo progetto, e appartengono ai rispettivi autori.
- **Librerie incluse negli eseguibili:** crate Rust open source con licenze permissive (MIT,
  Apache-2.0, BSD, ISC, Zlib, MPL-2.0 e simili; elenco con `cargo metadata`) e i font di egui
  (OFL-1.1 e Ubuntu Font Licence 1.0).
- Progetto indipendente, non affiliato né approvato da Visual Pinball, PinMAME o VPinFE; i
  loro nomi e l'icona sono usati solo per indicare i programmi che il launcher scarica.
