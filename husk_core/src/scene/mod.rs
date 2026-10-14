use crate::rig::Skeleton;
use crate::rig::skinning::VertexWeights;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

/// Everything that belongs to one animated thing in a scene.
#[derive(Default)]
pub struct Object {
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

/// Adds an empty object to the active scene and returns its index. The
/// renderer's list of drawn objects must grow in step with this one.
/// Like `active_object()`, the first call creates the starting scene
/// and its first object, so the new object never takes index 0.
pub fn add_object() -> usize {
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
    scene.objects.push(Object::default());
    scene.objects.len() - 1
}