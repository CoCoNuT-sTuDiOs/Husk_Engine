#[flutter_rust_bridge::frb(sync)] // Synchronous mode for simplicity of the demo
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}


/// Casts a ray from a screen-space point (in pixels, matching the active
/// renderer's own width/height) and returns the world-space point where
/// it hits the currently loaded mesh, if any.
#[flutter_rust_bridge::frb(sync)]
pub fn pick(x: f64, y: f64) -> Option<(f32, f32, f32)> {
    crate::platform::ffi::with_active_renderer(|renderer| renderer.pick(x as f32, y as f32))
        .flatten()
        .map(|point| (point.x, point.y, point.z))
}

#[flutter_rust_bridge::frb(sync)]
pub fn place_bone(x: f64, y: f64, parent: Option<usize>) -> Option<(usize, f32, f32, f32)> {
    // The hit is in the object's own space, where its skeleton lives. The
    // transform is taken before locking the scene for the bones.
    let space = crate::scene::active_transform().1;
    let hit = if crate::scene::active_is_rig() {
        // A rig has no mesh to click inside. The bone goes on the plane that
        // faces the view and passes through its parent bone (through the rig
        // itself for the first bone), the way Blender puts a new bone on the
        // plane through its 3D cursor. Orbit and click again to change depth.
        let anchor = match parent {
            Some(parent_index) => {
                let active = crate::scene::active_object();
                let world_matrices = active.skeleton.world_bind_matrices();
                let position = world_matrices
                    .get(parent_index)?
                    .transform_point3(glam::Vec3::ZERO);
                space.point_to_world(position)
            }
            None => space.translation,
        };
        let on_plane = crate::platform::ffi::with_active_renderer(|renderer| {
            renderer.screen_to_view_plane(x as f32, y as f32, anchor)
        })
        .flatten()?;
        space.point_to_local(on_plane)
    } else {
        crate::platform::ffi::with_active_renderer(|renderer| {
            renderer.pick_inside(x as f32, y as f32)
        })
        .flatten()?
    };
    crate::scene::push_undo(None);
    let mut active = crate::scene::active_object();
    let skeleton = &mut active.skeleton;
    let local_position = match parent {
    Some(parent_index) => {
            let world_matrices = skeleton.world_bind_matrices();
            let parent_world = world_matrices[parent_index].transform_point3(glam::Vec3::ZERO);
            hit - parent_world
        }
        None => hit,
    };

    let name = format!("bone_{}", skeleton.bones.len());
    let index = skeleton.add_bone(&name, parent, local_position);

    let world = space.point_to_world(hit);
    Some((index, world.x, world.y, world.z))
}

/// Returns the number of bones placed so far in the current skeleton.
#[flutter_rust_bridge::frb(sync)]
pub fn bone_count() -> usize {
    crate::scene::active_object().skeleton.bones.len()
}

/// Runs auto skin-weighting (Section 3c's naive distance-based scheme)
/// against the current skeleton and the active mesh, storing the result
/// for inspection/editing. Returns false if there's no active renderer or
/// no bones placed yet.
#[flutter_rust_bridge::frb(sync)]
pub fn compute_weights() -> bool {
    let computed = crate::platform::ffi::with_active_renderer(|renderer| {
        let active = crate::scene::active_object();
        let skeleton = &active.skeleton;
        if skeleton.bones.is_empty() {
        return None;
        }
        let world_matrices = skeleton.world_bind_matrices();
        let geodesic = crate::rig::geodesic::compute_geodesic_weights(
            &skeleton,
            &world_matrices,
            renderer.mesh_positions(),
            renderer.mesh_indices(),
        );
        Some(geodesic.unwrap_or_else(|| {
            crate::rig::skinning::compute_skin_weights(
                &skeleton,
                &world_matrices,
                renderer.mesh_positions(),
            )
        }))

    })
    .flatten();

    match computed {
        Some(weights) => {
            crate::platform::ffi::with_active_renderer(|renderer| renderer.update_vertex_weights(&weights));
            crate::scene::push_undo(None);
            crate::scene::active_object().weights = Some(weights);
            true
        }
        None => false,
    }
}

/// Returns the given vertex's currently dominant bone and its weight, if
/// weights have been computed yet.
#[flutter_rust_bridge::frb(sync)]
pub fn get_vertex_dominant_bone(vertex_index: usize) -> Option<(usize, f32)> {
    let active = crate::scene::active_object();
    let weights = active.weights.as_ref()?;
    let vertex_weights = weights.get(vertex_index)?;
    Some((
        vertex_weights.joint_indices[0] as usize,
        vertex_weights.weights[0],
    ))
}

/// Rotates one bone to an absolute angle (degrees, around Z) measured
/// from its original bind pose — not from wherever it currently is — so
/// repeated calls don't compound. Recomputes skinning matrices for the
/// whole skeleton and uploads them to the GPU. Returns false if the bone
/// index is out of range or there's no active renderer.

#[flutter_rust_bridge::frb(sync)]
pub fn rotate_bone(bone_index: usize, angle_degrees: f32) -> bool {
    let skin_matrices = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
        if bone_index >= skeleton.bones.len() {
            return false;
        }
        pose.ensure_covers(&skeleton);
        if !pose.set_rotation(
            bone_index,
            glam::Quat::from_rotation_z(angle_degrees.to_radians()),
        ) {
            return false;
        }
        pose.skinning_matrices(&skeleton)
    };

    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));

    true
}


/// Drags a joint: rotates the joint's PARENT in 3D, around the parent's
/// own position, so the dragged joint swings onto the cursor at screen
/// pixel (x, y). The cursor is projected onto a plane through the joint
/// that faces the camera, so the joint can move in any direction the
/// camera sees (orbit the camera to bend front/back). Returns false for
/// root joints, bad indices, or when there's no active renderer.
#[flutter_rust_bridge::frb(sync)]
pub fn drag_bone(bone_index: usize, x: f64, y: f64) -> bool {
    crate::scene::push_undo(Some("pose"));
    // Joints live in the object's own space; the cursor and the camera's
    // drag plane are in the scene. Taken before any scene lock.
    let space = crate::scene::active_transform().1;
    let joint_position = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
      pose.ensure_covers(&skeleton);
        match pose.posed_joint_positions(&skeleton).get(bone_index) {
            Some(position) => *position,
            None => return false,
        }
    };

    let Some(target) = crate::platform::ffi::with_active_renderer(|renderer| {
renderer.screen_to_view_plane(x as f32, y as f32, space.point_to_world(joint_position))
    })
    .flatten() else {
        return false;
    };

    let skin_matrices = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
        if !pose.aim_joint_at(&skeleton, bone_index, space.point_to_local(target)) {
        return false;
        }
        pose.skinning_matrices(&skeleton)
    };

    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));
    true
}


