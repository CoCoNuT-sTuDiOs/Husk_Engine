pub mod transform;
pub mod camera;
pub mod material;
pub mod output;
pub mod world;
pub mod light;
use crate::animation::channels::{Channels, Interpolation, Property, Value};
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
/// How an object hangs below another one: which object is its parent (by
/// id, since list positions shift when objects are removed) and the
/// "parent inverse" matrix that keeps the child where it was when it was
/// parented. The child's world matrix is the parent's world matrix times
/// `inverse` times the child's own local matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parenting {
    pub parent_id: u64,
pub inverse: glam::Mat4,
    /// The bone of the parent's skeleton that the child follows through the
    /// pose, if it follows one.
    pub bone: Option<usize>,
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
    /// The object's animation of everything except its bones: placement,
    /// look, lens and light.
    pub channels: Channels,
    /// A number that names this object for good (0 means it has none yet).
    /// Parent links use it, because list positions shift when an object is
    /// removed.
    pub id: u64,
    /// The object this one hangs below, if any. `transform` is then
    /// relative to the parent.
    pub parent: Option<Parenting>,
    /// A locked object stays exactly as it is: it cannot be picked in the
    /// viewport, moved, rotated, scaled or deleted. It still draws and
    /// still plays its animation.
    pub locked: bool,
}

impl Object {
    /// Every moment (seconds) that has a key on this object, bones or
    /// anything else, earliest first, with keys at nearly the same moment
    /// counted once.
    pub fn key_times(&self) -> Vec<f32> {
        let mut all: Vec<f32> = self.timeline.keys.iter().map(|key| key.time).collect();
        all.extend(self.channels.key_times());
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all.dedup_by(|a, b| (*a - *b).abs() < 0.005);
        all
    }

    /// Deletes the keys closest to `time` (within `tolerance` seconds) in
    /// the pose and in every channel. Returns whether any key was removed.
    pub fn remove_keys_near(&mut self, time: f32, tolerance: f32) -> bool {
        let pose_removed = self.timeline.remove_near(time, tolerance);
        let channels_removed = self.channels.remove_near(time, tolerance);
        pose_removed || channels_removed
    }

    /// Records what this object looks like and where it is as keys at
    /// `time` (seconds): position and rotation for everything, then scale,
    /// colour, roughness and metalness for a mesh, the field of view for a
    /// camera, or colour and intensity for a light. (A camera's or light's
    /// scale is only the size of its viewport shape, so it is not keyed.)
    pub fn key_channels(&mut self, time: f32) {
        let transform = self.transform;
        let material = self.material;
        let smooth = Interpolation::Smooth;
        let channels = &mut self.channels;
        channels.set_key(
            Property::Position,
            time,
            Value::Vector(transform.translation),
            smooth,
        );
        channels.set_key(
            Property::Rotation,
            time,
            Value::Rotation(transform.rotation),
            smooth,
        );
        match self.kind {
            ObjectKind::Mesh => {
                channels.set_key(Property::Scale, time, Value::Vector(transform.scale), smooth);
                channels.set_key(
                    Property::Colour,
                    time,
                    Value::Vector(glam::Vec3::from(material.base_color)),
                    smooth,
                );
                channels.set_key(Property::Roughness, time, Value::Scalar(material.roughness), smooth);
                channels.set_key(Property::Metalness, time, Value::Scalar(material.metalness), smooth);
            }
            ObjectKind::Camera(lens) => {
                channels.set_key(Property::CameraFov, time, Value::Scalar(lens.fov_degrees), smooth);
            }
            ObjectKind::Light(light) => {
                channels.set_key(
                    Property::LightColour,
                    time,
                    Value::Vector(glam::Vec3::from(light.color)),
                    smooth,
                );
                channels.set_key(
                    Property::LightIntensity,
                    time,
                    Value::Scalar(light.intensity),
                    smooth,
                );
            }
        }
    }

