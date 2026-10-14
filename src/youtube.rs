//! Resolve YouTube's selected video/audio pair without requiring a combined format.
use crate::media::{command, tool_path, LoadControl};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{path::PathBuf, process::Command};

#[derive(Clone, Debug)]
pub struct MediaInput {
    pub target: String,
    pub headers: Vec<(String, String)>,
}
impl MediaInput {
    pub fn configure(&self, cmd: &mut Command) {
        configure_input(cmd, &self.target, &self.headers);
    }
}
pub(crate) fn configure_input(cmd: &mut Command, target: &str, headers: &[(String, String)]) {
    if crate::bundle::is_network(std::path::Path::new(target)) {
        cmd.args(["-rw_timeout", "15000000"]);
        if !headers.is_empty() {
            let mut block = String::new();
            for (name, value) in headers {
                // Headers are metadata; never allow an extra header line from a value.
                if !name.is_empty()
                    && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                    && !value.contains(['\r', '\n'])
                {
                    block.push_str(&format!("{name}: {value}\r\n"));
                }
            }
            if !block.is_empty() {
                cmd.args(["-headers", &block]);
            }
        }
    }
    cmd.arg("-i").arg(target);
}
pub fn is_youtube_url(target: &str) -> bool {
    let Some((scheme, rest)) = target.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    matches!(
        host.as_str(),
        "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "music.youtube.com"
            | "youtu.be"
            | "www.youtu.be"
            | "youtube-nocookie.com"
            | "www.youtube-nocookie.com"
    )
}
pub(crate) struct ResolvedMedia {
    pub video: MediaInput,
    pub audio: Option<MediaInput>,
}
fn runtime_args(runtimes: &[(&str, PathBuf)]) -> Vec<String> {
    runtimes
        .iter()
        .flat_map(|(name, path)| ["--js-runtimes".into(), format!("{name}:{}", path.display())])
        .collect()
}
pub(crate) fn resolve(target: &str, control: &LoadControl) -> Result<ResolvedMedia> {
    let runtimes: Vec<_> = ["deno", "node"]
        .into_iter()
        .filter_map(|name| tool_path(name).map(|path| (name, path)))
        .collect();
    anyhow::ensure!(!runtimes.is_empty(), "YouTube needs Deno 2.3+ or Node.js 22+. On Windows, run scripts/install-media.ps1 -YouTubeOnly, then retry.");
    let mut cmd = command("yt-dlp");
    cmd.args([
        "--ignore-config",
        "--no-playlist",
        "--dump-single-json",
        "--socket-timeout",
        "10",
        "--retries",
        "1",
        "--extractor-retries",
        "1",
    ])
    .args(runtime_args(&runtimes))
    .args(["-f", "bv*+ba/b/bv*", "--", target]);
    let out = control.output(&mut cmd).context("YouTube needs an up-to-date yt-dlp in tools or PATH; run scripts/install-media.ps1 -YouTubeOnly")?;
    anyhow::ensure!(out.status.success(), "YouTube resolver: {}\nUpdate yt-dlp and its JavaScript runtime with scripts/install-media.ps1 -YouTubeOnly if extraction fails.", String::from_utf8_lossy(&out.stderr).trim());
    parse_streams(
        &serde_json::from_slice(&out.stdout).context("YouTube resolver returned invalid JSON")?,
    )
}
fn headers(format: &Value, parent: &Value) -> Vec<(String, String)> {
    format
        .get("http_headers")
        .or_else(|| parent.get("http_headers"))
        .and_then(Value::as_object)
        .map(|headers| {
            headers
                .iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|value| (name.clone(), value.into()))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn input(format: &Value, parent: &Value) -> Result<MediaInput> {
    let target = format["url"]
        .as_str()
        .context("YouTube format has no playable URL")?;
    anyhow::ensure!(
        crate::bundle::is_network(std::path::Path::new(target)),
        "YouTube format URL must use HTTP or HTTPS"
    );
    Ok(MediaInput {
        target: target.into(),
        headers: headers(format, parent),
    })
}
fn has_codec(format: &Value, key: &str) -> bool {
    format[key]
        .as_str()
        .is_some_and(|codec| !codec.is_empty() && codec != "none")
}
fn parse_streams(json: &Value) -> Result<ResolvedMedia> {
    let formats: Vec<&Value> = json
        .get("requested_formats")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_else(|| vec![json]);
    let video = formats
        .iter()
        .copied()
        .find(|format| has_codec(format, "vcodec"))
        .context("YouTube returned no playable video stream")?;
    let audio = if has_codec(video, "acodec") {
        None
    } else {
        formats
            .iter()
            .copied()
            .find(|format| has_codec(format, "acodec"))
            .map(|format| input(format, json))
            .transpose()?
    };
    Ok(ResolvedMedia {
        video: input(video, json)?,
        audio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn selected_pair_retains_both_urls_and_per_stream_headers() {
        let json = json!({"http_headers":{"Referer":"https://www.youtube.com/"},"requested_formats":[
            {"url":"https://cdn.example/video","vcodec":"vp9","acodec":"none","http_headers":{"User-Agent":"video-agent"}},
            {"url":"https://cdn.example/audio","vcodec":"none","acodec":"opus"}
        ]});
        let streams = parse_streams(&json).unwrap();
        assert_eq!(streams.video.target, "https://cdn.example/video");
        assert_eq!(
            streams.video.headers[0],
            ("User-Agent".into(), "video-agent".into())
        );
        let audio = streams.audio.unwrap();
        assert_eq!(audio.target, "https://cdn.example/audio");
        assert_eq!(audio.headers[0].0, "Referer");
    }
    #[test]
    fn combined_and_silent_video_formats_remain_supported() {
        for acodec in ["aac", "none"] {
            let result = parse_streams(
                &json!({"url":"https://cdn.example/muxed","vcodec":"h264","acodec":acodec}),
            )
            .unwrap();
            assert!(result.audio.is_none());
            assert_eq!(result.video.target, "https://cdn.example/muxed");
        }
    }
    #[test]
    fn audio_only_empty_and_local_file_resolver_results_are_rejected() {
        for json in [
            json!({}),
            json!({"url":"https://cdn.example/audio","vcodec":"none","acodec":"opus"}),
            json!({"url":"file:///private","vcodec":"h264"}),
        ] {
            assert!(parse_streams(&json).is_err());
        }
    }
    #[test]
    fn runtimes_are_explicit_and_paths_with_spaces_are_one_argument() {
        let args = runtime_args(&[("node", PathBuf::from("C:/Program Files/nodejs/node.exe"))]);
        assert_eq!(
            args,
            ["--js-runtimes", "node:C:/Program Files/nodejs/node.exe"]
        );
    }
    #[test]
    fn youtube_detection_checks_the_url_host() {
        for target in [
            "https://www.youtube.com/watch?v=abc",
            "https://youtu.be/abc",
            "HTTPS://M.YOUTUBE.COM/shorts/abc",
            "https://music.youtube.com/watch?v=abc",
        ] {
            assert!(is_youtube_url(target));
        }
        for target in [
            "C:/clips/youtube.com/clip.mp4",
            "https://youtube.com.example/watch",
            "https://evil.example/youtube.com/watch",
            "https://youtube.com@evil.example/watch",
            "https://youtube.com:443@evil.example/watch",
            "https://example.com/?ref=youtu.be/abc",
        ] {
            assert!(!is_youtube_url(target));
        }
    }
    #[test]
    fn input_headers_cannot_inject_extra_lines_and_local_files_get_no_http_options() {
        let input = MediaInput {
            target: "https://cdn.example/video".into(),
            headers: vec![
                ("User-Agent".into(), "player".into()),
                ("Referer".into(), "bad\r\nCookie: injected".into()),
                ("bad:name".into(), "value".into()),
            ],
        };
        let mut cmd = Command::new("ffmpeg");
        input.configure(&mut cmd);
        let args: Vec<_> = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-rw_timeout",
                "15000000",
                "-headers",
                "User-Agent: player\r\n",
                "-i",
                "https://cdn.example/video"
            ]
        );
        let mut cmd = Command::new("ffmpeg");
        configure_input(&mut cmd, "movie.mp4", &input.headers);
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["-i", "movie.mp4"]);
    }
}