/// IK drag: moves the joint onto the cursor at screen pixel (x, y) by
/// bending the two joints above it (drag an ankle and the hip and knee
/// follow). The cursor is projected onto a plane through the joint that
/// faces the camera. Returns false if the joint lacks a parent and
/// grandparent, or there's no active renderer.
#[flutter_rust_bridge::frb(sync)]
pub fn drag_limb(bone_index: usize, x: f64, y: f64) -> bool {
    crate::scene::push_undo(Some("pose"));
    // Joints live in the object's own space; the cursor and the camera's
    // drag plane are in the scene. Taken before any scene lock.
    let space = crate::scene::active_transform().1;
let joint_position = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
pose.ensure_covers(&skeleton);
        match pose.posed_joint_positions(&skeleton).get(bone_index) {
            Some(position) => *position,
            None => return false,
        }
    };

    let Some((target, toward_viewer)) = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer
            .screen_to_view_plane(x as f32, y as f32, space.point_to_world(joint_position))
        .map(|point| (point, -renderer.view_forward()))
    })
    .flatten() else {
        return false;
    };

    let skin_matrices = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
        if !pose.solve_two_bone_ik(
            &skeleton,
            bone_index,
            space.point_to_local(target),
            space.direction_to_local(toward_viewer),
        ) {
        return false;
        }
        pose.skinning_matrices(&skeleton)
    };

    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));
    true
}

/// Puts every joint back to its rest pose, keeping the skeleton and
/// skin weights as they are.
#[flutter_rust_bridge::frb(sync)]
pub fn reset_pose() {
    let mut active = crate::scene::active_object();
    let crate::scene::Object { skeleton, pose, .. } = &mut *active;
    *pose = crate::animation::pose::Pose::rest_for(&skeleton);
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&[]));
}

/// Undo for bone placement: removes the most recently placed bone and
/// returns (true, its parent) so the caller can select that parent, or
/// (false, None) if there is nothing to remove. Skin weights computed
/// earlier still refer to the old bone list, so recompute them afterward.
#[flutter_rust_bridge::frb(sync)]
pub fn undo_last_bone() -> (bool, Option<usize>) {
    let (skin_matrices, parent) = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
        let Some(removed) = skeleton.bones.pop() else {
        return (false, None);
        };
        pose.rotations.truncate(skeleton.bones.len());
        (pose.skinning_matrices(&skeleton), removed.parent)
    };
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));
    (true, parent)
}

/// Records the current pose as a keyframe at `time` (seconds). A key
/// already at that time is replaced. Returns how many keys there are now.
#[flutter_rust_bridge::frb(sync)]
pub fn add_keyframe(time: f32) -> usize {
    // Everything that is not a bone (placement, look, lens, light) is keyed
    // at the same moment as the pose. The lock is released before the next
    // line takes it again.
crate::scene::push_undo(None);
    crate::scene::active_object().key_channels(time);
    let mut active = crate::scene::active_object();
    let crate::scene::Object { skeleton, pose, timeline, .. } = &mut *active;
    pose.ensure_covers(&skeleton);
    let rotations: Vec<glam::Quat> = pose
        .rotations
        .iter()
        .take(skeleton.bones.len())
        .copied()
        .collect();
    timeline.set_key_with_offset(time, rotations, pose.root_offset);
        timeline.keys.len()
}

/// The time (seconds) of every keyframe, earliest first.
#[flutter_rust_bridge::frb(sync)]
pub fn keyframe_times() -> Vec<f32> {
    crate::scene::active_object().key_times()
}

/// Deletes the keyframe nearest `time` if one is within 0.05 s of it.
/// Returns whether a key was removed.
#[flutter_rust_bridge::frb(sync)]
pub fn delete_keyframe_at(time: f32) -> bool {
let has_key = crate::scene::active_object()
        .key_times()
        .iter()
        .any(|key| (key - time).abs() <= 0.05);
    if has_key {
        crate::scene::push_undo(None);
    }
    crate::scene::active_object().remove_keys_near(time, 0.05)
}

/// Poses the character as the keyframes say it looks at `time` (seconds).
/// Returns false, leaving the pose alone, if there are no keyframes.
#[flutter_rust_bridge::frb(sync)]
pub fn seek_to_time(time: f32) -> bool {
    // Every object in the scene is set to how its animation says it is at
    // this moment, and then the renderer is told.
    let frames = crate::scene::evaluate_at(time);
    let animated = frames
        .iter()
        .any(|frame| frame.placed || frame.skinning.is_some());
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        for frame in &frames {
            if let Some(skinning) = &frame.skinning {
                renderer.set_object_bone_matrices(frame.index, skinning);
            }
            // A child follows its parent even when it has no keys of its own.
            renderer.set_object_model(frame.index, frame.model);
            if frame.placed {
            // (the placement is set above, for every object)
                renderer.set_object_material(frame.index, &frame.material);
            }
        }
    });
    // The animation may have moved the scene camera or changed a light.
    push_scene_camera();
    push_scene_lights();
    animated
}

/// Makes the keys repeat: a cycle runs from the first key until
/// `end_time` (seconds), then starts over, over and over. `end_time` has
/// to be past the last key. Returns whether the repeat was set.
#[flutter_rust_bridge::frb(sync)]
pub fn set_repeat_end(end_time: f32) -> bool {
    crate::scene::active_object()
        .timeline
        .set_cycle_end(end_time)
}

/// Stops the keys from repeating.
#[flutter_rust_bridge::frb(sync)]
pub fn clear_repeat() {
    crate::scene::active_object().timeline.cycle_period = None;
}

/// How long one repeat of the keys lasts (seconds), or None if they don't repeat.
#[flutter_rust_bridge::frb(sync)]
pub fn repeat_period() -> Option<f32> {
    crate::scene::active_object()
        .timeline
        .effective_period()
}

/// Moves the whole character so its root joint follows the cursor at
/// screen pixel (x, y), on a plane through the root that faces the camera.
/// Returns false if there is no skeleton or no active renderer.
#[flutter_rust_bridge::frb(sync)]
pub fn move_character(x: f64, y: f64) -> bool {
    crate::scene::push_undo(Some("pose"));
    // The root offset lives in the object's own space; the cursor and the
    // camera's drag plane are in the scene. Taken before any scene lock.
    let space = crate::scene::active_transform().1;
    let (root_position, rest_position) = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;        
        pose.ensure_covers(&skeleton);
        let Some(root_index) = skeleton.bones.iter().position(|bone| bone.parent.is_none())
        else {
            return false;
        };
        let rest = skeleton.world_bind_matrices()[root_index].transform_point3(glam::Vec3::ZERO);
        (rest + pose.root_offset, rest)
    };

    let Some(target) = crate::platform::ffi::with_active_renderer(|renderer| {
renderer.screen_to_view_plane(x as f32, y as f32, space.point_to_world(root_position))
    })
    .flatten() else {
        return false;
    };

    let skin_matrices = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
        pose.root_offset = space.point_to_local(target) - rest_position;
        pose.skinning_matrices(&skeleton)
    };
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));
    true
}

/// Zooms the camera: a factor above 1 moves it away, below 1 closer.
#[flutter_rust_bridge::frb(sync)]
pub fn zoom_camera(factor: f64) {
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.zoom(factor as f32));
}

