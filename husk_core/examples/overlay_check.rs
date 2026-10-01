use husk_core::asset_import::load_gltf;
use husk_core::renderer::Renderer;

fn main() {
    let mesh = load_gltf("../test_assets/Box.glb", 500_000).expect("Failed to load test mesh");
    let mut renderer = Renderer::new(256, 256, mesh);
    renderer.render_frame();

    let center = renderer.project_to_screen(glam::Vec3::ZERO);
    println!("World origin projects to: {:?} (expected about (128, 128), the exact center)", center);

    let right = renderer.project_to_screen(glam::Vec3::new(1.0, 0.0, 0.0));
    println!("World +X point projects to: {:?} (expected x above 128, y about 128)", right);

    let up = renderer.project_to_screen(glam::Vec3::new(0.0, 1.0, 0.0));
    println!("World +Y point projects to: {:?} (expected x about 128, y below 128, i.e. higher on screen)", up);

    let behind = renderer.project_to_screen(glam::Vec3::new(0.0, 3.0, 6.0));
    println!("Point behind the camera projects to: {:?} (expected None)", behind);
}