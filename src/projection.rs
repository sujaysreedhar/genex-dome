use crate::{
    mesh::WarpMesh,
    optics::{normalize, Vec3},
    state::{Output, Projection, Stereo, View},
};
use image::{Rgba, RgbaImage};
use std::f32::consts::PI;

pub fn rotate(mut d: Vec3, yaw: f32, pitch: f32, roll: f32) -> Vec3 {
    let (s, c) = roll.to_radians().sin_cos();
    d = [c * d[0] - s * d[1], s * d[0] + c * d[1], d[2]];
    let (s, c) = pitch.to_radians().sin_cos();
    d = [d[0], c * d[1] + s * d[2], -s * d[1] + c * d[2]];
    let (s, c) = yaw.to_radians().sin_cos();
    [c * d[0] + s * d[2], d[1], -s * d[0] + c * d[2]]
}
pub fn fisheye_direction(uv: [f32; 2], fov: f32) -> Option<Vec3> {
    let p = [2. * uv[0] - 1., 1. - 2. * uv[1]];
    let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
    if r > 1. {
        return None;
    }
    let theta = r * fov.to_radians() * 0.5;
    let s = theta.sin() / r.max(1e-8);
    Some([p[0] * s, p[1] * s, theta.cos()])
}
pub fn direction_from_uv(
    uv: [f32; 2],
    projection: Projection,
    fov: f32,
    aspect: f32,
) -> Option<Vec3> {
    Some(match projection {
        Projection::Fisheye => return fisheye_direction(uv, fov),
        Projection::Equirectangular => {
            let lon = (uv[0] - 0.5) * 2. * PI;
            let lat = (0.5 - uv[1]) * PI;
            [lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()]
        }
        Projection::Cylindrical => {
            let lon = (uv[0] - 0.5) * 2. * PI;
            normalize([
                lon.sin(),
                (1. - 2. * uv[1]) * (fov.to_radians() * 0.25).tan(),
                lon.cos(),
            ])
        }
        Projection::Perspective => {
            let t = (fov.to_radians() * 0.5).tan();
            normalize([(uv[0] * 2. - 1.) * t, (1. - uv[1] * 2.) * t / aspect, 1.])
        }
        Projection::Cubemap => {
            let x = (uv[0] * 3.).min(2.999999);
            let y = (uv[1] * 2.).min(1.999999);
            let face = x.floor() as i32 + y.floor() as i32 * 3;
            let s = x.fract() * 2. - 1.;
            let t = y.fract() * 2. - 1.;
            normalize(match face {
                0 => [1., -t, -s],
                1 => [-1., -t, s],
                2 => [s, 1., t],
                3 => [s, -1., -t],
                4 => [s, -t, 1.],
                _ => [-s, -t, -1.],
            })
        }
        Projection::Prewarped => return None,
    })
}
pub fn source_uv(d: Vec3, p: Projection, fov: f32, aspect: f32) -> Option<[f32; 2]> {
    let d = normalize(d);
    let uv = match p {
        Projection::Equirectangular => [
            0.5 + d[0].atan2(d[2]) / (2. * PI),
            d[1].clamp(-1., 1.).acos() / PI,
        ],
        Projection::Cylindrical => [
            0.5 + d[0].atan2(d[2]) / (2. * PI),
            0.5 - d[1]
                / (d[0] * d[0] + d[2] * d[2]).sqrt().max(1e-8)
                / (2. * (fov.to_radians() * 0.25).tan()),
        ],
        Projection::Fisheye => {
            let theta = d[2].clamp(-1., 1.).acos();
            let r = theta / (fov.to_radians() * 0.5);
            if r > 1. {
                return None;
            }
            let n = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-8);
            [0.5 + 0.5 * r * d[0] / n, 0.5 - 0.5 * r * d[1] / n]
        }
        Projection::Perspective => {
            if d[2] <= 0. {
                return None;
            }
            let t = (fov.to_radians() * 0.5).tan();
            [
                0.5 + 0.5 * d[0] / d[2] / t,
                0.5 - 0.5 * d[1] / d[2] / t * aspect,
            ]
        }
        Projection::Cubemap => {
            let a = d.map(f32::abs);
            let (face, s, t, m) = if a[0] >= a[1] && a[0] >= a[2] {
                if d[0] > 0. {
                    (0, -d[2], -d[1], a[0])
                } else {
                    (1, d[2], -d[1], a[0])
                }
            } else if a[1] >= a[2] {
                if d[1] > 0. {
                    (2, d[0], d[2], a[1])
                } else {
                    (3, d[0], -d[2], a[1])
                }
            } else if d[2] > 0. {
                (4, d[0], -d[1], a[2])
            } else {
                (5, -d[0], -d[1], a[2])
            };
            [
                ((face % 3) as f32 + 0.5 + 0.5 * s / m) / 3.,
                ((face / 3) as f32 + 0.5 + 0.5 * t / m) / 2.,
            ]
        }
        Projection::Prewarped => return None,
    };
    if uv[0] < 0. || uv[0] > 1. || uv[1] < 0. || uv[1] > 1. {
        None
    } else {
        Some(uv)
    }
}
pub fn apply_offset(d: Vec3, offset: [f32; 2]) -> Vec3 {
    // Ray from an off-centre lens intersects a unit spherical dome.
    let o = [offset[0], offset[1], 0.];
    let b = crate::optics::dot(o, d);
    let t = -b + (b * b + 1. - crate::optics::dot(o, o)).max(0.).sqrt();
    normalize(crate::optics::add(o, crate::optics::mul(d, t)))
}
fn bilinear(img: &RgbaImage, uv: [f32; 2], wrap: bool) -> [f32; 3] {
    let x = uv[0] * img.width() as f32 - 0.5;
    let y = uv[1] * img.height() as f32 - 0.5;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let mut result = [0.; 3];
    for (dx, dy, w) in [
        (0, 0, (1. - fx) * (1. - fy)),
        (1, 0, fx * (1. - fy)),
        (0, 1, (1. - fx) * fy),
        (1, 1, fx * fy),
    ] {
        let xx = if wrap {
            (ix + dx).rem_euclid(img.width() as i32)
        } else {
            (ix + dx).clamp(0, img.width() as i32 - 1)
        };
        let yy = (iy + dy).clamp(0, img.height() as i32 - 1);
        let p = img.get_pixel(xx as u32, yy as u32);
        for c in 0..3 {
            result[c] += p[c] as f32 / 255. * w;
        }
    }
    result
}
pub fn render_cpu(
    source: &RgbaImage,
    view: &View,
    mesh: Option<&WarpMesh>,
    width: u32,
    height: u32,
) -> anyhow::Result<RgbaImage> {
    view.validate()?;
    let background = view
        .background
        .as_ref()
        .map(|p| image::open(p).map(|img| img.to_rgba8()))
        .transpose()?;
    anyhow::ensure!(
        width > 0 && height > 0 && width <= 8192 && height <= 8192,
        "Export dimensions must be 1–8192"
    );
    let needs_mesh = view.output == Output::Mirror && view.input != Projection::Prewarped;
    if needs_mesh {
        let m = mesh.ok_or_else(|| anyhow::anyhow!("Load or generate a warp mesh first"))?;
        m.validate()?;
    }
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    if view.blackout {
        return Ok(img);
    }
    let (eye_width, eye_height, eyes) = match view.stereo_output {
        Stereo::Mono => (width, height, 1),
        Stereo::SideBySide => (width / 2, height, 2),
        Stereo::TopBottom => (width, height / 2, 2),
    };
    anyhow::ensure!(
        eye_width > 0 && eye_height > 0,
        "Stereo export is too small"
    );
    // Rasterise triangles once rather than searching the whole mesh per pixel.
    let warp = if needs_mesh {
        Some(rasterize_mesh(mesh.unwrap(), eye_width, eye_height))
    } else {
        None
    };
    for eye in 0..eyes {
        for y in 0..eye_height {
            for x in 0..eye_width {
                let mut uv = [
                    (x as f32 + 0.5) / eye_width as f32,
                    (y as f32 + 0.5) / eye_height as f32,
                ];
                let mut intensity = 1.;
                let mut backdrop_uv = None;
                let aspect = eye_width as f32 / eye_height as f32;
                let sample = if view.output == Output::Source || view.input == Projection::Prewarped
                {
                    Some(uv)
                } else {
                    if let Some(w) = &warp {
                        if let Some(node) = w[(y * eye_width + x) as usize] {
                            uv = [node[0], 1. - node[1]];
                            intensity = node[2];
                        } else {
                            continue;
                        }
                    } else if view.output == Output::Fisheye {
                        if aspect >= 1. {
                            uv[0] = (uv[0] - 0.5) * aspect + 0.5;
                        } else {
                            uv[1] = (uv[1] - 0.5) / aspect + 0.5;
                        }
                    }
                    let dir = if view.output == Output::Mirror {
                        let p = mesh.unwrap().projection;
                        direction_from_uv(
                            uv,
                            p,
                            if p == Projection::Fisheye {
                                view.fisheye_fov
                            } else {
                                view.source_fov
                            },
                            source_aspect(source, view.stereo_input),
                        )
                    } else if view.output == Output::Perspective {
                        let t = (view.perspective_fov.to_radians() * 0.5).tan();
                        Some(normalize([
                            (uv[0] * 2. - 1.) * t,
                            (1. - uv[1] * 2.) * t / aspect,
                            1.,
                        ]))
                    } else {
                        fisheye_direction(uv, view.fisheye_fov)
                    };
                    dir.and_then(|d| {
                        let rotated = rotate(
                            apply_offset(d, view.offset),
                            view.yaw,
                            view.pitch,
                            view.roll,
                        );
                        backdrop_uv = source_uv(rotated, Projection::Fisheye, 180., 1.);
                        source_uv(
                            rotated,
                            view.input,
                            view.source_fov,
                            source_aspect(source, view.stereo_input),
                        )
                    })
                };
                let rgb = if let Some(mut uv) = sample {
                    if view.flip_x {
                        uv[0] = 1. - uv[0];
                    }
                    if view.flip_y {
                        uv[1] = 1. - uv[1];
                    }
                    match view.stereo_input {
                        Stereo::Mono => {}
                        Stereo::SideBySide => uv[0] = (uv[0] + eye as f32) / 2.,
                        Stereo::TopBottom => uv[1] = (uv[1] + eye as f32) / 2.,
                    }
                    Some(bilinear(
                        source,
                        uv,
                        view.input == Projection::Equirectangular
                            || view.input == Projection::Cylindrical,
                    ))
                } else {
                    background
                        .as_ref()
                        .zip(backdrop_uv)
                        .map(|(bg, uv)| bilinear(bg, uv, false))
                };
                if let Some(rgb) = rgb {
                    let px = Rgba([
                        (rgb[0] * intensity * view.brightness)
                            .max(0.)
                            .powf(1. / view.gamma)
                            .clamp(0., 1.)
                            .mul_add(255., 0.) as u8,
                        (rgb[1] * intensity * view.brightness)
                            .max(0.)
                            .powf(1. / view.gamma)
                            .clamp(0., 1.)
                            .mul_add(255., 0.) as u8,
                        (rgb[2] * intensity * view.brightness)
                            .max(0.)
                            .powf(1. / view.gamma)
                            .clamp(0., 1.)
                            .mul_add(255., 0.) as u8,
                        255,
                    ]);
                    let xx = x + if view.stereo_output == Stereo::SideBySide {
                        eye * eye_width
                    } else {
                        0
                    };
                    let yy = y + if view.stereo_output == Stereo::TopBottom {
                        eye * eye_height
                    } else {
                        0
                    };
                    img.put_pixel(xx, yy, px);
                }
            }
        }
    }
    Ok(img)
}
pub fn source_aspect(img: &RgbaImage, s: Stereo) -> f32 {
    let a = img.width() as f32 / img.height() as f32;
    match s {
        Stereo::Mono => a,
        Stereo::SideBySide => a / 2.,
        Stereo::TopBottom => a * 2.,
    }
}
fn rasterize_mesh(m: &WarpMesh, w: u32, h: u32) -> Vec<Option<[f32; 3]>> {
    let mut map = vec![None; (w * h) as usize];
    let tris = m.triangles();
    for t in tris.as_chunks::<3>().0 {
        let p: Vec<_> = t
            .iter()
            .map(|n| {
                [
                    (n.x / m.aspect + 1.) * 0.5 * w as f32,
                    (1. - n.y) * 0.5 * h as f32,
                ]
            })
            .collect();
        let minx = p
            .iter()
            .map(|a| a[0])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let maxx = p
            .iter()
            .map(|a| a[0])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .clamp(0., w as f32) as u32;
        let miny = p
            .iter()
            .map(|a| a[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let maxy = p
            .iter()
            .map(|a| a[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .clamp(0., h as f32) as u32;
        let [a, b, c] = [p[0], p[1], p[2]];
        let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if det.abs() < 1e-8 {
            continue;
        }
        for y in miny..maxy {
            for x in minx..maxx {
                let xx = x as f32 + 0.5;
                let yy = y as f32 + 0.5;
                let wa = ((b[1] - c[1]) * (xx - c[0]) + (c[0] - b[0]) * (yy - c[1])) / det;
                let wb = ((c[1] - a[1]) * (xx - c[0]) + (a[0] - c[0]) * (yy - c[1])) / det;
                let wc = 1. - wa - wb;
                if wa >= -1e-5 && wb >= -1e-5 && wc >= -1e-5 {
                    map[(y * w + x) as usize] = Some([
                        wa * t[0].u + wb * t[1].u + wc * t[2].u,
                        wa * t[0].v + wb * t[1].v + wc * t[2].v,
                        wa * t[0].intensity + wb * t[1].intensity + wc * t[2].intensity,
                    ]);
                }
            }
        }
    }
    map
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pattern {
    Panorama,
    PolarGrid,
    CartesianGrid,
    ColourBars,
}
pub fn pattern(kind: Pattern, size: u32) -> RgbaImage {
    let width = if kind == Pattern::Panorama {
        size * 2
    } else {
        size
    };
    RgbaImage::from_fn(width, size, |x, y| {
        let u = x as f32 / width as f32;
        let v = y as f32 / size as f32;
        let color = match kind {
            Pattern::PolarGrid => {
                let p = [2. * u - 1., 2. * v - 1.];
                let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
                if r > 1. {
                    [0, 0, 0]
                } else {
                    let a = p[1].atan2(p[0]) / PI * 6.;
                    let ring = (r * 6.).fract();
                    let spoke = (a - a.round()).abs() * r;
                    let line = ring < 0.008 || ring > 0.992 || spoke < 0.008;
                    if r > 0.992 {
                        [245, 165, 64]
                    } else if line {
                        [205, 222, 235]
                    } else {
                        [12, 18, 24]
                    }
                }
            }
            Pattern::CartesianGrid => {
                if (x % (size / 16).max(1)) < 2 || (y % (size / 16).max(1)) < 2 {
                    [220, 235, 240]
                } else {
                    [13, 20, 28]
                }
            }
            Pattern::ColourBars => {
                let colors = [
                    [255, 255, 255],
                    [255, 255, 0],
                    [0, 255, 255],
                    [0, 255, 0],
                    [255, 0, 255],
                    [255, 0, 0],
                    [0, 0, 255],
                    [0, 0, 0],
                ];
                colors[((u * 8.) as usize).min(7)]
            }
            Pattern::Panorama => {
                let sky = (1. - v * 1.8).max(0.);
                let mountain = 0.54 + 0.045 * (u * 37.).sin() + 0.025 * (u * 83. + 1.).sin();
                let star = ((x.wrapping_mul(1973)
                    ^ y.wrapping_mul(9277)
                    ^ x.wrapping_mul(y).wrapping_mul(26699))
                    % 2201)
                    == 0;
                if v < mountain {
                    if star && v < 0.4 {
                        [220, 233, 244]
                    } else {
                        [
                            (12. + 22. * sky) as u8,
                            (24. + 45. * sky) as u8,
                            (38. + 66. * sky) as u8,
                        ]
                    }
                } else {
                    let grid = (u * 36.).fract() < 0.025 || (v * 20.).fract() < 0.04;
                    let horizon = v < mountain + 0.004;
                    if horizon {
                        [224, 161, 73]
                    } else if grid {
                        [37, 64, 72]
                    } else {
                        [10, 23, 28]
                    }
                }
            }
        };
        Rgba([color[0], color[1], color[2], 255])
    })
}
pub fn text_image(message: &str, size: u32) -> RgbaImage {
    text_image_on(message, size, None)
}
pub fn text_image_on(message: &str, size: u32, background: Option<RgbaImage>) -> RgbaImage {
    let mut img =
        background.unwrap_or_else(|| RgbaImage::from_pixel(size, size, Rgba([5, 12, 18, 255])));
    // OFL font bundled with the app; no platform font dependency.
    if let Ok(font) = fontdue::Font::from_bytes(
        include_bytes!("../assets/AtkinsonHyperlegible-Regular.ttf") as &[u8],
        fontdue::FontSettings::default(),
    ) {
        let lines: Vec<_> = message.lines().take(12).collect();
        let px = size as f32 / 24.;
        let height = lines.len() as f32 * px * 1.4;
        let mut yy = (size as f32 - height) * 0.5;
        for line in lines {
            let width: f32 = line
                .chars()
                .map(|c| font.metrics(c, px).advance_width)
                .sum();
            let mut xx = (size as f32 - width) * 0.5;
            for c in line.chars() {
                let (m, bitmap) = font.rasterize(c, px);
                for y in 0..m.height {
                    for x in 0..m.width {
                        let ix = xx as i32 + m.xmin + x as i32;
                        let iy = yy as i32 + (px as i32 - m.height as i32 - m.ymin) + y as i32;
                        if ix >= 0 && iy >= 0 && ix < size as i32 && iy < size as i32 {
                            let alpha = bitmap[y * m.width + x] as f32 / 255.;
                            let p = img.get_pixel_mut(ix as u32, iy as u32);
                            for k in 0..3 {
                                p[k] = (p[k] as f32 * (1. - alpha) + 245. * alpha) as u8;
                            }
                        }
                    }
                }
                xx += m.advance_width;
            }
            yy += px * 1.4;
        }
    }
    img
}
