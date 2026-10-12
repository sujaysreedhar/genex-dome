use crate::state::Projection;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub intensity: f32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarpMesh {
    pub projection: Projection,
    pub width: usize,
    pub height: usize,
    pub aspect: f32,
    pub nodes: Vec<Node>,
}
impl WarpMesh {
    pub fn identity(width: usize, height: usize, aspect: f32) -> Self {
        let mut nodes = Vec::new();
        for j in 0..height {
            for i in 0..width {
                let u = i as f32 / (width - 1) as f32;
                let v = j as f32 / (height - 1) as f32;
                nodes.push(Node {
                    x: (u * 2. - 1.) * aspect,
                    y: v * 2. - 1.,
                    u,
                    v,
                    intensity: 1.,
                });
            }
        }
        Self {
            projection: Projection::Fisheye,
            width,
            height,
            aspect,
            nodes,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (2..=512).contains(&self.width) && (2..=512).contains(&self.height),
            "Mesh dimensions must be 2–512 nodes per axis"
        );
        ensure!(
            self.nodes.len() == self.width * self.height,
            "Mesh has {} nodes; expected {}",
            self.nodes.len(),
            self.width * self.height
        );
        ensure!(
            self.aspect.is_finite() && self.aspect > 0.,
            "Invalid mesh aspect ratio"
        );
        for (i, n) in self.nodes.iter().enumerate() {
            ensure!(
                [n.x, n.y, n.u, n.v, n.intensity]
                    .iter()
                    .all(|v| v.is_finite()),
                "Mesh node {i} is not finite"
            );
            ensure!(
                (-1. ..=1.).contains(&n.intensity),
                "Node {i}: intensity must be between -1 and 1"
            );
        }
        Ok(())
    }
    pub fn parse(text: &str) -> Result<Self> {
        let tokens: Vec<_> = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .flat_map(str::split_whitespace)
            .collect();
        ensure!(tokens.len() >= 3, "Mesh header is missing");
        let projection =
            Projection::from_code(tokens[0].parse().context("Invalid projection code")?)?;
        let width: usize = tokens[1].parse().context("Invalid mesh width")?;
        let height: usize = tokens[2].parse().context("Invalid mesh height")?;
        ensure!(
            (2..=512).contains(&width) && (2..=512).contains(&height),
            "Mesh dimensions must be 2–512"
        );
        ensure!(
            tokens.len() == 3 + width * height * 5,
            "Incorrect node count: expected {} values, found {}",
            width * height * 5,
            tokens.len() - 3
        );
        let mut nodes = Vec::with_capacity(width * height);
        for chunk in tokens[3..].as_chunks::<5>().0 {
            let values: Vec<f32> = chunk
                .iter()
                .map(|s| s.parse::<f32>().context("Invalid node value"))
                .collect::<Result<_>>()?;
            nodes.push(Node {
                x: values[0],
                y: values[1],
                u: values[2],
                v: values[3],
                intensity: values[4],
            });
        }
        let aspect = nodes.iter().map(|n| n.x.abs()).fold(0., f32::max);
        let mesh = Self {
            projection,
            width,
            height,
            aspect,
            nodes,
        };
        mesh.validate()?;
        Ok(mesh)
    }
    pub fn load(path: &std::path::Path) -> Result<Self> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "{}\n{} {}\n",
            self.projection.code(),
            self.width,
            self.height
        );
        for n in &self.nodes {
            text.push_str(&format!(
                "{:.7} {:.7} {:.7} {:.7} {:.7}\n",
                n.x, n.y, n.u, n.v, n.intensity
            ));
        }
        text
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        self.validate()?;
        crate::persistence::atomic_write(path, self.to_text().as_bytes())?;
        Ok(())
    }
    pub fn triangles(&self) -> Vec<Node> {
        let mut result = Vec::with_capacity((self.width - 1) * (self.height - 1) * 6);
        for j in 0..self.height - 1 {
            for i in 0..self.width - 1 {
                let ids = [
                    j * self.width + i,
                    j * self.width + i + 1,
                    (j + 1) * self.width + i + 1,
                    (j + 1) * self.width + i,
                ];
                // A negative intensity excludes the whole quad, as in Bourke's format.
                if ids.iter().any(|&k| self.nodes[k].intensity < 0.) {
                    continue;
                }
                for k in [0, 1, 2, 0, 2, 3] {
                    result.push(self.nodes[ids[k]]);
                }
            }
        }
        result
    }
    pub fn sample(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        for j in 0..self.height - 1 {
            for i in 0..self.width - 1 {
                let ids = [
                    j * self.width + i,
                    j * self.width + i + 1,
                    (j + 1) * self.width + i + 1,
                    (j + 1) * self.width + i,
                ];
                if ids.iter().any(|&k| self.nodes[k].intensity < 0.) {
                    continue;
                }
                for tri in [[ids[0], ids[1], ids[2]], [ids[0], ids[2], ids[3]]] {
                    let [a, b, c] = tri.map(|k| self.nodes[k]);
                    let det = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
                    if det.abs() < 1e-12 {
                        continue;
                    }
                    let wa = ((b.y - c.y) * (x - c.x) + (c.x - b.x) * (y - c.y)) / det;
                    let wb = ((c.y - a.y) * (x - c.x) + (a.x - c.x) * (y - c.y)) / det;
                    let wc = 1. - wa - wb;
                    if wa >= -1e-5 && wb >= -1e-5 && wc >= -1e-5 {
                        return Some([
                            wa * a.u + wb * b.u + wc * c.u,
                            wa * a.v + wb * b.v + wc * c.v,
                            wa * a.intensity + wb * b.intensity + wc * c.intensity,
                        ]);
                    }
                }
            }
        }
        None
    }
    pub fn require_fisheye(&self) -> Result<()> {
        if self.projection != Projection::Fisheye {
            bail!("This output mesh must use fisheye coordinates (header 2). Convert source projections with the input selector first.");
        }
        Ok(())
    }
}
