use crate::rig::Skeleton;

#[derive(Debug, Clone, Default)]
pub struct Pose {
    pub rotations: Vec<glam::Quat>,
}

impl Pose {
    /// A rest pose (identity offset for every bone) sized for `skeleton`.
    pub fn rest_for(skeleton: &Skeleton) -> Pose {
        Pose {
            rotations: vec![glam::Quat::IDENTITY; skeleton.bones.len()],
        }
    }

    /// Grows the pose with identity offsets so it covers every bone in
    /// `skeleton` (bones can be added after a pose was created).
    pub fn ensure_covers(&mut self, skeleton: &Skeleton) {
        if self.rotations.len() < skeleton.bones.len() {
            self.rotations
                .resize(skeleton.bones.len(), glam::Quat::IDENTITY);
        }
    }

    /// Sets one bone's rotation offset. Returns false if the bone index is
    /// out of range for this pose.
    pub fn set_rotation(&mut self, bone_index: usize, rotation: glam::Quat) -> bool {
        match self.rotations.get_mut(bone_index) {
            Some(slot) => {
                *slot = rotation;
                true
            }
            None => false,
        }
    }

    /// Each bone's model-space transform in the posed state. Requires
    /// bones stored parent-before-child, same as `Skeleton::world_bind_matrices`.
    pub fn posed_world_matrices(&self, skeleton: &Skeleton) -> Vec<glam::Mat4> {
        let mut world = vec![glam::Mat4::IDENTITY; skeleton.bones.len()];
        for (i, bone) in skeleton.bones.iter().enumerate() {
            let offset = self
                .rotations
                .get(i)
                .copied()
                .unwrap_or(glam::Quat::IDENTITY);
            world[i] = match bone.parent {
                Some(parent_index) => {
                    let bind_local = glam::Mat4::from_scale_rotation_translation(
                        bone.local_scale,
                        bone.local_rotation,
                        bone.local_position,
                    );
                    world[parent_index] * bind_local * glam::Mat4::from_quat(offset)
                }
                None => {
                    glam::Mat4::from_scale_rotation_translation(
                        bone.local_scale,
                        bone.local_rotation * offset,
                        bone.local_position,
                    )
                }
            };

        }
        world
    }

    /// Each joint's model-space position in the posed state.
    pub fn posed_joint_positions(&self, skeleton: &Skeleton) -> Vec<glam::Vec3> {
        self.posed_world_matrices(skeleton)
            .iter()
            .map(|m| m.transform_point3(glam::Vec3::ZERO))
            .collect()
    }

    /// Drags `joint_index` toward `target` (a model-space point) by
    /// rotating its PARENT joint, in 3D, around the parent's own
    /// position: the shortest swing that turns the parent-to-joint
    /// direction toward the parent-to-target direction. Everything
    /// hanging off that parent swings with it (standard FK). Always
    /// measured from the current pose, so aiming at the same target
    /// twice changes nothing. Returns false for root joints,
    /// out-of-range indices, or a zero-length direction.
    pub fn aim_joint_at(
        &mut self,
        skeleton: &Skeleton,
        joint_index: usize,
        target: glam::Vec3,
    ) -> bool {
        let Some(joint) = skeleton.bones.get(joint_index) else {
            return false;
        };
        let Some(parent_index) = joint.parent else {
            return false;
        };
        self.ensure_covers(skeleton);

        let posed_world = self.posed_world_matrices(skeleton);
        let pivot = posed_world[parent_index].transform_point3(glam::Vec3::ZERO);
        let current = posed_world[joint_index].transform_point3(glam::Vec3::ZERO) - pivot;
        let desired = target - pivot;
        if current.length_squared() < 1e-12 || desired.length_squared() < 1e-12 {
            return false;
        }

        // The swing, in model space, that carries the joint onto the ray
        // toward the target.
        let swing = glam::Quat::from_rotation_arc(current.normalize(), desired.normalize());

        // Convert it into the parent's own pose rotation: the parent's
        // world orientation is (frame it hangs in) * (its pose rotation).
        let parent = &skeleton.bones[parent_index];
        let grandparent_world = match parent.parent {
            Some(grandparent_index) => posed_world[grandparent_index],
            None => glam::Mat4::IDENTITY,
        };
        let (_, frame, _) =
            (grandparent_world * parent.local_matrix()).to_scale_rotation_translation();
        self.rotations[parent_index] =
            (frame.inverse() * swing * frame * self.rotations[parent_index]).normalize();
        true
    }


