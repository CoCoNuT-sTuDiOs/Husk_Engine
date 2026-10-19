/// A scene camera's lens. Where it sits and which way it looks come from
/// its object's transform: it looks along its own negative Z axis, with +Y
/// up, as Blender's cameras do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The vertical field of view, in degrees.
    pub fov_degrees: f32,
}

impl Camera {
    /// The narrowest and widest lens the panel offers, in degrees.
    pub const MIN_FOV: f32 = 10.0;
    pub const MAX_FOV: f32 = 120.0;

    /// The same lens with its field of view held inside the range.
    pub fn clamped(self) -> Camera {
        Camera {
            fov_degrees: self.fov_degrees.clamp(Self::MIN_FOV, Self::MAX_FOV),
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera { fov_degrees: 35.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_lens_is_inside_its_range() {
        let lens = Camera::default();
        assert!(lens.fov_degrees >= Camera::MIN_FOV && lens.fov_degrees <= Camera::MAX_FOV);
        assert_eq!(lens.clamped(), lens);
    }

    #[test]
    fn clamped_holds_the_field_of_view_in_range() {
        assert_eq!(Camera { fov_degrees: 1.0 }.clamped().fov_degrees, Camera::MIN_FOV);
        assert_eq!(Camera { fov_degrees: 500.0 }.clamped().fov_degrees, Camera::MAX_FOV);
        assert_eq!(Camera { fov_degrees: 60.0 }.clamped().fov_degrees, 60.0);
    }
}