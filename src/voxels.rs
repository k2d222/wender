use std::ops::Deref;

use nalgebra_glm as glm;
use ndarray::{s, Array3, ArrayView3, Axis, Zip};
use rayon::prelude::{
    IndexedParallelIterator, IntoParallelIterator, ParallelBridge, ParallelIterator,
};
// use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct Voxels {
    voxels: Array3<u32>,
    svo: Vec<SvoNode>,
    palette: Vec<glm::Vec4>,
}
// the octants are "pointers" (offsets in the contiguous memory buffer) to a next node.
// at index 0 lies the root svo node.
// if an octant pointer is 0, then it means empty (sparse).
// octants are indexed in Array3's logical order, i.e. row major, i.e. z varies first.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SvoNode {
    octants: [u32; 8],
}

impl SvoNode {
    fn is_empty(&self) -> bool {
        self.octants.iter().all(|o| *o == 0)
    }
    fn is_full(&self) -> bool {
        self.octants.iter().any(|o| *o != 0)
    }
}

impl Voxels {
    pub fn new() -> Self {
        let asset =
            dot_vox::load("assets/christmas_scene.vox").expect("failed to load magicvoxel asset");
        let model = asset
            .models
            .get(0)
            .expect("expected 1 model in the asset file");

        let max_dim = model.size.x.max(model.size.y).max(model.size.z);
        let pow2 = *[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024]
            .iter()
            .find(|x| **x >= max_dim)
            .expect("unsupported model dimensions") as usize;

        let dim = (pow2, pow2, pow2);
        println!("dim: {dim:?}");

        let mut voxels = Array3::zeros(dim);
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

        let svo = Self::build_svo(&voxels);

        Self {
            voxels,
            svo,
            palette,
        }
    }

    pub fn build_svo(voxels: &Array3<u32>) -> Vec<SvoNode> {
        let mut l1 = Zip::from(voxels.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        let mut l2 = Zip::from(l1.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        let mut l3 = Zip::from(l2.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        let mut l4 = Zip::from(l3.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        let mut l5 = Zip::from(l4.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        let mut l6 = Zip::from(l5.exact_chunks((2, 2, 2)))
            .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
        // let mut l7 = Zip::from(l6.exact_chunks((2, 2, 2)))
        //     .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);

        let mut ptr = 0u32;

        let mut update_indices = |l: &mut Array3<u32>| {
            l.iter_mut().filter(|o| **o != 0).for_each(|o| {
                ptr += 1;
                *o = ptr;
            });
        };

        // update_indices(&mut l7);
        update_indices(&mut l6);
        update_indices(&mut l5);
        update_indices(&mut l4);
        update_indices(&mut l3);
        update_indices(&mut l2);
        update_indices(&mut l1);

        let mut vec = Vec::new();

        let mut build_svo = |l: &Array3<u32>| {
            l.exact_chunks((2, 2, 2))
                .into_iter()
                .filter(|o| o.iter().any(|o| *o != 0))
                .for_each(|o| {
                    let svo = SvoNode {
                        octants: [
                            o[(0, 0, 0)],
                            o[(0, 0, 1)],
                            o[(0, 1, 0)],
                            o[(0, 1, 1)],
                            o[(1, 0, 0)],
                            o[(1, 0, 1)],
                            o[(1, 1, 0)],
                            o[(1, 1, 1)],
                        ],
                    };
                    vec.push(svo);
                });
        };

        // build_svo(&l7);
        build_svo(&l6);
        build_svo(&l5);
        build_svo(&l4);
        build_svo(&l3);
        build_svo(&l2);
        build_svo(&l1);
        build_svo(voxels);

        vec

        // vec![
        //     SvoNode {
        //         octants: [1, 0, 0, 1, 0, 1, 1, 0],
        //     },
        //     SvoNode {
        //         octants: [2, 0, 0, 2, 0, 2, 2, 0],
        //     },
        //     SvoNode {
        //         octants: [3, 0, 0, 3, 0, 3, 3, 0],
        //     },
        //     SvoNode {
        //         octants: [4, 0, 0, 4, 0, 4, 4, 0],
        //     },
        //     SvoNode {
        //         octants: [5, 0, 0, 5, 0, 5, 5, 0],
        //     },
        //     SvoNode {
        //         octants: [6, 0, 0, 6, 0, 6, 6, 0],
        //     },
        //     SvoNode {
        //         octants: [7, 0, 0, 7, 0, 7, 7, 0],
        //     },
        //     SvoNode {
        //         octants: [1, 1, 1, 1, 1, 1, 1, 1],
        //     },
        // ]
    }

    pub fn voxels_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.voxels.as_slice().unwrap())
    }

    pub fn svo_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.svo.as_slice())
    }

    pub fn palette_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.palette.as_slice())
    }
}
