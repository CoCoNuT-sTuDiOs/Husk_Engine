use husk_core::asset_import::load_gltf;
use husk_core::rig::Skeleton;
use husk_core::skinning::compute_skin_weights;

fn main() {
    let mesh = load_gltf("../test_assets/Box.glb", 500_000).expect("Failed to load test mesh");

    // A simple 3-bone vertical chain spanning the cube's -0.5..0.5 height:
    // hip (bottom) -> spine (middle) -> chest (top).
    let mut skeleton = Skeleton::default();
    let hip = skeleton.add_bone("hip", None, glam::Vec3::new(0.0, -0.5, 0.0));
    let spine = skeleton.add_bone("spine", Some(hip), glam::Vec3::new(0.0, 0.5, 0.0));
    let _chest = skeleton.add_bone("chest", Some(spine), glam::Vec3::new(0.0, 0.5, 0.0));

    let world_matrices = skeleton.world_bind_matrices();
    let weights = compute_skin_weights(&skeleton, &world_matrices, &mesh.positions);

    for (i, position) in mesh.positions.iter().enumerate() {
        if i % 6 == 0 {
            let w = weights[i];
            println!(
                "Vertex {} at y={:.2}: top influence = bone '{}' (weight {:.2})",
                i,
                position[1],
                skeleton.bones[w.joint_indices[0] as usize].name,
                w.weights[0]
            );
        }
    }
}