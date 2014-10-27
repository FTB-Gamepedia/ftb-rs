// Copyright © 2014, Peter Atashian

use image::{
    GenericImage,
    ImageBuf,
    MutableRefImage,
    Pixel,
    Rgba,
    SubImage,
};
use lodepng;
use std::collections::{
    HashMap,
};
use std::default::Default;
use std::io::{
    BufferedWriter,
    File,
    TypeFile,
};
use std::io::fs::{
    readdir,
    stat,
};
use std::mem::swap;
use {
    resize,
    encode_srgb,
    decode_srgb,
};

struct Tilesheet {
    size: u32,
    img: ImageBuf<Rgba<u8>>,
}
impl Tilesheet {
    fn insert(&mut self, x: u32, y: u32, img: &ImageBuf<Rgba<f32>>) {
        let (width, height) = img.dimensions();
        assert!(width == height);
        assert!(x < 16);
        let img = resize(img, self.size, self.size);
        let img = encode_srgb(&img);
        let (_, myheight) = self.img.dimensions();
        if (y + 1) * self.size > myheight {
            let mut img = ImageBuf::new(1, 1);
            swap(&mut self.img, &mut img);
            let mut pixels = img.into_vec();
            let len = pixels.len();
            let (w, h) = (self.size * 16, (y + 1) * self.size);
            pixels.grow((w * h) as uint - len, Default::default());
            let mut img = ImageBuf::from_pixels(pixels, w, h);
            swap(&mut self.img, &mut img);
        }
        let mut sub = SubImage::new(
            &mut self.img,
            x * self.size, y * self.size,
            self.size, self.size,
        );
        for ((_, _, from), (_, _, to)) in img.pixels().zip(sub.mut_pixels()) {
            *to = from;
        }
    }
}
struct TilesheetManager {
    name: String,
    lookup: HashMap<String, (u32, u32)>,
    entries: Vec<String>,
    tilesheets: Vec<Tilesheet>,
    unused: uint,
}
impl TilesheetManager {
    fn new(name: &str, sizes: &[u32]) -> TilesheetManager {
        let tilesheets = load_tilesheets(name, sizes);
        let lookup = load_tiles(name);
        let entries = load_entries(&lookup);
        TilesheetManager {
            name: name.into_string(),
            lookup: lookup,
            entries: entries,
            tilesheets: tilesheets,
            unused: 0,
        }
    }
    fn update(&mut self) {
        let path = Path::new(r"work\tilesheets").join(self.name.as_slice());
        for path in readdir(&path).unwrap().iter() {
            if stat(path).unwrap().kind != TypeFile { continue }
            if path.extension_str() != Some("png") { continue }
            let name = path.filestem_str().unwrap();
            let img = lodepng::load(path).unwrap();
            let img = decode_srgb(&img);
            let (x, y) = self.lookup(name);
            for tilesheet in self.tilesheets.iter_mut() {
                tilesheet.insert(x, y, &img);
            }
        }
    }
    fn save(&self) {
        for tilesheet in self.tilesheets.iter() {
            let name = format!("Tilesheet {} {}.png", self.name, tilesheet.size);
            let path = Path::new(r"work\tilesheets").join(name.as_slice());
            lodepng::save(&tilesheet.img, &path).unwrap();
        }
        let name = format!("Tilesheet {}.txt", self.name);
        let path = Path::new(r"work\tilesheets").join(name.as_slice());
        let mut file = BufferedWriter::new(File::create(&path).unwrap());
        for (i, tile) in self.entries.iter().enumerate() {
            let (x, y) = ((i % 16) as u32, (i / 16) as u32);
            (writeln!(file, "{} {} {}", x, y, tile)).unwrap();
        }
    }
    fn lookup(&mut self, name: &str) -> (u32, u32) {
        match self.lookup.find_equiv(&name) {
            Some(&x) => return x,
            None => (),
        }
        for i in range(self.unused, self.entries.len()) {
            if self.entries[i].as_slice() != "" { continue }
            *self.entries.get_mut(i) = name.into_string();
            self.unused = i;
            let (x, y) = ((i % 16) as u32, (i / 16) as u32);
            self.lookup.insert(name.into_string(), (x, y));
            return (x, y);
        }
        let i = self.entries.len();
        self.entries.push(name.into_string());
        self.unused = i;
        let (x, y) = ((i % 16) as u32, (i / 16) as u32);
        self.lookup.insert(name.into_string(), (x, y));
        (x, y)
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
        let x = from_str(cap.at(1)).unwrap();
        let y = from_str(cap.at(2)).unwrap();
        let name = cap.at(3).into_string();
        (name, (x, y))
    }).collect()
}
fn load_entries(tiles: &HashMap<String, (u32, u32)>) -> Vec<String> {
    let mut entries = Vec::new();
    for (name, &(x, y)) in tiles.iter() {
        let index = y as uint * 16 + x as uint;
        let len = entries.len();
        if index >= len { entries.grow(index + 1 - len, String::new()) }
        assert!(entries[index].as_slice() == "");
        *entries.get_mut(index) = name.clone();
    }
    entries
}
fn load_tilesheet(name: &str, size: u32) -> Tilesheet {
    let name = format!("Tilesheet {} {}.png", name, size);
    let path = Path::new(r"work\tilesheets").join(name.as_slice());
    let img = match lodepng::load(&path) {
        Ok(img) => img,
        Err(_) => ImageBuf::new(size * 16, size),
    };
    let (width, _) = img.dimensions();
    assert!(width == size * 16);
    Tilesheet { size: size, img: img }
}
fn load_tilesheets(name: &str, sizes: &[u32]) -> Vec<Tilesheet> {
    sizes.iter().map(|&size| load_tilesheet(name, size)).collect()
}
pub fn update_tilesheet(name: &str, sizes: &[u32]) {
    let mut manager = TilesheetManager::new(name, sizes);
    manager.update();
    manager.save();
}
