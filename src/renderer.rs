use crate::{
    mesh::{Node, WarpMesh},
    state::{Output, Stereo, View},
};
use eframe::{
    egui,
    glow::{self, HasContext},
};
use image::RgbaImage;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    mesh_vao: glow::VertexArray,
    mesh_vbo: glow::Buffer,
    mesh_vertices: Vec<Node>,
    mesh_dirty: bool,
    uniforms: HashMap<&'static str, Option<glow::UniformLocation>>,
    texture: glow::Texture,
    background: Option<glow::Texture>,
    pub source_size: [u32; 2],
    mesh: Option<WarpMesh>,
}
impl Renderer {
    pub fn new(gl: &glow::Context) -> anyhow::Result<Self> {
        unsafe {
            let program = gl.create_program().map_err(anyhow::Error::msg)?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, include_str!("../assets/dome.vert")),
                (glow::FRAGMENT_SHADER, include_str!("../assets/dome.frag")),
            ] {
                let shader = gl.create_shader(kind).map_err(anyhow::Error::msg)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    anyhow::bail!("Dome shader: {}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            anyhow::ensure!(
                gl.get_program_link_status(program),
                "Dome shader link: {}",
                gl.get_program_info_log(program)
            );
            let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let vbo = gl.create_buffer().map_err(anyhow::Error::msg)?;
            let mesh_vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let mesh_vbo = gl.create_buffer().map_err(anyhow::Error::msg)?;
            for (array, buffer) in [(vao, vbo), (mesh_vao, mesh_vbo)] {
                gl.bind_vertex_array(Some(array));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
                for (location, size, offset) in [(0, 2, 0), (1, 2, 8), (2, 1, 16)] {
                    gl.enable_vertex_attrib_array(location);
                    gl.vertex_attrib_pointer_f32(location, size, glow::FLOAT, false, 20, offset);
                }
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&WarpMesh::identity(2, 2, 1.).triangles()),
                glow::STATIC_DRAW,
            );
            let uniforms = [
                "source_texture",
                "background_texture",
                "background_enabled",
                "projection",
                "output_mode",
                "stereo_input",
                "blackout",
                "flip_x",
                "flip_y",
                "mesh_aspect",
                "mesh_projection",
                "fisheye_fov",
                "source_fov",
                "perspective_fov",
                "brightness",
                "gamma_value",
                "angles",
                "offset",
                "source_aspect",
                "output_aspect",
                "eye",
            ]
            .into_iter()
            .map(|name| (name, gl.get_uniform_location(program, name)))
            .collect();
            let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            Ok(Self {
                program,
                vao,
                vbo,
                mesh_vao,
                mesh_vbo,
                mesh_vertices: Vec::new(),
                mesh_dirty: true,
                uniforms,
                texture,
                background: None,
                source_size: [0, 0],
                mesh: None,
            })
        }
    }
    pub fn upload(&mut self, gl: &glow::Context, img: &RgbaImage) {
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            if self.source_size == [img.width(), img.height()] {
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    img.width() as i32,
                    img.height() as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(img.as_raw())),
                );
            } else {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    img.width() as i32,
                    img.height() as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(img.as_raw())),
                );
            }
            self.source_size = [img.width(), img.height()];
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }
    pub fn set_mesh(&mut self, mesh: Option<WarpMesh>) {
        self.mesh_vertices = mesh.as_ref().map_or_else(Vec::new, WarpMesh::triangles);
        self.mesh_dirty = true;
        self.mesh = mesh;
    }
    pub fn upload_background(
        &mut self,
        gl: &glow::Context,
        image: Option<&RgbaImage>,
    ) -> anyhow::Result<()> {
        unsafe {
            if let Some(old) = self.background.take() {
                gl.delete_texture(old);
            }
            if let Some(img) = image {
                let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                    gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::LINEAR as i32);
                }
                for parameter in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                    gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::CLAMP_TO_EDGE as i32);
                }
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    img.width() as i32,
                    img.height() as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(img.as_raw())),
                );
                self.background = Some(texture);
                gl.bind_texture(glow::TEXTURE_2D, None);
                gl.active_texture(glow::TEXTURE0);
            }
        }
        Ok(())
    }
    pub fn check_gpu(&mut self, gl: &glow::Context, path: &std::path::Path) -> anyhow::Result<()> {
        unsafe {
            let mut source = crate::projection::pattern(crate::projection::Pattern::Panorama, 128);
            self.upload(gl, &source);
            let (w, h) = (96, 64);
            let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                w,
                h,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            let fbo = gl.create_framebuffer().map_err(anyhow::Error::msg)?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
            anyhow::ensure!(
                gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE,
                "GPU check framebuffer incomplete"
            );
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::FRAMEBUFFER_SRGB);
            let mesh = WarpMesh::identity(5, 4, w as f32 / h as f32);
            let backdrop_path = path.with_extension("backdrop.png");
            let backdrop = crate::projection::pattern(crate::projection::Pattern::ColourBars, 64);
            backdrop.save(&backdrop_path)?;
            self.upload_background(gl, Some(&backdrop))?;
            let mut cases = Vec::new();
            for input in crate::state::Projection::ALL {
                for output in Output::ALL {
                    cases.push((
                        format!("{input:?} → {output:?}"),
                        View {
                            input,
                            output,
                            yaw: 23.,
                            pitch: -12.,
                            roll: 7.,
                            offset: [0.1, -0.15],
                            gamma: 1.2,
                            brightness: 0.8,
                            source_fov: if input == crate::state::Projection::Perspective {
                                90.
                            } else {
                                180.
                            },
                            ..View::default()
                        },
                        mesh.clone(),
                    ));
                }
            }
            for projection in [
                crate::state::Projection::Perspective,
                crate::state::Projection::Fisheye,
                crate::state::Projection::Cylindrical,
                crate::state::Projection::Equirectangular,
                crate::state::Projection::Cubemap,
            ] {
                let mut m = mesh.clone();
                m.projection = projection;
                cases.push((
                    format!("mesh header {projection:?}"),
                    View {
                        output: Output::Mirror,
                        source_fov: 90.,
                        yaw: 23.,
                        ..View::default()
                    },
                    m,
                ));
            }
            for input in [Stereo::Mono, Stereo::SideBySide, Stereo::TopBottom] {
                for output in [Stereo::Mono, Stereo::SideBySide, Stereo::TopBottom] {
                    cases.push((
                        format!("stereo {input:?} → {output:?}"),
                        View {
                            stereo_input: input,
                            stereo_output: output,
                            flip_x: true,
                            flip_y: true,
                            yaw: -35.,
                            gamma: 0.8,
                            ..View::default()
                        },
                        mesh.clone(),
                    ));
                }
            }
            cases.push((
                "standard media + backdrop".into(),
                View {
                    input: crate::state::Projection::Perspective,
                    source_fov: 60.,
                    yaw: 10.,
                    background: Some(backdrop_path.clone()),
                    ..View::default()
                },
                mesh.clone(),
            ));
            let mut masked = mesh.clone();
            masked.nodes[6].intensity = -1.;
            masked.nodes[12].intensity = 0.2;
            cases.push((
                "mesh mask and intensity".into(),
                View {
                    output: Output::Mirror,
                    ..View::default()
                },
                masked,
            ));
            cases.push((
                "prewarped without mesh + reused texture".into(),
                View {
                    input: crate::state::Projection::Prewarped,
                    output: Output::Mirror,
                    ..View::default()
                },
                mesh.clone(),
            ));
            let case_count = cases.len();
            let mut report = String::new();
            let mut failure = None;
            for (label, view, mesh) in cases {
                let no_mesh = label == "prewarped without mesh + reused texture";
                let mesh = if no_mesh {
                    source = image::RgbaImage::from_pixel(
                        source.width(),
                        source.height(),
                        image::Rgba([200, 30, 80, 255]),
                    );
                    self.upload(gl, &source);
                    None
                } else {
                    Some(mesh)
                };
                self.set_mesh(mesh.clone());
                gl.clear_color(0., 0., 0., 1.);
                gl.clear(glow::COLOR_BUFFER_BIT);
                let rect =
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32));
                self.paint(
                    gl,
                    egui::PaintCallbackInfo {
                        viewport: rect,
                        clip_rect: rect,
                        pixels_per_point: 1.,
                        screen_size_px: [w as u32, h as u32],
                    },
                    &view,
                );
                let mut bytes = vec![0; (w * h * 4) as usize];
                gl.read_pixels(
                    0,
                    0,
                    w,
                    h,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut bytes)),
                );
                let mut gpu = image::RgbaImage::from_raw(w as u32, h as u32, bytes).unwrap();
                image::imageops::flip_vertical_in_place(&mut gpu);
                let cpu = crate::projection::render_cpu(
                    &source,
                    &view,
                    mesh.as_ref(),
                    w as u32,
                    h as u32,
                )?;
                let differences: Vec<_> = cpu
                    .as_raw()
                    .iter()
                    .zip(gpu.as_raw())
                    .map(|(a, b)| a.abs_diff(*b) as u64)
                    .collect();
                let mean = differences.iter().sum::<u64>() as f64 / differences.len() as f64;
                report.push_str(&format!("{label}: mean channel difference {mean:.4}/255\n"));
                if mean > 3. {
                    failure = Some(format!("GPU/CPU mismatch for {label}: {mean:.2}"));
                }
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.delete_framebuffer(fbo);
            gl.delete_texture(texture);
            self.set_mesh(None);
            self.upload_background(gl, None)?;
            std::fs::write(path, &report)?;
            if let Some(error) = failure {
                anyhow::bail!(error);
            }
            eprintln!("GPU projection checks passed ({case_count} cases)");
            Ok(())
        }
    }
    pub fn paint(&mut self, gl: &glow::Context, info: egui::PaintCallbackInfo, view: &View) {
        unsafe {
            let viewport = info.viewport_in_pixels();
            let count = if view.stereo_output == Stereo::Mono {
                1
            } else {
                2
            };
            let use_mesh =
                view.output == Output::Mirror && view.input != crate::state::Projection::Prewarped;
            let vertex_count = if use_mesh {
                self.mesh_vertices.len()
            } else {
                6
            };
            if self.mesh_dirty {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.mesh_vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&self.mesh_vertices),
                    glow::STATIC_DRAW,
                );
                self.mesh_dirty = false;
            }
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(if use_mesh { self.mesh_vao } else { self.vao }));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, self.background.or(Some(self.texture)));
            gl.active_texture(glow::TEXTURE0);
            let f = |name: &str, value: f32| {
                gl.uniform_1_f32(self.uniforms.get(name).and_then(Option::as_ref), value)
            };
            let i = |name: &str, value: i32| {
                gl.uniform_1_i32(self.uniforms.get(name).and_then(Option::as_ref), value)
            };
            i("source_texture", 0);
            i("background_texture", 1);
            i(
                "background_enabled",
                (self.background.is_some() && view.background.is_some()) as i32,
            );
            i("projection", view.input.code());
            i("output_mode", view.output.code());
            i("stereo_input", view.stereo_input.code());
            i("blackout", view.blackout as i32);
            i("flip_x", view.flip_x as i32);
            i("flip_y", view.flip_y as i32);
            f(
                "mesh_aspect",
                if view.output == Output::Mirror
                    && view.input != crate::state::Projection::Prewarped
                {
                    self.mesh.as_ref().map_or(1., |m| m.aspect)
                } else {
                    1.
                },
            );
            i(
                "mesh_projection",
                self.mesh.as_ref().map_or(2, |m| m.projection.code()),
            );
            f("fisheye_fov", view.fisheye_fov.to_radians());
            f("source_fov", view.source_fov.to_radians());
            f("perspective_fov", view.perspective_fov.to_radians());
            f("brightness", view.brightness);
            f("gamma_value", view.gamma);
            gl.uniform_3_f32(
                self.uniforms.get("angles").and_then(Option::as_ref),
                view.yaw.to_radians(),
                view.pitch.to_radians(),
                view.roll.to_radians(),
            );
            gl.uniform_2_f32(
                self.uniforms.get("offset").and_then(Option::as_ref),
                view.offset[0],
                view.offset[1],
            );
            let source_aspect = self.source_size[0] as f32 / self.source_size[1] as f32
                * match view.stereo_input {
                    Stereo::Mono => 1.,
                    Stereo::SideBySide => 0.5,
                    Stereo::TopBottom => 2.,
                };
            f("source_aspect", source_aspect);
            for eye in 0..count {
                let (x, y, w, h) = match view.stereo_output {
                    Stereo::Mono => (
                        viewport.left_px,
                        viewport.from_bottom_px,
                        viewport.width_px,
                        viewport.height_px,
                    ),
                    Stereo::SideBySide => (
                        viewport.left_px + eye * viewport.width_px / 2,
                        viewport.from_bottom_px,
                        viewport.width_px / 2,
                        viewport.height_px,
                    ),
                    Stereo::TopBottom => (
                        viewport.left_px,
                        viewport.from_bottom_px + (1 - eye) * viewport.height_px / 2,
                        viewport.width_px,
                        viewport.height_px / 2,
                    ),
                };
                gl.viewport(x, y, w, h);
                f("output_aspect", w as f32 / h as f32);
                i("eye", eye);
                gl.draw_arrays(glow::TRIANGLES, 0, vertex_count as i32);
            }
            gl.viewport(
                viewport.left_px,
                viewport.from_bottom_px,
                viewport.width_px,
                viewport.height_px,
            );
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_vertex_array(self.mesh_vao);
            gl.delete_buffer(self.mesh_vbo);
            gl.delete_texture(self.texture);
            if let Some(texture) = self.background {
                gl.delete_texture(texture);
            }
        }
    }
}
pub fn canvas(
    ui: &mut egui::Ui,
    renderer: Arc<Mutex<Renderer>>,
    view: View,
    size: egui::Vec2,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());
    ui.painter().rect_filled(rect, 0., egui::Color32::BLACK);
    ui.painter().add(egui::PaintCallback {
        rect,
        callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
            if let Ok(mut r) = renderer.lock() {
                r.paint(painter.gl(), info, &view);
            }
        })),
    });
    response
}
