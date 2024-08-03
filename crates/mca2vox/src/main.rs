use core::panic;
use std::{
    cmp::{max, min},
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    ops::Deref,
    path::{Path, PathBuf},
};

use clap::Parser;
use fastanvil::Region;
use image::{io::Reader as ImageReader, GenericImage, GenericImageView, Pixel, RgbImage};
use itertools::{iproduct, Itertools};
use ndarray::{s, Array2, Array3};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(
    version = "1.0",
    author = "Mathis Brossier",
    about = "Convert Minecraft chunks to MagicaVoxel .vox"
)]
struct Cli {
    /// Path to the input Minecraft .mca file
    #[clap(required = true)]
    mc_save_dir: PathBuf,

    /// Path to the "block" folder of a Minecraft ressourcepack
    #[clap(required = true)]
    block_textures: PathBuf,

    /// X-coordinate of the start block
    #[clap(required = true)]
    x1: isize,

    /// Y-coordinate of the start block
    #[clap(required = true)]
    y1: isize,

    /// Z-coordinate of the start block
    #[clap(required = true)]
    z1: isize,

    /// X-coordinate of the end block
    #[clap(required = true)]
    x2: isize,

    /// Y-coordinate of the end block
    #[clap(required = true)]
    y2: isize,

    /// Z-coordinate of the end block
    #[clap(required = true)]
    z2: isize,

    /// Path to the output MagicaVoxel .vox file
    #[clap(required = true)]
    output_file: PathBuf,

    /// 1 minecraft block is 16 voxels.
    /// valid values are 1, 2, 4, 8, 16
    #[arg(long)]
    vox_per_block: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

static IGNORE_BLOCKS: &'static [&str] = &[
    "air",
    "short_grass",
    "tall_grass",
    // flowers
    "poppy",
    "azure_bluet",
    "dandelion",
    "cornflower",
    "sunflower",
    "oxeye_daisy",
    "lilac",
    "peony",
    "large_fern",
    "fire",
    "sugar_cane",
    "seagrass",
    "glow_lichen",
    "brown_mushroom",
    "vine",
    "lily_pad",
    "ladder",
    "brewing_stand",
    "torch",
    "redstone_wall_torch",
    "barrier",
    "end_rod",
    // generics
    "_pressure_plate",
    "_door",
    "_door_bottom",
    "_trapdoor",
    "_slab",
    "_wall",
    "_fence",
    "_bush",
    "_carpet",
];

static BLOCK_ALIASES: &'static [(&str, &str)] = &[
    ("podzol", "podzol_top"),
    ("quartz_block", "quartz_block_top"),
    ("quartz_stairs", "quartz_block_top"),
    ("dirt_path", "dirt_path_top"),
    ("smooth_sandstone", "sandstone_top"),
    ("sandstone_stairs", "sandstone_top"),
    ("cobblestone_stairs", "cobblestone"),
    ("dark_oak_stairs", "dark_oak_planks"),
    ("dark_oak_wood", "dark_oak_log"),
    ("oak_stairs", "oak_planks"),
    ("oak_wood", "oak_log"),
    ("birch_stairs", "birch_planks"),
    ("birch_wood", "birch_log"),
    ("jungle_stairs", "jungle_planks"),
    ("jungle_wood", "jungle_log"),
    ("spruce_stairs", "spruce_planks"),
    ("spruce_wood", "spruce_log"),
    ("acacia_stairs", "acacia_planks"),
    ("acacia_wood", "acacia_log"),
    ("bamboo_stairs", "bamboo_planks"),
    ("bamboo_wood", "bamboo_log"),
    ("cherry_stairs", "cherry_planks"),
    ("cherry_wood", "cherry_log"),
    ("warped_stairs", "warped_planks"),
    ("warped_wood", "warped_log"),
    ("crimson_stairs", "crimson_planks"),
    ("crimson_wood", "crimson_log"),
    ("mangrove_stairs", "mangrove_planks"),
    ("mangrove_wood", "mangrove_log"),
    ("_leaves", "azalea_leaves"),
];

static KNOWN_BLOCKS: &'static [(&str, Color)] = &[
    (
        "grass_block",
        Color {
            r: 68,
            g: 107,
            b: 58,
            a: 255,
        },
    ),
    (
        "water",
        Color {
            r: 10,
            g: 10,
            b: 128,
            a: 128,
        },
    ),
];

