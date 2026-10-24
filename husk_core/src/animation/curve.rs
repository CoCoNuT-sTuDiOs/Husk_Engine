
//! By Darwin.
//! Smooth motion between keys.
//!
//! Between two keys a value follows a cubic Hermite curve: the same shape as
//! a Bezier curve whose handles sit a third of the way across the gap. What
//! decides how it looks is the slope the curve has as it passes through each
//! key, and that slope follows the rules of Blender's auto-clamped handles:
//!
//! - at a peak, a valley or beside a hold the curve is level, so it eases
//!   there and never overshoots;
//! - anywhere else the slope is the mean of the slopes of the two segments
//!   that meet at the key, so speed carries on through the key instead of
//!   changing direction with a corner;
//! - the first and last key are level (ease in and out). When the animation
//!   repeats, the keys either side of the seam count as neighbours instead.
//!
//! Everything here is plain maths with no knowledge of scenes or objects.

use glam::Quat;

/// One keyed moment of `N` numbers: its time (seconds) and its values.
pub type Key<const N: usize> = (f32, [f32; N]);

/// How steeply (value per second) the curve runs as it passes through `key`
/// (time, value), given the key before it and the key after it. A missing
/// neighbour (the first or last key of an animation that does not repeat)
/// means the curve is level there.
pub fn smooth_slope(
    before: Option<(f32, f32)>,
    key: (f32, f32),
    after: Option<(f32, f32)>,
) -> f32 {
    let (Some(before), Some(after)) = (before, after) else {
        return 0.0;
    };
    let gap_before = key.0 - before.0;
    let gap_after = after.0 - key.0;
    if gap_before <= 0.0 || gap_after <= 0.0 {
        return 0.0;
    }
    let slope_before = (key.1 - before.1) / gap_before;
    let slope_after = (after.1 - key.1) / gap_after;
    // A peak, a valley or a hold: stay level there.
    if slope_before * slope_after <= 0.0 {
        return 0.0;
    }
    let mean = 0.5 * (slope_before + slope_after);
    // Never steeper than three times the gentler neighbour, which is what
    // keeps the curve from overshooting the next key.
    let limit = 3.0 * slope_before.abs().min(slope_after.abs());
    mean.max(-limit).min(limit)
}

/// The value at `time` on the curve from (`t0`, `v0`) leaving at slope `m0`
/// to (`t1`, `v1`) arriving at slope `m1`. Times outside the two keys are
/// held to the nearest one.
pub fn hermite(t0: f32, v0: f32, m0: f32, t1: f32, v1: f32, m1: f32, time: f32) -> f32 {
    let gap = t1 - t0;
    if gap <= 0.0 {
        return v0;
    }
    let s = ((time - t0) / gap).clamp(0.0, 1.0);
    let s2 = s * s;
    let s3 = s2 * s;
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    h00 * v0 + h10 * gap * m0 + h01 * v1 + h11 * gap * m1
}

/// The `N` numbers at `time` on the smooth curve from key `a` to key `b`.
/// `before` is the key ahead of `a` and `after` the key past `b` (None where
/// the animation ends), which decide the slopes the curve has at `a` and `b`.
pub fn smooth_between<const N: usize>(
    before: Option<Key<N>>,
    a: Key<N>,
    b: Key<N>,
    after: Option<Key<N>>,
    time: f32,
) -> [f32; N] {
    let mut out = [0.0_f32; N];
    for i in 0..N {
        let slope_a = smooth_slope(
            before.map(|key| (key.0, key.1[i])),
            (a.0, a.1[i]),
            Some((b.0, b.1[i])),
        );
        let slope_b = smooth_slope(
            Some((a.0, a.1[i])),
            (b.0, b.1[i]),
            after.map(|key| (key.0, key.1[i])),
        );
        out[i] = hermite(a.0, a.1[i], slope_a, b.0, b.1[i], slope_b, time);
    }
    out
}

/// `q`, or its negative if that sits on the same side as `reference`. A
/// rotation and its negative are the same turn, and blending numbers from
/// opposite sides would go the long way round.
fn same_side(reference: Quat, q: Quat) -> Quat {
    if reference.dot(q) < 0.0 {
        -q
    } else {
        q
    }
}

