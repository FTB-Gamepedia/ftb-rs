// Copyright © 2014, Peter Atashian

use image::{self, GenericImage, ImageBuffer, Pixel, RgbaImage};
use lodepng::{load};
use std::borrow::{ToOwned};
use std::cmp::{max};
use std::collections::{HashMap, HashSet};
use std::old_io::{BufferedWriter, File};
use std::old_io::fs::{PathExtensions, walk_dir};
use std::old_io::process::{Command};
use std::mem::{swap};
use {FloatImage, decode_srgb, encode_srgb, resize, save};

struct Tilesheet {
    size: u32,
    img: RgbaImage,
}
impl Tilesheet {
    fn grow(&mut self, w: u32, h: u32) {
        let mut img = ImageBuffer::new(w, h);
        for (x, y, &pix) in self.img.enumerate_pixels() {
            img.put_pixel(x, y, pix);
        }
        swap(&mut self.img, &mut img);
    }
    fn insert(&mut self, x: u32, y: u32, img: &FloatImage) {
        let (width, height) = img.dimensions();
        assert!(width == height);
        let img = resize(img, self.size, self.size);
        let img = encode_srgb(&img);
        let (w, h) = self.img.dimensions();
        if (x + 1) * self.size > w || (y + 1) * self.size > h {
            let (nw, nh) = (max((x + 1) * self.size, w), max((y + 1) * self.size, h));
            self.grow(nw, nh)
        }
        let (x, y) = (x * self.size, y * self.size);
        for (xx, yy, &pix) in img.enumerate_pixels() {
            self.img.put_pixel(x + xx, y + yy, pix);
        }
    }
}
struct TilesheetManager {
    name: String,
    lookup: HashMap<String, (u32, u32)>,
    entries: HashMap<(u32, u32), String>,
    tilesheets: Vec<Tilesheet>,
    next: (u32, u32),
}
impl TilesheetManager {
    fn new(name: &str, sizes: &[u32]) -> TilesheetManager {
        let tilesheets = load_tilesheets(name, sizes);
        let lookup = load_tiles(name);
        let entries = load_entries(&lookup);
        TilesheetManager {
            name: name.to_owned(),
            lookup: lookup,
            entries: entries,
            tilesheets: tilesheets,
            next: (0, 0),
        }
    }
    fn update(&mut self) {
        let path = Path::new(r"work\tilesheets").join(self.name.as_slice());
        let mut file = File::create(&Path::new(r"work\tilesheets\Added.txt"));
        let renames = if let Ok(mut file) = File::open(&path.join("renames.txt")) {
            let reg = regex!("(.*)=(.*)");
            let s = file.read_to_string().unwrap();
            s.lines_any().map(|line| {
                let cap = reg.captures(line).unwrap();
                (cap.at(1).unwrap().to_owned(), cap.at(2).unwrap().to_owned())
            }).collect()
        } else {
            HashMap::new()
        };
        for path in walk_dir(&path).unwrap() {
            if !path.is_file() { continue }
            if path.extension_str() != Some("png") { continue }
            let name = path.filestem_str().unwrap();
            let name = if let Some(r) = renames.get(name) { &**r } else { name };
            if name.contains("_") { panic!("Illegal name: {:?}", name) }
            let img = load(&path).unwrap();
            let img = decode_srgb(&img);
            let (x, y, new) = self.lookup(name);
            if new {
                writeln!(&mut file, "{} {} {}", x, y, name).unwrap();
            }
            for tilesheet in self.tilesheets.iter_mut() {
                tilesheet.insert(x, y, &img);
            }
        }
    }
    fn clear_unused(&mut self) {
        let path = Path::new(r"work\tilesheets").join(self.name.as_slice());
        let names: HashSet<_> = walk_dir(&path).unwrap().filter_map(|path| {
            if !path.is_file() { None }
            else if path.extension_str() != Some("png") { None }
            else { Some(path.filestem_str().unwrap().to_owned()) }
        }).collect();
        let mut file = File::create(&Path::new(r"work\tilesheets\Deleted.txt"));
        let lookup = self.lookup.drain().filter(|&(ref name, _)| {
            if !names.contains(name) {
                file.write_line(name).unwrap();
                false
            } else { true }
        }).collect();
        let entries = self.entries.drain().filter(|&(_, ref name)| {
            names.contains(name)
        }).collect();
        self.lookup = lookup;
        self.entries = entries;
    }
    fn save(&self) {
        let _optipng = self.tilesheets.iter().map(|tilesheet| {
            let name = format!("Tilesheet {} {}.png", self.name, tilesheet.size);
            let path = Path::new(r"work\tilesheets").join(name.as_slice());
            save(&tilesheet.img, &path);
            Command::new("optipng").arg(path).spawn().unwrap()
        }).collect::<Vec<_>>();
        let name = format!("Tilesheet {}.txt", self.name);
        let path = Path::new(r"work\tilesheets").join(name.as_slice());
        let mut file = BufferedWriter::new(File::create(&path).unwrap());
        let mut stuff = self.entries.iter().map(|(&(x, y), tile)| (x, y, tile)).collect::<Vec<_>>();
        stuff.sort_by(|a, b| if a.1 == b.1 { a.0.cmp(&b.0) } else { a.1.cmp(&b.1) });
        for &(x, y, tile) in stuff.iter() {
            (writeln!(&mut file, "{} {} {}", x, y, tile)).unwrap();
        }
        println!("Waiting for optipng to finish");
    }
    fn increment(&mut self) {
        self.next.1 += 1;
        if self.next.1 > self.next.0 * 2 {
            self.next.0 += 1;
            self.next.1 = 0;
        }
    }
    fn next_pos(&self) -> (u32, u32) {
        if self.next.1 < self.next.0 {
            (self.next.1, self.next.0)
        } else {
            (self.next.0, self.next.1 - self.next.0)
        }
    }
    fn lookup(&mut self, name: &str) -> (u32, u32, bool) {
        match self.lookup.get(name) {
            Some(&(x, y)) => return (x, y, false),
            None => (),
        }
        while self.entries.get(&self.next_pos()).is_some() {
            self.increment();
        }
        let pos = self.next_pos();
        self.lookup.insert(name.to_owned(), pos);
        self.entries.insert(pos, name.to_owned());
        (pos.0, pos.1, true)
    }
}
fn load_tiles(name: &str) -> HashMap<String, (u32, u32)> {
    let reg = regex!(r"(\d+) (\d+) (.+?)\r?\n");
    let name = format!("Tilesheet {}.txt", name);
    let path = Path::new(r"work\tilesheets").join(name.as_slice());
    let mut file = match File::open(&path) {
        Ok(x) => x,
        Err(_) => {
            println!("No tilesheet found. Creating new tilesheet.");
            return HashMap::new();
        }
    };
    let data = file.read_to_string().unwrap();
    reg.captures_iter(data.as_slice()).map(|cap| {
        let x = cap.at(1).unwrap().parse().unwrap();
        let y = cap.at(2).unwrap().parse().unwrap();
        let name = cap.at(3).unwrap().to_owned();
        (name, (x, y))
    }).collect()
}
fn load_entries(tiles: &HashMap<String, (u32, u32)>) -> HashMap<(u32, u32), String> {
    tiles.iter().map(|(key, value)| (value.clone(), key.clone())).collect()
}
fn load_tilesheet(name: &str, size: u32) -> Tilesheet {
    let name = format!("Tilesheet {} {}.png", name, size);
    let path = Path::new(r"work\tilesheets").join(name.as_slice());
    let img = match image::open(&path) {
        Ok(img) => img.to_rgba(),
        Err(_) => ImageBuffer::new(size, size),
    };
    Tilesheet { size: size, img: img }
}
fn load_tilesheets(name: &str, sizes: &[u32]) -> Vec<Tilesheet> {
    sizes.iter().map(|&size| load_tilesheet(name, size)).collect()
}
pub fn update_tilesheet(name: &str, sizes: &[u32], overwrite: bool) {
    println!("Loading tilesheet");
    let mut manager = TilesheetManager::new(name, sizes);
    println!("Updating tilesheet");
    if overwrite {
        manager.clear_unused();
    }
    manager.update();
    println!("Saving tilesheet");
    manager.save();
    println!("Done");
}
