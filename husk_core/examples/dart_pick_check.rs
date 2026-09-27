use husk_core::api::simple::pick;
use husk_core::ffi::{husk_renderer_create, husk_renderer_destroy, husk_renderer_render_frame};

fn main() {
    let handle = husk_renderer_create(256, 256);
    husk_renderer_render_frame(handle);

    match pick(128.0, 128.0) {
        Some((x, y, z)) => println!("pick() hit at ({x}, {y}, {z})"),
        None => println!("pick() missed"),
    }

    husk_renderer_destroy(handle);

    match pick(128.0, 128.0) {
        Some(_) => println!("pick() still returned a hit after destroy (unexpected!)"),
        None => println!("pick() correctly returned None after renderer was destroyed"),
    }
}