//! Editing a skeleton that already exists: moving a joint and deleting a bone,
//! and keeping everything that counts bones by number right afterwards.
//!
//! Modelled on Blender's Delete Selected Bone(s) (`ARMATURE_OT_delete`,
//! `armature_delete_selected_exec` in armature_edit.cc: it removes the bones and
//! their pose channels and forgets the active bone). No Blender code is copied.
//! Husk's bones are joints that carry their skin weights by number, so Husk has
//! rules of its own:
//! - the children of a deleted bone move up to its parent and stay where they are
//!   in space;
//! - its skin weights go to its parent (a root has none: they go to the influence
//!   that stays put, so nothing collapses to the origin);
//! - things that followed it follow its parent.
//! The child rule matches Blender's `ED_armature_ebone_remove_ex` (armature_edit.cc),
//! which the delete operator calls: the children of a deleted bone take its parent.


use glam::{Mat4, Quat, Vec3};

use crate::rig::geodesic::STATIC_INFLUENCE_INDEX;
use crate::rig::skinning::VertexWeights;
use crate::rig::Skeleton;

/// What deleting a bone did to the numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removal {
    /// The number the deleted bone had.
    pub removed: usize,
    /// The bone it hung below, which takes over its children, weights and
    /// followers. None for a root.
    pub parent: Option<usize>,
}

impl Removal {
    /// The number a bone has now that it had `old` before: bones after the
    /// deleted one move down by one. None for the deleted bone itself.
    pub fn renumber(&self, old: usize) -> Option<usize> {
        if old == self.removed {
            None
        } else if old > self.removed {
            Some(old - 1)
        } else {
            Some(old)
        }
    }

    /// The bone something that followed bone `old` follows now: the same bone
    /// under its new number, or for the deleted bone its parent (None when it
    /// had none, so the thing follows the whole object).
    pub fn follow(&self, old: usize) -> Option<usize> {
        self.renumber(old).or(self.parent)
    }
}

/// Moves joint `index` to `position`, in the skeleton's own space, in the rest
/// pose. Its children stay where they are in space, so only this joint and the
/// bones that lead to it change. False, changing nothing, for an unknown joint
/// or a position that is not a number.
pub fn move_joint(skeleton: &mut Skeleton, index: usize, position: Vec3) -> bool {
    if index >= skeleton.bones.len() || !position.is_finite() {
        return false;
    }
    let world = skeleton.world_bind_matrices();
    let parent_world = match skeleton.bones[index].parent {
        Some(parent) => world[parent],
        None => Mat4::IDENTITY,
    };
    let local = parent_world.inverse().transform_point3(position);
    let moved = Mat4::from_scale_rotation_translation(
        skeleton.bones[index].local_scale,
        skeleton.bones[index].local_rotation,
        local,
    );
    let to_local = (parent_world * moved).inverse();
    if !local.is_finite() || !to_local.is_finite() {
        return false;
    }
    // Each child keeps its place in space: its position is worked out again
    // below the moved joint. Its own turn and scale are left as they were.
    let children: Vec<(usize, Vec3)> = (0..skeleton.bones.len())
        .filter(|&child| skeleton.bones[child].parent == Some(index))
        .map(|child| {
            let at = world[child].transform_point3(Vec3::ZERO);
            (child, to_local.transform_point3(at))
        })
        .collect();
    skeleton.bones[index].local_position = local;
    for (child, at) in children {
        skeleton.bones[child].local_position = at;
    }
    true
}

