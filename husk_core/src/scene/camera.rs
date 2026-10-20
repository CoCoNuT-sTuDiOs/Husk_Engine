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

/// Where the output frame sits inside a viewport of `viewport_width` x
/// `viewport_height` pixels: the biggest rectangle with the frame's shape
/// (`frame_aspect` is width / height) that fits, centred. Returns
/// (x, y, width, height) in pixels.
pub fn frame_rect(
    viewport_width: f32,
    viewport_height: f32,
    frame_aspect: f32,
) -> (f32, f32, f32, f32) {
    let viewport_aspect = viewport_width / viewport_height;
    let (width, height) = if frame_aspect >= viewport_aspect {
        (viewport_width, viewport_width / frame_aspect)
    } else {
        (viewport_height * frame_aspect, viewport_height)
    };
    (
        (viewport_width - width) * 0.5,
        (viewport_height - height) * 0.5,
        width,
        height,
    )
}

/// The vertical field of view (degrees) the whole viewport must be drawn
/// with so that the output frame inside it, `frame_height` pixels tall in a
/// viewport `viewport_height` pixels tall, sees exactly `fov_degrees`.
pub fn viewport_fov_degrees(fov_degrees: f32, viewport_height: f32, frame_height: f32) -> f32 {
    let half = (fov_degrees.to_radians() * 0.5).tan() * (viewport_height / frame_height);
    2.0 * half.atan().to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_frame_is_the_biggest_centred_rectangle_of_its_shape_that_fits() {
        // A wide frame in a square viewport: full width, bars above and below.
        let (x, y, w, h) = frame_rect(800.0, 800.0, 16.0 / 9.0);
        assert!(x.abs() < 1e-3 && (w - 800.0).abs() < 1e-3);
        assert!((h - 450.0).abs() < 1e-3 && (y - 175.0).abs() < 1e-3);

        // A tall frame in a wide viewport: full height, bars at the sides.
        let (x, y, w, h) = frame_rect(1600.0, 900.0, 9.0 / 16.0);
        assert!((h - 900.0).abs() < 1e-3 && y.abs() < 1e-3);
        assert!((w - 506.25).abs() < 1e-2 && (x - 546.875).abs() < 1e-2);

        // The same shape fills the viewport exactly.
        let (x, y, w, h) = frame_rect(1600.0, 900.0, 16.0 / 9.0);
        assert!(x.abs() < 1e-3 && y.abs() < 1e-3);
        assert!((w - 1600.0).abs() < 1e-2 && (h - 900.0).abs() < 1e-2);
    }

    #[test]
    fn the_viewport_view_widens_when_the_frame_is_shorter_than_the_viewport() {
        assert!((viewport_fov_degrees(40.0, 900.0, 900.0) - 40.0).abs() < 1e-4);
        // Half the height at 90 degrees: tan(45) = 1 doubles to 2.
        let wide = viewport_fov_degrees(90.0, 800.0, 400.0);
        assert!((wide - 2.0 * 2.0_f32.atan().to_degrees()).abs() < 1e-3);
    }

    #[test]
    fn the_frame_edge_is_exactly_where_the_lens_says_it_is() {
        // A camera at the origin looking down -Z with a 40 degree lens, and
        // a 16:9 frame inside a square 800 x 800 viewport.
        let (viewport_w, viewport_h) = (800.0, 800.0);
        let (_, frame_y, _, frame_h) = frame_rect(viewport_w, viewport_h, 16.0 / 9.0);
        let fov = viewport_fov_degrees(40.0, viewport_h, frame_h);
        let projection =
            glam::Mat4::perspective_rh(fov.to_radians(), viewport_w / viewport_h, 0.1, 100.0);
        // The top edge of what the lens sees, ten units ahead.
        let top = glam::Vec3::new(0.0, 10.0 * 20.0_f32.to_radians().tan(), -10.0);
        let (_, y) =
            crate::render::picking::project_to_screen(projection, viewport_w, viewport_h, top)
                .unwrap();
        assert!(
            (y - frame_y).abs() < 0.05,
            "top edge at {y}, but the frame's top is at {frame_y}"
        );
    }

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