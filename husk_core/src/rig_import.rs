use glam::{Mat3, Mat4, Vec3};
use crate::asset_import::MeshData;
use crate::geodesic::STATIC_INFLUENCE_INDEX;
use crate::rig::{Bone, Skeleton};
use crate::skinning::VertexWeights;

/// A rigged character read from a glTF/GLB file: geometry, skeleton, and
/// the file's own per-vertex skin weights, all in Husk's formats.
#[derive(Debug)]
pub struct RiggedModel {
    pub mesh: MeshData,
    pub skeleton: Skeleton,
    /// One entry per vertex, aligned with `mesh.positions`. Joint indices
    /// are Husk bone indices (already remapped), weights sum to 1.
    pub vertex_weights: Vec<VertexWeights>,
    /// The file's inverse bind matrices, reordered to match `skeleton`'s
    /// bone order. Kept to validate the import; the renderer derives its
    /// own from the skeleton.
    pub inverse_bind_matrices: Vec<Mat4>,
}

/// Loads the first skin (rig) in a glTF/GLB file along with every mesh
/// attached to it. Supports one rig per file and four influences per
/// vertex (glTF's JOINTS_0 / WEIGHTS_0).
pub fn load_gltf_rig(path: &str, max_triangles: usize) -> Result<RiggedModel, String> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| format!("Failed to parse glTF: {e}"))?;

    let skin = document
        .skins()
        .next()
        .ok_or_else(|| "This glTF file has no rig (no skin). Try a rigged model.".to_string())?;

    // Local and global transforms for every node in the file.
    let node_count = document.nodes().len();
    let mut local_matrices = vec![Mat4::IDENTITY; node_count];
    let mut parents: Vec<Option<usize>> = vec![None; node_count];
    let mut node_names: Vec<Option<String>> = vec![None; node_count];
    for node in document.nodes() {
        local_matrices[node.index()] = Mat4::from_cols_array_2d(&node.transform().matrix());
        node_names[node.index()] = node.name().map(|name| name.to_string());
        for child in node.children() {
            parents[child.index()] = Some(node.index());
        }
    }
    let global = |node_index: usize| -> Mat4 {
        let mut matrix = local_matrices[node_index];
        let mut current = node_index;
        while let Some(parent) = parents[current] {
            matrix = local_matrices[parent] * matrix;
            current = parent;
        }
        matrix
    };

    // The skin's joints, in the file's order. Vertex JOINTS_0 values are
    // positions in this list ("slots").
    let joint_nodes: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
    let joint_count = joint_nodes.len();
    if joint_count == 0 {
        return Err("The rig in this file has no joints".to_string());
    }
    if joint_count > STATIC_INFLUENCE_INDEX as usize {
        return Err(format!(
            "This rig has {joint_count} joints; Husk supports up to {} for now",
            STATIC_INFLUENCE_INDEX
        ));
    }

    let mut slot_of_node: Vec<Option<usize>> = vec![None; node_count];
    for (slot, &node_index) in joint_nodes.iter().enumerate() {
        slot_of_node[node_index] = Some(slot);
    }

    // Each joint's nearest ancestor that is also a joint.
    let parent_slot: Vec<Option<usize>> = joint_nodes
        .iter()
        .map(|&node_index| {
            let mut current = node_index;
            while let Some(parent) = parents[current] {
                if let Some(slot) = slot_of_node[parent] {
                    return Some(slot);
                }
                current = parent;
            }
            None
        })
        .collect();

    // Husk needs parents before children; glTF doesn't promise that.
    let order = order_parents_first(&parent_slot)?;
    let mut bone_of_slot = vec![0usize; joint_count];
    for (bone_index, &slot) in order.iter().enumerate() {
        bone_of_slot[slot] = bone_index;
    }

    // The file's inverse bind matrices, by slot (identity if absent).
    let skin_reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
    let file_inverse_binds: Vec<Mat4> = match skin_reader.read_inverse_bind_matrices() {
        Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
        None => vec![Mat4::IDENTITY; joint_count],
    };
    if file_inverse_binds.len() != joint_count {
        return Err(format!(
            "The rig has {joint_count} joints but {} inverse bind matrices",
            file_inverse_binds.len()
        ));
    }

    // Build the skeleton in parents-first order. A bone's local transform
    // is relative to its nearest joint ancestor; a root bone's is its full
    // world transform, so any non-joint nodes above it are included.
    let mut skeleton = Skeleton::default();
    let mut inverse_bind_matrices = Vec::with_capacity(joint_count);
    for &slot in &order {
        let node_index = joint_nodes[slot];
        let node_global = global(node_index);
        let local = match parent_slot[slot] {
            Some(parent) => global(joint_nodes[parent]).inverse() * node_global,
            None => node_global,
        };
        let (scale, rotation, translation) = local.to_scale_rotation_translation();
        let name = node_names[node_index]
            .clone()
            .unwrap_or_else(|| format!("joint_{slot}"));

        skeleton.bones.push(Bone {
            name,
            parent: parent_slot[slot].map(|parent| bone_of_slot[parent]),
            local_position: translation,
            local_rotation: rotation,
            local_scale: scale,
        });
        inverse_bind_matrices.push(file_inverse_binds[slot]);
    }

    let rest_world = skeleton.world_bind_matrices();
    let base = rest_world[0] * inverse_bind_matrices[0];
    for (i, (world, inverse_bind)) in rest_world
        .iter()
        .zip(inverse_bind_matrices.iter())
        .enumerate()
    {
        let candidate = *world * *inverse_bind;
        let differs = candidate
            .to_cols_array()
            .iter()
            .zip(base.to_cols_array().iter())
            .any(|(a, b)| (a - b).abs() > 1e-3);
        if differs {
            return Err(format!(
                "The rig's bind pose is inconsistent at joint {i} ('{}')",
                skeleton.bones[i].name
            ));
        }
    }
    let normal_matrix = Mat3::from_mat4(base).inverse().transpose();

    // Geometry and weights, from every mesh attached to this skin.
    let mut mesh = MeshData::default();
    let mut vertex_weights: Vec<VertexWeights> = Vec::new();
    let mut found_skinned_mesh = false;

    for node in document.nodes() {
        let (Some(gltf_mesh), Some(node_skin)) = (node.mesh(), node.skin()) else {
            continue;
        };
        if node_skin.index() != skin.index() {
            return Err(
                "This file uses more than one rig; Husk supports one rig per model for now"
                    .to_string(),
            );
        }
        found_skinned_mesh = true;
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
                .ok_or_else(|| format!("Mesh '{mesh_name}' has no joint indices (JOINTS_0)"))?
                .into_u16()
                .collect();
            let weights: Vec<[f32; 4]> = reader
                .read_weights(0)
                .ok_or_else(|| format!("Mesh '{mesh_name}' has no skin weights (WEIGHTS_0)"))?
                .into_f32()
                .collect();

            if normals.len() != positions.len()
                || joints.len() != positions.len()
                || weights.len() != positions.len()
            {
                return Err(format!(
                    "Mesh '{mesh_name}' has mismatched vertex data lengths"
                ));
            }

            for (joint_set, weight_set) in joints.iter().zip(weights.iter()) {
                let mut joint_indices = [0u32; 4];
                let mut weights_out = [0.0f32; 4];
                let total: f32 = weight_set.iter().sum();

                if total > 1e-6 {
                    for i in 0..4 {
                        if weight_set[i] > 0.0 {
                            let slot = joint_set[i] as usize;
                            if slot >= joint_count {
                                return Err(format!(
                                    "Mesh '{mesh_name}' uses joint {slot}, but the rig only has {joint_count}"
                                ));
                            }
                            joint_indices[i] = bone_of_slot[slot] as u32;
                            weights_out[i] = weight_set[i] / total;
                        }
                    }
                } else {
                    // Unweighted vertex: keep it still.
                    joint_indices[0] = STATIC_INFLUENCE_INDEX;
                    weights_out[0] = 1.0;
                }

                vertex_weights.push(VertexWeights {
                    joint_indices,
                    weights: weights_out,
                });
            }

            let index_offset = mesh.positions.len() as u32;
            mesh.positions.extend(
                positions
                    .iter()
                    .map(|p| base.transform_point3(Vec3::from(*p)).to_array()),
            );
            mesh.normals.extend(normals.iter().map(|n| {
                (normal_matrix * Vec3::from(*n)).normalize_or_zero().to_array()
            }));
            mesh.indices
                .extend(indices.into_iter().map(|i| i + index_offset));
        }
    }

    if !found_skinned_mesh || mesh.positions.is_empty() {
        return Err("No mesh in this file is attached to the rig".to_string());
    }
    if mesh.triangle_count() > max_triangles {
        return Err(format!(
            "This mesh has {} triangles, over the {max_triangles} limit for this platform",
            mesh.triangle_count()
        ));
    }

    Ok(RiggedModel {
        mesh,
        skeleton,
        vertex_weights,
        inverse_bind_matrices,
    })
}

