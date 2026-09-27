use crate::rig::Skeleton;

const MAX_INFLUENCES: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct VertexWeights {
    pub joint_indices: [u32; MAX_INFLUENCES],
    pub weights: [f32; MAX_INFLUENCES],
}

pub fn compute_skin_weights(
    skeleton: &Skeleton,
    world_bind_matrices: &[glam::Mat4],
    positions: &[[f32; 3]],
) -> Vec<VertexWeights> {
    let segments: Vec<(glam::Vec3, glam::Vec3)> = skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            let tail = world_bind_matrices[i].transform_point3(glam::Vec3::ZERO);
            let head = match bone.parent {
                Some(parent_index) => {
                    world_bind_matrices[parent_index].transform_point3(glam::Vec3::ZERO)
                }
                None => tail,
            };
            (head, tail)
        })
        .collect();

    positions
        .iter()
        .map(|position| {
            let vertex = glam::Vec3::from(*position);

            let mut distances: Vec<(usize, f32)> = segments
                .iter()
                .enumerate()
                .map(|(bone_index, (head, tail))| {
                    (bone_index, distance_to_segment(vertex, *head, *tail))
                })
                .collect();

            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            distances.truncate(MAX_INFLUENCES);

            let mut joint_indices = [0u32; MAX_INFLUENCES];
            let mut raw_weights = [0.0f32; MAX_INFLUENCES];

            for (slot, (bone_index, distance)) in distances.iter().enumerate() {
                joint_indices[slot] = *bone_index as u32;
                raw_weights[slot] = 1.0 / (distance * distance + 0.0001);
            }

            let total: f32 = raw_weights.iter().sum();
            let mut weights = [0.0f32; MAX_INFLUENCES];
            if total > 0.0 {
                for i in 0..MAX_INFLUENCES {
                    weights[i] = raw_weights[i] / total;
                }
            }

            VertexWeights {
                joint_indices,
                weights,
            }
        })
        .collect()
}

/// Shortest distance from `point` to the line segment between `a` and `b`.
fn distance_to_segment(point: glam::Vec3, a: glam::Vec3, b: glam::Vec3) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_squared();

    if length_squared < 1e-8 {
        return (point - a).length(); // Degenerate (zero-length) segment.
    }

    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (point - closest).length()
}