/// Makes the character travel at `speed` units per second along the
/// camera's right-hand direction as it looks right now (negative goes
/// left), starting at the first key. Combines with keyed movement and
/// repeats. Returns false if there is no active renderer.
#[flutter_rust_bridge::frb(sync)]
pub fn set_travel_speed(speed: f32) -> bool {
    let Some(right) = crate::platform::ffi::with_active_renderer(|renderer| renderer.view_right()) else {
        return false;
    };
    // Root motion is stored in the object's own space (as in Blender), so
    // the scene-space velocity is carried into it, before the scene lock.
    let velocity = crate::scene::active_transform()
        .1
        .to_matrix()
        .inverse()
        .transform_vector3(right * speed);
    let mut active = crate::scene::active_object();
    let timeline = &mut active.timeline;
    timeline.travel_velocity = velocity;
    timeline.travel_speed = speed;
    true
}

/// The Travel slider's current value (units per second).
#[flutter_rust_bridge::frb(sync)]
pub fn travel_speed() -> f32 {
    crate::scene::active_object().timeline.travel_speed
}

/// Screen-space (pixel) position of every joint in its current posed
/// state, indexed the same as the skeleton's bones. An entry is None if
/// that joint is behind the camera.
#[flutter_rust_bridge::frb(sync)]
pub fn joint_screen_positions() -> Vec<Option<(f32, f32)>> {
    // Joints are in the object's own space; the screen needs scene space.
    let model = crate::scene::active_transform().1.to_matrix();
    let positions = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, .. } = &mut *active;
    pose.ensure_covers(&skeleton);
        pose.posed_joint_positions(&skeleton)
    };

    crate::platform::ffi::with_active_renderer(|renderer| {
        positions
            .iter()
            .map(|p| renderer.project_to_screen(model.transform_point3(*p)))
            .collect()
    })
    .unwrap_or_default()
}

#[flutter_rust_bridge::frb(sync)]
pub fn bone_parents() -> Vec<Option<usize>> {
    crate::scene::active_object()
        .skeleton
        .bones
        .iter()
        .map(|bone| bone.parent)
        .collect()
}


/// Clears the current skeleton and any computed weights, so you can
/// start placing bones over from a clean state.
#[flutter_rust_bridge::frb(sync)]
pub fn reset_skeleton() {
    crate::scene::push_undo(None);
    {
        let mut active = crate::scene::active_object();
        let name = std::mem::take(&mut active.name);
        let transform = active.transform;
        let material = active.material;
        let kind = active.kind;
        let channels = std::mem::take(&mut active.channels);
        let id = active.id;
        let parent = active.parent;
        let locked = active.locked;
        let mesh = active.mesh.clone();
        *active = crate::scene::Object {
            name,
            transform,
            material,
            kind,
            channels,
            id,
            parent,
            locked,
            mesh,
            ..crate::scene::Object::default()
        };

    }
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&[]));
}

/// Turns the viewport to one of the six standard views ("front", "back",
/// "right", "left", "top" or "bottom"), which are orthographic, as Blender's
/// numpad views are. False, changing nothing, for any other name.
#[flutter_rust_bridge::frb(sync)]
pub fn set_standard_view(name: String) -> bool {
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_standard_view(&name))
        .unwrap_or(false)
}

/// Switches the viewport between perspective and orthographic (Blender's
/// numpad 5). Returns whether it is orthographic now.
#[flutter_rust_bridge::frb(sync)]
pub fn toggle_orthographic() -> bool {
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        let on = !renderer.orthographic();
        renderer.set_orthographic(on);
        on
    })
    .unwrap_or(false)
}

/// Slides the camera's view by a drag of (dx, dy) render pixels, as with the
/// middle mouse button and Shift in Blender: the scene follows the cursor.
#[flutter_rust_bridge::frb(sync)]
pub fn pan_camera(dx: f64, dy: f64) {
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.pan(dx as f32, dy as f32));
}


#[flutter_rust_bridge::frb(sync)]
pub fn frame_all() -> bool {
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.frame_all())
        .unwrap_or(false)
}

#[flutter_rust_bridge::frb(sync)]
pub fn frame_selected() -> bool {
    // The scene knows which object is active; it is asked before the
    // renderer is locked.
    let active = crate::scene::object_list().1;
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.frame_object(active))
        .flatten()
        .is_some()
}

/// Orbits the camera by the given angle deltas (radians) around its
/// current target. Positive delta_yaw orbits rightward, positive
/// delta_pitch orbits upward.
#[flutter_rust_bridge::frb(sync)]
pub fn orbit_camera(delta_yaw: f64, delta_pitch: f64) {
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.orbit(delta_yaw as f32, delta_pitch as f32)
    });
}

#[flutter_rust_bridge::frb(sync)]
pub fn set_vertex_weight(vertex_index: usize, bone_index: usize) -> bool {
    let mut active = crate::scene::active_object();
    let Some(weights) = active.weights.as_mut() else {
    return false;
    };
    let Some(vertex_weights) = weights.get_mut(vertex_index) else {
        return false;
    };

    vertex_weights.joint_indices = [bone_index as u32, 0, 0, 0];
    vertex_weights.weights = [1.0, 0.0, 0.0, 0.0];

    true
}



#[flutter_rust_bridge::frb(sync)]
pub fn add_shape(name: String, x: f64, y: f64, snap: bool) -> Option<usize> {
    let mesh = crate::geometry::shapes::by_name(&name)?;
    let (render_index, model, colour) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
    renderer.add_shape_at_screen(mesh, x as f32, y as f32, snap)
    })
    .flatten()?;
    let mut letters = name.chars();
    let label = match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect::<String>(),
        None => String::new(),
    };
    let scene_index = crate::scene::add_object(
        &label,
        crate::scene::transform::Transform::from_matrix(model),
        crate::scene::material::Material {
            base_color: [colour[0], colour[1], colour[2]],
            ..crate::scene::material::Material::default()
        },
        crate::scene::ObjectKind::Mesh,
    );
    share_mesh_with_scene(render_index);
    if scene_index != render_index {
    eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }
    Some(render_index)
}

/// Makes the object under screen pixel (x, y) the active one, so the
/// bone and animation tools act on it. Returns its index, or None when
/// the pixel shows no object (the active object stays as it was).
#[flutter_rust_bridge::frb(sync)]
pub fn select_object_at(x: f64, y: f64) -> Option<usize> {
    // Locked objects cannot be clicked: the click goes through to what is
    // behind them. The scene is the source of truth, so it is asked first.
    let locks = crate::scene::object_locks();
    let index = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.pick_object_except(x as f32, y as f32, &locks)
    })
    .flatten()?;
    // The scene checks the index first, so a mismatch changes neither list.
    if !crate::scene::set_active_object(index) {
        eprintln!("Husk: the renderer picked object {index} but the scene has no such object");
        return None;
    }
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_active_object(index));
    Some(index)
}

#[flutter_rust_bridge::frb(sync)]
pub fn remove_active_object() -> Option<usize> {
    let index = crate::scene::remove_active_object()?;
    let removed = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.remove_object(index)
    });
    if removed != Some(true) {
        eprintln!(
            "Husk: the scene removed object {index} but the renderer did not; the two lists are out of step"
        );
    }
    // The object may have been the scene camera or a light.
    push_scene_camera();
    push_scene_lights();
    Some(index)
}