fn block_avg_color(block_textures: &Path, name: &str) -> Option<Color> {
    let mut img_path = block_textures.to_path_buf();
    img_path.push(format!("{}.png", name));
    let img = ImageReader::open(img_path)
        .ok()?
        .decode()
        .ok()?
        .to_rgba32f();

    let avg = img
        .pixels()
        .cloned()
        .reduce(|p1, p2| p1.map2(&p2, |c1, c2| c1 + c2))?
        .map(|c| c / img.pixels().len() as f32);

    Some(Color {
        r: (avg.0[0] * 255.0) as u8,
        g: (avg.0[1] * 255.0) as u8,
        b: (avg.0[2] * 255.0) as u8,
        a: (avg.0[3] * 255.0) as u8,
    })
}

fn block_avg_colors(block_textures: &Path, name: &str, size: usize) -> Option<Array2<Color>> {
    let mut img_path = block_textures.to_path_buf();
    img_path.push(format!("{}.png", name));
    let img = ImageReader::open(img_path)
        .ok()?
        .decode()
        .ok()?
        .to_rgba32f();

    let w = 16 / size as u32;

    let res = Array2::from_shape_fn((size, size), |i| {
        let mut col: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
        let img = img.view(i.0 as u32 * w, i.1 as u32 * w, w, w);
        for (i, j) in (0..w).cartesian_product(0..w) {
            col.iter_mut()
                .zip(img.get_pixel(i as u32, j as u32).0)
                .for_each(|(c1, c2)| *c1 += c2);
        }
        col.iter_mut().for_each(|c| *c /= (w * w) as f32);
        Color {
            r: (col[0] * 255.0) as u8,
            g: (col[1] * 255.0) as u8,
            b: (col[2] * 255.0) as u8,
            a: (col[3] * 255.0) as u8,
        }
    });

    Some(res)
}

fn block_colors(block_textures: &Path, name: &str) -> Option<Vec<Color>> {
    let mut img_path = block_textures.to_path_buf();
    img_path.push(format!("{}.png", name));
    let img = ImageReader::open(img_path).ok()?.decode().ok()?;

    let vec = img
        .to_rgb8()
        .pixels()
        .map(|p| Color {
            r: p.0[0],
            g: p.0[1],
            b: p.0[2],
            a: p.0[3],
        })
        .collect();

    Some(vec)
}

fn voxs_from_cols(w: u32, i: u32) -> Array3<u32> {
    let voxs: Array3<u32> =
        Array3::from_shape_fn((w as usize, w as usize, w as usize), |(x, y, z)| {
            let (x, y, z) = (x as u32, y as u32, z as u32);
            if y == w - 1 {
                i + x + z * w
            } else if x == 0 {
                i + z + y * w
            } else if z == 0 {
                i + x + y * w
            } else if x == w - 1 {
                i + z + y * w
            } else if z == w - 1 {
                i + x + y * w
            } else {
                i
            }
        });
    voxs
}

