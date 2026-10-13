use genex_dom_player::{
    bundle, media,
    playback::Preloader,
    show::{next_index, Sequencer},
    state::{CueView, Output, PlaylistItem, Projection, Session, View},
};
use image::{Rgba, RgbaImage};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn image(path: &Path, color: [u8; 4]) {
    RgbaImage::from_pixel(16, 16, Rgba(color))
        .save(path)
        .unwrap();
}
fn wait_preload(preload: &mut Preloader) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while preload.busy() {
        preload.poll();
        assert!(Instant::now() < deadline, "Preload did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn legacy_sessions_and_playlist_items_keep_default_cue_behavior() {
    let item: PlaylistItem =
        serde_json::from_str(r#"{"path":"legacy.png","projection":"Fisheye"}"#).unwrap();
    assert!(item.cue.view.is_none());
    assert!(item.cue.duration_seconds.is_none());
    assert_eq!(item.cue.fade_seconds, 0.);
    Session::load(Path::new("assets/calibration.genexdomplayer.json")).unwrap();
    Session::load(Path::new("assets/message.genexdomplayer.json")).unwrap();
}

#[test]
fn cue_preset_roundtrip_changes_source_view_and_preserves_installation_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let mut item = PlaylistItem::new("movie.mp4".into(), Projection::Fisheye);
    item.cue.name = "Opening sky".into();
    item.cue.duration_seconds = Some(12.5);
    item.cue.fade_seconds = 2.;
    item.cue.view = Some(CueView::capture(&View {
        yaw: 45.,
        pitch: 15.,
        roll: -10.,
        source_fov: 210.,
        ..Default::default()
    }));
    session.playlist.push(item);
    let path = dir.path().join("show.json");
    session.save(&path).unwrap();
    let saved = Session::load(&path).unwrap();
    assert_eq!(saved.playlist[0].label(), "Opening sky");
    assert_eq!(saved.playlist[0].cue.duration_seconds, Some(12.5));
    let mut output = View {
        output: Output::Mirror,
        brightness: 0.7,
        gamma: 1.5,
        offset: [0.2, 0.1],
        blackout: true,
        ..Default::default()
    };
    saved.playlist[0]
        .cue
        .view
        .as_ref()
        .unwrap()
        .apply(&mut output);
    assert_eq!(output.yaw, 45.);
    assert_eq!(output.source_fov, 210.);
    assert_eq!(output.output, Output::Mirror);
    assert_eq!(output.gamma, 1.5);
    assert_eq!(output.brightness, 0.7);
    assert_eq!(output.offset, [0.2, 0.1]);
    assert!(output.blackout);
}

#[test]
fn invalid_cues_fail_before_replacing_a_saved_show() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("show.json");
    let mut session = Session {
        playlist: vec![PlaylistItem::new("movie.mp4".into(), Projection::Fisheye)],
        ..Default::default()
    };
    session.save(&path).unwrap();
    let good = std::fs::read(&path).unwrap();
    for duration in [f64::NAN, -1., 0., 86401.] {
        session.playlist[0].cue.duration_seconds = Some(duration);
        assert!(session.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), good);
    }
    session.playlist[0].cue.duration_seconds = None;
    session.playlist[0].cue.fade_seconds = 11.;
    assert!(session.save(&path).is_err());
    session.playlist[0].cue.fade_seconds = 0.;
    session.playlist[0].cue.view = Some(CueView::capture(&View {
        yaw: f32::NAN,
        ..Default::default()
    }));
    assert!(session.save(&path).is_err());
    let mut bad: serde_json::Value = serde_json::from_slice(&good).unwrap();
    bad["playlist"][0]["cue"]["fade_seconds"] = (-1).into();
    std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(Session::load(&path).is_err());
}

