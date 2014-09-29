// Copyright © 2014, Peter Atashian

#![feature(phase, tuple_indexing)]

extern crate image;
extern crate lodepng;
#[phase(plugin)]
extern crate regex_macros;
extern crate regex;
extern crate serialize;
extern crate time;

use image::{
    GenericImage,
    ImageBuf,
    MutableRefImage,
    Pixel,
    Rgba,
    SubImage,
};
use std::collections::HashMap;
use std::default::Default;
use std::io::{
    AllPermissions,
    BufferedWriter,
    File,
    PathAlreadyExists,
    TypeFile,
};
use std::io::fs::{
    copy,
    mkdir,
    readdir,
    stat,
};
use std::mem::swap;
use time::precise_time_ns;

pub mod recipes;

pub fn dump_descriptions() {
    let path = Path::new(r"C:\Users\retep998\Minecraft\Wiki\GT Lang\GregTech.lang");
    let mut file = File::open(&path).unwrap();
    let data = file.read_to_string().unwrap();
    let reg = regex!(r"S:TileEntity_DESCRIPTION_(\d+)_([0-9A-Za-z_]+)=(.+?)\r?\n");
    let mut descs = Vec::new();
    for cap in reg.captures_iter(data.as_slice()) {
        let index: i32 = from_str(cap.at(1)).unwrap();
        let name = cap.at(2);
        let desc = cap.at(3);
        descs.push((index, name, desc));
    }
    descs.sort_by(|&(a, _, _), &(b, _, _)| a.cmp(&b));
    let mut file = BufferedWriter::new(File::create(&Path::new("descriptions.txt")).unwrap());
    for desc in descs.iter() {
        let &(index, name, desc) = desc;
        (write!(file, "{},{},", index, name)).unwrap();
        let mut color = false;
        let mut code = false;
        for c in desc.chars() {
            match code {
                true => {
                    code = false;
                    if color { file.write_str("}}").unwrap() }
                    color = true;
                    match c {
                        '0' => file.write_str("{{Color|000000|").unwrap(),
                        '1' => file.write_str("{{Color|0000AA|").unwrap(),
                        '2' => file.write_str("{{Color|00AA00|").unwrap(),
                        '3' => file.write_str("{{Color|00AAAA|").unwrap(),
                        '4' => file.write_str("{{Color|AA0000|").unwrap(),
                        '5' => file.write_str("{{Color|AA00AA|").unwrap(),
                        '6' => file.write_str("{{Color|FFAA00|").unwrap(),
                        '7' => file.write_str("{{Color|AAAAAA|").unwrap(),
                        '8' => file.write_str("{{Color|555555|").unwrap(),
                        '9' => file.write_str("{{Color|5555FF|").unwrap(),
                        'a' => file.write_str("{{Color|55FF55|").unwrap(),
                        'b' => file.write_str("{{Color|55FFFF|").unwrap(),
                        'c' => file.write_str("{{Color|FF5555|").unwrap(),
                        'd' => file.write_str("{{Color|FF55FF|").unwrap(),
                        'e' => file.write_str("{{Color|FFFF55|").unwrap(),
                        'f' => file.write_str("{{Color|FFFFFF|").unwrap(),
                        'r' => color = false,
                        _ => println!("Unknown: {}", c),
                    }
                },
                false => {
                    match c {
                        '§' => code = true,
                        c => file.write_char(c).unwrap(),
                    }
                },
            }
        }
        if color { file.write_str("}}").unwrap() }
        file.write_line("").unwrap();
    }
}
pub fn read_gt_lang() -> HashMap<String, String> {
    let path = Path::new(r"work/GregTech.lang");
    let mut file = File::open(&path).unwrap();
    let data = file.read_to_string().unwrap();
    let reg = regex!(r"S:([\w\.]+?)=(.+?)\r?\n");
    reg.captures_iter(data.as_slice()).map(|cap|
        (cap.at(1).into_string(), cap.at(2).into_string())
    ).collect()
}
pub fn import_special_metaitems(lang: &HashMap<String, String>) {
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\items");
    let outpath = Path::new(r"work\tilesheets\GT");
    match mkdir(&outpath, AllPermissions) {
        Ok(_) => (),
        Err(ref e) if e.kind == PathAlreadyExists => (),
        Err(e) => println!("{}", e),
    }
    let import = |category: &str| {
        for path in readdir(&inpath.join(category)).unwrap().iter() {
            if stat(path).unwrap().kind != TypeFile { continue }
            if path.extension_str() != Some("png") { continue }
            let stub: u32 = from_str(path.filestem_str().unwrap()).unwrap();
            let rawname = format!("{}.{}.name", category, stub + 32000);
            let name = match lang.find(&rawname) {
                Some(s) => s,
                None => continue,
            };
            let mut out = outpath.join(name.as_slice());
            out.set_extension("png");
            copy(path, &out).unwrap();
        }
    };
    import("gt.metaitem.01");
    import("gt.metaitem.02");
}
pub fn import_fluids(lang: &HashMap<String, String>) {
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\blocks\fluids");
    let outpath = Path::new(r"work\tilesheets\GT");
    match mkdir(&outpath, AllPermissions) {
        Ok(_) => (),
        Err(ref e) if e.kind == PathAlreadyExists => (),
        Err(e) => println!("{}", e),
    }
    for path in readdir(&inpath).unwrap().iter() {
        if stat(path).unwrap().kind != TypeFile { continue }
        if path.extension_str() != Some("png") { continue }
        let stub = path.filestem_str().unwrap();
        let name = match lang.find_equiv(&stub) {
            Some(s) => s,
            None => continue,
        };
        let name = format!("{} (Fluid)", name);
        let mut out = outpath.join(name.as_slice());
        out.set_extension("png");
        let img = lodepng::load(path).unwrap();
        let (w, h) = img.dimensions();
        let mut pixels = img.into_vec();
        pixels.truncate(w as uint * w as uint);
        let img = ImageBuf::from_pixels(pixels, w, w);
        lodepng::save(&img, &out).unwrap();
    }
}
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
fn decode_srgb(img: &ImageBuf<Rgba<u8>>) -> ImageBuf<Rgba<f32>> {
    fn decode(x: u8) -> f32 {
        let x = x as f32 * (1. / 255.);
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / (1. + 0.055)).powf(2.4)
        }
    }
    let (w, h) = img.dimensions();
    let pix = img.pixelbuf().iter().map(|p| {
        let p = Rgba(decode(p.0), decode(p.1), decode(p.2), decode(p.3));
        Rgba(p.0 * p.3, p.1 * p.3, p.2 * p.3, p.3)
    }).collect();
    ImageBuf::from_pixels(pix, w, h)
}
fn encode_srgb(img: &ImageBuf<Rgba<f32>>) -> ImageBuf<Rgba<u8>> {
    fn encode(x: f32) -> u8 {
        let x = if x <= 0.0031308 {
            x * 12.92
        } else {
            x.powf(1. / 2.4) * (1. + 0.055) - 0.055
        };
        (x * 255.).round().max(0.).min(255.) as u8
    }
    let (w, h) = img.dimensions();
    let pix = img.pixelbuf().iter().map(|p| {
        let p = if p.3 > 0.0001 {
            Rgba(p.0 / p.3, p.1 / p.3, p.2 / p.3, p.3)
        } else {
            Rgba(0., 0., 0., 0.)
        };
        Rgba(encode(p.0), encode(p.1), encode(p.2), encode(p.3))
    }).collect();
    ImageBuf::from_pixels(pix, w, h)
}
fn resize(img: &ImageBuf<Rgba<f32>>, width: u32, height: u32) -> ImageBuf<Rgba<f32>> {
    let (w, h) = img.dimensions();
    assert!(width == height);
    assert!(w == h);
    if width < w {
        let mut new = ImageBuf::new(width, height);
        let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
        for (x, y, pixel) in new.mut_pixels() {
            let (x1, x2) = ((x as f32 * rw) as u32, ((x + 1) as f32 * rw) as u32);
            let (y1, y2) = ((y as f32 * rh) as u32, ((y + 1) as f32 * rh) as u32);
            let (mut r, mut g, mut b, mut a) = (0., 0., 0., 0.);
            for xx in range(x1, x2) {
                for yy in range(y1, y2) {
                    let p = img.get_pixel(xx, yy);
                    r += p.0;
                    g += p.1;
                    b += p.2;
                    a += p.3;
                }
            }
            let m = 1. / (((x2 - x1) * (y2 - y1)) as f32);
            *pixel = Rgba(r * m, g * m, b * m, a * m);
        }
        new
    } else if width == w {
        img.clone()
    } else {
        let mut new = ImageBuf::new(width, height);
        let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
        for (x, y, pixel) in new.mut_pixels() {
            let xx = (x as f32 * rw) as u32;
            let yy = (y as f32 * rh) as u32;
            *pixel = img.get_pixel(xx, yy);
        }
        new
    }
}
pub fn update_tilesheet(name: &str, sizes: &[u32]) {
    let mut manager = TilesheetManager::new(name, sizes);
    manager.update();
    manager.save();
}

fn main() {
    // recipes::do_recipe_calc();
    let a = precise_time_ns();
    let lang = read_gt_lang();
    import_special_metaitems(&lang);
    import_fluids(&lang);
    update_tilesheet("GT", &[16, 32]);
    let b = precise_time_ns();
    println!("{}ms", (b - a) / 1_000_000);
}
