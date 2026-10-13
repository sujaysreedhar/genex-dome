use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Projection {
    #[default]
    Equirectangular,
    Fisheye,
    Cylindrical,
    Perspective,
    Cubemap,
    Prewarped,
}
impl Projection {
    pub const ALL: [Self; 6] = [
        Self::Equirectangular,
        Self::Fisheye,
        Self::Cylindrical,
        Self::Perspective,
        Self::Cubemap,
        Self::Prewarped,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Equirectangular => "360° equirectangular",
            Self::Fisheye => "Fisheye",
            Self::Cylindrical => "Cylindrical panorama",
            Self::Perspective => "Standard image / video",
            Self::Cubemap => "Cubemap · 3 × 2 atlas",
            Self::Prewarped => "Prewarped",
        }
    }
    pub fn code(self) -> i32 {
        match self {
            Self::Perspective => 1,
            Self::Fisheye => 2,
            Self::Cylindrical => 3,
            Self::Equirectangular => 4,
            Self::Cubemap => 5,
            Self::Prewarped => 6,
        }
    }
    pub fn from_code(code: i32) -> anyhow::Result<Self> {
        Ok(match code {
            1 => Self::Perspective,
            2 => Self::Fisheye,
            3 => Self::Cylindrical,
            4 => Self::Equirectangular,
            5 => Self::Cubemap,
            6 => Self::Prewarped,
            _ => anyhow::bail!("Unknown projection identifier {code}"),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Output {
    #[default]
    Fisheye,
    Mirror,
    Perspective,
    Source,
}
impl Output {
    pub const ALL: [Self; 4] = [Self::Fisheye, Self::Mirror, Self::Perspective, Self::Source];
    pub fn label(self) -> &'static str {
        match self {
            Self::Fisheye => "Direct fisheye",
            Self::Mirror => "Spherical mirror / mesh",
            Self::Perspective => "Perspective view",
            Self::Source => "Source / prewarped",
        }
    }
    pub fn code(self) -> i32 {
        match self {
            Self::Fisheye => 0,
            Self::Mirror => 1,
            Self::Perspective => 2,
            Self::Source => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Stereo {
    #[default]
    Mono,
    SideBySide,
    TopBottom,
}
impl Stereo {
    pub fn code(self) -> i32 {
        match self {
            Self::Mono => 0,
            Self::SideBySide => 1,
            Self::TopBottom => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub input: Projection,
    pub output: Output,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fisheye_fov: f32,
    pub source_fov: f32,
    pub perspective_fov: f32,
    pub brightness: f32,
    pub gamma: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub offset: [f32; 2],
    pub stereo_input: Stereo,
    pub stereo_output: Stereo,
    pub blackout: bool,
    pub background: Option<PathBuf>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            input: Projection::Equirectangular,
            output: Output::Fisheye,
            yaw: 0.,
            pitch: 0.,
            roll: 0.,
            fisheye_fov: 180.,
            source_fov: 180.,
            perspective_fov: 90.,
            brightness: 1.,
            gamma: 1.,
            flip_x: false,
            flip_y: false,
            offset: [0., 0.],
            stereo_input: Stereo::Mono,
            stereo_output: Stereo::Mono,
            blackout: false,
            background: None,
        }
    }
}
impl View {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            [
                self.yaw,
                self.pitch,
                self.roll,
                self.fisheye_fov,
                self.source_fov,
                self.perspective_fov,
                self.brightness,
                self.gamma,
                self.offset[0],
                self.offset[1]
            ]
            .iter()
            .all(|v| v.is_finite()),
            "View contains non-finite numbers"
        );
        anyhow::ensure!(
            (1. ..=360.).contains(&self.fisheye_fov)
                && (1. ..=360.).contains(&self.source_fov)
                && (1. ..179.).contains(&self.perspective_fov),
            "Field of view out of range"
        );
        anyhow::ensure!(
            (0. ..=4.).contains(&self.brightness) && (0.1..=5.).contains(&self.gamma),
            "Brightness or gamma out of range"
        );
        anyhow::ensure!(
            self.offset[0] * self.offset[0] + self.offset[1] * self.offset[1] < 0.99 * 0.99,
            "Projector offset must be inside the dome"
        );
        Ok(())
    }
}
/// Source settings travel with a cue; physical output calibration stays with the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueView {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub source_fov: f32,
    pub perspective_fov: f32,
    pub stereo_input: Stereo,
}
impl CueView {
    pub fn capture(view: &View) -> Self {
        Self {
            yaw: view.yaw,
            pitch: view.pitch,
            roll: view.roll,
            source_fov: view.source_fov,
            perspective_fov: view.perspective_fov,
            stereo_input: view.stereo_input,
        }
    }
    pub fn apply(&self, view: &mut View) {
        view.yaw = self.yaw;
        view.pitch = self.pitch;
        view.roll = self.roll;
        view.source_fov = self.source_fov;
        view.perspective_fov = self.perspective_fov;
        view.stereo_input = self.stereo_input;
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CueSettings {
    pub name: String,
    pub view: Option<CueView>,
    /// Media seconds for movies; wall seconds for stills. None uses the session default.
    pub duration_seconds: Option<f64>,
    /// Total fade-out + fade-in time on entry; zero selects a cut.
    pub fade_seconds: f32,
}
impl CueSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.duration_seconds
                .is_none_or(|d| d.is_finite() && (0.1..=86400.).contains(&d)),
            "Cue duration must be 0.1–86400 seconds"
        );
        anyhow::ensure!(
            self.fade_seconds.is_finite() && (0. ..=10.).contains(&self.fade_seconds),
            "Cue fade must be 0–10 seconds"
        );
        if let Some(preset) = &self.view {
            let mut view = View::default();
            preset.apply(&mut view);
            view.validate()?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub path: PathBuf,
    pub projection: Projection,
    #[serde(default)]
    pub cue: CueSettings,
}
impl PlaylistItem {
    pub fn new(path: PathBuf, projection: Projection) -> Self {
        Self {
            path,
            projection,
            cue: CueSettings::default(),
        }
    }
    pub fn label(&self) -> String {
        if self.cue.name.trim().is_empty() {
            self.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            self.cue.name.clone()
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub version: u32,
    pub view: View,
    pub playlist: Vec<PlaylistItem>,
    pub mesh: Option<crate::mesh::WarpMesh>,
    pub optics: crate::optics::Optics,
    pub looping: bool,
    pub auto_advance: bool,
    pub slide_seconds: f32,
    pub volume: f32,
    pub output_position: [f32; 2],
    pub output_display: Option<String>,
    pub output_size: [f32; 2],
    pub text: String,
    pub built_in_source: Option<String>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            version: 1,
            view: View::default(),
            playlist: vec![],
            mesh: None,
            optics: crate::optics::Optics::default(),
            looping: true,
            auto_advance: false,
            slide_seconds: 8.,
            volume: 0.8,
            output_position: [1920., 0.],
            output_display: None,
            output_size: [1920., 1080.],
            text: "Welcome to the dome".into(),
            built_in_source: None,
        }
    }
}
impl Session {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let mut s: Self = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        s.validate()?;
        let base = path.parent().unwrap_or(std::path::Path::new("."));
        if let Some(p) = &mut s.view.background {
            if p.is_relative() {
                *p = base.join(&*p);
            }
        }
        for item in &mut s.playlist {
            if item.path.is_relative() && !crate::bundle::is_network(&item.path) {
                item.path = base.join(&item.path);
            }
        }
        Ok(s)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        self.view.validate()?;
        anyhow::ensure!(self.version == 1, "Unsupported session version");
        anyhow::ensure!(
            self.built_in_source.as_ref().is_none_or(|value| [
                "panorama",
                "polar",
                "cartesian",
                "bars",
                "text"
            ]
            .contains(&value.as_str())),
            "Unknown built-in source"
        );
        anyhow::ensure!(
            self.slide_seconds.is_finite()
                && self.slide_seconds >= 0.1
                && self.volume.is_finite()
                && (0. ..=1.).contains(&self.volume),
            "Invalid playback settings"
        );
        anyhow::ensure!(
            self.output_position
                .iter()
                .chain(self.output_size.iter())
                .all(|v| v.is_finite())
                && self.output_size.iter().all(|v| *v >= 64.),
            "Invalid projector window geometry"
        );
        for item in &self.playlist {
            anyhow::ensure!(!item.path.as_os_str().is_empty(), "Cue has no media path");
            item.cue.validate()?;
        }
        if let Some(mesh) = &self.mesh {
            mesh.validate()?;
        }
        self.optics.validate()?;
        Ok(())
    }
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        self.validate()?;
        let mut saved = self.clone();
        let base = std::env::current_dir()?;
        if let Some(p) = &mut saved.view.background {
            if p.is_relative() {
                *p = base.join(&*p);
            }
        }
        for item in &mut saved.playlist {
            if item.path.is_relative() && !crate::bundle::is_network(&item.path) {
                item.path = base.join(&item.path);
            }
        }
        crate::persistence::atomic_write(path, serde_json::to_string_pretty(&saved)?.as_bytes())?;
        Ok(())
    }
}