/// Deletes bone `index`. Its children hang below its parent and stay where they
/// are in space, and the bones after it move down one place. None, changing
/// nothing, for an unknown bone.
///
/// Whatever else counts bones by number (skin weights, the pose, animation keys,
/// objects that follow a bone) must be brought into step with the returned
/// `Removal`.
pub fn remove_bone(skeleton: &mut Skeleton, index: usize) -> Option<Removal> {
    if index >= skeleton.bones.len() {
        return None;
    }
    let world = skeleton.world_bind_matrices();
    let parent = skeleton.bones[index].parent;
    let parent_world = match parent {
        Some(parent) => world[parent],
        None => Mat4::IDENTITY,
    };
    let to_local = parent_world.inverse();
    let removed = skeleton.bones[index].clone();
    // A bone that is only moved (no turn, no scale) hands its children a plain
    // addition, which keeps their numbers exact.
    let plain = removed.local_rotation == Quat::IDENTITY && removed.local_scale == Vec3::ONE;

    for child in 0..skeleton.bones.len() {
        if skeleton.bones[child].parent != Some(index) {
            continue;
        }
        let bone = &mut skeleton.bones[child];
        bone.parent = parent;
        if plain {
            bone.local_position += removed.local_position;
        } else {
            let local = to_local * world[child];
            if local.is_finite() {
                let (scale, rotation, translation) = local.to_scale_rotation_translation();
                bone.local_position = translation;
                bone.local_rotation = rotation.normalize();
                bone.local_scale = scale;
            }
        }
    }

    skeleton.bones.remove(index);
    let removal = Removal {
        removed: index,
        parent,
    };
    for bone in skeleton.bones.iter_mut() {
        bone.parent = bone.parent.and_then(|parent| removal.renumber(parent));
    }
    Some(removal)
}

/// Brings skin weights into step with a deleted bone: its weight goes to its
/// parent (or to the influence that stays put when it had none), and the bones
/// after it are renumbered. Every vertex keeps the total weight it had.
pub fn remap_weights(weights: &mut [VertexWeights], removal: &Removal) {
    let target = match removal.parent {
        Some(parent) => parent as u32,
        None => STATIC_INFLUENCE_INDEX,
    };
    let removed = removal.removed as u32;
    for vertex in weights.iter_mut() {
        for slot in 0..4 {
            if vertex.weights[slot] > 0.0 && vertex.joint_indices[slot] == removed {
                let moved = vertex.weights[slot];
                let existing = (0..4).find(|&other| {
                    other != slot
                        && vertex.weights[other] > 0.0
                        && vertex.joint_indices[other] == target
                });
                match existing {
                    Some(other) => {
                        vertex.weights[other] += moved;
                        vertex.weights[slot] = 0.0;
                    }
                    None => vertex.joint_indices[slot] = target,
                }
            }
        }
        for slot in 0..4 {
            if vertex.weights[slot] > 0.0 {
                let joint = vertex.joint_indices[slot];
                if joint != STATIC_INFLUENCE_INDEX && joint > removed {
                    vertex.joint_indices[slot] = joint - 1;
                }
            } else {
                vertex.joint_indices[slot] = 0;
                vertex.weights[slot] = 0.0;
            }
        }
    }
}

