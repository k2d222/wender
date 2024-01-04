// !! careful with the alignments! add padding fields if necessary.
// see https://www.w3.org/TR/WGSL/#alignment-and-size
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pos: glm::Vec3,
    fov_y: f32,
    view_mat_inv: glm::Mat4x4,
}

struct Camera {
    uniform: CameraUniform,
    quat: glm::Quat,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            uniform: CameraUniform {
                pos: glm::Vec3::new(0.0, 0.0, 5.0),
                fov_y: 70.0 / 180.0 * glm::pi::<f32>(),
                view_mat_inv: Default::default(),
            },
            quat: glm::Quat::identity(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.uniform)
    }
}
