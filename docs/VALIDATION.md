# Validation record

## Self-contained Windows distribution — 13 October 2026 (0.2.0)

- Rebuilt the `.exe` installer and portable ZIP with FFmpeg/FFprobe `N-127367-gb5902e69c7-20261011`, official yt-dlp `2026.08.19` and Deno `2.9.7`. Tools live in `tools` and are discovered relative to the application. Setup performs no software downloads; the earlier download helper is retired during upgrades.
- Pinned and checked upstream archive/executable SHA-256 digests. Included a runtime version/hash manifest, upstream licence notices, FFmpeg and yt-dlp source snapshots, and the FFmpeg build-recipe snapshot. Build-time preparation caches inputs; installed users need no Rust, Python, Node installation or media-tool downloads.
- All 410 payload checksums passed for the extracted ZIP, temporary install and reinstall. From an unrelated folder with only Windows system directories on PATH, FFmpeg/FFprobe and yt-dlp started, Deno evaluated JavaScript, and the application discovered the bundled runtime.
- Bundled FFmpeg generated an H.264/AAC clip. The application exported its opening frame and decoded all 12 frames at 12 fps with zero late or overwritten frames in the one-second CPU-delivery benchmark. This does not measure audible output or projector performance.
- A live preview of `https://www.youtube.com/watch?v=IrolgreFj2w` passed using the packaged application, a clean PATH and a fresh Deno cache. No external software installation or challenge-component download was requested.
- Silent install, reinstall and uninstall passed. Reinstall retained a user session; uninstall removed the application, its bundled tools and uninstall registration while preserving the user session and an additional file in `tools`. PowerShell scripts parsed and Git whitespace checks passed.

The bundled Windows distribution requires Windows 10 22H2 or Windows 11 x64 and remains unsigned. Local media works offline; online content requires internet. Clean second-machine acceptance and physical output remain installation checks. The historical installer-only validation below describes the earlier package before runtime bundling.

## Windows installer and distribution — 13 October 2026 (0.2.0)

- Built an Inno Setup installer and portable ZIP with runtime DLLs, examples, documentation, font/dependency licences, launch helpers and a SHA-256 payload manifest. The compiler bootstrap uses the official Inno Setup 7.1.0 download with its published SHA-256 digest.
- The installer targets Windows 10/11 x64, installs per user, offers a desktop shortcut, adds Start menu entries and records files for uninstall. The media helper accepts an installation-local `-RuntimeDir` and preserves the existing `-YouTubeOnly` option and Deno download.
- Tested the extracted ZIP and installed payload from paths containing spaces, with only Windows system directories on PATH and an unrelated working directory. All 396 payload hashes matched. Executable startup, the three-cue demo asset check, pattern generation and projected image export passed.
- A temporary silent install, reinstall and uninstall passed. Reinstall retained a user session; uninstall removed the application and its registration while preserving that session and a separately created file in `tools`. The initial sandboxed install was denied registry access and rolled back; the same lifecycle passed with the required test access.
- All PowerShell packaging scripts parsed successfully. The workflow in `.github/workflows/package.yml` defines packaging and installer lifecycle validation on tags or manual runs; it has not run on GitHub. Its MSVC branch requests a static C runtime; the locally validated package uses the LLVM MinGW toolchain.

The distributed artifacts are unsigned. External media executables are downloaded separately by the included helper. Validation above covers this Windows machine; acceptance on a clean second computer, physical dome/projector output and sustained playback remain installation checks.

## YouTube resolver repair — 13 October 2026 (0.2.0)

- Reproduced the failure for `IrolgreFj2w`: the previous resolver did not enable the installed Node runtime, and `best[ext=mp4]/best` still failed after Node was enabled because this source had no available combined video/audio stream.
- The resolver now explicitly enables detected Deno/Node runtimes and selects `bv*+ba/b/bv*`. Video and audio inputs retain their own URLs and HTTP headers, and paused seeking resolves the original source again. Network input reads have a 15-second timeout; the existing shared 30-second load deadline and cancellation controls remain in use.
- The Windows media installer now includes portable Deno and supports `-YouTubeOnly` to update yt-dlp and Deno without reinstalling FFmpeg. Locally verified yt-dlp 2026.08.19, Deno 2.9.7 and Node 22.20.0.
- Formatting and Clippy pass with warnings denied. All 56 standard tests and both optional local FFmpeg tests pass. Seven new standard tests cover split/combined/silent formats, stream-specific audio routing, runtime arguments, host matching and HTTP headers.
- The opt-in live YouTube test passes for `https://www.youtube.com/watch?v=IrolgreFj2w`: preview, separate stereo PCM audio decoding, timed video delivery and paused seek retain both streams. The source was probed at 2880 × 2160 with a 42-second duration; the integration test decodes at a 360-pixel height. This test requires internet and is excluded from the offline CI suite.
- Rebuilt the Windows release and local distribution package. Visually inspected the native release preview in `test-output/youtube-fixed.png`; the resolver metadata/check files in that ignored folder are transient and are not part of the repository.

Audible speaker playback and physical A/V synchronization remain unverified. YouTube availability and extraction can change with the service and installed resolver versions.

## Show production and performance — 13 October 2026 (0.2.0)

