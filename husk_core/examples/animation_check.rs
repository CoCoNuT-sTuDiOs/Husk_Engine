use husk_core::animation::{AnimationClip, AnimationTrack, Keyframe};
use husk_core::rig::Skeleton;

fn main() {
    let mut skeleton = Skeleton::default();
    let root = skeleton.add_bone("root", None, glam::Vec3::ZERO);
    let _tip = skeleton.add_bone("tip", Some(root), glam::Vec3::new(0.0, 1.0, 0.0));

    // Root bone rotates a quarter-turn around Z over 1 second. Since
    // "tip" is root's child, it should swing sideways along with it.
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

    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let posed = clip.apply(&skeleton, t);
        let world_matrices = posed.world_bind_matrices();
        let tip_position = world_matrices[1].transform_point3(glam::Vec3::ZERO);
        println!("t={t:.2}: tip world position = {tip_position:?}");
    }
}