    /// Rotates `joint_index` around its own position by `swing` (a
    /// model-space rotation), by converting it into that joint's pose
    /// rotation: its world orientation is (the frame it hangs in) times
    /// (its pose rotation).
    fn swing_joint(
        &mut self,
        skeleton: &Skeleton,
        posed_world: &[glam::Mat4],
        joint_index: usize,
        swing: glam::Quat,
    ) {
        let joint = &skeleton.bones[joint_index];
        let parent_world = match joint.parent {
            Some(parent_index) => posed_world[parent_index],
            None => glam::Mat4::IDENTITY,
        };
        let (_, frame, _) = (parent_world * joint.local_matrix()).to_scale_rotation_translation();
        self.rotations[joint_index] =
            (frame.inverse() * swing * frame * self.rotations[joint_index]).normalize();
    }

    /// Two-bone IK: moves `end_index` to `target` (a model-space point) by
    /// rotating the two joints above it (its parent, "mid", and its
    /// grandparent, "root"). Bone lengths never change: an unreachable
    /// target stretches the limb straight toward it. The mid joint
    /// bends to the side it is already bent toward; a straight limb has
    /// no side yet, so it bends toward `pole_hint` (a direction).
    /// Returns false if the end joint doesn't have both a parent and a
    /// grandparent, or a bone has zero length.
    pub fn solve_two_bone_ik(
        &mut self,
        skeleton: &Skeleton,
        end_index: usize,
        target: glam::Vec3,
        pole_hint: glam::Vec3,
    ) -> bool {
        let Some(mid_index) = skeleton.bones.get(end_index).and_then(|bone| bone.parent) else {
            return false;
        };
        let Some(root_index) = skeleton.bones[mid_index].parent else {
            return false;
        };
        self.ensure_covers(skeleton);

        let posed_world = self.posed_world_matrices(skeleton);
        let root = posed_world[root_index].transform_point3(glam::Vec3::ZERO);
        let mid = posed_world[mid_index].transform_point3(glam::Vec3::ZERO);
        let end = posed_world[end_index].transform_point3(glam::Vec3::ZERO);

        let upper = (mid - root).length();
        let lower = (end - mid).length();
        let target_distance = (target - root).length();
        if upper < 1e-6 || lower < 1e-6 || target_distance < 1e-6 {
            return false;
        }

        // How far the end joint can actually get.
        let min_reach = (upper - lower).abs() + 1e-4;
        let max_reach = upper + lower - 1e-4;
        let reach = target_distance.max(min_reach).min(max_reach);
        let direction = (target - root) / target_distance;

        // Which way the mid joint bends: keep the current side; a straight
        // limb falls back to the hint.
        let current_axis = (end - root).normalize_or_zero();
        let mut pole = (mid - root) - current_axis * (mid - root).dot(current_axis);
        if pole.length() < 1e-3 * (upper + lower) {
            pole = pole_hint;
        }
        pole -= direction * pole.dot(direction);
        if pole.length_squared() < 1e-12 {
            pole = direction.any_orthonormal_vector();
        }
        let pole = pole.normalize();

        // Law of cosines: where the mid joint has to sit.
        let along = (upper * upper - lower * lower + reach * reach) / (2.0 * reach);
        let height = (upper * upper - along * along).max(0.0).sqrt();
        let new_mid = root + direction * along + pole * height;
        let new_end = root + direction * reach;

        // First swing the root joint so the mid joint lands on new_mid ...
        let swing_root =
            glam::Quat::from_rotation_arc((mid - root).normalize(), (new_mid - root).normalize());
        self.swing_joint(skeleton, &posed_world, root_index, swing_root);

        // ... then, with the end joint carried along, swing the mid joint
        // so the end joint lands on new_end.
        let posed_world = self.posed_world_matrices(skeleton);
        let mid_now = posed_world[mid_index].transform_point3(glam::Vec3::ZERO);
        let end_now = posed_world[end_index].transform_point3(glam::Vec3::ZERO);
        let swing_mid = glam::Quat::from_rotation_arc(
            (end_now - mid_now).normalize(),
            (new_end - mid_now).normalize(),
        );
        self.swing_joint(skeleton, &posed_world, mid_index, swing_mid);
        true
    }

