use husk_core::asset_import::load_gltf;
use husk_core::renderer::Renderer;

fn main() {
    let mesh = load_gltf("../test_assets/Box.glb", 500_000).expect("Failed to load test mesh");
    let mut renderer = Renderer::new(256, 256, mesh);

    // Render exactly one frame (frame counter starts at 0, so the cube's
    // rotation angle is 0 — a known, reproducible camera/model state).
    renderer.render_frame();

    // Click dead-center of the 256x256 image. The camera looks straight at
    // the origin from (0, 1.5, 3), so a center-screen ray should hit
    // somewhere near the cube's front-top area.
    match renderer.pick(128.0, 128.0) {
        Some(point) => println!("Center-screen pick hit at {:?}", point),
        None => println!("Center-screen pick missed (unexpected)"),
    }

    // A click near the very edge of the image should miss the cube
    // entirely — the background has nothing to hit.
    match renderer.pick(10.0, 10.0) {
        Some(point) => println!("Corner pick hit at {:?} (unexpected)", point),
        None => println!("Corner pick correctly missed"),
    }
}