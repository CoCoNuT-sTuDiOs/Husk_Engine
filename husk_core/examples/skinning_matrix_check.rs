use husk_core::animation::{compute_skinning_matrices, AnimationClip, AnimationTrack, Keyframe};
use husk_core::rig::Skeleton;

fn main() {
    let mut skeleton = Skeleton::default();
    let root = skeleton.add_bone("root", None, glam::Vec3::ZERO);
    let _tip = skeleton.add_bone("tip", Some(root), glam::Vec3::new(0.0, 1.0, 0.0));

    let rest_world = skeleton.world_bind_matrices();

    let track = AnimationTrack {
        bone_index: root,
        keyframes: vec![
            Keyframe {
                time: 0.0,
                position: glam::Vec3::ZERO,
                rotation: glam::Quat::IDENTITY,
                scale: glam::Vec3::ONE,
            },
            Keyframe {
                time: 1.0,
                position: glam::Vec3::ZERO,
                rotation: glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                scale: glam::Vec3::ONE,
            },
        ],
    };
    let clip = AnimationClip {
        tracks: vec![track],
        duration: 1.0,
    };

    let posed = clip.apply(&skeleton, 1.0);
    let posed_world = posed.world_bind_matrices();

    let skin_matrices = compute_skinning_matrices(&rest_world, &posed_world);

    // A vertex sitting exactly at the tip bone's rest position, fully
    // weighted to that bone, should move to exactly where the tip bone
    // itself moved to under the animation.
    let rest_vertex = glam::Vec3::new(0.0, 1.0, 0.0);
    let skinned_vertex = skin_matrices[1].transform_point3(rest_vertex);

    println!("Rest vertex position: {rest_vertex:?}");
    println!("Skinned vertex position at t=1.0: {skinned_vertex:?}");
    println!("Expected: approximately (-1.0, 0.0, 0.0) — matching the tip bone's own animated position from the previous test");
}