    /// Sets what this object looks like and where it is to what its
    /// channels say at `time` (seconds). Properties without a channel are
    /// left as they are, and values are held inside their ranges. Returns
    /// whether any channel supplied a value.
    pub fn apply_channels(&mut self, time: f32) -> bool {
        // The object's repeat (set on its pose timeline) loops its
        // channels too. Keys are always made together, so the pose keys and
        // the channel keys start and end at the same moments.
        self.channels.loop_period = self.timeline.effective_period();
        let mut applied = false;
        let mut transform = self.transform;
        if let Some(Value::Vector(position)) = self.channels.sample(Property::Position, time) {
            transform.translation = position;
            applied = true;
        }
        if let Some(Value::Rotation(rotation)) = self.channels.sample(Property::Rotation, time) {
            transform.rotation = rotation;
            applied = true;
        }
        if let Some(Value::Vector(scale)) = self.channels.sample(Property::Scale, time) {
            transform.scale = transform::clamp_scale(scale);
            applied = true;
        }
        self.transform = transform;

        let mut material = self.material;
        if let Some(Value::Vector(colour)) = self.channels.sample(Property::Colour, time) {
            material.base_color = colour.to_array();
            applied = true;
        }
        if let Some(Value::Scalar(roughness)) = self.channels.sample(Property::Roughness, time) {
            material.roughness = roughness;
            applied = true;
        }
        if let Some(Value::Scalar(metalness)) = self.channels.sample(Property::Metalness, time) {
            material.metalness = metalness;
            applied = true;
        }
        self.material = material.clamped();

        match &mut self.kind {
            ObjectKind::Camera(lens) => {
                if let Some(Value::Scalar(fov)) = self.channels.sample(Property::CameraFov, time) {
                    *lens = camera::Camera { fov_degrees: fov }.clamped();
                    applied = true;
                }
            }
            ObjectKind::Light(light) => {
                let mut next = *light;
                if let Some(Value::Vector(colour)) =
                    self.channels.sample(Property::LightColour, time)
                {
                    next.color = colour.to_array();
                    applied = true;
                }
                if let Some(Value::Scalar(intensity)) =
                    self.channels.sample(Property::LightIntensity, time)
                {
                    next.intensity = intensity;
                    applied = true;
                }
                *light = next.clamped();
            }
            ObjectKind::Mesh => {}
        }
        applied
    }
}

/// One object as the renderer should draw it after the animation was
/// evaluated at some moment.
pub struct ObjectFrame {
    pub index: usize,
    pub model: glam::Mat4,
    pub material: material::Material,
    /// The skinning matrices, if the object has pose keys.
    pub skinning: Option<Vec<glam::Mat4>>,
    /// Whether the channels set the object's placement or look.
    pub placed: bool,
}

/// One shot: a list of objects, and which one the tools act on.
#[derive(Default)]
pub struct Scene {
    pub objects: Vec<Object>,
    pub active_object: usize,
    /// What the finished video looks like.
    pub output: output::Output,
    /// The space around the objects: background and studio light.
    pub world: world::World,
    /// The id the next object that needs one gets.
    pub next_id: u64,
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
     // A locked object cannot be deleted.
        if self.objects[index].locked {
            return false;
        }
        // The object's children stay where they are and move up to its parent.
        let removed_id = self.objects[index].id;
        if removed_id != 0 {
            let grandparent = self.parent_of(index);
            let children: Vec<usize> = (0..self.objects.len())
                .filter(|&i| {
                    self.objects[i].parent.map(|link| link.parent_id) == Some(removed_id)
                })
                .collect();
            for child in children {
                self.set_parent(child, grandparent);
            }
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
            .enumerate()
            .filter_map(|(index, object)| match object.kind {
            ObjectKind::Light(light) => Some((self.world_transform(index), light)),
                _ => None,
            })
            .take(light::MAX_LIGHTS)
            .collect()
    }

    /// Sets every object of the scene to how its animation says it is at
    /// `time` (seconds): the pose from the pose keys, and placement, look,
    /// lens and light from the channels. Objects without keys are left as
    /// they are. Returns what the renderer needs for each object, in list
    /// order.
pub fn evaluate_at(&mut self, time: f32) -> Vec<ObjectFrame> {
        let mut frames = self.evaluate_objects_at(time);
        // Each object's own placement is relative to its parent, so the
        // drawn placement comes from the whole chain.
        for frame in frames.iter_mut() {
            frame.model = self.world_matrix(frame.index);
        }
        frames
    }

    /// Evaluates every object on its own. The model matrix in each frame is
    /// the object's own placement, before its parents are applied.
    fn evaluate_objects_at(&mut self, time: f32) -> Vec<ObjectFrame> {
    self.objects
            .iter_mut()
            .enumerate()
            .map(|(index, object)| {
                let skinning = match object
                    .timeline
                    .sample(time, object.skeleton.bones.len())
                {
                    Some(rotations) => {
                        object.pose.rotations = rotations;
                        object.pose.root_offset = object
                            .timeline
                            .sample_offset(time)
                            .unwrap_or(glam::Vec3::ZERO);
                        Some(object.pose.skinning_matrices(&object.skeleton))
                    }
                    None => None,
                };
                let placed = object.apply_channels(time);
                ObjectFrame {
                    index,
                    model: object.transform.to_matrix(),
                    material: object.material,
                    skinning,
                    placed,
                }
            })
            .collect()
    }

