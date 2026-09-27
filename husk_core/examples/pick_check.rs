use husk_core::asset_import::load_gltf;
use husk_core::picking::{ray_mesh_intersection, Ray};

fn main() {
    let mesh = load_gltf("../test_assets/Box.glb", 500_000).expect("Failed to load test mesh");

    // The Box test mesh is a 1x1x1 cube centered at the origin. A ray
    // starting well in front of it (+z) and aimed straight at the origin
    // should hit the cube's front face at z = 0.5.
    let ray = Ray {
        origin: glam::Vec3::new(0.0, 0.0, 5.0),
        direction: glam::Vec3::new(0.0, 0.0, -1.0),
    };

    match ray_mesh_intersection(&ray, &mesh.positions, &mesh.indices) {
        Some(hit) => {
            println!("Hit at {:?}, distance = {}", hit.point, hit.distance);
        }
        None => {
            println!("No intersection found (unexpected)");
        }
    }
}