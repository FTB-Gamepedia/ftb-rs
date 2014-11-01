// Copyright © 2014, Peter Atashian

#![feature(phase, tuple_indexing, associated_types, slicing_syntax, if_let)]

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
    BufferedReader,
    BufferedWriter,
    File,
    TypeFile,
};
use std::io::fs::{
    PathExtensions,
    copy,
    mkdir,
    readdir,
    stat,
    walk_dir,
};
use std::rand::random;
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
pub fn greg_write_articles() {
    let lines: Vec<_> = {
        let file = File::open(&Path::new("work/foodlist.txt"));
        let mut file = BufferedReader::new(file.unwrap());
        file.lines().map(|s| s.unwrap()[].trim().into_string()).collect()
    };
}
#[allow(non_upper_case_globals, non_snake_case)]
pub fn greg_ores() {
    //Wut
    const Almandine: i32 = 820;
    const Aluminium: i32 = 19;
    const Amber: i32 = 514;
    const Amethyst: i32 = 509;
    const Apatite: i32 = 530;
    const BandedIron: i32 = 917;
    const Barite: i32 = 904;
    const Bastnasite: i32 = 905;
    const Bauxite: i32 = 822;
    const Bentonite: i32 = 927;
    const Beryllium: i32 = 8;
    const Bismuth: i32 = 90;
    const BlueTopaz: i32 = 513;
    const BrownLimonite: i32 = 930;
    const Calcite: i32 = 823;
    const Cassiterite: i32 = 824;
    const CertusQuartz: i32 = 516;
    const Chalcopyrite: i32 = 855;
    const Cinnabar: i32 = 826;
    const Cobaltite: i32 = 827;
    const Cooperite: i32 = 828;
    const Copper: i32 = 35;
    const Coal: i32 = 535;
    const Diamond: i32 = 500;
    const Emerald: i32 = 501;
    const FoolsRuby: i32 = 512;
    const Galena: i32 = 830;
    const GarnetRed: i32 = 527;
    const GarnetYellow: i32 = 528;
    const Garnierite: i32 = 906;
    const Glauconite: i32 = 933;
    const Gold: i32 = 86;
    const Graphite: i32 = 865;
    const GreenSapphire: i32 = 504;
    const Grossular: i32 = 831;
    const Ilmenite: i32 = 918;
    const Iridium: i32 = 84;
    const Iron: i32 = 32;
    const Jasper: i32 = 511;
    const Lapis: i32 = 526;
    const Lazurite: i32 = 524;
    const Lead: i32 = 89;
    const Lepidolite: i32 = 907;
    const Lignite: i32 = 538;
    const Lithium: i32 = 6;
    const Magnesite: i32 = 908;
    const Magnetite: i32 = 870;
    const Malachite: i32 = 871;
    const Molybdenite: i32 = 942;
    const Molybdenum: i32 = 48;
    const Monazite: i32 = 520;
    const Naquadah: i32 = 324;
    const NaquadahEnriched: i32 = 326;
    const Neodymium: i32 = 67;
    const NetherQuartz: i32 = 522;
    const Nickel: i32 = 34;
    const Olivine: i32 = 505;
    const Opal: i32 = 510;
    const Palladium: i32 = 52;
    const Pentlandite: i32 = 909;
    const Phosphate: i32 = 833;
    const Phosphorus: i32 = 534;
    const Pitchblende: i32 = 873;
    const Platinum: i32 = 85;
    const Plutonium: i32 = 100;
    const Powellite: i32 = 883;
    const Pyrite: i32 = 834;
    const Pyrolusite: i32 = 943;
    const Pyrope: i32 = 835;
    const Quartzite: i32 = 523;
    const Redstone: i32 = 810;
    const RockSalt: i32 = 944;
    const Ruby: i32 = 502;
    const Salt: i32 = 817;
    const Saltpeter: i32 = 836;
    const Sapphire: i32 = 503;
    const Scheelite: i32 = 910;
    const Silver: i32 = 54;
    const Soapstone: i32 = 877;
    const Sodalite: i32 = 525;
    const Spessartine: i32 = 838;
    const Sphalerite: i32 = 839;
    const Spodumene: i32 = 920;
    const Stibnite: i32 = 945;
    const Sulfur: i32 = 22;
    const Talc: i32 = 902;
    const Tantalite: i32 = 921;
    const Tanzanite: i32 = 508;
    const Tetrahedrite: i32 = 840;
    const Thorium: i32 = 96;
    const Tin: i32 = 57;
    const Topaz: i32 = 507;
    const Tungstate: i32 = 841;
    const Uranium: i32 = 98;
    const Uraninite: i32 = 922;
    const VanadiumMagnetite: i32 = 923;
    const Wulfenite: i32 = 882;
    const YellowLimonite: i32 = 931;
    const Zinc: i32 = 36;
    fn GT_Worldgen_GT_Ore_SmallPieces(name: &str, _: bool, min: i32, max: i32, amount: i32, overworld: bool, nether: bool, end: bool, material: i32) {
        let freq = amount as f32 * 0.75;
        let freqr = amount as f32 * 0.25;
        let height = min as f32 + (max - min) as f32 * 0.5;
        let heightr = (max - min) as f32 * 0.5;
        if overworld && false {
            println!("      <StandardGen name='{}.stone' inherits='SmallStone'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:16{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' />
        <Setting name='Frequency' avg='{}' range='{}' />
        <Setting name='Height' avg='{}' range='{}' />
      </StandardGen>", name, material, freq, freqr, height, heightr);
        }
        if nether && false {
            println!("      <StandardGen name='{}.netherrack' inherits='SmallNetherrack'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:17{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' />
        <Setting name='Frequency' avg='{}' range='{}' />
        <Setting name='Height' avg='{}' range='{}' />
      </StandardGen>", name, material, freq, freqr, height, heightr);
        }
        if end && false {
            println!("      <StandardGen name='{}.endstone' inherits='SmallEndstone'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:18{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' />
        <Setting name='Frequency' avg='{}' range='{}' />
        <Setting name='Height' avg='{}' range='{}' />
      </StandardGen>", name, material, freq, freqr, height, heightr);
        }
    }
    fn GT_Worldgen_GT_Ore_Layer(name: &str, _: bool, min: i32, max: i32, weight: i32, density: i32, size: i32, overworld: bool, nether: bool, end: bool, m1: i32, m2: i32, m3: i32, m4: i32) {
        let freq = weight as f32 * 0.001;
        let height = min as f32 + (max - min) as f32 * 0.5;
        let heightr = (max - min) as f32 * 0.5;
        let density = density as f32 * 2. / size as f32;
        let density = 1. - (1. - density) * 0.5;
        let size = size as f32 * 0.3;
        let color: u32 = random();
        let color = (color & 0x00ffffff) | 0x40000000;
        if overworld && false {
            println!("      <Cloud name='{}.stone' inherits='LargeStone' wireframeColor='{}'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <Setting name='DistributionFrequency' avg='{}' range='0' />
        <Setting name='CloudRadius' avg='{}' range='{}' />
        <Setting name='CloudThickness' avg='{}' range='{}' />
        <Setting name='CloudHeight' avg='{}' range='{}' />
        <Setting name='OreDensity' avg='{}' range='0' />
      </Cloud>", name, color, m1, m2, m3, m4, freq, size, size * 0.5, size * 0.5, size * 0.25, height, heightr, density);
        }
        if nether && false {
            println!("      <Cloud name='{}.netherrack' inherits='LargeNetherrack' wireframeColor='{}'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:1{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:1{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:1{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:1{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <Setting name='DistributionFrequency' avg='{}' range='0' />
        <Setting name='CloudRadius' avg='{}' range='{}' />
        <Setting name='CloudThickness' avg='{}' range='{}' />
        <Setting name='CloudHeight' avg='{}' range='{}' />
        <Setting name='OreDensity' avg='{}' range='0' />
      </Cloud>", name, color, m1, m2, m3, m4, freq, size, size * 0.5, size * 0.5, size * 0.25, height, heightr, density);
        }
        if end && true {
            println!("      <Cloud name='{}.endstone' inherits='LargeEndstone' wireframeColor='{}'>
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:2{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:2{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.375' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:2{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <OreBlock block='gregtech:gt.blockores' nbt='{{m:2{:03}, n:1, id:\"GT_TileEntity_Ores\"}}' weight='0.125' />
        <Setting name='DistributionFrequency' avg='{}' range='0' />
        <Setting name='CloudRadius' avg='{}' range='{}' />
        <Setting name='CloudThickness' avg='{}' range='{}' />
        <Setting name='CloudHeight' avg='{}' range='{}' />
        <Setting name='OreDensity' avg='{}' range='0' />
      </Cloud>", name, color, m1, m2, m3, m4, freq, size, size * 0.5, size * 0.5, size * 0.25, height, heightr, density);
        }
    }
    let tPFAA: bool = false;
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.copper", true, 60, 120, 32, !tPFAA, true, true, Copper);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.tin", true, 60, 120, 32, !tPFAA, true, true, Tin);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.bismuth", true, 80, 120, 8, !tPFAA, true, false, Bismuth);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.coal", true, 60, 100, 24, !tPFAA, false, false, Coal);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.iron", true, 40, 80, 16, !tPFAA, true, true, Iron);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.lead", true, 40, 80, 16, !tPFAA, true, true, Lead);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.zinc", true, 30, 60, 12, !tPFAA, true, true, Zinc);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.gold", true, 20, 40, 8, !tPFAA, true, true, Gold);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.silver", true, 20, 40, 8, !tPFAA, true, true, Silver);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.nickel", true, 20, 40, 8, !tPFAA, true, true, Nickel);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.lapis", true, 20, 40, 4, !tPFAA, false, false, Lapis);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.diamond", true, 5, 10, 2, !tPFAA, true, false, Diamond);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.emerald", true, 5, 250, 1, !tPFAA, true, false, Emerald);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.ruby", true, 5, 250, 1, !tPFAA, true, false, Ruby);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.sapphire", true, 5, 250, 1, !tPFAA, true, false, Sapphire);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.greensapphire", true, 5, 250, 1, !tPFAA, true, false, GreenSapphire);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.olivine", true, 5, 250, 1, !tPFAA, true, false, Olivine);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.topaz", true, 5, 250, 1, !tPFAA, true, false, Topaz);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.tanzanite", true, 5, 250, 1, !tPFAA, true, false, Tanzanite);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.amethyst", true, 5, 250, 1, !tPFAA, true, false, Amethyst);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.opal", true, 5, 250, 1, !tPFAA, true, false, Opal);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.jasper", true, 5, 250, 1, !tPFAA, true, false, Jasper);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.bluetopaz", true, 5, 250, 1, !tPFAA, true, false, BlueTopaz);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.amber", true, 5, 250, 1, !tPFAA, true, false, Amber);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.foolsruby", true, 5, 250, 1, !tPFAA, true, false, FoolsRuby);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.garnetred", true, 5, 250, 1, !tPFAA, true, false, GarnetRed);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.garnetyellow", true, 5, 250, 1, !tPFAA, true, false, GarnetYellow);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.redstone", true, 5, 20, 8, !tPFAA, true, false, Redstone);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.platinum", true, 20, 40, 8, false, false, true, Platinum);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.iridium", true, 20, 40, 8, false, false, true, Iridium);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.netherquartz", true, 30, 120, 64, false, true, false, NetherQuartz);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.saltpeter", true, 10, 60, 8, false, true, false, Saltpeter);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.sulfur_n", true, 10, 60, 32, false, true, false, Sulfur);
    GT_Worldgen_GT_Ore_SmallPieces("ore.small.sulfur_o", true, 5, 15, 8, !tPFAA, false, false, Sulfur);
    GT_Worldgen_GT_Ore_Layer("ore.mix.naquadah", false, 10, 60, 10, 5, 32, false, false, true, Naquadah, Naquadah, Naquadah, NaquadahEnriched);
    GT_Worldgen_GT_Ore_Layer("ore.mix.lignite", true, 50, 130, 160, 8, 32, !tPFAA, false, false, Lignite, Lignite, Lignite, Coal);
    GT_Worldgen_GT_Ore_Layer("ore.mix.coal", true, 50, 80, 80, 6, 32, !tPFAA, false, false, Coal, Coal, Coal, Lignite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.magnetite", true, 50, 120, 160, 3, 32, !tPFAA, true, false, Magnetite, Magnetite, Iron, VanadiumMagnetite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.gold", true, 60, 80, 160, 3, 32, !tPFAA, false, false, Magnetite, Magnetite, VanadiumMagnetite, Gold);
    GT_Worldgen_GT_Ore_Layer("ore.mix.iron", true, 10, 40, 120, 4, 24, !tPFAA, true, false, BrownLimonite, YellowLimonite, BandedIron, Malachite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.cassiterite", true, 40, 120, 50, 5, 24, !tPFAA, false, true, Tin, Tin, Cassiterite, Tin);
    GT_Worldgen_GT_Ore_Layer("ore.mix.tetrahedrite", true, 80, 120, 70, 4, 24, !tPFAA, true, false, Tetrahedrite, Tetrahedrite, Copper, Stibnite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.netherquartz", true, 40, 80, 80, 5, 24, false, true, false, NetherQuartz, NetherQuartz, NetherQuartz, NetherQuartz);
    GT_Worldgen_GT_Ore_Layer("ore.mix.sulfur", true, 5, 20, 100, 5, 24, false, true, false, Sulfur, Sulfur, Pyrite, Sphalerite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.copper", true, 10, 30, 80, 4, 24, !tPFAA, true, false, Chalcopyrite, Iron, Pyrite, Copper);
    GT_Worldgen_GT_Ore_Layer("ore.mix.bauxite", true, 50, 90, 80, 4, 24, !tPFAA, tPFAA, false, Bauxite, Bauxite, Aluminium, Ilmenite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.salts", true, 50, 60, 50, 3, 24, !tPFAA, false, false, RockSalt, Salt, Lepidolite, Spodumene);
    GT_Worldgen_GT_Ore_Layer("ore.mix.redstone", true, 10, 40, 60, 3, 24, !tPFAA, true, false, Redstone, Redstone, Ruby, Cinnabar);
    GT_Worldgen_GT_Ore_Layer("ore.mix.soapstone", true, 10, 40, 40, 3, 16, !tPFAA, false, false, Soapstone, Talc, Glauconite, Pentlandite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.nickel", true, 10, 40, 40, 3, 16, !tPFAA, true, true, Garnierite, Nickel, Cobaltite, Pentlandite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.platinum", true, 40, 50, 5, 3, 16, !tPFAA, false, true, Cooperite, Palladium, Platinum, Iridium);
    GT_Worldgen_GT_Ore_Layer("ore.mix.pitchblende", true, 10, 40, 40, 3, 16, !tPFAA, false, false, Pitchblende, Pitchblende, Uranium, Uraninite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.plutonium", true, 20, 30, 10, 3, 16, !tPFAA, false, false, Uraninite, Uraninite, Plutonium, Uranium);
    GT_Worldgen_GT_Ore_Layer("ore.mix.monazite", true, 20, 40, 30, 3, 16, !tPFAA, tPFAA, false, Bastnasite, Bastnasite, Monazite, Neodymium);
    GT_Worldgen_GT_Ore_Layer("ore.mix.molybdenum", true, 20, 50, 5, 3, 16, !tPFAA, false, true, Wulfenite, Molybdenite, Molybdenum, Powellite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.tungstate", true, 20, 50, 10, 3, 16, !tPFAA, false, true, Scheelite, Scheelite, Tungstate, Lithium);
    GT_Worldgen_GT_Ore_Layer("ore.mix.sapphire", true, 10, 40, 60, 3, 16, !tPFAA, tPFAA, tPFAA, Almandine, Pyrope, Sapphire, GreenSapphire);
    GT_Worldgen_GT_Ore_Layer("ore.mix.manganese", true, 20, 30, 20, 3, 16, !tPFAA, false, true, Grossular, Spessartine, Pyrolusite, Tantalite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.quartz", true, 40, 80, 60, 3, 16, !tPFAA, tPFAA, false, Quartzite, Barite, CertusQuartz, CertusQuartz);
    GT_Worldgen_GT_Ore_Layer("ore.mix.diamond", true, 5, 20, 40, 2, 16, !tPFAA, false, false, Graphite, Graphite, Diamond, Coal);
    GT_Worldgen_GT_Ore_Layer("ore.mix.olivine", true, 10, 40, 60, 3, 16, !tPFAA, false, true, Bentonite, Magnesite, Olivine, Glauconite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.apatite", true, 40, 60, 60, 3, 16, !tPFAA, false, false, Apatite, Apatite, Phosphorus, Phosphate);
    GT_Worldgen_GT_Ore_Layer("ore.mix.galena", true, 30, 60, 40, 5, 16, !tPFAA, false, false, Galena, Galena, Silver, Lead);
    GT_Worldgen_GT_Ore_Layer("ore.mix.lapis", true, 20, 50, 40, 5, 16, !tPFAA, false, true, Lazurite, Sodalite, Lapis, Calcite);
    GT_Worldgen_GT_Ore_Layer("ore.mix.beryllium", true, 5, 30, 30, 3, 16, !tPFAA, false, true, Beryllium, Beryllium, Emerald, Thorium);
}
fn main() {
    let a = precise_time_ns();
    greg_ores();
    // greg_scan_foods();
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
