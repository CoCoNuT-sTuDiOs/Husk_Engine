use crate::rig::Skeleton;
use crate::skinning::VertexWeights;
use std::sync::Mutex;

pub static SKELETON: Mutex<Skeleton> = Mutex::new(Skeleton{bones: Vec::new()});
pub  static WEIGHTS: Mutex<Option<Vec<VertexWeights>>> = Mutex::new(None);
pub static POSE: Mutex<crate::pose::Pose> =
    Mutex::new(crate::pose::Pose { rotations: Vec::new(), root_offset: glam::Vec3::ZERO });
pub static TIMELINE: Mutex<crate::keyframes::Timeline> =
    Mutex::new(crate::keyframes::Timeline {
        keys: Vec::new(),
        cycle_period: None,
        travel_velocity: glam::Vec3::ZERO,
        travel_speed: 0.0,
    });