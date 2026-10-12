use anyhow::{Context, Result};
use image::RgbaImage;
use rodio::Source;
use std::{
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct LoadControl {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    child: Arc<Mutex<Option<Child>>>,
}
impl LoadControl {
    fn new(timeout: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + timeout,
            child: Arc::new(Mutex::new(None)),
        }
    }
    fn check(&self) -> Result<()> {
        anyhow::ensure!(
            !self.cancelled.load(Ordering::Acquire),
            "Media load cancelled"
        );
        anyhow::ensure!(
            Instant::now() < self.deadline,
            "Media load timed out. Check the source and retry."
        );
        Ok(())
    }
    fn output(&self, cmd: &mut Command) -> Result<Output> {
        let mut active = self.child.lock().unwrap();
        self.check()?;
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = child.stdout.take().context("No subprocess stdout")?;
        let mut stderr = child.stderr.take().context("No subprocess stderr")?;
        *active = Some(child);
        drop(active);
        // Drain both pipes while polling, so a full pipe cannot hide cancellation.
        let out = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let err = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let status = loop {
            if let Err(error) = self.check() {
                break Err(error);
            }
            let waited = self
                .child
                .lock()
                .unwrap()
                .as_mut()
                .context("Media process disappeared")?
                .try_wait();
            match waited {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => break Err(error.into()),
            }
        };
        let mut active = self.child.lock().unwrap();
        let child = active.as_mut().context("Media process disappeared")?;
        if status.is_err() {
            let _ = child.kill();
        }
        let _ = child.wait();
        *active = None;
        drop(active);
        let stdout = out
            .join()
            .map_err(|_| anyhow::anyhow!("Media output reader failed"))?;
        let stderr = err
            .join()
            .map_err(|_| anyhow::anyhow!("Media error reader failed"))?;
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }
}

