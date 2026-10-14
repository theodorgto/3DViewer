use crate::model::{Model, Vertex};
use eframe::glow::{self, HasContext};
use glam::{Mat4, Vec3};

#[derive(Clone, Copy, PartialEq)]
pub enum DisplayMode {
    Solid,
    Wireframe,
    Points,
}

#[derive(Clone)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub orthographic: bool,
    pub z_up: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -0.9,
            pitch: 0.5,
            distance: 5.0,
            target: Vec3::ZERO,
            orthographic: false,
            z_up: true,
        }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        let direction = Vec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        );
        self.target
            + if self.z_up {
                direction
            } else {
                Vec3::new(direction.x, direction.z, -direction.y)
            } * self.distance
    }

    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let up = if self.z_up { Vec3::Z } else { Vec3::Y };
        let view = Mat4::look_at_rh(self.eye(), self.target, up);
        let half_height = self.distance * (45_f32.to_radians() * 0.5).tan();
        let projection = if self.orthographic {
            Mat4::orthographic_rh_gl(
                -half_height * aspect,
                half_height * aspect,
                -half_height,
                half_height,
                0.001,
                1000.0,
            )
        } else {
            Mat4::perspective_rh_gl(
                45_f32.to_radians(),
                aspect,
                (self.distance * 0.001).max(0.0001),
                1000.0,
            )
        };
        projection * view
    }

    pub fn orbit(&mut self, delta: [f32; 2]) {
        self.yaw -= delta[0] * 0.008;
        self.pitch = (self.pitch + delta[1] * 0.008).clamp(-1.55, 1.55);
    }

    pub fn pan(&mut self, delta: [f32; 2], height: f32) {
        let forward = (self.target - self.eye()).normalize();
        let right = forward
            .cross(if self.z_up { Vec3::Z } else { Vec3::Y })
            .normalize();
        let up = right.cross(forward);
        let scale = self.distance * 0.828 / height.max(1.0);
        self.target += (-right * delta[0] + up * delta[1]) * scale;
    }

    pub fn fit(&mut self, aspect: f32) {
        self.target = Vec3::ZERO;
        self.distance = 5.0 / aspect.clamp(0.2, 1.0);
    }
}

pub struct RenderSettings {
    pub mode: DisplayMode,
    pub grid: bool,
    pub colors: bool,
    pub flat: bool,
    pub point_size: f32,
    pub background: [f32; 3],
    pub tint: [f32; 3],
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            mode: DisplayMode::Solid,
            grid: true,
            colors: true,
            flat: true,
            point_size: 3.0,
            background: [0.12, 0.13, 0.13],
            tint: [0.48, 0.76, 0.69],
        }
    }
}

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vertices: glow::Buffer,
    indices: glow::Buffer,
    grid_vao: glow::VertexArray,
    grid_buffer: glow::Buffer,
    vertex_count: i32,
    index_count: i32,
    grid_count: i32,
}

impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let program = gl.create_program()?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX_SHADER),
                (glow::FRAGMENT_SHADER, FRAGMENT_SHADER),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(error);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(error);
            }
            let vao = gl.create_vertex_array()?;
            let vertices = gl.create_buffer()?;
            let indices = gl.create_buffer()?;
            Self::configure_vao(gl, vao, vertices);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(indices));
            let grid_vao = gl.create_vertex_array()?;
            let grid_buffer = gl.create_buffer()?;
            Self::configure_vao(gl, grid_vao, grid_buffer);
            gl.bind_vertex_array(None);
            Ok(Self {
                program,
                vao,
                vertices,
                indices,
                grid_vao,
                grid_buffer,
                vertex_count: 0,
                index_count: 0,
                grid_count: 0,
            })
        }
    }

    unsafe fn configure_vao(gl: &glow::Context, vao: glow::VertexArray, buffer: glow::Buffer) {
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
        for attribute in 0..3 {
            gl.enable_vertex_attrib_array(attribute);
            gl.vertex_attrib_pointer_f32(
                attribute,
                3,
                glow::FLOAT,
                false,
                std::mem::size_of::<Vertex>() as i32,
                attribute as i32 * 12,
            );
        }
    }

    pub fn upload(&mut self, gl: &glow::Context, model: &Model) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertices));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&model.vertices),
                glow::STATIC_DRAW,
            );
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.indices));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(&model.indices),
                glow::STATIC_DRAW,
            );
            gl.bind_vertex_array(None);
        }
        self.vertex_count = model.vertices.len() as i32;
        self.index_count = model.indices.len() as i32;
    }

    pub fn paint(
        &mut self,
        gl: &glow::Context,
        camera: &Camera,
        settings: &RenderSettings,
        aspect: f32,
        pixels_per_point: f32,
    ) {
        unsafe {
            gl.clear_color(
                settings.background[0],
                settings.background[1],
                settings.background[2],
                1.0,
            );
            gl.depth_mask(true);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "matrix").as_ref(),
                false,
                &camera.matrix(aspect).to_cols_array(),
            );
            gl.uniform_3_f32_slice(
                gl.get_uniform_location(self.program, "eye").as_ref(),
                &camera.eye().to_array(),
            );
            gl.uniform_3_f32_slice(
                gl.get_uniform_location(self.program, "tint").as_ref(),
                &settings.tint,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "point_size").as_ref(),
                settings.point_size * pixels_per_point,
            );
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "flat_shading")
                    .as_ref(),
                settings.flat as i32,
            );
            if settings.grid {
                let mut grid = Vec::new();
                let point = |first: f32, second: f32| {
                    if camera.z_up {
                        [first, second, -1.05]
                    } else {
                        [first, -1.05, second]
                    }
                };
                for index in -10..=10 {
                    let coordinate = index as f32 * 0.25;
                    let color = if index == 0 {
                        [0.4, 0.44, 0.43]
                    } else {
                        [0.24, 0.27, 0.26]
                    };
                    for position in [
                        point(coordinate, -2.5),
                        point(coordinate, 2.5),
                        point(-2.5, coordinate),
                        point(2.5, coordinate),
                    ] {
                        grid.push(Vertex {
                            position,
                            normal: [0.0, 0.0, 1.0],
                            color,
                        });
                    }
                }
                self.grid_count = grid.len() as i32;
                gl.bind_vertex_array(Some(self.grid_vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.grid_buffer));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&grid),
                    glow::DYNAMIC_DRAW,
                );
                gl.uniform_1_i32(gl.get_uniform_location(self.program, "lit").as_ref(), 0);
                gl.uniform_1_i32(
                    gl.get_uniform_location(self.program, "vertex_colors")
                        .as_ref(),
                    1,
                );
                gl.draw_arrays(glow::LINES, 0, self.grid_count);
            }
            gl.bind_vertex_array(Some(self.vao));
            let points = settings.mode == DisplayMode::Points || self.index_count == 0;
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "lit").as_ref(),
                (settings.mode == DisplayMode::Solid && !points) as i32,
            );
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "vertex_colors")
                    .as_ref(),
                settings.colors as i32,
            );
            if points {
                gl.enable(glow::PROGRAM_POINT_SIZE);
                gl.draw_arrays(glow::POINTS, 0, self.vertex_count);
                gl.disable(glow::PROGRAM_POINT_SIZE);
            } else {
                gl.polygon_mode(
                    glow::FRONT_AND_BACK,
                    if settings.mode == DisplayMode::Wireframe {
                        glow::LINE
                    } else {
                        glow::FILL
                    },
                );
                gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_vertex_array(self.grid_vao);
            gl.delete_buffer(self.vertices);
            gl.delete_buffer(self.indices);
            gl.delete_buffer(self.grid_buffer);
        }
    }
}

const VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 color;
uniform mat4 matrix;
uniform float point_size;
out vec3 world_position;
out vec3 world_normal;
out vec3 vertex_color;
void main() {
    world_position = position;
    world_normal = normal;
    vertex_color = color;
    gl_Position = matrix * vec4(position, 1.0);
    gl_PointSize = point_size;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 world_position;
in vec3 world_normal;
in vec3 vertex_color;
uniform vec3 eye;
uniform vec3 tint;
uniform bool lit;
uniform bool vertex_colors;
uniform bool flat_shading;
out vec4 fragment;
void main() {
    vec3 base = vertex_colors ? vertex_color : tint;
    if (lit) {
        vec3 normal = flat_shading ? normalize(cross(dFdx(world_position), dFdy(world_position))) : normalize(world_normal);
        vec3 view_direction = normalize(eye - world_position);
        if (dot(normal, view_direction) < 0.0) normal = -normal;
        vec3 light = normalize(vec3(0.4, -0.5, 0.8));
        float diffuse = 0.32 + 0.68 * max(dot(normal, light), 0.0);
        float specular = pow(max(dot(normal, normalize(light + view_direction)), 0.0), 48.0) * 0.22;
        base = base * diffuse + vec3(specular);
    }
    fragment = vec4(base, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_stays_finite_at_limits() {
        let mut camera = Camera::default();
        camera.orbit([10000.0, 10000.0]);
        camera.pan([200.0, -200.0], 600.0);
        assert!(camera.matrix(1.5).is_finite());
        camera.z_up = false;
        camera.orthographic = true;
        camera.fit(0.4);
        assert!(camera.matrix(0.4).is_finite());
    }
}
