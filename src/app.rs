mod inspector;

use crate::{
    calibration::MeshHistory,
    media::{self, Decoder},
    mesh::WarpMesh,
    playback::Playback,
    projection::{self, Pattern},
    renderer::{self, Renderer},
    state::{Output, PlaylistItem, Projection, Session, Stereo},
};
use eframe::{
    egui::{self, Color32, RichText, Vec2},
    glow,
};
use image::RgbaImage;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

const ACCENT: Color32 = Color32::from_rgb(231, 163, 78);

#[derive(Default)]
pub struct Launch {
    pub session: Session,
    pub autoplay: bool,
    pub projector: bool,
    pub fullscreen: bool,
    pub projector_windowed: bool,
    pub screenshot: Option<PathBuf>,
    pub gpu_check: Option<PathBuf>,
    pub panel: Option<String>,
}
#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Projection,
    Calibration,
    Output,
}

pub struct DomeApp {
    pub session: Session,
    renderer: Arc<Mutex<Renderer>>,
    gl: Arc<glow::Context>,
    frame: RgbaImage,
    playback: Playback,
    mesh_history: MeshHistory,
    candidate_mesh: Option<WarpMesh>,
    preview_source: bool,
    status: String,
    error: Option<String>,
    tab: Tab,
    projector: bool,
    projector_fullscreen: bool,
    displays: Vec<crate::displays::Display>,
    output_placed: bool,
    display_check: Instant,
    identify_until: Option<Instant>,
    fullscreen: bool,
    edit_mesh: bool,
    node: usize,
    mesh_revision: bool,
    capture_rect: [i32; 4],
    url: String,
    export_size: [u32; 2],
    exporting: Option<mpsc::Receiver<anyhow::Result<()>>>,
    screenshot: Option<PathBuf>,
    frame_count: u32,
    screenshot_requested: bool,
    help: bool,
}
impl DomeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> anyhow::Result<Self> {
        let gl = cc
            .gl
            .clone()
            .ok_or_else(|| anyhow::anyhow!("OpenGL 3.3 is required"))?;
        style(&cc.egui_ctx);
        let mut renderer = Renderer::new(&gl)?;
        let frame = session_frame(&launch.session)?;
        if let Some(path) = &launch.gpu_check {
            renderer.check_gpu(&gl, path)?;
        }
        renderer.upload(&gl, &frame);
        renderer.set_mesh(launch.session.mesh.clone());
        if let Some(path) = &launch.session.view.background {
            renderer.upload_background(&gl, Some(&image::open(path)?.to_rgba8()))?;
        }
        let mesh_history = MeshHistory::new(&launch.session.mesh);
        let mut app = Self {
            session: launch.session,
            renderer: Arc::new(Mutex::new(renderer)),
            gl,
            frame,
            playback: Playback::default(),
            mesh_history,
            candidate_mesh: None,
            preview_source: false,
            status: "Built-in 360° scene · drag the preview to look around".into(),
            error: None,
            tab: match launch.panel.as_deref() {
                Some("calibration") => Tab::Calibration,
                Some("output") => Tab::Output,
                _ => Tab::Projection,
            },
            projector: launch.projector,
            projector_fullscreen: !launch.projector_windowed,
            displays: crate::displays::list(),
            output_placed: false,
            display_check: Instant::now(),
            identify_until: None,
            fullscreen: launch.fullscreen,
            edit_mesh: false,
            node: 0,
            mesh_revision: false,
            capture_rect: [0, 0, 1024, 1024],
            url: String::new(),
            export_size: [1920, 1080],
            exporting: None,
            screenshot: launch.screenshot,
            frame_count: 0,
            screenshot_requested: false,
            help: false,
        };
        app.playback.title = built_in_title(app.session.built_in_source.as_deref());
        if !app.session.playlist.is_empty() && app.session.built_in_source.is_none() {
            app.select(0);
            app.playback.startup_autoplay = launch.autoplay;
        } else if app.session.built_in_source.is_some() {
            app.status = "Built-in presentation source".into();
        }
        if launch.fullscreen {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        Ok(app)
    }
    fn set_frame(&mut self, image: RgbaImage) {
        self.renderer.lock().unwrap().upload(&self.gl, &image);
        self.frame = image;
    }
    fn stop(&mut self) {
        self.playback.pause();
    }
    fn select(&mut self, index: usize) {
        let Some(item) = self.session.playlist.get(index) else {
            return;
        };
        let path = item.path.to_string_lossy().to_string();
        let title = item
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        self.playback.reset_source(title);
        self.playback.selected = Some(index);
        self.session.built_in_source = None;
        self.session.view.input = item.projection;
        if item.projection == Projection::Perspective && self.session.view.source_fov >= 179. {
            self.session.view.source_fov = 90.;
        }
        self.playback.load(path, 0.);
        self.status = "Loading media…".into();
        self.error = None;
    }
    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let old_len = self.session.playlist.len();
        for path in paths {
            if path.is_dir() {
                match std::fs::read_dir(&path) {
                    Ok(entries) => {
                        let mut files: Vec<_> = entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| media::is_media(p))
                            .collect();
                        files.sort();
                        self.add_paths(files);
                    }
                    Err(error) => self.error = Some(format!("Cannot read folder: {error}")),
                }
            } else if media::is_media(&path) || path.to_string_lossy().starts_with("http") {
                self.session.playlist.push(PlaylistItem {
                    path,
                    projection: self.session.view.input,
                });
            } else if path.extension().is_some_and(|e| e == "data") {
                self.load_mesh(path);
            } else if path.to_string_lossy().ends_with(".genexdomplayer.json")
                || path.to_string_lossy().ends_with(".idome.json")
            {
                self.load_session(path);
            }
        }
        if self.playback.selected.is_none() && self.session.playlist.len() > old_len {
            self.select(old_len);
        }
    }
    fn toggle_play(&mut self) {
        if self.playback.pending.is_some() {
            return;
        }
        if self.playback.playing {
            self.stop();
            return;
        }
        match self.playback.play(self.session.volume) {
            Ok(warning) => self.error = warning,
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn seek(&mut self, to: f64) {
        match self.playback.seek(to, self.session.volume) {
            Ok(warning) => {
                if warning.is_some() {
                    self.error = warning;
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn advance(&mut self, delta: i32) {
        if self.session.playlist.is_empty() {
            return;
        }
        let next = (self.playback.selected.unwrap_or(0) as i32 + delta)
            .rem_euclid(self.session.playlist.len() as i32) as usize;
        let was = self.playback.playing;
        self.select(next);
        self.playback.startup_autoplay = was;
    }
    fn apply_mesh(&mut self, mesh: WarpMesh) -> bool {
        match self.mesh_history.apply(&mut self.session.mesh, mesh) {
            Ok(()) => {
                self.mesh_revision = true;
                self.node = 0;
                self.session.view.output = Output::Mirror;
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }
    fn load_mesh(&mut self, path: PathBuf) {
        match WarpMesh::load(&path) {
            Ok(mesh) => {
                if self.apply_mesh(mesh) {
                    self.mesh_history.mark_saved(&self.session.mesh);
                    self.candidate_mesh = None;
                    self.status = format!("Loaded warp mesh: {}", path.display());
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn load_session(&mut self, path: PathBuf) {
        match Session::load(&path).and_then(|session| Ok((session_frame(&session)?, session))) {
            Ok((frame, session)) => {
                self.playback
                    .reset_source(built_in_title(session.built_in_source.as_deref()));
                self.error = None;
                self.mesh_history = MeshHistory::new(&session.mesh);
                self.candidate_mesh = None;
                self.session = session;
                self.output_placed = false;
                self.refresh_background();
                self.set_frame(frame);
                self.mesh_revision = true;
                self.node = 0;
                if !self.session.playlist.is_empty() && self.session.built_in_source.is_none() {
                    self.select(0);
                }
                self.status = format!("Opened session: {}", path.display());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn show_pattern(&mut self, pattern: Pattern) {
        self.playback.reset_source(format!("{pattern:?}"));
        self.error = None;
        self.session.built_in_source = Some(
            match pattern {
                Pattern::Panorama => "panorama",
                Pattern::PolarGrid => "polar",
                Pattern::CartesianGrid => "cartesian",
                Pattern::ColourBars => "bars",
            }
            .into(),
        );
        self.session.view.input = match pattern {
            Pattern::Panorama => Projection::Equirectangular,
            Pattern::PolarGrid => Projection::Fisheye,
            _ => Projection::Perspective,
        };
        self.session.view.source_fov = if self.session.view.input == Projection::Perspective {
            90.
        } else {
            180.
        };
        self.set_frame(projection::pattern(pattern, 1024));
        self.status = "Calibration / demo source".into();
    }
    fn refresh_background(&mut self) {
        let result = self
            .session
            .view
            .background
            .as_ref()
            .map(|p| image::open(p).map(|i| i.to_rgba8()))
            .transpose();
        match result {
            Ok(image) => {
                if let Err(error) = self
                    .renderer
                    .lock()
                    .unwrap()
                    .upload_background(&self.gl, image.as_ref())
                {
                    self.error = Some(error.to_string());
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn export(&mut self, path: PathBuf) {
        let frame = self.frame.clone();
        let view = self.session.view.clone();
        let mesh = self.session.mesh.clone();
        let [width, height] = self.export_size;
        let (tx, rx) = mpsc::channel();
        self.exporting = Some(rx);
        std::thread::spawn(move || {
            let result = projection::render_cpu(&frame, &view, mesh.as_ref(), width, height)
                .and_then(|image| {
                    image.save(path)?;
                    Ok(())
                });
            let _ = tx.send(result);
        });
    }
    fn tick(&mut self, ctx: &egui::Context) {
        if self.projector && self.display_check.elapsed() >= Duration::from_secs(2) {
            self.displays = crate::displays::list();
            self.display_check = Instant::now();
        }
        if self.projector {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if self
            .identify_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.identify_until = None;
        }
        if let Some(result) = self.playback.poll_load() {
            match result {
                Ok(frame) => {
                    if let Some(info) = &self.playback.info {
                        self.status = format!(
                            "{} × {} · {}",
                            info.width,
                            info.height,
                            if info.is_video { "video" } else { "image" }
                        );
                    }
                    self.set_frame(frame);
                    if self.playback.startup_autoplay {
                        self.playback.startup_autoplay = false;
                        self.toggle_play();
                    }
                }
                Err(error) => {
                    self.status = "Media could not be loaded".into();
                    self.error = Some(format!("{error:#}"));
                }
            }
        }
        let mut finished = false;
        let mut decoded = None;
        if let Some(decoder) = &self.playback.decoder {
            if let Some(audio) = &self.playback.audio {
                if !audio.sink.empty() {
                    decoder.sync_to_audio(audio.position());
                }
            }
            decoded = decoder.latest.lock().unwrap().take();
            self.playback.position = decoder.position();
            finished = decoder.ended.load(std::sync::atomic::Ordering::Relaxed);
            if let Some(error) = decoder.error.lock().unwrap().take() {
                self.error = Some(error);
            }
        }
        if let Some(frame) = decoded {
            self.set_frame(frame);
        }
        if finished {
            let was_capture = self.playback.capture_active;
            self.stop();
            if !was_capture && self.error.is_none() {
                if self.session.auto_advance {
                    self.playback.playing = true;
                    self.advance(1);
                } else if self.session.looping {
                    self.playback.position = 0.;
                    self.toggle_play();
                }
            }
        }
        if self.playback.playing
            && self.playback.decoder.is_none()
            && self.session.auto_advance
            && self.playback.slide_start.elapsed().as_secs_f32() >= self.session.slide_seconds
        {
            self.advance(1);
        }
        if let Some(rx) = &self.exporting {
            match rx.try_recv() {
                Ok(result) => {
                    self.exporting = None;
                    match result {
                        Ok(()) => self.status = "Export saved".into(),
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.exporting = None;
                    self.error = Some("Export worker stopped unexpectedly".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let drops = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !drops.is_empty() {
            self.add_paths(drops);
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.toggle_play();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::B)) {
                self.session.view.blackout = !self.session.view.blackout;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.projector_fullscreen = false;
                self.fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                self.reset_orientation();
            }
            let step = ctx.input(|i| i.stable_dt.min(0.1)) * 35.;
            for (key, yaw, pitch) in [
                (egui::Key::ArrowLeft, -step, 0.),
                (egui::Key::ArrowRight, step, 0.),
                (egui::Key::ArrowUp, 0., step),
                (egui::Key::ArrowDown, 0., -step),
            ] {
                if ctx.input(|i| i.key_down(key)) {
                    self.session.view.yaw += yaw;
                    self.session.view.pitch = (self.session.view.pitch + pitch).clamp(-90., 90.);
                    ctx.request_repaint();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Comma)) {
                self.seek(self.playback.position - 10.);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
                self.seek(self.playback.position + 10.);
            }
        }
        if self.playback.playing
            || self.playback.pending.is_some()
            || self.exporting.is_some()
            || self.screenshot.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
    fn reset_orientation(&mut self) {
        self.session.view.yaw = 0.;
        self.session.view.pitch = 0.;
        self.session.view.roll = 0.;
    }
    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(64.)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.);
                    ui.label(
                        RichText::new("GenexDomPlayer")
                            .size(22.)
                            .strong()
                            .color(ACCENT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(accent_button(if self.projector {
                                "Close output"
                            } else {
                                "Open projector"
                            }))
                            .clicked()
                        {
                            self.projector = !self.projector;
                        }
                        let blackout = self.session.view.blackout;
                        if ui
                            .add(
                                egui::Button::new(if blackout {
                                    "Restore output [B]"
                                } else {
                                    "Blackout [B]"
                                })
                                .fill(if blackout {
                                    Color32::from_rgb(130, 40, 35)
                                } else {
                                    Color32::from_rgb(48, 52, 58)
                                }),
                            )
                            .clicked()
                        {
                            self.session.view.blackout = !blackout;
                        }
                        let (label, color) = if !self.projector {
                            ("Output closed", Color32::GRAY)
                        } else if self.session.view.blackout {
                            ("BLACKOUT", ACCENT)
                        } else {
                            ("LIVE", Color32::from_rgb(122, 215, 164))
                        };
                        ui.colored_label(color, label);
                        if ui.button("Help").clicked() {
                            self.help = !self.help;
                        }
                        if ui.button("Save session").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("GenexDomPlayer session", &["json"])
                                .set_file_name("show.genexdomplayer.json")
                                .save_file()
                            {
                                match self.session.save(&path) {
                                    Ok(()) => {
                                        self.mesh_history.mark_saved(&self.session.mesh);
                                        self.status = "Session saved".into();
                                    }
                                    Err(error) => self.error = Some(error.to_string()),
                                }
                            }
                        }
                        if ui.button("Open session").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("GenexDomPlayer session", &["json"])
                                .pick_file()
                            {
                                self.load_session(path);
                            }
                        }
                    });
                });
            });
    }
    fn library(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("library")
            .default_width(222.)
            .min_width(190.)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(14.);
                ui.heading("Media library");
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    if ui.button("+ Add media").clicked() {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter(
                                "Images and movies",
                                &[
                                    "png", "jpg", "jpeg", "tga", "bmp", "webp", "tif", "tiff",
                                    "gif", "mp4", "mov", "mkv", "avi", "webm", "m4v", "mpg",
                                    "mpeg",
                                ],
                            )
                            .pick_files()
                        {
                            self.add_paths(paths);
                        }
                    }
                    if ui.button("Folder").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.add_paths(vec![path]);
                        }
                    }
                });
                ui.add_space(12.);
                let mut choose = None;
                let mut remove = None;
                let mut move_item = None;
                egui::ScrollArea::vertical()
                    .id_salt("playlist")
                    .max_height((ui.available_height() - 235.).max(120.))
                    .show(ui, |ui| {
                        if self.session.playlist.is_empty() {
                            ui.label(
                                RichText::new("Drop images, movies or a media folder here.")
                                    .color(Color32::GRAY),
                            );
                        }
                        for (index, item) in self.session.playlist.iter().enumerate() {
                            let selected = self.playback.selected == Some(index);
                            ui.horizontal(|ui| {
                                let name =
                                    item.path.file_name().unwrap_or_default().to_string_lossy();
                                let marker = if selected && self.playback.playing {
                                    "▶ "
                                } else {
                                    ""
                                };
                                let width = (ui.available_width() - 40.).max(60.);
                                if ui
                                    .add_sized(
                                        [width, 28.],
                                        egui::Button::new(format!(
                                            "{:02} {marker}{name}",
                                            index + 1
                                        ))
                                        .selected(selected)
                                        .truncate(),
                                    )
                                    .on_hover_text(item.path.display().to_string())
                                    .clicked()
                                {
                                    choose = Some(index);
                                }
                                ui.menu_button("···", |ui| {
                                    if ui.button("Move up").clicked() {
                                        move_item = Some((index, index.saturating_sub(1)));
                                        ui.close_menu();
                                    }
                                    if ui.button("Move down").clicked() {
                                        move_item = Some((
                                            index,
                                            (index + 1).min(self.session.playlist.len() - 1),
                                        ));
                                        ui.close_menu();
                                    }
                                    if ui.button("Remove").clicked() {
                                        remove = Some(index);
                                        ui.close_menu();
                                    }
                                });
                            });
                            let missing = !item.path.to_string_lossy().starts_with("http")
                                && !item.path.exists();
                            let detail = if missing {
                                "Missing file".into()
                            } else if selected {
                                self.playback
                                    .info
                                    .as_ref()
                                    .filter(|info| info.duration > 0.)
                                    .map_or_else(
                                        || item.projection.label().to_owned(),
                                        |info| {
                                            format!(
                                                "{} · {}",
                                                timecode(info.duration),
                                                item.projection.label()
                                            )
                                        },
                                    )
                            } else {
                                item.projection.label().to_owned()
                            };
                            ui.add(
                                egui::Label::new(RichText::new(detail).small().color(if missing {
                                    Color32::from_rgb(246, 127, 116)
                                } else {
                                    Color32::GRAY
                                }))
                                .truncate(),
                            );
                            ui.add_space(3.);
                        }
                    });
                if let Some(index) = choose {
                    self.select(index);
                }
                if let Some((a, b)) = move_item {
                    self.session.playlist.swap(a, b);
                    self.playback.selected = self.playback.selected.map(|index| {
                        if index == a {
                            b
                        } else if index == b {
                            a
                        } else {
                            index
                        }
                    });
                }
                if let Some(index) = remove {
                    self.session.playlist.remove(index);
                    if self.playback.selected == Some(index) {
                        self.playback.reset_source("No media selected".into());
                    } else if let Some(selected) = self.playback.selected {
                        if selected > index {
                            self.playback.selected = Some(selected - 1);
                        }
                    }
                }
                ui.separator();
                ui.label(RichText::new("TEST SOURCES").size(11.).color(Color32::GRAY));
                for (pattern, label) in [
                    (Pattern::Panorama, "Night horizon · 360°"),
                    (Pattern::PolarGrid, "Polar calibration grid"),
                    (Pattern::CartesianGrid, "Cartesian grid"),
                    (Pattern::ColourBars, "Colour bars"),
                ] {
                    if ui.button(label).clicked() {
                        self.show_pattern(pattern);
                    }
                }
                ui.add_space(12.);
                ui.label("Network / YouTube URL");
                ui.text_edit_singleline(&mut self.url);
                if ui
                    .add_enabled(!self.url.trim().is_empty(), egui::Button::new("Add URL"))
                    .clicked()
                {
                    let url = self.url.trim().to_owned();
                    self.add_paths(vec![PathBuf::from(url)]);
                    self.url.clear();
                }
            });
    }
    fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("transport")
            .min_height(108.)
            .show(ctx, |ui| {
                ui.add_space(8.);
                if let Some(message) = self.error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(message).color(Color32::from_rgb(246, 127, 116)));
                        if ui.small_button("Dismiss").clicked() {
                            self.error = None;
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(10.);
                    if ui
                        .add_enabled(
                            !self.session.playlist.is_empty(),
                            egui::Button::new("Previous"),
                        )
                        .clicked()
                    {
                        self.advance(-1);
                    }
                    if ui
                        .add_enabled(
                            self.playback.pending.is_none(),
                            egui::Button::new(if self.playback.playing {
                                "Pause"
                            } else {
                                "Play"
                            })
                            .min_size(Vec2::new(72., 30.)),
                        )
                        .clicked()
                    {
                        self.toggle_play();
                    }
                    if ui
                        .add_enabled(!self.session.playlist.is_empty(), egui::Button::new("Next"))
                        .clicked()
                    {
                        self.advance(1);
                    }
                    if ui.button("Stop").clicked() {
                        if let Err(error) = self.playback.rewind() {
                            self.error = Some(error.to_string());
                        }
                    }
                    ui.add_space(10.);
                    ui.checkbox(&mut self.session.looping, "Loop");
                    ui.checkbox(&mut self.session.auto_advance, "Auto advance");
                    ui.add(
                        egui::DragValue::new(&mut self.session.slide_seconds)
                            .range(0.1..=3600.)
                            .suffix(" s / slide")
                            .speed(0.1),
                    );
                    let old_speed = self.playback.speed;
                    egui::ComboBox::from_id_salt("speed")
                        .selected_text(format!("{}×", self.playback.speed))
                        .show_ui(ui, |ui| {
                            for speed in [0.25, 0.5, 1., 1.5, 2., 4.] {
                                ui.selectable_value(
                                    &mut self.playback.speed,
                                    speed,
                                    format!("{speed}×"),
                                );
                            }
                        });
                    if old_speed != self.playback.speed && self.playback.playing {
                        self.stop();
                        self.toggle_play();
                    }
                    ui.add(
                        egui::Slider::new(&mut self.session.volume, 0. ..=1.)
                            .text("Volume")
                            .show_value(false),
                    );
                    if let Some(audio) = &self.playback.audio {
                        audio.sink.set_volume(self.session.volume);
                    }
                });
                if let Some(duration) = self
                    .playback
                    .info
                    .as_ref()
                    .filter(|info| info.is_video && info.duration > 0.)
                    .map(|info| info.duration)
                {
                    ui.horizontal(|ui| {
                        ui.label(timecode(self.playback.position));
                        let mut seek = self.playback.scrub.unwrap_or(self.playback.position);
                        let response = ui.add_sized(
                            [ui.available_width() - 70., 18.],
                            egui::Slider::new(&mut seek, 0. ..=duration).show_value(false),
                        );
                        ui.label(timecode(duration));
                        if response.changed() {
                            self.playback.scrub = Some(seek);
                        }
                        if response.drag_stopped() || response.changed() && !response.dragged() {
                            self.seek(seek);
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add_space(10.);
                    if self.playback.pending.is_some() {
                        ui.spinner();
                        if ui.small_button("Cancel load").clicked() {
                            self.playback.cancel_load();
                            self.status = "Load cancelled".into();
                        }
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(&self.status).size(11.).color(Color32::GRAY),
                        )
                        .truncate(),
                    );
                });
            });
    }
    fn preview(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(15.);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(&self.playback.title).heading()).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.preview_source, true, "Source");
                    ui.selectable_value(&mut self.preview_source, false, "Projected");
                    ui.label(
                        RichText::new(self.session.view.output.label())
                            .small()
                            .color(ACCENT),
                    );
                });
            });
            ui.add_space(12.);
            let available = ui.available_size();
            let aspect = if self.preview_source {
                self.frame.width() as f32 / self.frame.height() as f32
            } else if self.session.view.output == Output::Mirror {
                self.session
                    .mesh
                    .as_ref()
                    .map_or(16. / 9., |mesh| mesh.aspect)
            } else {
                16. / 9.
            };
            let size = fit(available, aspect);
            let margin = ((available - size) * 0.5).max(Vec2::ZERO);
            ui.add_space(margin.y);
            ui.horizontal(|ui| {
                ui.add_space(margin.x);
                let mut view = self.session.view.clone();
                if self.preview_source {
                    view.output = Output::Source;
                    view.blackout = false;
                }
                let response = renderer::canvas(ui, self.renderer.clone(), view, size);
                if self.edit_mesh
                    && !self.preview_source
                    && self.session.view.output == Output::Mirror
                {
                    if response.drag_started() {
                        self.mesh_history.begin_edit(&self.session.mesh);
                    }
                    if let Some(mesh) = &mut self.session.mesh {
                        if let Some(pointer) = response.interact_pointer_pos() {
                            if response.drag_started() {
                                self.node = mesh
                                    .nodes
                                    .iter()
                                    .enumerate()
                                    .min_by(|(_, a), (_, b)| {
                                        node_position(a, mesh.aspect, response.rect)
                                            .distance_sq(pointer)
                                            .total_cmp(
                                                &node_position(b, mesh.aspect, response.rect)
                                                    .distance_sq(pointer),
                                            )
                                    })
                                    .map_or(0, |(index, _)| index);
                            }
                            if response.dragged() {
                                let node = &mut mesh.nodes[self.node];
                                node.x = ((pointer.x - response.rect.left())
                                    / response.rect.width()
                                    * 2.
                                    - 1.)
                                    * mesh.aspect;
                                node.y = 1.
                                    - (pointer.y - response.rect.top()) / response.rect.height()
                                        * 2.;
                                self.mesh_revision = true;
                            }
                        }
                        for (index, node) in mesh.nodes.iter().enumerate() {
                            if node.intensity < 0. {
                                continue;
                            }
                            ui.painter().circle_filled(
                                node_position(node, mesh.aspect, response.rect),
                                if index == self.node { 5. } else { 2. },
                                if index == self.node {
                                    ACCENT
                                } else {
                                    Color32::from_rgba_unmultiplied(210, 225, 235, 120)
                                },
                            );
                        }
                    }
                } else if response.dragged() {
                    let delta = response.drag_delta();
                    if ctx.input(|i| i.modifiers.shift) {
                        self.session.view.roll = (self.session.view.roll + delta.x * 0.15 + 180.)
                            .rem_euclid(360.)
                            - 180.;
                    } else {
                        self.session.view.yaw =
                            (self.session.view.yaw - delta.x * 0.2 + 180.).rem_euclid(360.) - 180.;
                        self.session.view.pitch =
                            (self.session.view.pitch + delta.y * 0.2).clamp(-90., 90.);
                    }
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0. {
                        self.session.view.perspective_fov =
                            (self.session.view.perspective_fov - scroll * 0.05).clamp(20., 150.);
                    }
                }
            });
            if self.session.view.output == Output::Mirror
                && self.session.view.input != Projection::Prewarped
                && self.session.mesh.is_none()
            {
                ui.colored_label(ACCENT, "Load or generate a warp mesh in Calibration.");
            }
        });
    }
    fn projector_window(&mut self, ctx: &egui::Context) {
        if !self.projector {
            self.output_placed = false;
            return;
        }
        let target = self
            .displays
            .iter()
            .find(|display| Some(&display.id) == self.session.output_display.as_ref())
            .cloned();
        if self.session.output_display.is_some() && target.is_none() {
            self.projector = false;
            self.output_placed = false;
            self.error = Some("The selected display is disconnected. Reconnect it or choose another output display.".into());
            return;
        }
        let view = self.session.view.clone();
        let renderer = self.renderer.clone();
        let mut close = false;
        let primary_scale = self
            .displays
            .iter()
            .find(|display| display.primary)
            .map_or(1., |display| display.scale)
            * ctx.zoom_factor();
        let initial_position = target
            .as_ref()
            .map_or(self.session.output_position, |display| {
                display.window_origin(primary_scale)
            });
        let fullscreen = self.projector_fullscreen && (target.is_none() || self.output_placed);
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("dome-projector"),
            egui::ViewportBuilder::default()
                .with_title("GenexDomPlayer · Projector output")
                .with_position(initial_position)
                .with_inner_size(self.session.output_size)
                .with_fullscreen(fullscreen),
            |ctx, _| {
                // Place the window using this viewport's actual DPI before fullscreen.
                if let Some(display) = &target {
                    if !self.output_placed {
                        let ppp = ctx.pixels_per_point();
                        let position = display.window_origin(ppp);
                        let placed = ctx.input(|i| {
                            i.viewport().outer_rect.is_some_and(|rect| {
                                (rect.min.x - position[0]).abs() < 2.
                                    && (rect.min.y - position[1]).abs() < 2.
                            })
                        });
                        if placed {
                            self.output_placed = true;
                        } else {
                            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                                egui::pos2(position[0], position[1]),
                            ));
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                                (display.size[0] as f32 * 0.8) / ppp,
                                (display.size[1] as f32 * 0.8) / ppp,
                            )));
                        }
                        ctx.request_repaint();
                    }
                }
                if ctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.projector_fullscreen = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
                if ctx.input(|i| i.key_pressed(egui::Key::B)) {
                    self.session.view.blackout = !self.session.view.blackout;
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::BLACK))
                    .show(ctx, |ui| {
                        if self.identify_until.is_some() {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new(
                                        target
                                            .as_ref()
                                            .map_or("Projector output".into(), |display| {
                                                display.label()
                                            }),
                                    )
                                    .size(40.)
                                    .color(Color32::WHITE),
                                );
                            });
                            ctx.request_repaint_after(Duration::from_millis(16));
                        } else {
                            let mut view = view.clone();
                            view.blackout = self.session.view.blackout;
                            renderer::canvas(ui, renderer.clone(), view, ui.available_size());
                        }
                    });
            },
        );
        if close {
            self.projector = false;
        }
    }
}
impl eframe::App for DomeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        self.header(ctx);
        self.transport(ctx);
        self.library(ctx);
        self.inspector(ctx);
        self.preview(ctx);
        if !ctx.input(|i| i.pointer.any_down()) {
            self.mesh_history.end_edit(&self.session.mesh);
        }
        if self.mesh_revision {
            self.renderer
                .lock()
                .unwrap()
                .set_mesh(self.session.mesh.clone());
            self.mesh_revision = false;
            ctx.request_repaint();
        }
        self.projector_window(ctx);
        if self.help {
            egui::Window::new("GenexDomPlayer controls").open(&mut self.help).show(ctx, |ui| {
                ui.label("Space: play / pause\nArrow keys: navigate\nDrag preview: look around\nShift + drag: rotate fisheye\nMouse wheel: perspective zoom\nHome: reset orientation\nComma / period: seek −10 / +10 seconds\nB: blackout\nF11: console fullscreen\nEscape: leave fullscreen\n\nGenerate a warp preview, then Apply. Undo/Redo restores mesh edits. Save a session to keep your playlist and settings. Previous saves are kept in .bak files.");
                ui.hyperlink_to("Projection guide", "https://paulbourke.net/dome/fishwarp/");
            });
        }
        self.frame_count += 1;
        if self.frame_count >= 12
            && self.playback.pending.is_none()
            && self.screenshot.is_some()
            && !self.screenshot_requested
        {
            self.screenshot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let captured = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let (Some(path), Some(image)) = (&self.screenshot, captured) {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|color| color.to_array())
                .collect();
            if let Err(error) = image::save_buffer(
                path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                eprintln!("Screenshot failed: {error}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.screenshot.is_some() && self.frame_count > 300 {
            eprintln!("Screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        self.playback.reset_source(String::new());
        if let Some(gl) = gl {
            self.renderer.lock().unwrap().destroy(gl);
        }
    }
}
fn style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(21, 25, 29);
    visuals.window_fill = Color32::from_rgb(27, 31, 35);
    visuals.extreme_bg_color = Color32::from_rgb(13, 16, 19);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.bg_fill = ACCENT.gamma_multiply(0.5);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 67, 72);
    visuals.override_text_color = Some(Color32::from_rgb(219, 226, 232));
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8., 8.);
        style.spacing.button_padding = Vec2::new(10., 6.);
        style.spacing.slider_width = 145.;
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.));
    });
}
fn accent_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::from_rgb(25, 20, 15))).fill(ACCENT)
}
fn stereo_combo(ui: &mut egui::Ui, id: &str, label: &str, value: &mut Stereo) {
    let name = |value| match value {
        Stereo::Mono => "Mono",
        Stereo::SideBySide => "Side by side",
        Stereo::TopBottom => "Top / bottom",
    };
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(name(*value))
            .show_ui(ui, |ui| {
                for stereo in [Stereo::Mono, Stereo::SideBySide, Stereo::TopBottom] {
                    ui.selectable_value(value, stereo, name(stereo));
                }
            });
    });
}
fn vec3_ui(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3]) {
    ui.label(label);
    ui.horizontal(|ui| {
        for (value, name) in values.iter_mut().zip(["X ", "Y ", "Z "]) {
            ui.add(egui::DragValue::new(value).speed(0.005).prefix(name));
        }
    });
}
fn fit(available: Vec2, aspect: f32) -> Vec2 {
    let width = available.x.min(available.y * aspect).max(1.);
    Vec2::new(width, width / aspect)
}
fn node_position(node: &crate::mesh::Node, aspect: f32, rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(
        rect.left() + (node.x / aspect + 1.) * 0.5 * rect.width(),
        rect.top() + (1. - node.y) * 0.5 * rect.height(),
    )
}
fn timecode(time: f64) -> String {
    format!("{:02}:{:02}", time as u64 / 60, time as u64 % 60)
}
fn built_in_title(source: Option<&str>) -> String {
    match source {
        Some("text") => "Dome message".into(),
        Some(name) => format!("{name} test source"),
        None => "Night horizon".into(),
    }
}
fn session_frame(session: &Session) -> anyhow::Result<RgbaImage> {
    Ok(match session.built_in_source.as_deref() {
        Some("text") => {
            let background = session
                .view
                .background
                .as_ref()
                .map(|path| {
                    image::open(path).map(|image| {
                        image::imageops::resize(
                            &image.to_rgba8(),
                            1024,
                            1024,
                            image::imageops::FilterType::Triangle,
                        )
                    })
                })
                .transpose()?;
            projection::text_image_on(&session.text, 1024, background)
        }
        Some("polar") => projection::pattern(Pattern::PolarGrid, 1024),
        Some("cartesian") => projection::pattern(Pattern::CartesianGrid, 1024),
        Some("bars") => projection::pattern(Pattern::ColourBars, 1024),
        _ => projection::pattern(Pattern::Panorama, 1024),
    })
}
