use std::{
    cmp::{max, min},
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use dot_vox::{Color, DotVoxData, Model, SceneNode, ShapeModel, Voxel};
use fastanvil::Region;
use image::{io::Reader as ImageReader, Pixel, RgbImage};
use itertools::iproduct;
use ndarray::Array3;
use palette::{
    color_difference::EuclideanDistance, convert::FromColorUnclamped, FromColor, IntoColor,
};

#[derive(Parser, Debug)]
#[command(
    version = "1.0",
    author = "Mathis Brossier",
    about = "Convert Minecraft chunks to MagicaVoxel .vox"
)]
struct Args {
    /// Path to the input Minecraft .mca file
    #[clap(required = true)]
    mc_save_dir: PathBuf,

    /// Path to the "block" folder of a Minecraft ressourcepack
    #[clap(required = true)]
    block_textures: PathBuf,

    /// X-coordinate of the start block
    #[clap(required = true)]
    s_x: isize,

    /// Y-coordinate of the start block
    #[clap(required = true)]
    s_y: isize,

    /// Z-coordinate of the start block
    #[clap(required = true)]
    s_z: isize,

    /// X-coordinate of the end block
    #[clap(required = true)]
    e_x: isize,

    /// Y-coordinate of the end block
    #[clap(required = true)]
    e_y: isize,

    /// Z-coordinate of the end block
    #[clap(required = true)]
    e_z: isize,

    /// Path to the output MagicaVoxel .vox file
    #[clap(required = true)]
    output_file: PathBuf,

    /// 1 voxel = 1/16 minecraft block
    #[arg(long)]
    tiny: bool,
}

static IGNORE_BLOCKS: [&str; 17] = [
    "air",
    "short_grass",
    "poppy",
    "azure_bluet",
    "dandelion",
    "cornflower",
    "oxeye_daisy",
    "sugar_cane",
    "seagrass",
    "glow_lichen",
    "brown_mushroom",
    "dead_bush",
    "vine",
    "lily_pad",
    "ladder",
    "torch",
    "brewing_stand",
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

fn run(args: &Args) -> (Array3<u32>, Vec<[u8; 4]>) {
    let mut voxels = Array3::zeros((
        (args.e_x - args.s_x + 1) as usize,
        (args.e_y - args.s_y + 1) as usize,
        (args.e_z - args.s_z + 1) as usize,
    ));
    let mut palette = HashMap::new();
    let mut colors = Vec::new();

    let s_rx = args.s_x.div_euclid(16 * 32);
    let s_rz = args.s_z.div_euclid(16 * 32);
    let e_rx = args.e_x.div_euclid(16 * 32);
    let e_rz = args.e_z.div_euclid(16 * 32);

    // for each region file
    for (rx, rz) in iproduct!(s_rx..=e_rx, s_rz..=e_rz) {
        println!("processing region {rx} {rz}");
        let mut region_file = args.mc_save_dir.clone();
        region_file.push("region");
        region_file.push(format!("r.{}.{}.mca", rx, rz));
        let region_file = std::fs::File::open(region_file).expect("missing region file");
        let mut region = Region::from_stream(region_file).expect("failed to parse region file");

        let s_cx = max(args.s_x.div_euclid(16) - rx * 32, 0);
        let s_cz = max(args.s_z.div_euclid(16) - rz * 32, 0);
        let e_cx = min(args.e_x.div_euclid(16) - rx * 32, 31);
        let e_cz = min(args.e_z.div_euclid(16) - rz * 32, 31);

        // for each chunk in region
        for (cx, cz) in iproduct!(s_cx..=e_cx, s_cz..=e_cz) {
            println!("processing chunk {cx} {cz}");
            let chunk = region.read_chunk(cx as usize, cz as usize).unwrap();

            if let Some(chunk) = chunk {
                let chunk =
                    fastanvil::complete::Chunk::from_bytes(&chunk).expect("corrupted chunk?");
                let s_x = max(args.s_x - rx * 32 * 16 - cx * 16, 0);
                let s_z = max(args.s_z - rz * 32 * 16 - cz * 16, 0);
                let e_x = min(args.e_x - rx * 32 * 16 - cx * 16, 15);
                let e_z = min(args.e_z - rz * 32 * 16 - cz * 16, 15);

                // for each block in chunk
                for (x, y, z) in iproduct!(s_x..=e_x, args.s_y..=args.e_y, s_z..=e_z) {
                    let block = chunk.sections.block(x as usize, y, z as usize).unwrap();
                    let name = &block.name()["minecraft:".len()..];

                    if !IGNORE_BLOCKS.contains(&name) {
                        let i = palette.get(name).copied().or_else(|| {
                            let color = block_avg_color(&args.block_textures, name)?;
                            println!("{:20}\t{:?}", name, color);
                            let i = palette.len() as u32;
                            colors.push([color.r, color.g, color.b, color.a]);
                            palette.insert(name.to_string(), i);
                            Some(i)
                        });

                        if let Some(i) = i {
                            let x = (x + cx * 16 + rx * 16 * 32 - args.s_x) as usize;
                            let y = (y - args.s_y) as usize;
                            let z = (z + cz * 16 + rz * 16 * 32 - args.s_z) as usize;
                            voxels[(x, y, z)] = i + 1;
                        }
                    }
                }
            } else {
                println!("chunk not generated!")
            }
        }
    }

    (voxels, colors)
}

fn main() {
    let mut args: Args = Args::parse();
    let s_x = min(args.s_x, args.e_x);
    let s_y = min(args.s_y, args.e_y);
    let s_z = min(args.s_z, args.e_z);
    args.e_x = max(args.s_x, args.e_x);
    args.e_y = max(args.s_y, args.e_y);
    args.e_z = max(args.s_z, args.e_z);
    args.s_x = s_x;
    args.s_y = s_y;
    args.s_z = s_z;
    println!(
        "parsing a minecraft region of size ({}, {}, {})",
        args.e_x - args.s_x + 1,
        args.e_y - args.s_y + 1,
        args.e_z - args.s_z + 1
    );
    let out_file = File::create(&args.output_file).expect("failed to create output file");
    let mut out_file = BufWriter::new(out_file);
    let (voxels, palette) = run(&args);
    println!("writing to file");
    bincode::serialize_into(&mut out_file, &(voxels, palette))
        .expect("failed to serialize / write data");
    out_file.flush().unwrap();
}
