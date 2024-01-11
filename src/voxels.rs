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
            dot_vox::load("assets/realistic_terrain.vox").expect("failed to load magicvoxel asset");
        let model = asset
            .models
            .get(0)
            .expect("expected 1 model in the asset file");

        let dim = (
            model.size.x as usize,
            model.size.y as usize,
            model.size.z as usize,
        );
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
        fn leaf_svo(voxels: ArrayView3<u32>) -> SvoNode {
            SvoNode {
                octants: [
                    voxels[(0, 0, 0)],
                    voxels[(0, 0, 1)],
                    voxels[(0, 1, 0)],
                    voxels[(0, 1, 1)],
                    voxels[(1, 0, 0)],
                    voxels[(1, 0, 1)],
                    voxels[(1, 1, 0)],
                    voxels[(1, 1, 1)],
                ],
            }
        }

        fn node_svo(voxels: ArrayView3<SvoNode>) -> SvoNode {
            SvoNode {
                octants: [
                    if voxels[(0, 0, 0)].is_empty() { 0 } else { 1 },
                    if voxels[(0, 0, 1)].is_empty() { 0 } else { 1 },
                    if voxels[(0, 1, 0)].is_empty() { 0 } else { 1 },
                    if voxels[(0, 1, 1)].is_empty() { 0 } else { 1 },
                    if voxels[(1, 0, 0)].is_empty() { 0 } else { 1 },
                    if voxels[(1, 0, 1)].is_empty() { 0 } else { 1 },
                    if voxels[(1, 1, 0)].is_empty() { 0 } else { 1 },
                    if voxels[(1, 1, 1)].is_empty() { 0 } else { 1 },
                ],
            }
        }

        let l0 = Zip::from(voxels.exact_chunks((2, 2, 2))).par_map_collect(|o| leaf_svo(o));
        let l1 = Zip::from(l0.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l2 = Zip::from(l1.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l3 = Zip::from(l2.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l4 = Zip::from(l3.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l5 = Zip::from(l4.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l6 = Zip::from(l5.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));
        let l7 = Zip::from(l6.exact_chunks((2, 2, 2))).par_map_collect(|o| node_svo(o));

        let mut vec = Vec::new();
        let mut ptr = 0u32;

        let mut update_indices = |l: Array3<SvoNode>| {
            // ptr += l.iter().filter(|o| o.is_full()).count() as u32;
            l.into_iter().for_each(|mut o| {
                if o.is_full() {
                    o.octants.iter_mut().filter(|p| **p != 0).for_each(|p| {
                        ptr += 1;
                        *p = ptr;
                    });
                    vec.push(o);
                }
            });
        };

        update_indices(l7);
        update_indices(l6);
        update_indices(l5);
        update_indices(l4);
        update_indices(l3);
        update_indices(l2);
        update_indices(l1);
        vec.extend(l0.into_iter());

        println!("{} {}", vec.len(), ptr);

        vec
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
