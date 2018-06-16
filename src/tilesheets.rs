// Copyright © 2015, Peter Atashian

use image::{self, ImageBuffer, RgbaImage};
use mediawiki::{Mediawiki, tilesheet::Tilesheet};
use regex::Regex;
use std::{
    borrow::ToOwned,
    cmp::max,
    collections::{HashMap, HashSet},
    fs::File,
    io::{prelude::*, BufWriter, stdin},
    process::Command,
    mem::swap,
    path::Path,
};
use walkdir::{WalkDir};
use {FloatImage, decode_srgb, encode_srgb, resize, save, fix_translucent};

struct Sheet {
    size: u32,
    img: RgbaImage,
}
impl Sheet {
    fn new(size: u32) -> Sheet {
        let img = ImageBuffer::new(size, size);
        Sheet { size: size, img: img }
    }
    fn load(data: &[u8], size: u32) -> Sheet {
        let img = image::load_from_memory(data).unwrap();
        Sheet { size: size, img: img.to_rgba() }
    }
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
    mw: Mediawiki,
    name: String,
    lookup: HashMap<String, (u32, u32)>,
    entries: HashMap<(u32, u32), String>,
    renames: HashMap<String, String>,
    tilesheets: Vec<Sheet>,
    next: (u32, u32),
}
impl TilesheetManager {
    fn new(name: &str) -> TilesheetManager {
        println!("Starting up tilesheet manager.");
        TilesheetManager {
            mw: Mediawiki::login_path("ftb.json").unwrap(),
            name: name.to_owned(),
            lookup: HashMap::new(),
            entries: HashMap::new(),
            renames: load_renames(name),
            tilesheets: Vec::new(),
            next: (0, 0),
        }
    }
    fn import_tilesheets(&mut self) {
        println!("Checking for existing tilesheet.");
        let sheet = self.mw.query_sheets().into_iter().find(|x| {
            x.as_ref().ok().and_then(|x| x.get("mod")).and_then(|x| x.as_str()).map(|x| x == self.name).unwrap_or(false)
        });
        if let Some(Ok(sheet)) = sheet {
            let sizes: Vec<u64> = sheet["sizes"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect();
            println!("Existing tilesheet sizes: {:?}", sizes);
            println!("Importing existing tilesheet images.");
            for size in sizes {
                let data = self.mw.download_file(&format!("Tilesheet {} {}.png", self.name, size)).unwrap();
                match data {
                    Some(data) => self.tilesheets.push(Sheet::load(&data, size as u32)),
                    None => {
                        println!("WARNING: No tilesheet image found for size {}!", size);
                        self.tilesheets.push(Sheet::new(size as u32));
                    }
                }
            }
        } else {
            println!("No tilesheet found. Please specify desired sizes separated by commas:");
            let mut sizes = String::new();
            stdin().read_line(&mut sizes).unwrap();
            for size in sizes.split(',').map(|x| x.trim().parse().unwrap()) {
                self.tilesheets.push(Sheet::new(size));
            }
        }
    }
    fn import_tiles(&mut self) {
        println!("Importing tiles.");
        for tile in self.mw.query_tiles(Some(&*self.name)) {
            println!("{:?}", tile);
        }
    }
    fn update(&mut self) {
        let path = Path::new(r"work/tilesheets").join(&self.name);
        let mut file = File::create(&Path::new(r"work/tilesheets/Added.txt")).unwrap();
        for entry in WalkDir::new(&path) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() { continue }
            if path.extension().and_then(|x| x.to_str()) != Some("png") { continue }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = if let Some(r) = self.renames.get(name) {
                r.clone()
            } else {
                name.to_owned()
            };
            if name.contains(&['_', '[', ']'][..]) { panic!("Illegal name: {:?}", name) }
            let mut img = image::open(&path).unwrap().to_rgba();
            fix_translucent(&mut img);
            let img = decode_srgb(&img);
            let (x, y, new) = self.lookup(&name);
            if new {
                writeln!(&mut file, "{} {} {}", x, y, name).unwrap();
            }
            for tilesheet in self.tilesheets.iter_mut() {
                tilesheet.insert(x, y, &img);
            }
        }
    }
    fn clear_unused(&mut self) {
        let path = Path::new(r"work/tilesheets").join(&self.name);
        let names: HashSet<_> = WalkDir::new(&path).into_iter().filter_map(|entry| {
            let entry = match entry { Ok(x) => x, Err(_) => return None };
            let path = entry.path();
            if !path.is_file() { None }
            else if path.extension().and_then(|x| x.to_str()) != Some("png") { None }
            else {
                let name = path.file_stem().unwrap().to_str().unwrap();
                Some(if let Some(r) = self.renames.get(name) { &**r } else { name }.to_owned())
            }
        }).collect();
        let mut file = File::create(&Path::new(r"work/tilesheets/Deleted.txt")).unwrap();
        let lookup = self.lookup.drain().filter(|&(ref name, _)| {
            if !names.contains(name) {
                writeln!(&mut file, "{}", name).unwrap();
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
        let name = format!("Tilesheet {}.txt", self.name);
        let path = Path::new(r"work/tilesheets").join(name);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        let mut stuff = self.entries.iter().map(|(&(x, y), tile)| {
            (x, y, tile)
        }).collect::<Vec<_>>();
        stuff.sort_by(|a, b| if a.1 == b.1 { a.0.cmp(&b.0) } else { a.1.cmp(&b.1) });
        for &(x, y, tile) in stuff.iter() {
            (writeln!(&mut file, "{} {} {}", x, y, tile)).unwrap();
        }
        println!("Optimizing tilesheets");
        let optipng = self.tilesheets.iter().map(|tilesheet| {
            let name = format!("Tilesheet {} {}.png", self.name, tilesheet.size);
            let path = Path::new(r"work/tilesheets").join(name);
            save(&tilesheet.img, &path);
            Command::new("optipng").arg(path).spawn().unwrap()
        }).collect::<Vec<_>>();
        for mut child in optipng {
            child.wait().unwrap();
        }
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
    let reg = Regex::new(r"(\d+) (\d+) (.+?)\r?\n").unwrap();
    let name = format!("Tilesheet {}.txt", name);
    let path = Path::new(r"work/tilesheets").join(name);
    let mut file = match File::open(&path) {
        Ok(x) => x,
        Err(_) => {
            println!("No tilesheet found. Creating new tilesheet.");
            return HashMap::new();
        }
    };
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    reg.captures_iter(&data).map(|cap| {
        let x = cap[1].parse().unwrap();
        let y = cap[2].parse().unwrap();
        let name = cap[3].to_owned();
        (name, (x, y))
    }).collect()
}
fn load_entries(tiles: &HashMap<String, (u32, u32)>) -> HashMap<(u32, u32), String> {
    tiles.iter().map(|(key, value)| (value.clone(), key.clone())).collect()
}
fn load_renames(name: &str) -> HashMap<String, String> {
    let path = Path::new(r"work/tilesheets").join(name);
    if let Ok(mut file) = File::open(&path.join("renames.txt")) {
        let reg = Regex::new("(.*)=(.*)").unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        s.lines().map(|line| {
            let cap = match reg.captures(line) {
                Some(cap) => cap,
                None => panic!("Invalid line in renames.txt {:?}", line),
            };
            (cap[1].to_owned(), cap[2].to_owned())
        }).collect()
    } else {
        HashMap::new()
    }
}
pub fn update_tilesheet(name: &str) {
    let mut manager = TilesheetManager::new(name);
    manager.import_tilesheets();
    manager.import_tiles();
    println!("Done");
}
