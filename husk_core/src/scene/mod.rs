pub mod transform;
pub mod camera;
pub mod material;
pub mod output;
pub mod light;
use crate::rig::Skeleton;
use crate::rig::skinning::VertexWeights;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

/// What kind of thing an object is. A mesh is drawn as geometry; a camera
/// is drawn as a small camera shape and carries a lens.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ObjectKind {
    #[default]
    Mesh,
    Camera(camera::Camera),
    Light(light::Light),
}
/// Everything that belongs to one animated thing in a scene.
#[derive(Default)]
pub struct Object {
    /// What the outliner calls it.
        pub name: String,
    /// Where the object sits in the scene. The scene owns this; the
    /// renderer's matrix is only the GPU-side copy of it.
    pub transform: transform::Transform,
    /// How the object's surface looks. The scene owns this too.
    pub material: material::Material,
    /// Whether this is a mesh or a camera.
    pub kind: ObjectKind,
    pub skeleton: Skeleton,
    pub weights: Option<Vec<VertexWeights>>,
    pub pose: crate::animation::pose::Pose,
    pub timeline: crate::animation::keyframes::Timeline,
}

/// One shot: a list of objects, and which one the tools act on.
#[derive(Default)]
pub struct Scene {
    pub objects: Vec<Object>,
    pub active_object: usize,
    /// What the finished video looks like.
    pub output: output::Output,
}

impl Scene {
    /// Removes the object at `index`. Object 0, the loaded model, cannot
    /// be removed, and neither can an index that does not exist: both
    /// return false and change nothing. The active object stays the same
    /// object, or becomes object 0 when the active one was removed.
    pub fn remove_object(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.objects.len() {
            return false;
        }
        self.objects.remove(index);
        self.active_object = active_after_removal(self.active_object, index);
        true
    }

    /// Sets the transform of the object at `index`. False, changing
    /// nothing, for an unknown index.
    pub fn set_transform(&mut self, index: usize, transform: transform::Transform) -> bool {
        match self.objects.get_mut(index) {
    Some(object) => {
                object.transform = transform;
                true
            }
            None => false,
        }
    }

    /// Sets the material of the object at `index`. False, changing
    /// nothing, for an unknown index.
    pub fn set_material(&mut self, index: usize, material: material::Material) -> bool {
        match self.objects.get_mut(index) {
            Some(object) => {
                object.material = material;
                true
            }
            None => false,
        }
    }

    /// Sets the lens of the camera at `index`. False, changing nothing, if
    /// there is no such object or it is not a camera.
    pub fn set_camera(&mut self, index: usize, lens: camera::Camera) -> bool {
        match self.objects.get_mut(index) {
            Some(Object {
                kind: ObjectKind::Camera(current),
                ..
            }) => {
                *current = lens;
                true
            }
            _ => false,
        }
    }

    /// Sets the light of the object at `index`. False, changing nothing, if
    /// there is no such object or it is not a light.
    pub fn set_light(&mut self, index: usize, light: light::Light) -> bool {
        match self.objects.get_mut(index) {
            Some(Object {
                kind: ObjectKind::Light(current),
                ..
            }) => {
                *current = light;
                true
            }
            _ => false,
        }
    }

    /// The scene's lights, in list order, each with its transform: the
    /// ones the renderer shines, at most `light::MAX_LIGHTS`.
    pub fn lights(&self) -> Vec<(transform::Transform, light::Light)> {
        self.objects
            .iter()
            .filter_map(|object| match object.kind {
                ObjectKind::Light(light) => Some((object.transform, light)),
                _ => None,
            })
            .take(light::MAX_LIGHTS)
            .collect()
    }

    /// The index of the scene camera: the first camera in the list, which
    /// is the one the viewport looks through in camera view.
    pub fn first_camera(&self) -> Option<usize> {
        self.objects
            .iter()
            .position(|object| matches!(object.kind, ObjectKind::Camera(_)))
    }
}

pub fn active_after_removal(active: usize, removed: usize) -> usize {
    if active == removed {
        0
    } else if active > removed {
        active - 1
    } else {
        active
    }
}

/// The whole project: a list of scenes, and which one is open.
pub struct Project {
    pub scenes: Vec<Scene>,
    pub active_scene: usize,
}

