/// A single mesh's raw geometry, in Husk's own format independent of
/// whatever file format it was loaded from.
#[derive(Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// A cube centred on the origin with sides of length 1 (flat shaded,
    /// four vertices per face). TEMPORARY test geometry for Section 5b;
    /// real shapes arrive in Section 5c.
    pub fn unit_cube() -> MeshData {
        // (normal, u axis, v axis), with u x v = normal.
        let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        let mut mesh = MeshData::default();
        for (normal, u, v) in faces {
            let base = mesh.positions.len() as u32;
            for (su, sv) in [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                mesh.positions.push([
                    normal[0] * 0.5 + u[0] * su + v[0] * sv,
                    normal[1] * 0.5 + u[1] * su + v[1] * sv,
                    normal[2] * 0.5 + u[2] * su + v[2] * sv,
                ]);
                mesh.normals.push(normal);
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        mesh
    }
}