/// The names of the scene's objects, in order, and the index of the
/// active one.
#[flutter_rust_bridge::frb(sync)]
pub fn scene_objects() -> (Vec<String>, usize) {
    crate::scene::object_list()
}

/// Makes the object at `index` the active one (a click in the outliner).
/// Returns false, changing nothing, if there is no such object.
#[flutter_rust_bridge::frb(sync)]
pub fn select_object(index: usize) -> bool {
    if !crate::scene::set_active_object(index) {
        return false;
    }
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_active_object(index));
    true
}

#[flutter_rust_bridge::frb(sync)]
pub fn set_object_locked(index: usize, locked: bool) -> bool {
    crate::scene::set_object_locked(index, locked)
}

/// Locks every object except the active one.
#[flutter_rust_bridge::frb(sync)]
pub fn lock_all_except_active() {
    crate::scene::lock_all_except_active();
}

/// Unlocks every object.
#[flutter_rust_bridge::frb(sync)]
pub fn unlock_all_objects() {
    crate::scene::unlock_all();
}

/// For each object, in list order, whether it is locked.
#[flutter_rust_bridge::frb(sync)]
pub fn object_locks() -> Vec<bool> {
    crate::scene::object_locks()
}

/// Whether the active object is locked.
#[flutter_rust_bridge::frb(sync)]
pub fn active_object_locked() -> bool {
    crate::scene::active_locked()
}

/// The names of the built-in shapes, in the order the tray shows them.
#[flutter_rust_bridge::frb(sync)]
pub fn shape_names() -> Vec<String> {
    crate::geometry::shapes::SHAPE_NAMES
        .iter()
        .map(|name| name.to_string())
        .collect()
}

pub fn shape_preview(name: String, size: u32) -> Option<Vec<u8>> {
    let mesh = crate::geometry::shapes::by_name(&name)?;
    Some(crate::render::renderer::Renderer::render_preview(
        mesh,
        size.clamp(16, 512),
    ))
}

/// An object's placement as the Properties panel shows it: position,
/// rotation in degrees about X, Y and Z (Blender's order) and scale.
pub struct ObjectTransform {
    pub position: (f32, f32, f32),
    pub rotation_degrees: (f32, f32, f32),
    pub scale: (f32, f32, f32),
}

/// The active object's transform.
#[flutter_rust_bridge::frb(sync)]
pub fn active_object_transform() -> ObjectTransform {
    let transform = crate::scene::active_transform().1;
    let euler = transform.euler_degrees();
    ObjectTransform {
        position: (
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
        ),
        rotation_degrees: (euler.x, euler.y, euler.z),
        scale: (transform.scale.x, transform.scale.y, transform.scale.z),
    }
}


/// Sets the active object's transform and redraws it there. Returns false,
/// changing nothing, for any value that is not a finite number. A scale
/// too close to zero is nudged away from it.
#[flutter_rust_bridge::frb(sync)]
pub fn set_active_object_transform(transform: ObjectTransform) -> bool {
    let position = glam::Vec3::new(transform.position.0, transform.position.1, transform.position.2);
    let rotation = glam::Vec3::new(
        transform.rotation_degrees.0,
        transform.rotation_degrees.1,
        transform.rotation_degrees.2,
    );
    let scale = glam::Vec3::new(transform.scale.0, transform.scale.1, transform.scale.2);
    if !(position.is_finite() && rotation.is_finite() && scale.is_finite()) {
        return false;
    }

    let mut placed = crate::scene::transform::Transform::default();
    placed.translation = position;
    placed.set_euler_degrees(rotation);
    placed.scale = crate::scene::transform::clamp_scale(scale);

    let Some(index) = crate::scene::set_active_transform(placed) else {
        return false;
    };
    let applied = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.set_object_model(index, placed.to_matrix())
    });
    if applied != Some(true) {
        eprintln!("Husk: the scene moved object {index} but the renderer did not; the two lists are out of step");
    }
// Children follow their parent, so every drawn placement is refreshed.
    push_scene_models();
    // The object may be the scene camera or a light.
    push_scene_camera();
    push_scene_lights();
    true
}
/// Where the cursor at screen pixel (x, y) falls along the axis line that
/// passes through `origin` in direction `axis` (both in the scene), as a
/// distance t along the axis from `origin`. A gizmo drag moves an object by
/// the difference between this value now and at the start of the drag.
/// None when the cursor ray runs parallel to the axis.
#[flutter_rust_bridge::frb(sync)]
pub fn axis_drag_param(
    origin: (f32, f32, f32),
    axis: (f32, f32, f32),
    x: f64,
    y: f64,
) -> Option<f32> {
    crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.axis_drag_param(
            x as f32,
            y as f32,
            glam::Vec3::new(origin.0, origin.1, origin.2),
            glam::Vec3::new(axis.0, axis.1, axis.2),
        )
    })
    .flatten()
}

/// The screen pixels to draw one gizmo axis between: where `origin` is on
/// screen, and the tip of an arrow `pixels` long pointing the way `axis`
/// points on screen. None if the origin is behind the camera.
#[flutter_rust_bridge::frb(sync)]
pub fn gizmo_axis_ends(
    origin: (f32, f32, f32),
    axis: (f32, f32, f32),
    pixels: f32,
) -> Option<((f32, f32), (f32, f32))> {
    crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.gizmo_axis_ends(
            glam::Vec3::new(origin.0, origin.1, origin.2),
            glam::Vec3::new(axis.0, axis.1, axis.2),
            pixels,
        )
    })
    .flatten()
}


/// The screen points of a gizmo ring: a circle about `pixels` in radius
/// around `origin`, in the plane perpendicular to `axis`, as `segments` + 1
/// points that close the loop. None if the origin is behind the camera.
#[flutter_rust_bridge::frb(sync)]
pub fn gizmo_ring(
    origin: (f32, f32, f32),
    axis: (f32, f32, f32),
    pixels: f32,
    segments: u32,
) -> Option<Vec<(f32, f32)>> {
    crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.gizmo_ring(
            glam::Vec3::new(origin.0, origin.1, origin.2),
            glam::Vec3::new(axis.0, axis.1, axis.2),
            pixels,
            segments as usize,
        )
    })
    .flatten()
}

/// The angle (radians) of the cursor at screen pixel (x, y) around the
/// axis through `origin`, measured on the plane perpendicular to it. A
/// rotate drag turns the object by the change in this angle. None when
/// that plane is seen almost edge-on.
#[flutter_rust_bridge::frb(sync)]
pub fn ring_drag_angle(
    origin: (f32, f32, f32),
    axis: (f32, f32, f32),
    x: f64,
    y: f64,
) -> Option<f32> {
    crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.ring_drag_angle(
            x as f32,
            y as f32,
            glam::Vec3::new(origin.0, origin.1, origin.2),
            glam::Vec3::new(axis.0, axis.1, axis.2),
        )
    })
    .flatten()
}

/// The smallest signed turn (radians, -pi to pi) that takes angle `from`
/// to angle `to`.
#[flutter_rust_bridge::frb(sync)]
pub fn angle_difference(from: f32, to: f32) -> f32 {
    crate::render::picking::angle_difference(from, to)
}

