use std::ops::Deref;

use dot_vox::{Dict, DotVoxData, Frame, Model, SceneNode, ShapeModel};
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

fn compute_dotvox_model(asset: &DotVoxData) -> Array3<u32> {
    fn process_node<'a>(
        res: &mut Vec<(&'a Model, glm::IVec3)>,
        asset: &'a DotVoxData,
        node: &SceneNode,
        mut tr: glm::IVec3,
    ) {
        match node {
            dot_vox::SceneNode::Transform {
                attributes: _,
                frames,
                child,
                layer_id,
            } => {
                let frame = &frames[0];
                if let Some(pos) = frame.position() {
                    tr += glm::vec3(pos.x, pos.y, pos.z);
                }
                let child = &asset.scenes[*child as usize];
                process_node(res, asset, child, tr);
            }
            dot_vox::SceneNode::Group {
                attributes: _,
                children,
            } => {
                for child in children {
                    let child = &asset.scenes[*child as usize];
                    process_node(res, asset, child, tr.clone());
                }
            }
            dot_vox::SceneNode::Shape {
                attributes: _,
                models,
            } => {
                for model in models {
                    let index = model.model_id as usize;
                    let model = &asset.models[index];
                    res.push((model, tr.clone()));
                }
            }
        };
    }

    let models = if let Some(root) = asset.scenes.get(0) {
        let mut models = Vec::new();
        process_node(&mut models, &asset, root, glm::vec3(0, 0, 0));
        models
    } else {
        asset
            .models
            .iter()
            .map(|m| (m, glm::vec3(0, 0, 0)))
            .collect()
    };

    let dim = models
        .iter()
        .map(|(m, tr)| {
            let size = glm::vec3(m.size.x as i32, m.size.y as i32, m.size.z as i32);
            size + tr
        })
        .reduce(|v1, v2| glm::max2(&v1, &v2))
        .unwrap();

    // round up to pow of 2
    let max_dim = 2 << dim.max().ilog2();

    println!("dim: {dim:?} ({max_dim})");

    let mut voxels = Array3::zeros((max_dim, max_dim, max_dim));

    models.iter().for_each(|(m, tr)| {
        m.voxels.iter().for_each(|v| {
            voxels[(
                (v.x as i32 + tr.x) as usize,
                (v.z as i32 + tr.z) as usize,
                (v.y as i32 + tr.y) as usize,
            )] = v.i as u32 + 1;
        })
    });

    voxels
}

impl Voxels {
    pub fn new() -> Self {
        let asset =
            dot_vox::load("assets/christmas_scene.vox").expect("failed to load magicvoxel asset");

        let voxels = compute_dotvox_model(&asset);

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

        // let svo = Self::build_svo(&voxels);
        let svo = Self::fractal_svo();

        Self {
            voxels,
            svo,
            palette,
        }
    }

    pub fn build_svo(voxels: &Array3<u32>) -> Vec<SvoNode> {
        let svo_depth = voxels.dim().0.ilog2() as usize;
        println!("depth: {svo_depth}");

        let mut levels = vec![voxels.clone()];

        // build the sparse layers (filled octant = 1, empty = 0)
        for n in 1..svo_depth {
            let prev_level = &levels[n - 1];
            let level = Zip::from(prev_level.exact_chunks((2, 2, 2)))
                .par_map_collect(|o| o.iter().any(|v| *v != 0) as u32);
            levels.push(level);
        }

        println!("computed layers");

        let mut ptr = 13u32;

        // update octants pointers
        levels.iter_mut().rev().take(svo_depth - 1).for_each(|l| {
            l.iter_mut().filter(|o| **o != 0).for_each(|o| {
                ptr += 1;
                *o = ptr;
            });
        });

        println!("computed pointers");

        let mut vec = vec![
            SvoNode {
                octants: [1, 1, 1, 1, 1, 1, 1, 1],
            },
            SvoNode {
                octants: [2, 2, 2, 2, 2, 2, 2, 2],
            },
            SvoNode {
                octants: [3, 3, 3, 3, 3, 3, 3, 3],
            },
            SvoNode {
                octants: [4, 4, 4, 4, 4, 4, 4, 4],
            },
            SvoNode {
                octants: [5, 5, 5, 5, 5, 5, 5, 5],
            },
            SvoNode {
                octants: [6, 6, 6, 6, 6, 6, 6, 6],
            },
            SvoNode {
                octants: [7, 7, 7, 7, 7, 7, 7, 7],
            },
            SvoNode {
                octants: [8, 8, 8, 8, 8, 8, 8, 8],
            },
            SvoNode {
                octants: [9, 9, 9, 9, 9, 9, 9, 9],
            },
            SvoNode {
                octants: [10, 10, 10, 10, 10, 10, 10, 10],
            },
            SvoNode {
                octants: [11, 11, 11, 11, 11, 11, 11, 11],
            },
            SvoNode {
                octants: [12, 12, 12, 12, 12, 12, 12, 12],
            },
            SvoNode {
                octants: [13, 13, 13, 13, 13, 13, 13, 13],
            },
        ];

        // build up the vec of nodes
        levels.iter().rev().for_each(|l| {
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
        });

        println!("computed nodes");

        vec
    }

    #[allow(unused)]
    fn fractal_svo() -> Vec<SvoNode> {
        vec![
            SvoNode {
                octants: [1, 0, 0, 1, 0, 1, 1, 0],
            },
            SvoNode {
                octants: [1, 0, 0, 1, 0, 1, 1, 0],
            },
        ]
    }

    pub fn svo_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.svo.as_slice())
    }

    pub fn palette_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.palette.as_slice())
    }
}
