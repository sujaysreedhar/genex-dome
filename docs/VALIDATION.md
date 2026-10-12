# Validation record

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
