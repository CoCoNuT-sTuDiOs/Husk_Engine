use crate::renderer::Renderer;

/// Creates a renderer of the given size and returns an opaque handle.
/// The caller (C++) owns this handle and must pass it to
/// `husk_renderer_destroy` when done.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_create(width: u32, height: u32) -> *mut Renderer {
    let renderer = Box::new(Renderer::new(width, height));
    Box::into_raw(renderer)
}

/// Renders the next frame into the renderer's internal buffer.
/// Call `husk_renderer_frame_ptr` / `husk_renderer_frame_len` afterward
/// to read it.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_render_frame(handle: *mut Renderer) {
    if handle.is_null() {
        return;
    }
    let renderer = unsafe { &mut *handle };
    renderer.render_frame();
}

/// Returns a pointer to the most recently rendered frame's pixel data.
/// Valid until the next call to `husk_renderer_render_frame` on the same
/// handle, or until the handle is destroyed.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_frame_ptr(handle: *mut Renderer) -> *const u8 {
    if handle.is_null() {
        return std::ptr::null();
    }
    let renderer = unsafe { &*handle };
    renderer.frame_ptr()
}

/// Returns the length in bytes of the most recently rendered frame.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_frame_len(handle: *mut Renderer) -> usize {
    if handle.is_null() {
        return 0;
    }
    let renderer = unsafe { &*handle };
    renderer.frame_len()
}

/// Destroys a renderer created with `husk_renderer_create`.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_destroy(handle: *mut Renderer) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}