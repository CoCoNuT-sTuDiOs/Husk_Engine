/// What the finished video looks like: its size in pixels and its frame
/// rate. It belongs to the scene, because the shot is framed for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Output {
    /// The smallest and largest width or height, in pixels.
    pub const MIN_SIDE: u32 = 16;
    pub const MAX_SIDE: u32 = 7680;
    /// The highest frame rate.
    pub const MAX_FPS: u32 = 120;

    /// The same settings held inside their ranges, with the width and
    /// height rounded down to even numbers (video encoders need them even).
    pub fn clamped(self) -> Output {
        Output {
            width: self.width.clamp(Self::MIN_SIDE, Self::MAX_SIDE) & !1,
            height: self.height.clamp(Self::MIN_SIDE, Self::MAX_SIDE) & !1,
            fps: self.fps.clamp(1, Self::MAX_FPS),
        }
    }

    /// The frame's shape: width divided by height.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

impl Default for Output {
    fn default() -> Self {
        Output {
            width: 1920,
            height: 1080,
            fps: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_output_is_full_hd_at_30_frames_and_already_valid() {
        let output = Output::default();
        assert_eq!((output.width, output.height, output.fps), (1920, 1080, 30));
        assert_eq!(output.clamped(), output);
    }

    #[test]
    fn clamped_holds_every_value_in_range_and_makes_the_sides_even() {
        let held = Output { width: 5, height: 99_999, fps: 0 }.clamped();
        assert_eq!(held.width, 16);
        assert_eq!(held.height, 7680);
        assert_eq!(held.fps, 1);

        let odd = Output { width: 1081, height: 721, fps: 500 }.clamped();
        assert_eq!((odd.width, odd.height, odd.fps), (1080, 720, 120));
    }

    #[test]
    fn aspect_is_width_over_height() {
        assert!((Output::default().aspect() - 16.0 / 9.0).abs() < 1e-6);
        let vertical = Output { width: 1080, height: 1920, fps: 30 };
        assert!((vertical.aspect() - 9.0 / 16.0).abs() < 1e-6);
    }
}