- Added per-cue source-view presets, names, durations, entry fades through black with audio gain, pause-aware still timing, and next-cue probe/opening-frame preloading. Non-looping shows stop at the final cue; EOF is retained if a short movie ends during its incoming fade.
- Added portable folder bundles with staged export, local-asset deduplication, relative paths, network-cue preservation and installation-specific display reset. Relinking changes missing files only when a filename match is unique.
- Added decode/delivery FPS, late and overwritten frame counters, CPU texture-upload timings, software A/V clock offsets, JSON reports and a CLI decode benchmark. Movies now use reported average source cadence rather than a fixed 30 fps; variable-rate material is normalized to that cadence.
- Formatting and Clippy pass with warnings denied. 49 standard tests pass (26 core, 8 reliability, 11 show, 4 subprocess/helper tests). The show tests exercise legacy compatibility, preset isolation, invalid cue rejection, pause/loading fade boundaries, final-cue policy, preload invalidation, bundle relocation, duplicate filenames, staged-export failure, ambiguous relinking and source-time versus still-time durations.
- Both optional FFmpeg tests pass: existing preview/seek/audio/rewind/shutdown coverage and new 24/60-fps cadence, late/overwritten frame accounting and benchmark checks. Audio-device coverage remains conditional; audible synchronization was not established.
- All 41 existing CPU/GPU comparisons pass. The Show editor was visually inspected. The native three-cue image demo ran to **Show complete**. A mixed still/24-fps movie show with separate output also completed; its 0.33-second final movie ended inside a 1-second incoming fade, and all 8 movie frames were delivered with zero drops.
- A generated H.264 3840 × 2160, 30-fps, 3-second clip delivered all 90 frames with zero late or overwritten drops in the CLI decode benchmark. This measures short timed decoding and CPU delivery without GPU upload or audio; it does not establish sustained 4K projector performance.
- CLI bundle export and asset checking passed for the demo. Relocation and missing-original-media checks also passed in automated tests. Windows release and distribution package were rebuilt at 0.2.0.

Screenshots and reports are excluded from Git under `test-output/`: `phase2-show.png`, `phase2-complete.png`, `phase2-mixed-complete.png`, `phase2-gpu.txt` and `phase2-4k-decode.json`. The reproducible demo is `assets/show.genexdomplayer.json`; its generated patterns are in `assets/show/`.

Remaining installation acceptance: run a mixed-media bundle on a second physical machine and the intended dome; measure long-run frame delivery, speaker A/V synchronization and display reconnection on target hardware. Preloading caches one opening frame and probe result; it does not prestart the video/audio decoders or guarantee gapless transitions. Bundle network cues require internet and external media tools. Physical calibration, audible output and sustained 4K performance remain unverified.

## Reliability and operator update — 12 October 2026

- Formatting and Clippy pass with warnings denied.
- 26 existing core tests and 8 new reliability tests pass. The new coverage includes source replacement, cancelled results, invalid calibration preservation, grouped undo/redo, atomic saves and backup failure, prewarped mesh bypass, and negative/mixed-DPI coordinate conversion.
- 3 subprocess regression tests pass for timeout, cancellation and synchronous cleanup when a load job is dropped (plus their child-process test helper).
- The FFmpeg integration test passes, including audio-clock catch-up and paused rewind preview. It uses silent media and does not establish audible A/V synchronization on a real device.
- 41 CPU/GPU comparisons pass in debug and release. The added case checks prewarped output without a mesh and same-size texture updates.
- Fresh Projection, Output and Calibration screenshots were inspected. The release was opened with the detected Windows display selected and its separate output window enabled. Screenshots are in `test-output/fixed-console.png`, `test-output/final-output.png`, `test-output/final-calibration.png` and `test-output/release-display.png`; GPU reports are `test-output/final-gpu.txt` and `test-output/release-final-gpu.txt`.
- The launcher release and `dist/GenexDomPlayer` package were rebuilt. A Windows GitHub Actions workflow now defines format, lint, tests, media integration and release build checks; it has not yet run on GitHub.

Physical multi-monitor placement/reconnection, a real dome/projector, live YouTube, audible A/V output and sustained 4K playback remain installation checks. Local image decoding observes cancellation at decode boundaries; the subprocess cancellation deadline applies to URL resolution, probing and movie preview extraction.

## Initial build

Validated on Windows x86-64, 11 October 2026.

- Rust release executable built with the workspace-local Rust 1.99 toolchain and LLVM MinGW linker.
- `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings` pass.
- 26 core integration tests pass, covering projection geometry, the Bourke mesh parser, node masks, optics, stereo sampling, colour correction, backdrops and session paths.
- The optional media integration test passes using a generated H.264/AAC video. It verifies preview, seeking, timed frame decoding, PCM decoding, end-of-file and process shutdown.
- 40 offscreen GPU comparisons pass against CPU output: six input projections, four output modes, all five mesh coordinate formats, nine stereo combinations, a custom backdrop, and intensity/masking. The comparison threshold is mean channel error ≤3/255; ordinary texture interpolation differences are expected.
- The native operator console and separate projector viewport were exercised and captured through the application's own screenshot facility.
- CLI pattern creation, projected PNG export and optical mesh generation were exercised.

This machine reports no audio output device, so audible speaker playback has not been verified. A real dome/projector, YouTube service, live desktop capture, and sustained 4K playback have not been exercised. Windows capture is implemented through FFmpeg gdigrab; Linux uses x11grab; macOS capture is unavailable. Frame-sequential stereo and continuous reverse playback are outside this build; packed stereo and backward seeking are provided.

Runtime checks and screenshots are in `test-output/`, which is excluded from Git. The generated optical example is `assets/example-mirror.data`; its input geometry is `assets/optics.json`. The example requires physical adjustment for an actual installation.
