//! The weight paint brush: how one stroke of the brush changes the weights of
//! the vertices it passes over.
//!
//! Ported from Blender's weight paint stroke code (`do_wpaint_brush_draw`,
//! `do_weight_paint_vertex_single`, `wpaint_blend` and `wpaint_clamp_monotonic`;
//! SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All rights reserved.,
//! SPDX-License-Identifier: GPL-2.0-or-later):
//! - A touch gives each vertex a strength: the falloff at its distance from the
//!   brush times the brush strength.
//! - Unless the stroke accumulates, a vertex is only painted again when a touch is
//!   stronger than every earlier touch of this stroke, and every touch blends from
//!   the weights the vertex had when the stroke began.
//! - A painted weight under 0.0001 counts as 0, so painting to exactly 0 is not
//!   tedious.
//! - `clamp_monotonic` stops a weaker later touch from undoing a stronger one.
//!
//! - `falloff` is `BKE_brush_curve_strength` for the Smooth preset, the falloff
//!   curve every Blender brush starts with (brush.cc,
//!   SPDX-FileCopyrightText: 2023 Blender Authors,
//!   SPDX-License-Identifier: GPL-2.0-or-later). The other presets (Sharp, Root,
//!   Linear and so on) are left out for now.
//!
//! Husk leaves out, for now: the brush hardness setting, the front-face test,
//! pen pressure and mirroring. The brush is a sphere.
//! 

use crate::rig::paint::{assign_weight, blend_weight, weight_of, BlendMode};
use crate::rig::skinning::VertexWeights;

/// A painted weight under this counts as 0.
const ZERO_THRESHOLD: f32 = 0.0001;

/// How much of the brush's strength reaches a vertex `distance` from the centre
/// of a brush of `radius`: 1 at the centre, easing down to 0 at the edge, and 0
/// beyond it (and for numbers that make no sense).
pub fn falloff(distance: f32, radius: f32) -> f32 {
    if !(radius > 0.0) || !radius.is_finite() || !(distance >= 0.0) || distance >= radius {
        return 0.0;
    }
    let closeness = 1.0 - distance / radius;
    closeness * closeness * (3.0 - 2.0 * closeness)
}

/// The weight a vertex ends up with: `new`, but never moving back past
/// `current` against the direction the stroke is pushing from `old`.
fn clamp_monotonic(old: f32, current: f32, new: f32) -> f32 {
    if new < old {
        new.min(current)
    } else if new > old {
        new.max(current)
    } else {
        new
    }
}

