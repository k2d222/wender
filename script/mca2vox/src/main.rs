use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use clap::Parser;
use dot_vox::{Color, DotVoxData, Model, SceneNode, ShapeModel, Voxel};
use image::{io::Reader as ImageReader, Pixel};
use itertools::iproduct;

#[derive(Parser, Debug)]
#[command(
    version = "1.0",
    author = "Mathis Brossier",
    about = "Convert Minecraft chunks to MagicaVoxel .vox"
)]

struct Args {
    /// Path to the input Minecraft .mca file
    #[clap(required = true)]
    input_mca: PathBuf,

    /// Path to the "block" folder of a Minecraft ressourcepack
    #[clap(required = true)]
    block_textures: PathBuf,

    /// X-coordinate of the chunk
    #[clap(required = true)]
    cx: usize,

    /// Y-coordinate of the chunk
    #[clap(required = true)]
    cy: isize,

    /// Z-coordinate of the chunk
    #[clap(required = true)]
    cz: usize,

    /// X-coordinate of the chunk
    #[clap(required = true)]
    cx_end: usize,

    /// Y-coordinate of the chunk
    #[clap(required = true)]
    cy_end: isize,

    /// Z-coordinate of the chunk
    #[clap(required = true)]
    cz_end: usize,

    /// Path to the output MagicaVoxel .vox file
    #[clap(required = true)]
    output_vox: PathBuf,
}

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

fn main() {
    let args: Args = Args::parse();

    let in_file = std::fs::File::open(args.input_mca).unwrap();
    let mut region = fastanvil::Region::from_stream(in_file).unwrap();

    let mut voxels = Vec::with_capacity(
        (16 * 16 * 16)
            * (args.cx_end - args.cx + 1)
            * (args.cy_end - args.cy + 1) as usize
            * (args.cz_end - args.cz + 1),
    );
    let mut colors = Vec::new();
    let mut palette = HashMap::new();
    let transparent = [
        "air",
        "short_grass",
        "poppy",
        "azure_bluet",
        "dandelion",
        "cornflower",
        "oxeye_daisy",
    ];

    for (cx, cz) in iproduct!(args.cx..=args.cx_end, args.cz..=args.cz_end) {
        let data = region.read_chunk(cx, cz).unwrap().unwrap();
        let chunk = fastanvil::complete::Chunk::from_bytes(&data).unwrap();

        for cy in args.cy..=args.cy_end {
            for (x, y, z) in iproduct!(0..16, 0..16, 0..16) {
                let block = chunk.sections.block(x, y + cy * 16, z).unwrap();
                let name = &block.name()["minecraft:".len()..];

                if !transparent.contains(&name) {
                    let i = palette.get(name).copied().or_else(|| {
                        let color = block_avg_color(&args.block_textures, name)?;
                        println!("{:20}\t{:?}", name, color);
                        colors.push(color);
                        let i = palette.len() as u8;
                        palette.insert(name.to_string(), i);
                        Some(i)
                    });

                    if let Some(i) = i {
                        voxels.push(Voxel {
                            x: ((cx - args.cx) * 16 + x) as u8,
                            y: ((cz - args.cz) * 16 + z) as u8,
                            z: ((cy - args.cy) * 16 + y) as u8,
                            i,
                        });
                    }
                }
            }
        }
    }

    let model = Model {
        size: dot_vox::Size {
            x: 16 * (args.cx_end - args.cx + 1) as u32,
            y: 16 * (args.cz_end - args.cz + 1) as u32,
            z: 16 * (args.cy_end - args.cy + 1) as u32,
        },
        voxels,
    };

    let scene = SceneNode::Shape {
        attributes: Default::default(),
        models: vec![ShapeModel {
            model_id: 0,
            attributes: Default::default(),
        }],
    };

    let vox_data = DotVoxData {
        version: 150,
        models: vec![model],
        palette: colors,
        materials: vec![],
        scenes: vec![scene],
        layers: vec![],
    };

    let mut out_file = std::fs::File::create(args.output_vox).unwrap();
    vox_data.write_vox(&mut out_file).unwrap();
}
