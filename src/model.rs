use anyhow::{bail, ensure, Context, Result};
use glam::{DVec3, Mat4, Vec3};
use ply_rs::ply::{DefaultElement, Property};
use std::{fs::File, io::BufReader, path::Path};

pub const EXTENSIONS: &[&str] = &["step", "stp", "ply", "stl", "obj", "gltf", "glb"];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize, serde::Deserialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub dimensions: DVec3,
    pub name: String,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct RawMesh {
    positions: Vec<DVec3>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
    warnings: Vec<String>,
}

impl RawMesh {
    fn polygon(&mut self, face: &[u32]) -> Result<()> {
        ensure!(face.len() >= 3, "A face has fewer than three vertices");
        ensure!(
            face.iter()
                .all(|&index| (index as usize) < self.positions.len()),
            "Face index is out of bounds"
        );
        if face.len() == 3 {
            self.indices.extend_from_slice(face);
            return Ok(());
        }
        let mut normal = DVec3::ZERO;
        for offset in 0..face.len() {
            let current = self.positions[face[offset] as usize];
            let next = self.positions[face[(offset + 1) % face.len()] as usize];
            normal += current.cross(next);
        }
        let axis = normal.abs().max_element();
        let coordinates: Vec<f64> = face
            .iter()
            .flat_map(|&index| {
                let point = self.positions[index as usize];
                if normal.x.abs() == axis {
                    [point.y, point.z]
                } else if normal.y.abs() == axis {
                    [point.z, point.x]
                } else {
                    [point.x, point.y]
                }
            })
            .collect();
        let triangles =
            earcutr::earcut(&coordinates, &[], 2).context("Cannot triangulate polygon")?;
        ensure!(
            !triangles.is_empty(),
            "Cannot triangulate degenerate polygon"
        );
        self.indices
            .extend(triangles.into_iter().map(|index| face[index]));
        Ok(())
    }

    fn finish(self, name: String) -> Result<Model> {
        ensure!(!self.positions.is_empty(), "The file contains no vertices");
        ensure!(
            self.positions.len() <= i32::MAX as usize && self.indices.len() <= i32::MAX as usize,
            "Model exceeds renderer limits"
        );
        ensure!(
            self.positions.iter().all(|point| point.is_finite()),
            "Model contains non-finite coordinates"
        );
        ensure!(
            self.indices.len().is_multiple_of(3),
            "Incomplete triangle data"
        );
        ensure!(
            self.indices
                .iter()
                .all(|&index| (index as usize) < self.positions.len()),
            "Triangle index is out of bounds"
        );
        let minimum = self
            .positions
            .iter()
            .copied()
            .fold(DVec3::splat(f64::INFINITY), DVec3::min);
        let maximum = self
            .positions
            .iter()
            .copied()
            .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
        let dimensions = maximum - minimum;
        ensure!(
            dimensions.is_finite(),
            "Model coordinate range is too large"
        );
        let center = minimum + dimensions * 0.5;
        let scale = dimensions.max_element().max(1e-12) * 0.5;
        let positions: Vec<Vec3> = self
            .positions
            .iter()
            .map(|point| ((*point - center) / scale).as_vec3())
            .collect();
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for triangle in self.indices.as_chunks::<3>().0 {
            let first = triangle[0] as usize;
            let second = triangle[1] as usize;
            let third = triangle[2] as usize;
            let normal =
                (positions[second] - positions[first]).cross(positions[third] - positions[first]);
            normals[first] += normal;
            normals[second] += normal;
            normals[third] += normal;
        }
        let vertices = positions
            .iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                position: position.to_array(),
                normal: normals[index].try_normalize().unwrap_or(Vec3::Y).to_array(),
                color: self
                    .colors
                    .get(index)
                    .copied()
                    .unwrap_or([0.48, 0.76, 0.69]),
            })
            .collect();
        Ok(Model {
            vertices,
            indices: self.indices,
            dimensions,
            name,
            warnings: self.warnings,
        })
    }
}

pub fn load(path: &Path, step_tolerance: f64) -> Result<Model> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let raw = match extension.as_str() {
        "obj" => load_obj(path)?,
        "stl" => load_stl(path)?,
        "ply" => load_ply(BufReader::new(File::open(path)?))?,
        "gltf" | "glb" => load_gltf(path)?,
        "step" | "stp" => load_step(path, step_tolerance)?,
        _ => bail!(
            "Unsupported format '{extension}'. Supported: STEP, STP, PLY, STL, OBJ, glTF, GLB."
        ),
    };
    raw.finish(
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    )
}

