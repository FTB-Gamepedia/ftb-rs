// Copyright © 2014, Peter Atashian

#![feature(phase, tuple_indexing)]

extern crate image;
#[phase(plugin)]
extern crate regex_macros;
extern crate regex;
extern crate time;

use image::{
    GenericImage,
    ImageBuf,
    Pixel,
    Rgba,
};
use std::collections::HashMap;
use std::io::{
    AllPermissions,
    BufferedWriter,
    PathAlreadyExists,
    TypeFile,
};
use std::io::fs::{
    File,
    copy,
    mkdir,
    readdir,
    stat,
};
use time::precise_time_ns;

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
pub fn read_lang() -> HashMap<String, String> {
    let path = Path::new(r"work/GregTech.lang");
    let mut file = File::open(&path).unwrap();
    let data = file.read_to_string().unwrap();
    let reg = regex!(r"S:([\w\.]+?)\.name=(.+?)\r?\n");
    reg.captures_iter(data.as_slice()).map(|cap|
        (cap.at(1).into_string(), cap.at(2).into_string())
    ).collect()
}
pub fn import_special_metaitems(lang: &HashMap<String, String>) {
    let inpath = Path::new(r"work\assets\gregtech\textures\items");
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
            let rawname = format!("{}.{}", category, stub + 32000);
            let name = match lang.find(&rawname) {
                Some(s) => s,
                None => continue,
            };
            let mut out = outpath.join(name.as_slice());
            out.set_extension("png");
            copy(path, &out).unwrap();
            println!("{} -> {}", rawname, name);
        }
    };
    import("gt.metaitem.01");
    import("gt.metaitem.02");
}
pub fn update_tilesheet(name: &str, sizes: &[u32]) {
    struct Tilesheet {
        size: u32,
        img: ImageBuf<Rgba<u8>>,
    }
    fn load_tiles(name: &str) -> HashMap<String, (u32, u32)> {
        let reg = regex!(r"(\d+) (\d+) (.+?)\r?\n");
        let name = format!("Tilesheet {}.txt", name);
        let path = Path::new(r"work\tilesheets").join(name.as_slice());
        let mut file = File::open(&path).unwrap();
        let data = file.read_to_string().unwrap();
        reg.captures_iter(data.as_slice()).map(|cap| {
            let x = from_str(cap.at(1)).unwrap();
            let y = from_str(cap.at(2)).unwrap();
            let name = cap.at(3).into_string();
            (name, (x, y))
        }).collect()
    }
    fn load_tilesheet(name: &str, size: u32) -> Tilesheet {
        let name = format!("Tilesheet {} {}.png", name, size);
        let path = Path::new(r"work\tilesheets").join(name.as_slice());
        let img = match image::open(&path) {
            Ok(img) => img.to_rgba(),
            Err(_) => ImageBuf::new(size * 32, size),
        };
        Tilesheet { size: size, img: img }
    }
    fn load_tilesheets(name: &str, sizes: &[u32]) -> Vec<Tilesheet> {
        sizes.iter().map(|&size| load_tilesheet(name, size)).collect()
    }
    fn decode(img: ImageBuf<Rgba<u8>>) -> ImageBuf<Rgba<f32>> {
        fn decode(x: u8) -> f32 {
            let x = x as f32 * (1. / 255.);
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / (1. + 0.055)).powf(2.4)
            }
        }
        let (w, h) = img.dimensions();
        let pix = img.pixelbuf().iter().map(|p|
            Rgba(decode(p.0), decode(p.1), decode(p.2), decode(p.3))
        ).collect();
        ImageBuf::from_pixels(pix, w, h)
    }
    fn encode(img: ImageBuf<Rgba<f32>>) -> ImageBuf<Rgba<u8>> {
        fn encode(x: f32) -> u8 {
            let x = if x <= 0.0031308 {
                x * 12.92
            } else {
                x.powf(1. / 2.4) * (1. + 0.055) - 0.055
            };
            x.round().max(0.).min(255.) as u8
        }
        let (w, h) = img.dimensions();
        let pix = img.pixelbuf().iter().map(|p|
            Rgba(encode(p.0), encode(p.1), encode(p.2), encode(p.3))
        ).collect();
        ImageBuf::from_pixels(pix, w, h)
    }
    let tilesheets = load_tilesheets(name, sizes);
    let tiles = load_tiles(name);
}
fn main() {
    fail!("hi");
    // let lang = read_lang();
    // import_special_metaitems(&lang);
    // update_tilesheet("IC2", &[16, 32]);
    let a = precise_time_ns();
    image::open(&Path::new(r"work\tilesheets\Tilesheet IC2 32.png"));
    let b = precise_time_ns();
    println!("{}ms", (b - a) / 1_000_000);
}
