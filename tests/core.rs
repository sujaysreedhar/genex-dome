use genex_dom_player::{
    mesh::{Node, WarpMesh},
    optics::{reflect, sphere_hit, Optics},
    projection::{self, Pattern},
    state::{Output, Projection, Session, Stereo, View},
};
use image::{Rgba, RgbaImage};
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn mesh_round_trip_and_orientation() {
    let m = WarpMesh::identity(7, 5, 16. / 9.);
    let parsed = WarpMesh::parse(&m.to_text()).unwrap();
    assert_eq!(parsed.nodes.len(), 35);
    let uv = parsed.sample(0., 0.).unwrap();
    near(uv[0], 0.5);
    near(uv[1], 0.5);
    let uv = parsed.sample(-m.aspect, -1.).unwrap();
    near(uv[0], 0.);
    near(uv[1], 0.);
}
#[test]
fn malformed_mesh_is_rejected() {
    for text in [
        "",
        "2\n1 2\n",
        "2\n18446744073709551615 9\n",
        "2\n2 2\n0 0 NaN 0 1",
        "99\n2 2\n",
    ] {
        assert!(WarpMesh::parse(text).is_err(), "accepted {text}");
    }
    let mut m = WarpMesh::identity(2, 2, 1.);
    m.nodes[0].u = f32::NAN;
    assert!(WarpMesh::parse(&m.to_text()).is_err());
}
#[test]
fn comments_in_mesh_are_supported() {
    let text = format!(
        "# calibration\n{}\n# end",
        WarpMesh::identity(2, 2, 1.).to_text()
    );
    assert_eq!(WarpMesh::parse(&text).unwrap().nodes.len(), 4);
}
#[test]
fn negative_intensity_excludes_entire_quad() {
    let mut m = WarpMesh::identity(3, 2, 1.);
    m.nodes[0].intensity = -1.;
    assert_eq!(m.triangles().len(), 6);
    assert!(m.sample(-0.5, 0.).is_none());
    assert!(m.sample(0.5, 0.).is_some());
}
#[test]
fn distorted_triangle_interpolation_is_continuous() {
    let mut m = WarpMesh::identity(2, 2, 1.);
    m.nodes[3] = Node {
        x: 0.7,
        y: 0.8,
        u: 1.,
        v: 1.,
        intensity: 0.5,
    };
    let p = m.sample(0., 0.).unwrap();
    assert!(p[0] > 0.4 && p[0] < 0.7);
    assert!(p[2] >= 0.5 && p[2] <= 1.);
}
#[test]
fn equatorial_cardinal_points_map_correctly() {
    for (dir, u) in [
        ([0., 0., 1.], 0.5),
        ([1., 0., 0.], 0.75),
        ([-1., 0., 0.], 0.25),
    ] {
        let uv = projection::source_uv(dir, Projection::Equirectangular, 180., 2.).unwrap();
        near(uv[0], u);
        near(uv[1], 0.5);
    }
    let uv = projection::source_uv([0., 1., 0.], Projection::Equirectangular, 180., 2.).unwrap();
    near(uv[1], 0.);
}
#[test]
fn fisheye_pole_rim_and_mask() {
    assert_eq!(
        projection::fisheye_direction([0.5, 0.5], 180.).unwrap(),
        [0., 0., 1.]
    );
    let d = projection::fisheye_direction([1., 0.5], 180.).unwrap();
    near(d[0], 1.);
    near(d[2], 0.);
    assert!(projection::fisheye_direction([0., 0.], 180.).is_none());
}
#[test]
fn rotations_preserve_norm_and_yaw() {
    let d = projection::rotate([0., 0., 1.], 90., 0., 0.);
    near(d[0], 1.);
    near(d[2], 0.);
    let d = projection::rotate([0., 0., 1.], 37., 24., 11.);
    near(d.iter().map(|x| x * x).sum::<f32>(), 1.);
}
#[test]
fn cubemap_atlas_faces_are_in_documented_order() {
    for (d, uv) in [
        ([1., 0., 0.], [1. / 6., 0.25]),
        ([-1., 0., 0.], [0.5, 0.25]),
        ([0., 1., 0.], [5. / 6., 0.25]),
        ([0., -1., 0.], [1. / 6., 0.75]),
        ([0., 0., 1.], [0.5, 0.75]),
        ([0., 0., -1.], [5. / 6., 0.75]),
    ] {
        let p = projection::source_uv(d, Projection::Cubemap, 180., 1.5).unwrap();
        near(p[0], uv[0]);
        near(p[1], uv[1]);
    }
}
#[test]
fn perspective_rejects_behind_camera() {
    assert!(projection::source_uv([0., 0., -1.], Projection::Perspective, 90., 1.).is_none());
    let p = projection::source_uv([0., 0., 1.], Projection::Perspective, 90., 1.).unwrap();
    near(p[0], 0.5);
    near(p[1], 0.5);
}
#[test]
fn sphere_intersection_and_reflection() {
    let p = sphere_hit([0., 0., -3.], [0., 0., 1.], [0., 0., 0.], 1.).unwrap();
    near(p[2], -1.);
    assert!(sphere_hit([3., 0., -3.], [0., 0., 1.], [0., 0., 0.], 1.).is_none());
    let d = reflect([0., -1., 0.], [0., 1., 0.]);
    assert_eq!(d, [0., 1., 0.]);
}
#[test]
fn inside_sphere_selects_forward_exit() {
    let p = sphere_hit([0., 0., 0.], [0., 0., 1.], [0., 0., 0.], 2.).unwrap();
    near(p[2], 2.);
}
#[test]
fn generated_optical_mesh_has_valid_coverage() {
    let m = Optics::default().generate().unwrap();
    m.validate().unwrap();
    let count = m.nodes.iter().filter(|n| n.intensity >= 0.).count();
    assert!(count > 20, "Only {count} optical rays reach the front dome");
    assert!(count < m.nodes.len());
    assert!(m
        .nodes
        .iter()
        .filter(|n| n.intensity >= 0.)
        .all(|n| (0. ..=1.).contains(&n.u) && (0. ..=1.).contains(&n.v)));
}
#[test]
fn invalid_optical_geometry_is_rejected() {
    let mut o = Optics {
        mirror_radius: -1.,
        ..Optics::default()
    };
    assert!(o.generate().is_err());
    o = Optics::default();
    o.projector = o.mirror_center;
    assert!(o.generate().is_err());
}
#[test]
fn source_export_preserves_pixels() {
    let img = RgbaImage::from_fn(32, 24, |x, y| Rgba([x as u8 * 7, y as u8 * 9, 120, 255]));
    let v = View {
        output: Output::Source,
        ..View::default()
    };
    let out = projection::render_cpu(&img, &v, None, 32, 24).unwrap();
    for (a, b) in img.pixels().zip(out.pixels()) {
        for k in 0..3 {
            assert!(a[k].abs_diff(b[k]) <= 1);
        }
    }
}
#[test]
fn fisheye_export_masks_corners_and_letterboxes() {
    let source = RgbaImage::from_pixel(64, 32, Rgba([200, 100, 50, 255]));
    let out = projection::render_cpu(&source, &View::default(), None, 96, 64).unwrap();
    assert_eq!(out.get_pixel(0, 0), &Rgba([0, 0, 0, 255]));
    assert_eq!(out.get_pixel(47, 31)[0], 200);
    assert_eq!(out.get_pixel(5, 31)[0], 0);
}
#[test]
fn mirror_identity_matches_square_fisheye() {
    let source = projection::pattern(Pattern::Panorama, 32);
    let a = projection::render_cpu(&source, &View::default(), None, 32, 32).unwrap();
    let v = View {
        output: Output::Mirror,
        ..View::default()
    };
    let m = WarpMesh::identity(3, 3, 1.);
    let b = projection::render_cpu(&source, &v, Some(&m), 32, 32).unwrap();
    for (a, b) in a.as_raw().iter().zip(b.as_raw()) {
        assert!(a.abs_diff(*b) <= 1);
    }
}
#[test]
fn stereo_renders_distinct_eyes() {
    let source = RgbaImage::from_fn(32, 16, |x, _| {
        if x < 16 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    });
    let view = View {
        output: Output::Source,
        stereo_input: Stereo::SideBySide,
        stereo_output: Stereo::SideBySide,
        ..View::default()
    };
    let out = projection::render_cpu(&source, &view, None, 32, 16).unwrap();
    assert_eq!(out.get_pixel(8, 8)[0], 255);
    assert_eq!(out.get_pixel(24, 8)[2], 255);
}
#[test]
fn blackout_and_missing_mesh_are_handled() {
    let source = RgbaImage::from_pixel(16, 16, Rgba([255, 255, 255, 255]));
    let mut v = View {
        blackout: true,
        ..View::default()
    };
    assert!(projection::render_cpu(&source, &v, None, 16, 16)
        .unwrap()
        .pixels()
        .all(|p| p[0] == 0));
    v.blackout = false;
    v.output = Output::Mirror;
    assert!(projection::render_cpu(&source, &v, None, 16, 16).is_err());
}
#[test]
fn gamma_and_brightness_apply_to_intensity() {
    let source = RgbaImage::from_pixel(16, 16, Rgba([128, 128, 128, 255]));
    let v = View {
        output: Output::Source,
        brightness: 0.5,
        gamma: 2.,
        ..View::default()
    };
    let out = projection::render_cpu(&source, &v, None, 16, 16).unwrap();
    assert!((out.get_pixel(8, 8)[0] as i32 - 128).abs() <= 1);
}
#[test]
fn offaxis_mapping_stays_on_unit_sphere() {
    let d = projection::apply_offset([0., 0., 1.], [0.2, -0.3]);
    near(d.iter().map(|x| x * x).sum(), 1.);
    near(d[0], 0.2);
    near(d[1], -0.3);
}
#[test]
fn session_roundtrip_validates_and_rejects_future_version() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("genexdomplayer-test-{}.json", std::process::id()));
    let s = Session::default();
    s.save(&path).unwrap();
    let loaded = Session::load(&path).unwrap();
    assert_eq!(loaded.view.input, s.view.input);
    let mut bad = serde_json::to_value(s).unwrap();
    bad["version"] = 2.into();
    std::fs::write(&path, serde_json::to_string(&bad).unwrap()).unwrap();
    assert!(Session::load(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn unicode_text_message_rasterizes() {
    let a = projection::text_image("Hello dome\nBienvenue · 360°", 256);
    assert!(
        a.pixels()
            .filter(|p| p[0] > 100 && p[1] > 100 && p[2] > 100)
            .count()
            > 100
    );
}

#[test]
fn session_keeps_network_urls_and_local_paths_resolvable() {
    let path = std::env::temp_dir().join(format!("genexdomplayer-url-{}.json", std::process::id()));
    let s = Session {
        playlist: vec![
            genex_dom_player::state::PlaylistItem {
                path: "https://example.com/movie.mp4".into(),
                projection: Projection::Equirectangular,
            },
            genex_dom_player::state::PlaylistItem {
                path: "local-movie.mp4".into(),
                projection: Projection::Fisheye,
            },
        ],
        ..Session::default()
    };
    s.save(&path).unwrap();
    let loaded = Session::load(&path).unwrap();
    assert_eq!(
        loaded.playlist[0].path.to_string_lossy(),
        "https://example.com/movie.mp4"
    );
    assert!(loaded.playlist[1].path.is_absolute());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn general_mesh_coordinates_invert_each_source_projection() {
    for projection in [
        Projection::Equirectangular,
        Projection::Cylindrical,
        Projection::Perspective,
        Projection::Fisheye,
        Projection::Cubemap,
    ] {
        for direction in [[0., 0., 1.], [0.3, 0.2, 1.], [-0.4, 0.1, 1.]] {
            let d = genex_dom_player::optics::normalize(direction);
            let uv = projection::source_uv(d, projection, 90., 1.5).unwrap();
            let actual = projection::direction_from_uv(uv, projection, 90., 1.5).unwrap();
            for k in 0..3 {
                near(actual[k], d[k]);
            }
        }
    }
}

#[test]
fn standard_media_uses_a_fisheye_backdrop_outside_its_rectangle() {
    let path = std::env::temp_dir().join(format!(
        "genexdomplayer-backdrop-{}.png",
        std::process::id()
    ));
    RgbaImage::from_pixel(32, 32, Rgba([0, 0, 255, 255]))
        .save(&path)
        .unwrap();
    let source = RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 255]));
    let view = View {
        input: Projection::Perspective,
        source_fov: 60.,
        background: Some(path.clone()),
        ..View::default()
    };
    let result = projection::render_cpu(&source, &view, None, 64, 64).unwrap();
    assert_eq!(result.get_pixel(32, 32)[0], 255);
    assert_eq!(result.get_pixel(3, 32)[2], 255);
    assert_eq!(result.get_pixel(0, 0)[2], 0);
    std::fs::remove_file(path).unwrap();
}
