use std::cmp::Reverse;
use std::collections::BinaryHeap;

use glam::Vec3;

use crate::rig::Skeleton;
use crate::skinning::VertexWeights;
use crate::voxel::VoxelGrid;

/// Cells along the mesh's longest side. Higher resolves thinner gaps
/// between limbs, at the cost of time and memory.
const GRID_RESOLUTION: usize = 192;

/// Bone matrices the renderer reserves on the GPU. Must match MAX_BONES
/// in renderer.rs. The last slot is never used by a real bone and stays
/// identity, so it doubles as the "stays put" influence.
const GPU_BONE_SLOTS: usize = 128;
pub const STATIC_INFLUENCE_INDEX: u32 = (GPU_BONE_SLOTS - 1) as u32;

const INFLUENCES: usize = 4;

const STATIC_REACH_FRACTION: f32 = 0.10;
const MAX_REACH_MULTIPLE: f32 = 4.0;

/// Tuning knob 2: falloff steepness. Higher = each vertex commits harder
/// to its nearest bone and joints bend more sharply.
const FALLOFF_POWER: i32 = 4;

/// Keeps weights finite for vertices sitting right on a bone (in cells).
const SOFTENING_CELLS: f32 = 1.5;

/// How far (in cells) a bone's seed point may move to reach a solid cell.
const SEED_SNAP_RADIUS: isize = 8;

