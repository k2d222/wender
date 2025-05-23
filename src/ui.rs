use std::time::{Duration, Instant};

use itertools::Itertools;

use crate::State;

pub struct FpsCounter {
    history: [Instant; Self::HISTORY_SIZE],
    ptr: usize,
}

impl FpsCounter {
    const HISTORY_SIZE: usize = 64;

    pub fn new() -> Self {
        Self {
            history: [Instant::now(); Self::HISTORY_SIZE],
            ptr: 1,
        }
    }

    pub fn len(&self) -> usize {
        Self::HISTORY_SIZE
    }

    pub fn tick(&mut self) {
        self.history[self.ptr] = Instant::now();
        self.ptr += 1;
        if self.ptr == Self::HISTORY_SIZE {
            self.ptr = 0;
        }
    }

    pub fn durations(&self) -> Vec<Duration> {
        self.history[self.ptr..]
            .iter()
            .chain(self.history[0..self.ptr].iter())
            .tuple_windows()
            .map(|(s1, s2)| s2.duration_since(*s1))
            .collect()
    }
}

pub fn run_egui(state: &mut State, egui_state: &mut egui_winit::State) -> egui::FullOutput {
    let raw_input = egui_state.take_egui_input(&state.window);

    state.fps.tick();

    let full_output = state.egui_ctx.run(raw_input, |ctx| {
        let fps = state.fps.durations();
        let avg_fps = 10000 / fps.iter().rev().take(10).sum::<Duration>().as_millis();

        egui::Window::new("Debug").show(&ctx, |ui| {
            egui_plot::Plot::new("FPS")
                .height(100.0)
                .include_y(0)
                .include_y(70)
                .include_x(0)
                .include_x(state.fps.len() as f64)
                .auto_bounds(false.into())
                .show(ui, |ui| {
                    let points = fps
                        .iter()
                        .enumerate()
                        .map(|(n, d)| [n as f64, 1000.0 / d.as_millis() as f64])
                        .collect::<egui_plot::PlotPoints>();
                    ui.line(egui_plot::Line::new(points));
                });
            ui.label(format!("fps: {}", avg_fps));
            ui.label(format!("cam: {:?}", state.camera.uniform.pos));
            ui.label(format!("speed: {}", state.controller.speed));
        });

        egui::Window::new("Controls").show(&ctx, |ui| {
            let c = &mut state.constants;
            ui.add(egui::Slider::new(&mut c.octree_depth, 0..=10).text("octree depth"));
            ui.add(egui::Slider::new(&mut c.svo_depth, 0..=10).text("svo depth"));
            ui.add(egui::Slider::new(&mut c.svo_max_iter, 0..=500).text("svo max iter"));
            ui.add(egui::Slider::new(&mut c.dvo_depth, 0..=10).text("dvo depth"));
            ui.add(egui::Slider::new(&mut c.dvo_max_iter, 0..=500).text("dvo max iter"));
            ui.add(egui::Slider::new(&mut c.grid_depth, 0..=10).text("grid depth"));
            ui.add(egui::Slider::new(&mut c.grid_max_iter, 0..=500).text("grid max iter"));
            ui.add(egui::Slider::new(&mut c.shadow_max_iter, 0..=1000).text("shadow max iter"));
            ui.add(egui::Slider::new(&mut c.shadow_cone_angle, 0..=180).text("shadow cone angle"));
            ui.add(egui::Slider::new(&mut c.shadow_strength, 0..=20).text("shadow strength"));
            ui.add(egui::Slider::new(&mut c.ao_strength, 0..=20).text("ao strength"));
            ui.add(egui::Slider::new(&mut c.debug_display, 0..=3).text("debug display"));
            ui.add(egui::Slider::new(&mut c.msaa_level, 0..=4).text("MSAA level"));
            ui.add(egui::Slider::new(&mut state.lights.angle, 0.0..=360.0).text("angle"));
            ui.add(egui::Slider::new(&mut state.lights.azimuth, 0.0..=90.0).text("azimuth"));
            ui.add(egui::Slider::new(&mut state.lights.speed, 0.0..=10.0).text("speed"));
        });
    });

    full_output
}
