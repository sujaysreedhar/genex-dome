use crate::mesh::WarpMesh;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
pub type Vec3 = [f32; 3];
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn mul(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn normalize(a: Vec3) -> Vec3 {
    mul(a, 1. / dot(a, a).sqrt().max(1e-8))
}
pub fn sphere_hit(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<Vec3> {
    let oc = sub(origin, center);
    let b = dot(oc, direction);
    let c = dot(oc, oc) - radius * radius;
    let discr = b * b - c;
    if discr < 0. {
        return None;
    }
    let root = discr.sqrt();
    let near = -b - root;
    let far = -b + root;
    let t = if near > 1e-5 {
        near
    } else if far > 1e-5 {
        far
    } else {
        return None;
    };
    Some(add(origin, mul(direction, t)))
}
pub fn reflect(direction: Vec3, normal: Vec3) -> Vec3 {
    sub(direction, mul(normal, 2. * dot(direction, normal)))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Optics {
    pub dome_radius: f32,
    pub dome_center: Vec3,
    pub dome_tilt: f32,
    pub mirror_radius: f32,
    pub mirror_center: Vec3,
    pub projector: Vec3,
    pub projector_yaw: f32,
    pub projector_pitch: f32,
    pub throw_ratio: f32,
    pub lens_shift: f32,
    pub aspect: f32,
    pub grid_width: usize,
    pub grid_height: usize,
    pub brightness_correction: bool,
}
impl Default for Optics {
    fn default() -> Self {
        Self {
            dome_radius: 1.5,
            dome_center: [0., 0., 0.],
            dome_tilt: 0.,
            mirror_radius: 0.25,
            mirror_center: [0., -1.2, -0.35],
            projector: [0., -0.7, 0.9],
            projector_yaw: 0.,
            projector_pitch: 0.,
            throw_ratio: 1.8,
            lens_shift: 0.,
            aspect: 16. / 9.,
            grid_width: 65,
            grid_height: 41,
            brightness_correction: false,
        }
    }
}
impl Optics {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [
                self.dome_radius,
                self.dome_tilt,
                self.mirror_radius,
                self.projector_yaw,
                self.projector_pitch,
                self.throw_ratio,
                self.lens_shift,
                self.aspect
            ]
            .iter()
            .chain(self.dome_center.iter())
            .chain(self.mirror_center.iter())
            .chain(self.projector.iter())
            .all(|v| v.is_finite()),
            "Optical geometry contains non-finite values"
        );
        ensure!(
            self.dome_radius > 0.
                && self.mirror_radius > 0.
                && self.throw_ratio > 0.
                && self.aspect > 0.,
            "Radii, throw and aspect must be positive"
        );
        ensure!(
            (2..=512).contains(&self.grid_width) && (2..=512).contains(&self.grid_height),
            "Optical mesh dimensions must be 2–512"
        );
        ensure!(
            dot(
                sub(self.mirror_center, self.projector),
                sub(self.mirror_center, self.projector)
            ) > self.mirror_radius * self.mirror_radius,
            "Projector must be outside the mirror"
        );
        Ok(())
    }
    pub fn generate(&self) -> Result<WarpMesh> {
        self.validate()?;
        let mut forward = normalize(sub(self.mirror_center, self.projector));
        forward = crate::projection::rotate(forward, self.projector_yaw, self.projector_pitch, 0.);
        let right = normalize(cross(forward, [0., 1., 0.]));
        let up = normalize(cross(right, forward));
        let mut mesh = WarpMesh::identity(self.grid_width, self.grid_height, self.aspect);
        let mut hits = vec![None; mesh.nodes.len()];
        for (i, n) in mesh.nodes.iter_mut().enumerate() {
            let ray = normalize(add(
                forward,
                add(
                    mul(right, n.x / (2. * self.aspect * self.throw_ratio)),
                    mul(
                        up,
                        (n.y + self.lens_shift) / (2. * self.aspect * self.throw_ratio),
                    ),
                ),
            ));
            let hit = sphere_hit(self.projector, ray, self.mirror_center, self.mirror_radius);
            let dome_hit = hit.and_then(|h| {
                let reflected = reflect(ray, normalize(sub(h, self.mirror_center)));
                sphere_hit(
                    add(h, mul(reflected, 0.0001)),
                    reflected,
                    self.dome_center,
                    self.dome_radius,
                )
            });
            if let Some(h) = dome_hit {
                let dir = crate::projection::rotate(
                    normalize(sub(h, self.dome_center)),
                    0.,
                    -self.dome_tilt,
                    0.,
                );
                let theta = dir[2].clamp(-1., 1.).acos();
                if theta <= std::f32::consts::FRAC_PI_2 {
                    let radius = theta / std::f32::consts::FRAC_PI_2;
                    let planar = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
                    n.u = 0.5 + 0.5 * radius * dir[0] / planar.max(1e-8);
                    n.v = 0.5 + 0.5 * radius * dir[1] / planar.max(1e-8);
                    hits[i] = Some(h);
                    continue;
                }
            }
            n.intensity = -1.;
        }
        if self.brightness_correction {
            let mut area = vec![0.; hits.len()];
            for j in 0..self.grid_height - 1 {
                for i in 0..self.grid_width - 1 {
                    let k = j * self.grid_width + i;
                    if let (Some(a), Some(b), Some(c)) =
                        (hits[k], hits[k + 1], hits[k + self.grid_width])
                    {
                        let v = cross(sub(b, a), sub(c, a));
                        area[k] = dot(v, v).sqrt();
                    }
                }
            }
            let mut sorted: Vec<_> = area.iter().copied().filter(|a| *a > 0.).collect();
            sorted.sort_by(f32::total_cmp);
            if !sorted.is_empty() {
                let reference = sorted[sorted.len() * 9 / 10].max(1e-8);
                for (n, a) in mesh.nodes.iter_mut().zip(area) {
                    if n.intensity >= 0. && a > 0. {
                        n.intensity = (a / reference).clamp(0.05, 1.);
                    }
                }
            }
        }
        mesh.validate()?;
        Ok(mesh)
    }
}
