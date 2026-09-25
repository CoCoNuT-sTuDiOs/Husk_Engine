use husk_core::ffi::{
    husk_renderer_create, husk_renderer_destroy, husk_renderer_frame_len,
    husk_renderer_frame_ptr, husk_renderer_render_frame,
};

fn main() {
    unsafe {
        let handle = husk_renderer_create(256, 256);

        for i in 0..3 {
            husk_renderer_render_frame(handle);
            let ptr = husk_renderer_frame_ptr(handle);
            let len = husk_renderer_frame_len(handle);
            let bytes = std::slice::from_raw_parts(ptr, len);
            println!(
                "Frame {}: len = {}, first pixel RGBA = ({}, {}, {}, {})",
                i, len, bytes[0], bytes[1], bytes[2], bytes[3]
            );
        }

        husk_renderer_destroy(handle);
    }
}