static PROJECT: Mutex<Project> = Mutex::new(Project {
    scenes: Vec::new(),
    active_scene: 0,
});

/// Holds the project lock and gives access to the active object.
/// Never call another function that calls `active_object()` while you
/// hold one of these: the lock is not reentrant and it would deadlock.
pub struct ActiveObject {
    guard: MutexGuard<'static, Project>,
}

impl Deref for ActiveObject {
    type Target = Object;

    fn deref(&self) -> &Object {
        let scene = &self.guard.scenes[self.guard.active_scene];
        &scene.objects[scene.active_object]
    }
}

impl DerefMut for ActiveObject {
    fn deref_mut(&mut self) -> &mut Object {
        let scene_index = self.guard.active_scene;
        let scene = &mut self.guard.scenes[scene_index];
        let object_index = scene.active_object;
        &mut scene.objects[object_index]
    }
}

/// Locks the project and returns its active object. The first call
/// creates the one scene with one object that exists today.
pub fn active_object() -> ActiveObject {
    let mut guard = PROJECT.lock().unwrap();
    if guard.scenes.is_empty() {
        guard.scenes.push(Scene::default());
        guard.active_scene = 0;
    }
    let scene_index = guard.active_scene;
    let scene = &mut guard.scenes[scene_index];
    if scene.objects.is_empty() {
        scene.objects.push(Object::default());
        scene.active_object = 0;
    }
    ActiveObject { guard }
}
pub fn add_object(
    base: &str,
    transform: transform::Transform,
    material: material::Material,
    kind: ObjectKind,
) -> usize {
    let mut guard = PROJECT.lock().unwrap();
    if guard.scenes.is_empty() {
        guard.scenes.push(Scene::default());
        guard.active_scene = 0;
    }
    let scene_index = guard.active_scene;
    let scene = &mut guard.scenes[scene_index];
    if scene.objects.is_empty() {
        scene.objects.push(Object::default());
        scene.active_object = 0;
    }
    let index = scene.objects.len();
    scene.objects.push(Object {
        name: format!("{base} {index}"),
        transform,
        material,
        kind,
        ..Object::default()
    });
    index
}

/// Makes `index` the active object of the active scene, the one the tools
/// act on. Returns false, changing nothing, if there is no such object.
pub fn set_active_object(index: usize) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    let Some(scene) = guard.scenes.get_mut(scene_index) else {
        return false;
    };
    if index >= scene.objects.len() {
        return false;
    }
    scene.active_object = index;
    true
}

/// Removes the active object of the active scene (see `Scene::remove_object`
/// for what cannot be removed). Returns the index that was removed.
pub fn remove_active_object() -> Option<usize> {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    let scene = guard.scenes.get_mut(scene_index)?;
    let index = scene.active_object;
    if scene.remove_object(index) {
        Some(index)
    } else {
        None
    }
}

/// The active object's index and transform.
pub fn active_transform() -> (usize, transform::Transform) {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => {
            let transform = scene
                .objects
                .get(scene.active_object)
                .map_or_else(transform::Transform::default, |object| object.transform);
            (scene.active_object, transform)
        }
        None => (0, transform::Transform::default()),
    }
}

/// Sets the active object's transform (see `Scene::set_transform` for
/// what is refused). Returns the index of the object that was changed.
pub fn set_active_transform(transform: transform::Transform) -> Option<usize> {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    let scene = guard.scenes.get_mut(scene_index)?;
    let index = scene.active_object;
    if scene.set_transform(index, transform) {
        Some(index)
    } else {
        None
    }
}

/// The active object's index and material.
pub fn active_material() -> (usize, material::Material) {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => {
            let material = scene
                .objects
                .get(scene.active_object)
                .map_or_else(material::Material::default, |object| object.material);
            (scene.active_object, material)
        }
        None => (0, material::Material::default()),
    }
}

/// Sets the active object's material. Returns the index of the object
/// that was changed.
pub fn set_active_material(material: material::Material) -> Option<usize> {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    let scene = guard.scenes.get_mut(scene_index)?;
    let index = scene.active_object;
    if scene.set_material(index, material) {
        Some(index)
    } else {
        None
    }
}

