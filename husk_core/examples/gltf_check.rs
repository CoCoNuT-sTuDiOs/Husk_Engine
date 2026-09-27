use husk_core::asset_import::load_gltf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("../test_assets/Box.glb");

    let document = gltf::Gltf::open(path).expect("Failed to open glTF for inspection");
    println!("--- Structure ---");
    println!("Meshes in file: {}", document.meshes().count());
    for mesh in document.meshes() {
        println!(
            "  Mesh '{}': {} primitive(s)",
            mesh.name().unwrap_or("(unnamed)"),
            mesh.primitives().count()
        );
    }
    println!("-----------------");

    match load_gltf(path, 500_000) {
        Ok(mesh) => {
            println!("Loaded mesh from {path}");
            println!("  Vertices: {}", mesh.positions.len());
            println!("  Triangles: {}", mesh.triangle_count());
            println!("  First position: {:?}", mesh.positions[0]);
            println!("  First normal: {:?}", mesh.normals[0]);
        }
        Err(e) => {
            println!("Failed to load {path}: {e}");
        }
    }
}