use crate::geometry::mesh::MeshData;
/// A character read from a glTF file that carries a skin: its geometry,
/// its skeleton, and per-vertex skin weights, all in Husk's own formats.
#[derive(Debug)]
pub struct RiggedMesh {
    pub mesh: MeshData,
    pub skeleton: crate::rig::Skeleton,
    /// One entry per mesh vertex, same order as `mesh.positions`. Joint
    /// indices refer to `skeleton.bones`. A vertex the file left
    /// unweighted points at the "stays put" slot instead.
    pub weights: Vec<crate::rig::skinning::VertexWeights>,
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
    let mut weights: Vec<crate::rig::skinning::VertexWeights> = Vec::new();

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
) -> Result<crate::rig::skinning::VertexWeights, String> {
    let mut joint_indices = [crate::rig::geodesic::STATIC_INFLUENCE_INDEX; 4];
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

    Ok(crate::rig::skinning::VertexWeights {
        joint_indices,
        weights: normalized,
    })
}

/// Every node of the file's scene that has a mesh, paired with its world
/// matrix: its own matrix (the file's matrix, or its position, rotation and
/// scale) on top of its parent's, all the way up. A node whose world matrix
/// flattens everything (a zero scale) is left out, since it shows nothing. A
/// file with no scene uses every node that has no parent. A node reached
/// twice (a loop, which the glTF specification forbids) is an error.
fn placed_mesh_nodes(
    document: &gltf::Document,
) -> Result<Vec<(gltf::Node<'_>, glam::Mat4)>, String> {
    let node_count = document.nodes().count();
    let mut stack: Vec<(gltf::Node<'_>, glam::Mat4)> = Vec::new();
    match document.default_scene().or_else(|| document.scenes().next()) {
        Some(scene) => {
            for node in scene.nodes() {
                stack.push((node, glam::Mat4::IDENTITY));
            }
        }
        None => {
            let mut has_parent = vec![false; node_count];
            for node in document.nodes() {
                for child in node.children() {
                    has_parent[child.index()] = true;
                }
            }
            for node in document.nodes() {
                if !has_parent[node.index()] {
                    stack.push((node, glam::Mat4::IDENTITY));
                }
            }
        }
    }

    let mut visited = vec![false; node_count];
    let mut placed = Vec::new();
    while let Some((node, parent_world)) = stack.pop() {
        if std::mem::replace(&mut visited[node.index()], true) {
            return Err(format!(
                "Node {} appears twice in this glTF file's node tree",
                node.index()
            ));
        }
        let world = parent_world * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        for child in node.children() {
            stack.push((child, world));
        }
        if node.mesh().is_some() && world.determinant() != 0.0 && world.is_finite() {
            placed.push((node, world));
        }
    }
    Ok(placed)
}

/// Moves a mesh's geometry into the scene by a node's world matrix: points
/// are moved by the matrix, and normals by its inverse transpose so they stay
/// at right angles to the surface under any scale, then made unit length
/// again. A matrix that mirrors the mesh (a negative scale) turns its
/// triangles inside out, so their winding is reversed, as the glTF
/// specification says.
fn place_geometry(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    mut indices: Vec<u32>,
    world: &glam::Mat4,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
    let linear = glam::Mat3::from_mat4(*world);
    let normal_matrix = linear.inverse().transpose();
    let positions: Vec<[f32; 3]> = positions
        .into_iter()
        .map(|p| world.transform_point3(glam::Vec3::from(p)).to_array())
        .collect();
    let normals: Vec<[f32; 3]> = normals
        .into_iter()
        .map(|n| {
            (normal_matrix * glam::Vec3::from(n))
                .normalize_or_zero()
                .to_array()
        })
        .collect();
    if linear.determinant() < 0.0 {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    (positions, normals, indices)
}

/// Loads every mesh in a glTF or GLB file's scene into Husk's own MeshData
/// format, combined into one mesh and placed where the file's nodes put them
/// (see `mesh_from_gltf`). Returns an error string on any failure: missing
/// file, malformed glTF, or a primitive missing positions/normals/indices.
pub fn load_gltf(path: &str, max_triangles: usize) -> Result<MeshData, String> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| format!("Failed to parse glTF: {e}"))?;
    mesh_from_gltf(&document, &buffers, max_triangles)
}

