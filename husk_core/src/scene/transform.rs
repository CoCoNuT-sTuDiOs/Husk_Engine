use glam::{EulerRot, Mat4, Quat, Vec3};

/// Where an object sits in the scene: a position, a rotation and a scale.
/// The scene owns it; the renderer only keeps the matrix made from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    /// The matrix that places the object: scale first, then rotate, then
    /// move.
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Splits a placement matrix back into its parts. The matrix must be
    /// made of a scale, a rotation and a move only (no shear).
    pub fn from_matrix(matrix: Mat4) -> Transform {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Transform {
            translation,
            rotation,
            scale,
        }
    }

    /// The rotation as Euler angles in degrees, in Blender's XYZ order:
    /// the object is turned about X first, then Y, then Z, all about the
    /// scene's fixed axes.
    pub fn euler_degrees(&self) -> Vec3 {
        let (z, y, x) = self.rotation.to_euler(EulerRot::ZYX);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    /// Sets the rotation from Euler angles in degrees (Blender's XYZ order,
    /// see `euler_degrees`).
    pub fn set_euler_degrees(&mut self, degrees: Vec3) {
        self.rotation = Quat::from_euler(
            EulerRot::ZYX,
            degrees.z.to_radians(),
            degrees.y.to_radians(),
            degrees.x.to_radians(),
        );
    }

    /// A point given in the object's own space, as the scene sees it.
    pub fn point_to_world(&self, point: Vec3) -> Vec3 {
        self.to_matrix().transform_point3(point)
    }

    /// A scene point, expressed in the object's own space (where its
    /// skeleton and mesh live).
    pub fn point_to_local(&self, point: Vec3) -> Vec3 {
        self.to_matrix().inverse().transform_point3(point)
    }

    /// A scene direction (such as "towards the viewer") expressed in the
    /// object's own space, as a unit vector. A zero direction stays zero.
    pub fn direction_to_local(&self, direction: Vec3) -> Vec3 {
        self.to_matrix()
            .inverse()
            .transform_vector3(direction)
            .normalize_or_zero()
    }
}

/// Keeps each scale component away from zero (a flattened-to-nothing
/// object cannot be lit or picked), keeping its sign.
pub fn clamp_scale(scale: Vec3) -> Vec3 {
    const MIN: f32 = 0.001;
    let fix = |s: f32| {
        if s.abs() >= MIN {
            s
        } else if s.is_sign_negative() {
            -MIN
        } else {
            MIN
        }
    };
    Vec3::new(fix(scale.x), fix(scale.y), fix(scale.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: Vec3, b: Vec3) {
assert!((a - b).length() < 1e-4, "{a:?} is not close to {b:?}");
    }

    #[test]
    fn the_default_leaves_points_where_they_are() {
        let m = Transform::default().to_matrix();
        let p = Vec3::new(1.5, -2.0, 3.0);
        assert_close(m.transform_point3(p), p);
    }

    #[test]
    fn the_matrix_scales_then_rotates_then_moves() {
        let mut t = Transform::default();
        t.scale = Vec3::splat(2.0);
        t.rotation = Quat::from_rotation_z(90f32.to_radians());
        t.translation = Vec3::new(0.0, 0.0, 5.0);
        // (1,0,0) -> scaled (2,0,0) -> turned (0,2,0) -> moved (0,2,5).
        assert_close(
            t.to_matrix().transform_point3(Vec3::X),
            Vec3::new(0.0, 2.0, 5.0),
        );
    }

    #[test]
    fn from_matrix_recovers_the_parts() {
        let mut t = Transform::default();
        t.scale = Vec3::new(0.5, 2.0, 1.5);
        t.rotation = Quat::from_euler(EulerRot::ZYX, 0.4, -0.3, 0.2);
        t.translation = Vec3::new(3.0, -1.0, 2.0);
        let back = Transform::from_matrix(t.to_matrix());
        assert_close(back.translation, t.translation);
        assert_close(back.scale, t.scale);
        // A rotation and its negative are the same turn, so compare what
        // they do to a point instead of comparing the numbers.
        let probe = Vec3::new(0.3, 0.8, -0.5);
        assert_close(back.rotation * probe, t.rotation * probe);
    }

    #[test]
    fn euler_degrees_turn_x_first_then_y_then_z() {
        let mut t = Transform::default();
        t.set_euler_degrees(Vec3::new(90.0, 0.0, 90.0));
        // X first: (1,0,0) stays. Then Z by 90: (1,0,0) -> (0,1,0).
        // Z first would have ended at (0,0,1) instead.
        assert_close(t.rotation * Vec3::X, Vec3::Y);

        t.set_euler_degrees(Vec3::new(90.0, 0.0, 0.0));
        assert_close(t.rotation * Vec3::Y, Vec3::Z);

        t.set_euler_degrees(Vec3::new(0.0, 90.0, 0.0));
        assert_close(t.rotation * Vec3::Z, Vec3::X);
    }
    fn awkward_transform() -> Transform {
        let mut t = Transform::default();
        t.translation = Vec3::new(3.0, -1.0, 2.0);
        t.scale = Vec3::new(2.0, 0.5, 1.5);
        t.set_euler_degrees(Vec3::new(30.0, 40.0, 50.0));
        t
    }

    #[test]
    fn points_convert_to_world_and_back() {
        let t = awkward_transform();
        let p = Vec3::new(0.7, -0.2, 1.1);
        assert_close(t.point_to_world(p), t.to_matrix().transform_point3(p));
        assert_close(t.point_to_local(t.point_to_world(p)), p);
        assert_close(t.point_to_world(t.point_to_local(p)), p);
    }

    #[test]
    fn directions_convert_into_object_space_as_unit_vectors() {
        let mut t = Transform::default();
        t.scale = Vec3::splat(3.0);
        t.rotation = Quat::from_rotation_z(90f32.to_radians());
        // Local X turns into world Y, so world Y is local X.
        assert_close(t.direction_to_local(Vec3::Y), Vec3::X);
        let d = awkward_transform().direction_to_local(Vec3::new(0.3, -0.8, 0.5));
        assert!((d.length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn clamp_scale_keeps_sign_and_avoids_zero() {
    assert_close(
            clamp_scale(Vec3::new(0.0, -0.0001, 2.0)),
            Vec3::new(0.001, -0.001, 2.0),
        );
        assert_close(
            clamp_scale(Vec3::new(-3.0, 0.5, 0.001)),
            Vec3::new(-3.0, 0.5, 0.001),
        );
    }

    #[test]
    fn euler_degrees_round_trip() {
        let mut t = Transform::default();
        t.set_euler_degrees(Vec3::new(30.0, 40.0, 50.0));
        assert_close(t.euler_degrees(), Vec3::new(30.0, 40.0, 50.0));
    }
}