fn load_obj(path: &Path) -> Result<RawMesh> {
    let options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (objects, _) = tobj::load_obj(path, &options)?;
    let mut raw = RawMesh::default();
    for object in objects {
        let offset = u32::try_from(raw.positions.len())?;
        raw.positions.extend(
            object
                .mesh
                .positions
                .as_chunks::<3>()
                .0
                .iter()
                .map(|point| DVec3::new(point[0] as f64, point[1] as f64, point[2] as f64)),
        );
        for index in object.mesh.indices {
            raw.indices
                .push(index.checked_add(offset).context("Too many OBJ vertices")?);
        }
    }
    Ok(raw)
}

fn load_stl(path: &Path) -> Result<RawMesh> {
    let mesh = stl_io::read_stl(&mut BufReader::new(File::open(path)?))?;
    let positions = mesh
        .vertices
        .iter()
        .map(|point| DVec3::new(point[0] as f64, point[1] as f64, point[2] as f64))
        .collect();
    let indices = mesh
        .faces
        .iter()
        .flat_map(|face| face.vertices)
        .map(u32::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(RawMesh {
        positions,
        indices,
        ..Default::default()
    })
}

fn scalar(property: &Property) -> Option<f64> {
    match property {
        Property::Char(value) => Some(*value as f64),
        Property::UChar(value) => Some(*value as f64),
        Property::Short(value) => Some(*value as f64),
        Property::UShort(value) => Some(*value as f64),
        Property::Int(value) => Some(*value as f64),
        Property::UInt(value) => Some(*value as f64),
        Property::Float(value) => Some(*value as f64),
        Property::Double(value) => Some(*value),
        _ => None,
    }
}

fn load_ply(mut reader: impl std::io::Read) -> Result<RawMesh> {
    let ply = ply_rs::parser::Parser::<DefaultElement>::new().read_ply(&mut reader)?;
    let vertices = ply
        .payload
        .get("vertex")
        .context("PLY has no vertex element")?;
    let mut raw = RawMesh::default();
    for vertex in vertices {
        let coordinate = |key: &str| {
            vertex
                .get(key)
                .and_then(scalar)
                .with_context(|| format!("Missing PLY coordinate {key}"))
        };
        raw.positions.push(DVec3::new(
            coordinate("x")?,
            coordinate("y")?,
            coordinate("z")?,
        ));
        let color = |key: &str| {
            vertex.get(key).and_then(|property| {
                scalar(property).map(|value| match property {
                    Property::Float(_) | Property::Double(_) => value as f32,
                    _ => (value / 255.0) as f32,
                })
            })
        };
        raw.colors
            .push(match (color("red"), color("green"), color("blue")) {
                (Some(red), Some(green), Some(blue)) => [
                    red.clamp(0.0, 1.0),
                    green.clamp(0.0, 1.0),
                    blue.clamp(0.0, 1.0),
                ],
                _ => [0.48, 0.76, 0.69],
            });
    }
    if let Some(faces) = ply.payload.get("face") {
        for face in faces {
            let property = face
                .get("vertex_indices")
                .or_else(|| face.get("vertex_index"))
                .context("PLY face has no vertex indices")?;
            let indices: Vec<u32> = match property {
                Property::ListInt(values) => values
                    .iter()
                    .map(|&value| u32::try_from(value))
                    .collect::<std::result::Result<_, _>>()?,
                Property::ListUInt(values) => values.clone(),
                Property::ListUChar(values) => values.iter().map(|&value| value as u32).collect(),
                Property::ListUShort(values) => values.iter().map(|&value| value as u32).collect(),
                Property::ListShort(values) => values
                    .iter()
                    .map(|&value| u32::try_from(value))
                    .collect::<std::result::Result<_, _>>()?,
                Property::ListChar(values) => values
                    .iter()
                    .map(|&value| u32::try_from(value))
                    .collect::<std::result::Result<_, _>>()?,
                _ => bail!("Unsupported PLY face index type"),
            };
            raw.polygon(&indices)?;
        }
    }
    Ok(raw)
}

fn load_gltf(path: &Path) -> Result<RawMesh> {
    let (document, buffers, _) = gltf::import(path)?;
    let mut raw = RawMesh::default();
    fn visit(
        node: gltf::Node<'_>,
        parent: Mat4,
        buffers: &[gltf::buffer::Data],
        raw: &mut RawMesh,
    ) -> Result<()> {
        let transform = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
                let positions: Vec<_> = reader
                    .read_positions()
                    .context("glTF primitive has no positions")?
                    .collect();
                let offset = u32::try_from(raw.positions.len())?;
                raw.positions.extend(positions.iter().map(|point| {
                    transform
                        .transform_point3(Vec3::from_array(*point))
                        .as_dvec3()
                }));
                let colors: Vec<_> = reader
                    .read_colors(0)
                    .map(|colors| colors.into_rgb_f32().collect())
                    .unwrap_or_else(|| {
                        let color = primitive
                            .material()
                            .pbr_metallic_roughness()
                            .base_color_factor();
                        vec![[color[0], color[1], color[2]]; positions.len()]
                    });
                raw.colors.extend(colors);
                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|indices| indices.into_u32().collect())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());
                ensure!(
                    indices
                        .iter()
                        .all(|&index| (index as usize) < positions.len()),
                    "glTF primitive index is out of bounds"
                );
                let mut triangle = |first: u32, second: u32, third: u32| -> Result<()> {
                    raw.indices.extend(
                        [first, second, third]
                            .into_iter()
                            .map(|index| {
                                index.checked_add(offset).context("Too many glTF vertices")
                            })
                            .collect::<Result<Vec<_>>>()?,
                    );
                    Ok(())
                };
                match primitive.mode() {
                    gltf::mesh::Mode::Triangles => {
                        ensure!(indices.len().is_multiple_of(3), "Incomplete glTF triangle");
                        for face in indices.as_chunks::<3>().0 {
                            triangle(face[0], face[1], face[2])?;
                        }
                    }
                    gltf::mesh::Mode::TriangleStrip => {
                        for index in 2..indices.len() {
                            if index % 2 == 0 {
                                triangle(indices[index - 2], indices[index - 1], indices[index])?;
                            } else {
                                triangle(indices[index - 1], indices[index - 2], indices[index])?;
                            }
                        }
                    }
                    gltf::mesh::Mode::TriangleFan => {
                        for index in 2..indices.len() {
                            triangle(indices[0], indices[index - 1], indices[index])?;
                        }
                    }
                    gltf::mesh::Mode::Points => {}
                    _ => bail!("glTF line primitives are not supported"),
                }
            }
        }
        for child in node.children() {
            visit(child, transform, buffers, raw)?;
        }
        Ok(())
    }
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .context("glTF has no scene")?;
    for node in scene.nodes() {
        visit(node, Mat4::IDENTITY, &buffers, &mut raw)?;
    }
    Ok(raw)
}