const TIP_LENGTH_FRACTION: f32 = 0.08;
/// Geodesic voxel binding: measures how far each vertex is from each bone
/// *through the body* (not through the air), so two nearby limbs stay
/// independent. Vertices far from every bone keep a share of weight on a
/// fixed influence so a half-built skeleton doesn't drag the whole body.
///
/// Returns None if there are no bones, too many bones for the GPU, or
/// the mesh can't be voxelized; the caller should fall back to the naive
/// weights in that case.
pub fn compute_geodesic_weights(
    skeleton: &Skeleton,
    world_bind_matrices: &[glam::Mat4],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<Vec<VertexWeights>> {
    if skeleton.bones.is_empty() || skeleton.bones.len() >= GPU_BONE_SLOTS {
        return None;
    }
    let grid = VoxelGrid::from_mesh(positions, indices, GRID_RESOLUTION)?;

    let cell_size = grid.cell_size();
    let longest_side = cell_size * GRID_RESOLUTION as f32;
    let static_reach = STATIC_REACH_FRACTION * longest_side;
    let softening = SOFTENING_CELLS * cell_size;
    // Flood distances are in chamfer units: 10 per straight cell step.
    let max_distance = (MAX_REACH_MULTIPLE * static_reach / cell_size * 10.0).ceil() as u32;

    // A joint owns the segments running from it to each of its children
    // (a joint with no children owns just its own point). This matches how
    // posing works: rotating a joint swings everything below it, so the
    // mesh that should swing with it is the part leading AWAY from it.
    let joint_positions: Vec<Vec3> = world_bind_matrices
        .iter()
        .map(|matrix| matrix.transform_point3(Vec3::ZERO))
        .collect();
    let owned_segments: Vec<Vec<(Vec3, Vec3)>> = (0..skeleton.bones.len())
        .map(|joint| {
            let mut segments: Vec<(Vec3, Vec3)> = skeleton
                .bones
                .iter()
                .enumerate()
                .filter(|(_, bone)| bone.parent == Some(joint))
                .map(|(child, _)| (joint_positions[joint], joint_positions[child]))
                .collect();
            if segments.is_empty() {
                // A joint with no children owns its own point plus a tip
                // continuing the direction its parent bone was pointing.
                let tip = match skeleton.bones[joint].parent {
                    Some(parent) => {
                        let along = joint_positions[joint] - joint_positions[parent];
                        let length = along.length();
                        if length > 1e-6 {
                            let tip_length = length.max(TIP_LENGTH_FRACTION * longest_side);
                            joint_positions[joint] + along / length * tip_length
                        } else {
                            joint_positions[joint]
                        }
                    }
                    None => joint_positions[joint],
                };
                segments.push((joint_positions[joint], tip));
            }
                segments
        })
        .collect();
    let vertex_cells: Vec<Option<[usize; 3]>> = positions
        .iter()
        .map(|p| grid.cell_of(Vec3::from(*p)))
        .collect();

    // For every vertex: which bones reached it, and the distance in world units.
    let mut reached: Vec<Vec<(u32, f32)>> = vec![Vec::new(); positions.len()];

    let offsets = neighbor_offsets();
    let dims = grid.dims();
    let mut distances = vec![u32::MAX; dims[0] * dims[1] * dims[2]];

    for (bone_index, segments) in owned_segments.iter().enumerate() {
        let mut seeds: Vec<[usize; 3]> = Vec::new();
        for (head, tail) in segments {
            seeds.extend(segment_seeds(&grid, *head, *tail));
        }
        if seeds.is_empty() {
            continue;
        }
    flood_distances(&grid, &seeds, max_distance, &offsets, &mut distances);

        for (vertex_index, cell) in vertex_cells.iter().enumerate() {
            let Some(cell) = cell else {
                continue;
            };
            let d = distances[grid.index(*cell)];
            if d != u32::MAX {
                reached[vertex_index].push((bone_index as u32, d as f32 / 10.0 * cell_size));
            }
        }
    }

    let weights = reached
        .into_iter()
        .map(|mut candidates| {
            if candidates.is_empty() {
                candidates.push((STATIC_INFLUENCE_INDEX, static_reach));
            }            
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            candidates.truncate(INFLUENCES);

            let mut joint_indices = [STATIC_INFLUENCE_INDEX; INFLUENCES];
            let mut raw = [0.0f32; INFLUENCES];
            for (slot, (bone, distance)) in candidates.iter().enumerate() {
                joint_indices[slot] = *bone;
                raw[slot] = 1.0 / (*distance + softening).powi(FALLOFF_POWER);
            }

            let total: f32 = raw.iter().sum();
            let mut weights = [0.0f32; INFLUENCES];
            for i in 0..INFLUENCES {
                weights[i] = raw[i] / total;
            }

            VertexWeights {
                joint_indices,
                weights,
            }
        })
        .collect();

    Some(weights)
}

/// The 26 neighbors of a cell with integer chamfer costs
/// (10 = straight, 14 = face diagonal, 17 = corner diagonal).
fn neighbor_offsets() -> Vec<(isize, isize, isize, u32)> {
    let mut offsets = Vec::with_capacity(26);
    for dz in -1..=1isize {
        for dy in -1..=1isize {
            for dx in -1..=1isize {
                let steps = dx.abs() + dy.abs() + dz.abs();
                if steps == 0 {
                    continue;
                }
                let cost = match steps {
                    1 => 10,
                    2 => 14,
                    _ => 17,
                };
                offsets.push((dx, dy, dz, cost));
            }
        }
    }
    offsets
}

/// Solid cells along a bone's segment, sampled twice per cell. Points
/// that fall outside the body are moved to the nearest solid cell.
fn segment_seeds(grid: &VoxelGrid, head: Vec3, tail: Vec3) -> Vec<[usize; 3]> {
    let length = (tail - head).length();
    let steps = ((length / (grid.cell_size() * 0.5)).ceil() as usize).max(1);
    let mut seeds: Vec<[usize; 3]> = Vec::new();
    for step in 0..=steps {
        let point = head + (tail - head) * (step as f32 / steps as f32);
        if let Some(cell) = grid.cell_of(point) {
            if let Some(solid) = nearest_solid_cell(grid, cell) {
                if seeds.last() != Some(&solid) {
                    seeds.push(solid);
                }
            }
        }
    }
    seeds
}

fn nearest_solid_cell(grid: &VoxelGrid, cell: [usize; 3]) -> Option<[usize; 3]> {
    if grid.is_solid(cell) {
        return Some(cell);
    }
    let dims = grid.dims();
    for radius in 1..=SEED_SNAP_RADIUS {
        let mut best: Option<([usize; 3], isize)> = None;
        for dz in -radius..=radius {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()).max(dz.abs()) != radius {
                        continue;
                    }
                    let nx = cell[0] as isize + dx;
                    let ny = cell[1] as isize + dy;
                    let nz = cell[2] as isize + dz;
                    if nx < 0
                        || ny < 0
                        || nz < 0
                        || nx >= dims[0] as isize
                        || ny >= dims[1] as isize
                        || nz >= dims[2] as isize
                    {
                        continue;
                    }
                    let candidate = [nx as usize, ny as usize, nz as usize];
                    if !grid.is_solid(candidate) {
                        continue;
                    }
                    let distance_squared = dx * dx + dy * dy + dz * dz;
                    if best.map_or(true, |(_, d)| distance_squared < d) {
                        best = Some((candidate, distance_squared));
                    }
                }
            }
        }
        if let Some((found, _)) = best {
            return Some(found);
        }
    }
    None
}

