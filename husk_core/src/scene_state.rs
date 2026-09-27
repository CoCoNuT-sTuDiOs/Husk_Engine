use crate::rig::Skeleton;
use crate::skinning::VertexWeights;
use std::sync::Mutex;

pub static SKELETON: Mutex<Skeleton> = Mutex::new(Skeleton{bones: Vec::new()});
pub  static WEIGHTS: Mutex<Option<Vec<VertexWeights>>> = Mutex::new(None);