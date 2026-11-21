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
//! - `smooth` is `do_wpaint_brush_blur` with `precompute_weight_values`: a touch
//!   moves each vertex toward the average of the faces around it (Mix, at the
//!   falloff times the strength). The averages come from the weights before the
//!   touch: before the stroke, unless it accumulates.
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
use crate::rig::adjacency::Adjacency;
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
            self.touch(weights, vertex, self.mode, self.paint_weight, alpha);
            painted += 1;

        }
        painted
    }
}

impl Stroke {
    /// Moves the weight of this stroke's bone on `vertex` one touch toward
    /// `paint_weight` by `alpha`, as `mode` blends, never undoing a stronger
    /// earlier touch, and makes the other influences give way.
    fn touch(
        &mut self,
        weights: &mut [VertexWeights],
        vertex: usize,
        mode: BlendMode,
        paint_weight: f32,
        alpha: f32,
    ) {
        let snapshot = *self.before[vertex].get_or_insert(weights[vertex]);
        let current = weight_of(&weights[vertex], self.bone);
        let previous = if self.accumulate {
            current
        } else {
            weight_of(&snapshot, self.bone)
        };
        let mut blended = blend_weight(mode, previous, paint_weight, alpha).clamp(0.0, 1.0);
        if blended < ZERO_THRESHOLD {
            blended = 0.0;
        }
        let weight = clamp_monotonic(previous, current, blended);
        assign_weight(&mut weights[vertex], self.bone, weight);
    }

    /// The weight `vertex` had before this touch: before the stroke began, or,
    /// when the stroke accumulates, before the last touch.
    fn weight_before_touch(&self, weights: &[VertexWeights], vertex: usize) -> f32 {
        let known = if self.accumulate {
            &weights[vertex]
        } else {
            self.before[vertex].as_ref().unwrap_or(&weights[vertex])
        };
        weight_of(known, self.bone).clamp(0.0, 1.0)
    }

    /// One touch of the Smooth brush: like `apply`, but every vertex under the
    /// brush moves toward the average weight of the faces around it instead of
    /// toward a weight that is painted. Every average is taken before any
    /// vertex changes, so the order of the vertices does not matter. Returns how
    /// many vertices it changed. Anything that does not fit (a mesh, a stroke
    /// or neighbour lists of another size, or a centre or radius that make no
    /// sense) changes nothing.
    pub fn smooth(
        &mut self,
        weights: &mut [VertexWeights],
        positions: &[[f32; 3]],
        adjacency: &Adjacency,
        centre: [f32; 3],
        radius: f32,
    ) -> usize {
        if weights.len() != positions.len()
            || weights.len() != self.strongest.len()
            || weights.len() != adjacency.vertex_count()
            || !(radius > 0.0)
            || !radius.is_finite()
        {
            return 0;
        }
        let centre = glam::Vec3::from(centre);
        if !centre.is_finite() {
            return 0;
        }
        // First, what each touched vertex averages to...
        let mut touches: Vec<(usize, f32, f32)> = Vec::new();
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
            let corners = adjacency.corners(vertex);
            if corners.is_empty() {
                continue;
            }
            let mut total = 0.0;
            for &corner in corners {
                total += self.weight_before_touch(weights, corner as usize);
            }
            touches.push((vertex, alpha, total / corners.len() as f32));
        }
        // ...then the change.
        for &(vertex, alpha, average) in &touches {
            self.touch(weights, vertex, BlendMode::Mix, average, alpha);
        }
        touches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// A vertex with `bone_two` of bone 2 and the rest on bone 1.
    fn mixed(bone_two: f32) -> VertexWeights {
        VertexWeights {
            joint_indices: [2, 1, 0, 0],
            weights: [bone_two, 1.0 - bone_two, 0.0, 0.0],
        }
    }

