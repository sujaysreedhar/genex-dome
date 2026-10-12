use genex_dom_player::media::{self, Audio, Decoder};
use std::time::{Duration, Instant};
#[test]
#[ignore = "requires the optional FFmpeg and FFprobe runtime"]
fn movie_preview_seek_stream_audio_and_shutdown() {
    let directory = std::env::temp_dir();
    let path = directory.join(format!("genexdomplayer-media-{}.mp4", std::process::id()));
    let result = media::command("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=128x64:r=30:d=2",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=48000:cl=stereo",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert!(result.success());
    let (info, frame) = media::load(&path.to_string_lossy(), 0.75, 64).unwrap();
    assert_eq!(frame.dimensions(), (128, 64));
    assert!(frame.get_pixel(64, 32)[0] > 240);
    assert!(info.has_audio);
    assert!((info.duration - 2.).abs() < 0.1);
    let decoder = Decoder::video(&info, 0.5, 2., 64).unwrap();
    let audio = match Audio::start(&info, 0.5, 2., 0.) {
        Ok(audio) => Some(audio),
        Err(error) if error.to_string().contains("No audio output device") => {
            eprintln!("Audio output device is unavailable; checking decoded PCM instead.");
            None
        }
        Err(error) => panic!("Audio startup failed: {error:#}"),
    };
    let pcm = media::command("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &info.target,
            "-map",
            "0:a:0",
            "-t",
            "0.2",
            "-f",
            "s16le",
            "-ac",
            "2",
            "-ar",
            "48000",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(pcm.status.success());
    assert!(pcm.stdout.len() >= 38000);
    let limit = Instant::now() + Duration::from_secs(3);
    while decoder.position() < 0.9 && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        decoder.position() >= 0.9,
        "Decoder failed to advance after seeking"
    );
    assert!(
        decoder
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .get_pixel(40, 20)[0]
            > 240
    );
    if let Some(audio) = &audio {
        assert!(!audio.sink.empty());
    }
    drop(audio);
    drop(decoder);
    // Video catches up to the audio master rather than drifting on its own clock.
    let decoder = Decoder::video(&info, 0., 1., 64).unwrap();
    decoder.sync_to_audio(1.2);
    let limit = Instant::now() + Duration::from_secs(2);
    while decoder.position() < 1.16 && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        decoder.position() >= 1.16,
        "Video did not follow its master clock"
    );
    drop(decoder);

    // Stop/rewind must fetch the opening frame while leaving playback paused.
    let mut playback = genex_dom_player::playback::Playback {
        info: Some(info.clone()),
        position: 1.,
        ..Default::default()
    };
    playback.rewind().unwrap();
    assert_eq!(playback.position, 0.);
    assert!(!playback.playing);
    assert!(playback.pending.is_some());
    let limit = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = playback.poll_load() {
            assert!(result.unwrap().get_pixel(40, 20)[0] > 240);
            break;
        }
        assert!(Instant::now() < limit, "Rewind preview did not complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    let decoder = Decoder::video(&info, 1.8, 1., 64).unwrap();
    let limit = Instant::now() + Duration::from_secs(3);
    while !decoder.ended.load(std::sync::atomic::Ordering::Relaxed) && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(decoder.ended.load(std::sync::atomic::Ordering::Relaxed));
    assert!(decoder.error.lock().unwrap().is_none());
    drop(decoder);
    std::fs::remove_file(path).unwrap();
}
