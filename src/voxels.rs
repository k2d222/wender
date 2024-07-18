use std::{fs::File, io::BufReader};

use nalgebra_glm as glm;
use ndarray::{s, Array3, Zip};

#[derive(Debug)]
pub struct Voxels {
    voxels: Array3<u8>,
    colors: Array3<glm::U8Vec4>,
}

impl Voxels {
    pub fn new() -> Self {
        let asset_file = File::open("assets/minecraft_511.wvox").expect("missing asset file");
        let asset_file = BufReader::new(asset_file);
        let (vox, palette): (Array3<u32>, Vec<[u8; 4]>) =
            bincode::deserialize_from(asset_file).expect("failed to load asset");

        // round up to pow of 2
        let dim = vox.shape().iter().max().unwrap();
        let max_dim: usize = 2 << (dim - 1).ilog2();
        println!(
            "dim: {dim:?} ({max_dim}) -> dvo_depth = {}",
            max_dim.ilog2() - 1
        );
        let mut voxels = Array3::zeros((max_dim, max_dim, max_dim));
        voxels
            .slice_mut(s![..vox.dim().0, ..vox.dim().1, ..vox.dim().2])
            .assign(&vox.mapv(|x| x as u8));
        println!(
            "mem: {}B = {}MiB",
            voxels.len() * 4,
            voxels.len() * 4 / 1024 / 1024
        );

        let colors = Zip::from(&voxels).par_map_collect(|i| {
            if *i == 0 {
                Default::default()
            } else {
                glm::U8Vec4::from(palette[*i as usize - 1])
            }
        });

        Self { voxels, colors }
    }

    pub fn dim(&self) -> u32 {
        self.voxels.dim().0 as u32
    }

    pub fn voxels_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.voxels.as_slice().unwrap())
    }

    pub fn colors_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.colors.as_slice().unwrap())
    }
}
