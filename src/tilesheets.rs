// Copyright © 2015, Peter Atashian

use image::{self, ImageBuffer, RgbaImage};
use mediawiki::{Mediawiki, tilesheet::Tilesheet};
use regex::Regex;
use std::{
    borrow::ToOwned,
    cmp::max,
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write, stdin},
    process::{Command, exit},
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
#[derive(Debug)]
struct Tile {
    x: u32,
    y: u32,
    id: Option<u64>,
}
struct TilesheetManager {
    mw: Mediawiki,
    name: String,
    tiles: HashMap<String, Tile>,
    entries: HashMap<(u32, u32), String>,
    renames: HashMap<String, String>,
    added: Vec<String>,
    missing: HashSet<String>,
    deleted: Vec<u64>,
    tilesheets: Vec<Sheet>,
    next: (u32, u32),
}
impl TilesheetManager {
    fn new(name: &str) -> TilesheetManager {
        println!("Starting up tilesheet manager.");
        TilesheetManager {
            mw: Mediawiki::login_path("ftb.json").unwrap(),
            name: name.to_owned(),
            tiles: HashMap::new(),
            entries: HashMap::new(),
            renames: load_renames(name),
            added: Vec::new(),
            missing: HashSet::new(),
            deleted: Vec::new(),
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
            let sizes = sizes.split(',').map(|x| x.trim()).collect::<Vec<_>>();
            for size in &sizes {
                self.tilesheets.push(Sheet::new(size.parse().unwrap()));
            }
            let token = self.mw.get_token().unwrap();
            self.mw.create_sheet(&token, &self.name, &sizes.join("|")).unwrap();
        }
    }
    fn import_tiles(&mut self) {
        println!("Importing tiles.");
        for tile in self.mw.query_tiles(Some(&*self.name)) {
            let tile = match tile {
                Ok(tile) => tile,
                Err(e) => {
                    println!("WARNING: Error while querying tiles {:?}", e);
                    continue
                },
            };
            let x = tile["x"].as_u64().unwrap() as u32;
            let y = tile["y"].as_u64().unwrap() as u32;
            let id = tile["id"].as_u64().unwrap();
            let name = tile["name"].as_str().unwrap();
            self.tiles.insert(name.to_owned(), Tile { x: x, y: y, id: Some(id) });
            self.entries.insert((x, y), name.to_owned());
            self.missing.insert(name.to_owned());
        }
    }
    fn check_changes(&mut self) {
        println!("Checking tiles.");
        let path = Path::new(r"work/tilesheets").join(&self.name);
        for entry in WalkDir::new(&path) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() { continue }
            if path.extension().and_then(|x| x.to_str()) != Some("png") { continue }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = match self.renames.get(name) {
                Some(name) => {
                    if name.is_empty() {
                        continue
                    }
                    name.clone()
                },
                None => name.to_owned(),
            };
            if name.contains(&['_', '[', ']'][..]) {
                println!("ERROR: Illegal name: {:?}", name);
                exit(1);
            }
            self.missing.remove(&name);
            if !self.tiles.contains_key(&name) {
                self.added.push(name);
            }
        }
    }
    fn confirm_changes(&mut self) {
        let mut additions = BufWriter::new(File::create("work/tilesheets/additions.txt").unwrap());
        let mut missing = BufWriter::new(File::create(r"work/tilesheets/missing.txt").unwrap());
        let _ = File::create(r"work/tilesheets/todelete.txt").unwrap();
        for tile in &self.added {
            writeln!(&mut additions, "{}", tile).unwrap();
        }
        for tile in &self.missing {
            writeln!(&mut missing, "{}", tile).unwrap();
        }
        drop(additions);
        drop(missing);
        println!("Please confirm that the tiles being added in additions.txt are correct.");
        println!("Also please check over the tiles in missing.txt and ensure that not updating them was intentional.");
        println!("If there are tiles in missing.txt that you no longer wish to keep, please copy them to todelete.txt.");
        println!("If you need to make any changes to the tiles or renames.txt please restart this program.");
        println!("When you are done, please enter \"continue\".");
        let mut response = String::new();
        stdin().read_line(&mut response).unwrap();
        if response.trim().to_lowercase() != "continue" {
            println!("Aborting!");
            exit(1);
        }
    }
    fn record_deletions(&mut self) {
        let todelete = BufReader::new(File::open(r"work/tilesheets/todelete.txt").unwrap());
        for line in todelete.lines() {
            let name = line.unwrap();
            match self.tiles.remove(&name) {
                Some(tile) => {
                    self.deleted.push(tile.id.unwrap());
                    self.entries.remove(&(tile.x, tile.y));
                },
                None => {
                    println!("ERROR: Requested to delete tile that doesn't exist {:?}", name);
                },
            };
        }
    }
    fn lookup(&mut self, name: &str) -> (u32, u32) {
        match self.tiles.get(name) {
            Some(ref tile) => return (tile.x, tile.y),
            None => (),
        }
        let pos = loop {
            let pos = if self.next.1 < self.next.0 {
                (self.next.1, self.next.0)
            } else {
                (self.next.0, self.next.1 - self.next.0)
            };
            if self.entries.get(&pos).is_none() {
                break pos
            }
            self.next.1 += 1;
            if self.next.1 > self.next.0 * 2 {
                self.next.0 += 1;
                self.next.1 = 0;
            }
        };
        self.tiles.insert(name.to_owned(), Tile { x: pos.0, y: pos.1, id: None });
        self.entries.insert(pos, name.to_owned());
        (pos.0, pos.1)
    }
    fn update(&mut self) {
        println!("Updating tilesheet with new tiles.");
        let path = Path::new(r"work/tilesheets").join(&self.name);
        for entry in WalkDir::new(&path) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() { continue }
            if path.extension().and_then(|x| x.to_str()) != Some("png") { continue }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = match self.renames.get(name) {
                Some(name) => {
                    if name.is_empty() {
                        continue
                    }
                    name.clone()
                },
                None => name.to_owned(),
            };
            if name.contains(&['_', '[', ']'][..]) {
                println!("ERROR: Illegal name: {:?}", name);
                exit(1);
            }
            let mut img = image::open(&path).unwrap().to_rgba();
            fix_translucent(&mut img);
            let img = decode_srgb(&img);
            let (x, y) = self.lookup(&name);
            for tilesheet in self.tilesheets.iter_mut() {
                tilesheet.insert(x, y, &img);
            }
        }
    }
    fn optimize(&self) {
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
    fn upload_sheets(&self) {
        println!("Tilesheet uploading does not work currently.");
        println!("Please manually uploaded the tilesheet images.");
    }
    fn delete_tiles(&self) {
        println!("Deleting old tiles that are no longer needed.");
        let token = self.mw.get_token().unwrap();
        for chunk in self.deleted.chunks(50) {
            let tiles = chunk.iter().map(|id| id.to_string()).collect::<Vec<_>>().join("|");
            if let Err(e) = self.mw.delete_tiles(&token, &tiles) {
                println!("ERROR: {:?}", e);
            }
        }
    }
    fn add_tiles(&self) {
        println!("Adding new tiles.");
        let token = self.mw.get_token().unwrap();
        for chunk in self.added.chunks(50) {
            let tiles = chunk.iter().map(|name| {
                let tile = &self.tiles[name];
                format!("{} {} {}", tile.x, tile.y, name)
            }).collect::<Vec<_>>().join("|");
            if let Err(e) = self.mw.add_tiles(&token, &self.name, &tiles) {
                println!("ERROR: {:?}", e);
            }
        }
    }
}
fn load_renames(name: &str) -> HashMap<String, String> {
    let path = Path::new(r"work/tilesheets").join(name);
    match File::open(&path.join("renames.txt")) {
        Ok(mut file) => {
            let reg = Regex::new("(.*)=(.*)").unwrap();
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();
            s.lines().filter_map(|line| {
                match reg.captures(line) {
                    Some(cap) => Some((cap[1].to_owned(), cap[2].to_owned())),
                    None => {
                        println!("WARNING: Invalid line in renames.txt {:?}", line);
                        None
                    },
                }
            }).collect()
        },
        Err(e) => {
            println!("WARNING: Failed to load renames.txt {:?}", e);
            HashMap::new()
        },
    }
}
pub fn update_tilesheet(name: &str) {
    let mut manager = TilesheetManager::new(name);
    manager.import_tilesheets();
    manager.import_tiles();
    manager.check_changes();
    manager.confirm_changes();
    manager.record_deletions();
    manager.update();
    manager.optimize();
    manager.upload_sheets();
    manager.delete_tiles();
    manager.add_tiles();
    println!("Done");
}
