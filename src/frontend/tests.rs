//! Prove dell'interfaccia con egui_kittest: una cartella dati finta, clic e tasti simulati,
//! un motore finto che annota come viene avviato.

use super::*;
use egui::accesskit::Role;
use egui::Vec2;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::fs;

fn png(path: &Path, color: [u8; 3]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(320, 180, image::Rgb(color)).save(path).unwrap();
}

fn table(paths: &Paths, folder: &str, info: Option<&str>) -> PathBuf {
    let dir = paths.tables().join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{folder}.vpx")), b"x").unwrap();
    if let Some(info) = info {
        fs::write(dir.join(format!("{folder}.info")), info).unwrap();
    }
    dir
}

/// Cartella dati con tre tavoli: uno completo, uno preferito senza ROM, uno non riconosciuto.
/// Il motore è uno script che annota argomenti e ambiente.
fn fixture(name: &str) -> (PathBuf, Arc<Paths>, PathBuf) {
    let root = std::env::temp_dir().join(format!("pinball-aio-ui-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let paths = Arc::new(Paths { root: root.clone() });
    paths.create_all().unwrap();
    let afm = table(
        &paths,
        "Attack from Mars (Bally 1995)",
        Some(r#"{"Info":{"Title":"Attack from Mars","Manufacturer":"Bally","Year":1995,"Rom":"afm_113b","Authors":["jpsalas"]},"VPXFile":{"usesPinMAME":true},"User":{"StartCount":2,"RunTime":30,"LastRun":1700000000,"Favorite":0}}"#),
    );
    png(&afm.join("medias").join("bg.png"), [200, 40, 40]);
    fs::write(paths.pinmame().join("roms").join("afm_113b.zip"), b"x").unwrap();
    let fish = table(
        &paths,
        "Fish Tales (Williams 1992)",
        Some(r#"{"Info":{"Title":"Fish Tales","Manufacturer":"Williams","Year":1992,"Rom":"ft_l5"},"VPXFile":{"usesPinMAME":true},"User":{"Favorite":1}}"#),
    );
    png(&fish.join("medias").join("flyer.png"), [40, 40, 200]);
    table(&paths, "Unknown Thing (Nobody 1999)", None);
    // Il tavolo sconosciuto è già stato cercato: niente rete durante i test.
    library::set_tried(&paths, &["Unknown Thing (Nobody 1999)".to_string()]).unwrap();
    let engine = root.join("engine").join("fake-engine");
    fs::write(
        &engine,
        format!("#!/bin/sh\necho \"$@\" > '{0}/args'\necho \"$XDG_DATA_HOME\" >> '{0}/args'\npwd >> '{0}/args'\nsleep 1\n", root.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&engine, fs::Permissions::from_mode(0o755)).unwrap();
    }
    (root, paths, engine)
}

/// Primo fotogramma: come nel programma, il tema va installato prima di disegnare (i font
/// valgono dal fotogramma successivo).
fn start(ctx: &egui::Context, library: &mut Option<Library>, paths: &Arc<Paths>, engine: &Path) {
    theme::install(ctx);
    *library = Some(Library::new(ctx, paths.clone(), engine.to_path_buf()));
    ctx.request_repaint();
}

fn harness(paths: Arc<Paths>, engine: PathBuf) -> Harness<'static, Option<Library>> {
    Harness::builder().with_size(Vec2::new(1400.0, 900.0)).build_state(
        move |ctx, library: &mut Option<Library>| match library {
            Some(library) => library.show(ctx),
            None => start(ctx, library, &paths, &engine),
        },
        None,
    )
}

/// Lascia lavorare i thread in background (immagini, partita) e ridisegna.
fn settle(harness: &mut Harness<'_, Option<Library>>, until: impl Fn(&Library) -> bool) {
    for _ in 0..100 {
        harness.run_steps(2);
        if until(harness.state().as_ref().unwrap()) {
            harness.run_steps(2);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("condizione non raggiunta");
}

fn type_text(harness: &mut Harness<'_, Option<Library>>, text: &str) {
    harness.input_mut().events.push(egui::Event::Text(text.into()));
    harness.run_steps(2);
}

#[test]
fn the_library_shows_filters_and_searches_tables() {
    let (root, paths, engine) = fixture("browse");
    let mut harness = harness(paths, engine);
    harness.run_steps(3);
    for title in ["Attack from Mars", "Fish Tales", "Unknown Thing"] {
        harness.get_by_role_and_label(Role::Button, title);
    }
    harness.get_by_label("3 tavoli");
    harness.get_by_label("⚠  Da sistemare (2)").click();
    harness.run_steps(2);
    assert!(harness.query_by_role_and_label(Role::Button, "Attack from Mars").is_none());
    harness.get_by_label("★  Preferiti").click();
    harness.run_steps(2);
    harness.get_by_label("1 tavolo");
    harness.get_by_label("Tutti").click();
    harness.run_steps(2);
    type_text(&mut harness, "fish");
    assert_eq!(harness.state().as_ref().unwrap().search, "fish");
    assert!(harness.query_by_role_and_label(Role::Button, "Attack from Mars").is_none());
    harness.get_by_role_and_label(Role::Button, "Fish Tales");
    // Dalla ricerca, freccia giù seleziona il primo risultato e Esc libera la griglia.
    harness.press_key(Key::ArrowDown);
    harness.run_steps(2);
    assert_eq!(harness.state().as_ref().unwrap().selected.as_deref(), Some("Fish Tales (Williams 1992)"));
    harness.press_key(Key::Escape);
    harness.run_steps(2);
    assert_eq!(harness.state().as_ref().unwrap().search, "");
    harness.get_by_role_and_label(Role::Button, "Attack from Mars");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_detail_panel_warns_and_toggles_favorites() {
    let (root, paths, engine) = fixture("detail");
    let mut harness = harness(paths.clone(), engine);
    harness.run_steps(3);
    harness.get_by_role_and_label(Role::Button, "Fish Tales").click();
    harness.run_steps(2);
    harness.get_by_label_contains("Manca la ROM ft_l5.zip");
    harness.get_by_label("★  Nei preferiti").click();
    harness.run_steps(2);
    harness.get_by_label("☆  Aggiungi ai preferiti");
    let info = fs::read_to_string(paths.tables().join("Fish Tales (Williams 1992)").join("Fish Tales (Williams 1992).info")).unwrap();
    assert!(info.contains("\"Favorite\": 0"));

    harness.get_by_role_and_label(Role::Button, "Unknown Thing").click();
    harness.run_steps(2);
    harness.get_by_label_contains("Tavolo non riconosciuto");
    assert!(harness.query_by_label("☆  Aggiungi ai preferiti").is_none());
    harness.press_key(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().as_ref().unwrap().selected.is_none());
    // Con il menu aperto, Esc chiude il menu e non tocca la selezione.
    harness.get_by_role_and_label(Role::Button, "Fish Tales").click();
    harness.get_by_label("☰").click();
    harness.run_steps(2);
    harness.get_by_label("Ricarica la libreria");
    harness.press_key(Key::Escape);
    harness.run_steps(3);
    assert!(harness.query_by_label("Ricarica la libreria").is_none());
    assert!(harness.state().as_ref().unwrap().selected.is_some());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn playing_launches_the_engine_and_records_the_game() {
    let (root, paths, engine) = fixture("play");
    let mut harness = harness(paths.clone(), engine.clone());
    harness.run_steps(3);
    // All'avvio è selezionato l'ultimo tavolo giocato; Invio lo avvia.
    assert_eq!(harness.state().as_ref().unwrap().selected.as_deref(), Some("Attack from Mars (Bally 1995)"));
    harness.press_key(Key::ArrowRight);
    harness.press_key(Key::ArrowLeft);
    harness.run_steps(2);
    harness.press_key(Key::Enter);
    harness.run_steps(2);
    harness.get_by_label("In gioco");
    settle(&mut harness, |l| l.playing.is_none());
    let args = fs::read_to_string(root.join("args")).unwrap();
    let dir = paths.tables().join("Attack from Mars (Bally 1995)");
    let expected = format!(
        "-ini {} -play {}\n{}\n{}\n",
        paths.vpx_ini().display(),
        dir.join("Attack from Mars (Bally 1995).vpx").display(),
        paths.vpx_data().display(),
        engine.parent().unwrap().display()
    );
    assert_eq!(args, expected);
    let t = catalog::load(&dir).unwrap();
    assert_eq!((t.start_count, t.run_time), (3, 31));
    assert!(t.last_run.unwrap() > 1700000000);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dropped_roms_are_copied_into_the_roms_folder() {
    let (root, paths, engine) = fixture("drop");
    let rom = root.join("ft_l5.zip");
    fs::write(&rom, b"rom").unwrap();
    let mut harness = harness(paths.clone(), engine);
    harness.run_steps(3);
    harness.input_mut().dropped_files.push(egui::DroppedFile { path: Some(rom), ..Default::default() });
    settle(&mut harness, |l| l.task.is_none());
    assert!(paths.pinmame().join("roms").join("ft_l5.zip").is_file());
    harness.get_by_label("1 ROM aggiunta");
    // Con la ROM al suo posto il tavolo non è più da sistemare.
    harness.get_by_label("⚠  Da sistemare (1)");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn covers_are_loaded_in_the_background() {
    let (root, paths, engine) = fixture("covers");
    let mut harness = harness(paths.clone(), engine);
    let tables = paths.tables();
    let cover = tables.join("Attack from Mars (Bally 1995)").join("medias").join("bg.png");
    settle(&mut harness, |l| l.images.ready(&cover) == Some(true));
    // Il flyer fa da copertina a chi non ha il backglass; chi non ha immagini non ne chiede.
    let flyer = tables.join("Fish Tales (Williams 1992)").join("medias").join("flyer.png");
    settle(&mut harness, |l| l.images.ready(&flyer) == Some(true));
    let library = harness.state().as_ref().unwrap();
    assert!(library.entry("Unknown Thing (Nobody 1999)").unwrap().table.cover.is_none());
    assert!(fs::read_dir(paths.thumbs()).unwrap().count() >= 2);
    fs::remove_dir_all(root).unwrap();
}

mod visual {
    use super::*;

    /// Immagini della libreria con dati veri, da guardare a occhio:
    /// `VISUAL_DIR=<cartella dati> VISUAL_OUT=<cartella> cargo test visual -- --ignored`.
    #[test]
    #[ignore]
    fn screenshots() {
        let root = PathBuf::from(std::env::var("VISUAL_DIR").unwrap());
        let out = PathBuf::from(std::env::var("VISUAL_OUT").unwrap());
        let paths = Arc::new(Paths { root });
        metadata::mark_pinmame(&paths, &|_| {});
        let (w, h) = (std::env::var("VISUAL_W").map_or(1600.0, |v| v.parse().unwrap()), std::env::var("VISUAL_H").map_or(900.0, |v| v.parse().unwrap()));
        let mut harness = Harness::builder().with_size(Vec2::new(w, h)).wgpu().build_state(
            move |ctx, library: &mut Option<Library>| match library {
                Some(library) => library.show(ctx),
                None => start(ctx, library, &paths, Path::new("/bin/true")),
            },
            None,
        );
        let shot = |harness: &mut Harness<'_, Option<Library>>, name: &str| {
            for _ in 0..60 {
                harness.run_steps(2);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            harness.render().unwrap().save(out.join(format!("{name}.png"))).unwrap();
        };
        if let Some(library) = harness.state_mut().as_mut() {
            library.selected = None;
            library.reveal = false;
        }
        shot(&mut harness, "1-griglia");
        harness.get_by_role_and_label(egui::accesskit::Role::Button, "Fish Tales").hover();
        shot(&mut harness, "1b-passaggio");
        harness.get_by_label("☰").click();
        shot(&mut harness, "1c-menu");
        harness.press_key(Key::Escape);
        harness.run_steps(2);
        harness.get_by_role(egui::accesskit::Role::ComboBox).click();
        shot(&mut harness, "1d-ordina");
        harness.press_key(Key::Escape);
        harness.state_mut().as_mut().unwrap().notify("2 tavoli aggiunti · 1 ROM aggiunta");
        shot(&mut harness, "1e-avviso");
        harness.get_by_role_and_label(egui::accesskit::Role::Button, "Metallica (Premium Monsters)").click();
        shot(&mut harness, "2-dettaglio");
        harness.input_mut().events.push(egui::Event::Text("stern".into()));
        shot(&mut harness, "3-ricerca");
        if let Ok(db) = std::env::var("VPSDB") {
            let games = serde_json::from_slice(&std::fs::read(db).unwrap()).unwrap();
            let library = harness.state_mut().as_mut().unwrap();
            library.database = Some(Arc::new(Mutex::new(DbState::Ready(Arc::new(Database::new(games))))));
            library.matcher = Some(Matcher {
                folder: "Indiana Jones (Stern 2008)".into(),
                query: "Indiana Jones".into(),
                focus: true,
                results: Vec::new(),
                results_for: None,
            });
            shot(&mut harness, "4-abbinamento");
            harness.state_mut().as_mut().unwrap().matcher = None;
        }
        let library = harness.state_mut().as_mut().unwrap();
        library.search.clear();
        library.playing = Some(Playing { title: "Metallica (Premium Monsters)".into(), folder: String::new(), done: Arc::new(AtomicBool::new(false)) });
        shot(&mut harness, "5-in-gioco");
        let library = harness.state_mut().as_mut().unwrap();
        library.playing = None;
        library.entries.clear();
        library.view_key = None;
        library.selected = None;
        shot(&mut harness, "6-vuota");
    }
}
