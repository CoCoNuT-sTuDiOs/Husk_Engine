//! Weight painting math: how one brush touch changes the weights of one vertex.
//!
//! Ported from Blender (GPL-2.0-or-later):
//! - `blend_weight` is `ED_wpaint_blend_tool` with `wval_blend`, `wval_add` and
//!   `wval_sub` (the weight paint utilities, paint_vertex_weight_utils.cc,
//!   SPDX-FileCopyrightText: 2023 Blender Authors,
//!   SPDX-License-Identifier: GPL-2.0-or-later).
//! - `normalize_with_lock` is `BKE_defvert_normalize_ex` (deform.cc,
//!   SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All rights reserved.,
//!   SPDX-License-Identifier: GPL-2.0-or-later) for the case weight painting
//!   uses: one locked group, no soft locks, no subset.
//! - `paint_vertex` follows `gradientVert_update` and the Auto Normalize part of
//!   `paint_weight_gradient_exec` (paint_vertex_weight_ops.cc,
//!   SPDX-FileCopyrightText: 2023 Blender Authors,
//!   SPDX-License-Identifier: GPL-2.0-or-later): blend the weight, clamp it to
//!   0..1, then normalize the vertex with the painted group locked, so the other
//!   groups make room for it.
//!
//! Husk differs in three ways: a vertex holds at most 4 influences (the GPU
//! vertex has 4 slots, like glTF), an influence with weight 0 counts as
//! unassigned (Blender's own comment says zero-weight groups are treated like
//! unassigned ones when normalizing), and Auto Normalize is always on.

use crate::rig::skinning::VertexWeights;

/// How a brush touch combines the weight it paints with what the vertex has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// The painted weight is a target: the vertex moves toward it by `alpha`.
    Mix,
    /// Adds the painted weight times `alpha`.
    Add,
    /// Takes away the painted weight times `alpha`.
    Subtract,
}

/// The weight a vertex has after one brush touch, before it is clamped and
/// normalized: `weight` is what it had, `paint` the weight being painted and
/// `alpha` how much of it this touch applies (strength times falloff).
pub fn blend_weight(mode: BlendMode, weight: f32, paint: f32, alpha: f32) -> f32 {
    match mode {
        BlendMode::Mix => {
            // Blending with an alpha over 1 does not make sense.
            let alpha = alpha.min(1.0);
            (paint * alpha) + (weight * (1.0 - alpha))
        }
        BlendMode::Add => weight + (paint * alpha),
        BlendMode::Subtract => weight - (paint * alpha),
    }
}

impl BlendMode {
    /// Every blend mode, in the order a menu lists them.
    pub const ALL: [BlendMode; 3] = [BlendMode::Mix, BlendMode::Add, BlendMode::Subtract];

    /// The name the bridge and the brush strip use.
    pub fn name(self) -> &'static str {
        match self {
            BlendMode::Mix => "mix",
            BlendMode::Add => "add",
            BlendMode::Subtract => "subtract",
        }
    }

    /// The blend mode with this name, or None for any other name.
    pub fn from_name(name: &str) -> Option<BlendMode> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// The weight `bone` has on this vertex: 0 when it has none.
pub fn weight_of(vertex: &VertexWeights, bone: u32) -> f32 {
    let mut total = 0.0;
    for slot in 0..vertex.weights.len() {
        if vertex.joint_indices[slot] == bone {
            total += vertex.weights[slot];
        }
    }
    total
}

/// Gives `bone` the weight `weight` on this vertex. A bone that has no slot
/// takes a free one; with no free slot it takes the place of the weakest
/// influence, but only if it is stronger than that one. A weight of 0 frees the
/// bone's slot.
fn set_weight(vertex: &mut VertexWeights, bone: u32, weight: f32) {
    let slots = vertex.weights.len();
    let own = (0..slots)
        .find(|&slot| vertex.weights[slot] > 0.0 && vertex.joint_indices[slot] == bone);
    if weight <= 0.0 {
        if let Some(slot) = own {
            vertex.weights[slot] = 0.0;
        }
        return;
    }
    let slot = match own {
        Some(slot) => slot,
        None => match (0..slots).find(|&slot| vertex.weights[slot] <= 0.0) {
            Some(free) => free,
            None => {
                let weakest = (0..slots)
                    .min_by(|&a, &b| vertex.weights[a].total_cmp(&vertex.weights[b]))
                    .unwrap_or(0);
                if weight <= vertex.weights[weakest] {
                    return;
                }
                weakest
            }
        },
    };
    vertex.joint_indices[slot] = bone;
    vertex.weights[slot] = weight;
}

