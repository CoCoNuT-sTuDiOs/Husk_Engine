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
    let hit = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.pick_inside(x as f32, y as f32)
    })
    .flatten()?;

    // The hit is in the object's own space, where its skeleton lives. The
    // transform is taken before locking the scene for the bones.
    let space = crate::scene::active_transform().1;
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
    crate::scene::active_object()
        .timeline
        .keys
        .iter()
        .map(|key| key.time)
        .collect()
}

/// Deletes the keyframe nearest `time` if one is within 0.05 s of it.
/// Returns whether a key was removed.
#[flutter_rust_bridge::frb(sync)]
pub fn delete_keyframe_at(time: f32) -> bool {
    crate::scene::active_object()
        .timeline
        .remove_near(time, 0.05)
}

/// Poses the character as the keyframes say it looks at `time` (seconds).
/// Returns false, leaving the pose alone, if there are no keyframes.
#[flutter_rust_bridge::frb(sync)]
pub fn seek_to_time(time: f32) -> bool {
    let skin_matrices = {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, timeline, .. } = &mut *active;
        let Some(rotations) = timeline.sample(time, skeleton.bones.len()) else {
            return false;
        };
        pose.rotations = rotations;
        pose.root_offset = timeline.sample_offset(time).unwrap_or(glam::Vec3::ZERO);
        pose.skinning_matrices(&skeleton)
    };
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&skin_matrices));
    true
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

/// Saves the skeleton, skin weights and animation to `path` as a JSON
/// file. Returns (true, summary) on success or (false, reason) on failure.
#[flutter_rust_bridge::frb(sync)]
pub fn save_project(path: String) -> (bool, String) {
    let vertex_count =
        crate::platform::ffi::with_active_renderer(|renderer| renderer.mesh_positions().len()).unwrap_or(0);
    let active = crate::scene::active_object();
    let skeleton = &active.skeleton;
    let weights = &active.weights;
    let timeline = &active.timeline;    
    match crate::project::write_project(
        &path,
        vertex_count,
        &skeleton,
        weights.as_deref(),
        &timeline,
    ) {
        Ok(()) => (
            true,
            format!(
                "Saved {} bones, {} keys and the skin weights to {path}",
                skeleton.bones.len(),
                timeline.keys.len()
            ),
        ),
        Err(reason) => (false, reason),
    }
}

/// Loads a project saved with `save_project` onto the model that is
/// currently open, replacing the skeleton, skin weights and animation.
/// The project must have been saved for a model with the same number of
/// vertices. Returns (true, summary) or (false, reason).
#[flutter_rust_bridge::frb(sync)]
pub fn load_project(path: String) -> (bool, String) {
    let project = match crate::project::read_project(&path) {
        Ok(project) => project,
        Err(reason) => return (false, reason),
    };
    let vertex_count =
        crate::platform::ffi::with_active_renderer(|renderer| renderer.mesh_positions().len()).unwrap_or(0);
    if project.vertex_count != vertex_count {
        return (
            false,
            format!(
                "This project was saved for a model with {} vertices, but the open model has {vertex_count}",
                project.vertex_count
            ),
        );
    }

    // Skin weights go to the GPU; a project saved without any gets the
    // plain default (everything on bone 0).
    let gpu_weights: Vec<crate::rig::skinning::VertexWeights> = match &project.weights {
        Some(weights) => weights.clone(),
        None => vec![
            crate::rig::skinning::VertexWeights {
                joint_indices: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            };
            vertex_count
        ],
    };
    crate::platform::ffi::with_active_renderer(|renderer| renderer.update_vertex_weights(&gpu_weights));

    let summary = format!(
        "Loaded {} bones and {} keys from {path}",
        project.skeleton.bones.len(),
        project.timeline.keys.len()
    );
    {
        let mut active = crate::scene::active_object();
        let crate::scene::Object { skeleton, pose, weights, timeline, .. } = &mut *active;
        *pose = crate::animation::pose::Pose::rest_for(&project.skeleton);
        *skeleton = project.skeleton;
        *weights = project.weights;
        *timeline = project.timeline;
    }
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&[]));
    (true, summary)
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
    {
        let mut active = crate::scene::active_object();
        let name = std::mem::take(&mut active.name);
        let transform = active.transform;
        let material = active.material;
        *active = crate::scene::Object {
            name,
            transform,
            material,
            ..crate::scene::Object::default()
        };

    }
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_bone_matrices(&[]));
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
pub fn add_shape(name: String, x: f64, y: f64) -> Option<usize> {
    let mesh = crate::geometry::shapes::by_name(&name)?;
    let (render_index, model, colour) = crate::platform::ffi::with_active_renderer_mut(|renderer| {
    renderer.add_shape_at_screen(mesh, x as f32, y as f32)
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
    let index = crate::platform::ffi::with_active_renderer(|renderer| {
        renderer.pick_object(x as f32, y as f32)
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
    if scene_index != render_index {
        eprintln!(
            "Husk: scene has object {scene_index} but the renderer has {render_index}; the two lists are out of step"
        );
    }
    push_scene_camera();
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

/// Tells the renderer which lights the scene has and where they are. The
/// scene is the source of truth, so this is called after anything that
/// changes them.
fn push_scene_lights() {
    let lights = crate::scene::scene_lights();
    crate::platform::ffi::with_active_renderer(|renderer| renderer.set_lights(&lights));
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
