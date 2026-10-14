use crate::{
    import_job::ImportJob,
    model::{self, Model},
    renderer::{Camera, DisplayMode, RenderSettings, Renderer},
};
use eframe::{egui, egui_glow};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct Viewer {
    renderer: Arc<Mutex<Renderer>>,
    gl: Arc<eframe::glow::Context>,
    model: Model,
    camera: Camera,
    settings: RenderSettings,
    receiver: Option<ImportJob>,
    path: Option<PathBuf>,
    loading_name: String,
    error: Option<String>,
    status: String,
    tolerance: f64,
    aspect: f32,
    smoke: Option<SmokeTest>,
}

struct SmokeTest {
    started: Instant,
    requested: bool,
    previous: Option<Vec<egui::Color32>>,
    viewport: egui::Rect,
    compact: bool,
}

impl Viewer {
    pub fn new(
        context: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let gl = context.gl.clone().ok_or("OpenGL is unavailable")?;
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(31, 34, 33);
        visuals.window_fill = visuals.panel_fill;
        visuals.selection.bg_fill = egui::Color32::from_rgb(41, 105, 88);
        context.egui_ctx.set_visuals(visuals);
        let model = model::demo();
        let mut renderer = Renderer::new(&gl)?;
        renderer.upload(&gl, &model);
        let mut viewer = Self {
            renderer: Arc::new(Mutex::new(renderer)),
            gl,
            model,
            camera: Camera::default(),
            settings: RenderSettings::default(),
            receiver: None,
            path: None,
            loading_name: String::new(),
            error: None,
            status: "Ready".into(),
            tolerance: 0.001,
            aspect: 1.5,
            smoke: None,
        };
        let mut arguments = std::env::args_os().skip(1);
        if let Some(argument) = arguments.next() {
            if argument == "--smoke-test" || argument == "--smoke-test-compact" {
                let compact = argument == "--smoke-test-compact";
                viewer.smoke = Some(SmokeTest {
                    started: Instant::now(),
                    requested: false,
                    previous: None,
                    viewport: egui::Rect::NOTHING,
                    compact,
                });
                viewer.settings.grid = false;
                if compact {
                    context
                        .egui_ctx
                        .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            680.0, 480.0,
                        )));
                }
                if let Some(path) = arguments.next() {
                    viewer.open(PathBuf::from(path), &context.egui_ctx);
                }
            } else {
                viewer.open(PathBuf::from(argument), &context.egui_ctx);
            }
        }
        Ok(viewer)
    }

    fn open(&mut self, path: PathBuf, context: &egui::Context) {
        if self.receiver.is_some() {
            return;
        }
        self.loading_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.error = None;
        match ImportJob::start(path, self.tolerance) {
            Ok(job) => self.receiver = Some(job),
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        context.request_repaint();
    }

    fn file_dialog(&mut self, context: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("3D models", model::EXTENSIONS)
            .pick_file()
        {
            self.open(path, context);
        }
    }

    fn poll_load(&mut self) {
        let Some(receiver) = &mut self.receiver else {
            return;
        };
        match receiver.poll() {
            Ok(Some(model)) => {
                self.path = Some(receiver.path.clone());
                self.status = format!("Loaded in {:.2}s", receiver.elapsed().as_secs_f64());
                self.renderer.lock().unwrap().upload(&self.gl, &model);
                self.model = model;
                self.camera.fit(self.aspect);
                self.receiver = None;
            }
            Err(error) => {
                self.receiver = None;
                self.error = Some(format!("{error:#}"));
                self.status = "Import failed".into();
            }
            Ok(None) => {}
        }
    }

    fn smoke_test(&mut self, context: &egui::Context) {
        let Some(smoke) = &mut self.smoke else {
            return;
        };
        assert!(
            smoke.started.elapsed() < Duration::from_secs(60),
            "Native smoke test timed out"
        );
        assert!(
            self.error.is_none(),
            "Smoke-test import failed: {:?}",
            self.error
        );
        context.request_repaint_after(Duration::from_millis(30));
        let screenshot = context.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = screenshot {
            let pixels_per_point = context.pixels_per_point();
            let left = (smoke.viewport.left() * pixels_per_point) as usize;
            let right = ((smoke.viewport.right() * pixels_per_point) as usize).min(image.width());
            let top = (smoke.viewport.top() * pixels_per_point) as usize;
            let bottom =
                ((smoke.viewport.bottom() * pixels_per_point) as usize).min(image.height());
            let pixels: Vec<_> = (top..bottom)
                .flat_map(|row| {
                    image.pixels[row * image.width() + left..row * image.width() + right]
                        .iter()
                        .copied()
                })
                .collect();
            let background = pixels[0];
            let visible = pixels
                .iter()
                .filter(|pixel| {
                    pixel.r().abs_diff(background.r()) as u32
                        + pixel.g().abs_diff(background.g()) as u32
                        + pixel.b().abs_diff(background.b()) as u32
                        > 40
                })
                .count();
            assert!(
                visible > 300,
                "Native viewport is blank: only {visible} visible pixels"
            );
            std::fs::create_dir_all("target").expect("screenshot directory");
            let stage = if smoke.previous.is_some() {
                "moved"
            } else {
                "initial"
            };
            let size = if smoke.compact { "compact" } else { "desktop" };
            image::save_buffer(
                format!("target/smoke-{size}-{stage}.png"),
                image.as_raw(),
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .expect("write screenshot");
            if let Some(previous) = &smoke.previous {
                let changed = previous
                    .iter()
                    .zip(&pixels)
                    .filter(|(before, after)| before != after)
                    .count();
                assert!(
                    changed > 300,
                    "Camera movement did not change rendered geometry"
                );
                println!("Native smoke test passed ({size}): {visible} geometry pixels, {changed} changed pixels");
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                smoke.previous = Some(pixels);
                smoke.requested = false;
                self.camera.orbit([80.0, 25.0]);
                self.camera.pan([15.0, 5.0], smoke.viewport.height());
                self.camera.distance *= 0.85;
            }
        }
        if !smoke.requested && self.receiver.is_none() && smoke.viewport.is_positive() {
            smoke.requested = true;
            context.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("MODEL")
                .small()
                .color(egui::Color32::from_rgb(120, 176, 153)),
        );
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&self.model.name).strong().size(17.0));
        if let Some(path) = &self.path {
            ui.label(egui::RichText::new(path.to_string_lossy()).small().weak());
        }
        ui.add_space(12.0);
        for warning in &self.model.warnings {
            ui.colored_label(egui::Color32::from_rgb(245, 186, 87), warning);
        }
        egui::Grid::new("statistics")
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Vertices");
                ui.label(self.model.vertices.len().to_string());
                ui.end_row();
                ui.label("Triangles");
                ui.label((self.model.indices.len() / 3).to_string());
                ui.end_row();
                for (label, dimension) in ["X", "Y", "Z"]
                    .into_iter()
                    .zip(self.model.dimensions.to_array())
                {
                    ui.label(format!("Extent {label}"));
                    ui.label(format!("{dimension:.4}"));
                    ui.end_row();
                }
            });
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(egui::RichText::new("DISPLAY").small().strong());
        ui.add_space(8.0);
        ui.checkbox(&mut self.settings.grid, "Grid");
        ui.checkbox(&mut self.settings.colors, "File colors");
        ui.checkbox(&mut self.settings.flat, "Flat shading");
        ui.checkbox(&mut self.camera.orthographic, "Orthographic");
        ui.horizontal(|ui| {
            ui.label("Up axis");
            if ui
                .selectable_value(&mut self.camera.z_up, true, "Z")
                .clicked()
                || ui
                    .selectable_value(&mut self.camera.z_up, false, "Y")
                    .clicked()
            {
                self.camera.target = glam::Vec3::ZERO;
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.color_edit_button_rgb(&mut self.settings.tint);
            ui.label("Model color");
        });
        ui.horizontal(|ui| {
            ui.color_edit_button_rgb(&mut self.settings.background);
            ui.label("Background");
        });
        ui.add(egui::Slider::new(&mut self.settings.point_size, 1.0..=12.0).text("Point size"));
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(egui::RichText::new("CAD IMPORT").small().strong());
        ui.add(egui::Slider::new(&mut self.tolerance, 0.0001..=0.01).logarithmic(true).text("Tolerance"))
            .on_hover_text("Relative STEP tessellation tolerance. Smaller values create finer meshes. Applies on the next import.");
        if ui
            .add_enabled(
                self.path.is_some() && self.receiver.is_none(),
                egui::Button::new("Reload model"),
            )
            .clicked()
        {
            self.open(self.path.clone().unwrap(), context);
        }
        ui.add_space(16.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Front").clicked() {
                self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                self.camera.pitch = 0.0;
            }
            if ui.button("Right").clicked() {
                self.camera.yaw = 0.0;
                self.camera.pitch = 0.0;
            }
            if ui.button("Top").clicked() {
                self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                self.camera.pitch = 1.55;
            }
            if ui.button("Isometric").clicked() {
                self.camera.yaw = -0.9;
                self.camera.pitch = 0.5;
            }
        });
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load();
        let loading = self.receiver.is_some();
        if loading {
            context.request_repaint_after(Duration::from_millis(50));
        }
        if !loading {
            let dropped = context.input(|input| {
                input
                    .raw
                    .dropped_files
                    .iter()
                    .find_map(|file| file.path.clone())
            });
            if let Some(path) = dropped {
                self.open(path, context);
            }
            let open =
                context.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::O));
            if open {
                self.file_dialog(context);
            }
        }
        if !context.wants_keyboard_input() && context.input(|input| input.key_pressed(egui::Key::F))
        {
            self.camera.fit(self.aspect);
        }
        egui::TopBottomPanel::top("toolbar").show(context, |ui| {
            ui.add_space(5.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("MODEL VIEWER").strong());
                ui.separator();
                if ui
                    .add_enabled(!loading, egui::Button::new("Open model"))
                    .on_hover_text("Open model (Ctrl+O)")
                    .clicked()
                {
                    self.file_dialog(context);
                }
                if ui
                    .button("Fit view")
                    .on_hover_text("Fit model (F)")
                    .clicked()
                {
                    self.camera.fit(self.aspect);
                }
                if ui.button("Reset view").clicked() {
                    self.camera = Camera::default();
                    self.camera.fit(self.aspect);
                }
                ui.separator();
                ui.selectable_value(&mut self.settings.mode, DisplayMode::Solid, "Solid");
                ui.selectable_value(&mut self.settings.mode, DisplayMode::Wireframe, "Wireframe");
                ui.selectable_value(&mut self.settings.mode, DisplayMode::Points, "Points");
            });
            ui.add_space(5.0);
        });
        egui::TopBottomPanel::bottom("status").show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                if loading {
                    ui.spinner();
                    if let Some(job) = &self.receiver {
                        ui.label(format!(
                            "Loading {} ({:.0}s)",
                            self.loading_name,
                            job.elapsed().as_secs_f64()
                        ));
                        let progress = job.progress();
                        let bar = egui::ProgressBar::new(progress.fraction.unwrap_or(0.0))
                            .desired_width(180.0)
                            .animate(true);
                        ui.add(bar);
                        ui.label(progress.stage);
                    }
                    if ui.button("Cancel").clicked() {
                        self.receiver = None;
                        self.status = "Import cancelled".into();
                    }
                } else {
                    ui.label(&self.status);
                }
                ui.separator();
                if !self.model.warnings.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(245, 186, 87), "INCOMPLETE MODEL");
                }
                ui.label(if self.model.indices.is_empty() {
                    "Point cloud"
                } else {
                    "Triangle mesh"
                });
            });
        });
        egui::SidePanel::left("inspector")
            .default_width(235.0)
            .min_width(210.0)
            .max_width(360.0)
            .resizable(true)
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui, context));
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(context, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());
            self.aspect = (rect.width() / rect.height().max(1.0)).max(0.01);
            if let Some(smoke) = &mut self.smoke { smoke.viewport = rect; }
            if response.dragged() {
                let delta = context.input(|input| input.pointer.delta());
                let pan = response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary) || context.input(|input| input.modifiers.shift);
                if pan { self.camera.pan([delta.x, delta.y], rect.height()); }
                else { self.camera.orbit([delta.x, delta.y]); }
            }
            if response.hovered() {
                let scroll = context.input(|input| input.smooth_scroll_delta.y);
                self.camera.distance = (self.camera.distance * (-scroll * 0.003).exp()).clamp(0.05, 200.0);
            }
            response.on_hover_text("Left drag: orbit\nRight / middle / Shift drag: pan\nScroll: zoom\nF: fit view\nDrop a model to open");
            let camera = self.camera.clone();
            let settings = RenderSettings {
                mode: self.settings.mode, grid: self.settings.grid, colors: self.settings.colors,
                flat: self.settings.flat, point_size: self.settings.point_size,
                background: self.settings.background, tint: self.settings.tint,
            };
            let renderer = self.renderer.clone();
            let aspect = self.aspect;
            ui.painter().add(egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                    renderer.lock().unwrap().paint(painter.gl(), &camera, &settings, aspect, info.pixels_per_point);
                })),
            });
        });
        if let Some(error) = self.error.clone() {
            let mut open = true;
            egui::Window::new("Cannot open model")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(context, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(&error);
                    if ui.button("Close").clicked() {
                        self.error = None;
                    }
                });
            if !open {
                self.error = None;
            }
        }
        self.smoke_test(context);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.receiver = None;
        if let Some(gl) = gl {
            self.renderer.lock().unwrap().destroy(gl);
        }
    }
}
