use glam::{Quat, Vec3};

/// Two keys closer together than this (in seconds) count as the same key,
/// as in the pose timeline.
const KEY_MERGE_SECONDS: f32 = 0.005;

/// How a value moves from one key to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    /// Holds a key's value until the next key, then jumps.
    Step,
    /// Moves in a straight line from one key to the next. Rotations turn
    /// the short way round (so one segment never turns more than half a
    /// circle).
    Linear,
}

/// What a channel animates on its object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Property {
    Position,
    Rotation,
    Scale,
    /// A mesh's colour (red, green, blue).
    Colour,
    Roughness,
    Metalness,
    /// A camera's vertical field of view, in degrees.
    CameraFov,
    LightColour,
    LightIntensity,
}

/// A keyed value: a number, a vector (positions, scales, colours) or a
/// rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Vector(Vec3),
    Rotation(Quat),
}

impl Property {
    /// Whether `value` is the kind of value this property takes.
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Property::Rotation, Value::Rotation(_)) => true,
            (
                Property::Position
                | Property::Scale
                | Property::Colour
                | Property::LightColour,
                Value::Vector(_),
            ) => true,
            (
                Property::Roughness
                | Property::Metalness
                | Property::CameraFov
                | Property::LightIntensity,
                Value::Scalar(_),
            ) => true,
            _ => false,
        }
    }
}

impl Value {
    fn is_finite(&self) -> bool {
        match self {
            Value::Scalar(v) => v.is_finite(),
            Value::Vector(v) => v.is_finite(),
            Value::Rotation(q) => q.is_finite() && q.length_squared() > 1e-12,
        }
    }

    /// The value `t` of the way (0 to 1) from `self` to `other`. Two
    /// values of different kinds cannot be blended: `self` is returned.
    fn blend(self, other: Value, t: f32) -> Value {
        match (self, other) {
            (Value::Scalar(a), Value::Scalar(b)) => Value::Scalar(a + (b - a) * t),
            (Value::Vector(a), Value::Vector(b)) => Value::Vector(a.lerp(b, t)),
            (Value::Rotation(a), Value::Rotation(b)) => Value::Rotation(a.slerp(b, t)),
            _ => self,
        }
    }
}

/// One value at one moment, and how it moves on to the next key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelKey {
    pub time: f32,
    pub value: Value,
    pub interpolation: Interpolation,
}

/// The keys of one property, always kept in order of time.
#[derive(Clone, Debug)]
pub struct Channel {
    pub property: Property,
    pub keys: Vec<ChannelKey>,
}

impl Channel {
    pub fn new(property: Property) -> Channel {
        Channel {
            property,
            keys: Vec::new(),
        }
    }


    /// Like `sample`, for an animation that repeats every `period` seconds
    /// from its first key: after the last key the value blends back round
    /// to the first key's value, and then the cycle starts over. With no
    /// period, or one that does not reach past the last key, it is just
    /// `sample`.
    pub fn sample_looped(&self, time: f32, period: Option<f32>) -> Option<Value> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        let period = match period {
            Some(p) if p > last.time - first.time => p,
            _ => return self.sample(time),
        };
        if time <= first.time {
            return Some(first.value);
        }
        let time = first.time + (time - first.time) % period;
        if time >= last.time {
            return Some(match last.interpolation {
                Interpolation::Step => last.value,
                Interpolation::Linear => {
                    let t = (time - last.time) / (first.time + period - last.time);
                    last.value.blend(first.value, t)
                }
            });
        }
        self.sample(time)
    }


    /// Records `value` at `time` (seconds). A key already at that time is
    /// replaced. Returns false, changing nothing, if the value is not the
    /// kind this property takes, or a number is not finite. Rotations are
    /// normalized.
    pub fn set_key(&mut self, time: f32, value: Value, interpolation: Interpolation) -> bool {
        if !time.is_finite() || !self.property.accepts(&value) || !value.is_finite() {
            return false;
        }
        let value = match value {
            Value::Rotation(q) => Value::Rotation(q.normalize()),
            other => other,
        };
        let time = time.max(0.0);
        let key = ChannelKey {
            time,
            value,
            interpolation,
        };
        if let Some(existing) = self
            .keys
            .iter_mut()
            .find(|existing| (existing.time - time).abs() < KEY_MERGE_SECONDS)
        {
            *existing = ChannelKey {
                time: existing.time,
                ..key
            };
            return true;
        }
        let index = self
            .keys
            .iter()
            .position(|existing| existing.time > time)
            .unwrap_or(self.keys.len());
        self.keys.insert(index, key);
        true
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

    /// The value at `time` (seconds). Before the first key and after the
    /// last the value holds; between two keys it follows the earlier key's
    /// interpolation. None if there are no keys.
    pub fn sample(&self, time: f32) -> Option<Value> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if time <= first.time {
            return Some(first.value);
        }
        if time >= last.time {
            return Some(last.value);
        }
        let next_index = self.keys.iter().position(|key| key.time > time)?;
        let a = &self.keys[next_index - 1];
        let b = &self.keys[next_index];
        match a.interpolation {
            Interpolation::Step => Some(a.value),
            Interpolation::Linear => {
                let t = (time - a.time) / (b.time - a.time);
                Some(a.value.blend(b.value, t))
            }
        }
    }
}

