#![warn(clippy::all)]
#![allow(clippy::many_single_char_names)]

use image::{ImageBuffer, Rgba, RgbaImage};
use std::{
    cmp::Ordering,
    env::args,
    fs::{create_dir, File},
    io::{stdin, Write},
    path::Path,
};
use walkdir::WalkDir;

mod tilesheets;

type FloatImage = ImageBuffer<Rgba<f32>, Vec<f32>>;

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
        #[inline]
        fn unmult(x: u8, a: u8) -> u8 {
            let n = (x as u16) * 255 / (a as u16);
            if n > 255 {
                255
            } else {
                n as u8
            }
        }
        if p[3] == 0 || p[3] == 255 {
            continue;
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
            Rgba([
                self[0] / self[3],
                self[1] / self[3],
                self[2] / self[3],
                self[3],
            ])
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
    match width.cmp(&w) {
        Ordering::Less => {
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
        }
        Ordering::Equal => img.clone(),
        Ordering::Greater => {
            let (rw, rh) = (w as f32 / (width as f32), h as f32 / (height as f32));
            ImageBuffer::from_fn(width, height, |x: u32, y: u32| {
                let xx = (x as f32 * rw) as u32;
                let yy = (y as f32 * rh) as u32;
                img[(xx, yy)]
            })
        }
    }
}
#[allow(dead_code)]
fn shrink() {
    let _ = create_dir("work/shrunk");
    for entry in WalkDir::new("work/shrink") {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        println!("{:?}", name);
        let mut img = image::open(path).unwrap().to_rgba8();
        fix_translucent(&mut img);
        let img = decode_srgb(&img);
        assert_eq!(
            img.dimensions().0,
            img.dimensions().1,
            "Image was not square!"
        );
        assert!(img.dimensions().0 >= 384, "Image dimensions are too small!");
        let img = resize(&img, 192, 192);
        let img = encode_srgb(&img);
        img.save(format!("work/shrunk/Block {}", name)).unwrap();
    }
}
fn main() {
    println!("Welcome to the FTB tilesheet program!");
    if !Path::new("ftb.json").is_file() {
        println!("Failed to locate ftb.json.");
        println!("Please modify the template ftb.json that was created.");
        println!("Make sure you use a bot account!");
        let mut file = File::create("ftb.json").unwrap();
        file.write_all(
            r#"{
    "useragent": "ftb-rs",
    "username": "insert bot username here",
    "password": "insert bot password here",
    "baseapi": "https://ftb.gamepedia.com/api.php"
}
"#
            .as_bytes(),
        )
        .unwrap();
        return;
    }
    let mut args = args();
    args.next();
    let abbrv = match args.next() {
        Some(x) => x,
        None => {
            println!("Enter mod abbreviation:");
            let mut abbrv = String::new();
            stdin().read_line(&mut abbrv).unwrap();
            abbrv.trim().to_owned()
        }
    };
    tilesheets::update_tilesheet(&abbrv);
}
