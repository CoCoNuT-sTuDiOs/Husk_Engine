use crate::renderer::Renderer;
use std::sync::Mutex;

static ACTIVE_RENDERER: Mutex<Option<usize>> = Mutex::new(None);

/// Runs `f` with a reference to the currently active renderer, if one
/// exists. Returns `None` if no renderer has been created (or it has
/// since been destroyed).
pub(crate) fn with_active_renderer<R>(f: impl FnOnce(&Renderer) -> R) -> Option<R> {
    let guard = ACTIVE_RENDERER.lock().unwrap();
    guard.map(|ptr| {
        let renderer = unsafe { &*(ptr as *const Renderer) };
        f(renderer)
    })
}

/// Creates a renderer of the given size and returns an opaque handle.
/// The caller (C++) owns this handle and must pass it to
/// `husk_renderer_destroy` when done.
#[unsafe(no_mangle)]
pub extern "C" fn husk_renderer_create(width: u32, height: u32) -> *mut Renderer {

// TEMPORARY: hardcoded absolute path for Section 2 testing only.
    // Replaced once real asset-loading (file picker / project system) exists.
    // TEMPORARY: hardcoded desktop cap, matching the PRD's current
    // (unvalidated) 500k-triangle desktop estimate. Will move to a real
    // config/settings source later.
    const MAX_DESKTOP_TRIANGLES: usize = 500_000;


    let mesh = crate::asset_import::load_gltf(
        "C:\\Users\\HomePC\\Downloads\\anime-girl-casual-outfit-stylized-3d-character\\source\\one_one.glb",
        MAX_DESKTOP_TRIANGLES,
    )
    .unwrap_or_else(|e| {
        eprintln!("Husk: failed to load test mesh ({e}), rendering empty scene");
        crate::asset_import::MeshData::default()
    });
    let renderer = Box::new(Renderer::new(width, height, mesh));
    let raw = Box::into_raw(renderer);

    *ACTIVE_RENDERER.lock().unwrap() = Some(raw as usize);

    raw
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

    let mut active = ACTIVE_RENDERER.lock().unwrap();
    if *active == Some(handle as usize) {
        *active = None;
    }
    drop(active);

    unsafe {
        drop(Box::from_raw(handle));
    }
}