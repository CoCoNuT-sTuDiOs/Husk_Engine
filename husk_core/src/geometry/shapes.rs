use super::mesh::MeshData;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A flat square lying in the XZ plane, centred on the origin, facing up
/// (+Y), with sides of length `size`.
pub fn plane(size: f32) -> MeshData {
    let h = size * 0.5;
    MeshData {
        positions: vec![[-h, 0.0, -h], [-h, 0.0, h], [h, 0.0, -h], [h, 0.0, h]],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        indices: vec![0, 1, 2, 1, 3, 2],
    }
}

/// A capsule standing along the Y axis, centred on the origin: a
/// cylinder of `cylinder_height` between two hemispheres of `radius`.
/// Total height is `cylinder_height + 2 * radius`. `segments` is how many
/// slices go around the axis, `rings` how many bands each hemisphere has.
pub fn capsule(radius: f32, cylinder_height: f32, segments: u32, rings: u32) -> MeshData {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let half = cylinder_height * 0.5;
    let row_len = segments + 1;
    let mut mesh = MeshData::default();

    // Rows run from the top pole down to the bottom pole. The top
    // hemisphere is centred at +half, the bottom one at -half; the last
    // row of the top and the first row of the bottom are both equators,
    // and the band between them is the cylinder wall.
    for hemisphere in 0..2 {
        for step in 0..=rings {
            let t = step as f32 / rings as f32 * FRAC_PI_2;
            let (theta, centre_y) = if hemisphere == 0 {
                (t, half)
            } else {
                (FRAC_PI_2 + t, -half)
            };
            for s in 0..=segments {
                let phi = s as f32 / segments as f32 * TAU;
                let n = [
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                ];
                mesh.positions
                    .push([radius * n[0], centre_y + radius * n[1], radius * n[2]]);
                mesh.normals.push(n);
            }
        }
    }

    let rows = 2 * (rings + 1);
    for row in 0..rows - 1 {
        for s in 0..segments {
            let a = row * row_len + s;
            let b = a + 1;
            let c = a + row_len;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    mesh
}

/// A torus lying flat (hole along the Y axis), centred on the origin.
/// `major_radius` is the distance from the centre to the middle of the
/// tube, `minor_radius` the tube's own radius. `major_segments` slices go
/// around the hole, `minor_segments` around the tube.
pub fn torus(
    major_radius: f32,
    minor_radius: f32,
    major_segments: u32,
    minor_segments: u32,
) -> MeshData {
    let major_segments = major_segments.max(3);
    let minor_segments = minor_segments.max(3);
    let row_len = minor_segments + 1;
    let mut mesh = MeshData::default();

    for i in 0..=major_segments {
        let u = i as f32 / major_segments as f32 * TAU;
        for j in 0..=minor_segments {
            let v = j as f32 / minor_segments as f32 * TAU;
            let n = [v.cos() * u.cos(), v.sin(), v.cos() * u.sin()];
            let ring = major_radius + minor_radius * v.cos();
            mesh.positions
                .push([ring * u.cos(), minor_radius * v.sin(), ring * u.sin()]);
            mesh.normals.push(n);
        }
    }

    for i in 0..major_segments {
        for j in 0..minor_segments {
            let a = i * row_len + j;
            let b = a + 1;
            let c = a + row_len;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    mesh
}


/// A sphere centred on the origin. `segments` slices go around the Y
/// axis and `rings` bands run from pole to pole.
pub fn sphere(radius: f32, segments: u32, rings: u32) -> MeshData {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let row_len = segments + 1;
    let mut mesh = MeshData::default();

    for step in 0..=rings {
        let theta = step as f32 / rings as f32 * PI;
        for s in 0..=segments {
            let phi = s as f32 / segments as f32 * TAU;
            let n = [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ];
            mesh.positions
                .push([radius * n[0], radius * n[1], radius * n[2]]);
            mesh.normals.push(n);
        }
    }

    for row in 0..rings {
        for s in 0..segments {
            let a = row * row_len + s;
            let b = a + 1;
            let c = a + row_len;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    mesh
}

/// The built-in shapes, in the order the tray shows them. `by_name`
/// understands exactly these (any letter case). The tray asks the engine
/// for this list, so a new shape only has to be added here and in
/// `by_name`.
pub const SHAPE_NAMES: [&str; 5] = ["cube", "sphere", "capsule", "torus", "plane"];

/// Looks up a built-in shape by name (see `SHAPE_NAMES`, any letter
/// case). Each is about one unit across, except the plane, which is
/// four. None for an unknown name.
pub fn by_name(name: &str) -> Option<MeshData> {
    match name.to_lowercase().as_str() {
        "cube" => Some(MeshData::unit_cube()),
        "sphere" => Some(sphere(0.5, 32, 16)),
        "capsule" => Some(capsule(0.25, 0.5, 24, 8)),
        "torus" => Some(torus(0.4, 0.1, 32, 16)),
        "plane" => Some(plane(4.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    fn extent(mesh: &MeshData, axis: usize) -> f32 {
        let min = mesh.positions.iter().map(|p| p[axis]).fold(f32::MAX, f32::min);
        let max = mesh.positions.iter().map(|p| p[axis]).fold(f32::MIN, f32::max);
        max - min
    }

    fn assert_well_formed(mesh: &MeshData) {
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        for &index in &mesh.indices {
            assert!((index as usize) < mesh.positions.len(), "index out of range");
        }
        for n in &mesh.normals {
            let length = Vec3::from(*n).length();
            assert!((length - 1.0).abs() < 1e-4, "normal length {length}");
        }
    }

    /// Every real triangle's face normal (from its winding) must point the
    /// same way as its vertex normals, so the shape is not inside out.
    fn assert_outward_winding(mesh: &MeshData) {
        for tri in mesh.indices.chunks(3) {
            let p: Vec<Vec3> = tri
                .iter()
                .map(|&i| Vec3::from(mesh.positions[i as usize]))
                .collect();
            let face = (p[1] - p[0]).cross(p[2] - p[0]);
            if face.length() < 1e-6 {
                continue; // degenerate sliver at a pole
            }
            let n: Vec3 = tri
                .iter()
                .map(|&i| Vec3::from(mesh.normals[i as usize]))
                .sum();
            assert!(face.dot(n) > 0.0, "triangle {tri:?} faces inward");
        }
    }

    #[test]
    fn by_name_finds_the_four_shapes_and_rejects_others() {
        for name in SHAPE_NAMES.iter().copied().chain(["Torus", "CUBE"]) {
            let mesh = by_name(name).unwrap_or_else(|| panic!("{name} not found"));
            assert_well_formed(&mesh);
            assert!(mesh.triangle_count() > 0);
        }
        assert!(by_name("teapot").is_none());
        assert!(by_name("").is_none());
    }
    #[test]
    fn sphere_has_the_right_size_and_counts() {
        let (radius, segments, rings) = (0.5, 24u32, 12u32);
        let mesh = sphere(radius, segments, rings);
        assert_well_formed(&mesh);
        assert_outward_winding(&mesh);
        assert_eq!(mesh.positions.len() as u32, (rings + 1) * (segments + 1));
        assert_eq!(mesh.indices.len() as u32, rings * segments * 2 * 3);
        for axis in 0..3 {
            assert!((extent(&mesh, axis) - 2.0 * radius).abs() < 1e-4);
        }
        for p in &mesh.positions {
            assert!((Vec3::from(*p).length() - radius).abs() < 1e-5);
        }
    }

    #[test]
    fn plane_is_flat_and_faces_up() {
    let mesh = plane(2.0);
        assert_well_formed(&mesh);
        assert_outward_winding(&mesh);
        assert_eq!(mesh.indices.len(), 6);
        assert!((extent(&mesh, 0) - 2.0).abs() < 1e-5);
        assert!((extent(&mesh, 2) - 2.0).abs() < 1e-5);
        assert!(extent(&mesh, 1).abs() < 1e-6);
    }

    #[test]
    fn capsule_has_the_right_size_and_counts() {
        let (radius, height, segments, rings) = (0.5, 1.0, 16u32, 8u32);
        let mesh = capsule(radius, height, segments, rings);
        assert_well_formed(&mesh);
        assert_outward_winding(&mesh);
        assert_eq!(
            mesh.positions.len() as u32,
            2 * (rings + 1) * (segments + 1)
        );
        assert_eq!(
            mesh.indices.len() as u32,
            (2 * (rings + 1) - 1) * segments * 2 * 3
        );
        assert!((extent(&mesh, 1) - (height + 2.0 * radius)).abs() < 1e-4);
        assert!((extent(&mesh, 0) - 2.0 * radius).abs() < 1e-4);
        assert!((extent(&mesh, 2) - 2.0 * radius).abs() < 1e-4);
    }

    #[test]
    fn torus_has_the_right_size_and_counts() {
        let (major, minor, ms, ns) = (1.0, 0.25, 24u32, 12u32);
        let mesh = torus(major, minor, ms, ns);
        assert_well_formed(&mesh);
        assert_outward_winding(&mesh);
        assert_eq!(mesh.positions.len() as u32, (ms + 1) * (ns + 1));
        assert_eq!(mesh.indices.len() as u32, ms * ns * 2 * 3);
        assert!((extent(&mesh, 0) - 2.0 * (major + minor)).abs() < 1e-4);
        assert!((extent(&mesh, 2) - 2.0 * (major + minor)).abs() < 1e-4);
        assert!((extent(&mesh, 1) - 2.0 * minor).abs() < 1e-4);
    }
}