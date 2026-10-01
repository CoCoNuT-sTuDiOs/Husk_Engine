use husk_core::pose::Pose;
use husk_core::rig::Skeleton;

fn main() {
    let mut skeleton = Skeleton::default();
    let root = skeleton.add_bone("root", None, glam::Vec3::new(0.0, 0.0, 0.0));
    let mid = skeleton.add_bone("mid", Some(root), glam::Vec3::new(0.0, 1.0, 0.0));
    let _tip = skeleton.add_bone("tip", Some(mid), glam::Vec3::new(0.0, 1.0, 0.0));
    let side = skeleton.add_bone("side", Some(root), glam::Vec3::new(1.0, 0.0, 0.0));

    let mut pose = Pose::rest_for(&skeleton);
    println!("Rest:  {:?}", pose.posed_joint_positions(&skeleton));

    pose.set_rotation(mid, glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
    let posed = pose.posed_joint_positions(&skeleton);
    println!("Posed: {:?}", posed);
    println!(
        "root-to-mid distance: {} (must still be 1.0)",
        posed[root].distance(posed[mid])
    );

    let skin = pose.skinning_matrices(&skeleton);
    println!(
        "Vertex bound to 'tip' moves from (0,2,0) to {:?}",
        skin[2].transform_point3(glam::Vec3::new(0.0, 2.0, 0.0))
    );
    println!(
        "Vertex bound to 'side' moves from (1,0,0) to {:?}",
        skin[side].transform_point3(glam::Vec3::new(1.0, 0.0, 0.0))
    );

    let rest_positions: Vec<glam::Vec3> = skeleton
        .world_bind_matrices()
        .iter()
        .map(|m| m.transform_point3(glam::Vec3::ZERO))
        .collect();
    println!("Rest skeleton afterward (must be unchanged): {:?}", rest_positions);
}