use husk_core::rig::Skeleton;

fn main() {
    let mut skeleton = Skeleton::default();

    // A simple 3-bone vertical chain: root -> spine -> head.
    let root = skeleton.add_bone("root", None, glam::Vec3::new(0.0, 0.0, 0.0));
    let spine = skeleton.add_bone("spine", Some(root), glam::Vec3::new(0.0, 1.0, 0.0));
    let _head = skeleton.add_bone("head", Some(spine), glam::Vec3::new(0.0, 1.0, 0.0));

    let world_matrices = skeleton.world_bind_matrices();

    for (i, bone) in skeleton.bones.iter().enumerate() {
        let world_position = world_matrices[i].transform_point3(glam::Vec3::ZERO);
        println!("Bone '{}': world position = {:?}", bone.name, world_position);
    }
}