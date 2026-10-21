/// The space around the objects: the colour behind them and how strong the
/// soft studio light on their surfaces and its reflections are. The scene
/// owns it; the renderer only keeps a GPU-side copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    /// The background colour: red, green and blue, each from 0 to 1.
    pub background: [f32; 3],
    /// How strong the soft light all around the scene is. 1 is the usual
    /// look, 0 leaves surfaces lit only by the scene's lights.
    pub ambient: f32,
    /// How strong the studio reflections are. 1 is the usual look, 0
    /// switches them off.
    pub reflections: f32,
}

impl World {
    /// The top of the range for the ambient and reflection strengths.
    pub const MAX_STRENGTH: f32 = 2.0;

    /// The same world with every value held inside its range.
    pub fn clamped(self) -> World {
        World {
            background: self.background.map(|c| c.clamp(0.0, 1.0)),
            ambient: self.ambient.clamp(0.0, Self::MAX_STRENGTH),
            reflections: self.reflections.clamp(0.0, Self::MAX_STRENGTH),
        }
    }

    /// The four values the shader reads: ambient, reflections and two that
    /// are not used yet. The background is not a shader value: it is the
    /// colour the frame is cleared to.
    pub fn shader_values(&self) -> [f32; 4] {
        [self.ambient, self.reflections, 0.0, 0.0]
    }
}

impl Default for World {
    /// The dark blue-grey background and the full studio light the
    /// renderer has always had.
    fn default() -> Self {
        World {
            background: [0.05, 0.05, 0.08],
            ambient: 1.0,
            reflections: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_look_the_renderer_always_had() {
        let world = World::default();
        assert_eq!(world.background, [0.05, 0.05, 0.08]);
        assert_eq!(world.shader_values(), [1.0, 1.0, 0.0, 0.0]);
        assert_eq!(world.clamped(), world);
    }

    #[test]
    fn clamped_holds_every_value_in_range() {
        let wild = World {
            background: [-1.0, 0.5, 4.0],
            ambient: 9.0,
            reflections: -3.0,
        };
        let held = wild.clamped();
        assert_eq!(held.background, [0.0, 0.5, 1.0]);
        assert_eq!(held.ambient, World::MAX_STRENGTH);
        assert_eq!(held.reflections, 0.0);
    }

    #[test]
    fn the_shader_values_carry_ambient_then_reflections() {
        let world = World {
            background: [0.0, 0.0, 0.0],
            ambient: 0.25,
            reflections: 1.5,
        };
        assert_eq!(world.shader_values(), [0.25, 1.5, 0.0, 0.0]);
    }
}