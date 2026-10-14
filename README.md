# GenexDome

**Open-source dome projection, media playback, and calibration.**

GenexDome brings 360° media, fulldome playback, and projector calibration into one native desktop application. Load an image or movie, choose its projection, adjust the view, and send the result to a separate projector window.

Built with **Rust and OpenGL**, it supports direct fisheye projection and spherical mirror installations, with editable warp meshes and reusable show settings. It is intended for planetariums, immersive displays, educational spaces, and dome experiments.

**Repository:** [sujaysreedhar/genex-dome](https://github.com/sujaysreedhar/genex-dome)  
**Current version:** `0.2.0` · **License:** [MIT](LICENSE)

The application currently uses the executable name `GenexDomPlayer` and the Cargo package name `genex-dom-player`. The launch commands and session filenames below use those existing names.

## Features

| Feature | What you can do |
| --- | --- |
| Media playback | Play images and FFmpeg-supported movies with playlists, scrubbing, looping, automatic advance, playback speed, and audio volume controls. |
| Projection conversion | Use equirectangular, fisheye, cylindrical, perspective, cubemap, and prewarped sources. |
| 360° navigation | Adjust yaw, pitch, roll, and perspective field of view interactively. |
| Dome output | Choose direct fisheye or spherical mirror / mesh output, with field of view, image flips, and off-centre fisheye controls. |
| Mirror calibration | Generate warp meshes from dome, mirror, and projector geometry; preview and apply a warp; edit nodes with undo and redo. |
| Warp compatibility | Import and export Bourke `.data` meshes with XYUV coordinates, intensity correction, and masking. |
| Image correction | Adjust brightness, gamma, per-node intensity, and estimated brightness compensation. |
| Stereo media | Use mono, side-by-side, and top/bottom input and output layouts. |
| Presentation tools | Display warped multiline text, use custom fisheye backdrops, or warp a captured desktop region. |
| Network media | Open playable network URLs and resolve YouTube URLs through external `yt-dlp`. |
| Projector window | Keep the operator console and projector output in separate windows, with placement and fullscreen controls. |
| Saved sessions | Save playlists, projection settings, optical geometry, and meshes in JSON sessions. |
| Show cues | Name cues, store source-view presets, set durations, fade through black with audio, and preload the next opening frame. |
| Portable shows | Export a folder with local media, backdrop, calibration and cues; open it elsewhere and relink missing media. |
| Playback diagnostics | Inspect decode/delivery FPS, late and replaced frames, texture upload time and video/audio clock offset; save JSON reports. |
| Command-line tools | Generate test patterns and optical meshes, export projected PNGs, and check the media runtime. |

## Requirements

- **Rust 1.88 or newer** and Cargo to build from source. See the [official Rust installation guide](https://rust-lang.org/tools/install/). On Windows, install the C++ build tools required by your chosen Rust toolchain.
- **OpenGL 3.3** for the desktop renderer.
- **FFmpeg and FFprobe** for movie playback, audio decoding, and desktop capture.
- **yt-dlp** and **Deno 2.3+** (or **Node.js 22+**) for YouTube URL resolution. See [yt-dlp's JavaScript runtime requirements](https://github.com/yt-dlp/yt-dlp/wiki/EJS).
- A projector and calibrated dome setup for physical dome output. You can explore the built-in scene, images, and calibration tools on a regular monitor.

Images, built-in patterns, and calibration do not require the external media tools.

Windows has been built and exercised locally. Linux builds require the appropriate X11/Wayland and ALSA development libraries for your distribution; Linux and macOS have not been validated here. Desktop capture uses FFmpeg `gdigrab` on Windows and `x11grab` on Linux; macOS desktop capture is currently unavailable.

## Quick start

Install the prerequisites, then clone the repository and build the application:

```sh
git clone https://github.com/sujaysreedhar/genex-dome.git
cd genex-dome
cargo build --release --locked
cargo run --release --locked
```

The application opens a built-in 360° scene. Drag the preview to look around. Use **Add media**, **Folder**, or drag and drop to populate the playlist, then select the correct input projection for each item.

To open a fulldome movie directly:

```sh
cargo run --release --locked -- --projection fisheye "path/to/show.mp4"
```

### Windows launcher and media tools

After installing Rust and its build prerequisites, you can also double-click [Start-GenexDomPlayer.cmd](Start-GenexDomPlayer.cmd). The launcher uses an existing release or debug executable, or runs Cargo to build and start the application.

To install portable Windows copies of FFmpeg, FFprobe, yt-dlp, and Deno, run this from PowerShell in the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-media.ps1
. .\scripts\env.ps1
cargo run --release --locked
```

The installer downloads the media tools into `.tools/media`. The environment script configures the current shell to use available project-local tools. `.tools/` is excluded from Git and is **not included in a fresh clone**; install Rust separately unless you already have a local toolchain there.

You can also place the media executables beside `GenexDomPlayer.exe` or make them available through your system `PATH`.

For a YouTube resolver update without reinstalling FFmpeg, run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-media.ps1 -YouTubeOnly
```

The player explicitly enables available Deno and Node runtimes. It resolves video and audio separately when a combined stream is unavailable, forwarding each stream's HTTP headers to FFmpeg. Paused seeking retains the original YouTube URL and resolves a fresh pair of streams. Restart an already running player after rebuilding it.

### Try the included sessions

```sh
cargo run --release --locked -- --session assets/calibration.genexdomplayer.json
cargo run --release --locked -- --session assets/message.genexdomplayer.json
```

These open a polar calibration grid and a sample dome message. [assets/optics.json](assets/optics.json) and [assets/example-mirror.data](assets/example-mirror.data) provide example optical geometry and a generated warp mesh.

### Build and run a show

Open **Show** in the inspector after adding media to the library. For each selected cue, give it a name and adjust its view in **Projection**, then choose **Store current view** in Show. Presets store source projection, yaw, pitch, roll, source/view field of view and input stereo; the session retains output calibration, brightness, gamma and projector settings. **Use current view as show default** sets the source view used by cues without presets.

Enable **Custom duration** to hold an image for a chosen number of seconds, or stop a movie at that media time. Otherwise images use **s / slide** and movies run to their end. Image timing pauses with playback; movie durations follow media time, including speed changes. **Fade through black on entry** splits its time equally between outgoing and incoming fades. Output remains black while a source loads, and audio volume follows the fade. Fades add outgoing time; the incoming fade counts toward the new cue's duration. A cue that ends during its incoming fade advances after the fade completes.

Enable **Auto advance** to run the cue list. With **Loop** enabled the whole show repeats; with Loop off it stops on the final cue. Without Auto advance, Loop repeats only the current cue. The next cue's probe result and opening frame preload in the background. This avoids repeating a completed preview load; video/audio decoders still start when the cue activates, so this is not guaranteed gapless playback. Failed or cancelled loads stop that activation with black output and an error.

Try the included three-cue show, which demonstrates presets, durations, fades and automatic stopping:

```sh
cargo run --release --locked -- --session assets/show.genexdomplayer.json --panel show --autoplay
```

Under **Show → Portable show**, choose **Export show bundle…** and select a parent folder. Export creates a new folder using the entered name, copying local media and the backdrop once per unique file with relative paths. It embeds the mesh and show settings and clears the installation-specific display selection. Existing bundle folders are preserved; missing local assets prevent export. Network cues remain URLs and require internet and their runtime tools.

Copy the entire folder to another computer, then choose **Open bundle** or open its `show.genexdomplayer.json` manifest. Select and verify the destination projector before opening output. **Relink folder** searches a selected folder and its subfolders for unique filename matches to missing assets; ambiguous matches stay unresolved. Use a cue's **··· → Replace / relink file…** for an exact replacement. Save the session after relinking. Ordinary session saves retain absolute media paths; use a new bundle export when you need portability again.

**Playback diagnostics** in Show report averages for the current decoder run. Delivered frames reach the console; projector refresh is not measured. Late frames missed their clock deadline, while replaced frames were overwritten before the console consumed them. Upload time measures CPU submission to OpenGL, and video/audio offset compares software clocks rather than speaker latency. **Save diagnostics JSON** exports those measurements. Movies use their reported average frame rate, with variable-rate material normalized to that cadence and unknown rates falling back to 30 fps.

## Set up a dome

1. Configure your operating system to use an extended desktop for the operator display and projector.
2. Load media and choose its input projection in **Projection**. A typical fulldome fisheye is a square 180° image; a typical equirectangular panorama has a 2:1 aspect ratio.
3. Choose **Direct fisheye** for a fisheye lens, or **Spherical mirror / mesh** for a mirror installation.
4. In **Calibration**, load your site's `.data` mesh. To create one, enter measured dome, mirror, and projector geometry, select **Generate warp preview**, and then **Apply warp**.
5. Display the polar grid, adjust geometry and mesh nodes against the physical dome, and save the calibrated `.data` file.
6. In **Output**, choose the projector under **Output display**, use **Identify display** to check it, then open output and enable fullscreen. Automatic display selection is available on Windows. Manual placement remains available; its coordinates use egui logical pixels.
7. Use **Save session** to keep the playlist and calibration for the next show.

The example geometry is a starting point and must be adjusted for your installation. The optical model assumes a spherical dome, spherical mirror, and pinhole projector. Physical alignment, colour calibration, and playback performance need to be checked on your hardware.

## Controls

| Key or gesture | Action |
| --- | --- |
| Space | Play / pause |
| Arrow keys or drag | Navigate the view |
| Shift + drag | Rotate the fisheye view |
| Mouse wheel | Adjust perspective field of view |
| Home | Reset orientation |
| Comma / period | Seek backward / forward 10 seconds |
| B | Toggle blackout |
| F11 | Toggle console fullscreen |
| Escape | Leave fullscreen |

## Media and projection formats

Media loads have a **Cancel load** action and a 30-second deadline across URL resolution, probing, and preview extraction. Cancelling or closing the application terminates the loading subprocess. Local image decoding checks cancellation before and after decoding. Stop rewinds a video and refreshes its opening frame while remaining paused.

Calibration generation first shows a coverage preview. **Apply warp** commits it, while **Undo/Redo** retains up to 20 mesh changes, including node drags. Unusable generated meshes keep the previous calibration. The **Modified** indicator tracks changes since the last save.

Session and mesh saves use atomic replacement and keep the previous file alongside the original as **`.bak`**. Recover by copying that backup to the original extension and opening it. The header always exposes Blackout and output status. If a selected Windows display disconnects, output closes rather than moving to another display. Use `--panel calibration` or `--panel output` to open an inspector panel at startup.

**Images:** PNG, JPEG, TGA, BMP, TIFF, WebP, and GIF. GIF loading uses the first frame.

**Movies:** common FFmpeg-supported formats, including MP4, MOV, MKV, AVI, and WebM. Available codecs depend on your FFmpeg build.

**Cubemaps:** a 3 × 2 atlas with square faces in this order:

```text
+X  -X  +Y
-Y  +Z  -Z
```

Forward is +Z, up is +Y, and right is +X. The fisheye image centre corresponds to the forward axis.

**Warp meshes:** Bourke `.data` text files use this structure:

```text
2
Nx Ny
x y u v intensity
...
```

Header codes 1–5 identify perspective, fisheye, cylindrical, spherical, and cubic coordinates. Nodes are row-major from the bottom left; screen X is normalised to ±aspect and Y to ±1. The importer infers aspect from the maximum absolute X coordinate and converts bottom-up mesh V coordinates to top-down image coordinates.

An intensity of zero creates a black mask. Negative intensity excludes the connected mesh quad. Mesh-node dragging selects the nearest node when the drag begins.

## Command-line examples

Run these commands from the repository root. On Windows, load `.\scripts\env.ps1` first if you use project-local tools.

```sh
# Check the media runtime and default optical geometry.
cargo run --release --locked -- doctor

# Generate calibration and panorama patterns.
cargo run --release --locked -- pattern --kind polar --output polar.png
cargo run --release --locked -- pattern --kind panorama --output panorama.png

# Generate a mirror warp from the included optical geometry.
cargo run --release --locked -- mesh --geometry assets/optics.json --aspect 1.7777778 --output warpmap.data

# Convert a panorama into a square fisheye PNG.
cargo run --release --locked -- export --input panorama.png --projection equirectangular --yaw 45 --output fisheye.png --width 1024 --height 1024

# Export a movie's first frame using a saved session's view settings.
cargo run --release --locked -- export --input movie.mp4 --session show.genexdomplayer.json --output projected.png

# Restore a session, start playback, and open the projector window.
cargo run --release --locked -- --session show.genexdomplayer.json --autoplay --projector

# Check assets and package a show into a new folder.
cargo run --release --locked -- check-show --session assets/show.genexdomplayer.json
cargo run --release --locked -- bundle --session assets/show.genexdomplayer.json --output my-show-bundle

# Measure timed decoding and CPU frame delivery without a window or audio device.
cargo run --release --locked -- benchmark --input movie.mp4 --seconds 10 --max-height 2160 --output decode-report.json
```

`export --session` uses the session's view settings. Without a session, use `--projection`, `--mode`, `--yaw`, and `--pitch` to configure the export. `--mesh` selects mesh output. Movie exports use the first frame.

At startup, `default.data` or `warpmap.data` in the working directory is loaded automatically when no explicit `--mesh` is supplied. Saved local media paths are absolute; those files must remain accessible when you move a session to another computer.

For all available command-line options:

```sh
cargo run --release --locked -- --help
cargo run --release --locked -- export --help
```

## Development

| Directory | Contents |
| --- | --- |
| `src/` | Desktop UI, media playback, projection math, rendering, optics, calibration, and persistence. |
| `assets/` | GLSL shaders, example geometry, warp meshes, sessions, and the bundled font. |
| `scripts/` | Windows environment, launcher, media installation, and packaging helpers. |
| `tests/` | Core, reliability, and optional media integration tests. |
| `docs/` | Validation records and known testing limits. |

Run the standard checks:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

With FFmpeg and FFprobe installed, run the optional media integration test:

```sh
cargo test --locked --test media -- --ignored
```

See [the validation record](docs/VALIDATION.md) for the documented CPU/GPU comparisons, media checks, and installation limitations.

### Package for Windows

```powershell
.\scripts\install-inno.ps1  # first time: download the verified portable compiler
.\scripts\prepare-media-bundle.ps1  # build-time download of pinned tools and notices
.\scripts\package.ps1
.\scripts\test-package.ps1
```

This builds the release executable and produces these files in `dist/`:

- `GenexDome-0.2.0-windows-x64-setup.exe`: a Windows 10 22H2/11 x64 installer with Start menu shortcuts, an optional desktop shortcut, and uninstall support. It installs for the current user without administrator privileges.
- `GenexDome-0.2.0-windows-x64-portable.zip`: extract the whole archive and launch `GenexDomPlayer.exe` or `Start-GenexDome.cmd`.
- `SHA256SUMS.txt`: checksums for the installer and ZIP.

The unpacked payload is also available in `dist/GenexDomPlayer`. Both distributions include **FFmpeg, FFprobe, yt-dlp and Deno**, runtime DLLs for the local MinGW build, assets, documentation, third-party notices, upstream source snapshots, and checksum manifests. Rust, Cargo, Python, a separate JavaScript runtime and media-tool downloads are not needed on the destination computer. Generated package folders are replaced on each build; keep your shows outside `dist/`.

Setup and local media playback work offline. YouTube and other online sources require internet access to stream their content. The tools live in the application's `tools` folder and are discovered without PATH changes. See [the distribution quick start](docs/DISTRIBUTION.txt) and [bundled-media notices](docs/BUNDLED-MEDIA-NOTICES.txt). Updates replace packaged examples; uninstall removes bundled tools and preserves user-created files. The artifacts are unsigned.

The media preparation script caches checksum-verified inputs under `.tools/media-bundle`; subsequent builds reuse them. Versions and download hashes are pinned in `scripts/media-runtime.lock.json`. Refresh that lock and cached bundle when updating the tools; upstream daily FFmpeg assets have limited retention, so retain the verified cache for this release. `package.ps1` validates the prepared runtime before building either distribution. A custom prepared payload can be supplied with `-MediaBundleDir <path>`.

Use `-PortableOnly` to package without Inno Setup, `-InnoCompiler <path>` for an existing compiler, or `-SkipBuild` to reuse a release executable that matches the current version. `test-package.ps1` verifies checksums, all bundled tools, JavaScript execution and local H.264/AAC movie loading/decoding with a clean PATH. `-YouTubeUrl <url>` optionally exercises a live network preview. `-TestInstaller` additionally tests a temporary install, reinstall and uninstall; it refuses to run when an existing GenexDome installation is registered. Installer tests use no shortcuts and preserve their logs under `test-output/`.

## Contributing

Contributions are welcome, including bug fixes, projection improvements, calibration tools, documentation, and platform testing.

1. Open an issue describing the problem or proposed improvement.
2. Fork the repository and create a branch for your change.
3. Add focused tests for behaviour changes and run the relevant checks above.
4. Submit a pull request explaining the change and how you verified it.

For bug reports, include your operating system, GPU, input projection, media format, reproduction steps, and any error output. Calibration reports benefit from an example mesh and optical settings.

## Status and limitations

- The documented validation covers Windows builds, projection mathematics, GPU comparisons, CLI utilities, and generated media playback tests. See [docs/VALIDATION.md](docs/VALIDATION.md) for details.
- Physical dome/projector calibration, audible speaker playback, live desktop capture, and sustained 4K playback have not been verified in the recorded validation. YouTube preview, split-stream decoding and paused seeking were exercised with the source recorded in the validation notes.
- Stereo output uses side-by-side or top/bottom packing. Frame-sequential projector synchronisation is not implemented.
- Playback supports forward decoding and backward seeking. Continuous reverse playback is not implemented.
- YouTube playback depends on the source's availability and the external resolver.

## Acknowledgements

This project is an independent implementation of dome playback and calibration workflows informed by Paul Bourke's public projection resources:

- [iDome](https://paulbourke.net/dome/iDome/)
- [Warpplayer](https://paulbourke.net/dome/warpplayer/)
- [Spherical mirror projection tools](https://paulbourke.net/dome/fishwarp/)
- [Meshmapper](https://paulbourke.net/dome/meshmapper/)
- [Warp mesh format documentation](https://paulbourke.net/dome/warpingfisheye/)

It is an independent open-source application, not an official release of the original iDome companion software. It does not include the physical display hardware or the games and datasets shown in the original gallery.

## License

The application is released under the [MIT License](LICENSE).

The bundled Atkinson Hyperlegible font is covered by the [SIL Open Font License](assets/FONT-LICENSE.txt). External media tools and their dependencies retain their own licences.
