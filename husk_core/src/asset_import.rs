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

/// A character read from a glTF file that carries a skin: its geometry,
/// its skeleton, and per-vertex skin weights, all in Husk's own formats.
#[derive(Debug)]
pub struct RiggedMesh {
    pub mesh: MeshData,
    pub skeleton: crate::rig::Skeleton,
    /// One entry per mesh vertex, same order as `mesh.positions`. Joint
    /// indices refer to `skeleton.bones`. A vertex the file left
    /// unweighted points at the "stays put" slot instead.
    pub weights: Vec<crate::skinning::VertexWeights>,
    /// Largest relative difference between the file's inverse bind
    /// matrices and the ones implied by the skeleton built here. Near
    /// zero means Husk's skinning math will match the file.
    pub bind_mismatch: f32,
    /// Mesh nodes in the file that were not part of the imported skin.
    pub skipped_mesh_nodes: usize,
}

/// Loads a rigged glTF or GLB file: the first skin's skeleton, plus every
/// mesh that uses it, with per-vertex joint weights.
pub fn load_gltf_rigged(path: &str, max_triangles: usize) -> Result<RiggedMesh, String> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| format!("Failed to parse glTF: {e}"))?;
    rigged_mesh_from_gltf(&document, &buffers, max_triangles)
}

/// The parsing half of `load_gltf_rigged`, split out so tests can feed it
/// a glTF built in memory.
pub fn rigged_mesh_from_gltf(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    max_triangles: usize,
) -> Result<RiggedMesh, String> {
    // The GPU holds 128 bone matrices; slot 127 is reserved as "stays put".
    const MAX_JOINTS: usize = 127;

    let skin = document.skins().next().ok_or_else(|| {
        "This glTF file has no skin, so it has no skeleton or weights to import".to_string()
    })?;
    let joint_nodes: Vec<usize> = skin.joints().map(|node| node.index()).collect();
    if joint_nodes.is_empty() {
        return Err("The skin in this glTF file has no joints".to_string());
    }
    if joint_nodes.len() > MAX_JOINTS {
        return Err(format!(
            "This rig has {} joints; Husk supports up to {MAX_JOINTS}",
            joint_nodes.len()
        ));
    }

    // Node hierarchy: parent of every node, and every node's world matrix.
    let node_count = document.nodes().count();
    let node_names: Vec<String> = document
        .nodes()
        .map(|node| node.name().unwrap_or("(unnamed)").to_string())
        .collect();
    let mut parent_of: Vec<Option<usize>> = vec![None; node_count];
    for node in document.nodes() {
        for child in node.children() {
            parent_of[child.index()] = Some(node.index());
        }
    }
    let local_matrices: Vec<glam::Mat4> = document
        .nodes()
        .map(|node| glam::Mat4::from_cols_array_2d(&node.transform().matrix()))
        .collect();

    let mut known_world: Vec<Option<glam::Mat4>> = vec![None; node_count];
    for start in 0..node_count {
        // Walk up to the first ancestor whose world matrix is known,
        // then come back down multiplying local matrices.
        let mut chain: Vec<usize> = Vec::new();
        let mut current = Some(start);
        while let Some(node) = current {
            if known_world[node].is_some() {
                break;
            }
            chain.push(node);
            current = parent_of[node];
        }
        let mut running = match current {
            Some(node) => known_world[node].unwrap(),
            None => glam::Mat4::IDENTITY,
        };
        for &node in chain.iter().rev() {
            running = running * local_matrices[node];
            known_world[node] = Some(running);
        }
    }
    let world_matrices: Vec<glam::Mat4> =
        known_world.into_iter().map(|m| m.unwrap()).collect();

    // Each joint's parent is its nearest ancestor that is also a joint
    // (other nodes in between, like an "Armature" node, are folded into
    // the transforms).
    let mut slot_of_node: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for (slot, &node) in joint_nodes.iter().enumerate() {
        slot_of_node.insert(node, slot);
    }
    let parent_slot: Vec<Option<usize>> = joint_nodes
        .iter()
        .map(|&node| {
            let mut ancestor = parent_of[node];
            while let Some(candidate) = ancestor {
                if let Some(&slot) = slot_of_node.get(&candidate) {
                    return Some(slot);
                }
                ancestor = parent_of[candidate];
            }
            None
        })
        .collect();

    // Husk needs parents stored before children; glTF doesn't promise
    // that. Sort joints by depth and remember where each one moved to.
    let depth: Vec<usize> = (0..joint_nodes.len())
        .map(|slot| {
            let mut steps = 0;
            let mut current = parent_slot[slot];
            while let Some(parent) = current {
                steps += 1;
                current = parent_slot[parent];
            }
            steps
        })
        .collect();
    let mut order: Vec<usize> = (0..joint_nodes.len()).collect();
    order.sort_by_key(|&slot| (depth[slot], slot));
    let mut remap = vec![0usize; joint_nodes.len()];
    for (new_index, &slot) in order.iter().enumerate() {
        remap[slot] = new_index;
    }

    let mut skeleton = crate::rig::Skeleton::default();
    for &slot in &order {
        let node = joint_nodes[slot];
        let local = match parent_slot[slot] {
            Some(parent) => world_matrices[joint_nodes[parent]].inverse() * world_matrices[node],
            None => world_matrices[node],
        };
        let (scale, rotation, translation) = local.to_scale_rotation_translation();
        skeleton.bones.push(crate::rig::Bone {
            name: node_names[node].clone(),
            parent: parent_slot[slot].map(|parent| remap[parent]),
            local_position: translation,
            local_rotation: rotation,
            local_scale: scale,
        });
    }

    // Sanity check against the file's own inverse bind matrices.
    let inverse_binds: Vec<glam::Mat4> = match skin
        .reader(|buffer| Some(&buffers[buffer.index()]))
        .read_inverse_bind_matrices()
    {
        Some(matrices) => matrices
            .map(|m| glam::Mat4::from_cols_array_2d(&m))
            .collect(),
        None => vec![glam::Mat4::IDENTITY; joint_nodes.len()],
    };
    let rest_world = skeleton.world_bind_matrices();
    let mut bind_mismatch = 0.0f32;
    for slot in 0..joint_nodes.len() {
        let expected = rest_world[remap[slot]].inverse().to_cols_array();
        let actual = inverse_binds
            .get(slot)
            .copied()
            .unwrap_or(glam::Mat4::IDENTITY)
            .to_cols_array();
        let scale = expected.iter().fold(1.0f32, |m, v| m.max(v.abs()));
        let difference = expected
            .iter()
            .zip(actual.iter())
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        bind_mismatch = bind_mismatch.max(difference / scale);
    }

    // Geometry: every mesh on a node that uses this skin.
    let mesh_node_count = document
        .nodes()
        .filter(|node| node.mesh().is_some())
        .count();
    let skinned_nodes: Vec<_> = document
        .nodes()
        .filter(|node| {
            node.mesh().is_some() && node.skin().map(|s| s.index()) == Some(skin.index())
        })
        .collect();

    let mut total_triangles = 0usize;
    for node in &skinned_nodes {
        if let Some(gltf_mesh) = node.mesh() {
            for primitive in gltf_mesh.primitives() {
                if let Some(indices_accessor) = primitive.indices() {
                    total_triangles += indices_accessor.count() / 3;
                }
            }
        }
    }
    if total_triangles > max_triangles {
        return Err(format!(
            "This mesh has {total_triangles} triangles across all its parts, which is over \
             the {max_triangles} limit for this platform."
        ));
    }

    let mut mesh = MeshData::default();
    let mut weights: Vec<crate::skinning::VertexWeights> = Vec::new();

    for node in &skinned_nodes {
        let Some(gltf_mesh) = node.mesh() else {
            continue;
        };
        let mesh_name = gltf_mesh.name().unwrap_or("(unnamed)");
        for primitive in gltf_mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

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
            let joints: Vec<[u16; 4]> = reader
                .read_joints(0)
                .ok_or_else(|| format!("Mesh '{mesh_name}' is missing joint indices"))?
                .into_u16()
                .collect();
            let raw_weights: Vec<[f32; 4]> = reader
                .read_weights(0)
                .ok_or_else(|| format!("Mesh '{mesh_name}' is missing joint weights"))?
                .into_f32()
                .collect();

            if joints.len() != positions.len() || raw_weights.len() != positions.len() {
                return Err(format!(
                    "Mesh '{mesh_name}' has {} vertices but {} joint entries and {} weight entries",
                    positions.len(),
                    joints.len(),
                    raw_weights.len()
                ));
            }

            for (vertex_joints, vertex_weights) in joints.iter().zip(raw_weights.iter()) {
                weights.push(remap_vertex_weights(vertex_joints, vertex_weights, &remap)?);
            }

            let index_offset = mesh.positions.len() as u32;
            mesh.positions.extend(positions);
            mesh.normals.extend(normals);
            mesh.indices
                .extend(indices.into_iter().map(|i| i + index_offset));
        }
    }

    if mesh.positions.is_empty() {
        return Err("No mesh in this glTF file uses its skin".to_string());
    }

    Ok(RiggedMesh {
        mesh,
        skeleton,
        weights,
        bind_mismatch,
        skipped_mesh_nodes: mesh_node_count - skinned_nodes.len(),
    })
}

