//! Playback/source transitions, independent of egui and OpenGL.
use crate::media::{Audio, Decoder, LoadJob, MediaInfo};
use image::RgbaImage;
use std::time::Instant;

pub struct Playback {
    pub info: Option<MediaInfo>,
    pub pending: Option<LoadJob>,
    pub selected: Option<usize>,
    pub decoder: Option<Decoder>,
    pub audio: Option<Audio>,
    pub playing: bool,
    pub position: f64,
    pub scrub: Option<f64>,
    pub speed: f32,
    pub max_height: u32,
    pub slide_start: Instant,
    pub title: String,
    pub capture_active: bool,
    pub startup_autoplay: bool,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            info: None,
            pending: None,
            selected: None,
            decoder: None,
            audio: None,
            playing: false,
            position: 0.,
            scrub: None,
            speed: 1.,
            max_height: 1080,
            slide_start: Instant::now(),
            title: "Night horizon".into(),
            capture_active: false,
            startup_autoplay: false,
        }
    }
}
impl Playback {
    pub fn pause(&mut self) {
        if let Some(decoder) = &self.decoder {
            self.position = decoder.position();
        }
        self.decoder = None;
        self.audio = None;
        self.playing = false;
        self.capture_active = false;
    }
    pub fn reset_source(&mut self, title: String) {
        self.pause();
        self.pending = None; // Dropping a job cancels its active subprocess.
        self.info = None;
        self.selected = None;
        self.position = 0.;
        self.scrub = None;
        self.startup_autoplay = false;
        self.slide_start = Instant::now();
        self.title = title;
    }
    pub fn load(&mut self, target: String, seek: f64) {
        self.pending = None;
        self.pending = Some(LoadJob::start(target, seek, self.max_height));
    }
    pub fn poll_load(&mut self) -> Option<anyhow::Result<RgbaImage>> {
        let result = self.pending.as_ref()?.poll()?;
        self.pending = None;
        Some(match result {
            Ok((info, frame)) => {
                self.info = Some(info);
                Ok(frame)
            }
            Err(error) => {
                self.info = None;
                self.startup_autoplay = false;
                Err(error)
            }
        })
    }
    pub fn cancel_load(&mut self) {
        self.pending = None;
        self.startup_autoplay = false;
        self.info = None;
        self.playing = false;
    }
    /// Returns a non-fatal audio warning; video may still play without a device.
    pub fn play(&mut self, volume: f32) -> anyhow::Result<Option<String>> {
        if self.pending.is_some() {
            return Ok(None);
        }
        let mut warning = None;
        if let Some(info) = &self.info {
            if info.is_video {
                if info.duration > 0. && self.position >= info.duration - 0.05 {
                    self.position = 0.;
                }
                self.decoder = Some(Decoder::video(
                    info,
                    self.position,
                    self.speed,
                    self.max_height,
                )?);
                if info.has_audio {
                    match Audio::start(info, self.position, self.speed, volume) {
                        Ok(audio) => self.audio = Some(audio),
                        Err(error) => {
                            warning = Some(format!("Video is playing; audio unavailable: {error}"))
                        }
                    }
                }
            }
        }
        self.playing = true;
        self.slide_start = Instant::now();
        Ok(warning)
    }
    pub fn seek(&mut self, to: f64, volume: f32) -> anyhow::Result<Option<String>> {
        self.scrub = None;
        let was = self.playing;
        self.pause();
        if let Some(info) = self.info.clone() {
            self.position = to.max(0.).min(if info.duration > 0. {
                (info.duration - 0.001).max(0.)
            } else {
                to.max(0.)
            });
            if was {
                return self.play(volume);
            }
            if info.is_video {
                self.load(info.target, self.position);
            }
        }
        Ok(None)
    }
    pub fn rewind(&mut self) -> anyhow::Result<Option<String>> {
        self.pause();
        self.startup_autoplay = false;
        self.seek(0., 0.)
    }
}
