// Copyright © 2015-2016, Peter Atashian

extern crate image;
extern crate mediawiki;
extern crate regex;
extern crate rustc_serialize;
extern crate walkdir;

use image::{ImageBuffer, Rgba, RgbaImage};
use image::ColorType::{RGBA};
use regex::{Regex};
use std::collections::{HashMap};
use std::fs::{File, create_dir};
use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::path::{Path};
use walkdir::{WalkDir};

mod tilesheets;

type FloatImage = ImageBuffer<Rgba<f32>, Vec<f32>>;
fn save(img: &RgbaImage, path: &Path) {
    image::save_buffer(path, img, img.width(), img.height(), RGBA(8)).unwrap();
}

trait Srgb {
    type Linear;
    fn decode(&self) -> <Self as Srgb>::Linear;
}
impl Srgb for Rgba<u8> {
    type Linear = Rgba<f32>;
    fn decode(&self) -> Rgba<f32> {
        fn dec(x: u8) -> f32 {
            let x = x as f32 * (1. / 255.);
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / (1. + 0.055)).powf(2.4)
            }
        }
        let p = Rgba([dec(self[0]), dec(self[1]), dec(self[2]), dec(self[3])]);
        Rgba([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
    }
}
fn fix_translucent(img: &mut RgbaImage) {
    for p in img.pixels_mut() {
        if p[3] == 0 || p[3] == 255 { continue }
        #[inline] fn unmult(x: u8, a: u8) -> u8 {
            let n = (x as u16) * 255 / (a as u16);
            if n > 255 { 255 } else { n as u8 }
        }
        p[0] = unmult(p[0], p[3]);
        p[1] = unmult(p[1], p[3]);
        p[2] = unmult(p[2], p[3]);
    }
}
fn decode_srgb(img: &RgbaImage) -> FloatImage {
    let (w, h) = img.dimensions();
    ImageBuffer::from_fn(w, h, |x, y| img[(x, y)].decode())
}
trait Linear {
    type Srgb;
    fn encode(&self) -> <Self as Linear>::Srgb;
}
impl Linear for Rgba<f32> {
    type Srgb = Rgba<u8>;
    fn encode(&self) -> Rgba<u8> {
        fn enc(x: f32) -> u8 {
            let x = if x <= 0.0031308 {
                x * 12.92
            } else {
                x.powf(1. / 2.4) * (1. + 0.055) - 0.055
            };
            (x * 255.).round().max(0.).min(255.) as u8
        }
        let p = if self[3] > 0.0001 {
            Rgba([self[0] / self[3], self[1] / self[3], self[2] / self[3], self[3]])
        } else {
            Rgba([0., 0., 0., 0.])
        };
        Rgba([enc(p[0]), enc(p[1]), enc(p[2]), enc(p[3])])
    }
}
fn encode_srgb(img: &FloatImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    ImageBuffer::from_fn(w, h, |x, y| img[(x, y)].encode())
}
fn resize(img: &FloatImage, width: u32, height: u32) -> FloatImage {
    let (w, h) = img.dimensions();
    assert!(width.cmp(&w) == height.cmp(&h));
    if width < w {
        let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
        ImageBuffer::from_fn(width, height, |x: u32, y: u32| {
            let (x1, x2) = ((x as f32 * rw) as u32, ((x + 1) as f32 * rw) as u32);
            let (y1, y2) = ((y as f32 * rh) as u32, ((y + 1) as f32 * rh) as u32);
            let (mut r, mut g, mut b, mut a) = (0., 0., 0., 0.);
            for xx in x1..x2 {
                for yy in y1..y2 {
                    let p = img[(xx, yy)];
                    r += p[0];
                    g += p[1];
                    b += p[2];
                    a += p[3];
                }
            }
            let m = 1. / (((x2 - x1) * (y2 - y1)) as f32);
            Rgba([r * m, g * m, b * m, a * m])
        })
    } else if width == w {
        img.clone()
    } else {
        let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
        ImageBuffer::from_fn(width, height, |x: u32, y: u32| {
            let xx = (x as f32 * rw) as u32;
            let yy = (y as f32 * rh) as u32;
            img[(xx, yy)]
        })
    }
}
fn import_old_tilesheet(name: &str) {
    let path = Path::new(r"work/tilesheets/import.txt");
    if !path.is_file() { return }
    println!("Importing old tilesheet");
    let mut file = File::open(&path).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    let name = format!("work/tilesheets/Tilesheet {}.txt", name);
    let path = Path::new(&name);
    let mut out = File::create(&path).unwrap();
    let reg = Regex::new(r"Edit\s+Translate\s+[0-9]+\s+(.+?)\s+[A-Z0-9-]+\s+([0-9]+)\s+([0-9]+)\s+16px, 32px").unwrap();
    for line in data.lines() {
        let cap = reg.captures(line).unwrap();
        let name = &cap[1];
        let x = &cap[2];
        let y = &cap[3];
        (writeln!(&mut out, "{} {} {}", x, y, name)).unwrap();
    }
}
fn deleted_ids() {
    println!("Converting tile names to IDs");
    let mut file = File::open("work/tilesheets/import.txt").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    let reg = Regex::new(r"Edit\s+Translate\s+([0-9]+)\s+(.+?)\s+[A-Z0-9-]+\s+[0-9]+\s+[0-9]+\s+16px, 32px").unwrap();
    let map: HashMap<_, _> = data.lines().map(|line| {
        let cap = reg.captures(line).unwrap();
        let id = cap[1].to_owned();
        let name = cap[2].to_owned();
        (name, id)
    }).collect();
    let mut file = File::open("work/tilesheets/Deleted.txt").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();
    let mut out = File::create("work/tilesheets/IDs.txt").unwrap();
    let mut ids: Vec<_> = data.lines().map(|name| &*map[name]).collect();
    ids.sort();
    for chunk in ids.chunks(40) {
        let ids = chunk.join("|");
        writeln!(&mut out, "{}", ids).unwrap();
    }
}
fn shrink() {
    let _ = create_dir("work/shrunk");
    for entry in WalkDir::new("work/shrink") {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_file() { continue }
        let name = path.file_name().unwrap().to_str().unwrap();
        println!("{:?}", name);
        let mut img = image::open(path).unwrap().to_rgba();
        fix_translucent(&mut img);
        let img = decode_srgb(&img);
        assert_eq!(img.dimensions().0, img.dimensions().1, "Image was not square!");
        assert!(img.dimensions().0 >= 384, "Image dimensions are too small!");
        let img = resize(&img, 192, 192);
        let img = encode_srgb(&img);
        save(&img, format!("work/shrunk/Block {}", name).as_ref());
    }
}
fn main() {
    let cout = stdout();
    let mut cout = cout.lock();
    let cin = stdin();
    let mut cin = cin.lock();
    writeln!(&mut cout, "Welcome to the FTB tilesheet program!").unwrap();
    if !Path::new("ftb.json").is_file() {
        writeln!(&mut cout, "Failed to locate ftb.json.").unwrap();
        writeln!(&mut cout, "Please modify the template ftb.json that was created.").unwrap();
        writeln!(&mut cout, "Make sure you use a bot account!").unwrap();
        let mut file = File::create("ftb.json").unwrap();
        file.write_all(r#"{
    "useragent": "ftb-rs",
    "username": "insert username here",
    "password": "insert password here",
    "baseapi": "http://ftb.gamepedia.com/api.php"
}
"#.as_bytes()).unwrap(); //"
        return
    }
    write!(&mut cout, "Mod abbreviation: ").unwrap();
    cout.flush().unwrap();
    let mut abbrv = String::new();
    cin.read_line(&mut abbrv).unwrap();
    let abbrv = abbrv.trim();
    write!(&mut cout, "Would you like to delete existing tiles not included in your dump? [Y/N]: ").unwrap();
    cout.flush().unwrap();
    let mut response = String::new();
    cin.read_line(&mut response).unwrap();
    let response = response.trim();
    let overwrite = match &*response.to_lowercase() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => {
            writeln!(&mut cout, "Invalid response specified. Aborting.").unwrap();
            return
        }
    };
    tilesheets::update_tilesheet(abbrv, &[16, 32], overwrite);

    // match args[1] {
    //     "update" => tilesheets::update_tilesheet(args[2], &[16, 32], false),
    //     "overwrite" => tilesheets::update_tilesheet(args[2], &[16, 32], true),
    //     "import" => import_old_tilesheet(args[2]),
    //     "todelete" => deleted_ids(),
    //     "shrink" => shrink(),
    //     _ => println!("Invalid command"),
    // }
}
