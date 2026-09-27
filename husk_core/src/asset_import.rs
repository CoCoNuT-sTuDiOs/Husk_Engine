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
}

/// Loads the first mesh primitive found in a glTF or GLB file into Husk's
/// own MeshData format. Returns an error string on any failure missing
/// file, malformed glTF, or a primitive missing positions/normals/indices.
pub fn load_gltf(path: &str, max_triangles: usize) -> Result<MeshData, String> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| format!("Failed to parse glTF: {e}"))?;

    // First pass: sum the total triangle count across every mesh primitive
    // in the file, using cheap accessor metadata only — no buffer decode —
    // so an oversized combined mesh is rejected before any real processing.
    let mut total_triangles = 0usize;
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            if let Some(indices_accessor) = primitive.indices() {
                total_triangles += indices_accessor.count() / 3;
            }
        }
    }

    if total_triangles > max_triangles {
        return Err(format!(
            "This mesh has {total_triangles} triangles across all its parts, which is over \
             the {max_triangles} limit for this platform. Try decimating it in Blender to \
             under {max_triangles} triangles and re-exporting."
        ));
    }

    // Second pass: read and combine every mesh primitive's actual geometry
    // into one MeshData, offsetting each primitive's indices by the vertex
    // count accumulated so far, so they still point at the right vertices
    // in the combined buffer.
    let mut combined = MeshData::default();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let mesh_name = mesh.name().unwrap_or("(unnamed)");

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| format!("Mesh '{mesh_name}' is missing position data"))?
                .collect();

            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .ok_or_else(|| format!("Mesh '{mesh_name}' is missing normal data"))?
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .ok_or_else(|| format!("Mesh '{mesh_name}' is missing index data"))?
                .into_u32()
                .collect();

            let index_offset = combined.positions.len() as u32;

            combined.positions.extend(positions);
            combined.normals.extend(normals);
            combined
                .indices
                .extend(indices.into_iter().map(|i| i + index_offset));
        }
    }

    if combined.positions.is_empty() {
        return Err("glTF file contains no usable mesh geometry".to_string());
    }

    Ok(combined)
}