/// Takes the deleted bone's turn out of a list with one turn per bone (the pose,
/// or one animation key).
pub fn remap_rotations(rotations: &mut Vec<Quat>, removal: &Removal) {
    if removal.removed < rotations.len() {
        rotations.remove(removal.removed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    /// 0 root, 1 a (above root), 2 b (above a), 3 side (beside root), 4 tip
    /// (above b). In space: a (0,1,0), b (0,2,0), side (1,0,0), tip (0,3,0).
    fn tree() -> Skeleton {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let a = skeleton.add_bone("a", Some(root), Vec3::new(0.0, 1.0, 0.0));
        let b = skeleton.add_bone("b", Some(a), Vec3::new(0.0, 1.0, 0.0));
        skeleton.add_bone("side", Some(root), Vec3::new(1.0, 0.0, 0.0));
        skeleton.add_bone("tip", Some(b), Vec3::new(0.0, 1.0, 0.0));
        skeleton
    }

    /// Where the bone with this name is in space, at rest.
    fn at(skeleton: &Skeleton, name: &str) -> Vec3 {
        let index = skeleton
            .bones
            .iter()
            .position(|bone| bone.name == name)
            .expect(name);
        skeleton.world_bind_matrices()[index].transform_point3(Vec3::ZERO)
    }

    fn names(skeleton: &Skeleton) -> Vec<&str> {
        skeleton.bones.iter().map(|bone| bone.name.as_str()).collect()
    }

    fn parents(skeleton: &Skeleton) -> Vec<Option<usize>> {
        skeleton.bones.iter().map(|bone| bone.parent).collect()
    }

    fn total(vertex: &VertexWeights) -> f32 {
        vertex.weights.iter().sum()
    }

    /// How much of the vertex's weight sits on `bone`.
    fn on(vertex: &VertexWeights, bone: u32) -> f32 {
        (0..4)
            .filter(|&slot| vertex.joint_indices[slot] == bone && vertex.weights[slot] > 0.0)
            .map(|slot| vertex.weights[slot])
            .sum()
    }

    #[test]
    fn moving_a_joint_moves_only_it_and_its_children_stay_where_they_are() {
        let mut skeleton = tree();
        assert!(move_joint(&mut skeleton, 1, Vec3::new(0.5, 1.5, 0.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.5, 1.5, 0.0)));
        // Below it, nothing moved in space.
        assert!(close(at(&skeleton, "b"), Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
        // Beside and above it, nothing moved either.
        assert!(close(at(&skeleton, "root"), Vec3::ZERO));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn moving_the_root_leaves_every_other_joint_in_place() {
        let mut skeleton = tree();
        assert!(move_joint(&mut skeleton, 0, Vec3::new(1.0, 2.0, 3.0)));
        assert!(close(at(&skeleton, "root"), Vec3::new(1.0, 2.0, 3.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn an_unknown_joint_or_a_position_that_is_not_a_number_moves_nothing() {
        let mut skeleton = tree();
        assert!(!move_joint(&mut skeleton, 99, Vec3::ONE));
        assert!(!move_joint(&mut skeleton, 1, Vec3::new(f32::NAN, 0.0, 0.0)));
        assert!(!move_joint(&mut skeleton, 1, Vec3::new(f32::INFINITY, 0.0, 0.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn deleting_a_middle_bone_hands_its_children_to_its_parent_in_place() {
        let mut skeleton = tree();
        let removal = remove_bone(&mut skeleton, 1).expect("bone 1 exists");
        assert_eq!(
            removal,
            Removal {
                removed: 1,
                parent: Some(0)
            }
        );
        assert_eq!(names(&skeleton), vec!["root", "b", "side", "tip"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0), Some(0), Some(1)]);
        assert!(close(at(&skeleton, "b"), Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn deleting_a_turned_bone_keeps_its_children_in_place_and_turned() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let a = skeleton.add_bone("a", Some(root), Vec3::new(0.0, 1.0, 0.0));
        skeleton.bones[a].local_rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        skeleton.add_bone("c", Some(a), Vec3::new(1.0, 0.0, 0.0));
        // Turned a quarter, a carries c from one unit along its own X to (0, 2, 0).
        assert!(close(at(&skeleton, "c"), Vec3::new(0.0, 2.0, 0.0)));
        let turned_before = skeleton.world_bind_matrices()[2].transform_vector3(Vec3::X);

        assert!(remove_bone(&mut skeleton, a).is_some());
        assert_eq!(names(&skeleton), vec!["root", "c"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0)]);
        assert!(close(at(&skeleton, "c"), Vec3::new(0.0, 2.0, 0.0)));
        let turned_after = skeleton.world_bind_matrices()[1].transform_vector3(Vec3::X);
        assert!(close(turned_before, turned_after));
    }

    #[test]
    fn deleting_the_root_makes_its_children_roots_in_place() {
        let mut skeleton = tree();
        let removal = remove_bone(&mut skeleton, 0).expect("bone 0 exists");
        assert_eq!(
            removal,
            Removal {
                removed: 0,
                parent: None
            }
        );
        assert_eq!(names(&skeleton), vec!["a", "b", "side", "tip"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0), None, Some(1)]);
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn deleting_a_bone_that_does_not_exist_changes_nothing() {
        let mut skeleton = tree();
        assert!(remove_bone(&mut skeleton, 5).is_none());
        assert_eq!(skeleton.bones.len(), 5);
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn the_numbering_follows_the_deleted_bone_and_the_turns_lose_its_slot() {
        let removal = Removal {
            removed: 2,
            parent: Some(1),
        };
        assert_eq!(removal.renumber(0), Some(0));
        assert_eq!(removal.renumber(2), None);
        assert_eq!(removal.renumber(3), Some(2));
        assert_eq!(removal.follow(2), Some(1));
        assert_eq!(removal.follow(3), Some(2));
        assert_eq!(removal.follow(0), Some(0));

        let root = Removal {
            removed: 0,
            parent: None,
        };
        assert_eq!(root.follow(0), None);
        assert_eq!(root.follow(2), Some(1));

        let mut rotations = vec![
            Quat::IDENTITY,
            Quat::from_rotation_x(1.0),
            Quat::from_rotation_y(1.0),
        ];
        remap_rotations(
            &mut rotations,
            &Removal {
                removed: 1,
                parent: Some(0),
            },
        );
        assert_eq!(rotations, vec![Quat::IDENTITY, Quat::from_rotation_y(1.0)]);
        // A list that is shorter than the bone numbers is left alone.
        let mut short = vec![Quat::IDENTITY];
        remap_rotations(
            &mut short,
            &Removal {
                removed: 5,
                parent: Some(0),
            },
        );
        assert_eq!(short.len(), 1);
    }

    #[test]
    fn the_weight_of_a_deleted_bone_goes_to_its_parent_and_the_total_never_changes() {
        let removal = Removal {
            removed: 1,
            parent: Some(0),
        };
        let mut vertices = vec![
            // Bone 1 with bone 2: bone 1's share goes to bone 0.
            VertexWeights {
                joint_indices: [1, 2, 0, 0],
                weights: [0.6, 0.4, 0.0, 0.0],
            },
            // Bone 0 is already there: the two shares are added.
            VertexWeights {
                joint_indices: [0, 1, 2, 0],
                weights: [0.3, 0.5, 0.2, 0.0],
            },
            // All four slots full: bone 1's slot is the one that is freed.
            VertexWeights {
                joint_indices: [1, 2, 3, 4],
                weights: [0.25, 0.25, 0.25, 0.25],
            },
            // Nothing to do with bone 1.
            VertexWeights {
                joint_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ];
        remap_weights(&mut vertices, &removal);

        assert!((on(&vertices[0], 0) - 0.6).abs() < 1e-6);
        assert!((on(&vertices[0], 1) - 0.4).abs() < 1e-6);

        assert!((on(&vertices[1], 0) - 0.8).abs() < 1e-6);
        assert!((on(&vertices[1], 1) - 0.2).abs() < 1e-6);

        for (bone, share) in [(0u32, 0.25f32), (1, 0.25), (2, 0.25), (3, 0.25)] {
            assert!((on(&vertices[2], bone) - share).abs() < 1e-6);
        }

        assert!((on(&vertices[3], 0) - 1.0).abs() < 1e-6);
        for vertex in &vertices {
            assert!((total(vertex) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_weight_of_a_deleted_root_goes_to_the_influence_that_stays_put() {
        let removal = Removal {
            removed: 0,
            parent: None,
        };
        let mut vertices = vec![
            VertexWeights {
                joint_indices: [0, 2, 0, 0],
                weights: [0.7, 0.3, 0.0, 0.0],
            },
            // The static influence is already there: the shares are added.
            VertexWeights {
                joint_indices: [0, STATIC_INFLUENCE_INDEX, 0, 0],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
        ];
        remap_weights(&mut vertices, &removal);

        assert!((on(&vertices[0], STATIC_INFLUENCE_INDEX) - 0.7).abs() < 1e-6);
        assert!((on(&vertices[0], 1) - 0.3).abs() < 1e-6);
        assert!((on(&vertices[1], STATIC_INFLUENCE_INDEX) - 1.0).abs() < 1e-6);
        for vertex in &vertices {
            assert!((total(vertex) - 1.0).abs() < 1e-6);
        }
    }
}