/// The Euler angles (degrees, Blender's XYZ order) of an object that has
/// rotation `start_degrees`, after turning it by `angle` radians about the
/// fixed scene axis `axis`.
#[flutter_rust_bridge::frb(sync)]
pub fn rotated_euler_degrees(
    start_degrees: (f32, f32, f32),
    axis: (f32, f32, f32),
    angle: f32,
) -> (f32, f32, f32) {
    let mut turned = crate::scene::transform::Transform::default();
    turned.set_euler_degrees(glam::Vec3::new(
        start_degrees.0,
        start_degrees.1,
        start_degrees.2,
    ));
    turned.rotate_about_world_axis(glam::Vec3::new(axis.0, axis.1, axis.2), angle);
    let euler = turned.euler_degrees();
    (euler.x, euler.y, euler.z)
}

/// The active object's own X, Y and Z axes as directions in the scene.
#[flutter_rust_bridge::frb(sync)]
pub fn active_object_axes() -> ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32)) {
    let axes = crate::scene::active_transform().1.local_axes();
    (
        (axes[0].x, axes[0].y, axes[0].z),
        (axes[1].x, axes[1].y, axes[1].z),
        (axes[2].x, axes[2].y, axes[2].z),
    )
}

/// How an object's surface looks, as the Properties panel shows it:
/// colour (red, green, blue, each 0 to 1), roughness and metalness.
pub struct ObjectMaterial {
    pub base_color: (f32, f32, f32),
    pub roughness: f32,
    pub metalness: f32,
}

/// The active object's material.
#[flutter_rust_bridge::frb(sync)]
pub fn active_object_material() -> ObjectMaterial {
    let material = crate::scene::active_material().1;
    ObjectMaterial {
        base_color: (
            material.base_color[0],
            material.base_color[1],
            material.base_color[2],
        ),
        roughness: material.roughness,
        metalness: material.metalness,
    }
}

/// Sets the active object's material and redraws it. Values outside 0 to 1
/// are held inside it. Returns false, changing nothing, for any value that
/// is not a finite number.
#[flutter_rust_bridge::frb(sync)]
pub fn set_active_object_material(material: ObjectMaterial) -> bool {
    let values = [
        material.base_color.0,
        material.base_color.1,
        material.base_color.2,
        material.roughness,
        material.metalness,
    ];
    if !values.iter().all(|value| value.is_finite()) {
        return false;
    }
    let placed = crate::scene::material::Material {
        base_color: [material.base_color.0, material.base_color.1, material.base_color.2],
        roughness: material.roughness,
        metalness: material.metalness,
    }
    .clamped();

    let Some(index) = crate::scene::set_active_material(placed) else {
        return false;
    };
    let applied = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.set_object_material(index, &placed)
    });
    if applied != Some(true) {
        eprintln!("Husk: the scene restyled object {index} but the renderer did not; the two lists are out of step");
    }
    true
}

/// Adds a scene camera where the viewport is looking from, pointing the
/// way the viewport looks. Returns its index. The active object does not
/// change.
#[flutter_rust_bridge::frb(sync)]
pub fn add_camera() -> Option<usize> {
    let (render_index, model, colour) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.add_camera_at_view(crate::geometry::shapes::camera_proxy())
    })?;
    let scene_index = crate::scene::add_object(
        "Camera",
        crate::scene::transform::Transform::from_matrix(model),
        crate::scene::material::Material {
            base_color: [colour[0], colour[1], colour[2]],
            ..crate::scene::material::Material::default()
        },
        crate::scene::ObjectKind::Camera(crate::scene::camera::Camera::default()),
    );
    share_mesh_with_scene(render_index);
        if scene_index != render_index {
        eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }
    push_scene_camera();
    Some(render_index)
}

/// Adds a rig (an object that owns a skeleton and has no mesh to click
/// inside) at the middle of what the viewport is looking at. Returns its
/// index. The active object does not change.
#[flutter_rust_bridge::frb(sync)]
pub fn add_rig() -> Option<usize> {
    let (render_index, model, colour) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.add_rig_at_view(crate::geometry::shapes::rig_proxy())
    })?;
    let scene_index = crate::scene::add_object(
        "Rig",
        crate::scene::transform::Transform::from_matrix(model),
        crate::scene::material::Material {
            base_color: [colour[0], colour[1], colour[2]],
            ..crate::scene::material::Material::default()
        },
        crate::scene::ObjectKind::Rig,
    );
    share_mesh_with_scene(render_index);
        if scene_index != render_index {
        eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }
    Some(render_index)
}

/// The active object's vertical field of view in degrees, if it is a
/// camera; None for anything else.
#[flutter_rust_bridge::frb(sync)]
pub fn active_camera_fov() -> Option<f32> {
    crate::scene::active_camera().map(|lens| lens.fov_degrees)
}

/// Sets the active camera's vertical field of view (held between 10 and
/// 120 degrees). False, changing nothing, if the active object is not a
/// camera or the value is not a finite number.
#[flutter_rust_bridge::frb(sync)]
pub fn set_active_camera_fov(degrees: f32) -> bool {
    if !degrees.is_finite() {
        return false;
    }
    if crate::scene::active_camera().is_some() {
        crate::scene::push_undo(Some("lens"));
    }
    let applied = crate::scene::set_active_camera(
        crate::scene::camera::Camera {
            fov_degrees: degrees,
        }
        .clamped(),
    );
    if applied {
        push_scene_camera();
    }
    applied
}
/// Adds a light of the given kind ("sun" or "point", any letter case),
/// above and beside what the viewport is looking at and facing it.
/// Returns its index, or None for an unknown kind. The active object does
/// not change.
#[flutter_rust_bridge::frb(sync)]
pub fn add_light(kind: String) -> Option<usize> {
    let (light, mesh, label) = match kind.to_lowercase().as_str() {
        "sun" => (
            crate::scene::light::Light::sun(),
            crate::geometry::shapes::sun_proxy(),
            "Sun",
        ),
        "point" => (
            crate::scene::light::Light::point(),
            crate::geometry::shapes::point_proxy(),
            "Point light",
        ),
        _ => return None,
    };
    let (render_index, model, colour) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.add_light_at_view(mesh)
    })?;
    let scene_index = crate::scene::add_object(
        label,
        crate::scene::transform::Transform::from_matrix(model),
        crate::scene::material::Material {
            base_color: [colour[0], colour[1], colour[2]],
            ..crate::scene::material::Material::default()
        },
        crate::scene::ObjectKind::Light(light),
    );
    share_mesh_with_scene(render_index);
        if scene_index != render_index {
        eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }
    push_scene_lights();
    Some(render_index)
}

/// A light as the Properties panel shows it: sun or point, colour (red,
/// green, blue, each 0 to 1), intensity, and the top of the intensity
/// range for this kind of light.
pub struct ObjectLight {
    pub is_sun: bool,
    pub color: (f32, f32, f32),
    pub intensity: f32,
    pub max_intensity: f32,
}

