use crate::media::{self, Decoder, DecoderStats};
use anyhow::Result;
use serde::Serialize;
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Default, Serialize)]
pub struct Diagnostics {
    pub video: Option<DecoderStats>,
    pub av_offset_ms: Option<f64>,
    pub uploads: u64,
    pub upload_total_ms: f64,
    pub upload_max_ms: f64,
}
impl Diagnostics {
    pub fn upload(&mut self, milliseconds: f64) {
        self.uploads += 1;
        self.upload_total_ms += milliseconds;
        self.upload_max_ms = self.upload_max_ms.max(milliseconds);
    }
    pub fn upload_average_ms(&self) -> f64 {
        self.upload_total_ms / self.uploads.max(1) as f64
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
}
/// Measures timed decode and CPU delivery without requiring a window or audio device.
pub fn benchmark(target: &str, seconds: f64, max_height: u32) -> Result<Diagnostics> {
    anyhow::ensure!(
        seconds.is_finite() && (0.1..=7200.).contains(&seconds),
        "Benchmark duration must be 0.1–7200 seconds"
    );
    let info = media::probe(target)?;
    let decoder = Decoder::video(&info, 0., 1., max_height)?;
    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    loop {
        let _ = decoder.take_frame();
        if let Some(error) = decoder.error.lock().unwrap().take() {
            anyhow::bail!(error);
        }
        if decoder.ended.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let video = decoder.stats();
    anyhow::ensure!(
        video.presented > 0,
        "Decoder produced no frames during benchmark"
    );
    Ok(Diagnostics {
        video: Some(video),
        ..Default::default()
    })
}