    /// Skinning matrices (posed * rest.inverse()) for the whole skeleton,
    /// ready to upload to the GPU.
    pub fn skinning_matrices(&self, skeleton: &Skeleton) -> Vec<glam::Mat4> {
        let rest_world = skeleton.world_bind_matrices();
        let posed_world = self.posed_world_matrices(skeleton);
        crate::animation::compute_skinning_matrices(&rest_world, &posed_world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distance(a: glam::Vec3, b: glam::Vec3) -> f32 {
        (a - b).length()
    }

    /// Bones: 0 root at the origin; 1 "a" at (1,0,0); 2 "b" at (-1,0,0),
    /// both children of the root; 3 "c" one step further out from "a".
    fn branching_skeleton() -> Skeleton {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, glam::Vec3::ZERO);
        let a = skeleton.add_bone("a", Some(root), glam::Vec3::new(1.0, 0.0, 0.0));
        skeleton.add_bone("b", Some(root), glam::Vec3::new(-1.0, 0.0, 0.0));
        skeleton.add_bone("c", Some(a), glam::Vec3::new(1.0, 0.0, 0.0));
        skeleton
    }

    #[test]
    fn dragging_a_joint_swings_its_parent_onto_the_target_in_3d() {
        let skeleton = branching_skeleton();
        let mut pose = Pose::rest_for(&skeleton);

        // Toward +Y, then toward +Z, a direction the old Z-only drag could not reach.
        assert!(pose.aim_joint_at(&skeleton, 1, glam::Vec3::new(0.0, 5.0, 0.0)));
        let p = pose.posed_joint_positions(&skeleton);
        assert!(distance(p[0], glam::Vec3::ZERO) < 1e-5);
        assert!(distance(p[1], glam::Vec3::new(0.0, 1.0, 0.0)) < 1e-5);
        assert!(distance(p[3], glam::Vec3::new(0.0, 2.0, 0.0)) < 1e-5);

        assert!(pose.aim_joint_at(&skeleton, 1, glam::Vec3::new(0.0, 0.0, 5.0)));
        let p = pose.posed_joint_positions(&skeleton);
        assert!(distance(p[1], glam::Vec3::new(0.0, 0.0, 1.0)) < 1e-5);
        assert!(distance(p[3], glam::Vec3::new(0.0, 0.0, 2.0)) < 1e-5);
    }

    #[test]
    fn aiming_at_the_same_target_twice_changes_nothing() {
        let skeleton = branching_skeleton();
        let mut pose = Pose::rest_for(&skeleton);
        let target = glam::Vec3::new(0.3, 0.8, -0.5);

        assert!(pose.aim_joint_at(&skeleton, 3, target));
        let first = pose.posed_joint_positions(&skeleton);
        assert!(pose.aim_joint_at(&skeleton, 3, target));
        let second = pose.posed_joint_positions(&skeleton);
        for (a, b) in first.iter().zip(second.iter()) {
            assert!(distance(*a, *b) < 1e-4);
        }

        // "c" now lies on the ray from its parent "a" toward the target.
        let toward_c = (first[3] - first[1]).normalize();
        let toward_target = (target - first[1]).normalize();
        assert!(distance(toward_c, toward_target) < 1e-4);
    }

    #[test]
    fn rotating_a_joint_moves_only_its_own_children() {
        let skeleton = branching_skeleton();
        let mut pose = Pose::rest_for(&skeleton);

        // Dragging "c" rotates "a" around itself; the root and "b" stay put.
        assert!(pose.aim_joint_at(&skeleton, 3, glam::Vec3::new(1.0, 5.0, 0.0)));
        let p = pose.posed_joint_positions(&skeleton);
        assert!(distance(p[0], glam::Vec3::ZERO) < 1e-5);
        assert!(distance(p[1], glam::Vec3::new(1.0, 0.0, 0.0)) < 1e-5);
        assert!(distance(p[2], glam::Vec3::new(-1.0, 0.0, 0.0)) < 1e-5);
        assert!(distance(p[3], glam::Vec3::new(1.0, 1.0, 0.0)) < 1e-5);
    }

    #[test]
    fn roots_and_bad_indices_cannot_be_aimed() {
        let skeleton = branching_skeleton();
        let mut pose = Pose::rest_for(&skeleton);
        let target = glam::Vec3::new(0.0, 1.0, 0.0);
        assert!(!pose.aim_joint_at(&skeleton, 0, target));

        assert!(!pose.aim_joint_at(&skeleton, 99, target));
    }

    /// A straight leg hanging down: 0 hip (root), 1 knee, 2 ankle, one unit apart.
    fn leg_skeleton() -> Skeleton {
        let mut skeleton = Skeleton::default();
        let hip = skeleton.add_bone("hip", None, glam::Vec3::ZERO);
        let knee = skeleton.add_bone("knee", Some(hip), glam::Vec3::new(0.0, -1.0, 0.0));
        skeleton.add_bone("ankle", Some(knee), glam::Vec3::new(0.0, -1.0, 0.0));
        skeleton
    }

    #[test]
    fn ik_puts_the_end_joint_on_a_reachable_target_and_keeps_bone_lengths() {
        let skeleton = leg_skeleton();
        let mut pose = Pose::rest_for(&skeleton);
        let target = glam::Vec3::new(0.5, -1.2, 0.4);

        assert!(pose.solve_two_bone_ik(&skeleton, 2, target, glam::Vec3::Z));
        let p = pose.posed_joint_positions(&skeleton);
        assert!(distance(p[2], target) < 1e-4);
        assert!((distance(p[0], p[1]) - 1.0).abs() < 1e-4);
        assert!((distance(p[1], p[2]) - 1.0).abs() < 1e-4);

        // Solving again for the same target changes nothing.
        assert!(pose.solve_two_bone_ik(&skeleton, 2, target, glam::Vec3::Z));
        let again = pose.posed_joint_positions(&skeleton);
        for (a, b) in p.iter().zip(again.iter()) {
            assert!(distance(*a, *b) < 1e-4);
        }
    }

    #[test]
    fn ik_stretches_toward_an_unreachable_target_without_stretching_the_bones() {
        let skeleton = leg_skeleton();
        let mut pose = Pose::rest_for(&skeleton);
        assert!(pose.solve_two_bone_ik(
            &skeleton,
            2,
            glam::Vec3::new(0.0, -5.0, 0.0),
            glam::Vec3::Z
        ));
        let p = pose.posed_joint_positions(&skeleton);
        assert!(distance(p[2], glam::Vec3::new(0.0, -2.0, 0.0)) < 1e-3);
        assert!((distance(p[0], p[1]) - 1.0).abs() < 1e-4);
        assert!((distance(p[1], p[2]) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn ik_keeps_the_current_bend_side_instead_of_the_hint() {
        let skeleton = leg_skeleton();
        let mut pose = Pose::rest_for(&skeleton);

        // A straight leg has no bend side yet, so the hint (+Z) decides.
        assert!(pose.solve_two_bone_ik(
            &skeleton,
            2,
            glam::Vec3::new(0.0, -1.0, 0.0),
            glam::Vec3::Z
        ));
        let bent = pose.posed_joint_positions(&skeleton);
        assert!(bent[1].z > 0.5);

        // Now it is bent toward +Z, so an opposite hint (-Z) must not flip it.
        assert!(pose.solve_two_bone_ik(
            &skeleton,
            2,
            glam::Vec3::new(0.3, -1.2, 0.0),
            glam::Vec3::NEG_Z
        ));
        let moved = pose.posed_joint_positions(&skeleton);
        assert!(moved[1].z > 0.0);
    }

    #[test]
    fn ik_needs_a_two_bone_chain() {
        let skeleton = leg_skeleton();
        let mut pose = Pose::rest_for(&skeleton);
        let target = glam::Vec3::new(0.0, -1.0, 0.5);
        assert!(!pose.solve_two_bone_ik(&skeleton, 0, target, glam::Vec3::Z)); // a root has no parent
        assert!(!pose.solve_two_bone_ik(&skeleton, 1, target, glam::Vec3::Z)); // no grandparent
        assert!(!pose.solve_two_bone_ik(&skeleton, 99, target, glam::Vec3::Z));
    }
    }