/// The active object's lens, if the active object is a camera.
pub fn active_camera() -> Option<camera::Camera> {
    let guard = PROJECT.lock().unwrap();
    let scene = guard.scenes.get(guard.active_scene)?;
    match scene.objects.get(scene.active_object)?.kind {
        ObjectKind::Camera(lens) => Some(lens),
        ObjectKind::Mesh | ObjectKind::Light(_) => None,
    }
}

/// Sets the active object's lens. False, changing nothing, if the active
/// object is not a camera.
pub fn set_active_camera(lens: camera::Camera) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => {
            let index = scene.active_object;
            scene.set_camera(index, lens)
        }
        None => false,
    }
}

/// The scene camera's placement and lens, if the scene has a camera.
pub fn scene_camera() -> Option<(transform::Transform, camera::Camera)> {
    let guard = PROJECT.lock().unwrap();
    let scene = guard.scenes.get(guard.active_scene)?;
    let object = scene.objects.get(scene.first_camera()?)?;
    match object.kind {
        ObjectKind::Camera(lens) => Some((object.transform, lens)),
        ObjectKind::Mesh | ObjectKind::Light(_) => None,
    }
}

/// The active object's light, if the active object is a light.
pub fn active_light() -> Option<light::Light> {
    let guard = PROJECT.lock().unwrap();
    let scene = guard.scenes.get(guard.active_scene)?;
    match scene.objects.get(scene.active_object)?.kind {
        ObjectKind::Light(light) => Some(light),
        ObjectKind::Mesh | ObjectKind::Camera(_) => None,
    }
}

/// Sets the active object's light. False, changing nothing, if the active
/// object is not a light.
pub fn set_active_light(light: light::Light) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => {
            let index = scene.active_object;
            scene.set_light(index, light)
        }
        None => false,
    }
}

/// The active scene's lights and where they are (see `Scene::lights`).
pub fn scene_lights() -> Vec<(transform::Transform, light::Light)> {
    let guard = PROJECT.lock().unwrap();
    guard
        .scenes
        .get(guard.active_scene)
        .map_or_else(Vec::new, |scene| scene.lights())
}

/// What the finished video looks like: its size and frame rate.
pub fn output() -> output::Output {
let guard = PROJECT.lock().unwrap();
    guard
        .scenes
        .get(guard.active_scene)
        .map_or_else(output::Output::default, |scene| scene.output)
}

/// Sets what the finished video looks like. The settings are kept as given;
/// hold them inside their ranges first (`Output::clamped`).
pub fn set_output(settings: output::Output) {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    if let Some(scene) = guard.scenes.get_mut(scene_index) {
        scene.output = settings;
    }
}