fn load_step(path: &Path, tolerance: f64) -> Result<RawMesh> {
    let trace = std::env::var_os("MODEL_VIEWER_IMPORT_TRACE").is_some();
    let started = std::time::Instant::now();
    ensure!(
        tolerance.is_finite() && tolerance > 0.0,
        "STEP tolerance must be positive"
    );
    if trace {
        eprintln!("STEP: reading with OpenCASCADE");
    }
    opencascade_sys::b_rep::reset_skipped_cad_faces();
    let shape = opencascade::primitives::Shape::read_step(path)
        .context("OpenCASCADE cannot read this STEP file")?;
    if trace {
        eprintln!(
            "STEP: transferred CAD shapes in {:.2}s",
            started.elapsed().as_secs_f64()
        );
    }
    let coarse = shape
        .mesh_with_tolerance(0.05)
        .context("Cannot tessellate STEP surfaces")?;
    ensure!(
        !coarse.vertices.is_empty(),
        "STEP contains no renderable surfaces"
    );
    let minimum = coarse
        .vertices
        .iter()
        .fold(DVec3::splat(f64::INFINITY), |bounds, point| {
            bounds.min(DVec3::new(point.x, point.y, point.z))
        });
    let maximum = coarse
        .vertices
        .iter()
        .fold(DVec3::splat(f64::NEG_INFINITY), |bounds, point| {
            bounds.max(DVec3::new(point.x, point.y, point.z))
        });
    let absolute_tolerance = ((maximum - minimum).length() * tolerance).max(1e-4);
    if trace {
        eprintln!("STEP: tessellating with deflection {absolute_tolerance}");
    }
    let mesh = if absolute_tolerance >= 0.05 {
        coarse
    } else {
        opencascade_sys::b_rep::reset_skipped_cad_faces();
        shape
            .mesh_with_tolerance(absolute_tolerance)
            .context("Cannot refine STEP tessellation")?
    };
    if trace {
        eprintln!(
            "STEP: tessellated in {:.2}s",
            started.elapsed().as_secs_f64()
        );
    }
    let raw = RawMesh {
        positions: mesh.vertices.iter().map(|point| DVec3::new(point.x, point.y, point.z)).collect(),
        indices: mesh.indices.into_iter().map(u32::try_from).collect::<std::result::Result<_, _>>()?,
        warnings: match opencascade_sys::b_rep::skipped_cad_faces() {
            0 => Vec::new(),
            count => vec![format!("Incomplete STEP model: {count} surface(s) could not be tessellated. Do not use this view for CAD verification or measurements.")],
        },
        ..Default::default()
    };
    ensure!(!raw.indices.is_empty(), "STEP produced no triangles");
    Ok(raw)
}

