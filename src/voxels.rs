use nalgebra_glm as glm;
use ndarray::{Array3, Axis};
use rayon::prelude::{
    IndexedParallelIterator, IntoParallelIterator, ParallelBridge, ParallelIterator,
};
// use rayon::prelude::*;

pub struct Voxels {
    voxels: Array3<u32>,
    palette: Vec<glm::Vec4>,
    pub dim: glm::TVec3<u32>,
}
// the octants are "pointers" (offsets in the contiguous memory buffer) to a next node.
// at index 0 lies the root svo node.
// if an octant pointer is 0, then it means empty (sparse).
pub struct Svo1 {
    octants: [u32; 8],
}

impl Voxels {
    pub fn new() -> Self {
        // let dim = glm::vec3(512, 64, 512);

        // println!("Generating voxels...");
        // let voxels = (0..dim.product())
        //     .into_par_iter()
        //     .map(|i| {
        //         let z = i % dim.z;
        //         let y = (i / dim.z) % dim.y;
        //         let x = i / (dim.z * dim.y);
        //         let pos = glm::vec3(x as f32, y as f32, z as f32);
        //         let pos_n = pos.component_div(&dim.cast());
        //         let is_block =
        //             pos_n.y < (pos_n.x * 32.93).cos() * (pos_n.z * 17.39).sin() * 0.5 + 0.5;
        //         if is_block {
        //             1
        //         } else {
        //             0
        //         }
        //     })
        //     .collect();
        // println!("done");

        // let mut voxels = Array3::uninit((dim.x as usize, dim.y as usize, dim.z as usize))
        //     .indexed_iter_mut()
        //     .par_bridge()
        //     .for_each(|(dim, mut row)| {
        //     });

        let asset =
            dot_vox::load("assets/realistic_terrain.vox").expect("failed to load magicvoxel asset");
        let model = asset
            .models
            .get(0)
            .expect("expected 1 model in the asset file");

        let dim = glm::vec3(model.size.x, model.size.y, model.size.z);
        println!("dim: {dim}");

        let mut voxels = Array3::zeros((dim.x as usize, dim.y as usize, dim.z as usize));
        model.voxels.iter().for_each(|v| {
            voxels[(v.x as usize, v.z as usize, v.y as usize)] = v.i as u32 + 1;
        });

        let palette = asset
            .palette
            .iter()
            .map(|c| {
                glm::vec4(
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                )
            })
            .collect();

        Self {
            voxels,
            palette,
            dim,
        }
    }

    // pub fn build_svo(&self) -> Svo {
    //     let leaves = self
    // }

    pub fn voxels_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.voxels.as_slice().unwrap())
    }

    pub fn palette_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.palette.as_slice())
    }
}