/// Scales every influence except `locked` so that all of them add up to 1
/// again, leaving the locked bone's weight as it is. When nothing but the
/// locked bone has any weight there is nothing to scale and the vertex is left
/// as it is (so its weights may add up to less than 1), as in Blender.
fn normalize_with_lock(vertex: &mut VertexWeights, locked: u32) {
    let slots = vertex.weights.len();
    let mut locked_total = 0.0;
    let mut regular_total = 0.0;
    for slot in 0..slots {
        let weight = vertex.weights[slot];
        if weight <= 0.0 {
            continue;
        }
        if vertex.joint_indices[slot] == locked {
            locked_total += weight;
        } else {
            regular_total += weight;
        }
    }
    if regular_total <= 0.0 {
        return;
    }
    let available = (1.0 - locked_total).max(0.0);
    let scale = available / regular_total;
    for slot in 0..slots {
        if vertex.weights[slot] > 0.0 && vertex.joint_indices[slot] != locked {
            vertex.weights[slot] = (vertex.weights[slot] * scale).clamp(0.0, 1.0);
        }
    }
}

/// Gives `bone` exactly the weight `weight` (held between 0 and 1) on this
/// vertex, and makes the other influences give way so the vertex adds up to 1
/// again. A weight that is not a finite number changes nothing.
pub fn assign_weight(vertex: &mut VertexWeights, bone: u32, weight: f32) {
    if !weight.is_finite() {
        return;
    }
    set_weight(vertex, bone, weight.clamp(0.0, 1.0));
    normalize_with_lock(vertex, bone);
}

