use genex_dom_player::{
    media::{self, Decoder},
    playback::Playback,
};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires internet, current yt-dlp, a JavaScript runtime, FFmpeg and FFprobe"]
fn youtube_split_stream_preview_audio_playback_and_paused_seek() {
    // This source triggered the combined-format failure reported by the operator.
    let source = "https://www.youtube.com/watch?v=IrolgreFj2w";
    let (info, frame) = media::load(source, 0., 360).unwrap();
    assert_eq!(info.source, source);
    assert!(
        info.audio_source.is_some(),
        "Expected separate video/audio streams"
    );
    assert!(info.has_audio);
    assert_eq!(frame.dimensions(), media::decode_size(&info, 360));
    assert!(frame
        .pixels()
        .any(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20));

    let mut cmd = media::command("ffmpeg");
    cmd.args(["-v", "error", "-ss", "1"]);
    info.audio_input(&mut cmd);
    let pcm = cmd
        .args([
            "-map", "0:a:0", "-t", "0.2", "-f", "s16le", "-ac", "2", "-ar", "48000", "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        pcm.status.success(),
        "Audio decode: {}",
        String::from_utf8_lossy(&pcm.stderr)
    );
    assert!(
        pcm.stdout.len() >= 38000,
        "Audio stream did not produce PCM"
    );

    let decoder = Decoder::video(&info, 1., 1., 360).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while decoder.position() < 1.25 {
        let _ = decoder.take_frame();
        assert!(Instant::now() < deadline, "YouTube decoder did not advance");
        if let Some(error) = decoder.error.lock().unwrap().as_ref() {
            panic!("Video decode: {error}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(decoder.stats().presented > 0);
    drop(decoder);

    let mut playback = Playback {
        info: Some(info),
        max_height: 360,
        ..Default::default()
    };
    playback.seek(2., 0.).unwrap();
    assert!(!playback.playing);
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        if let Some(result) = playback.poll_load() {
            assert_eq!(result.unwrap().height(), 360);
            break;
        }
        assert!(Instant::now() < deadline, "Paused seek did not complete");
        std::thread::sleep(Duration::from_millis(20));
    }
    let info = playback.info.unwrap();
    assert_eq!(info.source, source);
    assert!(
        info.has_audio && info.audio_source.is_some(),
        "Paused seek lost the separate audio stream"
    );
}
