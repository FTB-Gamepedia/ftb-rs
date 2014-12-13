// Copyright © 2014, Peter Atashian

#![feature(phase, associated_types, slicing_syntax)]

extern crate cookie;
extern crate hyper;
extern crate image;
#[phase(plugin)]
extern crate regex_macros;
extern crate regex;
extern crate serialize;
extern crate url;

use image::{
    GenericImage,
    ImageBuffer,
    Pixel,
    Rgba,
    RgbaImage,
};
use image::ColorType::RGBA;
use std::collections::{
    HashMap,
};
use std::io::{
    ALL_PERMISSIONS,
    BufferedWriter,
    File,
    FileType,
};
use std::io::fs::{
    PathExtensions,
    copy,
    mkdir,
    readdir,
    stat,
};
use std::io::stdio::{
    flush,
    stdin,
};
use std::num::{
    Float,
    FloatMath,
};

pub mod tilesheets;
pub mod api;

type FloatImage = ImageBuffer<Vec<f32>, f32, Rgba<f32>>;
pub fn save(img: &RgbaImage, path: &Path) {
    image::save_buffer(path, img.as_slice(), img.width(), img.height(), RGBA(8)).unwrap();
}

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
        (write!(&mut file, "{},{},", index, name)).unwrap();
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
    print!("Importing special metaitems... ");
    flush();
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\items");
    let outpath = Path::new(r"work\tilesheets\GT");
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
    let import = |category: &str| {
        for path in readdir(&inpath.join(category)).unwrap().iter() {
            if !path.is_file() { continue }
            if path.extension_str() != Some("png") { continue }
            let stub: u32 = from_str(path.filestem_str().unwrap()).unwrap();
            let rawname = format!("{}.{}.name", category, stub + 32000);
            let name = match lang.get(&rawname) {
                Some(s) => format!("{}.png", s),
                None => continue,
            };
            let out = outpath.join(name[]);
            copy(path, &out).unwrap();
        }
    };
    import("gt.metaitem.01");
    import("gt.metaitem.02");
    println!(" done!");
}
pub fn import_fluids(lang: &HashMap<String, String>) {
    print!("Importing fluids... ");
    flush();
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\blocks\fluids");
    let outpath = Path::new(r"work\tilesheets\GT");
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
    for path in readdir(&inpath).unwrap().iter() {
        if !path.is_file() { continue }
        if path.extension_str() != Some("png") { continue }
        let stub = path.filestem_str().unwrap();
        let name = match lang.get(stub) {
            Some(s) => s,
            None => continue,
        };
        let name = format!("{} (Fluid)", name);
        let mut out = outpath.join(name.as_slice());
        out.set_extension("png");
        let img = image::open(path).unwrap().to_rgba();
        let w = img.width();
        let mut pixels = img.into_vec();
        pixels.truncate((w * w * 4) as uint);
        let img = ImageBuffer::from_raw(w, w, pixels).unwrap();
        save(&img, &out);
    }
    println!(" done!");
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
        ImageBuffer::from_fn(width, height, |x, y| {
            let (x1, x2) = ((x as f32 * rw) as u32, ((x + 1) as f32 * rw) as u32);
            let (y1, y2) = ((y as f32 * rh) as u32, ((y + 1) as f32 * rh) as u32);
            let (mut r, mut g, mut b, mut a) = (0., 0., 0., 0.);
            for xx in range(x1, x2) {
                for yy in range(y1, y2) {
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
        ImageBuffer::from_fn(width, height, |x, y| {
            let xx = (x as f32 * rw) as u32;
            let yy = (y as f32 * rh) as u32;
            img[(xx, yy)]
        })
    }
}

pub fn check_lang_dups(lang: &HashMap<String, String>) {
    let mut stuff = HashMap::new();
    for (key, val) in lang.iter() {
        if key.as_slice().contains(".tooltip") { continue }
        if !key.as_slice().contains(".metaitem") { continue }
        match stuff.get(val) {
            Some(other) => {
                println!("Collision for {}", val);
                println!("{} && {}", key, other);
            },
            None => (),
        }
        stuff.insert(val.clone(), key);
    }
}
pub fn check_navbox() {
    let reg = regex!(r"(\d+) (\d+) (.+?)\r?\n");
    let path = Path::new(r"work\navbox.txt");
    let mut file = File::open(&path).unwrap();
    let navbox = file.read_to_string().unwrap();
    let navbox = navbox.as_slice();
    let path = Path::new(r"work\tilesheets\Tilesheet GT.txt");
    let mut file = File::open(&path).unwrap();
    let data = file.read_to_string().unwrap();
    for cap in reg.captures_iter(data.as_slice()) {
        let name = format!("mod=GT|{}", cap.at(3));
        let name = name.as_slice();
        if !navbox.contains(name) && !name.contains("(Fluid)") {
            println!("{}", cap.at(3));
        }
    }
}
pub fn import_old_tilesheet(name: &str) {
    let path = Path::new(r"work/tilesheets/import.txt");
    if stat(&path).map(|s| s.kind != FileType::RegularFile).unwrap_or(true) { return }
    let mut file = File::open(&path).unwrap();
    let data = file.read_to_string().unwrap();
    let name = format!("work/tilesheets/Tilesheet {}.txt", name);
    let path = Path::new(name[]);
    let mut out = File::create(&path).unwrap();
    let reg = regex!(r"Edit	[0-9]+	(.+?)	[A-Z0-9]+	([0-9]+)	([0-9]+)	16px, 32px\r?\n");
    for cap in reg.captures_iter(data.as_slice()) {
        let name = cap.at(1);
        let x = cap.at(2);
        let y = cap.at(3);
        (writeln!(&mut out, "{} {} {}", x, y, name)).unwrap();
    }
}

fn main() {
    let mut cin = stdin();
    print!("Mod name: ");
    flush();
    let blah = cin.read_line().unwrap();
    let blah = blah[].trim();
    tilesheets::update_tilesheet(blah, &[16, 32]);
    // api::api_things();
    // greg_scan_foods();
    // let lang = read_gt_lang();
    // import_special_metaitems(&lang);
    // import_fluids(&lang);
    // check_lang_dups(&lang);
    // import_old_tilesheet(blah);
    // check_navbox();
}