    /// How far a bone of the object at `object` has moved from its rest
    /// position in the current pose, in that object's own space. The
    /// identity for an unknown object or bone.
    pub fn bone_matrix(&self, object: usize, bone: usize) -> glam::Mat4 {
        let Some(object) = self.objects.get(object) else {
            return glam::Mat4::IDENTITY;
        };
        if bone >= object.skeleton.bones.len() {
            return glam::Mat4::IDENTITY;
        }
        let posed = object.pose.posed_world_matrices(&object.skeleton);
        let rest = object.skeleton.world_bind_matrices();
        posed[bone] * rest[bone].inverse()
    }

    /// The id of the object at `index`, giving it one if it has none yet.
    fn id_of(&mut self, index: usize) -> u64 {
        if self.objects[index].id == 0 {
            self.next_id += 1;
            self.objects[index].id = self.next_id;
        }
        self.objects[index].id
    }

    /// The index of the object with `id` (an id of 0 means none).
    pub fn index_of_id(&self, id: u64) -> Option<usize> {
        if id == 0 {
            return None;
        }
        self.objects.iter().position(|object| object.id == id)
    }

    /// The index of the parent of the object at `index`, if it has one.
    pub fn parent_of(&self, index: usize) -> Option<usize> {
        let link = self.objects.get(index)?.parent?;
        self.index_of_id(link.parent_id)
    }

    /// Where the object at `index` is in the scene, parents included: its
    /// own placement, then each parent's, up the chain.
    pub fn world_matrix(&self, index: usize) -> glam::Mat4 {
        let Some(object) = self.objects.get(index) else {
            return glam::Mat4::IDENTITY;
        };
        let mut matrix = object.transform.to_matrix();
        let mut current = index;
        for _ in 0..self.objects.len() {
            let (Some(parent_index), Some(link)) =
                (self.parent_of(current), self.objects[current].parent)
            else {
                break;
            };
let bone = match link.bone {
                Some(bone) => self.bone_matrix(parent_index, bone),
                None => glam::Mat4::IDENTITY,
            };
            matrix = self.objects[parent_index].transform.to_matrix() * bone * link.inverse * matrix;
            current = parent_index;
        }
        matrix
    }

    /// The object's world placement as a position, rotation and scale. An
    /// object with no parent gives its own transform exactly; with
    /// parents, any shear is dropped.
    pub fn world_transform(&self, index: usize) -> transform::Transform {
        match self.objects.get(index) {
            Some(object) if self.parent_of(index).is_none() => object.transform,
            Some(_) => transform::Transform::from_matrix(self.world_matrix(index)),
            None => transform::Transform::default(),
        }
    }

    /// Places the object at `index` at `world` in the scene, working out
    /// the local placement that gives it. False for an unknown index.
    pub fn set_world_transform(&mut self, index: usize, world: transform::Transform) -> bool {
        if index >= self.objects.len() {
            return false;
        }
        // A locked object cannot be moved.
        if self.objects[index].locked {
            return false;
        }
                // A locked object cannot be moved.
        if self.objects[index].locked {
            return false;
        }
        let (Some(parent_index), Some(link)) = (self.parent_of(index), self.objects[index].parent)
        else {
            return self.set_transform(index, world);
        };
        let parent_world = self.world_matrix(parent_index);
let bone = match link.bone {
            Some(bone) => self.bone_matrix(parent_index, bone),
            None => glam::Mat4::IDENTITY,
        };
        let local = (parent_world * bone * link.inverse).inverse() * world.to_matrix();
        self.objects[index].transform = transform::Transform::from_matrix(local);
        true
    }

    /// Hangs the object at `child` below the object at `parent`, or frees
    /// it (`parent` None). Either way the object stays where it is in the
    /// scene. False, changing nothing, for an unknown index, an object
    /// below itself, or a parent that is itself below `child`.
pub fn set_parent(&mut self, child: usize, parent: Option<usize>) -> bool {
        self.link(child, parent, None)
    }

    /// Makes the object at `child` follow one bone of the object at
    /// `parent` through its pose, staying where it is. False, changing
    /// nothing, for an unknown object, a parent with no such bone, or a loop.
    pub fn set_parent_to_bone(&mut self, child: usize, parent: usize, bone: usize) -> bool {
        match self.objects.get(parent) {
            Some(object) if bone < object.skeleton.bones.len() => {
                self.link(child, Some(parent), Some(bone))
            }
            _ => false,
        }
    }

