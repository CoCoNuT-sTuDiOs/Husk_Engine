use crate::rig::Skeleton;
use crate::rig::skinning::VertexWeights;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

/// Everything that belongs to one animated thing in a scene.
#[derive(Default)]
pub struct Object {
    /// What the outliner calls it.
    pub name: String,
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
}

/// Which index is active once `removed` has been taken out of the list:
/// objects after it shift down by one, and if the active object itself
/// went, object 0 takes over. The renderer uses this too, so both lists
/// always agree.
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
pub fn add_object(base: &str) -> usize {
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
    fn the_loaded_model_and_bad_indices_cannot_be_removed() {
        let mut scene = scene_with(3, 1);
        assert!(!scene.remove_object(0));
        assert!(!scene.remove_object(3));
        assert!(!scene.remove_object(99));
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.active_object, 1);
    }
}