/// The active object's light, if it is a light; None for anything else.
#[flutter_rust_bridge::frb(sync)]
pub fn active_light() -> Option<ObjectLight> {
    crate::scene::active_light().map(|light| ObjectLight {
        is_sun: light.kind == crate::scene::light::LightKind::Sun,
        color: (light.color[0], light.color[1], light.color[2]),
        intensity: light.intensity,
        max_intensity: light.max_intensity(),
    })
}

/// Sets the active light's colour and intensity (held inside their
/// ranges). The kind (sun or point) stays as it is. False, changing
/// nothing, if the active object is not a light or a value is not a
/// finite number.
#[flutter_rust_bridge::frb(sync)]
pub fn set_active_light(color: (f32, f32, f32), intensity: f32) -> bool {
    let values = [color.0, color.1, color.2, intensity];
    if !values.iter().all(|value| value.is_finite()) {
        return false;
    }
    let Some(current) = crate::scene::active_light() else {
        return false;
    };
crate::scene::push_undo(Some("light"));
    let applied = crate::scene::set_active_light(
    crate::scene::light::Light {
            kind: current.kind,
            color: [color.0, color.1, color.2],
            intensity,
        }
        .clamped(),
    );
    if applied {
        push_scene_lights();
    }
    applied
}

/// Hangs the object at `child` below the object at `parent`, or frees it
/// (`parent` None). The object stays where it is. False, changing nothing,
/// for an unknown index or a loop (a parent below its own child).
#[flutter_rust_bridge::frb(sync)]
pub fn set_object_parent(child: usize, parent: Option<usize>) -> bool {
    let changed = crate::scene::set_parent(child, parent);
    if changed {
        push_scene_models();
        push_scene_camera();
        push_scene_lights();
    }
    changed
}

/// For each object, in list order, the index of its parent (None at the
/// top level).
#[flutter_rust_bridge::frb(sync)]
pub fn object_parents() -> Vec<Option<usize>> {
    crate::scene::object_parents()
}

/// Makes the object at `child` follow bone `bone` of the object at `parent`
/// through the pose, staying where it is. False, changing nothing, for an
/// unknown object, a bone the parent does not have, or a loop.
#[flutter_rust_bridge::frb(sync)]
pub fn set_object_bone_parent(child: usize, parent: usize, bone: usize) -> bool {
    let changed = crate::scene::set_parent_to_bone(child, parent, bone);
    if changed {
        push_scene_models();
        push_scene_camera();
        push_scene_lights();
    }
    changed
}

/// For each object, in list order, the bone of its parent that it follows
/// (None for an object that does not follow a bone).
#[flutter_rust_bridge::frb(sync)]
pub fn object_bone_parents() -> Vec<Option<usize>> {
    crate::scene::object_bone_parents()
}

/// Brings every drawn placement up to date with the scene. Call after
/// anything that changes a pose, so parts that follow bones follow it.
#[flutter_rust_bridge::frb(sync)]
pub fn refresh_models() {
    push_scene_models();
    push_scene_camera();
    push_scene_lights();
}

/// Imports a glTF or GLB file as a new object and makes it the active one.
/// A file with a rig comes with its skeleton and skin weights; any other file
/// comes in as a plain mesh. The viewport then looks at the new object.
/// Returns (true, summary) or (false, reason).
#[flutter_rust_bridge::frb(sync)]
pub fn import_glb(path: String) -> (bool, String) {
    // TEMPORARY: the same desktop cap the startup loader used.
    const MAX_TRIANGLES: usize = 500_000;
    const IMPORT_COLOR: [f32; 4] = [0.80, 0.80, 0.82, 1.0];

    let (mesh, rig, note) = match crate::import::rigged::load_gltf_rig(&path, MAX_TRIANGLES) {
        Ok(model) => (
            model.mesh,
            Some((model.skeleton, model.vertex_weights)),
            String::new(),
        ),
        Err(rig_error) => match crate::import::mesh::load_gltf(&path, MAX_TRIANGLES) {
            Ok(mesh) => {
                let note = if rig_error.starts_with("This glTF file has no rig") {
                    String::from(" (no rig in the file)")
                } else {
                    format!(" (the rig was not imported: {rig_error})")
                };
                (mesh, None, note)
            }
            Err(reason) => return (false, reason),
        },
    };

    let name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Model")
        .to_string();
    let vertex_count = mesh.positions.len();
    let bone_count = rig.as_ref().map_or(0, |(skeleton, _)| skeleton.bones.len());

    let Some(render_index) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.add_object(mesh, glam::Mat4::IDENTITY, IMPORT_COLOR)
    }) else {
        return (false, String::from("There is no renderer to import into"));
    };
    let scene_index = crate::scene::add_object(
        &name,
        crate::scene::transform::Transform::default(),
        crate::scene::material::Material {
            base_color: [IMPORT_COLOR[0], IMPORT_COLOR[1], IMPORT_COLOR[2]],
            ..crate::scene::material::Material::default()
        },
        crate::scene::ObjectKind::Mesh,
    );
    if scene_index != render_index {
        eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }

    share_mesh_with_scene(render_index);

    // The new object becomes the active one, so the weights below and every
    // tool act on it.
    crate::scene::set_active_object(render_index);
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.set_active_object(render_index)
    });
    crate::scene::active_object().name = name.clone();

    if let Some((skeleton, vertex_weights)) = rig {
        crate::platform::ffi::with_active_renderer(|renderer| {
            renderer.update_vertex_weights(&vertex_weights)
        });
        let mut active = crate::scene::active_object();
        active.pose = crate::animation::pose::Pose::rest_for(&skeleton);
        active.skeleton = skeleton;
        active.weights = Some(vertex_weights);
    }

    let size = crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.frame_object(render_index)
    })
    .flatten();
    let size_text = match size {
        Some(size) => format!(", {size:.2} units across"),
        None => String::new(),
    };
    let rig_text = if bone_count > 0 {
        format!(", {bone_count} bones")
    } else {
        String::new()
    };
    (
        true,
        format!("Imported {name}: {vertex_count} vertices{rig_text}{size_text}{note}"),
    )
}

/// Gives the scene object at `index` the same mesh the renderer draws that
/// object from. Both hold a handle to the same data, so nothing is copied.
fn share_mesh_with_scene(index: usize) {
    let mesh = crate::platform::ffi::with_active_renderer(|renderer| renderer.object_mesh(index))
        .flatten();
    if let Some(mesh) = mesh {
        crate::scene::set_object_mesh(index, mesh);
    }
}

/// A surface under the cursor, as the viewport reads it: which object it
/// belongs to, the point and which way the surface faces there (both in the
/// scene).
pub struct SurfaceSpot {
    pub object: usize,
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
}