/// Converts one vertex's glTF joint slots and weights into Husk's bone
/// indices: remaps to the reordered skeleton, drops zero weights, and
/// normalizes so the weights sum to 1. A vertex with no weight at all
/// points at the "stays put" slot.
fn remap_vertex_weights(
    joints: &[u16; 4],
    weights: &[f32; 4],
    remap: &[usize],
) -> Result<crate::skinning::VertexWeights, String> {
    let mut joint_indices = [crate::geodesic::STATIC_INFLUENCE_INDEX; 4];
    let mut normalized = [0.0f32; 4];
    let mut total = 0.0f32;

    for slot in 0..4 {
        if weights[slot] > 0.0 {
            let bone = *remap.get(joints[slot] as usize).ok_or_else(|| {
                format!(
                    "A vertex refers to joint {} but the skin only has {} joints",
                    joints[slot],
                    remap.len()
                )
            })?;
            joint_indices[slot] = bone as u32;
            normalized[slot] = weights[slot];
            total += weights[slot];
        }
    }

    if total > 0.0 {
        for weight in normalized.iter_mut() {
            *weight /= total;
        }
    } else {
        normalized[0] = 1.0;
    }

    Ok(crate::skinning::VertexWeights {
        joint_indices,
        weights: normalized,
    })
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
