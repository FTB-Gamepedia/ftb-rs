// Copyright © 2014, Peter Atashian

#![feature(phase, tuple_indexing, associated_types, slicing_syntax, if_let)]

extern crate cookie;
extern crate hyper;
extern crate image;
extern crate lodepng;
#[phase(plugin)]
extern crate regex_macros;
extern crate regex;
extern crate serialize;
extern crate url;

use image::{
    GenericImage,
    ImageBuf,
    MutableRefImage,
    Pixel,
    Rgba,
};
use std::collections::{
    HashMap,
};
use std::io::{
    ALL_PERMISSIONS,
    BufferedReader,
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
    walk_dir,
};
use std::num::{
    Float,
    FloatMath,
};

pub mod tilesheets;
pub mod api;

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
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\items");
    let outpath = Path::new(r"work\tilesheets\GT");
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
    let import = |category: &str| {
        for path in readdir(&inpath.join(category)).unwrap().iter() {
            if stat(path).unwrap().kind != FileType::RegularFile { continue }
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
}
pub fn import_fluids(lang: &HashMap<String, String>) {
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\blocks\fluids");
    let outpath = Path::new(r"work\tilesheets\GT");
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
    for path in readdir(&inpath).unwrap().iter() {
        if stat(path).unwrap().kind != FileType::RegularFile { continue }
        if path.extension_str() != Some("png") { continue }
        let stub = path.filestem_str().unwrap();
        let name = match lang.get(stub) {
            Some(s) => s,
            None => continue,
        };
        let name = format!("{} (Fluid)", name);
        let mut out = outpath.join(name.as_slice());
        out.set_extension("png");
        let img = lodepng::load(path).unwrap();
        let (w, _) = img.dimensions();
        let mut pixels = img.into_vec();
        pixels.truncate(w as uint * w as uint);
        let img = ImageBuf::from_pixels(pixels, w, w);
        lodepng::save(&img, &out).unwrap();
    }
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
        let p = Rgba(dec(self.0), dec(self.1), dec(self.2), dec(self.3));
        Rgba(p.0 * p.3, p.1 * p.3, p.2 * p.3, p.3)
    }
}
fn decode_srgb(img: &ImageBuf<Rgba<u8>>) -> ImageBuf<Rgba<f32>> {
    let (w, h) = img.dimensions();
    let pix = img.pixelbuf().iter().map(|p| p.decode()).collect();
    ImageBuf::from_pixels(pix, w, h)
}
trait Linear {
    type Srgb;
    fn encode(&self) -> <Self as Linear>::Srgb;
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
    assert!(width.cmp(&w) == height.cmp(&h));
    if width < w {
        let mut new = ImageBuf::new(width, height);
        let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
        for (x, y, pixel) in new.pixels_mut() {
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
        for (x, y, pixel) in new.pixels_mut() {
            let xx = (x as f32 * rw) as u32;
            let yy = (y as f32 * rh) as u32;
            *pixel = img.get_pixel(xx, yy);
        }
        new
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
pub fn greg_scan_foods() {
    let scandir = Path::new(r"C:\Users\retep998\Minecraft\Wiki\GT Dev");
    let mut lines = Vec::new();
    for p in walk_dir(&scandir).unwrap() {
        if !p.is_file() || p.extension_str() != Some("java") { continue }
        let mut inf = BufferedReader::new(File::open(&p).unwrap());
        for line in inf.lines() {
            let line = line.unwrap();
            lines.push(line);
        }
    }
    let namef = File::open(&Path::new("work/foodlist.txt")).unwrap();
    let mut namef = BufferedReader::new(namef);
    let mut outf = File::create(&Path::new("work/Food.java")).unwrap();
    let reg = regex!(r#"addFluid\("([^"]*)""#);
    let mut found_lines = Vec::new();
    for name in namef.lines() {
        let name = name.unwrap();
        let name = name[].trim();
        for line in lines.iter() {
            if line[].contains(name[]) && !line[].contains("TE_Slag") {
                if let Some(cap) = reg.captures(line[]) {
                    for line in lines.iter() {
                        if line[].contains(cap.at(1)) {
                            found_lines.push(line.clone());
                        }
                    }
                }
                found_lines.push(line.clone());
            }
        }
    }
    found_lines.sort();
    found_lines.dedup();
    for line in found_lines.iter() {
        outf.write_str(line[]).unwrap();
    }
}
fn main() {
    api::api_things();
    // greg_scan_foods();
    // let lang = read_gt_lang();
    // check_lang_dups(&lang);
    // render_blocks(&lang);
    // import_special_metaitems(&lang);
    // import_fluids(&lang);
    // tilesheets::update_tilesheet("GT", &[16, 32]);
    // check_navbox();
}