#[test]
fn fades_pause_switch_once_hold_black_for_load_and_resume_incoming() {
    let mut show = Sequencer::default();
    show.tick(3., true);
    show.tick(50., false);
    assert_eq!(show.elapsed, 3.);
    assert_eq!(show.request(2, 2.), None);
    assert_eq!(show.tick(0.5, true), None);
    assert!((show.gain() - 0.5).abs() < 0.001);
    show.tick(10., false);
    assert_eq!(show.gain(), 0.5);
    assert_eq!(show.tick(0.5, true), Some(2));
    assert!(show.loading());
    assert_eq!(show.gain(), 0.);
    assert_eq!(show.tick(100., true), None);
    assert_eq!(show.gain(), 0.);
    show.loaded();
    assert_eq!(show.elapsed, 0.);
    show.tick(0.25, true);
    assert_eq!(show.gain(), 0.25);
    show.tick(0.75, true);
    assert!(!show.transitioning());
    assert_eq!(show.gain(), 1.);
    assert_eq!(show.elapsed, 1.);
    assert_eq!(show.request(0, 0.), Some(0));
    show.reset();
    assert_eq!(show.elapsed, 0.);
    assert_eq!(show.gain(), 1.);
}

#[test]
fn show_end_stops_unless_loop_is_enabled() {
    assert_eq!(next_index(0, 3, false), Some(1));
    assert_eq!(next_index(2, 3, false), None);
    assert_eq!(next_index(2, 3, true), Some(0));
    assert_eq!(next_index(0, 1, true), Some(0));
    assert_eq!(next_index(0, 1, false), None);
    assert_eq!(next_index(0, 0, true), None);
    assert_eq!(next_index(5, 3, true), None);
}

#[test]
fn preloader_reuses_one_matching_result_and_invalidates_source_and_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let red = dir.path().join("red.png");
    let blue = dir.path().join("blue.png");
    image(&red, [255, 0, 0, 255]);
    image(&blue, [0, 0, 255, 255]);
    let red = red.to_string_lossy().into_owned();
    let blue = blue.to_string_lossy().into_owned();
    let mut preload = Preloader::default();
    preload.ensure(Some(red.clone()), 1080);
    wait_preload(&mut preload);
    preload.ensure(Some(red.clone()), 1080);
    assert!(!preload.busy());
    assert!(preload.take(&red, 2160).is_none());
    assert!(preload.take(&blue, 1080).is_none());
    let (_, frame) = preload.take(&red, 1080).unwrap().unwrap();
    assert_eq!(frame.get_pixel(0, 0)[0], 255);
    assert!(preload.take(&red, 1080).is_none());
    preload.ensure(Some(red), 1080);
    preload.ensure(Some(blue.clone()), 2160);
    wait_preload(&mut preload);
    assert_eq!(
        preload
            .take(&blue, 2160)
            .unwrap()
            .unwrap()
            .1
            .get_pixel(0, 0)[2],
        255
    );
    preload.ensure(Some("missing.png".into()), 1080);
    wait_preload(&mut preload);
    assert!(preload.take("missing.png", 1080).unwrap().is_err());
    preload.ensure(Some(blue), 1080);
    preload.clear();
    assert!(!preload.busy());
}

#[test]
fn portable_bundle_relocates_deduplicates_and_handles_same_named_media() {
    let dir = tempfile::tempdir().unwrap();
    let sources = dir.path().join("original");
    std::fs::create_dir_all(sources.join("a")).unwrap();
    std::fs::create_dir_all(sources.join("b")).unwrap();
    let red = sources.join("a/sky.png");
    let blue = sources.join("b/sky.png");
    image(&red, [255, 0, 0, 255]);
    image(&blue, [0, 0, 255, 255]);
    let mut session = Session {
        playlist: vec![
            PlaylistItem::new(red.clone(), Projection::Fisheye),
            PlaylistItem::new(blue, Projection::Perspective),
            PlaylistItem::new(red.clone(), Projection::Fisheye),
            PlaylistItem::new("https://example.com/movie.mp4".into(), Projection::Fisheye),
        ],
        output_display: Some("original-monitor".into()),
        ..Default::default()
    };
    session.view.background = Some(red);
    session.playlist[0].cue.name = "Opening".into();
    session.playlist[0].cue.fade_seconds = 1.;
    let destination = dir.path().join("portable");
    let report = bundle::export(&session, &destination).unwrap();
    assert_eq!(report.copied_files, 2);
    assert_eq!(report.network_cues, 1);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(destination.join(bundle::MANIFEST)).unwrap())
            .unwrap();
    let relative = manifest["playlist"][0]["path"].as_str().unwrap();
    assert!(relative.starts_with("media/"));
    assert!(!relative.contains('\\'));
    let moved = dir.path().join("relocated");
    std::fs::rename(&destination, &moved).unwrap();
    std::fs::rename(&sources, dir.path().join("unavailable")).unwrap();
    let opened = Session::load(&moved.join(bundle::MANIFEST)).unwrap();
    assert!(bundle::check(&opened).missing.is_empty());
    assert_eq!(opened.playlist[0].path, opened.playlist[2].path);
    assert_eq!(
        opened.view.background.as_ref().unwrap(),
        &opened.playlist[0].path
    );
    assert_ne!(opened.playlist[0].path, opened.playlist[1].path);
    assert_eq!(opened.playlist[0].label(), "Opening");
    assert_eq!(opened.playlist[0].cue.fade_seconds, 1.);
    assert!(opened.output_display.is_none());
    assert!(session.output_display.is_some());
    assert_eq!(
        image::open(&opened.playlist[0].path)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)[0],
        255
    );
    assert_eq!(
        image::open(&opened.playlist[1].path)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)[2],
        255
    );
}

