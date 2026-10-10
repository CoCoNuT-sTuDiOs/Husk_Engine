use serde::{Deserialize, Serialize};

use crate::geodesic::STATIC_INFLUENCE_INDEX;
use crate::keyframes::Timeline;
use crate::rig::{Bone, Skeleton};
use crate::skinning::VertexWeights;

/// Bump this when the file layout changes, so older or newer files are
/// refused with a clear message instead of being misread.
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct BoneData {
    name: String,
    parent: Option<usize>,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct WeightData {
    joints: [u32; 4],
    weights: [f32; 4],
}

#[derive(Serialize, Deserialize)]
struct KeyData {
    time: f32,
    rotations: Vec<[f32; 4]>,
    offset: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct ProjectData {
    version: u32,
    vertex_count: usize,
    bones: Vec<BoneData>,
    weights: Option<Vec<WeightData>>,
    keys: Vec<KeyData>,
    cycle_period: Option<f32>,
    travel_speed: f32,
    travel_velocity: [f32; 3],
}

/// Everything read back from a project file.
pub struct Project {
    pub vertex_count: usize,
    pub skeleton: Skeleton,
    pub weights: Option<Vec<VertexWeights>>,
    pub timeline: Timeline,
}

/// Writes the skeleton, skin weights and animation to `path` as JSON.
pub fn write_project(
    path: &str,
    vertex_count: usize,
    skeleton: &Skeleton,
    weights: Option<&[VertexWeights]>,
    timeline: &Timeline,
) -> Result<(), String> {
    let data = ProjectData {
        version: FORMAT_VERSION,
        vertex_count,
        bones: skeleton
            .bones
            .iter()
            .map(|bone| BoneData {
                name: bone.name.clone(),
                parent: bone.parent,
                position: bone.local_position.to_array(),
                rotation: bone.local_rotation.to_array(),
                scale: bone.local_scale.to_array(),
            })
            .collect(),
        weights: weights.map(|all| {
            all.iter()
                .map(|w| WeightData {
                    joints: w.joint_indices,
                    weights: w.weights,
                })
                .collect()
        }),
        keys: timeline
            .keys
            .iter()
            .map(|key| KeyData {
                time: key.time,
                rotations: key.rotations.iter().map(|q| q.to_array()).collect(),
                offset: key.offset.to_array(),
            })
            .collect(),
        cycle_period: timeline.cycle_period,
        travel_speed: timeline.travel_speed,
        travel_velocity: timeline.travel_velocity.to_array(),
    };
    let text = serde_json::to_string(&data)
        .map_err(|e| format!("Could not encode the project: {e}"))?;
    std::fs::write(path, text).map_err(|e| format!("Could not write {path}: {e}"))
}

fn finite3(values: [f32; 3]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// A normalized rotation, or None if the numbers are invalid.
fn unit_quat(values: [f32; 4]) -> Option<glam::Quat> {
    if !values.iter().all(|v| v.is_finite()) {
        return None;
    }
    let q = glam::Quat::from_array(values);
    if q.length_squared() < 1e-12 {
        None
    } else {
        Some(q.normalize())
    }
}

/// Reads a project file, checking it carefully so a damaged or mismatched
/// file is refused with a reason instead of corrupting the scene.
pub fn read_project(path: &str) -> Result<Project, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("Could not read {path}: {e}"))?;
    let data: ProjectData = serde_json::from_str(&text)
        .map_err(|e| format!("This is not a Husk project file: {e}"))?;
    if data.version != FORMAT_VERSION {
        return Err(format!(
            "This project uses file format version {}, but this build reads version {FORMAT_VERSION}",
            data.version
        ));
    }

    // Skeleton: parents must come before their children.
    let bone_count = data.bones.len();
    let mut skeleton = Skeleton::default();
    for (i, bone) in data.bones.iter().enumerate() {
        if let Some(parent) = bone.parent {
            if parent >= i {
                return Err(format!(
                    "Bone {i} ('{}') is listed before its parent {parent}",
                    bone.name
                ));
            }
        }
        let Some(rotation) = unit_quat(bone.rotation) else {
            return Err(format!("Bone {i} ('{}') has an invalid rotation", bone.name));
        };
        if !finite3(bone.position) || !finite3(bone.scale) {
            return Err(format!("Bone {i} ('{}') has an invalid number", bone.name));
        }
        skeleton.bones.push(Bone {
            name: bone.name.clone(),
            parent: bone.parent,
            local_position: glam::Vec3::from_array(bone.position),
            local_rotation: rotation,
            local_scale: glam::Vec3::from_array(bone.scale),
        });
    }

    // Skin weights: one set per vertex, pointing at real bones (or at the
    // reserved "stays put" slot).
    let weights = match &data.weights {
        Some(all) => {
            if all.len() != data.vertex_count {
                return Err(format!(
                    "The project has {} sets of skin weights for {} vertices",
                    all.len(),
                    data.vertex_count
                ));
            }
            let mut converted = Vec::with_capacity(all.len());
            for (i, w) in all.iter().enumerate() {
                for slot in 0..4 {
                    if !w.weights[slot].is_finite() {
                        return Err(format!("Vertex {i} has an invalid weight"));
                    }
                    let joint = w.joints[slot];
                    if w.weights[slot] > 0.0
                        && (joint as usize) >= bone_count
                        && joint != STATIC_INFLUENCE_INDEX
                    {
                        return Err(format!(
                            "Vertex {i} is weighted to bone {joint}, but the project only has {bone_count} bones"
                        ));
                    }
                }
                converted.push(VertexWeights {
                    joint_indices: w.joints,
                    weights: w.weights,
                });
            }
            Some(converted)
        }
        None => None,
    };

    // Animation.
    if let Some(period) = data.cycle_period {
        if !period.is_finite() || period <= 0.0 {
            return Err("The repeat length is invalid".to_string());
        }
    }
    if !data.travel_speed.is_finite() || !finite3(data.travel_velocity) {
        return Err("The travel speed is invalid".to_string());
    }
    let mut timeline = Timeline::default();
    for (i, key) in data.keys.iter().enumerate() {
        if !key.time.is_finite() || !finite3(key.offset) {
            return Err(format!("Key {i} has an invalid time or position"));
        }
        let mut rotations = Vec::with_capacity(key.rotations.len());
        for values in &key.rotations {
            let Some(q) = unit_quat(*values) else {
                return Err(format!("Key {i} has an invalid rotation"));
            };
            rotations.push(q);
        }
        timeline.set_key_with_offset(key.time, rotations, glam::Vec3::from_array(key.offset));
    }
    timeline.cycle_period = data.cycle_period;
    timeline.travel_speed = data.travel_speed;
    timeline.travel_velocity = glam::Vec3::from_array(data.travel_velocity);

    Ok(Project {
        vertex_count: data.vertex_count,
        skeleton,
        weights,
        timeline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn temp_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("husk_project_test_{}_{}.json", std::process::id(), name))
            .to_string_lossy()
            .to_string()
    }

    fn sample_skeleton() -> Skeleton {
        let mut skeleton = Skeleton::default();
        let hip = skeleton.add_bone("hip", None, Vec3::new(0.0, 1.0, 0.0));
        skeleton.add_bone("knee", Some(hip), Vec3::new(0.0, -0.5, 0.1));
        skeleton
    }

    fn sample_weights() -> Vec<VertexWeights> {
        vec![
            VertexWeights {
                joint_indices: [0, 1, STATIC_INFLUENCE_INDEX, STATIC_INFLUENCE_INDEX],
                weights: [0.5, 0.25, 0.25, 0.0],
            },
            VertexWeights {
                joint_indices: [1, 0, STATIC_INFLUENCE_INDEX, STATIC_INFLUENCE_INDEX],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            VertexWeights {
                joint_indices: [STATIC_INFLUENCE_INDEX; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ]
    }

    fn sample_timeline() -> Timeline {
        let mut timeline = Timeline::default();
        timeline.set_key_with_offset(
            0.0,
            vec![Quat::IDENTITY, Quat::from_rotation_z(0.5)],
            Vec3::ZERO,
        );
        timeline.set_key_with_offset(
            0.5,
            vec![Quat::from_rotation_z(0.2), Quat::from_rotation_z(1.0)],
            Vec3::new(0.5, 0.0, 0.0),
        );
        assert!(timeline.set_cycle_end(1.0));
        timeline.travel_speed = 0.7;
        timeline.travel_velocity = Vec3::new(0.7, 0.0, 0.0);
        timeline
    }

    #[test]
    fn a_saved_project_loads_back_the_same() {
        let path = temp_path("round_trip");
        let timeline = sample_timeline();
        write_project(&path, 3, &sample_skeleton(), Some(&sample_weights()), &timeline).unwrap();
        let loaded = read_project(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded.vertex_count, 3);
        assert_eq!(loaded.skeleton.bones.len(), 2);
        assert_eq!(loaded.skeleton.bones[1].name, "knee");
        assert_eq!(loaded.skeleton.bones[1].parent, Some(0));
        assert!(
            (loaded.skeleton.bones[1].local_position - Vec3::new(0.0, -0.5, 0.1)).length() < 1e-6
        );

        let weights = loaded.weights.unwrap();
        assert_eq!(weights.len(), 3);
        assert_eq!(
            weights[0].joint_indices,
            [0, 1, STATIC_INFLUENCE_INDEX, STATIC_INFLUENCE_INDEX]
        );
        assert!((weights[0].weights[1] - 0.25).abs() < 1e-6);

        assert_eq!(loaded.timeline.keys.len(), 2);
        assert_eq!(loaded.timeline.effective_period(), Some(1.0));
        assert!((loaded.timeline.travel_speed - 0.7).abs() < 1e-6);

        // The loaded animation poses and moves the character exactly like the original.
        for time in [0.0, 0.25, 0.5, 0.8, 1.3] {
            let before = timeline.sample(time, 2).unwrap();
            let after = loaded.timeline.sample(time, 2).unwrap();
            for (a, b) in before.iter().zip(after.iter()) {
                assert!(a.dot(*b).abs() > 0.99999);
            }
            let moved = timeline.sample_offset(time).unwrap();
            let moved_after = loaded.timeline.sample_offset(time).unwrap();
            assert!((moved - moved_after).length() < 1e-5);
        }
    }

    fn valid_data() -> ProjectData {
        ProjectData {
            version: FORMAT_VERSION,
            vertex_count: 3,
            bones: vec![
                BoneData {
                    name: "hip".into(),
                    parent: None,
                    position: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                BoneData {
                    name: "knee".into(),
                    parent: Some(0),
                    position: [0.0, -1.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
            ],
            weights: None,
            keys: vec![],
            cycle_period: None,
            travel_speed: 0.0,
            travel_velocity: [0.0; 3],
        }
    }

    fn read_back(name: &str, data: &ProjectData) -> Result<Project, String> {
        let path = temp_path(name);
        std::fs::write(&path, serde_json::to_string(data).unwrap()).unwrap();
        let result = read_project(&path);
        std::fs::remove_file(&path).ok();
        result
    }

    #[test]
    fn a_project_without_weights_loads() {
        assert!(read_back("valid", &valid_data()).unwrap().weights.is_none());
    }

    #[test]
    fn a_bone_listed_before_its_parent_is_refused() {
        let mut data = valid_data();
        data.bones[0].parent = Some(1);
        assert!(read_back("order", &data).is_err());
    }

    #[test]
    fn the_wrong_file_version_is_refused() {
        let mut data = valid_data();
        data.version = FORMAT_VERSION + 1;
        assert!(read_back("version", &data).is_err());
    }

    #[test]
    fn weights_for_the_wrong_number_of_vertices_are_refused() {
        let mut data = valid_data(); // says 3 vertices
        data.weights = Some(vec![WeightData {
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }]);
        assert!(read_back("count", &data).is_err());
    }

    #[test]
    fn a_weight_pointing_at_a_missing_bone_is_refused() {
        let mut data = valid_data();
        data.vertex_count = 1;
        data.weights = Some(vec![WeightData {
            joints: [5, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }]);
        assert!(read_back("missing_bone", &data).is_err());
    }

    #[test]
    fn missing_files_and_garbage_are_refused() {
        assert!(read_project("this/file/does/not/exist.json").is_err());
        let path = temp_path("garbage");
        std::fs::write(&path, "not json at all").unwrap();
        assert!(read_project(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}