pub fn demo() -> Model {
    let positions = vec![
        DVec3::new(-1.0, -0.6, -0.7),
        DVec3::new(1.0, -0.6, -0.7),
        DVec3::new(1.0, 0.6, -0.7),
        DVec3::new(-1.0, 0.6, -0.7),
        DVec3::new(-1.0, -0.6, 0.7),
        DVec3::new(1.0, -0.6, 0.7),
        DVec3::new(1.0, 0.6, 0.7),
        DVec3::new(-1.0, 0.6, 0.7),
    ];
    RawMesh {
        positions,
        indices: vec![
            0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7,
            3, 1, 2, 6, 1, 6, 5,
        ],
        ..Default::default()
    }
    .finish("Sample block".into())
    .expect("valid built-in model")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_normalized() {
        let model = demo();
        assert_eq!(model.indices.len(), 36);
        assert!(model
            .vertices
            .iter()
            .all(|vertex| Vec3::from_array(vertex.position).abs().max_element() <= 1.0));
    }

    #[test]
    fn curved_step_round_trip_and_uppercase_extension() {
        let directory = tempfile::tempdir().unwrap();
        for (name, shape) in [
            (
                "cylinder.step",
                opencascade::primitives::Shape::cylinder_radius_height(5.0, 20.0),
            ),
            (
                "sphere.STEP",
                opencascade::primitives::Shape::sphere(5.0).build(),
            ),
        ] {
            let path = directory.path().join(name);
            shape.write_step(&path).unwrap();
            let model = load(&path, 0.001).unwrap();
            assert!(!model.indices.is_empty());
            assert!(model.warnings.is_empty());
            assert!(model.dimensions.x > 9.5);
            assert!(model.dimensions.y > 9.5);
            assert!(model
                .vertices
                .iter()
                .all(|vertex| Vec3::from_array(vertex.position).is_finite()));
        }
    }

    #[test]
    fn ply_mesh_and_point_cloud() {
        let header = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\n";
        let vertices = "0 0 0 255 0 0\n1 0 0 0 255 0\n0 1 0 0 0 255\n";
        let mesh = format!("{header}element face 1\nproperty list uchar int vertex_indices\nend_header\n{vertices}3 0 1 2\n");
        let model = load_ply(mesh.as_bytes())
            .unwrap()
            .finish("test".into())
            .unwrap();
        assert_eq!(model.indices, [0, 1, 2]);
        assert_eq!(model.vertices[0].color, [1.0, 0.0, 0.0]);
        let cloud = format!("{header}end_header\n{vertices}");
        assert!(load_ply(cloud.as_bytes())
            .unwrap()
            .finish("cloud".into())
            .unwrap()
            .indices
            .is_empty());
    }

    #[test]
    fn rejects_invalid_geometry() {
        let raw = RawMesh {
            positions: vec![DVec3::ZERO],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        assert!(raw.finish("invalid".into()).is_err());
        let raw = RawMesh {
            positions: vec![DVec3::splat(f64::NAN)],
            ..Default::default()
        };
        assert!(raw.finish("invalid".into()).is_err());
    }

    #[test]
    fn triangulates_concave_polygon() {
        let mut raw = RawMesh {
            positions: vec![
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(2.0, 0.0, 0.0),
                DVec3::new(1.0, 0.5, 0.0),
                DVec3::new(2.0, 2.0, 0.0),
                DVec3::new(0.0, 2.0, 0.0),
            ],
            ..Default::default()
        };
        raw.polygon(&[0, 1, 2, 3, 4]).unwrap();
        assert_eq!(raw.indices.len(), 9);
    }

    #[test]
    fn imports_sample_files_including_step() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
        for file in [
            "pyramid.ply",
            "pyramid.obj",
            "triangle.stl",
            "triangle.gltf",
            "plate.step",
        ] {
            let model = load(&directory.join(file), 0.001)
                .unwrap_or_else(|error| panic!("{file}: {error:#}"));
            assert!(!model.indices.is_empty(), "{file}");
            assert!(
                model
                    .vertices
                    .iter()
                    .all(|vertex| Vec3::from_array(vertex.position).is_finite()),
                "{file}"
            );
            if file == "plate.step" {
                assert!((model.dimensions.x - 20.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn binary_ply_point_cloud() {
        for (format, little_endian) in
            [("binary_little_endian", true), ("binary_big_endian", false)]
        {
            let mut bytes = format!("ply\nformat {format} 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n").into_bytes();
            for value in [1.0_f32, 2.0, 3.0] {
                bytes.extend_from_slice(&if little_endian {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            let model = load_ply(bytes.as_slice())
                .unwrap()
                .finish("binary cloud".into())
                .unwrap();
            assert_eq!(model.vertices.len(), 1);
            assert!(model.indices.is_empty());
        }
    }
}