/// The nearest surface under screen pixel (x, y), or None over empty space.
/// With `skip_active` the active object and everything parented below it are
/// left out, so an object being dragged does not land on itself.
#[flutter_rust_bridge::frb(sync)]
pub fn surface_under_cursor(x: f64, y: f64, skip_active: bool) -> Option<SurfaceSpot> {
    let (_, active) = crate::scene::object_list();
    let parents = crate::scene::object_parents();
    let mut skip = vec![false; parents.len()];
    if skip_active {
        for index in 0..parents.len() {
            let mut current = Some(index);
            for _ in 0..=parents.len() {
                match current {
                    Some(i) if i == active => {
                        skip[index] = true;
                        break;
                    }
                    Some(i) => current = parents[i],
                    None => break,
                }
            }
        }
    }
    let hit = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.surface_at(x as f32, y as f32, &skip)
    })
    .flatten()?;
    Some(SurfaceSpot {
        object: hit.object,
        position: (hit.point.x, hit.point.y, hit.point.z),
        normal: (hit.normal.x, hit.normal.y, hit.normal.z),
    })
}

/// Puts the active object on the surface under screen pixel (x, y): its
/// origin goes to the point the cursor meets, and its +Y axis turns to the
/// way the surface faces, keeping as much of its spin as possible. The active
/// object and its children are ignored, so it cannot land on itself. False,
/// changing nothing, over empty space or when the active object is locked.
#[flutter_rust_bridge::frb(sync)]
pub fn snap_active_to_surface(x: f64, y: f64) -> bool {
    let Some(spot) = surface_under_cursor(x, y, true) else {
        return false;
    };
    let mut world = crate::scene::active_transform().1;
    world.translation = glam::Vec3::new(spot.position.0, spot.position.1, spot.position.2);
    world.rotation = crate::scene::transform::align_up_to(
        world.rotation,
        glam::Vec3::new(spot.normal.0, spot.normal.1, spot.normal.2),
    );
    let euler = world.euler_degrees();
    set_active_object_transform(ObjectTransform {
        position: (world.translation.x, world.translation.y, world.translation.z),
        rotation_degrees: (euler.x, euler.y, euler.z),
        scale: (world.scale.x, world.scale.y, world.scale.z),
    })
}


/// One project in Husk's own folder, as the Home screen shows it: its name,
/// where it is, how long ago it changed, its size and a small picture of how
/// it looked when it was saved (raw RGBA pixels, empty when there is none).
pub struct ProjectCard {
    pub name: String,
    pub path: String,
    pub seconds_ago: u64,
    pub bytes: u64,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub thumb_rgba: Vec<u8>,
}

/// The projects in Husk's own folder, newest first.
#[flutter_rust_bridge::frb(sync)]
pub fn home_projects() -> Vec<ProjectCard> {
    crate::project::library::list()
        .into_iter()
        .map(|entry| {
            let (thumb_width, thumb_height, thumb_rgba) =
                crate::project::library::read_thumbnail(std::path::Path::new(&entry.path))
                    .unwrap_or((0, 0, Vec::new()));
            ProjectCard {
                name: entry.name,
                path: entry.path,
                seconds_ago: entry.seconds_ago,
                bytes: entry.bytes,
                thumb_width,
                thumb_height,
                thumb_rgba,
            }
        })
        .collect()
}

/// How many projects Husk keeps in its own folder at once.
#[flutter_rust_bridge::frb(sync)]
pub fn home_project_limit() -> usize {
    crate::project::library::MAX_PROJECTS
}

/// A path in Husk's own folder for a new project, or None when the folder is
/// already full.
#[flutter_rust_bridge::frb(sync)]
pub fn home_new_project_path() -> Option<String> {
    crate::project::library::new_path().map(|path| path.to_string_lossy().to_string())
}

/// Deletes a project (and its picture) from Husk's own folder. Returns
/// (true, message) or (false, reason).
#[flutter_rust_bridge::frb(sync)]
pub fn home_delete_project(path: String) -> (bool, String) {
    match crate::project::library::delete(&path) {
        Ok(()) => (true, String::from("Project deleted")),
        Err(reason) => (false, reason),
    }
}

/// Whether the scene has changes that are not in a saved project file.
#[flutter_rust_bridge::frb(sync)]
pub fn has_unsaved_changes() -> bool {
    crate::scene::has_unsaved_changes()
}

/// Marks the scene as having unsaved changes (for recovered work).
#[flutter_rust_bridge::frb(sync)]
pub fn mark_scene_unsaved() {
    crate::scene::mark_unsaved();
}

/// Starts a fresh, empty scene, as at start-up.
#[flutter_rust_bridge::frb(sync)]
pub fn new_scene() {
    crate::scene::reset_scene();
    let world = crate::scene::world();
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_world(&world));
    rebuild_renderer_from_scene();
}

/// Saves a picture of the viewport beside a project in Husk's own folder, for
/// the Home screen's card. Projects saved anywhere else get none.
fn save_thumbnail_if_managed(path: &str) {
    if !crate::project::library::is_managed(path) {
        return;
    }
    let thumbnail = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.last_frame_rgba().and_then(|(width, height, pixels)| {
            crate::project::library::shrink(pixels, width as usize, height as usize, 320)
        })
    })
    .flatten();
    if let Some((width, height, rgba)) = thumbnail {
        crate::project::library::write_thumbnail(std::path::Path::new(path), width, height, &rgba);
    }
}

/// Writes a recovery copy of the scene if it changed since the last one. It is
/// called every couple of minutes. Returns whether it wrote one.
#[flutter_rust_bridge::frb(sync)]
pub fn autosave_if_changed() -> bool {
    match crate::scene::autosave_if_changed() {
        Ok(wrote) => wrote,
        Err(reason) => {
            eprintln!("Husk: the recovery copy could not be written: {reason}");
            false
        }
    }
}

/// A recovery copy left by an earlier session: where it is and how many
/// seconds old it is.
pub struct RecoveryInfo {
    pub path: String,
    pub seconds_ago: u64,
}

/// The recovery copy an earlier session left, if there is one.
#[flutter_rust_bridge::frb(sync)]
pub fn recovery_info() -> Option<RecoveryInfo> {
    crate::project::recovery::recovery_info()
        .map(|(path, seconds_ago)| RecoveryInfo { path, seconds_ago })
}

/// Deletes the recovery copy.
#[flutter_rust_bridge::frb(sync)]
pub fn discard_recovery() {
    crate::project::recovery::clear_recovery();
}

/// Saves the whole scene (every object, the world and the output settings) to
/// `path` as a Husk project file. Returns (true, summary) or (false, reason).
#[flutter_rust_bridge::frb(sync)]
pub fn save_scene(path: String) -> (bool, String) {
    match crate::scene::with_scene(|scene| {
        crate::project::scene_file::write_scene(&path, scene)
    }) {
        Some(Ok(summary)) => {
            crate::project::recovery::clear_recovery();
            crate::scene::mark_saved();
            save_thumbnail_if_managed(&path);
            (true, summary)
        }
        Some(Err(reason)) => (false, reason),
        None => (false, String::from("There is no scene to save")),
    }
}

/// Opens a Husk project file, replacing the whole scene, and looks at it.
/// Returns (true, summary) or (false, reason); on false nothing changed.
#[flutter_rust_bridge::frb(sync)]
pub fn load_scene(path: String) -> (bool, String) {
    let scene = match crate::project::scene_file::read_scene(&path) {
        Ok(scene) => scene,
        Err(reason) => return (false, reason),
    };
    let summary = format!("Opened {} objects from {path}", scene.objects.len());
    let world = scene.world;
    crate::scene::replace_scene(scene);
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_world(&world));
    rebuild_renderer_from_scene();
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.frame_all());
    (true, summary)
}

