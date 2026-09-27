/// A single bone in a skeleton's bind pose.
///
/// `parent` indexes into the same `Skeleton::bones` Vec this bone belongs
/// to. Bones must be stored parent-before-child (a bone's parent index is
/// always smaller than its own index) this is required by
/// `world_bind_matrices` below, and holds naturally as long as bones are
/// added root-first via `Skeleton::add_bone`.
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Transform relative to the parent bone, in the bind pose.
    pub local_position: glam::Vec3,
    pub local_rotation: glam::Quat,
    pub local_scale: glam::Vec3,
}

impl Bone {
    pub fn local_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.local_position,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    /// Adds a bone and returns its index. `parent` must already exist in
    /// this skeleton (i.e. have a smaller index), or be `None` for a root
    /// bone.
    pub fn add_bone(&mut self, name: &str, parent: Option<usize>, local_position: glam::Vec3) -> usize {
        self.bones.push(Bone {
            name: name.to_string(),
            parent,
            local_position,
            local_rotation: glam::Quat::IDENTITY,
            local_scale: glam::Vec3::ONE,
        });
        self.bones.len() - 1
    }

    /// Computes each bone's bind-pose transform in model space, by
    /// composing local transforms up the parent chain. Index `i` in the
    /// returned Vec is bone `i`'s world-space bind matrix.
    pub fn world_bind_matrices(&self) -> Vec<glam::Mat4> {
        let mut world = vec![glam::Mat4::IDENTITY; self.bones.len()];
        for i in 0..self.bones.len() {
            let local = self.bones[i].local_matrix();
            world[i] = match self.bones[i].parent {
                Some(parent_index) => world[parent_index] * local,
                None => local,
            };
        }
        world
    }
}