//! Immagini della libreria. Le copertine sono grandi (fino a 3840×2160): se ne fa una copia
//! ridotta in `config/cache/thumbs`, la si carica in background e la si consegna alla
//! finestra come texture. Così la griglia resta fluida anche con molti tavoli.

use crate::{files, hash};
use eframe::egui;
use image::imageops::FilterType;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

/// Lato maggiore delle copie ridotte, in pixel.
pub const TILE: u32 = 512;
pub const LARGE: u32 = 1280;

/// Texture tenute in memoria al massimo: oltre, si scartano quelle non mostrate da più tempo
/// (si ricaricano dalla cache se tornano visibili).
const KEEP: usize = 240;

/// Nome del file in cache: cambia se cambia l'immagine di partenza o la dimensione.
fn cache_name(source: &Path, max: u32) -> Option<String> {
    let meta = fs::metadata(source).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let key = hash::fnv1a(&[
        source.as_os_str().as_encoded_bytes(),
        &meta.len().to_le_bytes(),
        &modified.to_le_bytes(),
        &max.to_le_bytes(),
    ]);
    Some(format!("{key:016x}.png"))
}

/// Copia ridotta dell'immagine, presa dalla cache o creata e salvata lì.
fn thumbnail(cache: &Path, source: &Path, max: u32) -> Option<image::DynamicImage> {
    let cached = cache_name(source, max).map(|name| cache.join(name));
    if let Some(image) = cached.as_ref().and_then(|c| image::open(c).ok()) {
        return Some(image);
    }
    let original = image::open(source).ok()?;
    let image = if original.width().max(original.height()) > max {
        original.resize(max, max, FilterType::Triangle)
    } else {
        original
    };
    if let Some(cached) = &cached {
        let mut png = Vec::new();
        let saved = image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_ok()
            && fs::create_dir_all(cache).is_ok()
            && files::write_atomic(cached, &png).is_ok();
        if !saved {
            eprintln!("miniatura di {} non salvata in cache", source.display());
        }
    }
    Some(image)
}

fn load(cache: &Path, source: &Path, max: u32) -> Option<egui::ColorImage> {
    let rgba = thumbnail(cache, source, max)?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

enum Slot {
    Loading,
    /// Con il numero del fotogramma in cui è stata mostrata l'ultima volta.
    Ready(egui::TextureHandle, u64),
    Missing,
}

type Request = (PathBuf, u32);

pub struct Images {
    slots: HashMap<Request, Slot>,
    requests: Sender<Request>,
    results: Receiver<(Request, Option<egui::ColorImage>)>,
    frame: u64,
}

impl Images {
    pub fn new(ctx: &egui::Context, cache: &Path) -> Self {
        let (requests, queue) = channel::<Request>();
        let (done, results) = channel();
        let queue = Arc::new(Mutex::new(queue));
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4));
        for _ in 0..workers {
            let (queue, done, ctx, cache) = (queue.clone(), done.clone(), ctx.clone(), cache.to_path_buf());
            std::thread::spawn(move || loop {
                let Ok(request) = queue.lock().unwrap().recv() else { return };
                let image = load(&cache, &request.0, request.1);
                if done.send((request, image)).is_err() {
                    return;
                }
                ctx.request_repaint();
            });
        }
        Self { slots: HashMap::new(), requests, results, frame: 0 }
    }

    /// Consegna alla finestra le immagini pronte. Va chiamata a ogni fotogramma.
    pub fn receive(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok((request, image)) = self.results.try_recv() {
            let slot = match image {
                Some(image) => {
                    let name = request.0.display().to_string();
                    Slot::Ready(ctx.load_texture(name, image, egui::TextureOptions::LINEAR), self.frame)
                }
                None => Slot::Missing,
            };
            self.slots.insert(request, slot);
        }
        let mut ready: Vec<(u64, Request)> = self
            .slots
            .iter()
            .filter_map(|(key, slot)| if let Slot::Ready(_, at) = slot { Some((*at, key.clone())) } else { None })
            .collect();
        if ready.len() > KEEP {
            ready.sort_unstable_by_key(|(at, _)| *at);
            for (_, key) in &ready[..ready.len() - KEEP] {
                self.slots.remove(key);
            }
        }
    }

    /// La texture dell'immagine, se è già pronta; altrimenti ne chiede il caricamento.
    pub fn get(&mut self, source: &Path, max: u32) -> Option<egui::TextureHandle> {
        let key = (source.to_path_buf(), max);
        match self.slots.get_mut(&key) {
            Some(Slot::Ready(texture, at)) => {
                *at = self.frame;
                Some(texture.clone())
            }
            Some(Slot::Loading | Slot::Missing) => None,
            None => {
                self.slots.insert(key.clone(), Slot::Loading);
                let _ = self.requests.send(key);
                None
            }
        }
    }

    /// `Some(true)` se l'immagine è pronta, `Some(false)` se manca, `None` se si sta caricando.
    #[cfg(test)]
    pub fn ready(&self, source: &Path) -> Option<bool> {
        self.slots.iter().find(|((path, _), _)| path == source).and_then(|(_, slot)| match slot {
            Slot::Ready(..) => Some(true),
            Slot::Missing => Some(false),
            Slot::Loading => None,
        })
    }

    /// Dimentica le immagini di una cartella, per ricaricarle dopo che sono cambiate.
    pub fn forget(&mut self, dir: &Path) {
        self.slots.retain(|(path, _), _| !path.starts_with(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_most_recently_shown_textures_are_kept() {
        let dir = std::env::temp_dir().join(format!("pinball-aio-images-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let ctx = egui::Context::default();
        let mut images = Images::new(&ctx, &dir.join("cache"));
        let sources: Vec<PathBuf> = (0..KEEP + 20)
            .map(|i| {
                let path = dir.join(format!("{i}.png"));
                image::RgbaImage::from_pixel(4, 4, image::Rgba([i as u8, 0, 0, 255])).save(&path).unwrap();
                path
            })
            .collect();
        let missing = dir.join("missing.png");
        for source in sources.iter().chain([&missing]) {
            assert!(images.get(source, TILE).is_none());
        }
        for _ in 0..200 {
            images.receive(&ctx);
            if images.slots.values().all(|s| !matches!(s, Slot::Loading)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ready = images.slots.values().filter(|s| matches!(s, Slot::Ready(..))).count();
        assert_eq!(ready, KEEP);
        assert_eq!(images.ready(&missing), Some(false));
        // Una texture scartata si richiede di nuovo quando torna visibile.
        let dropped = sources.iter().find(|s| images.ready(s).is_none()).unwrap().clone();
        assert!(images.get(&dropped, TILE).is_none());
        assert!(matches!(images.slots.get(&(dropped, TILE)), Some(Slot::Loading)));
        fs::remove_dir_all(dir).unwrap();
    }
}