    fn link(&mut self, child: usize, parent: Option<usize>, bone: Option<usize>) -> bool {
    if child >= self.objects.len() {
            return false;
        }
        let world = self.world_matrix(child);
        match parent {
            None => {
                if self.objects[child].parent.is_some() {
                    self.objects[child].parent = None;
                    self.objects[child].transform = transform::Transform::from_matrix(world);
                }
                true
            }
            Some(parent) => {
                if parent >= self.objects.len() || parent == child {
                    return false;
                }
                // Walking up from the new parent must never meet the child.
                let mut above = Some(parent);
                for _ in 0..=self.objects.len() {
                    match above {
                        Some(i) if i == child => return false,
                        Some(i) => above = self.parent_of(i),
                        None => break,
                    }
                }
                let parent_id = self.id_of(parent);
                let local = self.objects[child].transform.to_matrix();
                let bone_matrix = match bone {
                    Some(bone) => self.bone_matrix(parent, bone),
                    None => glam::Mat4::IDENTITY,
                };
                let inverse =
                    (self.world_matrix(parent) * bone_matrix).inverse() * world * local.inverse();
                    self.objects[child].parent = Some(Parenting {
                    parent_id,
                    bone,
                    inverse,
                });
                    true
            }
        }
    }

    /// Locks or unlocks the object at `index`. False, changing nothing, for
    /// an unknown index.
    pub fn set_locked(&mut self, index: usize, locked: bool) -> bool {
        match self.objects.get_mut(index) {
            Some(object) => {
                object.locked = locked;
                true
            }
            None => false,
        }
    }

    /// Locks every object except the active one, which is unlocked: the
    /// "work on this one only" switch.
    pub fn lock_all_except_active(&mut self) {
        let keep = self.active_object;
        for (index, object) in self.objects.iter_mut().enumerate() {
            object.locked = index != keep;
        }
    }

    /// Unlocks every object.
    pub fn unlock_all(&mut self) {
        for object in self.objects.iter_mut() {
            object.locked = false;
        }
    }

    /// For each object, in list order, whether it is locked.
    pub fn locks(&self) -> Vec<bool> {
        self.objects.iter().map(|object| object.locked).collect()
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

/// Locks or unlocks the object at `index` (see `Scene::set_locked`).
pub fn set_object_locked(index: usize, locked: bool) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => scene.set_locked(index, locked),
        None => false,
    }
}

/// Locks every object of the active scene except the active one.
pub fn lock_all_except_active() {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    if let Some(scene) = guard.scenes.get_mut(scene_index) {
        scene.lock_all_except_active();
    }
}

/// Unlocks every object of the active scene.
pub fn unlock_all() {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    if let Some(scene) = guard.scenes.get_mut(scene_index) {
        scene.unlock_all();
    }
}

/// For each object of the active scene, in list order, whether it is locked.
pub fn object_locks() -> Vec<bool> {
    let guard = PROJECT.lock().unwrap();
    guard
        .scenes
        .get(guard.active_scene)
        .map_or_else(Vec::new, |scene| scene.locks())
}

/// Whether the active object is locked.
pub fn active_locked() -> bool {
    let guard = PROJECT.lock().unwrap();
    guard.scenes.get(guard.active_scene).map_or(false, |scene| {
        scene
            .objects
            .get(scene.active_object)
            .map_or(false, |object| object.locked)
    })
}

/// The active object's index and transform.
pub fn active_transform() -> (usize, transform::Transform) {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => {
            let transform = scene
                .objects
                .get(scene.active_object)
                .map_or_else(transform::Transform::default, |_| {
                    scene.world_transform(scene.active_object)
                });
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
    if scene.set_world_transform(index, transform) {
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
    ObjectKind::Camera(lens) => {
            Some((scene.world_transform(scene.first_camera()?), lens))
        }
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

/// Every object's world placement, in list order (parents included).
pub fn world_matrices() -> Vec<glam::Mat4> {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => (0..scene.objects.len())
            .map(|index| scene.world_matrix(index))
            .collect(),
        None => Vec::new(),
    }
}

/// Hangs one object below another, or frees it (see `Scene::set_parent`).
pub fn set_parent(child: usize, parent: Option<usize>) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => scene.set_parent(child, parent),
        None => false,
    }
}

/// Makes one object follow a bone of another (see `Scene::set_parent_to_bone`).
pub fn set_parent_to_bone(child: usize, parent: usize, bone: usize) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => scene.set_parent_to_bone(child, parent, bone),
        None => false,
    }
}

/// For each object, in list order, the bone of its parent that it follows,
/// if any.
pub fn object_bone_parents() -> Vec<Option<usize>> {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => scene
            .objects
            .iter()
            .map(|object| object.parent.and_then(|link| link.bone))
            .collect(),
        None => Vec::new(),
    }
}

/// For each object, in list order, the index of its parent.
pub fn object_parents() -> Vec<Option<usize>> {
    let guard = PROJECT.lock().unwrap();
    match guard.scenes.get(guard.active_scene) {
        Some(scene) => (0..scene.objects.len())
            .map(|index| scene.parent_of(index))
            .collect(),
        None => Vec::new(),
    }
}