/// Returns slot indices ordered so every slot comes after its parent.
/// `parent_slot[i]` is slot i's parent slot (None for a root). Errors if
/// the hierarchy contains a cycle.
fn order_parents_first(parent_slot: &[Option<usize>]) -> Result<Vec<usize>, String> {
    let count = parent_slot.len();
    let mut order: Vec<usize> = Vec::with_capacity(count);
    let mut placed = vec![false; count];

    while order.len() < count {
        let before = order.len();
        for slot in 0..count {
            if placed[slot] {
                continue;
            }
            let parent_ready = match parent_slot[slot] {
                Some(parent) => placed[parent],
                None => true,
            };
            if parent_ready {
                placed[slot] = true;
                order.push(slot);
            }
        }
        if order.len() == before {
            return Err("The rig's joint hierarchy contains a cycle".to_string());
        }
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets");

    #[test]
    fn shuffled_hierarchy_is_reordered_parents_first() {
        // Slot 1 is the root, slot 2 hangs off it, slot 0 hangs off slot 2.
        let order = order_parents_first(&[Some(2), None, Some(1)]).unwrap();
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn cyclic_hierarchy_is_rejected() {
        assert!(order_parents_first(&[Some(1), Some(0)]).is_err());
    }

    fn check_rig(file_name: &str, expected_bones: usize, expected_vertices: usize) {
    let path = format!("{ASSET_DIR}/{file_name}");
        let model = load_gltf_rig(&path, usize::MAX)
            .unwrap_or_else(|e| panic!("could not load {path}: {e}"));

        println!(
            "{file_name}: {} bones, {} vertices, {} triangles",
            model.skeleton.bones.len(),
            model.mesh.positions.len(),
            model.mesh.triangle_count()
        );

        // Enough bones, and every array lines up.
        assert_eq!(model.skeleton.bones.len(), expected_bones);
        assert_eq!(model.mesh.positions.len(), expected_vertices);
        assert_eq!(model.mesh.positions.len(), model.mesh.normals.len());
        assert_eq!(model.mesh.positions.len(), model.vertex_weights.len());
        assert_eq!(model.skeleton.bones.len(), model.inverse_bind_matrices.len());

        // Parents come before children, and there is at least one root.
        let mut roots = 0;
        for (i, bone) in model.skeleton.bones.iter().enumerate() {
            match bone.parent {
                Some(parent) => assert!(parent < i, "{file_name}: bone {i} comes before its parent"),
                None => roots += 1,
            }
        }
        assert!(roots >= 1, "{file_name}: no root bone");

        // Every vertex's weights sum to 1 and point at real bones.
        let bone_count = model.skeleton.bones.len() as u32;
        for (i, vertex) in model.vertex_weights.iter().enumerate() {
            let total: f32 = vertex.weights.iter().sum();
            assert!(
                (total - 1.0).abs() < 1e-3,
                "{file_name}: vertex {i} weights sum to {total}"
            );
            for slot in 0..4 {
                if vertex.weights[slot] > 0.0 {
                    let joint = vertex.joint_indices[slot];
                    assert!(
                        joint < bone_count || joint == STATIC_INFLUENCE_INDEX,
                        "{file_name}: vertex {i} points at bone {joint}"
                    );
                }
            }
        }

        // After the import bakes the file's mesh-to-scene transform into the
        // vertices, every bone must sit inside the mesh's bounding box (with
        // 5% slack), and the model must be taller than it is wide (Y-up).
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for p in &model.mesh.positions {
            for axis in 0..3 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
        }
        let world = model.skeleton.world_bind_matrices();
        for (i, rest) in world.iter().enumerate() {
            let position = rest.transform_point3(glam::Vec3::ZERO).to_array();
            for axis in 0..3 {
                let slack = 0.05 * (high[axis] - low[axis]);
                assert!(
                    position[axis] >= low[axis] - slack && position[axis] <= high[axis] + slack,
                    "{file_name}: bone {i} ('{}') sits outside the mesh on axis {axis}: {} not in [{}, {}]",
                    model.skeleton.bones[i].name,
                    position[axis],
                    low[axis],
                    high[axis]
                );
            }
        }
        let extent = [high[0] - low[0], high[1] - low[1], high[2] - low[2]];
        assert!(
            extent[1] >= extent[0] && extent[1] >= extent[2],
            "{file_name}: model is not upright, extents (x, y, z) = {extent:?}"
        );

    }

    #[test]
    fn rigged_simple_imports_cleanly() {
        check_rig("RiggedSimple.glb", 2, 160);
    }

    #[test]
    fn cesium_man_imports_cleanly() {
        check_rig("CesiumMan.glb", 19, 3273);
    }
}