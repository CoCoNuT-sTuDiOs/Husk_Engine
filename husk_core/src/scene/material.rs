
/* How an object's surface looks: its colour and how rough or metallic it
is. The scene owns it; the renderer only keeps a GPU-side copy.
  */

/// How an object's surface looks: its colour and how rough or metallic it
/// is. The scene owns it; the renderer only keeps a GPU-side copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Red, green and blue, each from 0 to 1.
    pub base_color: [f32; 3],
    /// 0 is mirror-smooth, 1 is fully rough.
    pub roughness: f32,
    /// 0 is a non-metal, 1 is a metal.
    pub metalness: f32,
}

impl Default for Material {
    /// Husk orange with the renderer's usual finish: the look the loaded
    /// model has always had.
    fn default() -> Self {
        Material {
            base_color: [0.976, 0.451, 0.086],
            roughness: 0.4,
            metalness: 0.2,
        }
    }
}

impl Material {
    /// The same material with every value held inside its range.
    pub fn clamped(self) -> Material {
        Material {
            base_color: self.base_color.map(|c| c.clamp(0.0, 1.0)),
            roughness: self.roughness.clamp(0.0, 1.0),
            metalness: self.metalness.clamp(0.0, 1.0),
        }
    }

    /// The four values the shader reads: roughness, metalness and two
    /// that are not used yet.
    pub fn shader_values(&self) -> [f32; 4] {
        [self.roughness, self.metalness, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::renderer::OBJECT_COLORS;

    #[test]
    fn the_default_is_the_look_the_loaded_model_always_had() {
        let m = Material::default();
        // The renderer's first palette colour, and its fixed finish.
        assert_eq!(m.base_color, [OBJECT_COLORS[0][0], OBJECT_COLORS[0][1], OBJECT_COLORS[0][2]]);
        assert_eq!(m.shader_values(), [0.4, 0.2, 0.0, 0.0]);
    }

    #[test]
    fn clamped_holds_every_value_in_range_and_leaves_good_ones_alone() {
        let wild = Material {
            base_color: [-0.5, 0.5, 2.0],
            roughness: 3.0,
            metalness: -1.0,
        };
        let held = wild.clamped();
        assert_eq!(held.base_color, [0.0, 0.5, 1.0]);
        assert_eq!(held.roughness, 1.0);
        assert_eq!(held.metalness, 0.0);

        let fine = Material::default();
        assert_eq!(fine.clamped(), fine);
    }
}