/// One brush touch on one vertex: paints `bone` with `paint_weight` (0 to 1) at
/// strength `alpha` (the brush strength times the falloff at this vertex),
/// then makes the other influences give way so the vertex adds up to 1. A
/// vertex the touch does not reach (alpha 0), or numbers that are not finite,
/// leave it exactly as it is.
pub fn paint_vertex(
    vertex: &mut VertexWeights,
    bone: u32,
    mode: BlendMode,
    paint_weight: f32,
    alpha: f32,
) {
    if !(paint_weight.is_finite() && alpha.is_finite()) || alpha <= 0.0 {
        return;
    }
    let paint_weight = paint_weight.clamp(0.0, 1.0);
    let blended = blend_weight(mode, weight_of(vertex, bone), paint_weight, alpha).clamp(0.0, 1.0);
    assign_weight(vertex, bone, blended);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn vertex(joints: [u32; 4], weights: [f32; 4]) -> VertexWeights {
        VertexWeights {
            joint_indices: joints,
            weights,
        }
    }

    fn total(vertex: &VertexWeights) -> f32 {
        vertex.weights.iter().sum()
    }

    #[test]
    fn mix_moves_the_weight_toward_the_painted_one_by_alpha() {
        assert!(close(blend_weight(BlendMode::Mix, 0.2, 1.0, 0.5), 0.6));
        // Full strength paints the target at once, none changes nothing.
        assert!(close(blend_weight(BlendMode::Mix, 0.2, 1.0, 1.0), 1.0));
        assert!(close(blend_weight(BlendMode::Mix, 0.2, 1.0, 0.0), 0.2));
        // An alpha over 1 counts as 1.
        assert!(close(blend_weight(BlendMode::Mix, 0.2, 1.0, 3.0), 1.0));
        // Painting weight 0 with Mix pulls toward 0.
        assert!(close(blend_weight(BlendMode::Mix, 0.8, 0.0, 0.5), 0.4));
    }

    #[test]
    fn add_and_subtract_use_the_painted_weight_times_alpha() {
        assert!(close(blend_weight(BlendMode::Add, 0.3, 0.5, 0.5), 0.55));
        assert!(close(blend_weight(BlendMode::Subtract, 0.3, 0.5, 0.5), 0.05));
        // Painting weight 0 with Add does nothing.
        assert!(close(blend_weight(BlendMode::Add, 0.3, 0.0, 1.0), 0.3));
        // The blend itself does not clamp.
        assert!(close(blend_weight(BlendMode::Add, 0.9, 1.0, 0.5), 1.4));
        assert!(close(blend_weight(BlendMode::Subtract, 0.1, 1.0, 0.5), -0.4));
    }

    #[test]
    fn weight_of_reads_the_bone_and_ignores_empty_slots() {
        let v = vertex([2, 7, 5, 0], [0.6, 0.0, 0.4, 0.0]);
        assert!(close(weight_of(&v, 2), 0.6));
        assert!(close(weight_of(&v, 5), 0.4));
        assert!(close(weight_of(&v, 7), 0.0));
        assert!(close(weight_of(&v, 9), 0.0));
    }

    #[test]
    fn a_bone_painted_for_the_first_time_takes_a_free_slot_and_the_others_make_room() {
        let mut v = vertex([1, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
        paint_vertex(&mut v, 2, BlendMode::Mix, 1.0, 0.25);
        assert!(close(weight_of(&v, 2), 0.25));
        assert!(close(weight_of(&v, 1), 0.75));
        assert!(close(total(&v), 1.0));
    }

    #[test]
    fn painting_to_full_takes_everything_from_the_others() {
        let mut v = vertex([1, 2, 0, 0], [0.7, 0.3, 0.0, 0.0]);
        paint_vertex(&mut v, 2, BlendMode::Mix, 1.0, 1.0);
        assert!(close(weight_of(&v, 2), 1.0));
        assert!(close(weight_of(&v, 1), 0.0));
    }

    #[test]
    fn painting_zero_gives_the_weight_back_to_the_others() {
        let mut v = vertex([1, 2, 0, 0], [0.6, 0.4, 0.0, 0.0]);
        paint_vertex(&mut v, 2, BlendMode::Mix, 0.0, 1.0);
        assert!(close(weight_of(&v, 2), 0.0));
        assert!(close(weight_of(&v, 1), 1.0));
    }

    #[test]
    fn a_vertex_with_only_the_painted_bone_keeps_the_painted_weight() {
        // Nothing else to scale, so the weights may add up to less than 1.
        let mut v = vertex([0, 0, 0, 0], [0.0, 0.0, 0.0, 0.0]);
        paint_vertex(&mut v, 3, BlendMode::Add, 1.0, 0.5);
        assert!(close(weight_of(&v, 3), 0.5));
        assert!(close(total(&v), 0.5));
    }

    #[test]
    fn with_four_influences_a_stronger_new_bone_replaces_the_weakest() {
        let mut v = vertex([1, 2, 3, 4], [0.4, 0.3, 0.2, 0.1]);
        paint_vertex(&mut v, 9, BlendMode::Mix, 1.0, 0.5);
        assert!(close(weight_of(&v, 9), 0.5));
        assert!(close(weight_of(&v, 4), 0.0));
        // The other three share the remaining half in the same proportions.
        assert!(close(weight_of(&v, 1), 0.4 * 0.5 / 0.9));
        assert!(close(weight_of(&v, 2), 0.3 * 0.5 / 0.9));
        assert!(close(weight_of(&v, 3), 0.2 * 0.5 / 0.9));
        assert!(close(total(&v), 1.0));
    }

    #[test]
    fn a_weaker_new_bone_does_not_push_out_a_stronger_one() {
        let mut v = vertex([1, 2, 3, 4], [0.4, 0.3, 0.2, 0.1]);
        paint_vertex(&mut v, 9, BlendMode::Mix, 1.0, 0.05);
        assert!(close(weight_of(&v, 9), 0.0));
        assert!(close(weight_of(&v, 4), 0.1));
        assert!(close(total(&v), 1.0));
    }

    #[test]
    fn subtract_cannot_go_below_zero_and_the_others_take_the_weight() {
        let mut v = vertex([1, 2, 0, 0], [0.2, 0.8, 0.0, 0.0]);
        paint_vertex(&mut v, 1, BlendMode::Subtract, 1.0, 0.5);
        assert!(close(weight_of(&v, 1), 0.0));
        assert!(close(weight_of(&v, 2), 1.0));
    }

    #[test]
    fn weights_stay_inside_zero_and_one_and_untouched_or_bad_input_changes_nothing() {
        let mut v = vertex([1, 0, 0, 0], [0.9, 0.0, 0.0, 0.0]);
        paint_vertex(&mut v, 1, BlendMode::Add, 1.0, 0.5);
        assert!(close(weight_of(&v, 1), 1.0));

        let before = vertex([1, 2, 0, 0], [0.6, 0.4, 0.0, 0.0]);
        for (paint, alpha) in [(1.0, 0.0), (1.0, f32::NAN), (f32::NAN, 0.5), (1.0, -1.0)] {
            let mut v = before;
            paint_vertex(&mut v, 2, BlendMode::Mix, paint, alpha);
            assert_eq!(v.joint_indices, before.joint_indices);
            assert_eq!(v.weights, before.weights);
        }
    }
}