/// Distance from the nearest seed to every solid cell reachable through
/// solid cells, stopping at `max_distance`. Chamfer units (10 per cell
/// step). Cells not reached stay `u32::MAX`.
fn flood_distances(
    grid: &VoxelGrid,
    seeds: &[[usize; 3]],
    max_distance: u32,
    offsets: &[(isize, isize, isize, u32)],
    distances: &mut [u32],
) {
    for d in distances.iter_mut() {
        *d = u32::MAX;
    }
    let dims = grid.dims();
    let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();

    for seed in seeds {
        let i = grid.index(*seed);
        if distances[i] != 0 {
            distances[i] = 0;
            heap.push(Reverse((0, i)));
        }
    }

    while let Some(Reverse((distance, i))) = heap.pop() {
        if distance > distances[i] {
            continue;
        }
        let x = i % dims[0];
        let y = (i / dims[0]) % dims[1];
        let z = i / (dims[0] * dims[1]);

        for &(dx, dy, dz, cost) in offsets {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            let nz = z as isize + dz;
            if nx < 0
                || ny < 0
                || nz < 0
                || nx >= dims[0] as isize
                || ny >= dims[1] as isize
                || nz >= dims[2] as isize
            {
                continue;
            }
            let neighbor = [nx as usize, ny as usize, nz as usize];
            if !grid.is_solid(neighbor) {
                continue;
            }
            let next = distance + cost;
            if next > max_distance {
                continue;
            }
            let ni = grid.index(neighbor);
            if next < distances[ni] {
                distances[ni] = next;
                heap.push(Reverse((next, ni)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed axis-aligned box as 8 vertices and 12 triangles.
    fn box_mesh(min: Vec3, max: Vec3) -> (Vec<[f32; 3]>, Vec<u32>) {
        let positions = vec![
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [max.x, max.y, min.z],
            [min.x, max.y, min.z],
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
        ];
        let indices = vec![
            0, 1, 2, 0, 2, 3, // -z
            4, 5, 6, 4, 6, 7, // +z
            0, 1, 5, 0, 5, 4, // -y
            3, 2, 6, 3, 6, 7, // +y
            0, 3, 7, 0, 7, 4, // -x
            1, 2, 6, 1, 6, 5, // +x
        ];
        (positions, indices)
    }

    /// Two thin legs (each 0.5 wide, 1.5 apart) joined by a pelvis block.
    fn two_leg_body() -> (Vec<[f32; 3]>, Vec<u32>) {
        let parts = [
            box_mesh(Vec3::new(-1.25, 0.0, -0.25), Vec3::new(-0.75, 6.0, 0.25)),
            box_mesh(Vec3::new(0.75, 0.0, -0.25), Vec3::new(1.25, 6.0, 0.25)),
            box_mesh(Vec3::new(-1.25, 6.0, -0.25), Vec3::new(1.25, 6.5, 0.25)),
        ];
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for (part_positions, part_indices) in parts.iter() {
            let offset = positions.len() as u32;
            positions.extend(part_positions.iter().copied());
            indices.extend(part_indices.iter().map(|i| i + offset));
        }
        (positions, indices)
    }

    /// Bones: 0 hip (root), 1 left knee, 2 left ankle, 3 right knee, 4 right ankle.
    fn two_leg_skeleton() -> (Skeleton, Vec<glam::Mat4>) {
        let mut skeleton = Skeleton::default();
        let hip = skeleton.add_bone(&String::from("hip"), None, Vec3::new(0.0, 6.25, 0.0));
        let left_knee = skeleton.add_bone(
            &String::from("left_knee"),
            Some(hip),
            Vec3::new(-1.0, 3.5 - 6.25, 0.0),
        );
        skeleton.add_bone(
            &String::from("left_ankle"),
            Some(left_knee),
            Vec3::new(0.0, 0.25 - 3.5, 0.0),
        );
        let right_knee = skeleton.add_bone(
            &String::from("right_knee"),
            Some(hip),
            Vec3::new(1.0, 3.5 - 6.25, 0.0),
        );
        skeleton.add_bone(
            &String::from("right_ankle"),
            Some(right_knee),
            Vec3::new(0.0, 0.25 - 3.5, 0.0),
        );
        let world = skeleton.world_bind_matrices();
        (skeleton, world)
    }

    #[test]
    fn lower_left_leg_ignores_the_right_leg_bones() {
        let (positions, indices) = two_leg_body();
        let (skeleton, world) = two_leg_skeleton();
        let weights = compute_geodesic_weights(&skeleton, &world, &positions, &indices).unwrap();

        let mut checked = 0;
        for (i, position) in positions.iter().enumerate() {
            // The four bottom corners of the left leg.
            if position[1] == 0.0 && position[0] < 0.0 {
                checked += 1;
                let w = &weights[i];
                let mut left_share = 0.0;
                for slot in 0..INFLUENCES {
                    let bone = w.joint_indices[slot];
                    assert!(
                        !(w.weights[slot] > 0.0 && (bone == 3 || bone == 4)),
                        "left foot vertex {i} leans on a right-leg bone"
                    );
                    if bone == 1 || bone == 2 {
                        left_share += w.weights[slot];
                    }
                }
                assert!(left_share > 0.6, "left foot vertex {i} left share was {left_share}");
            }
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn vertices_far_from_every_bone_stay_static() {
        let (positions, indices) = two_leg_body();
        let mut skeleton = Skeleton::default();
        skeleton.add_bone(&String::from("lone_root"), None, Vec3::new(-1.0, 0.25, 0.0));
        let world = skeleton.world_bind_matrices();
        let weights = compute_geodesic_weights(&skeleton, &world, &positions, &indices).unwrap();

        let mut checked = 0;
        for (i, position) in positions.iter().enumerate() {
            // The two top corners of the pelvis on the far (right) side.
            if position[1] == 6.5 && position[0] > 0.0 {
                checked += 1;
                assert_eq!(weights[i].joint_indices[0], STATIC_INFLUENCE_INDEX);
                assert!(weights[i].weights[0] > 0.999);
            }
        }
        assert_eq!(checked, 2);
    }

    /// Skins one vertex on the CPU the way the GPU does: the weighted sum of
    /// each influencing joint's skinning matrix applied to the vertex.
    fn skin_vertex(vertex: Vec3, weights: &VertexWeights, matrices: &[glam::Mat4]) -> Vec3 {
        let mut result = Vec3::ZERO;
        for slot in 0..INFLUENCES {
            let joint = weights.joint_indices[slot] as usize;
            let moved = match matrices.get(joint) {
                Some(matrix) => matrix.transform_point3(vertex),
                None => vertex, // the "stays put" influence
            };
            result += moved * weights.weights[slot];
        }
        result
    }

    #[test]
    fn bending_a_knee_moves_the_shin_and_leaves_the_thigh_alone() {
        // A thin bar standing in for a leg: hip at the top, knee in the
        // middle, ankle at the bottom.
        let (positions, indices) =
            box_mesh(Vec3::new(-0.05, -2.05, -0.05), Vec3::new(0.05, 0.05, 0.05));
        let mut skeleton = Skeleton::default();
        let hip = skeleton.add_bone("hip", None, Vec3::ZERO);
        let knee = skeleton.add_bone("knee", Some(hip), Vec3::new(0.0, -1.0, 0.0));
        skeleton.add_bone("ankle", Some(knee), Vec3::new(0.0, -1.0, 0.0));
        let world = skeleton.world_bind_matrices();
        let weights = compute_geodesic_weights(&skeleton, &world, &positions, &indices).unwrap();

        // Flex the knee 90 degrees: a joint's rotation swings what hangs below it.
        let quarter_turn = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut pose = crate::pose::Pose::rest_for(&skeleton);
        assert!(pose.set_rotation(1, quarter_turn));
        let matrices = pose.skinning_matrices(&skeleton);

        let knee_position = Vec3::new(0.0, -1.0, 0.0);
        for (i, position) in positions.iter().enumerate() {
            let rest = Vec3::from(*position);
            let posed = skin_vertex(rest, &weights[i], &matrices);
            if rest.y > 0.0 {
                // Top of the bar, up in the thigh: must stay where it was.
                assert!(
                    (posed - rest).length() < 0.05,
                    "thigh vertex {i} moved to {posed:?}"
                );
            } else {
                // Bottom of the bar, down in the shin: must swing around the knee.
                let ideal = knee_position + quarter_turn * (rest - knee_position);
                assert!(
                    (posed - ideal).length() < 0.1,
                    "shin vertex {i} ended at {posed:?}, expected about {ideal:?}"
                );
            }
        }
    }

    /// A measurement, not a pass/fail gate. Runs our geodesic weights on
    /// CesiumMan's OWN skeleton (so joint placement is identical) and
    /// compares them with the weights the file ships with. Run on demand:
    ///   cargo test weights_benchmark -- --ignored --nocapture
    #[test]
    #[ignore]
    fn weights_benchmark_against_the_files_own_weights() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/CesiumMan.glb");
        let model = crate::rig_import::load_gltf_rig(path, usize::MAX).unwrap();
        let skeleton = &model.skeleton;
        let world = skeleton.world_bind_matrices();

        let started = std::time::Instant::now();
        let ours = compute_geodesic_weights(
            skeleton,
            &world,
            &model.mesh.positions,
            &model.mesh.indices,
        )
        .unwrap();
        println!(
            "our weights: {} vertices, {} joints, {:?}",
            ours.len(),
            skeleton.bones.len(),
            started.elapsed()
        );
        let file = &model.vertex_weights;

        // One weight per joint, plus a last slot for "stays put".
        let joint_count = skeleton.bones.len();
        let dense = |w: &VertexWeights| -> Vec<f32> {
            let mut all = vec![0.0f32; joint_count + 1];
            for slot in 0..INFLUENCES {
                let joint = w.joint_indices[slot] as usize;
                let index = if joint < joint_count { joint } else { joint_count };
                all[index] += w.weights[slot];
            }
            all
        };
        let strongest = |all: &[f32]| -> usize {
            let mut best = 0;
            for (i, weight) in all.iter().enumerate() {
                if *weight > all[best] {
                    best = i;
                }
            }
            best
        };

        // 1. How often the strongest joint agrees, and how far apart the weights are overall.
        let mut same_strongest = 0usize;
        let mut total_difference = 0.0f32;
        for (o, f) in ours.iter().zip(file.iter()) {
            let (o, f) = (dense(o), dense(f));
            if strongest(&o) == strongest(&f) {
                same_strongest += 1;
            }
            total_difference +=
                o.iter().zip(f.iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() * 0.5;
        }
        let count = ours.len() as f32;
        println!(
            "strongest joint matches the file for {:.1}% of vertices",
            100.0 * same_strongest as f32 / count
        );
        println!(
            "average weight difference (0 = identical, 1 = completely different): {:.3}",
            total_difference / count
        );

        // 2. Weight leaking onto the opposite limb.
        let names: Vec<&str> = skeleton.bones.iter().map(|bone| bone.name.as_str()).collect();
        let in_group =
            |joint: usize, key: &str| joint < joint_count && names[joint].contains(key);
        for (label, own, other) in [
            ("left leg", "leg_joint_L", "leg_joint_R"),
            ("right leg", "leg_joint_R", "leg_joint_L"),
            ("left arm", "arm_joint_L", "arm_joint_R"),
            ("right arm", "arm_joint_R", "arm_joint_L"),
        ] {
            let mut vertices = 0usize;
            let mut leaked_sum = 0.0f32;
            let mut leaked_worst = 0.0f32;
            for (o, f) in ours.iter().zip(file.iter()) {
                if !in_group(strongest(&dense(f)), own) {
                    continue;
                }
                let ours_dense = dense(o);
                let leaked: f32 = (0..joint_count)
                    .filter(|&joint| in_group(joint, other))
                    .map(|joint| ours_dense[joint])
                    .sum();
                vertices += 1;
                leaked_sum += leaked;
                leaked_worst = leaked_worst.max(leaked);
            }
            println!(
                "{label}: {vertices} vertices, weight leaking onto the opposite side: average {:.3}, worst {:.3}",
                leaked_sum / vertices.max(1) as f32,
                leaked_worst
            );
        }

        // 3. What it looks like: flex each joint on its own, skin the mesh both
        // ways, and measure how far apart the results end up (model units; the
        // figure is about 1.5 tall).
        let positions: Vec<Vec3> = model.mesh.positions.iter().map(|p| Vec3::from(*p)).collect();
        let skin_all = |weights: &[VertexWeights], matrices: &[glam::Mat4]| -> Vec<Vec3> {
            positions
                .iter()
                .zip(weights.iter())
                .map(|(p, w)| skin_vertex(*p, w, matrices))
                .collect()
        };
        for (i, name) in names.iter().enumerate() {
            let mut pose = crate::pose::Pose::rest_for(skeleton);
            pose.set_rotation(i, glam::Quat::from_rotation_x(0.8));
            let matrices = pose.skinning_matrices(skeleton);
            let ours_posed = skin_all(ours.as_slice(), matrices.as_slice());
            let file_posed = skin_all(file.as_slice(), matrices.as_slice());
            let gaps: Vec<f32> = ours_posed
                .iter()
                .zip(file_posed.iter())
                .map(|(a, b)| (*a - *b).length())
                .collect();
            let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
            let worst = gaps.iter().cloned().fold(0.0f32, f32::max);
            let far = gaps.iter().filter(|gap| **gap > 0.02).count();
            println!(
                "flex {name:<30} mean {mean:.4}  worst {worst:.3}  vertices more than 0.02 off: {far}"
            );
        }
    }


    /// Where our weights disagree with the file's, and what happens if the
    /// "stays put" influence is removed. Run on demand:
    ///   cargo test weights_breakdown -- --ignored --nocapture
    #[test]
    #[ignore]
    fn weights_breakdown_by_joint_and_without_the_static_influence() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/CesiumMan.glb");
        let model = crate::rig_import::load_gltf_rig(path, usize::MAX).unwrap();
        let skeleton = &model.skeleton;
        let world = skeleton.world_bind_matrices();
        let ours = compute_geodesic_weights(
            skeleton,
            &world,
            &model.mesh.positions,
            &model.mesh.indices,
        )
        .unwrap();
        let file = &model.vertex_weights;

        let joint_count = skeleton.bones.len();
        let names: Vec<&str> = skeleton.bones.iter().map(|bone| bone.name.as_str()).collect();
        let name_of = |index: usize| -> String {
            if index < joint_count {
                names[index].to_string()
            } else {
                "(stays put)".to_string()
            }
        };
        let dense = |w: &VertexWeights| -> Vec<f32> {
            let mut all = vec![0.0f32; joint_count + 1];
            for slot in 0..INFLUENCES {
                let joint = w.joint_indices[slot] as usize;
                let index = if joint < joint_count { joint } else { joint_count };
                all[index] += w.weights[slot];
            }
            all
        };
        let strongest = |all: &[f32]| -> usize {
            let mut best = 0;
            for (i, weight) in all.iter().enumerate() {
                if *weight > all[best] {
                    best = i;
                }
            }
            best
        };

        // 1. How much of our weight sits on "stays put".
        let mut static_sum = 0.0f32;
        let mut mostly_static = 0usize;
        let mut fully_static = 0usize;
        for w in &ours {
            let share = dense(w)[joint_count];
            static_sum += share;
            if share > 0.5 {
                mostly_static += 1;
            }
            if share > 0.999 {
                fully_static += 1;
            }
        }
        println!(
            "weight on 'stays put': average {:.3} | vertices more than half static: {} | fully static (no joint reached them): {}",
            static_sum / ours.len() as f32,
            mostly_static,
            fully_static
        );

        // 2. For each joint the FILE considers strongest somewhere: how often we agree.
        for joint in 0..joint_count {
            let mut members = 0usize;
            let mut agree = 0usize;
            let mut static_members = 0usize;
            let mut tally = vec![0usize; joint_count + 1];
            for (o, f) in ours.iter().zip(file.iter()) {
                if strongest(&dense(f)) != joint {
                    continue;
                }
                members += 1;
                let ours_strongest = strongest(&dense(o));
                if ours_strongest == joint {
                    agree += 1;
                }
                if ours_strongest == joint_count {
                    static_members += 1;
                }
                tally[ours_strongest] += 1;
            }
            if members == 0 {
                continue;
            }
            let common = (0..=joint_count).max_by_key(|&j| tally[j]).unwrap();
            println!(
                "file's strongest = {:<28} {:>5} vertices | we agree {:>5.1}% | we say static {:>5.1}% | we most often pick {}",
                names[joint],
                members,
                100.0 * agree as f32 / members as f32,
                100.0 * static_members as f32 / members as f32,
                name_of(common)
            );
        }

        // 3. The same weights with "stays put" removed and the rest renormalized.
        let without_static: Vec<VertexWeights> = ours
            .iter()
            .map(|w| {
                let mut result = *w;
                let mut total = 0.0f32;
                for slot in 0..INFLUENCES {
                    if result.joint_indices[slot] as usize >= joint_count {
                        result.weights[slot] = 0.0;
                    }
                    total += result.weights[slot];
                }
                if total <= 1e-6 {
                    return *w;
                }
                for slot in 0..INFLUENCES {
                    result.weights[slot] /= total;
                }
                result
            })
            .collect();

        let positions: Vec<Vec3> = model.mesh.positions.iter().map(|p| Vec3::from(*p)).collect();
        let flex_mean = |set: &[VertexWeights], joint: usize| -> f32 {
            let mut pose = crate::pose::Pose::rest_for(skeleton);
            pose.set_rotation(joint, glam::Quat::from_rotation_x(0.8));
            let matrices = pose.skinning_matrices(skeleton);
            let mut sum = 0.0f32;
            for ((p, a), b) in positions.iter().zip(set.iter()).zip(file.iter()) {
                sum += (skin_vertex(*p, a, &matrices) - skin_vertex(*p, b, &matrices)).length();
            }
            sum / positions.len() as f32
        };
        let spine_joints: Vec<usize> = (0..joint_count)
            .filter(|&j| names[j].contains("torso") || names[j].contains("neck"))
            .collect();
        let limb_joints: Vec<usize> = (0..joint_count)
            .filter(|j| !spine_joints.contains(j))
            .collect();

        for (label, set) in [
            ("ours", &ours),
            ("ours without 'stays put'", &without_static),
        ] {
            let mut same = 0usize;
            let mut difference = 0.0f32;
            for (o, f) in set.iter().zip(file.iter()) {
                let (o, f) = (dense(o), dense(f));
                if strongest(&o) == strongest(&f) {
                    same += 1;
                }
                difference += o.iter().zip(f.iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() * 0.5;
            }
            let count = set.len() as f32;
            println!(
                "{label}: strongest joint matches {:.1}% | average weight difference {:.3}",
                100.0 * same as f32 / count,
                difference / count
            );
            let spine: Vec<String> = spine_joints
                .iter()
                .map(|&j| format!("{} {:.4}", names[j], flex_mean(set, j)))
                .collect();
            println!("  flex mean error, spine and neck: {}", spine.join(" | "));
            let limb_total: f32 = limb_joints.iter().map(|&j| flex_mean(set, j)).sum();
            println!(
                "  flex mean error, all arm and leg joints averaged: {:.4}",
                limb_total / limb_joints.len() as f32
            );
        }
    }

    #[test]
    fn the_part_of_the_body_beyond_a_last_joint_follows_that_joint() {
        // A bar that runs 0.65 past the ankle, like a foot past an ankle or
        // a head above a neck.
        let (positions, indices) =
            box_mesh(Vec3::new(-0.05, -2.65, -0.05), Vec3::new(0.05, 0.05, 0.05));
        let mut skeleton = Skeleton::default();
        let hip = skeleton.add_bone("hip", None, Vec3::ZERO);
        let knee = skeleton.add_bone("knee", Some(hip), Vec3::new(0.0, -1.0, 0.0));
        skeleton.add_bone("ankle", Some(knee), Vec3::new(0.0, -1.0, 0.0));
        let world = skeleton.world_bind_matrices();
        let weights = compute_geodesic_weights(&skeleton, &world, &positions, &indices).unwrap();

        // Flex only the ankle (the last joint) by 90 degrees.
        let quarter_turn = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut pose = crate::pose::Pose::rest_for(&skeleton);
        assert!(pose.set_rotation(2, quarter_turn));
        let matrices = pose.skinning_matrices(&skeleton);

        // Everything past the ankle must swing around it.
        let ankle_position = Vec3::new(0.0, -2.0, 0.0);
        for (i, position) in positions.iter().enumerate() {
            let rest = Vec3::from(*position);
            if rest.y > -2.0 {
                continue;
            }
            let posed = skin_vertex(rest, &weights[i], &matrices);
            let ideal = ankle_position + quarter_turn * (rest - ankle_position);
            assert!(
                (posed - ideal).length() < 0.1,
                "vertex {i} beyond the ankle ended at {posed:?}, expected about {ideal:?}"
            );
        }
    }

    #[test]
    fn no_bones_gives_none() {
    let (positions, indices) = two_leg_body();
        let skeleton = Skeleton::default();
        assert!(compute_geodesic_weights(&skeleton, &[], &positions, &indices).is_none());
    }
}