    /// Four triangles around vertex 0, with vertices 1 to 4 in a ring around it.
    fn fan() -> (Vec<[f32; 3]>, Vec<u32>) {
        (
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ],
            vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1],
        )
    }

    /// The fan with 0.9 of bone 2 on the middle vertex and none on the ring.
    fn spiky_fan() -> Vec<VertexWeights> {
        vec![mixed(0.9), mixed(0.0), mixed(0.0), mixed(0.0), mixed(0.0)]
    }

    #[test]
    fn smooth_moves_a_vertex_to_the_average_of_the_faces_around_it() {
        let (positions, indices) = fan();
        let adjacency = Adjacency::build(&positions, &indices);
        let mut weights = spiky_fan();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, false);
        // Only the middle vertex is inside the radius.
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5),
            1
        );
        // It counts four times and each ring vertex twice, over 12 corners.
        assert!(close(weight_of(&weights[0], 2), 0.9 * 4.0 / 12.0));
        assert!(close(weight_of(&weights[0], 1), 0.7));
        for ring in 1..5 {
            assert!(close(weight_of(&weights[ring], 2), 0.0));
        }
    }

    #[test]
    fn a_weaker_smooth_only_goes_part_of_the_way() {
        let (positions, indices) = fan();
        let adjacency = Adjacency::build(&positions, &indices);
        let mut weights = spiky_fan();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 0.5, false);
        stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5);
        // Halfway between 0.9 and the average 0.3.
        assert!(close(weight_of(&weights[0], 2), 0.6));
    }

    #[test]
    fn smoothing_the_same_place_again_does_nothing_until_the_stroke_accumulates() {
        let (positions, indices) = fan();
        let adjacency = Adjacency::build(&positions, &indices);

        let mut weights = spiky_fan();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, false);
        stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5);
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5),
            0
        );
        assert!(close(weight_of(&weights[0], 2), 0.3));

        // With accumulate, each touch averages what the last one left.
        let mut weights = spiky_fan();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, true);
        stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5);
        assert!(close(weight_of(&weights[0], 2), 0.3));
        stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5);
        assert!(close(weight_of(&weights[0], 2), 0.3 * 4.0 / 12.0));
    }

    #[test]
    fn every_vertex_in_a_touch_averages_the_weights_from_before_the_touch() {
        let (positions, indices) = fan();
        let adjacency = Adjacency::build(&positions, &indices);
        let mut weights = spiky_fan();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, false);
        // The brush reaches the middle vertex and the whole ring.
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 1.5),
            5
        );
        assert!(close(weight_of(&weights[0], 2), 0.3));
        // Each ring vertex averages the middle vertex's old 0.9 (it is in two of
        // its faces), not the 0.3 it has just been given, so all four match.
        let expected = 0.3 * falloff(1.0, 1.5);
        for ring in 1..5 {
            assert!(close(weight_of(&weights[ring], 2), expected));
        }
    }

    #[test]
    fn smooth_treats_vertices_at_the_same_position_as_one_so_a_seam_stays_closed() {
        // Two triangles; the second has its own copies (4, 5, 6) of the corners
        // at 0, 2 and 3. The copies of the corner at the origin start different.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let adjacency = Adjacency::build(&positions, &[0, 1, 2, 4, 5, 6]);
        let mut weights = vec![
            mixed(0.9),
            mixed(0.0),
            mixed(0.0),
            mixed(0.0),
            mixed(0.3),
            mixed(0.0),
            mixed(0.0),
        ];
        let mut stroke = Stroke::new(7, 2, BlendMode::Mix, 1.0, 1.0, false);
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.5),
            2
        );
        // Both copies average the same six corners: (0.9 + 0.3) / 6.
        assert!(close(weight_of(&weights[0], 2), 0.2));
        assert!(close(weight_of(&weights[4], 2), 0.2));
    }

    #[test]
    fn smooth_with_the_wrong_mesh_or_numbers_that_make_no_sense_changes_nothing() {
        let (positions, indices) = fan();
        let adjacency = Adjacency::build(&positions, &indices);
        let before = spiky_fan();
        let mut weights = before.clone();
        let mut stroke = Stroke::new(5, 2, BlendMode::Mix, 1.0, 1.0, false);
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 0.0),
            0
        );
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &adjacency, [0.0; 3], f32::NAN),
            0
        );
        assert_eq!(
            stroke.smooth(
                &mut weights,
                &positions,
                &adjacency,
                [f32::NAN, 0.0, 0.0],
                1.0
            ),
            0
        );
        // Neighbour lists made for another mesh, and a stroke made for another size.
        let small = Adjacency::build(&positions[..3], &[0, 1, 2]);
        assert_eq!(
            stroke.smooth(&mut weights, &positions, &small, [0.0; 3], 1.0),
            0
        );
        let mut other = Stroke::new(9, 2, BlendMode::Mix, 1.0, 1.0, false);
        assert_eq!(
            other.smooth(&mut weights, &positions, &adjacency, [0.0; 3], 1.0),
            0
        );
        for vertex in 0..5 {
            assert_eq!(weights[vertex].weights, before[vertex].weights);
            assert_eq!(weights[vertex].joint_indices, before[vertex].joint_indices);
        }
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