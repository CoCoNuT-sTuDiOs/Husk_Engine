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

pub(crate) fn with_active_renderer_mut<R>(f: impl FnOnce(&mut Renderer) -> R) -> Option<R> {
    let guard = ACTIVE_RENDERER.lock().unwrap();
    guard.map(|ptr| {
        let renderer = unsafe { &mut *(ptr as *mut Renderer) };
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


    // TEMPORARY: a rigged test model from the crate's test assets. If it
    // loads, its skeleton and skin weights come with it. If not (missing
    // file, no rig), the old hardcoded plain mesh loads instead.
    const RIGGED_TEST_MODEL: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/off.glb");
    let (mesh, rig) =
        match crate::rig_import::load_gltf_rig(RIGGED_TEST_MODEL, MAX_DESKTOP_TRIANGLES) {
            Ok(model) => (model.mesh, Some((model.skeleton, model.vertex_weights))),
            Err(e) => {
                eprintln!("Husk: rigged test model not loaded ({e}), trying the plain mesh");
                let mesh = crate::asset_import::load_gltf(
                "C:\\Users\\HomePC\\Downloads\\the_amazing_spiderman.glb",
                    MAX_DESKTOP_TRIANGLES,
                )
                .unwrap_or_else(|e| {
                    eprintln!("Husk: failed to load test mesh ({e}), rendering empty scene");
                    crate::asset_import::MeshData::default()
                });
                (mesh, None)
            }
        };
    let renderer = Box::new(Renderer::new(width, height, mesh));

    // Hand the imported rig to the renderer and to the shared scene state,
    // so the overlay, dragging and posing tools all see it.
    if let Some((skeleton, vertex_weights)) = rig {
        renderer.update_vertex_weights(&vertex_weights);
        *crate::scene_state::POSE.lock().unwrap() = crate::pose::Pose::rest_for(&skeleton);
        *crate::scene_state::SKELETON.lock().unwrap() = skeleton;
        *crate::scene_state::WEIGHTS.lock().unwrap() = Some(vertex_weights);
    }
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