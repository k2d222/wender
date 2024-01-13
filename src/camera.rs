use nalgebra_glm as glm;

use winit::{
    event::*,
    keyboard::{KeyCode, PhysicalKey},
};

// !! careful with the alignments! add padding fields if necessary.
// see https://www.w3.org/TR/WGSL/#alignment-and-size
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub pos: glm::Vec3,
    pub fov_y: f32,
    pub aspect: f32,
    _pad: [f32; 3], // padding to ensure correct alignment
    pub view_mat_inv: glm::Mat4x4,
}

pub struct Camera {
    pub uniform: CameraUniform,
    pub quat: glm::Quat,
}

pub struct Controller {
    speed: f32,
    sensitivity: f64,
    is_forward: bool,
    is_back: bool,
    is_left: bool,
    is_right: bool,
    is_up: bool,
    is_down: bool,
    mouse_pos: (f64, f64),
}

impl Camera {
    pub fn new() -> Self {
        Self {
            uniform: CameraUniform {
                pos: glm::Vec3::new(0.0, 20.0, -5.0),
                fov_y: 70.0 / 180.0 * glm::pi::<f32>(),
                aspect: 1.0,
                _pad: Default::default(),
                view_mat_inv: Default::default(),
            },
            quat: glm::Quat::identity(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.uniform)
    }
}

impl Controller {
    pub fn new() -> Self {
        Self {
            speed: 5.0,
            sensitivity: 0.005,
            is_forward: false,
            is_back: false,
            is_left: false,
            is_right: false,
            is_up: false,
            is_down: false,
            mouse_pos: (0.0, 0.0),
        }
    }

    pub fn process_keyboard(&mut self, input: &KeyEvent) {
        let pressed = input.state == ElementState::Pressed;

        match input.physical_key {
            PhysicalKey::Code(KeyCode::KeyW) => {
                self.is_forward = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyA) => {
                self.is_left = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyS) => {
                self.is_back = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyD) => {
                self.is_right = pressed;
            }
            PhysicalKey::Code(KeyCode::Space) => {
                self.is_up = pressed;
            }
            PhysicalKey::Code(KeyCode::ShiftLeft) => {
                self.is_down = pressed;
            }
            _ => {}
        }
    }

    pub fn process_mouse(&mut self, delta: (f64, f64)) {
        self.mouse_pos.0 += delta.0;
        self.mouse_pos.1 += delta.1;
    }

    pub fn update_camera(&mut self, cam: &mut Camera) {
        let half_angle_x = (self.mouse_pos.1 * self.sensitivity * 0.5) as f32;
        let half_angle_y = (self.mouse_pos.0 * self.sensitivity * 0.5) as f32;
        cam.quat = glm::Quat::new(half_angle_y.cos(), 0.0, half_angle_y.sin(), 0.0)
            * glm::Quat::new(half_angle_x.cos(), half_angle_x.sin(), 0.0, 0.0);

        if self.is_forward {
            let dir = glm::quat_cast(&cam.quat) * glm::vec4(0.0, 0.0, 1.0, 0.0);
            cam.uniform.pos += dir.xyz() * self.speed;
        }
        if self.is_back {
            let dir = glm::quat_cast(&cam.quat) * glm::vec4(0.0, 0.0, 1.0, 0.0);
            cam.uniform.pos -= dir.xyz() * self.speed;
        }
        if self.is_left {
            let dir = glm::quat_cast(&cam.quat) * glm::vec4(1.0, 0.0, 0.0, 0.0);
            cam.uniform.pos -= dir.xyz() * self.speed;
            // let half_angle = -self.speed.to_radians() * 2.0;
            // cam.quat *= glm::Quat::new(half_angle.cos(), 0.0, half_angle.sin(), 0.0)
        }
        if self.is_right {
            let dir = glm::quat_cast(&cam.quat) * glm::vec4(1.0, 0.0, 0.0, 0.0);
            cam.uniform.pos += dir.xyz() * self.speed;
            // let half_angle = self.speed.to_radians() * 2.0;
            // cam.quat *= glm::Quat::new(half_angle.cos(), 0.0, half_angle.sin(), 0.0)
        }
        if self.is_up {
            cam.uniform.pos.y += self.speed;
        }
        if self.is_down {
            cam.uniform.pos.y -= self.speed;
        }

        cam.uniform.view_mat_inv = glm::quat_cast(&cam.quat);
    }
}
