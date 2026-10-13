//! Portable folder bundles, committed only after all local assets copy successfully.
use crate::state::Session;
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub const MANIFEST: &str = "show.genexdomplayer.json";
pub fn is_network(path: &Path) -> bool {
    let value = path.to_string_lossy().to_ascii_lowercase();
    value.starts_with("http://") || value.starts_with("https://")
}
#[derive(Debug, Default, Serialize)]
pub struct BundleReport {
    pub copied_files: usize,
    pub network_cues: usize,
}
pub fn export(session: &Session, destination: &Path) -> Result<BundleReport> {
    session.validate()?;
    anyhow::ensure!(
        !destination.exists(),
        "Bundle folder already exists; choose a new folder name"
    );
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    anyhow::ensure!(parent.is_dir(), "Bundle parent folder does not exist");
    let stage = tempfile::Builder::new()
        .prefix(".genex-show-")
        .tempdir_in(parent)?;
    std::fs::create_dir(stage.path().join("media"))?;
    let mut saved = session.clone();
    let mut copies = HashMap::<PathBuf, PathBuf>::new();
    let mut report = BundleReport::default();
    let mut copy_asset = |path: &Path| -> Result<PathBuf> {
        let source = path
            .canonicalize()
            .with_context(|| format!("Missing media: {}", path.display()))?;
        anyhow::ensure!(
            source.is_file(),
            "Media is not a file: {}",
            source.display()
        );
        if let Some(relative) = copies.get(&source) {
            return Ok(relative.clone());
        }
        let name: String = source
            .file_name()
            .context("Asset has no filename")?
            .to_string_lossy()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || ".-_".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let relative = PathBuf::from(format!("media/{:04}-{name}", copies.len() + 1));
        std::fs::copy(&source, stage.path().join(&relative))
            .with_context(|| format!("Could not copy {}", source.display()))?;
        copies.insert(source, relative.clone());
        Ok(relative)
    };
    for item in &mut saved.playlist {
        if is_network(&item.path) {
            report.network_cues += 1;
        } else {
            item.path = copy_asset(&item.path)?;
        }
    }
    if let Some(background) = &mut saved.view.background {
        *background = copy_asset(background)?;
    }
    report.copied_files = copies.len();
    // A display identifier belongs to one installation. Bundles start with manual output placement.
    saved.output_display = None;
    crate::persistence::atomic_write(
        &stage.path().join(MANIFEST),
        &serde_json::to_vec_pretty(&saved)?,
    )?;
    std::fs::write(stage.path().join("BUNDLE.txt"), format!(
        "GenexDome show bundle\nOpen {MANIFEST} or choose Open bundle in Show.\nCopy this entire folder to move the show. Select the destination projector before opening output.\n{} local assets; {} network cues (require internet and the media runtime).\n",
        report.copied_files, report.network_cues
    ))?;
    // TempDir removes the staging directory on every failure before this commit.
    std::fs::rename(stage.path(), destination).context("Could not commit show bundle")?;
    Ok(report)
}

#[derive(Debug, Default, Serialize)]
pub struct RelinkReport {
    pub linked: usize,
    pub unresolved: Vec<PathBuf>,
}
/// Replace only missing assets with uniquely named files. Ambiguous matches remain explicit.
pub fn relink_folder(session: &mut Session, root: &Path) -> Result<RelinkReport> {
    let root = root.canonicalize()?;
    anyhow::ensure!(root.is_dir(), "Relink location is not a folder");
    let mut names = HashMap::<std::ffi::OsString, Vec<PathBuf>>::new();
    let mut pending = vec![root];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                names
                    .entry(entry.file_name())
                    .or_default()
                    .push(entry.path());
            }
        }
    }
    let mut report = RelinkReport::default();
    let paths = session
        .playlist
        .iter_mut()
        .map(|item| &mut item.path)
        .chain(session.view.background.iter_mut());
    for path in paths {
        if is_network(path) || path.is_file() {
            continue;
        }
        let candidates = path.file_name().and_then(|name| names.get(name));
        if let Some(files) = candidates.filter(|files| files.len() == 1) {
            *path = files[0].clone();
            report.linked += 1;
        } else {
            report.unresolved.push(path.clone());
        }
    }
    Ok(report)
}
#[derive(Debug, Default, Serialize)]
pub struct AssetReport {
    pub local_cues: usize,
    pub network_cues: usize,
    pub missing: Vec<PathBuf>,
}
pub fn check(session: &Session) -> AssetReport {
    let mut report = AssetReport::default();
    for item in &session.playlist {
        if is_network(&item.path) {
            report.network_cues += 1;
        } else {
            report.local_cues += 1;
            if !item.path.is_file() {
                report.missing.push(item.path.clone());
            }
        }
    }
    if let Some(path) = &session.view.background {
        if !path.is_file() {
            report.missing.push(path.clone());
        }
    }
    report
}
