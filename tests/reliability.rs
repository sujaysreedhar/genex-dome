use genex_dom_player::{
    calibration::MeshHistory,
    media::MediaInfo,
    mesh::WarpMesh,
    persistence::backup_path,
    playback::Playback,
    projection,
    state::{Output, Projection, Session, View},
};
use image::{Rgba, RgbaImage};
use std::time::{Duration, Instant};

#[test]
fn switching_to_builtin_clears_video_and_queued_autoplay() {
    let mut playback = Playback {
        info: Some(MediaInfo {
            target: "previous.mp4".into(),
            width: 32,
            height: 32,
            duration: 60.,
            frame_rate: 30.,
            is_video: true,
            has_audio: false,
        }),
        selected: Some(2),
        playing: true,
        position: 23.,
        scrub: Some(25.),
        startup_autoplay: true,
        title: "previous.mp4".into(),
        ..Playback::default()
    };
    playback.reset_source("Dome message".into());
    assert!(playback.info.is_none());
    assert!(playback.selected.is_none());
    assert_eq!(playback.position, 0.);
    assert!(playback.scrub.is_none());
    assert!(!playback.playing);
    assert!(!playback.startup_autoplay);
    assert_eq!(playback.title, "Dome message");
    playback.play(0.).unwrap();
    assert!(
        playback.decoder.is_none(),
        "A built-in source must never restart the old video"
    );
}

#[test]
fn replacement_load_accepts_only_the_current_source() {
    let dir = tempfile::tempdir().unwrap();
    let red = dir.path().join("red.png");
    let blue = dir.path().join("blue.png");
    RgbaImage::from_pixel(16, 16, Rgba([255, 0, 0, 255]))
        .save(&red)
        .unwrap();
    RgbaImage::from_pixel(16, 16, Rgba([0, 0, 255, 255]))
        .save(&blue)
        .unwrap();
    let mut playback = Playback::default();
    playback.load(red.to_string_lossy().into(), 0.);
    playback.reset_source("blue".into());
    playback.load(blue.to_string_lossy().into(), 0.);
    let deadline = Instant::now() + Duration::from_secs(5);
    let image = loop {
        if let Some(result) = playback.poll_load() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(image.get_pixel(8, 8), &Rgba([0, 0, 255, 255]));
    assert_eq!(
        playback.info.as_ref().unwrap().target,
        blue.to_string_lossy()
    );
    playback.load(red.to_string_lossy().into(), 0.);
    playback.cancel_load();
    assert!(playback.poll_load().is_none());
    assert!(playback.pending.is_none());
}

#[test]
fn empty_or_isolated_warp_nodes_do_not_replace_calibration() {
    let original = WarpMesh::identity(3, 3, 1.);
    let mut current = Some(original.clone());
    let mut history = MeshHistory::new(&current);
    let mut invalid = original.clone();
    for node in &mut invalid.nodes {
        node.intensity = -1.;
    }
    assert!(history.apply(&mut current, invalid.clone()).is_err());
    invalid.nodes[4].intensity = 1.; // Nonzero coverage still has no drawable quad.
    assert!(history.apply(&mut current, invalid).is_err());
    assert_eq!(current, Some(original));
    assert!(!history.can_undo());
    assert!(!history.modified(&current));
}

#[test]
fn calibration_undo_redo_groups_drag_and_tracks_saved_state() {
    let original = Some(WarpMesh::identity(3, 3, 1.));
    let mut current = original.clone();
    let mut history = MeshHistory::new(&current);
    history.begin_edit(&current);
    current.as_mut().unwrap().nodes[4].u = 0.6;
    history.begin_edit(&current); // Subsequent frame of the same gesture.
    current.as_mut().unwrap().nodes[4].u = 0.7;
    history.end_edit(&current);
    let changed = current.clone();
    assert!(history.modified(&current));
    history.undo(&mut current);
    assert_eq!(current, original);
    assert!(!history.can_undo());
    assert!(!history.modified(&current));
    history.redo(&mut current);
    assert_eq!(current, changed);
    history.mark_saved(&current);
    assert!(!history.modified(&current));
    history.undo(&mut current);
    assert!(history.modified(&current));
    history
        .apply(&mut current, WarpMesh::identity(4, 4, 1.5))
        .unwrap();
    assert!(!history.can_redo());
}

#[test]
fn session_replacement_keeps_previous_valid_backup_and_rejects_bad_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("show.json");
    let mut session = Session::default();
    session.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    session.text = "Updated show".into();
    session.save(&path).unwrap();
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), original);
    assert_eq!(Session::load(&path).unwrap().text, "Updated show");
    assert_eq!(
        Session::load(&backup_path(&path)).unwrap().text,
        "Welcome to the dome"
    );
    let good = std::fs::read(&path).unwrap();
    session.volume = 2.;
    assert!(session.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), good);
    assert_eq!(std::fs::read(backup_path(&path)).unwrap(), original);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        2,
        "No staging files should remain"
    );
}

#[test]
fn failed_backup_preserves_destination() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("show.json");
    let mut session = Session::default();
    session.save(&path).unwrap();
    let good = std::fs::read(&path).unwrap();
    std::fs::create_dir(backup_path(&path)).unwrap();
    session.text = "Cannot commit this version".into();
    assert!(session.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), good);
}

#[test]
fn prewarped_export_bypasses_missing_and_invalid_meshes_in_every_output_mode() {
    let image = RgbaImage::from_fn(32, 16, |x, y| Rgba([x as u8 * 7, y as u8 * 12, 90, 255]));
    let mut view = View {
        input: Projection::Prewarped,
        output: Output::Source,
        ..View::default()
    };
    let expected = projection::render_cpu(&image, &view, None, 32, 16).unwrap();
    let mut invalid = WarpMesh::identity(2, 2, 1.);
    invalid.nodes[0].u = f32::NAN;
    for output in Output::ALL {
        view.output = output;
        assert_eq!(
            projection::render_cpu(&image, &view, None, 32, 16).unwrap(),
            expected
        );
        assert_eq!(
            projection::render_cpu(&image, &view, Some(&invalid), 32, 16).unwrap(),
            expected
        );
    }
}

#[test]
fn display_origin_handles_negative_positions_and_mixed_dpi() {
    let display = genex_dom_player::displays::Display {
        id: "test".into(),
        position: [-1920, -1080],
        size: [1920, 1080],
        scale: 1.5,
        primary: false,
    };
    for scale in [1., 1.25, 1.5, 2.] {
        let origin = display.window_origin(scale);
        assert!((origin[0] * scale + 1888.).abs() < 0.001);
        assert!((origin[1] * scale + 1048.).abs() < 0.001);
    }
}