/// Evaluates the active scene's animation at `time` (see
/// `Scene::evaluate_at`).
pub fn evaluate_at(time: f32) -> Vec<ObjectFrame> {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    match guard.scenes.get_mut(scene_index) {
        Some(scene) => scene.evaluate_at(time),
        None => Vec::new(),
    }
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

/// The active scene's world: its background and studio light.
pub fn world() -> world::World {
    let guard = PROJECT.lock().unwrap();
    guard
        .scenes
        .get(guard.active_scene)
        .map_or_else(world::World::default, |scene| scene.world)
}

/// Sets the active scene's world. The settings are kept as given; hold
/// them inside their ranges first (`World::clamped`).
pub fn set_world(settings: world::World) {
    let mut guard = PROJECT.lock().unwrap();
    let scene_index = guard.active_scene;
    if let Some(scene) = guard.scenes.get_mut(scene_index) {
        scene.world = settings;
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
    fn keying_a_mesh_and_applying_brings_back_its_placement_and_look() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.transform.translation = glam::Vec3::new(1.0, 2.0, 3.0);
        object.transform.scale = glam::Vec3::new(2.0, 2.0, 2.0);
        object.material.roughness = 0.9;
        object.material.base_color = [0.1, 0.2, 0.3];
        let keyed = (object.transform, object.material);
        object.key_channels(1.0);

        // Change everything, then ask for the keyed moment.
        object.transform = transform::Transform::default();
        object.material = material::Material::default();
        assert!(object.apply_channels(1.0));
        assert_eq!(object.transform, keyed.0);
        assert_eq!(object.material, keyed.1);
    }

    #[test]
    fn the_channels_blend_between_two_keyed_moments() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.material.roughness = 0.2;
        object.key_channels(0.0);
        object.transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        object.transform.rotation = glam::Quat::from_rotation_z(90f32.to_radians());
        object.material.roughness = 0.6;
        object.key_channels(2.0);

        assert!(object.apply_channels(1.0));
        assert!((object.transform.translation - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5);
        assert!((object.material.roughness - 0.4).abs() < 1e-5);
        let halfway = glam::Quat::from_rotation_z(45f32.to_radians());
        assert!(object.transform.rotation.dot(halfway).abs() > 0.99999);
    }

    #[test]
    fn cameras_and_lights_key_only_what_they_animate() {
        let mut scene = scene_with(2, 0);
        scene.objects[0].kind = ObjectKind::Camera(camera::Camera { fov_degrees: 50.0 });
        scene.objects[1].kind = ObjectKind::Light(light::Light::sun());
        for object in scene.objects.iter_mut() {
            object.key_channels(0.0);
        }

        let camera_object = &scene.objects[0];
        assert!(camera_object.channels.sample(Property::Position, 0.0).is_some());
        assert!(camera_object.channels.sample(Property::CameraFov, 0.0).is_some());
        assert!(camera_object.channels.sample(Property::Scale, 0.0).is_none());
        assert!(camera_object.channels.sample(Property::Colour, 0.0).is_none());

        let light_object = &scene.objects[1];
        assert!(light_object.channels.sample(Property::LightColour, 0.0).is_some());
        assert!(light_object.channels.sample(Property::LightIntensity, 0.0).is_some());
        assert!(light_object.channels.sample(Property::CameraFov, 0.0).is_none());
        assert!(light_object.channels.sample(Property::Scale, 0.0).is_none());

        // Change both, then apply: the lens and the light come back, and a
        // sun stays a sun.
        scene.objects[0].kind = ObjectKind::Camera(camera::Camera { fov_degrees: 90.0 });
        let mut changed = light::Light::sun();
        changed.intensity = 9.0;
        scene.objects[1].kind = ObjectKind::Light(changed);
        assert!(scene.objects[0].apply_channels(0.0));
        assert!(scene.objects[1].apply_channels(0.0));
        assert_eq!(
            scene.objects[0].kind,
            ObjectKind::Camera(camera::Camera { fov_degrees: 50.0 })
        );
        assert_eq!(scene.objects[1].kind, ObjectKind::Light(light::Light::sun()));
    }

    #[test]
    fn an_object_without_channels_is_left_alone() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.transform.translation = glam::Vec3::new(5.0, 0.0, 0.0);
        let before = object.transform;
        assert!(!object.apply_channels(1.0));
        assert_eq!(object.transform, before);
    }

    #[test]
    fn applied_values_are_held_inside_their_ranges() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.kind = ObjectKind::Camera(camera::Camera::default());
        assert!(object.channels.set_key(
            Property::CameraFov,
            0.0,
            Value::Scalar(500.0),
            Interpolation::Linear
        ));
        assert!(object.apply_channels(0.0));
        assert_eq!(
            object.kind,
            ObjectKind::Camera(camera::Camera {
                fov_degrees: camera::Camera::MAX_FOV
            })
        );
    }

    #[test]
    fn evaluating_a_moment_animates_every_object_not_just_the_active_one() {
        let mut scene = scene_with(3, 2);
        // Object 0: a one-bone character whose bone turns 90 degrees over
        // two seconds.
        scene.objects[0]
            .skeleton
            .add_bone("bone", None, glam::Vec3::ZERO);
        let turn = |degrees: f32| glam::Quat::from_rotation_z(degrees.to_radians());
        scene.objects[0].timeline.set_key(0.0, vec![turn(0.0)]);
        scene.objects[0].timeline.set_key(2.0, vec![turn(90.0)]);
        // Object 1: slides along X over two seconds.
        scene.objects[1].key_channels(0.0);
        scene.objects[1].transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        scene.objects[1].key_channels(2.0);
        // Object 2, the active one, has no animation at all.

        let frames = scene.evaluate_at(1.0);
        assert_eq!(frames.len(), 3);

        let skinning = frames[0].skinning.as_ref().expect("object 0 has pose keys");
        let turned = skinning[0].transform_point3(glam::Vec3::X);
        let half = 0.5f32.sqrt();
        assert!((turned - glam::Vec3::new(half, half, 0.0)).length() < 1e-4);
        assert!(!frames[0].placed);

        assert!(frames[1].placed);
        assert!(frames[1].skinning.is_none());
        let at = frames[1].model.transform_point3(glam::Vec3::ZERO);
        assert!((at - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);

        assert!(!frames[2].placed);
        assert!(frames[2].skinning.is_none());
    }

    #[test]
    fn key_times_and_deleting_keys_cover_the_pose_and_the_channels() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.timeline.set_key(1.0, vec![]);
        object.key_channels(1.0);
        object.key_channels(2.0);
        assert_eq!(object.key_times().len(), 2);

        assert!(object.remove_keys_near(1.01, 0.05));
        assert!(object.timeline.keys.is_empty());
        let times = object.key_times();
        assert_eq!(times.len(), 1);
        assert!((times[0] - 2.0).abs() < 1e-6);
        assert!(!object.remove_keys_near(1.5, 0.05));
    }

    #[test]
    fn a_repeat_covers_the_channels_too() {
        let mut scene = scene_with(1, 0);
        let object = &mut scene.objects[0];
        object.timeline.set_key(0.0, vec![]);
        object.timeline.set_key(1.0, vec![]);
        object.key_channels(0.0);
        object.transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        object.key_channels(1.0);
        assert!(object.timeline.set_cycle_end(2.0));

        let x_at = |object: &mut Object, time: f32| {
            object.apply_channels(time);
            object.transform.translation.x
        };
        assert!((x_at(object, 1.0) - 4.0).abs() < 1e-4);
        // Halfway back to the first key.
        assert!((x_at(object, 1.5) - 2.0).abs() < 1e-4);
        // The second cycle starts over.
        assert!(x_at(object, 2.0).abs() < 1e-4);
        assert!((x_at(object, 2.5) - 2.0).abs() < 1e-4);
        assert!((x_at(object, 3.0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn parenting_keeps_the_child_where_it_is_and_then_it_follows() {
        let mut scene = scene_with(2, 0);
        scene.objects[0].transform.translation = glam::Vec3::new(2.0, 0.0, 0.0);
        scene.objects[0].transform.rotation = glam::Quat::from_rotation_z(90f32.to_radians());
        scene.objects[1].transform.translation = glam::Vec3::new(1.0, 5.0, 0.0);
        let before = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);

        assert!(scene.set_parent(1, Some(0)));
        assert_eq!(scene.parent_of(1), Some(0));
        let after = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((after - before).length() < 1e-4);

        // Move the parent: the child goes with it.
        scene.objects[0].transform.translation = glam::Vec3::new(2.0, 3.0, 0.0);
        let moved = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((moved - (before + glam::Vec3::new(0.0, 3.0, 0.0))).length() < 1e-4);
    }

    #[test]
    fn turning_the_parent_swings_the_child_around_it() {
        let mut scene = scene_with(2, 0);
        scene.objects[1].transform.translation = glam::Vec3::new(1.0, 0.0, 0.0);
        assert!(scene.set_parent(1, Some(0)));
        scene.objects[0].transform.rotation = glam::Quat::from_rotation_z(90f32.to_radians());
        let at = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((at - glam::Vec3::new(0.0, 1.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn clearing_the_parent_keeps_the_world_placement() {
        let mut scene = scene_with(2, 0);
        scene.objects[1].transform.translation = glam::Vec3::new(1.0, 0.0, 0.0);
        assert!(scene.set_parent(1, Some(0)));
        scene.objects[0].transform.translation = glam::Vec3::new(0.0, 3.0, 0.0);
        let before = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);

        assert!(scene.set_parent(1, None));
        assert_eq!(scene.parent_of(1), None);
        let after = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((after - before).length() < 1e-4);
        assert!((scene.objects[1].transform.translation - before).length() < 1e-4);
    }

    #[test]
    fn a_parent_cannot_hang_below_its_own_child_and_bad_indices_are_refused() {
        let mut scene = scene_with(3, 0);
        assert!(scene.set_parent(1, Some(0)));
        assert!(scene.set_parent(2, Some(1)));
        // 0 <- 1 <- 2: putting 0 below 2 or below 1 would close a loop.
        assert!(!scene.set_parent(0, Some(2)));
        assert!(!scene.set_parent(0, Some(1)));
        assert!(!scene.set_parent(1, Some(1)));
        assert!(!scene.set_parent(9, Some(0)));
        assert!(!scene.set_parent(1, Some(9)));
        assert_eq!(scene.parent_of(0), None);
    }

    #[test]
    fn removing_a_parent_hands_its_children_to_its_own_parent_in_place() {
        // 0 grandparent, 1 parent, 2 child, 3 bystander.
        let mut scene = scene_with(4, 0);
        scene.objects[1].transform.translation = glam::Vec3::new(1.0, 0.0, 0.0);
        scene.objects[2].transform.translation = glam::Vec3::new(0.0, 2.0, 0.0);
        assert!(scene.set_parent(1, Some(0)));
        assert!(scene.set_parent(2, Some(1)));
        let before = scene.world_matrix(2).transform_point3(glam::Vec3::ZERO);

        assert!(scene.remove_object(1));
        // The child is now index 1 and hangs below the grandparent, in place.
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.parent_of(1), Some(0));
        let after = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((after - before).length() < 1e-4);
    }

    #[test]
    fn parent_links_survive_other_objects_being_removed() {
        // 0 the model, 1 a bystander, 2 the parent, 3 the child.
        let mut scene = scene_with(4, 0);
        scene.objects[2].transform.translation = glam::Vec3::new(5.0, 0.0, 0.0);
        assert!(scene.set_parent(3, Some(2)));
        assert!(scene.remove_object(1));
        // The parent is now index 1 and the child index 2, still linked.
        assert_eq!(scene.parent_of(2), Some(1));
        // The child stayed at the origin when parented; moving the parent
        // two units further moves it two units.
        scene.objects[1].transform.translation = glam::Vec3::new(7.0, 0.0, 0.0);
        let at = scene.world_matrix(2).transform_point3(glam::Vec3::ZERO);
        assert!((at - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn setting_a_childs_world_placement_works_out_its_local_one() {
        let mut scene = scene_with(2, 0);
        scene.objects[0].transform.translation = glam::Vec3::new(1.0, 0.0, 0.0);
        scene.objects[0].transform.rotation = glam::Quat::from_rotation_y(30f32.to_radians());
        assert!(scene.set_parent(1, Some(0)));

        let mut wanted = transform::Transform::default();
        wanted.translation = glam::Vec3::new(3.0, 2.0, -1.0);
        wanted.set_euler_degrees(glam::Vec3::new(10.0, 20.0, 30.0));
        assert!(scene.set_world_transform(1, wanted));
        let got = scene.world_transform(1);
        assert!((got.translation - wanted.translation).length() < 1e-3);
        let probe = glam::Vec3::new(0.3, 0.8, -0.5);
        assert!((got.rotation * probe - wanted.rotation * probe).length() < 1e-3);

        // An object with no parent takes the placement exactly as given.
        assert!(scene.set_world_transform(0, wanted));
        assert_eq!(scene.objects[0].transform, wanted);
    }

    #[test]
    fn a_child_follows_its_parent_through_the_animation() {
        let mut scene = scene_with(2, 0);
        assert!(scene.set_parent(1, Some(0)));
        scene.objects[0].key_channels(0.0);
        scene.objects[0].transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        scene.objects[0].key_channels(2.0);

        let frames = scene.evaluate_at(1.0);
        // The parent is halfway along; the child has no keys but goes with it.
        let child_at = frames[1].model.transform_point3(glam::Vec3::ZERO);
        assert!((child_at - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
        assert!(!frames[1].placed);
    }

    #[test]
    fn a_part_parented_to_a_bone_swings_with_that_bone() {
        let mut scene = scene_with_an_arm();
        scene.objects[1].transform.translation = glam::Vec3::new(2.0, 0.0, 0.0);
        assert!(scene.set_parent_to_bone(1, 0, 1));
        // At rest nothing moves.
        let rest = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((rest - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);

        // Turn the arm a quarter turn about its own joint, at (1, 0, 0): the
        // part, one unit further out, swings up to (1, 1, 0).
        scene.objects[0].pose.rotations = vec![
            glam::Quat::IDENTITY,
            glam::Quat::from_rotation_z(90f32.to_radians()),
        ];
        let swung = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((swung - glam::Vec3::new(1.0, 1.0, 0.0)).length() < 1e-4);
    }

    /// Object 0 has a two-bone arm (a root at the origin and an "arm" joint
    /// one unit along +X); object 1 is a bare part.
    fn scene_with_an_arm() -> Scene {
        let mut scene = scene_with(2, 0);
        let root = scene.objects[0]
            .skeleton
            .add_bone("root", None, glam::Vec3::ZERO);
        scene.objects[0]
            .skeleton
            .add_bone("arm", Some(root), glam::Vec3::new(1.0, 0.0, 0.0));
        scene
    }

    #[test]
    fn parenting_to_a_bone_while_posed_keeps_the_part_in_place() {
        let mut scene = scene_with_an_arm();
        let quarter = glam::Quat::from_rotation_z(90f32.to_radians());
        scene.objects[0].pose.rotations = vec![glam::Quat::IDENTITY, quarter];
        scene.objects[1].transform.translation = glam::Vec3::new(1.0, 1.0, 0.0);
        let before = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);

        assert!(scene.set_parent_to_bone(1, 0, 1));
        let after = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((after - before).length() < 1e-4);

        // Back to rest: the part goes back with the arm, to where it hung
        // when the arm was straight.
        scene.objects[0].pose.rotations = vec![glam::Quat::IDENTITY, glam::Quat::IDENTITY];
        let rest = scene.world_matrix(1).transform_point3(glam::Vec3::ZERO);
        assert!((rest - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn a_bone_parent_must_be_a_real_bone_of_a_real_object() {
        let mut scene = scene_with_an_arm();
        assert!(!scene.set_parent_to_bone(1, 0, 2)); // no such bone
        assert!(!scene.set_parent_to_bone(0, 1, 0)); // object 1 has no skeleton
        assert!(!scene.set_parent_to_bone(1, 5, 0)); // no such parent
        assert!(!scene.set_parent_to_bone(5, 0, 0)); // no such child
        assert_eq!(scene.parent_of(1), None);
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
    fn a_locked_object_cannot_be_deleted_until_it_is_unlocked() {
        let mut scene = scene_with(3, 0);
        assert!(scene.set_locked(2, true));
        assert!(!scene.remove_object(2));
        assert_eq!(scene.objects.len(), 3);
        assert!(scene.set_locked(2, false));
        assert!(scene.remove_object(2));
        assert_eq!(scene.objects.len(), 2);
        assert!(!scene.set_locked(9, true));
    }

    #[test]
    fn lock_all_except_active_locks_the_others_and_unlock_all_frees_them() {
        let mut scene = scene_with(4, 2);
        scene.lock_all_except_active();
        assert_eq!(scene.locks(), vec![true, true, false, true]);
        scene.unlock_all();
        assert_eq!(scene.locks(), vec![false, false, false, false]);
    }

    #[test]
    fn a_locked_object_cannot_be_moved_but_others_can() {
        let mut scene = scene_with(2, 0);
        let mut wanted = transform::Transform::default();
        wanted.translation = glam::Vec3::new(1.0, 2.0, 3.0);
        assert!(scene.set_locked(0, true));
        assert!(!scene.set_world_transform(0, wanted));
        assert_eq!(scene.objects[0].transform, transform::Transform::default());
        assert!(scene.set_world_transform(1, wanted));
        assert!(scene.set_locked(0, false));
        assert!(scene.set_world_transform(0, wanted));
        assert_eq!(scene.objects[0].transform, wanted);
    }

    #[test]
    fn a_locked_object_still_plays_its_animation() {
        let mut scene = scene_with(1, 0);
        scene.objects[0].key_channels(0.0);
        scene.objects[0].transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        scene.objects[0].key_channels(2.0);
        assert!(scene.set_locked(0, true));
        let frames = scene.evaluate_at(1.0);
        let at = frames[0].model.transform_point3(glam::Vec3::ZERO);
        assert!((at - glam::Vec3::new(2.0, 0.0, 0.0)).length() < 1e-4);
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


#[cfg(test)]
mod smooth_key_tests {
    use super::*;

    #[test]
    fn keys_made_from_an_object_ease_between_moments() {
        let mut scene = Scene::default();
        scene.objects.push(Object::default());
        let object = &mut scene.objects[0];
        object.key_channels(0.0);
        object.transform.translation = glam::Vec3::new(4.0, 0.0, 0.0);
        object.key_channels(2.0);

        // A straight line would be at x = 1 a quarter of the way through.
        // Easing out of the first key means it has moved less than that.
        assert!(object.apply_channels(0.5));
        assert!(object.transform.translation.x > 0.0);
        assert!(object.transform.translation.x < 1.0);
        // The middle is still the middle.
        assert!(object.apply_channels(1.0));
        assert!((object.transform.translation.x - 2.0).abs() < 1e-4);
    }
}