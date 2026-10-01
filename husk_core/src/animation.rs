#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub time: f32,
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

#[derive(Debug, Clone, Default)]
pub struct AnimationTrack {
    /// Which bone (by index into the Skeleton) this track animates.
    pub bone_index: usize,
    /// Keyframes, must be stored in increasing `time` order.
    pub keyframes: Vec<Keyframe>,
}

impl AnimationTrack {
    /// Evaluates this track's transform at the given time, interpolating
    /// between the two surrounding keyframes (linear for position/scale,
    /// spherical for rotation). Clamps to the first/last keyframe if
    /// `time` is outside the track's range.
    pub fn sample(&self, time: f32) -> Option<(glam::Vec3, glam::Quat, glam::Vec3)> {
        if self.keyframes.is_empty() {
            return None;
        }

        if time <= self.keyframes[0].time {
            let k = &self.keyframes[0];
            return Some((k.position, k.rotation, k.scale));
        }

        if time >= self.keyframes[self.keyframes.len() - 1].time {
            let k = &self.keyframes[self.keyframes.len() - 1];
            return Some((k.position, k.rotation, k.scale));
        }

        for window in self.keyframes.windows(2) {
            let (a, b) = (&window[0], &window[1]);
            if time >= a.time && time <= b.time {
                let span = b.time - a.time;
                let t = if span > 0.0 { (time - a.time) / span } else { 0.0 };
                let position = a.position.lerp(b.position, t);
                let rotation = a.rotation.slerp(b.rotation, t);
                let scale = a.scale.lerp(b.scale, t);
                return Some((position, rotation, scale));
            }
        }

        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnimationClip {
    pub tracks: Vec<AnimationTrack>,
    pub duration: f32,
}

/// Computes each bone's "skinning matrix": posed_world * rest_world.inverse().
/// This is the standard matrix used in linear blend skinning applying
/// it to a vertex's original (rest-pose) world-space position gives that
/// vertex's position under the current pose, accounting for however much
/// that particular bone has moved since the bind pose.
pub fn compute_skinning_matrices(
    rest_world_matrices: &[glam::Mat4],
    posed_world_matrices: &[glam::Mat4],
) -> Vec<glam::Mat4> {
    rest_world_matrices
        .iter()
        .zip(posed_world_matrices.iter())
        .map(|(rest, posed)| *posed * rest.inverse())
        .collect()
}

impl AnimationClip {
    /// Applies this clip's pose at `time` onto a copy of `skeleton`,
    /// returning a new Skeleton with animated local transforms (bones
    /// this clip doesn't touch keep their original bind-pose transform).
    pub fn apply(&self, skeleton: &crate::rig::Skeleton, time: f32) -> crate::rig::Skeleton {
        let mut posed = skeleton.clone();

        for track in &self.tracks {
            if let Some((position, rotation, scale)) = track.sample(time) {
                if let Some(bone) = posed.bones.get_mut(track.bone_index) {
                    bone.local_position = position;
                    bone.local_rotation = rotation;
                    bone.local_scale = scale;
                }
            }
        }

        posed
    }
}