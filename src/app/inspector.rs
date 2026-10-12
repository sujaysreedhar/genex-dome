use super::*;

impl DomeApp {
    pub(super) fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(300.)
            .min_width(270.)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(14.);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Projection, "Projection");
                    ui.selectable_value(&mut self.tab, Tab::Calibration, "Calibration");
                    ui.selectable_value(&mut self.tab, Tab::Output, "Output");
                });
                ui.add_space(12.);
                egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                    Tab::Projection => self.projection_ui(ui),
                    Tab::Calibration => self.calibration_ui(ui),
                    Tab::Output => self.output_ui(ui),
                });
            });
    }
    fn projection_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Source & view");
        ui.add_space(8.);
        ui.label("Input projection");
        let old_input = self.session.view.input;
        egui::ComboBox::from_id_salt("input")
            .selected_text(self.session.view.input.label())
            .width(240.)
            .show_ui(ui, |ui| {
                for projection in Projection::ALL {
                    ui.selectable_value(
                        &mut self.session.view.input,
                        projection,
                        projection.label(),
                    );
                }
            });
        if old_input != self.session.view.input
            && self.session.view.input == Projection::Perspective
        {
            self.session.view.source_fov = 90.;
        }
        if let Some(index) = self.playback.selected {
            self.session.playlist[index].projection = self.session.view.input;
        }
        ui.add_space(10.);
        ui.label("Output projection");
        egui::ComboBox::from_id_salt("output")
            .selected_text(self.session.view.output.label())
            .width(240.)
            .show_ui(ui, |ui| {
                for output in Output::ALL {
                    ui.selectable_value(&mut self.session.view.output, output, output.label());
                }
            });
        ui.add_space(15.);
        let view = &mut self.session.view;
        let projected = view.output != Output::Source && view.input != Projection::Prewarped;
        if projected {
            ui.label("Orientation");
            ui.add(egui::Slider::new(&mut view.yaw, -180. ..=180.).text("Yaw °"));
            ui.add(egui::Slider::new(&mut view.pitch, -90. ..=90.).text("Pitch °"));
            ui.add(egui::Slider::new(&mut view.roll, -180. ..=180.).text("Roll °"));
            if ui.button("Reset orientation").clicked() {
                view.yaw = 0.;
                view.pitch = 0.;
                view.roll = 0.;
            }
            ui.add_space(12.);
            if matches!(view.output, Output::Fisheye | Output::Mirror) {
                ui.add(egui::Slider::new(&mut view.fisheye_fov, 90. ..=240.).text("Dome FOV °"));
            }
            if matches!(
                view.input,
                Projection::Perspective | Projection::Fisheye | Projection::Cylindrical
            ) {
                let maximum = if view.input == Projection::Perspective {
                    178.
                } else {
                    360.
                };
                ui.add(
                    egui::Slider::new(&mut view.source_fov, 10. ..=maximum).text("Source FOV °"),
                );
            }
            if view.output == Output::Perspective {
                ui.add(
                    egui::Slider::new(&mut view.perspective_fov, 20. ..=150.).text("View FOV °"),
                );
            }
        }
        ui.add_space(12.);
        ui.label("Image correction");
        ui.add(egui::Slider::new(&mut view.brightness, 0. ..=2.).text("Brightness"));
        ui.add(egui::Slider::new(&mut view.gamma, 0.2..=3.).text("Gamma"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut view.flip_x, "Flip horizontal");
            ui.checkbox(&mut view.flip_y, "Vertical");
        });
        ui.add_space(12.);
        ui.collapsing("Advanced · lens & stereo", |ui| {
            if projected {
                ui.label("Off-centre fisheye lens");
                ui.add(egui::Slider::new(&mut view.offset[0], -0.65..=0.65).text("Offset X / R"));
                ui.add(egui::Slider::new(&mut view.offset[1], -0.65..=0.65).text("Offset Y / R"));
            }
            ui.label("Stereoscopic video");
            stereo_combo(ui, "stereo in", "Input", &mut view.stereo_input);
            stereo_combo(ui, "stereo out", "Output", &mut view.stereo_output);
        });
        ui.collapsing("Backdrop for standard media", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Load backdrop").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Fisheye image", &["png", "jpg", "jpeg", "tga"])
                        .pick_file()
                    {
                        self.session.view.background = Some(path);
                        self.refresh_background();
                    }
                }
                if ui.button("Clear").clicked() {
                    self.session.view.background = None;
                    self.refresh_background();
                }
            });
        });
        ui.separator();
        ui.label("Dome message");
        ui.add(
            egui::TextEdit::multiline(&mut self.session.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Show warped message").clicked() {
            // Render before replacing the active source, so bad backdrop paths do not destroy it.
            let mut candidate = self.session.clone();
            candidate.built_in_source = Some("text".into());
            match session_frame(&candidate) {
                Ok(frame) => {
                    self.playback.reset_source("Dome message".into());
                    self.error = None;
                    self.session.built_in_source = candidate.built_in_source;
                    self.session.view.input = Projection::Fisheye;
                    self.session.view.source_fov = 180.;
                    self.set_frame(frame);
                    self.status = "Built-in presentation source".into();
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn calibration_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Warp calibration");
        ui.add_space(8.);
        ui.horizontal(|ui| {
            if ui.button("Load .data").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Bourke warp mesh", &["data"])
                    .pick_file()
                {
                    self.load_mesh(path);
                }
            }
            if ui
                .add_enabled(self.session.mesh.is_some(), egui::Button::new("Save .data"))
                .clicked()
            {
                if let Some(mesh) = &self.session.mesh {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("warpmap.data")
                        .save_file()
                    {
                        match mesh.save(&path) {
                            Ok(()) => {
                                self.mesh_history.mark_saved(&self.session.mesh);
                                self.status = "Calibration saved".into();
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.mesh_history.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                self.mesh_history.undo(&mut self.session.mesh);
                self.mesh_revision = true;
                self.node = 0;
            }
            if ui
                .add_enabled(self.mesh_history.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                self.mesh_history.redo(&mut self.session.mesh);
                self.mesh_revision = true;
                self.node = 0;
            }
            if self.mesh_history.modified(&self.session.mesh) {
                ui.colored_label(ACCENT, "Modified");
            }
        });
        if let Some(mesh) = &self.session.mesh {
            ui.label(format!(
                "{} × {} nodes · {:.3}:1",
                mesh.width, mesh.height, mesh.aspect
            ));
            ui.label(format!(
                "{} active nodes / {}",
                mesh.nodes
                    .iter()
                    .filter(|node| node.intensity >= 0.)
                    .count(),
                mesh.nodes.len()
            ));
        }
        ui.add_space(12.);
        ui.collapsing("Dome & mirror geometry", |ui| {
            let optics = &mut self.session.optics;
            ui.label("Metres · origin at dome centre");
            ui.add(
                egui::DragValue::new(&mut optics.dome_radius)
                    .speed(0.01)
                    .range(0.01..=100.)
                    .prefix("Dome radius  "),
            );
            vec3_ui(ui, "Dome centre", &mut optics.dome_center);
            ui.add(
                egui::DragValue::new(&mut optics.dome_tilt)
                    .speed(0.1)
                    .prefix("Dome tilt °  "),
            );
            ui.add(
                egui::DragValue::new(&mut optics.mirror_radius)
                    .speed(0.001)
                    .range(0.01..=10.)
                    .prefix("Mirror radius  "),
            );
            vec3_ui(ui, "Mirror centre", &mut optics.mirror_center);
        });
        ui.collapsing("Projector optics", |ui| {
            let optics = &mut self.session.optics;
            vec3_ui(ui, "Projector position", &mut optics.projector);
            ui.add(
                egui::DragValue::new(&mut optics.projector_yaw)
                    .speed(0.1)
                    .prefix("Yaw correction °  "),
            );
            ui.add(
                egui::DragValue::new(&mut optics.projector_pitch)
                    .speed(0.1)
                    .prefix("Tilt correction °  "),
            );
            ui.add(
                egui::DragValue::new(&mut optics.throw_ratio)
                    .speed(0.01)
                    .range(0.1..=20.)
                    .prefix("Throw ratio  "),
            );
            ui.add(
                egui::DragValue::new(&mut optics.lens_shift)
                    .speed(0.01)
                    .prefix("Lens shift  "),
            );
        });
        ui.collapsing("Mesh resolution & aspect", |ui| {
            let optics = &mut self.session.optics;
            ui.horizontal(|ui| {
                for (label, aspect) in [
                    ("16:9", 16. / 9.),
                    ("16:10", 1.6),
                    ("4:3", 4. / 3.),
                    ("5:4", 1.25),
                ] {
                    if ui.button(label).clicked() {
                        optics.aspect = aspect;
                    }
                }
            });
            ui.add(
                egui::DragValue::new(&mut optics.aspect)
                    .speed(0.01)
                    .range(0.2..=5.)
                    .prefix("Aspect  "),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut optics.grid_width)
                        .range(2..=256)
                        .prefix("Columns "),
                );
                ui.add(
                    egui::DragValue::new(&mut optics.grid_height)
                        .range(2..=256)
                        .prefix("Rows "),
                );
            });
            ui.checkbox(
                &mut optics.brightness_correction,
                "Estimate brightness compensation",
            );
        });
        ui.add_space(8.);
        if ui.add(accent_button("Generate warp preview")).clicked() {
            self.candidate_mesh = None;
            match self.session.optics.generate().and_then(|mesh| {
                MeshHistory::validate_candidate(&mesh)?;
                Ok(mesh)
            }) {
                Ok(mesh) => {
                    self.candidate_mesh = Some(mesh);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if let Some(mesh) = &self.candidate_mesh {
            ui.label("Warp preview · current output unchanged");
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 125.), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3., Color32::BLACK);
            for triangle in mesh.triangles().as_chunks::<3>().0 {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    ui.painter().line_segment(
                        [
                            node_position(&triangle[a], mesh.aspect, rect),
                            node_position(&triangle[b], mesh.aspect, rect),
                        ],
                        egui::Stroke::new(0.5_f32, ACCENT),
                    );
                }
            }
            ui.label(format!(
                "{} active nodes",
                mesh.nodes
                    .iter()
                    .filter(|node| node.intensity >= 0.)
                    .count()
            ));
            ui.horizontal(|ui| {
                if ui.add(accent_button("Apply warp")).clicked() {
                    if let Some(mesh) = self.candidate_mesh.take() {
                        if self.apply_mesh(mesh) {
                            self.status =
                                "Warp applied · Undo restores the previous calibration".into();
                        }
                    }
                }
                if ui.button("Discard preview").clicked() {
                    self.candidate_mesh = None;
                }
            });
        }
        if ui.button("Create flat / truncated mapping").clicked() {
            let optics = &self.session.optics;
            self.apply_mesh(WarpMesh::identity(
                optics.grid_width,
                optics.grid_height,
                optics.aspect,
            ));
        }
        ui.separator();
        ui.checkbox(&mut self.edit_mesh, "Edit mesh nodes on preview");
        ui.label(
            RichText::new("Drag a node or edit its coordinates. Undo restores the previous edit.")
                .small()
                .color(Color32::GRAY),
        );
        self.mesh_history.begin_edit(&self.session.mesh);
        if let Some(mesh) = &mut self.session.mesh {
            self.node = self.node.min(mesh.nodes.len() - 1);
            ui.add(
                egui::DragValue::new(&mut self.node)
                    .range(0..=mesh.nodes.len() - 1)
                    .prefix("Node  "),
            );
            let node = &mut mesh.nodes[self.node];
            let mut changed = false;
            for (value, label) in [
                (&mut node.x, "Screen X  "),
                (&mut node.y, "Screen Y  "),
                (&mut node.u, "Texture U  "),
                (&mut node.v, "Texture V  "),
            ] {
                changed |= ui
                    .add(egui::DragValue::new(value).speed(0.001).prefix(label))
                    .changed();
            }
            changed |= ui
                .add(egui::Slider::new(&mut node.intensity, -1. ..=1.).text("Intensity"))
                .changed();
            self.mesh_revision |= changed;
        }
        ui.separator();
        if ui.button("Show polar grid").clicked() {
            self.show_pattern(Pattern::PolarGrid);
        }
    }
    fn output_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Projector output");
        ui.add_space(8.);
        ui.label("Output display");
        let old_display = self.session.output_display.clone();
        let selected = self
            .displays
            .iter()
            .find(|display| Some(&display.id) == old_display.as_ref())
            .map_or_else(
                || {
                    if old_display.is_some() {
                        "Display disconnected".into()
                    } else {
                        "Manual position".into()
                    }
                },
                |display| display.label(),
            );
        egui::ComboBox::from_id_salt("display picker")
            .width(240.)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.session.output_display, None, "Manual position");
                for display in &self.displays {
                    ui.selectable_value(
                        &mut self.session.output_display,
                        Some(display.id.clone()),
                        display.label(),
                    );
                }
            });
        if old_display != self.session.output_display {
            self.output_placed = false;
        }
        ui.horizontal(|ui| {
            if ui.button("Refresh displays").clicked() {
                self.displays = crate::displays::list();
            }
            if ui
                .add_enabled(
                    self.session.output_display.is_some(),
                    egui::Button::new("Identify display"),
                )
                .clicked()
            {
                self.projector = true;
                self.identify_until = Some(Instant::now() + Duration::from_secs(3));
            }
        });
        if self.displays.is_empty() {
            ui.small(
                "Automatic display selection is available on Windows. Use manual placement here.",
            );
        }
        ui.checkbox(&mut self.projector, "Open separate output window");
        ui.checkbox(
            &mut self.projector_fullscreen,
            "Fullscreen on output display",
        );
        ui.label("Display position (logical pixels)");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.session.output_position[0])
                    .speed(1.)
                    .prefix("X "),
            );
            ui.add(
                egui::DragValue::new(&mut self.session.output_position[1])
                    .speed(1.)
                    .prefix("Y "),
            );
        });
        ui.label("Window size");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.session.output_size[0])
                    .range(64. ..=8192.)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.session.output_size[1])
                    .range(64. ..=8192.)
                    .prefix("H "),
            );
        });
        ui.label(RichText::new("Set X/Y inside the projector display, then enable fullscreen. Use extended desktop mode.").small().color(Color32::GRAY));
        ui.checkbox(&mut self.session.view.blackout, "Blackout projector [B]");
        ui.separator();
        ui.heading("Frame export");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.export_size[0])
                    .range(64..=8192)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.export_size[1])
                    .range(64..=8192)
                    .prefix("H "),
            );
        });
        if ui
            .add_enabled(
                self.exporting.is_none(),
                egui::Button::new("Export projected PNG"),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name("dome-output.png")
                .save_file()
            {
                self.export(path);
            }
        }
        if self.exporting.is_some() {
            ui.spinner();
            ui.label("Rendering export…");
        }
        ui.separator();
        ui.heading("Live screen warping");
        ui.label("Capture a fisheye region from another app");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.capture_rect[0]).prefix("X "));
            ui.add(egui::DragValue::new(&mut self.capture_rect[1]).prefix("Y "));
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.capture_rect[2])
                    .range(64..=8192)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.capture_rect[3])
                    .range(64..=8192)
                    .prefix("H "),
            );
        });
        if ui
            .button(if self.playback.capture_active {
                "Stop capture"
            } else {
                "Start capture"
            })
            .clicked()
        {
            if self.playback.capture_active {
                self.stop();
            } else {
                self.playback.reset_source("Live screen capture".into());
                self.error = None;
                match Decoder::capture(self.capture_rect, 30) {
                    Ok(decoder) => {
                        self.playback.decoder = Some(decoder);
                        self.playback.playing = true;
                        self.playback.capture_active = true;
                        self.status = "Capturing selected desktop region".into();
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        ui.label(
            RichText::new("Keep this console outside the capture region to avoid a feedback loop.")
                .small()
                .color(Color32::GRAY),
        );
        ui.separator();
        ui.heading("Playback quality");
        let old_height = self.playback.max_height;
        egui::ComboBox::from_id_salt("decode height")
            .selected_text(format!("{} px maximum height", self.playback.max_height))
            .show_ui(ui, |ui| {
                for height in [720, 1080, 2160, 4320] {
                    ui.selectable_value(
                        &mut self.playback.max_height,
                        height,
                        format!("{height} px"),
                    );
                }
            });
        if self.playback.max_height != old_height {
            self.seek(self.playback.position);
        }
    }
}
