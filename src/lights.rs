use nalgebra_glm as glm;

// !! careful with the alignments! add padding fields if necessary.
// see https://www.w3.org/TR/WGSL/#alignment-and-size
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightsUniform {
    pub sun_dir: glm::Vec3,
    _pad: [f32; 1], // padding to ensure correct alignment
}

pub struct Lights {
    pub uniform: LightsUniform,
    pub angle: f32,   // degrees
    pub azimuth: f32, // degrees
}

fn from_angle_azimuth(angle: f32, azimuth: f32) -> glm::Vec3 {
    let angle_rad = f32::to_radians(angle);
    let azimuth_rad = f32::to_radians(azimuth);

    return glm::normalize(&glm::vec3(
        f32::cos(angle_rad) * f32::cos(azimuth_rad),
        f32::sin(azimuth_rad),
        f32::sin(angle_rad) * f32::cos(azimuth_rad),
    ));
}

impl Lights {
    pub fn new(angle: f32, azimuth: f32) -> Self {
        Self {
            uniform: LightsUniform {
                sun_dir: from_angle_azimuth(angle, azimuth),
                _pad: Default::default(),
            },
            angle,
            azimuth,
        }
    }

    pub fn update(&mut self) {
        self.uniform.sun_dir = from_angle_azimuth(self.angle, self.azimuth)
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.uniform)
    }
}
