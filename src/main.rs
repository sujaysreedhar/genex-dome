use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use genex_dom_player::{
    app::{DomeApp, Launch},
    mesh::WarpMesh,
    optics::Optics,
    projection::{self, Pattern},
    state::{Output, PlaylistItem, Projection, Session},
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "GenexDomPlayer",
    version,
    about = "Rust dome media and projection workstation"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(value_name = "MEDIA")]
    files: Vec<PathBuf>,
    #[arg(long)]
    session: Option<PathBuf>,
    #[arg(long)]
    mesh: Option<PathBuf>,
    #[arg(long, default_value = "equirectangular")]
    projection: String,
    #[arg(long)]
    autoplay: bool,
    #[arg(long)]
    projector: bool,
    #[arg(long)]
    projector_windowed: bool,
    #[arg(long)]
    fullscreen: bool,
    #[arg(long, hide = true)]
    screenshot: Option<PathBuf>,
    #[arg(long, hide = true)]
    gpu_check: Option<PathBuf>,
    /// Inspector panel to open on startup.
    #[arg(long, value_parser = ["projection", "calibration", "output"])]
    panel: Option<String>,
}
#[derive(Subcommand)]
enum Commands {
    /// Generate a spherical-mirror .data warp map from optical geometry.
    Mesh {
        #[arg(long)]
        geometry: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long,default_value_t=16./9.)]
        aspect: f32,
        #[arg(long)]
        write_geometry: Option<PathBuf>,
    },
    /// Render a projected still image without opening a window.
    Export {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        session: Option<PathBuf>,
        #[arg(long)]
        mesh: Option<PathBuf>,
        #[arg(long, default_value = "equirectangular")]
        projection: String,
        #[arg(long, default_value = "fisheye")]
        mode: String,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 0.)]
        yaw: f32,
        #[arg(long, default_value_t = 0.)]
        pitch: f32,
    },
    /// Write a built-in test source to PNG.
    Pattern {
        #[arg(long, default_value = "polar")]
        kind: String,
        #[arg(long, default_value_t = 1024)]
        size: u32,
        #[arg(long)]
        output: PathBuf,
    },
    /// Check the media runtime and basic projection configuration.
    Doctor,
}
fn parse_projection(s: &str) -> Result<Projection> {
    Ok(match s.to_lowercase().as_str() {
        "equirectangular" | "spherical" | "360" => Projection::Equirectangular,
        "fisheye" => Projection::Fisheye,
        "cylindrical" => Projection::Cylindrical,
        "perspective" | "standard" => Projection::Perspective,
        "cubemap" | "cube" => Projection::Cubemap,
        "prewarped" => Projection::Prewarped,
        _ => anyhow::bail!("Unknown projection: {s}"),
    })
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        match command {
            Commands::Doctor => {
                println!("{}", genex_dom_player::media::runtime_status());
                let mesh = Optics::default().generate()?;
                println!(
                    "Default calibration: {} active nodes",
                    mesh.nodes.iter().filter(|n| n.intensity >= 0.).count()
                );
                println!("Desktop renderer: OpenGL 3.3; image export: CPU");
            }
            Commands::Pattern { kind, size, output } => {
                anyhow::ensure!((16..=8192).contains(&size), "Pattern size must be 16–8192");
                let p = match kind.as_str() {
                    "polar" => Pattern::PolarGrid,
                    "cartesian" => Pattern::CartesianGrid,
                    "bars" => Pattern::ColourBars,
                    "panorama" => Pattern::Panorama,
                    _ => anyhow::bail!("Unknown pattern {kind}"),
                };
                projection::pattern(p, size).save(output)?;
            }
            Commands::Mesh {
                geometry,
                output,
                aspect,
                write_geometry,
            } => {
                let mut optics: Optics = if let Some(p) = geometry {
                    serde_json::from_str(&std::fs::read_to_string(p)?)?
                } else {
                    Optics::default()
                };
                optics.aspect = aspect;
                let mesh = optics.generate()?;
                anyhow::ensure!(
                    mesh.nodes.iter().any(|n| n.intensity >= 0.),
                    "No rays hit the dome; adjust optical geometry"
                );
                mesh.save(&output)?;
                if let Some(p) = write_geometry {
                    std::fs::write(p, serde_json::to_string_pretty(&optics)?)?;
                }
                println!(
                    "Saved {} × {} calibration to {}",
                    mesh.width,
                    mesh.height,
                    output.display()
                );
            }
            Commands::Export {
                input,
                output,
                session,
                mesh,
                projection: proj,
                mode,
                width,
                height,
                yaw,
                pitch,
            } => {
                let from_session = session.is_some();
                let mut s = if let Some(p) = session {
                    Session::load(&p)?
                } else {
                    Session::default()
                };
                if !from_session {
                    s.view.input = parse_projection(&proj)?;
                    if s.view.input == Projection::Perspective {
                        s.view.source_fov = 90.;
                    }
                    s.view.yaw = yaw;
                    s.view.pitch = pitch;
                    s.view.output = match mode.as_str() {
                        "fisheye" => Output::Fisheye,
                        "mirror" => Output::Mirror,
                        "perspective" => Output::Perspective,
                        "source" => Output::Source,
                        _ => anyhow::bail!("Unknown output mode {mode}"),
                    };
                }
                if let Some(p) = mesh {
                    s.mesh = Some(WarpMesh::load(&p)?);
                    s.view.output = Output::Mirror;
                }
                let (_, source) =
                    genex_dom_player::media::load(&input.to_string_lossy(), 0., 8192)?;
                projection::render_cpu(&source, &s.view, s.mesh.as_ref(), width, height)?
                    .save(&output)?;
                println!(
                    "Saved {} × {} projection to {}",
                    width,
                    height,
                    output.display()
                );
            }
        }
        return Ok(());
    }
    let mut session = if let Some(p) = cli.session {
        Session::load(&p)?
    } else {
        Session::default()
    };
    let input = parse_projection(&cli.projection)?;
    for path in cli.files {
        session.playlist.push(PlaylistItem {
            path,
            projection: input,
        });
    }
    let mesh_path = cli.mesh.or_else(|| {
        ["default.data", "warpmap.data"]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
    });
    if let Some(path) = mesh_path {
        session.mesh = Some(WarpMesh::load(&path)?);
        session.view.output = Output::Mirror;
    }
    let launch = Launch {
        session,
        autoplay: cli.autoplay,
        projector: cli.projector || cli.projector_windowed,
        projector_windowed: cli.projector_windowed,
        fullscreen: cli.fullscreen,
        screenshot: cli.screenshot,
        gpu_check: cli.gpu_check,
        panel: cli.panel,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440., 900.])
            .with_min_inner_size([1050., 650.])
            .with_title("GenexDomPlayer · Projection Studio"),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "GenexDomPlayer Projection Studio",
        options,
        Box::new(move |cc| {
            let app = DomeApp::new(cc, launch)?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
fn main() {
    if let Err(e) = run().context("GenexDomPlayer") {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