/// The names of the active scene's objects, in order, and the index of
/// the active one.
pub fn object_list() -> (Vec<String>, usize) {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => (
            scene.objects.iter().map(|object| object.name.clone()).collect(),
            scene.active_object,
        ),
        None => (Vec::new(), 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with(count: usize, active: usize) -> Scene {
let mut scene = Scene::default();
        for _ in 0..count {
            scene.objects.push(Object::default());
        }
        scene.active_object = active;
        scene
    }

    #[test]
    fn removing_before_the_active_object_keeps_it_active() {
        let mut scene = scene_with(4, 3);
        assert!(scene.remove_object(1));
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.active_object, 2);
    }

    #[test]
    fn removing_after_the_active_object_changes_nothing_for_it() {
        let mut scene = scene_with(4, 1);
        assert!(scene.remove_object(3));
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.active_object, 1);
    }

    #[test]
    fn removing_the_active_object_hands_over_to_object_zero() {
        let mut scene = scene_with(4, 2);
        assert!(scene.remove_object(2));
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.active_object, 0);
    }
    #[test]
    fn set_transform_moves_any_object_and_rejects_unknown_ones() {
        let mut scene = scene_with(3, 1);
        let mut moved = transform::Transform::default();
        moved.translation = glam::Vec3::new(1.0, 2.0, 3.0);
        assert!(scene.set_transform(2, moved));
        assert_eq!(scene.objects[2].transform, moved);
        // The loaded model moves like any other object.
        assert!(scene.set_transform(0, moved));
        assert_eq!(scene.objects[0].transform, moved);
        // The others stay where they were.
        assert_eq!(scene.objects[1].transform, transform::Transform::default());
        assert!(!scene.set_transform(9, moved));
    }
    
    #[test]
    fn set_material_changes_only_the_object_asked_for() {
        let mut scene = scene_with(3, 1);
        let red = material::Material {
            base_color: [1.0, 0.0, 0.0],
            roughness: 0.9,
            metalness: 0.1,
        };
        assert!(scene.set_material(2, red));
        assert_eq!(scene.objects[2].material, red);
        assert_eq!(scene.objects[0].material, material::Material::default());
        assert_eq!(scene.objects[1].material, material::Material::default());
        assert!(!scene.set_material(9, red));
    }

    #[test]
    fn set_camera_changes_a_camera_and_nothing_else() {
        let mut scene = scene_with(3, 1);
        scene.objects[1].kind = ObjectKind::Camera(camera::Camera::default());
        let wide = camera::Camera { fov_degrees: 80.0 };
        assert!(scene.set_camera(1, wide));
        assert_eq!(scene.objects[1].kind, ObjectKind::Camera(wide));
        // A mesh is not a camera, and 9 is not an object.
        assert!(!scene.set_camera(2, wide));
        assert_eq!(scene.objects[2].kind, ObjectKind::Mesh);
        assert!(!scene.set_camera(9, wide));
    }

    #[test]
    fn set_light_changes_a_light_and_nothing_else() {
        let mut scene = scene_with(3, 1);
        scene.objects[1].kind = ObjectKind::Light(light::Light::sun());
        let lamp = light::Light::point();
        assert!(scene.set_light(1, lamp));
        assert_eq!(scene.objects[1].kind, ObjectKind::Light(lamp));
        // A mesh is not a light, and 9 is not an object.
        assert!(!scene.set_light(2, lamp));
        assert_eq!(scene.objects[2].kind, ObjectKind::Mesh);
        assert!(!scene.set_light(9, lamp));
    }

    #[test]
    fn the_scene_lights_are_the_light_objects_in_order_and_stop_at_the_cap() {
        let mut scene = scene_with(12, 0);
        assert!(scene.lights().is_empty());
        scene.objects[3].kind = ObjectKind::Light(light::Light::point());
        scene.objects[3].transform.translation = glam::Vec3::new(1.0, 2.0, 3.0);
        scene.objects[1].kind = ObjectKind::Light(light::Light::sun());
        // A camera is not a light.
        scene.objects[2].kind = ObjectKind::Camera(camera::Camera::default());
        let lights = scene.lights();
        assert_eq!(lights.len(), 2);
        assert_eq!(lights[0].1, light::Light::sun());
        assert_eq!(lights[1].1, light::Light::point());
        assert_eq!(lights[1].0.translation, glam::Vec3::new(1.0, 2.0, 3.0));

        // Eleven lights in the list: only the first eight shine.
        for object in scene.objects.iter_mut().skip(1) {
            object.kind = ObjectKind::Light(light::Light::point());
        }
        assert_eq!(scene.lights().len(), light::MAX_LIGHTS);
    }

    #[test]
    fn the_scene_camera_is_the_first_camera_in_the_list() {
    let mut scene = scene_with(4, 0);
        assert_eq!(scene.first_camera(), None);
        scene.objects[3].kind = ObjectKind::Camera(camera::Camera::default());
        assert_eq!(scene.first_camera(), Some(3));
        scene.objects[2].kind = ObjectKind::Camera(camera::Camera::default());
        assert_eq!(scene.first_camera(), Some(2));
        // Removing the first camera hands over to the next one, which moves
        // down into its place.
        assert!(scene.remove_object(2));
        assert_eq!(scene.first_camera(), Some(2));
    }

    #[test]
    fn the_loaded_model_and_bad_indices_cannot_be_removed() {
    let mut scene = scene_with(3, 1);
        assert!(!scene.remove_object(0));
        assert!(!scene.remove_object(3));
        assert!(!scene.remove_object(99));
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.active_object, 1);
    }
}