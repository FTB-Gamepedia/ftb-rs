// Copyright © 2014, Peter Atashian

#![feature(phase, tuple_indexing, associated_types, slicing_syntax)]

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
};
use std::collections::{
    HashMap,
};
use std::io::{
    ALL_PERMISSIONS,
    BufferedWriter,
    File,
    TypeFile,
};
use std::io::fs::{
    copy,
    mkdir,
    readdir,
    stat,
};
use time::precise_time_ns;

pub mod tilesheets;

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
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
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
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
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
pub fn render_blocks(lang: &HashMap<String, String>) {
    let inpath = Path::new(r"work\assets\gt\gregtech\textures\blocks\iconsets");
    let outpath = Path::new(r"work\tilesheets\GT");
    let _ = mkdir(&outpath, ALL_PERMISSIONS);
    let skew_down = |img: &ImageBuf<Rgba<f32>>| {
        let (width, height) = img.dimensions();
        let mut out = ImageBuf::from_pixel(width, height + width / 2, Rgba(0., 0., 0., 0.));
        for (x, y, p) in img.pixels() {
            let y = y + x / 2;
            out.put_pixel(x, y, p);
        }
        out
    };
    let render_block = |langname, texture| {
        let name = format!("{}.png", texture);
        let img = decode_srgb(&lodepng::load(&inpath.join(name)).unwrap());
        let size = 150f64;
        let sin30 = 30f64.to_radians().sin();
        let cos30 = 30f64.to_radians().cos();
        let sqrt2 = 2f64.sqrt();
        let sqrt3 = 3f64.sqrt();
        let sidelen = size * 2. * (sqrt3 - sqrt2);
        let vertlen = sidelen * cos30;
        let diagonal = sidelen * sqrt2;
        let halfwidth = diagonal * 0.5;
        let img = resize(&img, halfwidth as u32, vertlen as u32);
        let name = lang.find(&format!("{}.name", langname)).unwrap();
        let name = format!("{}.png", name);
        let img = skew_down(&img);
        lodepng::save(&encode_srgb(&img), &outpath.join(name)).unwrap();
    };
    render_block("gt.blockgranites.8", "GRANITE_RED_STONE");
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

pub fn check_lang_dups(lang: &HashMap<String, String>) {
    let mut stuff = HashMap::new();
    for (key, val) in lang.iter() {
        if key.as_slice().contains(".tooltip") { continue }
        if !key.as_slice().contains(".metaitem") { continue }
        match stuff.find(val) {
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

pub fn ms_fun() {
    let original = File::open(&Path::new("dump.data")).read_to_end().unwrap();
    for n in range(0, 8u) {
        let data = range(0, 185120u / 16).map(|i| {
            Rgba(
                (original[i * 16 + n * 2 + 0] & 0x0f) << 4,
                (original[i * 16 + n * 2 + 0] & 0xf0),
                (original[i * 16 + n * 2 + 1] & 0x0f) << 4,
                (original[i * 16 + n * 2 + 1] & 0xf0),
            )
        }).collect();
        let img = ImageBuf::from_pixels(data, 178, 65);
        let name = format!("{}.png", n);
        lodepng::save(&img, &Path::new(name[])).unwrap();
    }
}
pub fn gen_navbox() {
    let fin = File::open(&Path::new("work/navboxin.txt"));
    let fout = File::create(&Path::new("work/navboxout.txt"));
}

fn main() {
    let a = precise_time_ns();
    // let lang = read_gt_lang();
    // check_lang_dups(&lang);
    // render_blocks(&lang);
    // import_special_metaitems(&lang);
    // import_fluids(&lang);
    // tilesheets::update_tilesheet("GT", &[16, 32]);
    // check_navbox();
    let b = precise_time_ns();
    println!("{}ms", (b - a) / 1_000_000);
}
