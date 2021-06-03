use crate::{decode_srgb, encode_srgb, fix_translucent, resize, FloatImage};
use image::{self, ImageBuffer, RgbaImage};
use lazy_static::lazy_static;
use mediawiki::{tilesheet::Tilesheet, Mediawiki, Upload};
use regex::Regex;
use std::{
    borrow::ToOwned,
    cmp::max,
    collections::{HashMap, HashSet},
    fs::File,
    io::{stdin, BufRead, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    process::{exit, Command},
};
use walkdir::WalkDir;

const MAX_SIZE: u32 = 64;
lazy_static! {
    static ref BASE_PATH: PathBuf = "tilesheets".into();
}

struct Sheet {
    size: u32,
    layers: Vec<RgbaImage>,
}
impl Sheet {
    fn new(size: u32) -> Sheet {
        Sheet {
            size,
            layers: Vec::new(),
        }
    }
    fn load_layer(&mut self, data: &[u8]) {
        let layer = image::load_from_memory(data).unwrap();
        self.layers.push(layer.to_rgba8());
    }
    fn add_layer(&mut self) {
        let layer = ImageBuffer::new(self.size, self.size);
        self.layers.push(layer);
    }
    fn grow(&mut self, w: u32, h: u32, z: u32) {
        let mut new_layer = ImageBuffer::new(w, h);
        let old_layer = &mut self.layers[z as usize];
        for (x, y, &pix) in old_layer.enumerate_pixels() {
            new_layer.put_pixel(x, y, pix);
        }
        *old_layer = new_layer;
    }
    fn insert(&mut self, TilePos { x, y, z }: TilePos, img: &FloatImage) {
        let (width, height) = img.dimensions();
        assert!(width == height);
        let img = resize(img, self.size, self.size);
        let img = encode_srgb(&img);
        if z as usize == self.layers.len() {
            self.add_layer();
        }
        let (w, h) = self.layers[z as usize].dimensions();
        let (nw, nh) = ((x + 1) * self.size, (y + 1) * self.size);
        if nw > w || nh > h {
            let (nw, nh) = (max((x + 1) * self.size, w), max((y + 1) * self.size, h));
            self.grow(max(w, nw), max(h, nh), z)
        }
        let (x, y) = (x * self.size, y * self.size);
        let layer = &mut self.layers[z as usize];
        for (xx, yy, &pix) in img.enumerate_pixels() {
            layer.put_pixel(x + xx, y + yy, pix);
        }
    }
}
#[derive(Debug)]
struct Tile {
    pos: TilePos,
    id: Option<u64>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TilePos {
    x: u32,
    y: u32,
    z: u32,
}
struct TilesheetManager {
    mw: Mediawiki,
    name: String,
    tiles: HashMap<String, Tile>,
    entries: HashMap<TilePos, String>,
    renames: HashMap<String, String>,
    added: Vec<String>,
    missing: HashSet<String>,
    deleted: Vec<u64>,
    tilesheets: Vec<Sheet>,
    next: (u32, u32, u32),
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
            next: (0, 0, 0),
        }
    }
    fn import_tilesheets(&mut self) {
        println!("Checking for existing tilesheet.");
        let sheet = self.mw.query_sheets().into_iter().find(|x| {
            x.as_ref()
                .ok()
                .and_then(|x| x.get("mod"))
                .and_then(|x| x.as_str())
                .map_or(false, |x| x == self.name)
        });
        if let Some(Ok(sheet)) = sheet {
            let sizes: Vec<u64> = sheet["sizes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap())
                .collect();
            println!("Existing tilesheet sizes: {:?}", sizes);
            println!("Importing existing tilesheet images.");
            for size in sizes {
                let mut sheet = Sheet::new(size as u32);
                for z in 0.. {
                    if let Some(data) = self
                        .mw
                        .download_file(&format!("Tilesheet {} {} {}.png", self.name, size, z))
                        .unwrap()
                    {
                        sheet.load_layer(&data);
                    } else {
                        if z == 0 {
                            println!("WARNING: No tilesheet image found for size {}!", size);
                        }
                        break;
                    }
                }
                self.tilesheets.push(sheet);
            }
        } else {
            println!("No tilesheet found. Please specify desired sizes separated by commas:");
            let mut sizes = String::new();
            stdin().read_line(&mut sizes).unwrap();
            let sizes = sizes.split(',').map(str::trim).collect::<Vec<_>>();
            for size in &sizes {
                self.tilesheets.push(Sheet::new(size.parse().unwrap()));
            }
            let token = self.mw.get_token().unwrap();
            self.mw
                .create_sheet(&token, &self.name, &sizes.join("|"))
                .unwrap();
        }
    }
    fn import_tiles(&mut self) {
        println!("Importing tiles.");
        for tile in self.mw.query_tiles(Some(&*self.name)) {
            let tile = match tile {
                Ok(tile) => tile,
                Err(e) => {
                    println!("WARNING: Error while querying tiles {:?}", e);
                    continue;
                }
            };
            let x = tile["x"].as_u64().unwrap() as u32;
            let y = tile["y"].as_u64().unwrap() as u32;
            let z = tile["z"].as_u64().unwrap() as u32;
            let id = tile["id"].as_u64().unwrap();
            let name = tile["name"].as_str().unwrap();
            let pos = TilePos { x, y, z };
            self.tiles
                .insert(name.to_owned(), Tile { pos, id: Some(id) });
            self.entries.insert(pos, name.to_owned());
            self.missing.insert(name.to_owned());
        }
    }
    fn check_changes(&mut self) {
        println!("Checking tiles.");
        let path = BASE_PATH.join(&self.name);
        for entry in WalkDir::new(&path) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|x| x.to_str()) != Some("png") {
                continue;
            }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = match self.renames.get(name) {
                Some(name) => {
                    if name.is_empty() {
                        continue;
                    }
                    name.clone()
                }
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
        let mut additions = BufWriter::new(File::create(BASE_PATH.join("additions.txt")).unwrap());
        let mut missing = BufWriter::new(File::create(BASE_PATH.join("missing.txt")).unwrap());
        let _ = File::create(BASE_PATH.join("todelete.txt")).unwrap();
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
        let todelete = BufReader::new(File::open(BASE_PATH.join("todelete.txt")).unwrap());
        for line in todelete.lines() {
            let name = line.unwrap();
            if let Some(tile) = self.tiles.remove(&name) {
                self.deleted.push(tile.id.unwrap());
                self.entries.remove(&tile.pos);
            } else {
                println!(
                    "ERROR: Requested to delete tile that doesn't exist {:?}",
                    name
                );
            }
        }
    }
    fn lookup(&mut self, name: &str) -> TilePos {
        if let Some(tile) = self.tiles.get(name) {
            return tile.pos;
        }
        let pos = loop {
            let pos = if self.next.1 < self.next.0 {
                TilePos {
                    x: self.next.1,
                    y: self.next.0,
                    z: self.next.2,
                }
            } else {
                TilePos {
                    x: self.next.0,
                    y: self.next.1 - self.next.0,
                    z: self.next.2,
                }
            };
            if self.entries.get(&pos).is_none() {
                break pos;
            }
            self.next.1 += 1;
            if self.next.1 > self.next.0 * 2 {
                self.next.0 += 1;
                self.next.1 = 0;
                if self.next.0 == MAX_SIZE {
                    self.next.0 = 0;
                    self.next.2 += 1;
                }
            }
        };
        self.tiles.insert(name.to_owned(), Tile { pos, id: None });
        self.entries.insert(pos, name.to_owned());
        pos
    }
    fn update(&mut self) {
        println!("Updating tilesheet with new tiles.");
        let path = BASE_PATH.join(&self.name);
        for entry in WalkDir::new(&path) {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|x| x.to_str()) != Some("png") {
                continue;
            }
            let name = path.file_stem().unwrap().to_str().unwrap();
            let name = match self.renames.get(name) {
                Some(name) => {
                    if name.is_empty() {
                        continue;
                    }
                    name.clone()
                }
                None => name.to_owned(),
            };
            if name.contains(&['_', '[', ']'][..]) {
                println!("ERROR: Illegal name: {:?}", name);
                exit(1);
            }
            let mut img = image::open(&path).unwrap().to_rgba8();
            fix_translucent(&mut img);
            let img = decode_srgb(&img);
            let pos = self.lookup(&name);
            for tilesheet in &mut self.tilesheets {
                tilesheet.insert(pos, &img);
            }
        }
    }
    fn optimize(&self) {
        println!("Optimizing tilesheets");
        let optipng = self
            .tilesheets
            .iter()
            .flat_map(|tilesheet| {
                tilesheet.layers.iter().enumerate().map(move |(z, layer)| {
                    let name = format!("Tilesheet {} {} {}.png", self.name, tilesheet.size, z);
                    let path = BASE_PATH.join(name);
                    layer.save(&path).unwrap();
                    Command::new("optipng").arg(path).spawn().unwrap()
                })
            })
            .collect::<Vec<_>>();
        for mut child in optipng {
            child.wait().unwrap();
        }
    }
    fn upload_sheets(&self) {
        println!("Uploading tilesheets.");
        let token = &self.mw.get_token().unwrap();
        let failed_uploads = self
            .tilesheets
            .iter()
            .flat_map(|tilesheet| {
                tilesheet.layers.iter().enumerate().flat_map(move |(z, _)| {
                    let name = format!("Tilesheet {} {} {}.png", self.name, tilesheet.size, z);
                    let path = BASE_PATH.join(&name);
                    let result = self
                        .mw
                        .upload(
                            &name,
                            token,
                            Upload::File(&path),
                            Some("[[Category:Tilesheets]]"),
                            Some("Tilesheet uploaded by ftb-rs"),
                            false,
                        )
                        .unwrap();
                    match result["upload"]["result"].as_str().unwrap() {
                        "Warning" => {
                            let warnings = result["upload"]["warnings"]
                                .as_object()
                                .unwrap()
                                .iter()
                                .map(|(warning, value)| (warning.clone(), value.clone()))
                                .collect::<Vec<_>>();
                            let filekey = result["upload"]["filekey"].as_str().unwrap().to_string();
                            Some((name, filekey, warnings))
                        }
                        "Success" => None,
                        other => panic!("Unknown result: {}", other),
                    }
                })
            })
            .collect::<Vec<_>>();
        if failed_uploads.is_empty() {
            return;
        }
        println!("Encountered the following warnings while uploading tilesheets:");
        for (name, _, warnings) in &failed_uploads {
            for (warning, value) in warnings {
                println!("[{}] {}: {}", name, warning, value);
            }
        }
        println!("To proceed with file uploads, please enter \"continue\".");
        let mut response = String::new();
        stdin().read_line(&mut response).unwrap();
        if response.trim().to_lowercase() != "continue" {
            println!("Aborting!");
            exit(1);
        }
        for (name, filekey, _) in failed_uploads {
            let result = self
                .mw
                .upload(
                    &name,
                    token,
                    Upload::Filekey(&filekey),
                    Some("[[Category:Tilesheets]]"),
                    Some("Tilesheet uploaded by ftb-rs"),
                    true,
                )
                .unwrap();
            match result["upload"]["result"].as_str().unwrap() {
                "Warning" => (),
                "Success" => (),
                other => panic!("Unknown result: {}", other),
            }
        }
    }
    fn delete_tiles(&self) {
        println!("Deleting old tiles that are no longer needed.");
        let token = self.mw.get_token().unwrap();
        for chunk in self.deleted.chunks(50) {
            let tiles = chunk
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join("|");
            if let Err(e) = self
                .mw
                .delete_tiles(&token, &tiles, Some("ftb-rs deleting tiles"))
            {
                println!("ERROR: {:?}", e);
            }
        }
    }
    fn add_tiles(&self) {
        println!("Adding new tiles.");
        let token = self.mw.get_token().unwrap();
        for chunk in self.added.chunks(50) {
            let tiles = chunk
                .iter()
                .map(|name| {
                    let tile = &self.tiles[name];
                    format!("{} {} {} {}", tile.pos.x, tile.pos.y, tile.pos.z, name)
                })
                .collect::<Vec<_>>()
                .join("|");
            if let Err(e) =
                self.mw
                    .add_tiles(&token, &self.name, &tiles, Some("ftb-rs adding tiles"))
            {
                println!("ERROR: {:?}", e);
            }
        }
    }
}
fn load_renames(name: &str) -> HashMap<String, String> {
    let path = BASE_PATH.join(name);
    match File::open(&path.join("renames.txt")) {
        Ok(mut file) => {
            let reg = Regex::new("(.*)=(.*)").unwrap();
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();
            s.lines()
                .filter_map(|line| match reg.captures(line) {
                    Some(cap) => Some((cap[1].to_owned(), cap[2].to_owned())),
                    None => {
                        println!("WARNING: Invalid line in renames.txt {:?}", line);
                        None
                    }
                })
                .collect()
        }
        Err(e) => {
            println!("WARNING: Failed to load renames.txt {:?}", e);
            HashMap::new()
        }
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