/// All of one object's channels, one per property it animates.
#[derive(Clone, Debug, Default)]
pub struct Channels {
    pub channels: Vec<Channel>,
    /// How long one repeat lasts (seconds), set by the object before it
    /// samples: with a period, every channel loops (see
    /// `Channel::sample_looped`). None means no repeat.
    pub loop_period: Option<f32>,
}

impl Channels {
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Records `value` for `property` at `time`, creating the channel if
    /// needed (see `Channel::set_key` for what is refused).
    pub fn set_key(
        &mut self,
        property: Property,
        time: f32,
        value: Value,
        interpolation: Interpolation,
    ) -> bool {
        let index = match self.channels.iter().position(|c| c.property == property) {
            Some(i) => i,
            None => {
                self.channels.push(Channel::new(property));
                self.channels.len() - 1
            }
        };
        let accepted = self.channels[index].set_key(time, value, interpolation);
        if !accepted && self.channels[index].keys.is_empty() {
            self.channels.remove(index);
        }
        accepted
    }

    /// The value of `property` at `time`, if it has a channel with keys.
    pub fn sample(&self, property: Property, time: f32) -> Option<Value> {
        self.channels
            .iter()
            .find(|channel| channel.property == property)?
            .sample_looped(time, self.loop_period)
        }