/// The rotation at `time` on the smooth curve from rotation `a` to rotation
/// `b`, each with its time, and `before` and `after` as in `smooth_between`.
/// The four numbers of the rotation are smoothed and the result is made a
/// proper rotation again. Every segment takes the short way round.
pub fn smooth_rotation(
    before: Option<(f32, Quat)>,
    a: (f32, Quat),
    b: (f32, Quat),
    after: Option<(f32, Quat)>,
    time: f32,
) -> Quat {
    let b_rotation = same_side(a.1, b.1);
    let before = before.map(|(t, q)| (t, same_side(a.1, q).to_array()));
    let after = after.map(|(t, q)| (t, same_side(b_rotation, q).to_array()));
    let blended = smooth_between::<4>(
        before,
        (a.0, a.1.to_array()),
        (b.0, b_rotation.to_array()),
        after,
        time,
    );
    let rotation = Quat::from_array(blended);
    if rotation.is_finite() && rotation.length_squared() > 1e-12 {
        rotation.normalize()
    } else {
        a.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_in_a_steady_run_takes_the_mean_of_the_two_slopes() {
        // Slopes of 1 and 2 meet at the key: the curve runs through it at 1.5.
        let slope = smooth_slope(Some((0.0, 0.0)), (1.0, 1.0), Some((2.0, 3.0)));
        assert!((slope - 1.5).abs() < 1e-6);
        // Uneven gaps do not matter, only the slopes: 2 a second, then 4.
        let slope = smooth_slope(Some((0.0, 0.0)), (1.0, 2.0), Some((1.5, 4.0)));
        assert!((slope - 3.0).abs() < 1e-6);
    }

    #[test]
    fn a_peak_a_valley_a_hold_and_the_ends_stay_level() {
        assert_eq!(smooth_slope(Some((0.0, 0.0)), (1.0, 5.0), Some((2.0, 0.0))), 0.0);
        assert_eq!(smooth_slope(Some((0.0, 5.0)), (1.0, 0.0), Some((2.0, 5.0))), 0.0);
        // A key beside a hold (an equal value) eases into it.
        assert_eq!(smooth_slope(Some((0.0, 2.0)), (1.0, 2.0), Some((2.0, 9.0))), 0.0);
        // The first and the last key of an animation that does not repeat.
        assert_eq!(smooth_slope(None, (0.0, 1.0), Some((1.0, 9.0))), 0.0);
        assert_eq!(smooth_slope(Some((0.0, 1.0)), (1.0, 9.0), None), 0.0);
    }

    #[test]
    fn the_slope_is_held_to_three_times_the_gentler_neighbour() {
        // 1 a second in and 10 a second out: the mean of 5.5 is held to 3.
        let slope = smooth_slope(Some((0.0, 0.0)), (1.0, 1.0), Some((2.0, 11.0)));
        assert!((slope - 3.0).abs() < 1e-6);
        // Falling the same way.
        let slope = smooth_slope(Some((0.0, 0.0)), (1.0, -1.0), Some((2.0, -11.0)));
        assert!((slope + 3.0).abs() < 1e-6);
    }

    #[test]
    fn the_curve_passes_through_both_keys_and_matching_slopes_make_a_straight_line() {
        assert_eq!(hermite(1.0, 5.0, 0.7, 3.0, 9.0, -2.0, 1.0), 5.0);
        assert_eq!(hermite(1.0, 5.0, 0.7, 3.0, 9.0, -2.0, 3.0), 9.0);
        // 2 units a second all the way: a quarter of the way in time is a
        // quarter of the way in value, and halfway is halfway.
        assert!((hermite(0.0, 0.0, 2.0, 1.0, 2.0, 2.0, 0.25) - 0.5).abs() < 1e-6);
        assert!((hermite(0.0, 0.0, 2.0, 1.0, 2.0, 2.0, 0.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_curve_beside_a_hold_does_not_overshoot_it() {
        // A steep rise to 10, then a tiny creep up to 10.2. Using the plain
        // mean of the slopes would swing the curve above 10.2 in between.
        let before = Some((0.0, [0.0]));
        let a = (1.0, [10.0]);
        let b = (2.0, [10.2]);
        let after = Some((3.0, [10.4]));
        for step in 0..=20 {
            let time = 1.0 + step as f32 / 20.0;
            let [value] = smooth_between(before, a, b, after, time);
            assert!(
                value >= 10.0 - 1e-4 && value <= 10.2 + 1e-4,
                "at {time} s the value was {value}"
            );
        }
    }

    #[test]
    fn smooth_rotations_take_the_short_way_and_stay_unit_length() {
        let z = |degrees: f32| Quat::from_rotation_z(degrees.to_radians());
        // 10 degrees to 350 degrees is a 20 degree turn the short way round,
        // so halfway is no turn at all.
        let halfway = smooth_rotation(None, (0.0, z(10.0)), (1.0, z(350.0)), None, 0.5);
        assert!(halfway.dot(Quat::IDENTITY).abs() > 0.99999);
        assert!((halfway.length() - 1.0).abs() < 1e-5);
    }
}