fn sane(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// One stroke of the weight brush, from the mouse going down to it coming up.
pub struct Stroke {
    bone: u32,
    mode: BlendMode,
    paint_weight: f32,
    strength: f32,
    accumulate: bool,
    /// What each vertex had when the stroke first reached it.
    before: Vec<Option<VertexWeights>>,
    /// The strongest touch each vertex has had in this stroke.
    strongest: Vec<f32>,
}

impl Stroke {
    /// Starts a stroke on a mesh of `vertex_count` vertices that paints `bone`
    /// with `paint_weight` (0 to 1) at `strength` (0 to 1). With `accumulate`
    /// on, every touch adds to the last, so staying in one place builds up.
    pub fn new(
        vertex_count: usize,
        bone: u32,
        mode: BlendMode,
        paint_weight: f32,
        strength: f32,
        accumulate: bool,
    ) -> Stroke {
        Stroke {
            bone,
            mode,
            paint_weight: sane(paint_weight),
            strength: sane(strength),
            accumulate,
            before: vec![None; vertex_count],
            strongest: vec![0.0; vertex_count],
        }
    }

    /// One touch of the brush: its centre is `centre` (in the same space as
    /// `positions`) and it reaches `radius`. Returns how many vertices it
    /// painted. A mesh that does not match the stroke, or a centre or radius
    /// that make no sense, change nothing.
    pub fn apply(
        &mut self,
        weights: &mut [VertexWeights],
        positions: &[[f32; 3]],
        centre: [f32; 3],
        radius: f32,
    ) -> usize {
        if weights.len() != positions.len()
            || weights.len() != self.strongest.len()
            || !(radius > 0.0)
            || !radius.is_finite()
        {
            return 0;
        }
        let centre = glam::Vec3::from(centre);
        if !centre.is_finite() {
            return 0;
        }
        let mut painted = 0;
        for vertex in 0..weights.len() {
            let distance = (glam::Vec3::from(positions[vertex]) - centre).length();
            let alpha = falloff(distance, radius) * self.strength;
            if alpha <= 0.0 {
                continue;
            }
            if !self.accumulate {
                if self.strongest[vertex] < alpha {
                    self.strongest[vertex] = alpha;
                } else {
                    continue;
                }
            }
            let snapshot = *self.before[vertex].get_or_insert(weights[vertex]);
            let current = weight_of(&weights[vertex], self.bone);
            let previous = if self.accumulate {
                current
            } else {
                weight_of(&snapshot, self.bone)
            };
            let mut blended =
                blend_weight(self.mode, previous, self.paint_weight, alpha).clamp(0.0, 1.0);
            if blended < ZERO_THRESHOLD {
                blended = 0.0;
            }
            let weight = clamp_monotonic(previous, current, blended);
            assign_weight(&mut weights[vertex], self.bone, weight);
            painted += 1;
        }
        painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn on_bone_one() -> VertexWeights {
        VertexWeights {
            joint_indices: [1, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn the_falloff_is_full_at_the_centre_gone_at_the_edge_and_never_stronger_farther_out() {
        assert!(close(falloff(0.0, 1.0), 1.0));
        assert!(close(falloff(0.5, 1.0), 0.5));
        assert_eq!(falloff(1.0, 1.0), 0.0);
        assert_eq!(falloff(3.0, 1.0), 0.0);
        let mut last = 0.0;
        for step in (0..=100).rev() {
            let strength = falloff(step as f32 / 100.0, 1.0);
            assert!(strength >= last - 1e-6);
            last = strength;
        }
        assert_eq!(falloff(-1.0, 1.0), 0.0);
        assert_eq!(falloff(0.5, 0.0), 0.0);
        assert_eq!(falloff(0.5, -1.0), 0.0);
        assert_eq!(falloff(f32::NAN, 1.0), 0.0);
    }

    #[test]
    fn a_touch_paints_inside_the_radius_by_the_falloff_and_leaves_the_rest_alone() {
        let positions = [[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let mut weights = [on_bone_one(), on_bone_one(), on_bone_one()];
        let mut stroke = Stroke::new(3, 2, BlendMode::Mix, 1.0, 1.0, false);
        let painted = stroke.apply(&mut weights, &positions, [0.0, 0.0, 0.0], 1.0);
        assert_eq!(painted, 2);
        // At the centre the target is reached, halfway out half of it, and the
        // other bone gives way each time.
        assert!(close(weight_of(&weights[0], 2), 1.0));
        assert!(close(weight_of(&weights[0], 1), 0.0));
        assert!(close(weight_of(&weights[1], 2), 0.5));
        assert!(close(weight_of(&weights[1], 1), 0.5));
        // Out of reach: exactly as it was.
        assert_eq!(weights[2].weights, on_bone_one().weights);
        assert_eq!(weights[2].joint_indices, on_bone_one().joint_indices);
    }

    #[test]
    fn touching_the_same_place_again_at_the_same_strength_builds_nothing() {
        let positions = [[0.0, 0.0, 0.0]];
        let mut weights = [on_bone_one()];
        let mut stroke = Stroke::new(1, 2, BlendMode::Mix, 1.0, 0.5, false);
        assert_eq!(stroke.apply(&mut weights, &positions, [0.0; 3], 1.0), 1);
        assert!(close(weight_of(&weights[0], 2), 0.5));
        assert_eq!(stroke.apply(&mut weights, &positions, [0.0; 3], 1.0), 0);
        assert!(close(weight_of(&weights[0], 2), 0.5));
    }

    #[test]
    fn a_stronger_touch_later_in_the_stroke_raises_the_weight_to_its_own_level() {
        let positions = [[0.5, 0.0, 0.0]];
        let mut weights = [on_bone_one()];
        let mut stroke = Stroke::new(1, 2, BlendMode::Mix, 1.0, 1.0, false);
        // Half a radius from the vertex: half strength.
        stroke.apply(&mut weights, &positions, [1.0, 0.0, 0.0], 1.0);
        assert!(close(weight_of(&weights[0], 2), 0.5));
        // Right on it: full strength, blended from the weights before the stroke.
        stroke.apply(&mut weights, &positions, [0.5, 0.0, 0.0], 1.0);
        assert!(close(weight_of(&weights[0], 2), 1.0));
    }

    #[test]
    fn with_accumulate_every_touch_adds_to_the_last() {
        let positions = [[0.0, 0.0, 0.0]];
        let mut weights = [on_bone_one()];
        let mut stroke = Stroke::new(1, 2, BlendMode::Mix, 1.0, 0.5, true);
        stroke.apply(&mut weights, &positions, [0.0; 3], 1.0);
        assert!(close(weight_of(&weights[0], 2), 0.5));
        stroke.apply(&mut weights, &positions, [0.0; 3], 1.0);
        assert!(close(weight_of(&weights[0], 2), 0.75));
    }

    #[test]
    fn a_weight_that_would_end_up_tiny_becomes_exactly_zero() {
        let positions = [[0.0, 0.0, 0.0]];
        let mut weights = [VertexWeights {
            joint_indices: [2, 1, 0, 0],
            weights: [0.0002, 0.9998, 0.0, 0.0],
        }];
        let mut stroke = Stroke::new(1, 2, BlendMode::Mix, 0.0, 0.75, false);
        stroke.apply(&mut weights, &positions, [0.0; 3], 1.0);
        assert_eq!(weight_of(&weights[0], 2), 0.0);
        assert!(close(weight_of(&weights[0], 1), 1.0));
    }

    #[test]
    fn a_weaker_later_touch_never_undoes_a_stronger_one() {
        assert!(close(clamp_monotonic(0.5, 0.7, 0.6), 0.7));
        assert!(close(clamp_monotonic(0.5, 0.2, 0.4), 0.2));
        assert!(close(clamp_monotonic(0.5, 0.3, 0.5), 0.5));
        assert!(close(clamp_monotonic(0.5, 0.9, 0.8), 0.9));
        assert!(close(clamp_monotonic(0.5, 0.1, 0.6), 0.6));
    }

    #[test]
    fn a_mesh_that_does_not_match_or_numbers_that_make_no_sense_change_nothing() {
        let positions = [[0.0, 0.0, 0.0], [0.1, 0.0, 0.0]];
        let before = [on_bone_one(), on_bone_one()];
        let mut stroke = Stroke::new(2, 2, BlendMode::Mix, 1.0, 1.0, false);

        let mut weights = before;
        assert_eq!(stroke.apply(&mut weights, &positions[..1], [0.0; 3], 1.0), 0);
        assert_eq!(stroke.apply(&mut weights, &positions, [0.0; 3], 0.0), 0);
        assert_eq!(stroke.apply(&mut weights, &positions, [0.0; 3], f32::NAN), 0);
        assert_eq!(
            stroke.apply(&mut weights, &positions, [f32::NAN, 0.0, 0.0], 1.0),
            0
        );
        for vertex in 0..2 {
            assert_eq!(weights[vertex].weights, before[vertex].weights);
            assert_eq!(weights[vertex].joint_indices, before[vertex].joint_indices);
        }

        // A stroke made for another mesh size refuses this one too.
        let mut other = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, false);
        assert_eq!(other.apply(&mut weights, &positions, [0.0; 3], 1.0), 0);
    }
}