pub struct LoadJob {
    control: LoadControl,
    receiver: mpsc::Receiver<Result<(MediaInfo, RgbaImage)>>,
}
impl LoadJob {
    pub fn start(target: String, seek: f64, max_height: u32) -> Self {
        let control = LoadControl::new(Duration::from_secs(30));
        let worker = control.clone();
        let (tx, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(load_controlled(&target, seek, max_height, &worker));
        });
        Self { control, receiver }
    }
    pub fn poll(&self) -> Option<Result<(MediaInfo, RgbaImage)>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "Media loader stopped unexpectedly. Retry loading the source."
            ))),
        }
    }
}
impl Drop for LoadJob {
    fn drop(&mut self) {
        self.control.cancelled.store(true, Ordering::Release);
        // Kill synchronously as well: application exit must not outrun the worker's poll.
        if let Some(child) = self.control.child.lock().unwrap().as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn command(tool: &str) -> Command {
    let exe = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    };
    let mut locations = vec![
        PathBuf::from(".tools/media").join(&exe),
        PathBuf::from("tools").join(&exe),
    ];
    if let Ok(p) = std::env::current_exe() {
        if let Some(parent) = p.parent() {
            locations.insert(0, parent.join(&exe));
            locations.push(parent.join("tools").join(&exe));
        }
    }
    let mut cmd = Command::new(
        locations
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from(exe)),
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
pub fn is_image(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        [
            "png", "jpg", "jpeg", "tga", "bmp", "tif", "tiff", "webp", "gif",
        ]
        .contains(&e.to_lowercase().as_str())
    })
}
pub fn is_media(path: &Path) -> bool {
    is_image(path)
        || path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            ["mp4", "mov", "mkv", "avi", "webm", "m4v", "mpg", "mpeg"]
                .contains(&e.to_lowercase().as_str())
        })
}
pub fn runtime_status() -> String {
    match command("ffmpeg").arg("-version").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .unwrap_or("FFmpeg available")
            .to_string(),
        _ => "FFmpeg missing · images and calibration work; install runtime for movies".into(),
    }
}
#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub target: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub is_video: bool,
    pub has_audio: bool,
}
fn resolve_url(target: &str, control: &LoadControl) -> Result<String> {
    if target.contains("youtube.com/") || target.contains("youtu.be/") {
        let out = control
            .output(command("yt-dlp").args([
                "--no-playlist",
                "--get-url",
                "-f",
                "best[ext=mp4]/best",
                target,
            ]))
            .context("YouTube needs yt-dlp in tools or PATH")?;
        anyhow::ensure!(
            out.status.success(),
            "YouTube resolver: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(String::from_utf8(out.stdout)?
            .lines()
            .next()
            .context("No playable URL returned")?
            .to_string())
    } else {
        Ok(target.to_owned())
    }
}
pub fn probe(target: &str) -> Result<MediaInfo> {
    probe_controlled(target, &LoadControl::new(Duration::from_secs(30)))
}
fn probe_controlled(target: &str, control: &LoadControl) -> Result<MediaInfo> {
    let target = resolve_url(target, control)?;
    let out = control
        .output(command("ffprobe").args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,width,height:format=duration",
            "-of",
            "json",
            "-i",
            &target,
        ]))
        .context(
            "FFprobe missing. Run scripts/install-media.ps1 or put FFmpeg and FFprobe in PATH.",
        )?;
    anyhow::ensure!(
        out.status.success(),
        "Cannot open media: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let streams = json["streams"].as_array().context("Media has no streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .context("Media has no video track")?;
    let width = video["width"].as_u64().context("No video width")? as u32;
    let height = video["height"].as_u64().context("No video height")? as u32;
    anyhow::ensure!(
        width > 0 && height > 0 && width <= 16384 && height <= 16384,
        "Unsupported video dimensions"
    );
    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.);
    Ok(MediaInfo {
        target,
        width,
        height,
        duration,
        is_video: true,
        has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
    })
}
pub fn load(target: &str, seek: f64, max_height: u32) -> Result<(MediaInfo, RgbaImage)> {
    load_controlled(
        target,
        seek,
        max_height,
        &LoadControl::new(Duration::from_secs(30)),
    )
}
fn load_controlled(
    target: &str,
    seek: f64,
    max_height: u32,
    control: &LoadControl,
) -> Result<(MediaInfo, RgbaImage)> {
    control.check()?;
    if is_image(Path::new(target)) {
        let image = image::ImageReader::open(target)?
            .with_guessed_format()?
            .decode()?
            .to_rgba8();
        let info = MediaInfo {
            target: target.into(),
            width: image.width(),
            height: image.height(),
            duration: 0.,
            is_video: false,
            has_audio: false,
        };
        control.check()?;
        return Ok((info, image));
    }
    let info = probe_controlled(target, control)?;
    let (w, h) = decode_size(&info, max_height);
    let out = control.output(command("ffmpeg").args([
        "-v",
        "error",
        "-ss",
        &seek.to_string(),
        "-i",
        &info.target,
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-vf",
        &format!("scale={w}:{h}"),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ]))?;
    anyhow::ensure!(
        out.status.success(),
        "Video preview: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let image =
        RgbaImage::from_raw(w, h, out.stdout).context("Video preview was empty or truncated")?;
    Ok((info, image))
}
pub fn decode_size(info: &MediaInfo, max_height: u32) -> (u32, u32) {
    let h = info.height.min(max_height.max(64));
    let w = ((info.width as f64 * h as f64 / info.height as f64).round() as u32).max(1);
    (w, h)
}

pub struct Decoder {
    child: Arc<Mutex<Child>>,
    cancel: Arc<AtomicBool>,
    pub latest: Arc<Mutex<Option<RgbaImage>>>,
    pub ended: Arc<AtomicBool>,
    pub position: Arc<AtomicU64>,
    pub error: Arc<Mutex<Option<String>>>,
    clock: Arc<Mutex<Option<(f64, Instant)>>>,
}
impl Decoder {
    pub fn video(info: &MediaInfo, start: f64, speed: f32, max_height: u32) -> Result<Self> {
        let (w, h) = decode_size(info, max_height);
        let mut cmd = command("ffmpeg");
        cmd.args([
            "-v",
            "error",
            "-ss",
            &start.to_string(),
            "-i",
            &info.target,
            "-map",
            "0:v:0",
            "-an",
            "-vf",
            &format!("fps=30,scale={w}:{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ]);
        Self::spawn(cmd, w, h, start, speed, false)
    }
    pub fn capture(rect: [i32; 4], fps: u32) -> Result<Self> {
        anyhow::ensure!(
            rect[2] > 0 && rect[3] > 0 && rect[2] <= 8192 && rect[3] <= 8192,
            "Capture width and height must be 1–8192"
        );
        let mut cmd = command("ffmpeg");
        cmd.args(["-v", "error"]);
        #[cfg(windows)]
        cmd.args([
            "-f",
            "gdigrab",
            "-framerate",
            &fps.to_string(),
            "-offset_x",
            &rect[0].to_string(),
            "-offset_y",
            &rect[1].to_string(),
            "-video_size",
            &format!("{}x{}", rect[2], rect[3]),
            "-draw_mouse",
            "0",
            "-i",
            "desktop",
        ]);
        #[cfg(target_os = "linux")]
        cmd.args([
            "-f",
            "x11grab",
            "-framerate",
            &fps.to_string(),
            "-video_size",
            &format!("{}x{}", rect[2], rect[3]),
            "-i",
            &format!(
                "{}+{},{}",
                std::env::var("DISPLAY").unwrap_or_else(|_| ":0.0".into()),
                rect[0],
                rect[1]
            ),
        ]);
        #[cfg(target_os = "macos")]
        anyhow::bail!(
            "macOS capture requires a platform capture provider; not enabled in this build"
        );
        cmd.args(["-an", "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"]);
        Self::spawn(
            cmd,
            rect[2] as u32,
            rect[3] as u32,
            0.,
            fps as f32 / 30.,
            true,
        )
    }
    fn spawn(mut cmd: Command, w: u32, h: u32, start: f64, speed: f32, live: bool) -> Result<Self> {
        anyhow::ensure!(
            speed.is_finite() && speed > 0.,
            "Playback speed must be positive"
        );
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .context("Could not start FFmpeg decoder")?;
        let mut stdout = BufReader::new(child.stdout.take().context("No decoder output")?);
        let mut stderr = child.stderr.take().unwrap();
        let child = Arc::new(Mutex::new(child));
        let cancel = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(None));
        let ended = Arc::new(AtomicBool::new(false));
        let position = Arc::new(AtomicU64::new(start.to_bits()));
        let error = Arc::new(Mutex::new(None));
        let err = error.clone();
        let cancelled = cancel.clone();
        let error_reader = std::thread::spawn(move || {
            let mut log = String::new();
            let _ = stderr.read_to_string(&mut log);
            if !cancelled.load(Ordering::Relaxed) && !log.trim().is_empty() {
                *err.lock().unwrap() = Some(log);
            }
        });
        let (cancel2, latest2, ended2, position2) = (
            cancel.clone(),
            latest.clone(),
            ended.clone(),
            position.clone(),
        );
        let clock = Arc::new(Mutex::new(None));
        let clock2 = clock.clone();
        std::thread::spawn(move || {
            let mut frame = vec![0u8; w as usize * h as usize * 4];
            let mut frame_index = 0u64;
            while !cancel2.load(Ordering::Relaxed) {
                if stdout.read_exact(&mut frame).is_err() {
                    break;
                }
                let pts = start + frame_index as f64 / 30.;
                frame_index += 1;
                let current_time = || {
                    let mut clock = clock2.lock().unwrap();
                    let (position, epoch) = *clock.get_or_insert_with(|| (start, Instant::now()));
                    position + epoch.elapsed().as_secs_f64() * speed as f64
                };
                while !live && pts > current_time() && !cancel2.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                if cancel2.load(Ordering::Relaxed) {
                    break;
                }
                // Catch up to the audio master without displaying a burst of late frames.
                if !live && current_time() - pts > 1. / 30. {
                    continue;
                }
                *latest2.lock().unwrap() = RgbaImage::from_raw(w, h, frame.clone());
                position2.store(pts.to_bits(), Ordering::Relaxed);
            }
            let _ = error_reader.join();
            ended2.store(true, Ordering::Relaxed);
        });
        Ok(Self {
            child,
            cancel,
            latest,
            ended,
            position,
            error,
            clock,
        })
    }
    pub fn position(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Relaxed))
    }
    pub fn sync_to_audio(&self, position: f64) {
        if position.is_finite() {
            *self.clock.lock().unwrap() = Some((position, Instant::now()));
        }
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct Audio {
    pub stream: rodio::OutputStream,
    pub sink: rodio::Sink,
    child: Child,
    start_position: f64,
    speed: f32,
}
struct PcmSource {
    reader: BufReader<std::process::ChildStdout>,
}
impl Iterator for PcmSource {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        let mut bytes = [0; 2];
        self.reader.read_exact(&mut bytes).ok()?;
        Some(i16::from_le_bytes(bytes))
    }
}
impl Source for PcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        48000
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
impl Audio {
    pub fn start(info: &MediaInfo, position: f64, speed: f32, volume: f32) -> Result<Self> {
        let (stream, handle) =
            rodio::OutputStream::try_default().context("No audio output device")?;
        let sink = rodio::Sink::try_new(&handle)?;
        let mut child = command("ffmpeg")
            .args([
                "-v",
                "error",
                "-ss",
                &position.to_string(),
                "-i",
                &info.target,
                "-map",
                "0:a:0",
                "-vn",
                "-f",
                "s16le",
                "-ac",
                "2",
                "-ar",
                "48000",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        sink.set_volume(volume);
        sink.set_speed(speed);
        sink.append(PcmSource {
            reader: BufReader::with_capacity(65536, child.stdout.take().unwrap()),
        });
        Ok(Self {
            stream,
            sink,
            child,
            start_position: position,
            speed,
        })
    }
    pub fn position(&self) -> f64 {
        self.start_position + self.sink.get_pos().as_secs_f64() * self.speed as f64
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.sink.stop();
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn child_waits() {
        if std::env::var_os("GENEX_TEST_WAIT_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }
    fn waiting_child() -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "media::process_tests::child_waits",
            "--nocapture",
        ])
        .env("GENEX_TEST_WAIT_CHILD", "1");
        cmd
    }
    #[test]
    fn subprocess_timeout_is_bounded_and_reaped() {
        let control = LoadControl::new(Duration::from_millis(150));
        let start = Instant::now();
        let error = control.output(&mut waiting_child()).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn subprocess_cancellation_is_bounded_and_reaped() {
        let control = LoadControl::new(Duration::from_secs(10));
        let token = control.cancelled.clone();
        let cancel = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            token.store(true, Ordering::Release);
        });
        let start = Instant::now();
        let error = control.output(&mut waiting_child()).unwrap_err();
        cancel.join().unwrap();
        assert!(error.to_string().contains("cancelled"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn dropping_load_job_reaps_its_active_process_before_returning() {
        let control = LoadControl::new(Duration::from_secs(5));
        let worker_control = control.clone();
        let worker = std::thread::spawn(move || worker_control.output(&mut waiting_child()));
        let deadline = Instant::now() + Duration::from_secs(3);
        while control.child.lock().unwrap().is_none() {
            assert!(Instant::now() < deadline, "Child did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (_tx, receiver) = mpsc::channel();
        let job = LoadJob {
            control: control.clone(),
            receiver,
        };
        drop(job);
        if let Some(child) = control.child.lock().unwrap().as_mut() {
            assert!(
                child.try_wait().unwrap().is_some(),
                "Drop returned with a running subprocess"
            );
        }
        assert!(worker.join().unwrap().is_err());
    }
}