    /// Every moment that has a key in any channel, earliest first, with
    /// keys at the same moment counted once.
    pub fn key_times(&self) -> Vec<f32> {
        let mut all: Vec<f32> = self
            .channels
            .iter()
            .flat_map(|channel| channel.keys.iter().map(|key| key.time))
            .collect();
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut times: Vec<f32> = Vec::new();
        for time in all {
            match times.last() {
                Some(last) if (time - last).abs() < KEY_MERGE_SECONDS => {}
                _ => times.push(time),
            }
        }
        times
    }

    
    /// Deletes the keys closest to `time` (within `tolerance` seconds) in
    /// every channel, and drops any channel left with no keys. Returns
    /// whether any key was removed.
    pub fn remove_near(&mut self, time: f32, tolerance: f32) -> bool {
        let mut removed = false;
        for channel in self.channels.iter_mut() {
            if channel.remove_near(time, tolerance) {
                removed = true;
            }
        }
        self.channels.retain(|channel| !channel.keys.is_empty());
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(degrees: f32) -> Quat {
        Quat::from_rotation_z(degrees.to_radians())
    }

    /// Two quaternions are the same rotation if their dot product is +-1.
    fn same(a: Quat, b: Quat) -> bool {
        a.dot(b).abs() > 0.99999
    }

    fn scalar(value: Option<Value>) -> f32 {
        match value {
            Some(Value::Scalar(v)) => v,
            other => panic!("expected a number, got {other:?}"),
        }
    }

    fn vector(value: Option<Value>) -> Vec3 {
        match value {
            Some(Value::Vector(v)) => v,
            other => panic!("expected a vector, got {other:?}"),
        }
    }

    fn rotation(value: Option<Value>) -> Quat {
        match value {
            Some(Value::Rotation(q)) => q,
            other => panic!("expected a rotation, got {other:?}"),
        }
    }

    #[test]
    fn halfway_between_two_linear_keys_is_halfway() {
        let mut roughness = Channel::new(Property::Roughness);
        assert!(roughness.set_key(0.0, Value::Scalar(0.2), Interpolation::Linear));
        assert!(roughness.set_key(2.0, Value::Scalar(0.6), Interpolation::Linear));
        assert!((scalar(roughness.sample(1.0)) - 0.4).abs() < 1e-5);
        assert!((scalar(roughness.sample(0.5)) - 0.3).abs() < 1e-5);

        let mut position = Channel::new(Property::Position);
        assert!(position.set_key(0.0, Value::Vector(Vec3::ZERO), Interpolation::Linear));
        assert!(position.set_key(
            2.0,
            Value::Vector(Vec3::new(4.0, -2.0, 0.0)),
            Interpolation::Linear
        ));
        assert!((vector(position.sample(1.0)) - Vec3::new(2.0, -1.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn rotations_turn_the_short_way_round() {
        let mut channel = Channel::new(Property::Rotation);
        assert!(channel.set_key(0.0, Value::Rotation(z(0.0)), Interpolation::Linear));
        assert!(channel.set_key(2.0, Value::Rotation(z(90.0)), Interpolation::Linear));
        assert!(same(rotation(channel.sample(1.0)), z(45.0)));

        // 10 degrees to 350 degrees is a 20 degree turn the short way, so
        // halfway is no rotation at all.
        let mut wrap = Channel::new(Property::Rotation);
        assert!(wrap.set_key(0.0, Value::Rotation(z(10.0)), Interpolation::Linear));
        assert!(wrap.set_key(1.0, Value::Rotation(z(350.0)), Interpolation::Linear));
        assert!(same(rotation(wrap.sample(0.5)), Quat::IDENTITY));
    }

    #[test]
    fn a_step_key_holds_its_value_until_the_next_key() {
        let mut channel = Channel::new(Property::Metalness);
        assert!(channel.set_key(0.0, Value::Scalar(0.0), Interpolation::Step));
        assert!(channel.set_key(2.0, Value::Scalar(1.0), Interpolation::Linear));
        assert_eq!(scalar(channel.sample(0.0)), 0.0);
        assert_eq!(scalar(channel.sample(1.99)), 0.0);
        assert_eq!(scalar(channel.sample(2.0)), 1.0);
    }

    #[test]
    fn before_the_first_key_and_after_the_last_the_value_holds() {
        let mut channel = Channel::new(Property::CameraFov);
        assert!(channel.set_key(1.0, Value::Scalar(30.0), Interpolation::Linear));
        assert!(channel.set_key(3.0, Value::Scalar(80.0), Interpolation::Linear));
        assert_eq!(scalar(channel.sample(0.0)), 30.0);
        assert_eq!(scalar(channel.sample(9.0)), 80.0);
        assert!(Channel::new(Property::CameraFov).sample(1.0).is_none());
    }

    #[test]
    fn keys_stay_sorted_and_a_key_at_the_same_time_is_replaced() {
        let mut channel = Channel::new(Property::Roughness);
        assert!(channel.set_key(2.0, Value::Scalar(0.2), Interpolation::Linear));
        assert!(channel.set_key(0.5, Value::Scalar(0.5), Interpolation::Linear));
        assert!(channel.set_key(1.0, Value::Scalar(0.1), Interpolation::Linear));
        let times: Vec<f32> = channel.keys.iter().map(|key| key.time).collect();
        assert_eq!(times, vec![0.5, 1.0, 2.0]);

        assert!(channel.set_key(1.0, Value::Scalar(0.9), Interpolation::Step));
        assert_eq!(channel.keys.len(), 3);
        assert_eq!(scalar(channel.sample(1.0)), 0.9);
        assert_eq!(channel.keys[1].interpolation, Interpolation::Step);
    }

    #[test]
    fn the_wrong_kind_of_value_or_a_bad_number_is_refused() {
        let mut channels = Channels::default();
        // A rotation property takes rotations, not numbers.
        assert!(!channels.set_key(Property::Rotation, 0.0, Value::Scalar(1.0), Interpolation::Linear));
        assert!(channels.is_empty(), "a refused key must not leave an empty channel");
        assert!(!channels.set_key(
            Property::Roughness,
            0.0,
            Value::Scalar(f32::NAN),
            Interpolation::Linear
        ));
        assert!(!channels.set_key(
            Property::Roughness,
            f32::INFINITY,
            Value::Scalar(0.5),
            Interpolation::Linear
        ));
        assert!(!channels.set_key(
            Property::Rotation,
            0.0,
            Value::Rotation(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)),
            Interpolation::Linear
        ));
        assert!(channels.is_empty());

        // Rotations are normalized when they are keyed.
        assert!(channels.set_key(
            Property::Rotation,
            0.0,
            Value::Rotation(Quat::from_xyzw(0.0, 0.0, 2.0, 0.0)),
            Interpolation::Linear
        ));
        let stored = rotation(channels.sample(Property::Rotation, 0.0));
        assert!((stored.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_repeat_blends_the_last_key_back_to_the_first_and_starts_over() {
        let mut channel = Channel::new(Property::Roughness);
        assert!(channel.set_key(0.0, Value::Scalar(0.0), Interpolation::Linear));
        assert!(channel.set_key(1.0, Value::Scalar(1.0), Interpolation::Linear));
        let at = |time: f32| scalar(channel.sample_looped(time, Some(2.0)));
        assert!((at(0.5) - 0.5).abs() < 1e-5);
        assert!((at(1.0) - 1.0).abs() < 1e-5);
        // Halfway through the gap between the last key and the cycle's end.
        assert!((at(1.5) - 0.5).abs() < 1e-5);
        // The second cycle starts over exactly like the first.
        assert!(at(2.0).abs() < 1e-5);
        assert!((at(2.5) - 0.5).abs() < 1e-5);
        assert!(at(4.0).abs() < 1e-5);
        assert!((at(5.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn without_a_usable_period_a_looped_sample_is_the_plain_one() {
        let mut channel = Channel::new(Property::Roughness);
        assert!(channel.set_key(0.0, Value::Scalar(0.0), Interpolation::Linear));
        assert!(channel.set_key(1.0, Value::Scalar(1.0), Interpolation::Linear));
        assert_eq!(scalar(channel.sample_looped(5.0, None)), 1.0);
        // A period that does not reach past the last key is no repeat.
        assert_eq!(scalar(channel.sample_looped(5.0, Some(0.5))), 1.0);
        assert!(Channel::new(Property::Roughness).sample_looped(1.0, Some(2.0)).is_none());
    }

    #[test]
    fn a_full_spin_loops_with_keys_a_third_of_a_turn_apart() {
        let mut channel = Channel::new(Property::Rotation);
        assert!(channel.set_key(0.0, Value::Rotation(z(0.0)), Interpolation::Linear));
        assert!(channel.set_key(1.0, Value::Rotation(z(120.0)), Interpolation::Linear));
        assert!(channel.set_key(2.0, Value::Rotation(z(240.0)), Interpolation::Linear));
        // The cycle is 3 seconds, so the last key blends on round to the
        // first, a further 120 degrees the short way.
        let at = |time: f32| rotation(channel.sample_looped(time, Some(3.0)));
        assert!(same(at(2.5), z(300.0)));
        assert!(same(at(3.0), z(0.0)));
        assert!(same(at(4.0), z(120.0)));
    }

    #[test]
    fn removing_a_key_works_across_channels_and_drops_empty_ones() {
    let mut channels = Channels::default();
        assert!(channels.set_key(Property::Roughness, 1.0, Value::Scalar(0.1), Interpolation::Linear));
        assert!(channels.set_key(Property::Metalness, 1.0, Value::Scalar(0.2), Interpolation::Linear));
        assert!(channels.set_key(Property::Metalness, 2.0, Value::Scalar(0.3), Interpolation::Linear));

        assert!(!channels.remove_near(1.5, 0.05));
        assert!(channels.remove_near(1.02, 0.05));
        // Roughness had only that key, so its channel is gone; metalness keeps its second key.
        assert_eq!(channels.channels.len(), 1);
        assert_eq!(channels.channels[0].property, Property::Metalness);
        assert_eq!(channels.channels[0].keys.len(), 1);
        assert!(channels.sample(Property::Roughness, 1.0).is_none());
    }

    #[test]
    fn key_times_lists_each_moment_once_across_channels() {
        let mut channels = Channels::default();
        assert!(channels.key_times().is_empty());
        assert!(channels.set_key(Property::Roughness, 2.0, Value::Scalar(0.1), Interpolation::Linear));
        assert!(channels.set_key(Property::Roughness, 0.5, Value::Scalar(0.2), Interpolation::Linear));
        assert!(channels.set_key(Property::Metalness, 2.001, Value::Scalar(0.3), Interpolation::Linear));
        assert!(channels.set_key(Property::Metalness, 3.0, Value::Scalar(0.4), Interpolation::Linear));
        let times = channels.key_times();
        assert_eq!(times.len(), 3);
        assert!((times[0] - 0.5).abs() < 1e-6);
        assert!((times[1] - 2.0).abs() < 0.01);
        assert!((times[2] - 3.0).abs() < 1e-6);
    }
}