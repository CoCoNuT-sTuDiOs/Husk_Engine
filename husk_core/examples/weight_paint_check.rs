use husk_core::api::simple::{compute_weights, get_vertex_dominant_bone, set_vertex_weight};
use husk_core::ffi::{husk_renderer_create, husk_renderer_destroy};
use husk_core::rig::Skeleton;
use husk_core::scene_state::SKELETON;

fn main() {
    let handle = husk_renderer_create(256, 256);

    {
        let mut skeleton = SKELETON.lock().unwrap();
        *skeleton = Skeleton::default();
        skeleton.add_bone("hip", None, glam::Vec3::new(0.0, -0.5, 0.0));
        let spine = skeleton.add_bone("spine", Some(0), glam::Vec3::new(0.0, 0.5, 0.0));
        skeleton.add_bone("chest", Some(spine), glam::Vec3::new(0.0, 0.5, 0.0));
    }

    println!("compute_weights(): {}", compute_weights());

    match get_vertex_dominant_bone(0) {
        Some((bone, weight)) => println!("Vertex 0 auto-weighted to bone {bone} ({weight:.2})"),
        None => println!("Vertex 0: no weights found (unexpected)"),
    }

    println!("set_vertex_weight(0, 2): {}", set_vertex_weight(0, 2));

    match get_vertex_dominant_bone(0) {
        Some((bone, weight)) => println!("Vertex 0 now forced to bone {bone} ({weight:.2})"),
        None => println!("Vertex 0: no weights found (unexpected)"),
    }

    husk_renderer_destroy(handle);
}