fn run(cli: &Cli) -> (Array3<u32>, Vec<Color>) {
    let dim = (
        (cli.x2 - cli.x1 + 1) as usize * cli.vox_per_block,
        (cli.y2 - cli.y1 + 1) as usize * cli.vox_per_block,
        (cli.z2 - cli.z1 + 1) as usize * cli.vox_per_block,
    );
    let vox_dim = (cli.vox_per_block, cli.vox_per_block, cli.vox_per_block);
    let mut voxels: Array3<u32> = Array3::zeros(dim);
    let mut colors: Vec<Color> = Vec::new();
    let mut palette: HashMap<String, Array3<u32>> = HashMap::new();

    let default_color = Color {
        r: 128,
        g: 128,
        b: 128,
        a: 255,
    };

    colors.push(default_color);
    let default_voxs = Array3::from_elem(vox_dim, colors.len() as u32);

    for (name, color) in KNOWN_BLOCKS {
        colors.push(*color);
        let voxs = Array3::from_elem(vox_dim, colors.len() as u32);
        palette.insert(name.to_string(), voxs);
    }

    let s_rx = cli.x1.div_euclid(16 * 32);
    let s_rz = cli.z1.div_euclid(16 * 32);
    let e_rx = cli.x2.div_euclid(16 * 32);
    let e_rz = cli.z2.div_euclid(16 * 32);

    // for each region file
    for (rx, rz) in iproduct!(s_rx..=e_rx, s_rz..=e_rz) {
        println!("processing region {rx} {rz}");
        let mut region_file = cli.mc_save_dir.clone();
        region_file.push("region");
        region_file.push(format!("r.{}.{}.mca", rx, rz));
        let region_file = std::fs::File::open(region_file).expect("missing region file");
        let mut region = Region::from_stream(region_file).expect("failed to parse region file");

        let s_cx = max(cli.x1.div_euclid(16) - rx * 32, 0);
        let s_cz = max(cli.z1.div_euclid(16) - rz * 32, 0);
        let e_cx = min(cli.x2.div_euclid(16) - rx * 32, 31);
        let e_cz = min(cli.z2.div_euclid(16) - rz * 32, 31);

        // for each chunk in region
        for (cx, cz) in iproduct!(s_cx..=e_cx, s_cz..=e_cz) {
            println!("processing chunk {cx} {cz}");
            let chunk = region.read_chunk(cx as usize, cz as usize).unwrap();

            if let Some(chunk) = chunk {
                let chunk =
                    fastanvil::complete::Chunk::from_bytes(&chunk).expect("corrupted chunk?");
                let s_x = max(cli.x1 - rx * 32 * 16 - cx * 16, 0);
                let s_z = max(cli.z1 - rz * 32 * 16 - cz * 16, 0);
                let e_x = min(cli.x2 - rx * 32 * 16 - cx * 16, 15);
                let e_z = min(cli.z2 - rz * 32 * 16 - cz * 16, 15);

                // for each block in chunk
                for (x, y, z) in iproduct!(s_x..=e_x, cli.y1..=cli.y2, s_z..=e_z) {
                    let block = chunk.sections.block(x as usize, y, z as usize).unwrap();
                    let name = &block.name()["minecraft:".len()..];

                    if IGNORE_BLOCKS.iter().any(|b| name.ends_with(b)) {
                        continue;
                    }

                    let search_name = BLOCK_ALIASES
                        .iter()
                        .find_map(|(b, a)| if name.ends_with(b) { Some(*a) } else { None })
                        .unwrap_or(name);

                    let voxs = palette.get(search_name).cloned().unwrap_or_else(|| {
                        let Some(b_colors) =
                            block_avg_colors(&cli.block_textures, search_name, cli.vox_per_block)
                        else {
                            println!("{:20}\tunknown!", search_name);
                            palette.insert(search_name.to_string(), default_voxs.clone());
                            return default_voxs.clone();
                        };
                        println!("{:20}", name);
                        let i = colors.len() as u32 + 1;
                        let w = cli.vox_per_block as u32;
                        for c in b_colors {
                            colors.push(c);
                        }
                        let voxs = voxs_from_cols(w, i);
                        palette.insert(search_name.to_string(), voxs.clone());
                        voxs
                    });

                    let x1 = (x + cx * 16 + rx * 16 * 32 - cli.x1) as usize * cli.vox_per_block;
                    let y1 = (y - cli.y1) as usize * cli.vox_per_block;
                    let z1 = (z + cz * 16 + rz * 16 * 32 - cli.z1) as usize * cli.vox_per_block;
                    let x2 = x1 + cli.vox_per_block;
                    let y2 = y1 + cli.vox_per_block;
                    let z2 = z1 + cli.vox_per_block;
                    voxels.slice_mut(s![x1..x2, y1..y2, z1..z2]).assign(&voxs);
                }
            } else {
                println!("chunk not generated!")
            }
        }
    }

    (voxels, colors)
}

fn main() {
    let mut args: Cli = Cli::parse();
    let s_x = min(args.x1, args.x2);
    let s_y = min(args.y1, args.y2);
    let s_z = min(args.z1, args.z2);
    args.x2 = max(args.x1, args.x2);
    args.y2 = max(args.y1, args.y2);
    args.z2 = max(args.z1, args.z2);
    args.x1 = s_x;
    args.y1 = s_y;
    args.z1 = s_z;
    args.vox_per_block = max(1, args.vox_per_block);
    if !args.vox_per_block.is_power_of_two() {
        panic!("vox_per_block must be a power of two");
    }
    println!(
        "parsing a minecraft region of size ({}, {}, {})",
        args.x2 - args.x1 + 1,
        args.y2 - args.y1 + 1,
        args.z2 - args.z1 + 1
    );
    let out_file = File::create(&args.output_file).expect("failed to create output file");
    let mut out_file = BufWriter::new(out_file);
    let (voxels, palette) = run(&args);
    println!("writing to file");
    bincode::serialize_into(&mut out_file, &(voxels, palette))
        .expect("failed to serialize / write data");
    out_file.flush().unwrap();
}
