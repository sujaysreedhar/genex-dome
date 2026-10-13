use super::*;

impl DomeApp {
    pub(super) fn show_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Show production");
        ui.label("Select a cue in the library to edit its settings.");
        ui.add_space(10.);
        if let Some(index) = self
            .playback
            .selected
            .filter(|i| *i < self.session.playlist.len())
        {
            ui.label(RichText::new(format!("CUE {:02}", index + 1)).color(ACCENT));
            let item = &mut self.session.playlist[index];
            ui.label("Cue name");
            ui.text_edit_singleline(&mut item.cue.name);
            let mut timed = item.cue.duration_seconds.is_some();
            if ui.checkbox(&mut timed, "Custom duration").changed() {
                item.cue.duration_seconds = timed.then_some(self.session.slide_seconds as f64);
            }
            if let Some(duration) = &mut item.cue.duration_seconds {
                ui.add(
                    egui::DragValue::new(duration)
                        .range(0.1..=86400.)
                        .speed(0.1)
                        .suffix(" seconds"),
                );
                ui.label(
                    RichText::new("Movie: stop at this media time. Still: hold for this long.")
                        .small()
                        .color(Color32::GRAY),
                );
            }
            let mut fade = item.cue.fade_seconds > 0.;
            if ui
                .checkbox(&mut fade, "Fade through black on entry")
                .changed()
            {
                item.cue.fade_seconds = if fade { 1. } else { 0. };
            }
            if fade {
                ui.add(
                    egui::Slider::new(&mut item.cue.fade_seconds, 0.1..=10.).text("Fade seconds"),
                );
                ui.label(
                    RichText::new(
                        "Half out, half in. Loading holds black; audio follows the fade.",
                    )
                    .small()
                    .color(Color32::GRAY),
                );
            }
            ui.add_space(8.);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Store current view").clicked() {
                    item.projection = self.session.view.input;
                    item.cue.view = Some(crate::state::CueView::capture(&self.session.view));
                    self.status = "Cue projection and orientation stored".into();
                }
                if ui
                    .add_enabled(item.cue.view.is_some(), egui::Button::new("Clear preset"))
                    .clicked()
                {
                    item.cue.view = None;
                }
            });
            if let Some(preset) = &item.cue.view {
                ui.label(format!(
                    "{} · yaw {:.0}° · pitch {:.0}° · roll {:.0}°",
                    item.projection.label(),
                    preset.yaw,
                    preset.pitch,
                    preset.roll
                ));
            } else {
                ui.label("Uses the session's default source view.");
            }
            if ui.button("Use current view as show default").clicked() {
                self.cue_base = crate::state::CueView::capture(&self.session.view);
                self.status = "Default source view updated".into();
            }
        }
        ui.separator();
        ui.label(self.preloader.status());
        ui.label(format!(
            "Cue elapsed: {}{}",
            timecode(self.sequencer.elapsed),
            if self.sequencer.transitioning() {
                " · transitioning"
            } else {
                ""
            }
        ));
        ui.label(RichText::new("Auto advance runs the cue list. Loop repeats the show; without Auto advance, Loop repeats the current cue.").small().color(Color32::GRAY));
        ui.separator();
        ui.collapsing("Portable show", |ui| {
        let assets = crate::bundle::check(&self.session);
        ui.label(format!(
            "{} local cues · {} network cues · {} missing assets",
            assets.local_cues,
            assets.network_cues,
            assets.missing.len()
        ));
        if !assets.missing.is_empty() {
            ui.collapsing("Missing assets", |ui| {
                for path in &assets.missing {
                    ui.label(path.display().to_string());
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open bundle").clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    self.load_session(folder.join(crate::bundle::MANIFEST));
                }
            }
            if ui.button("Relink folder").clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut candidate = self.session.clone();
                    match crate::bundle::relink_folder(&mut candidate, &folder) {
                        Ok(report) => {
                            let selected = self.playback.selected;
                            let active_changed = selected.is_some_and(|index| {
                                candidate.playlist[index].path != self.session.playlist[index].path
                            });
                            self.session = candidate;
                            self.preloader.clear();
                            if active_changed {
                                if let Some(index) = selected {
                                    self.select(index);
                                }
                            }
                            self.refresh_background();
                            self.status = format!(
                                "Relinked {} assets · {} unresolved",
                                report.linked,
                                report.unresolved.len()
                            );
                        }
                        Err(error) => self.error = Some(format!("{error:#}")),
                    }
                }
            }
        });
        ui.label("New bundle folder name");
        ui.text_edit_singleline(&mut self.bundle_name);
        let name = self.bundle_name.trim();
        let valid = !name.is_empty()
            && std::path::Path::new(name).components().count() == 1
            && matches!(
                std::path::Path::new(name).components().next(),
                Some(std::path::Component::Normal(_))
            );
        if ui
            .add_enabled(
                valid && self.bundle_job.is_none() && assets.missing.is_empty(),
                egui::Button::new("Export show bundle…"),
            )
            .clicked()
        {
            if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                let destination = parent.join(name);
                let session = self.session_for_save();
                let (tx, rx) = mpsc::channel();
                self.bundle_job = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(crate::bundle::export(&session, &destination));
                });
                self.status = "Copying show assets…".into();
            }
        }
        if self.bundle_job.is_some() {
            ui.spinner();
        }
        ui.label(RichText::new("Copies local media and backdrop with calibration and cue settings. Network cues still require internet.").small().color(Color32::GRAY));
        });
        ui.separator();
        ui.heading("Playback diagnostics");
        if let Some(stats) = &self.diagnostics.video {
            ui.label(format!(
                "{} × {} · source {:.2} fps",
                stats.width, stats.height, stats.source_fps
            ));
            ui.label(format!(
                "Decoded {:.1} fps · delivered {:.1} fps",
                stats.decoded_fps(),
                stats.presented_fps()
            ));
            ui.label(format!(
                "Frames {} · delivered {}",
                stats.decoded, stats.presented
            ));
            ui.label(format!(
                "Dropped: {} late · {} replaced before delivery",
                stats.dropped_late, stats.dropped_unpresented
            ));
        } else {
            ui.label("Play a movie to collect frame statistics.");
        }
        ui.label(format!(
            "Texture upload: {:.2} ms average · {:.2} ms max",
            self.diagnostics.upload_average_ms(),
            self.diagnostics.upload_max_ms
        ));
        match self.diagnostics.av_offset_ms {
            Some(ms) => {
                ui.label(format!("Video minus audio clock: {ms:+.1} ms"));
            }
            None => {
                ui.label("Audio clock: unavailable");
            }
        }
        ui.label(RichText::new("Rates average the current decode run. Delivery is to the console, not measured projector refresh. Upload timing is CPU submission time.").small().color(Color32::GRAY));
        if ui.button("Save diagnostics JSON").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("playback-diagnostics.json")
                .save_file()
            {
                match self.diagnostics.save(&path) {
                    Ok(()) => self.status = "Diagnostics saved".into(),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
    }
    pub(super) fn session_for_save(&self) -> Session {
        let mut session = self.session.clone();
        // Save the default source view independently of the currently selected cue preset.
        if self
            .playback
            .selected
            .is_some_and(|index| self.session.playlist[index].cue.view.is_some())
        {
            self.cue_base.apply(&mut session.view);
        }
        session
    }
}