/// Duplicates the active object beside itself and makes the copy the active
/// one. A copy keeps its original's parent, so a copy of an eye stays on the
/// same head. Returns the copy's index, or None when the active object has
/// nothing to copy.
#[flutter_rust_bridge::frb(sync)]
pub fn duplicate_active_object() -> Option<usize> {
    let index = crate::scene::duplicate_active()?;
    rebuild_renderer_from_scene();
    Some(index)
}

/// Copies the active object to the clipboard (Ctrl+C). False, changing
/// nothing, when it has nothing to copy.
#[flutter_rust_bridge::frb(sync)]
pub fn copy_active_object() -> bool {
    crate::scene::copy_active()
}

/// Pastes the clipboard as a new object (Ctrl+V) and makes it the active
/// one. Returns its index, or None when nothing was copied.
#[flutter_rust_bridge::frb(sync)]
pub fn paste_object() -> Option<usize> {
    let index = crate::scene::paste()?;
    rebuild_renderer_from_scene();
    Some(index)
}

/// Brings the renderer in step with the scene from scratch: every object is
/// drawn again from what the scene holds. Undo and redo use it, because they
/// can add, remove and change any object at once.
fn rebuild_renderer_from_scene() {
    let (states, active) = crate::scene::draw_states();
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        renderer.rebuild_objects(states, active)
    });
    push_scene_camera();
    push_scene_lights();
}

/// Goes back to the state before the last change to the scene. False,
/// changing nothing, when there is nothing to undo.
#[flutter_rust_bridge::frb(sync)]
pub fn undo_edit() -> bool {
    if !crate::scene::undo() {
        return false;
    }
    rebuild_renderer_from_scene();
    true
}

/// Goes forward again after an undo. False, changing nothing, when there is
/// nothing to redo.
#[flutter_rust_bridge::frb(sync)]
pub fn redo_edit() -> bool {
    if !crate::scene::redo() {
        return false;
    }
    rebuild_renderer_from_scene();
    true
}

/// Tells the renderer where every object is drawn: its world placement,
/// parents included. The scene is the source of truth, so this is called
/// after anything that moves an object that may have children.
fn push_scene_models() {
    let models = crate::scene::world_matrices();
    crate::platform::ffi::with_active_renderer_mut(|renderer| {
        for (index, model) in models.iter().enumerate() {
            renderer.set_object_model(index, *model);
        }
    });
}

/// Tells the renderer which lights the scene has and where they are. The
/// scene is the source of truth, so this is called after anything that
/// changes them.
fn push_scene_lights() {
    let lights = crate::scene::scene_lights();
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_lights(&lights));
}

/// The scene's world as the Properties panel shows it: the background
/// colour (red, green, blue, each 0 to 1), how strong the soft studio light
/// on surfaces is, and how strong the studio reflections are (each 0 to 2;
/// 1 is the usual look).
pub struct WorldSettings {
    pub background: (f32, f32, f32),
    pub ambient: f32,
    pub reflections: f32,
    /// The colour at the top of the background when the gradient is on; the
    /// background colour is then the one at the bottom.
    pub background_top: (f32, f32, f32),
    pub gradient: bool,
}

/// The scene's world.
#[flutter_rust_bridge::frb(sync)]
pub fn world_settings() -> WorldSettings {
    let world = crate::scene::world();
    WorldSettings {
        background: (world.background[0], world.background[1], world.background[2]),
        ambient: world.ambient,
        reflections: world.reflections,
        background_top: (
            world.background_top[0],
            world.background_top[1],
            world.background_top[2],
        ),
        gradient: world.gradient,
    }
}

/// Sets the scene's world and redraws it. Values outside their ranges are
/// held inside them. Returns false, changing nothing, for any value that is
/// not a finite number.
#[flutter_rust_bridge::frb(sync)]
pub fn set_world_settings(settings: WorldSettings) -> bool {
    let values = [
        settings.background.0,
        settings.background.1,
        settings.background.2,
        settings.ambient,
        settings.reflections,
    ];
    if !values.iter().all(|value| value.is_finite()) {
        return false;
    }
    let top = settings.background_top;
    if !(top.0.is_finite() && top.1.is_finite() && top.2.is_finite()) {
        return false;
    }
    let world = crate::scene::world::World {
    background: [settings.background.0, settings.background.1, settings.background.2],
        ambient: settings.ambient,
        reflections: settings.reflections,
        background_top: [top.0, top.1, top.2],
        gradient: settings.gradient,
    }
    .clamped();
    crate::scene::set_world(world);
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_world(&world));
    true
}

/// Tells the renderer which scene camera to look through, where it is, its
/// lens and the output frame's shape. The scene is the source of truth, so
/// this is called after anything that changes them.
fn push_scene_camera() {
    let aspect = crate::scene::output().aspect();
    let camera = crate::scene::scene_camera().map(|(transform, lens)| {
        crate::render::renderer::CameraView {
            world: transform.placement_matrix(),
            fov_degrees: lens.fov_degrees,
            frame_aspect: aspect,
        }
    });
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_scene_camera(camera));
}

/// Starts or stops looking through the scene camera (the first camera in
/// the scene). Returns whether the viewport is looking through it now:
/// false when asked to start but the scene has no camera.
#[flutter_rust_bridge::frb(sync)]
pub fn set_camera_view(on: bool) -> bool {
    push_scene_camera();
    crate::platform::ffi::with_active_renderer_mut(|renderer| renderer.set_camera_view(on))
        .unwrap_or(false)
}

/// Whether the viewport is looking through the scene camera.
#[flutter_rust_bridge::frb(sync)]
pub fn camera_view_active() -> bool {
    crate::platform::ffi::with_active_renderer(|renderer| renderer.camera_view_active())
        .unwrap_or(false)
}

/// While looking through the scene camera: where the output frame sits in
/// the viewport, as (x, y, width, height) in render pixels. None otherwise.
#[flutter_rust_bridge::frb(sync)]
pub fn camera_frame_rect() -> Option<(f32, f32, f32, f32)> {
    crate::platform::ffi::with_active_renderer(|renderer| renderer.camera_frame_rect()).flatten()
}

/// The finished video's width, height (pixels) and frame rate.
#[flutter_rust_bridge::frb(sync)]
pub fn output_settings() -> (u32, u32, u32) {
    let output = crate::scene::output();
    (output.width, output.height, output.fps)
}

/// Sets the finished video's width, height and frame rate. Sizes are held
/// between 16 and 7680 and made even, and the rate between 1 and 120.
/// Returns the values that were kept.
#[flutter_rust_bridge::frb(sync)]
pub fn set_output_settings(width: u32, height: u32, fps: u32) -> (u32, u32, u32) {
    let kept = crate::scene::output::Output { width, height, fps }.clamped();
    crate::scene::set_output(kept);
    push_scene_camera();
    (kept.width, kept.height, kept.fps)
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
flutter_rust_bridge::setup_default_user_utils();
}
