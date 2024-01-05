use std::iter;

use wgpu::util::DeviceExt;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use nalgebra_glm as glm;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    blocks_bind_group: wgpu::BindGroup,
    window: Window,
    cursor_grabbed: bool,

    camera: Camera,
    controller: Controller,
}

// !! careful with the alignments! add padding fields if necessary.
// see https://www.w3.org/TR/WGSL/#alignment-and-size
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pos: glm::Vec3,
    fov_y: f32,
    aspect: f32,
    _pad: [f32; 3], // padding to ensure correct alignment
    view_mat_inv: glm::Mat4x4,
}

struct Camera {
    uniform: CameraUniform,
    quat: glm::Quat,
}

struct Controller {
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
            speed: 0.5,
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

    fn process_keyboard(&mut self, input: &KeyboardInput) {
        let pressed = input.state == ElementState::Pressed;

        match input.virtual_keycode {
            Some(VirtualKeyCode::W) => {
                self.is_forward = pressed;
            }
            Some(VirtualKeyCode::A) => {
                self.is_left = pressed;
            }
            Some(VirtualKeyCode::S) => {
                self.is_back = pressed;
            }
            Some(VirtualKeyCode::D) => {
                self.is_right = pressed;
            }
            Some(VirtualKeyCode::Space) => {
                self.is_up = pressed;
            }
            Some(VirtualKeyCode::LShift) => {
                self.is_down = pressed;
            }
            _ => {}
        }
    }

    fn process_mouse(&mut self, delta: (f64, f64)) {
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

// !! careful with the alignments! add padding fields if necessary.
// see https://www.w3.org/TR/WGSL/#alignment-and-size
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlocksUniform {
    blocks: [[[u32; 16]; 16]; 16],
}

struct Blocks {
    uniform: BlocksUniform,
}

impl Blocks {
    pub fn new() -> Self {
        let mut blocks = [[[0; 16]; 16]; 16];

        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    blocks[x][y][z] = if rand::random::<f32>() < 0.2 { 1 } else { 0 };
                }
            }
        }

        Self {
            uniform: BlocksUniform { blocks },
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.uniform)
    }
}

impl State {
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // --- camera

        let camera = Camera::new();

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: camera.as_bytes(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // --- blocks

        let blocks = Blocks::new();

        let blocks_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blocks buffer"),
            contents: blocks.as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let blocks_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let blocks_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blocks bind group"),
            layout: &blocks_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: blocks_buffer.as_entire_binding(),
            }],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &blocks_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<glm::Vec2>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });

        const BUF_DATA: &[glm::Vec2] = &[
            glm::Vec2::new(-1.0, -1.0),
            glm::Vec2::new(1.0, -1.0),
            glm::Vec2::new(1.0, 1.0),
            glm::Vec2::new(-1.0, -1.0),
            glm::Vec2::new(1.0, 1.0),
            glm::Vec2::new(-1.0, 1.0),
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(BUF_DATA),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let controller = Controller::new();

        Self {
            surface,
            device,
            queue,
            size,
            config,
            render_pipeline,
            vertex_buffer,
            camera_buffer,
            camera_bind_group,
            blocks_bind_group,
            window,
            cursor_grabbed: false,
            camera,
            controller,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.camera.uniform.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        self.controller.update_camera(&mut self.camera);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.blocks_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Could't initialize logger");
        } else {
            env_logger::init();
        }
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Wender")
        .with_inner_size(LogicalSize::new(800.0, 800.0))
        .build(&event_loop)
        .unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        // Winit prevents sizing with CSS, so we have to set
        // the size manually when on web.
        use winit::dpi::PhysicalSize;
        window.set_inner_size(PhysicalSize::new(450, 400));

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    // State::new uses async code, so we're going to wait for it to finish
    let mut state = State::new(window).await;

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::DeviceEvent { ref event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    if state.cursor_grabbed {
                        state.controller.process_mouse(*delta);
                    }
                }
                _ => {}
            },
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::KeyboardInput { input, .. } => {
                            if input.state == ElementState::Pressed
                                && input.virtual_keycode == Some(VirtualKeyCode::Escape)
                            {
                                state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None)
                                    .ok();
                                state.window.set_cursor_visible(true);
                                state.cursor_grabbed = false;
                            } else {
                                state.controller.process_keyboard(input);
                            }
                        }
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        WindowEvent::MouseInput {
                            state: button_state,
                            button,
                            ..
                        } => {
                            if *button_state == ElementState::Pressed
                                && *button == MouseButton::Left
                            {
                                state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                                    .ok();
                                state.window.set_cursor_visible(false);
                                state.cursor_grabbed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == state.window.id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.resize(state.size)
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                state.window.request_redraw();
            }
            _ => {}
        }

        state
            .queue
            .write_buffer(&state.camera_buffer, 0, state.camera.as_bytes());
    });
}
