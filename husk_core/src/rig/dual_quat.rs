//! Dual quaternion skinning: blending what the bones do as dual quaternions
//! instead of matrices, so a bent or twisted limb keeps its thickness instead of
//! pinching (Blender's "Preserve Volume").
//!
//! Ported from Blender, the dual quaternion code of BLI math_rotation
//! (SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All rights reserved.,
//! SPDX-License-Identifier: GPL-2.0-or-later), whose conversion routines come
//! from the University of Dublin, Trinity College (SPDX-License-Identifier: Zlib,
//! Copyright 2006-2007 University of Dublin, Trinity College, All Rights Reserved.):
//! - `DualQuat::from_matrix` is `mat4_to_dquat` for a matrix without scale: the
//!   rotation as a quaternion, and a dual part that is half the translation
//!   times the rotation.
//! - `DualQuat::blend` is `add_weighted_dq_dq` and `normalize_dq`: each bone's
//!   quaternion is turned round when it points away from what has been added so
//!   far, so two bones that make the same movement never cancel. A vertex whose
//!   weights add up to less than 0.0001 is not moved, as in `armature_deform`.
//! - `DualQuat::transform_point` is `mul_v3m3_dq`, and `transform_direction` is
//!   its rotation.
//!
//! Husk leaves out, for now: Blender's handling of bone scale. A matrix with
//! scale or shear keeps only its rotation and position here, which is all a rig
//! with bones at scale 1 ever has.
//!
//! The vertex shader (`dual_quat_skin` in shader.wgsl) does the same blend on the
//! GPU; the tests below are what it follows.

use glam::{Mat3, Mat4, Vec3, Vec4};

/// A vertex whose weights add up to less than this is not moved.
const MIN_TOTAL_WEIGHT: f32 = 0.0001;

/// A rotation and a movement in one: a unit dual quaternion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DualQuat {
    /// The rotation as a quaternion, (x, y, z, w).
    pub real: Vec4,
    /// Half the translation times the rotation, (x, y, z, w).
    pub dual: Vec4,
}

/// The Hamilton product of two quaternions held as (x, y, z, w).
fn quat_mul(a: Vec4, b: Vec4) -> Vec4 {
    let (a_vector, b_vector) = (a.truncate(), b.truncate());
    let vector = b_vector * a.w + a_vector * b.w + a_vector.cross(b_vector);
    vector.extend(a.w * b.w - a_vector.dot(b_vector))
}

/// Turns `v` by the unit quaternion `q`, held as (x, y, z, w).
fn quat_rotate(q: Vec4, v: Vec3) -> Vec3 {
    let axis = q.truncate();
    let t = 2.0 * axis.cross(v);
    v + t * q.w + axis.cross(t)
}

impl DualQuat {
    /// No rotation and no movement.
    pub const IDENTITY: DualQuat = DualQuat {
        real: Vec4::new(0.0, 0.0, 0.0, 1.0),
        dual: Vec4::ZERO,
    };

