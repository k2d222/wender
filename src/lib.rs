mod camera;
mod lights;
mod preproc;
mod ui;
mod voxels;
mod wgpu_util;

use std::{
    iter,
    sync::Arc,
    time::{Duration, Instant},
};

use ui::{run_egui, FpsCounter};
use wgpu::util::DeviceExt;
use winit::{
    dpi::LogicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    platform::x11::EventLoopBuilderExtX11,
    window::{Window, WindowBuilder},
};

use nalgebra_glm as glm;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::camera::{Camera, Controller};
use crate::lights::Lights;
use crate::{voxels::Voxels, wgpu_util::*};

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    wgpu_state: WgpuState,

    window: Arc<Window>,
    cursor_grabbed: bool,

    camera: Camera,
    lights: Lights,
    controller: Controller,

    egui_renderer: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
    fps: FpsCounter,
    last_frame: Instant,

    constants: ShaderConstants,
}

impl State {
    async fn new(window: Window) -> Self {
        let window = Arc::new(window);
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        println!("{:#?}", adapter.get_info());
        println!("{:#?}", adapter.limits());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                        | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_defaults()
                    } else {
                        // wgpu::Limits {
                        //     max_storage_buffer_binding_size: (1 << 30) * 2 - 1, // 5 GiB
                        //     max_buffer_size: (1 << 30) * 2 - 1,                 // 5 GiB
                        //     max_texture_dimension_3d: 2048,
                        //     ..Default::default()
                        // }
                        adapter.limits()
                    },
                    // memory_hints: wgpu::MemoryHints::Performance,
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
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        let camera = Camera::new(glm::vec2(size.width as f32, size.height as f32));
        let lights = Lights::new(
            f32::to_degrees(glm::half_pi()),
            f32::to_degrees(glm::quarter_pi()),
        );

        let voxels = Voxels::new();

        let controller = Controller::new();

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_config.format, None, 1);
        let egui_ctx = egui::Context::default();
        let fps = FpsCounter::new();

        let octree_depth = voxels.dim().ilog2() - 1;
        let svo_depth = 0;
        let grid_depth = 2;
        let dvo_depth = octree_depth - svo_depth - grid_depth;

        let constants = ShaderConstants {
            octree_depth,
            svo_depth,
            svo_max_iter: 200,
            dvo_depth,
            dvo_max_iter: 200,
            grid_depth,
            grid_max_iter: 2u32.pow(grid_depth) * 4,
            shadow_max_iter: 100,
            shadow_cone_angle: 1,
            shadow_strength: 10,
            ao_strength: 10,
            msaa_level: 1,
            debug_display: 0,
        };

        let wgpu_state = WgpuState::new(
            &device,
            &queue,
            &surface_config,
            &Buffers {
                camera: camera.as_bytes(),
                lights: lights.as_bytes(),
                voxels: voxels.voxels_bytes(),
                colors: voxels.colors_bytes(),
            },
            &constants,
        );

        {
            // compute svo on the gpu in the compute shader
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compute encoder"),
            });
            wgpu_state.compute_octree(&device, &mut encoder, voxels.dim());
            wgpu_state.compute_mipmap(&device, &mut encoder, voxels.dim());
            queue.submit(iter::once(encoder.finish()));
        }

        Self {
            window,
            cursor_grabbed: false,
            wgpu_state,
            surface,
            device,
            queue,
            size,
            config: surface_config,
            camera,
            lights,
            controller,
            egui_renderer,
            egui_ctx,
            fps,
            last_frame: Instant::now(),
            constants,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.camera.uniform.aspect = new_size.width as f32 / new_size.height as f32;
            self.camera.uniform.size = glm::vec2(new_size.width as f32, new_size.height as f32);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame);
        self.controller.update_camera(&mut self.camera);
        self.lights.update(dt);
    }

    fn render(&mut self, egui_state: &mut egui_winit::State) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render Encoder"),
            });

        self.draw_scene(&view, &mut encoder);
        self.draw_egui(egui_state, &view, &mut encoder);

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn draw_scene(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.wgpu_state.draw(view, encoder);
    }

    fn draw_egui(
        &mut self,
        egui_state: &mut egui_winit::State,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let egui_output = run_egui(self, egui_state);

        let egui_screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let egui_primitives = self
            .egui_ctx
            .tessellate(egui_output.shapes.clone(), egui_screen.pixels_per_point);

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            encoder,
            &egui_primitives,
            &egui_screen,
        );

        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            self.egui_renderer
                .render(&mut render_pass, &egui_primitives, &egui_screen);
        }

        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
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

    let event_loop = EventLoopBuilder::new()
        .with_x11()
        .build()
        .expect("failed to create event loop");
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

    event_loop
        .run(move |event, elwt| {
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
                                } else if event.state == ElementState::Pressed
                                    && matches!(
                                        event.physical_key,
                                        PhysicalKey::Code(KeyCode::KeyR)
                                    )
                                {
                                    state.wgpu_state.reload_shaders(
                                        &state.device,
                                        &state.config,
                                        &state.constants,
                                    );
                                } else {
                                    state.controller.process_keyboard(event);
                                }
                            }
                            WindowEvent::Resized(physical_size) => {
                                state.resize(*physical_size);
                            }
                            WindowEvent::MouseWheel { delta, .. } => match delta {
                                MouseScrollDelta::LineDelta(_, y) => {
                                    state.controller.speed *= 2f32.powf(-y);
                                }
                                MouseScrollDelta::PixelDelta(_) => {}
                            },
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
                                    Err(
                                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                                    ) => {
                                        state.resize(state.size);
                                    }
                                    Err(wgpu::SurfaceError::OutOfMemory) => {
                                        elwt.exit();
                                    }
                                    Err(wgpu::SurfaceError::Timeout) => {
                                        log::warn!("Surface timeout")
                                    }
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
                .write_buffer(&state.wgpu_state.camera_buffer, 0, state.camera.as_bytes());
            state
                .queue
                .write_buffer(&state.wgpu_state.lights_buffer, 0, state.lights.as_bytes());
        })
        .expect("event loop run failed");
}
