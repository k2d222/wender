mod camera;
mod ui;
mod voxels;
mod wgpu_util;

use std::iter;

use ui::FpsCounter;
use wgpu::util::DeviceExt;
use winit::{
    dpi::LogicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

use nalgebra_glm as glm;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::{
    camera::{Camera, Controller},
    wgpu_util::init_voxels_buffers,
};
use crate::{voxels::Voxels, wgpu_util::init_camera_buffers};

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
    voxels_bind_group: wgpu::BindGroup,

    window: Window,
    cursor_grabbed: bool,

    camera: Camera,
    controller: Controller,

    egui_renderer: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
    fps: FpsCounter,
}

impl State {
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
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
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None, // trace_path
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
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera = Camera::new();

        let (camera_buffer, camera_bind_group, camera_bind_group_layout) =
            init_camera_buffers(&device, camera.as_bytes());

        let voxels = Voxels::new();

        let (_voxels_buffer, _palette_buffer, voxels_bind_group, voxels_bind_group_layout) =
            init_voxels_buffers(&device, voxels.voxels_bytes(), voxels.palette_bytes());

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &voxels_bind_group_layout],
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
                    format: surface_config.format,
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

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1);
        let egui_ctx = egui::Context::default();
        let fps = FpsCounter::new();

        Self {
            surface,
            device,
            queue,
            size,
            config: surface_config,
            render_pipeline,
            vertex_buffer,
            camera_buffer,
            camera_bind_group,
            voxels_bind_group,
            window,
            cursor_grabbed: false,
            camera,
            controller,
            egui_renderer,
            egui_ctx,
            fps,
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

    fn update(&mut self) {
        self.controller.update_camera(&mut self.camera);
    }

    fn prepare_egui(
        &mut self,
        egui_state: &mut egui_winit::State,
    ) -> (
        egui_wgpu::renderer::ScreenDescriptor,
        Vec<egui::ClippedPrimitive>,
        egui::FullOutput,
    ) {
        let raw_input = egui_state.take_egui_input(&self.window);

        self.fps.tick();

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Window::new("Hello").show(&ctx, |ui| {
                egui_plot::Plot::new("FPS")
                    .height(100.0)
                    .include_y(0)
                    .include_y(100)
                    .include_x(0)
                    .include_x(100)
                    .auto_bounds(false.into())
                    .show(ui, |ui| {
                        let durations = self.fps.durations();
                        let points = durations
                            .iter()
                            .enumerate()
                            .map(|(n, d)| [n as f64, 1000.0 / d.as_millis() as f64])
                            .collect::<egui_plot::PlotPoints>();
                        ui.line(egui_plot::Line::new(points));
                    });
                ui.label("hello");
                let _ = ui.button("world");
            });
        });

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [
                self.window.inner_size().width,
                self.window.inner_size().height,
            ],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let tess = self.egui_ctx.tessellate(
            full_output.shapes.clone(),
            screen_descriptor.pixels_per_point,
        );

        (screen_descriptor, tess, full_output)
    }

    fn render(&mut self, egui_state: &mut egui_winit::State) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let (egui_screen, egui_primitives, egui_output) = self.prepare_egui(egui_state);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &egui_primitives,
            &egui_screen,
        );

        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.voxels_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            render_pass.draw(0..6, 0..1);

            self.egui_renderer
                .render(&mut render_pass, &egui_primitives, &egui_screen);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

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

    let event_loop = EventLoop::new().expect("failed to create event loop");
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

    let mut state = State::new(window).await;

    let mut egui_state = egui_winit::State::new(
        state.egui_ctx.clone(),
        state.egui_ctx.viewport_id(),
        &event_loop,
        None,
        None,
    );

    event_loop.set_control_flow(ControlFlow::Wait);

    event_loop.run(move |event, elwt| {
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
                let egui_winit::EventResponse {
                    consumed,
                    repaint: _,
                } = egui_state.on_window_event(&state.window, event);

                if !consumed {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::KeyboardInput { event, .. } => {
                            if event.state == ElementState::Pressed
                                && event.logical_key == Key::Named(NamedKey::Escape)
                            {
                                state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None)
                                    .ok();
                                state.window.set_cursor_visible(true);
                                state.cursor_grabbed = false;
                            } else {
                                state.controller.process_keyboard(event);
                            }
                        }
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
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
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render(&mut egui_state) {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    state.resize(state.size);
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    elwt.exit();
                                }
                                Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                state.window.request_redraw();
            }
            _ => {}
        }

        state
            .queue
            .write_buffer(&state.camera_buffer, 0, state.camera.as_bytes());
    });
}