    /// The dual quaternion of a skinning matrix: its rotation and its
    /// translation. Scale and shear are dropped.
    pub fn from_matrix(matrix: Mat4) -> DualQuat {
        let x = matrix.x_axis.truncate().normalize_or_zero();
        let y_raw = matrix.y_axis.truncate();
        let y = (y_raw - x * x.dot(y_raw)).normalize_or_zero();
        let real = if x != Vec3::ZERO && y != Vec3::ZERO {
            let rotation = glam::Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)));
            Vec4::new(rotation.x, rotation.y, rotation.z, rotation.w)
        } else {
            DualQuat::IDENTITY.real
        };
        let translation = matrix.w_axis.truncate();
        let dual = quat_mul(translation.extend(0.0), real) * 0.5;
        DualQuat { real, dual }
    }

    /// Blends bones' dual quaternions by weight, each with the weight it has on
    /// one vertex. A weight that is not above 0 is ignored. None when the weights
    /// add up to less than 0.0001: that vertex is not moved by the bones.
    pub fn blend(influences: &[(DualQuat, f32)]) -> Option<DualQuat> {
        let mut real = Vec4::ZERO;
        let mut dual = Vec4::ZERO;
        let mut total = 0.0_f32;
        for (bone, weight) in influences {
            let weight = *weight;
            if !(weight > 0.0) {
                continue;
            }
            // Turn a quaternion that points away from what is added so far
            // round, so it adds to it instead of cancelling it.
            let sign = if bone.real.dot(real) < 0.0 { -1.0 } else { 1.0 };
            real += bone.real * (weight * sign);
            dual += bone.dual * (weight * sign);
            total += weight;
        }
        if total < MIN_TOTAL_WEIGHT {
            return None;
        }
        let scale = 1.0 / total;
        Some(DualQuat {
            real: real * scale,
            dual: dual * scale,
        })
    }

    /// Where `point` goes. A blend that is not exactly unit length is made unit
    /// length here, as `mul_v3m3_dq` does.
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let length_squared = self.real.length_squared();
        if length_squared < 1e-12 {
            return point;
        }
        let real_vector = self.real.truncate();
        let dual_vector = self.dual.truncate();
        let rotation = self.real / length_squared.sqrt();
        let translation = 2.0
            * (dual_vector * self.real.w - real_vector * self.dual.w
                - dual_vector.cross(real_vector))
            / length_squared;
        quat_rotate(rotation, point) + translation
    }

    /// Where a direction (a normal, say) goes: only the rotation applies.
    pub fn transform_direction(&self, direction: Vec3) -> Vec3 {
        let length_squared = self.real.length_squared();
        if length_squared < 1e-12 {
            return direction;
        }
        quat_rotate(self.real / length_squared.sqrt(), direction)
    }

    /// The two quaternions as the shader reads them: the real part, then the
    /// dual part, each as (x, y, z, w).
    pub fn to_gpu(&self) -> [[f32; 4]; 2] {
        [self.real.to_array(), self.dual.to_array()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    /// A turn of `degrees` about the line through `pivot` along `axis`.
    fn about(axis: Vec3, degrees: f32, pivot: Vec3) -> Mat4 {
        Mat4::from_translation(pivot)
            * Mat4::from_axis_angle(axis.normalize(), degrees.to_radians())
            * Mat4::from_translation(-pivot)
    }

    #[test]
    fn a_rigid_matrix_moves_points_and_directions_as_its_dual_quaternion_does() {
        let matrices = [
            about(Vec3::new(1.0, 2.0, 3.0), 70.0, Vec3::new(0.5, -1.0, 2.0)),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            Mat4::IDENTITY,
            about(Vec3::Z, 180.0, Vec3::ZERO),
            about(Vec3::new(-1.0, 0.5, 0.2), 250.0, Vec3::new(3.0, 3.0, -2.0)),
        ];
        for matrix in matrices {
            let dual_quat = DualQuat::from_matrix(matrix);
            for point in [
                Vec3::new(0.3, -0.7, 1.9),
                Vec3::ZERO,
                Vec3::new(-4.0, 2.0, 0.1),
            ] {
                assert!(close(
                    dual_quat.transform_point(point),
                    matrix.transform_point3(point)
                ));
                assert!(close(
                    dual_quat.transform_direction(point),
                    matrix.transform_vector3(point)
                ));
            }
        }
    }

    #[test]
    fn two_bones_turned_apart_keep_their_thickness_with_dual_quaternions_but_not_with_matrices() {
        let rest = Mat4::IDENTITY;
        let turned = about(Vec3::Z, 120.0, Vec3::ZERO);
        let point = Vec3::X;

        // Blending the matrices pulls the point toward the middle: half of each.
        let linear = (rest.transform_point3(point) + turned.transform_point3(point)) * 0.5;
        assert!((linear.length() - 0.5).abs() < 1e-4);

        let blended = DualQuat::blend(&[
            (DualQuat::from_matrix(rest), 0.5),
            (DualQuat::from_matrix(turned), 0.5),
        ])
        .expect("the weights add up to 1");
        let moved = blended.transform_point(point);
        // It stays as far from the axis as it was, and went halfway round.
        assert!((moved.length() - 1.0).abs() < 1e-4);
        assert!(close(moved, Vec3::new(0.5, 0.75_f32.sqrt(), 0.0)));
    }

    #[test]
    fn bones_turned_about_a_joint_that_is_not_the_origin_blend_to_a_turn_about_that_joint() {
        let joint = Vec3::new(2.0, 0.0, 0.0);
        let turned = about(Vec3::Z, 120.0, joint);
        let blended = DualQuat::blend(&[
            (DualQuat::IDENTITY, 0.5),
            (DualQuat::from_matrix(turned), 0.5),
        ])
        .expect("the weights add up to 1");
        // One unit from the joint, halfway round (60 degrees) about it.
        let moved = blended.transform_point(Vec3::new(3.0, 0.0, 0.0));
        assert!(close(moved, Vec3::new(2.5, 0.75_f32.sqrt(), 0.0)));
    }

    #[test]
    fn a_quaternion_pointing_away_is_turned_round_so_one_movement_never_cancels_itself() {
        let matrix = about(Vec3::Y, 50.0, Vec3::new(1.0, 0.0, 0.0));
        let movement = DualQuat::from_matrix(matrix);
        // The same movement written with every sign changed.
        let flipped = DualQuat {
            real: -movement.real,
            dual: -movement.dual,
        };
        let blended = DualQuat::blend(&[(movement, 0.4), (flipped, 0.6)]).expect("weights");
        let point = Vec3::new(0.3, -0.7, 1.9);
        assert!(close(
            blended.transform_point(point),
            matrix.transform_point3(point)
        ));
    }

    #[test]
    fn too_little_weight_gives_no_blend_and_weights_that_do_not_add_up_to_one_are_scaled() {
        assert!(DualQuat::blend(&[]).is_none());
        assert!(DualQuat::blend(&[(DualQuat::IDENTITY, 0.00005)]).is_none());
        assert!(DualQuat::blend(&[(DualQuat::IDENTITY, 0.0), (DualQuat::IDENTITY, -1.0)]).is_none());
        assert!(DualQuat::blend(&[(DualQuat::IDENTITY, f32::NAN)]).is_none());

        // A quarter of one bone is still that bone's movement.
        let matrix = about(Vec3::new(1.0, 1.0, 0.0), 40.0, Vec3::new(0.0, 2.0, 0.0));
        let blended = DualQuat::blend(&[(DualQuat::from_matrix(matrix), 0.25)]).expect("weights");
        let point = Vec3::new(1.0, -1.0, 0.5);
        assert!(close(
            blended.transform_point(point),
            matrix.transform_point3(point)
        ));
    }

    #[test]
    fn scale_is_dropped_and_a_matrix_with_no_axes_is_just_its_movement() {
        let scaled =
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)) * Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let dual_quat = DualQuat::from_matrix(scaled);
        assert!(close(
            dual_quat.transform_point(Vec3::new(1.0, 1.0, 1.0)),
            Vec3::new(2.0, 1.0, 1.0)
        ));

        let empty = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::ZERO);
        assert_eq!(DualQuat::from_matrix(empty), DualQuat::IDENTITY);
    }

    #[test]
    fn the_gpu_layout_is_the_real_part_then_the_dual_part() {
        assert_eq!(
            DualQuat::IDENTITY.to_gpu(),
            [[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 0.0]]
        );
        // Half the translation, with no rotation.
        let moved = DualQuat::from_matrix(Mat4::from_translation(Vec3::new(2.0, 4.0, 6.0)));
        assert_eq!(moved.to_gpu(), [[0.0, 0.0, 0.0, 1.0], [1.0, 2.0, 3.0, 0.0]]);
    }
}