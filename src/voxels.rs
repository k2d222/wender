use nalgebra_glm as glm;
use rayon::prelude::*;

pub struct Voxels {
    voxels: Vec<u32>, // contiguous in z, then y, then x
    pub dim: glm::TVec3<u32>,
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

        let asset =
            dot_vox::load("assets/realistic_terrain.vox").expect("failed to load magicvoxel asset");
        let model = asset
            .models
            .get(0)
            .expect("expected 1 model in the asset file");

        let dim = glm::vec3(model.size.x, model.size.y, model.size.z);
        println!("dim: {dim}");

        let mut voxels = vec![0; dim.product() as usize];
        model.voxels.iter().for_each(|v| {
            let i = (v.y as u32 + v.z as u32 * dim.z + v.x as u32 * dim.z * dim.y) as usize;
            voxels[i] = v.i as u32 + 1;
        });

        Self { voxels, dim }
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.voxels.as_slice())
    }
}