#[test]
fn failed_bundle_export_cleans_staging_and_never_overwrites_existing_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let session = Session {
        playlist: vec![PlaylistItem::new(
            dir.path().join("missing.mp4"),
            Projection::Fisheye,
        )],
        ..Default::default()
    };
    let destination = dir.path().join("show");
    assert!(bundle::export(&session, &destination).is_err());
    assert!(!destination.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("keep.txt"), "unchanged").unwrap();
    assert!(bundle::export(&Session::default(), &destination).is_err());
    assert_eq!(
        std::fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "unchanged"
    );
}

#[test]
fn relinking_changes_only_missing_unique_matches_and_preserves_presets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    image(&dir.path().join("unique.png"), [255, 0, 0, 255]);
    image(&dir.path().join("duplicate.png"), [255, 0, 0, 255]);
    image(&dir.path().join("nested/duplicate.png"), [0, 0, 255, 255]);
    let existing = dir.path().join("unique.png");
    let mut session = Session {
        playlist: vec![
            PlaylistItem::new("missing/unique.png".into(), Projection::Fisheye),
            PlaylistItem::new("missing/duplicate.png".into(), Projection::Fisheye),
            PlaylistItem::new(existing.clone(), Projection::Fisheye),
            PlaylistItem::new("HTTPS://example.com/live.mp4".into(), Projection::Fisheye),
        ],
        ..Default::default()
    };
    session.playlist[0].cue.fade_seconds = 2.;
    session.view.background = Some("missing/unique.png".into());
    let report = bundle::relink_folder(&mut session, dir.path()).unwrap();
    assert_eq!(report.linked, 2);
    assert_eq!(report.unresolved.len(), 1);
    assert!(session.playlist[0].path.is_file());
    assert_eq!(session.playlist[0].cue.fade_seconds, 2.);
    assert_eq!(session.playlist[1].path, Path::new("missing/duplicate.png"));
    assert_eq!(session.playlist[2].path, existing);
    assert!(bundle::is_network(&session.playlist[3].path));
}

#[test]
fn frame_rate_parsing_preserves_common_rates_and_rejects_invalid_values() {
    assert!((media::parse_frame_rate("30000/1001") - 29.97002997).abs() < 0.0001);
    assert_eq!(media::parse_frame_rate("24/1"), 24.);
    assert_eq!(media::parse_frame_rate("60"), 60.);
    for value in ["0/0", "1/0", "NaN", "-24", "999", "", "wrong"] {
        assert_eq!(media::parse_frame_rate(value), 30.);
    }
}

#[test]
fn movie_cue_limits_use_media_time_and_images_use_pause_aware_elapsed_time() {
    use genex_dom_player::{show::duration_reached, state::CueSettings};
    let mut cue = CueSettings::default();
    assert!(!duration_reached(&cue, true, 100., 100., 8.)); // Full movies end via decoder EOF.
    assert!(!duration_reached(&cue, false, 100., 7., 8.));
    assert!(duration_reached(&cue, false, 0., 8., 8.));
    cue.duration_seconds = Some(2.);
    assert!(!duration_reached(&cue, true, 1., 50., 8.));
    assert!(duration_reached(&cue, true, 2., 0., 8.));
    assert!(!duration_reached(&cue, false, 100., 1., 8.));
    assert!(duration_reached(&cue, false, 0., 2., 8.));
}