/// The parsing half of `load_gltf`, split out so tests can feed it a glTF
/// built in memory. Every mesh is placed where the file's node tree puts it:
/// each node's matrix is applied on top of its parent's, as the glTF 2.0
/// specification says, and a mesh used by two nodes appears twice.
pub fn mesh_from_gltf(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    max_triangles: usize,
) -> Result<MeshData, String> {
    let placed = placed_mesh_nodes(document)?;

    // First pass: sum the total triangle count across every mesh primitive
// in the file, using cheap accessor metadata only — no buffer decode —
    // so an oversized combined mesh is rejected before any real processing.
    let mut total_triangles = 0usize;
    for (node, _) in &placed {
        let Some(mesh) = node.mesh() else {
            continue;
        };
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

    for (node, world) in &placed {
        let Some(mesh) = node.mesh() else {
            continue;
        };
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

            let (positions, normals, indices) = place_geometry(positions, normals, indices, world);
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

#[cfg(test)]
mod tests {
    use super::*;

    // A glTF with one triangle, built in memory: the points (0,0,0), (1,0,0)
    // and (0,1,0), all with their normals along +Z. The start and end of the
    // file are fixed; each test writes the middle, which holds the scenes and
    // nodes that say where the triangle's mesh is placed.
    const FILE_START: &str = r#"{"asset":{"version":"2.0"},"#;
    const FILE_END: &str = r#","meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1},"indices":2}]}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0.0,0.0,0.0],"max":[1.0,1.0,0.0]},{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":2,"componentType":5123,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36},{"buffer":0,"byteOffset":72,"byteLength":6}],"buffers":[{"uri":"triangle.bin","byteLength":78}]}"#;

    fn triangle_buffers() -> Vec<gltf::buffer::Data> {
        let mut bytes: Vec<u8> = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for _ in 0..3 {
            for value in [0.0f32, 0.0, 1.0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        for index in [0u16, 1, 2] {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        vec![gltf::buffer::Data(bytes)]
    }

    fn load(nodes_and_scenes: &str) -> Result<MeshData, String> {
        let json = format!("{FILE_START}{nodes_and_scenes}{FILE_END}");
        let file = gltf::Gltf::from_slice(json.as_bytes()).expect("the test glTF should parse");
        mesh_from_gltf(&file.document, &triangle_buffers(), 1000)
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (glam::Vec3::from(a) - glam::Vec3::from(b)).length() < 1e-4
    }

    #[test]
    fn a_matrix_moves_points_and_turns_normals() {
        let world = glam::Mat4::from_translation(glam::Vec3::new(10.0, 0.0, 0.0))
            * glam::Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * glam::Mat4::from_scale(glam::Vec3::splat(2.0));
        let (positions, normals, indices) = place_geometry(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![0, 1, 2],
            &world,
        );
        // Twice as big, a quarter turn about Y (which sends +X to -Z), then
        // 10 along X.
        assert!(close(positions[0], [10.0, 0.0, 0.0]));
        assert!(close(positions[1], [10.0, 0.0, -2.0]));
        assert!(close(positions[2], [10.0, 2.0, 0.0]));
        // A quarter turn sends +Z to +X, and the size does not change a
        // normal's length.
        assert!(close(normals[0], [1.0, 0.0, 0.0]));
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn a_mirrored_matrix_turns_the_triangles_inside_out() {
        let world = glam::Mat4::from_scale(glam::Vec3::new(-1.0, 1.0, 1.0));
        let (positions, normals, indices) = place_geometry(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![0, 1, 2],
            &world,
        );
        assert!(close(positions[1], [-1.0, 0.0, 0.0]));
        // The surface still faces the same way.
        assert!(close(normals[0], [0.0, 0.0, 1.0]));
        // Mirroring reverses the winding, so the triangle is listed the other
        // way round.
        assert_eq!(indices, vec![0, 2, 1]);
    }

    #[test]
    fn a_meshs_node_and_its_parents_place_it_in_the_scene() {
        // The parent moves 10 along X. Its child, which holds the mesh, is
        // turned a quarter turn about Y and made twice as big.
        let mesh = load(
            r#""scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"translation":[10.0,0.0,0.0],"children":[1]},{"mesh":0,"rotation":[0.0,0.7071068,0.0,0.7071068],"scale":[2.0,2.0,2.0]}]"#,
        )
        .unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert!(close(mesh.positions[0], [10.0, 0.0, 0.0]));
        assert!(close(mesh.positions[1], [10.0, 0.0, -2.0]));
        assert!(close(mesh.positions[2], [10.0, 2.0, 0.0]));
        assert!(close(mesh.normals[0], [1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_mesh_used_by_two_nodes_appears_twice() {
        let mesh = load(
            r#""scene":0,"scenes":[{"nodes":[0,1]}],"nodes":[{"mesh":0},{"mesh":0,"translation":[5.0,0.0,0.0]}]"#,
        )
        .unwrap();
        assert_eq!(mesh.positions.len(), 6);
        assert_eq!(mesh.triangle_count(), 2);
        // One copy sits at the origin and the other 5 along X, in either order.
        let mut starts = [mesh.positions[0][0], mesh.positions[3][0]];
        starts.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(starts, [0.0, 5.0]);
        // The second copy's triangle points at the second copy's vertices.
        assert_eq!(&mesh.indices[3..], &[3, 4, 5]);
    }

    #[test]
    fn a_file_without_a_scene_places_the_nodes_that_have_no_parent() {
        // Node 1 is node 0's child, so it is placed once, through its parent,
        // and not a second time as if it were a top-level node.
        let mesh = load(
            r#""nodes":[{"mesh":0,"children":[1]},{"mesh":0,"translation":[5.0,0.0,0.0]}]"#,
        )
        .unwrap();
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn a_node_tree_that_loops_is_refused_instead_of_hanging() {
        let middle =
            r#""scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0,"children":[1]},{"children":[0]}]"#;
        let json = format!("{FILE_START}{middle}{FILE_END}");
        // The file may be refused when it is parsed or when its nodes are
        // walked; either way it must not hang or load.
        if let Ok(file) = gltf::Gltf::from_slice(json.as_bytes()) {
            assert!(mesh_from_gltf(&file.document, &triangle_buffers(), 1000).is_err());
        }
    }
}