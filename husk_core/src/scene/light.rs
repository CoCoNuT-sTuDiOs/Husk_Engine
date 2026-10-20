use super::transform::Transform;
use glam::Vec3;

/// The most lights the renderer uses. The scene may hold more light
/// objects; the first ones in the list are the ones that shine.
pub const MAX_LIGHTS: usize = 8;

/// What kind of light this is. A sun shines the same everywhere from one
/// direction; a point light shines in every direction from where it sits
/// and fades with the square of the distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    Sun,
    Point,
}

/// A light's own settings. Where it sits (point) or which way it shines
/// (sun) comes from its object's transform: it shines along its own
/// negative Z axis, like a camera looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    /// Red, green and blue, each from 0 to 1.
    pub color: [f32; 3],
    /// A sun's intensity is its brightness. A point light's is its
    /// brightness one unit away; it fades with distance squared.
    pub intensity: f32,
}

impl Light {
    pub const MAX_SUN_INTENSITY: f32 = 20.0;
    pub const MAX_POINT_INTENSITY: f32 = 2000.0;
    /// Closest distance the point-light fade counts, so a surface touching
    /// the light does not shine infinitely bright.
    pub const MIN_DISTANCE: f32 = 0.1;

    /// A white sun of a sensible strength.
    pub fn sun() -> Light {
        Light {
            kind: LightKind::Sun,
            color: [1.0, 1.0, 1.0],
            intensity: 2.5,
        }
    }

    /// A white point light that reads well a few units away.
    pub fn point() -> Light {
        Light {
            kind: LightKind::Point,
            color: [1.0, 1.0, 1.0],
            intensity: 40.0,
        }
    }

    /// The top of the intensity range for this kind of light.
    pub fn max_intensity(&self) -> f32 {
        match self.kind {
            LightKind::Sun => Self::MAX_SUN_INTENSITY,
            LightKind::Point => Self::MAX_POINT_INTENSITY,
        }
    }

    /// The same light with every value held inside its range.
    pub fn clamped(self) -> Light {
        Light {
            kind: self.kind,
            color: self.color.map(|c| c.clamp(0.0, 1.0)),
            intensity: self.intensity.clamp(0.0, self.max_intensity()),
        }
    }
}

/// How much of a point light's brightness arrives at `distance` units:
/// one over the distance squared, with the distance held above
/// `Light::MIN_DISTANCE`. The shader uses the same formula.
pub fn point_falloff(distance: f32) -> f32 {
    let d = distance.max(Light::MIN_DISTANCE);
    1.0 / (d * d)
}

/// The direction the light's rays travel, as a unit vector in the scene:
/// the object's own negative Z axis. For a sun this is its one direction;
/// for a point light it is not used.
pub fn shine_direction(transform: &Transform) -> Vec3 {
    (transform.rotation * -Vec3::Z).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_inside_their_ranges() {
        for light in [Light::sun(), Light::point()] {
            assert_eq!(light.clamped(), light);
            assert!(light.intensity > 0.0);
        }
    }

    #[test]
    fn clamped_holds_colour_and_intensity_in_range_per_kind() {
        let wild_sun = Light {
            kind: LightKind::Sun,
            color: [-1.0, 0.5, 3.0],
            intensity: 500.0,
        };
        let held = wild_sun.clamped();
        assert_eq!(held.color, [0.0, 0.5, 1.0]);
        assert_eq!(held.intensity, Light::MAX_SUN_INTENSITY);

        let wild_point = Light {
            kind: LightKind::Point,
            color: [1.0, 1.0, 1.0],
            intensity: 500.0,
        };
        // 500 is fine for a point light but far too much for a sun.
        assert_eq!(wild_point.clamped().intensity, 500.0);
        let too_much = Light {
            intensity: 9999.0,
            ..wild_point
        };
        assert_eq!(too_much.clamped().intensity, Light::MAX_POINT_INTENSITY);
        let negative = Light {
            intensity: -4.0,
            ..wild_point
        };
        assert_eq!(negative.clamped().intensity, 0.0);
    }

    #[test]
    fn a_point_light_fades_with_the_square_of_the_distance() {
        assert!((point_falloff(1.0) - 1.0).abs() < 1e-6);
        assert!((point_falloff(2.0) - 0.25).abs() < 1e-6);
        assert!((point_falloff(4.0) - 0.0625).abs() < 1e-6);
        // Doubling the distance quarters the light.
        assert!((point_falloff(3.0) / point_falloff(6.0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn the_fade_never_blows_up_at_the_light() {
        let near = point_falloff(0.0);
        assert!(near.is_finite());
        assert!((near - 100.0).abs() < 1e-3);
        assert_eq!(point_falloff(0.0), point_falloff(0.05));
    }

    #[test]
    fn a_light_shines_along_its_own_negative_z_axis() {
        let mut t = Transform::default();
        assert!((shine_direction(&t) - -Vec3::Z).length() < 1e-5);
        // A quarter turn about Y sends -Z to -X, as for a camera.
        t.rotation = glam::Quat::from_rotation_y(90f32.to_radians());
        assert!((shine_direction(&t) - -Vec3::X).length() < 1e-5);
        // Scale does not change the direction or its length.
        t.scale = Vec3::new(5.0, 0.5, 2.0);
        let d = shine_direction(&t);
        assert!((d - -Vec3::X).length() < 1e-5);
        assert!((d.length() - 1.0).abs() < 1e-5);
    }
}