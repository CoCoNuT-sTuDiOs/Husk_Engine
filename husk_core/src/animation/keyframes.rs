/// Two keys closer together than this (in seconds) count as the same key.
const KEY_MERGE_SECONDS: f32 = 0.005;

/// A repeat cycle has to run at least this much (seconds) past the last
/// key, so the last key has somewhere to blend back to the first.
const MIN_CYCLE_GAP: f32 = 0.01;
/// One snapshot of the pose: every joint's rotation at a moment in time.
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f32,
    pub rotations: Vec<glam::Quat>,
    /// Where the character stands (its offset from the rest position).
    pub offset: glam::Vec3,
}

/// All of the keyframes, always kept in order of time.
#[derive(Debug, Clone, Default)]
pub struct Timeline {
    pub keys: Vec<Keyframe>,
    pub cycle_period: Option<f32>,
    pub travel_velocity: glam::Vec3,
    pub travel_speed: f32,
}

impl Timeline {
    /// How long one repeat lasts (seconds), or None if the keys don't repeat
    /// (or the cycle no longer reaches past the last key).
    pub fn effective_period(&self) -> Option<f32> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        self.cycle_period
            .filter(|period| *period > last.time - first.time + MIN_CYCLE_GAP)
    }

    /// Makes the keys repeat: a cycle runs from the first key until
    /// `end_time`, then starts over. `end_time` has to be past the last
    /// key. Returns whether the repeat was set.
    pub fn set_cycle_end(&mut self, end_time: f32) -> bool {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else {
            return false;
        };
        let period = end_time - first.time;
        if period <= last.time - first.time + MIN_CYCLE_GAP {
            return false;
        }
        self.cycle_period = Some(period);
        true
    }
    /// Records `rotations` as the pose at `time` (seconds). A key that is
    /// already at that time has its pose replaced.
    pub fn set_key(&mut self, time: f32, rotations: Vec<glam::Quat>) {
        self.set_key_with_offset(time, rotations, glam::Vec3::ZERO);
    }

    /// Like `set_key`, and also records where the character stands (its
    /// offset from the rest position).
    pub fn set_key_with_offset(
        &mut self,
        time: f32,
        rotations: Vec<glam::Quat>,
        offset: glam::Vec3,
    ) {
        let time = time.max(0.0);        
        if let Some(existing) = self
            .keys
            .iter_mut()
            .find(|key| (key.time - time).abs() < KEY_MERGE_SECONDS)
        {
            existing.rotations = rotations;
            existing.offset = offset;
            return;
        }
        let index = self
            .keys
            .iter()
            .position(|key| key.time > time)
            .unwrap_or(self.keys.len());
           self.keys.insert(index, Keyframe { time, rotations, offset });
    }

    pub fn sample_offset(&self, time: f32) -> Option<glam::Vec3> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if time <= first.time {
            return Some(first.offset);
        }

        let period = self.effective_period();
        let elapsed_total = time - first.time;
        let span = last.time - first.time;
        let step = match period {
            Some(p) if span > 0.0 => (last.offset - first.offset) * (p / span),
            _ => glam::Vec3::ZERO,
        };
        let (time, repeats) = match period {
            Some(p) => {
                let elapsed = time - first.time;
                let remainder = elapsed % p;
                (first.time + remainder, ((elapsed - remainder) / p).round())
            }
            None => (time, 0.0),
        };

        let within_the_cycle = if time >= last.time {
            match period {
                // Heading from the last key to where the first key will be
                // one step further on.
                Some(p) => {
                    let t = (time - last.time) / (first.time + p - last.time);
                    last.offset.lerp(first.offset + step, t)
                }
                None => last.offset,
            }
        } else {
            let next_index = self.keys.iter().position(|key| key.time > time)?;
            let a = &self.keys[next_index - 1];
            let b = &self.keys[next_index];
            a.offset.lerp(b.offset, (time - a.time) / (b.time - a.time))
        };
        Some(within_the_cycle + step * repeats + self.travel_velocity * elapsed_total)
    }

    /// Deletes the key closest to `time`, if one is within `tolerance`
    /// seconds of it. Returns whether a key was removed.
    pub fn remove_near(&mut self, time: f32, tolerance: f32) -> bool {
        let closest = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, key)| (i, (key.time - time).abs()))
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        match closest {
            Some((i, _)) => {
                self.keys.remove(i);
                true
            }
            None => false,
        }
    }

    /// The pose at `time` (seconds) for a skeleton with `joint_count`
    /// joints: before the first key and after the last the pose holds;
    /// between two keys each joint turns the short way round, in a
    /// straight line from one key's rotation to the other's. A joint a
    /// key doesn't know about (it was added later) is left at rest.
    /// Returns None if there are no keys.
    pub fn sample(&self, time: f32, joint_count: usize) -> Option<Vec<glam::Quat>> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        let pick = |key: &Keyframe, joint: usize| -> glam::Quat {
            key.rotations
                .get(joint)
                .copied()
                .unwrap_or(glam::Quat::IDENTITY)
        };

        if time <= first.time {
            return Some((0..joint_count).map(|j| pick(first, j)).collect());
        }
        // With a repeat, fold the time back into the first cycle.
        let period = self.effective_period();
        let time = match period {
            Some(p) => first.time + (time - first.time) % p,
            None => time,
        };

        if time >= last.time {
            return Some(match period {
                // The gap at the end of a cycle: blend from the last key back
                // around to the first.
                Some(p) => {
                    let t = (time - last.time) / (first.time + p - last.time);
                    (0..joint_count)
                        .map(|j| pick(last, j).slerp(pick(first, j), t))
                        .collect()
                }
                None => (0..joint_count).map(|j| pick(last, j)).collect(),
            });
        }
        // The two keys on either side of `time`.
        let next_index = self.keys.iter().position(|key| key.time > time)?;
        let a = &self.keys[next_index - 1];
        let b = &self.keys[next_index];
        let t = (time - a.time) / (b.time - a.time);
        Some(
            (0..joint_count)
                .map(|j| pick(a, j).slerp(pick(b, j), t))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn z(degrees: f32) -> Quat {
        Quat::from_rotation_z(degrees.to_radians())
    }

    /// Two quaternions are the same rotation if their dot product is +-1.
    fn same(a: Quat, b: Quat) -> bool {
        a.dot(b).abs() > 0.99999
    }

    #[test]
    fn halfway_between_two_keys_is_halfway_between_the_rotations() {
        let mut timeline = Timeline::default();
        timeline.set_key(0.0, vec![z(0.0)]);
        timeline.set_key(2.0, vec![z(90.0)]);
        assert!(same(timeline.sample(1.0, 1).unwrap()[0], z(45.0)));
        assert!(same(timeline.sample(0.5, 1).unwrap()[0], z(22.5)));
    }

    #[test]
    fn before_the_first_key_and_after_the_last_the_pose_holds() {
        let mut timeline = Timeline::default();
        timeline.set_key(1.0, vec![z(10.0)]);
        timeline.set_key(3.0, vec![z(80.0)]);
        assert!(same(timeline.sample(0.0, 1).unwrap()[0], z(10.0)));
        assert!(same(timeline.sample(9.0, 1).unwrap()[0], z(80.0)));
    }

    #[test]
    fn keys_stay_sorted_and_a_key_at_the_same_time_is_replaced() {
        let mut timeline = Timeline::default();
        timeline.set_key(2.0, vec![z(20.0)]);
        timeline.set_key(0.5, vec![z(5.0)]);
        timeline.set_key(1.0, vec![z(10.0)]);
        let times: Vec<f32> = timeline.keys.iter().map(|key| key.time).collect();
        assert_eq!(times, vec![0.5, 1.0, 2.0]);

        timeline.set_key(1.0, vec![z(99.0)]);
        assert_eq!(timeline.keys.len(), 3);
        assert!(same(timeline.sample(1.0, 1).unwrap()[0], z(99.0)));
    }

    #[test]
    fn the_shortest_way_around_is_taken() {
        // 10 degrees to 350 degrees is a 20 degree turn the short way,
        // not a 340 degree turn the long way: halfway is no rotation at all.
        let mut timeline = Timeline::default();
        timeline.set_key(0.0, vec![z(10.0)]);
        timeline.set_key(1.0, vec![z(350.0)]);
        assert!(same(timeline.sample(0.5, 1).unwrap()[0], Quat::IDENTITY));
    }

    #[test]
    fn missing_joints_are_at_rest_and_extra_joints_are_dropped() {
        let mut timeline = Timeline::default();
        timeline.set_key(0.0, vec![z(30.0)]);
        timeline.set_key(1.0, vec![z(60.0)]);

        let three = timeline.sample(0.5, 3).unwrap();
        assert_eq!(three.len(), 3);
        assert!(same(three[0], z(45.0)));
        assert!(same(three[1], Quat::IDENTITY));
        assert!(same(three[2], Quat::IDENTITY));

        timeline.set_key(2.0, vec![z(0.0), z(70.0)]);
        assert_eq!(timeline.sample(2.0, 1).unwrap().len(), 1);
    }

    #[test]
    fn no_keys_means_nothing_to_sample() {
        assert!(Timeline::default().sample(1.0, 3).is_none());
    }

    #[test]
    fn remove_near_deletes_only_a_key_within_the_tolerance() {
        let mut timeline = Timeline::default();
        timeline.set_key(1.0, vec![z(10.0)]);
        timeline.set_key(2.0, vec![z(20.0)]);
        assert!(!timeline.remove_near(1.5, 0.05));
        assert_eq!(timeline.keys.len(), 2);
        assert!(timeline.remove_near(1.02, 0.05));
        assert_eq!(timeline.keys.len(), 1);
        assert!((timeline.keys[0].time - 2.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod cycle_tests {
    use super::*;
    use glam::Quat;

    fn z(degrees: f32) -> Quat {
        Quat::from_rotation_z(degrees.to_radians())
    }

    fn same(a: Quat, b: Quat) -> bool {
        a.dot(b).abs() > 0.99999
    }

    /// Left foot forward at 0 s (0 degrees), right foot forward at 0.5 s (90 degrees).
    fn left_right() -> Timeline {
        let mut timeline = Timeline::default();
        timeline.set_key(0.0, vec![z(0.0)]);
        timeline.set_key(0.5, vec![z(90.0)]);
        timeline
    }

    #[test]
    fn a_cycle_repeats_the_keys_every_period() {
        let mut timeline = left_right();
        assert!(timeline.set_cycle_end(1.0));

        // First cycle: key, key, then blend back toward the first key.
        assert!(same(timeline.sample(0.5, 1).unwrap()[0], z(90.0)));
        assert!(same(timeline.sample(0.75, 1).unwrap()[0], z(45.0)));
        // The second cycle starts over exactly like the first.
        assert!(same(timeline.sample(1.0, 1).unwrap()[0], z(0.0)));
        assert!(same(timeline.sample(1.25, 1).unwrap()[0], z(45.0)));
        assert!(same(timeline.sample(1.5, 1).unwrap()[0], z(90.0)));
        // And so does the third.
        assert!(same(timeline.sample(2.5, 1).unwrap()[0], z(90.0)));
    }

    #[test]
    fn the_cycle_must_end_after_the_last_key() {
        let mut timeline = left_right();
        assert!(!timeline.set_cycle_end(0.5));
        assert!(!timeline.set_cycle_end(0.505));
        assert!(!Timeline::default().set_cycle_end(1.0));
        assert!(timeline.set_cycle_end(1.0));
        assert_eq!(timeline.effective_period(), Some(1.0));
    }

    #[test]
    fn a_key_beyond_the_cycle_switches_it_off() {
        let mut timeline = left_right();
        assert!(timeline.set_cycle_end(1.0));
        timeline.set_key(1.2, vec![z(30.0)]);
        assert!(timeline.effective_period().is_none());
        assert!(same(timeline.sample(5.0, 1).unwrap()[0], z(30.0)));
    }

    #[test]
    fn before_the_first_key_the_pose_holds_even_with_a_cycle() {
        let mut timeline = Timeline::default();
        timeline.set_key(1.0, vec![z(10.0)]);
        timeline.set_key(1.5, vec![z(80.0)]);
        assert!(timeline.set_cycle_end(2.0));
        assert!(same(timeline.sample(0.2, 1).unwrap()[0], z(10.0)));
        assert!(same(timeline.sample(2.5, 1).unwrap()[0], z(80.0)));
    }
}

#[cfg(test)]
mod offset_tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn the_character_moves_between_keys_and_holds_outside_them() {
        let mut timeline = Timeline::default();
        timeline.set_key_with_offset(0.0, vec![Quat::IDENTITY], Vec3::ZERO);
        timeline.set_key_with_offset(2.0, vec![Quat::IDENTITY], Vec3::new(4.0, 0.0, 0.0));
        assert!(near(timeline.sample_offset(1.0).unwrap(), Vec3::new(2.0, 0.0, 0.0)));
        assert!(near(timeline.sample_offset(0.5).unwrap(), Vec3::new(1.0, 0.0, 0.0)));
        assert!(near(timeline.sample_offset(9.0).unwrap(), Vec3::new(4.0, 0.0, 0.0)));
        assert!(Timeline::default().sample_offset(1.0).is_none());
    }

    #[test]
    fn a_repeating_cycle_carries_the_character_on_at_a_steady_speed() {
        let mut timeline = Timeline::default();
        timeline.set_key_with_offset(0.0, vec![Quat::IDENTITY], Vec3::ZERO);
        timeline.set_key_with_offset(0.5, vec![Quat::IDENTITY], Vec3::new(0.5, 0.0, 0.0));
        assert!(timeline.set_cycle_end(1.0));

        // Half a unit in half a second is one unit per second, forever.
        for time in [0.0, 0.2, 0.5, 0.75, 1.0, 1.3, 2.0, 2.5, 3.9] {
            let offset = timeline.sample_offset(time).unwrap();
            assert!(
                near(offset, Vec3::new(time, 0.0, 0.0)),
                "at {time} s the character was at {offset:?}"
            );
        }
    }

    #[test]
    fn an_in_place_cycle_never_travels() {
        let mut timeline = Timeline::default();
        timeline.set_key_with_offset(0.0, vec![Quat::IDENTITY], Vec3::ZERO);
        timeline.set_key_with_offset(0.5, vec![Quat::IDENTITY], Vec3::ZERO);
        assert!(timeline.set_cycle_end(1.0));
        for time in [0.0, 0.3, 1.0, 2.7, 4.9] {
            assert!(near(timeline.sample_offset(time).unwrap(), Vec3::ZERO));
        }
    }

    #[test]
    fn a_key_at_the_same_time_replaces_its_offset() {
        let mut timeline = Timeline::default();
        timeline.set_key_with_offset(1.0, vec![Quat::IDENTITY], Vec3::new(1.0, 0.0, 0.0));
        timeline.set_key_with_offset(1.0, vec![Quat::IDENTITY], Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(timeline.keys.len(), 1);
        assert!(near(timeline.sample_offset(1.0).unwrap(), Vec3::new(5.0, 0.0, 0.0)));
    }
}

#[cfg(test)]
mod travel_tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn travel_speed_moves_the_character_steadily_from_the_first_key() {
        let mut timeline = Timeline::default();
        timeline.set_key(1.0, vec![Quat::IDENTITY]);
        timeline.set_key(2.0, vec![Quat::IDENTITY]);
        timeline.travel_velocity = Vec3::new(0.5, 0.0, 0.0);

        // Nothing happens before the first key ...
        assert!(near(timeline.sample_offset(0.0).unwrap(), Vec3::ZERO));
        assert!(near(timeline.sample_offset(1.0).unwrap(), Vec3::ZERO));
        // ... then half a unit every second, even after the last key.
        assert!(near(timeline.sample_offset(3.0).unwrap(), Vec3::new(1.0, 0.0, 0.0)));
        assert!(near(timeline.sample_offset(5.0).unwrap(), Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn travel_speed_adds_to_a_repeating_cycle_without_a_jump() {
        let mut timeline = Timeline::default();
        timeline.set_key(0.0, vec![Quat::IDENTITY]);
        timeline.set_key(0.5, vec![Quat::IDENTITY]);
        assert!(timeline.set_cycle_end(1.0));
        timeline.travel_velocity = Vec3::new(1.0, 0.0, 0.0);
        for time in [0.0, 0.4, 0.9, 1.0, 1.7, 2.0, 3.3] {
            assert!(near(
                timeline.sample_offset(time).unwrap(),
                Vec3::new(time, 0.0, 0.0)
            ));
        }
    }
}