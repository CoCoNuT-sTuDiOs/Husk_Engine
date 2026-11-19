//! Modes: how the active object is being worked on.
//!
//! Ported from Blender (SPDX-FileCopyrightText: 2023 Blender Authors,
//! SPDX-License-Identifier: GPL-2.0-or-later), object_modes.cc and
//! DNA_object_enums.h:
//! - `ObjectMode` is `eObjectMode` (DNA_object_enums.h), with only the modes
//!   Husk has so far.
//! - `mode_compat_test` is `mode_compat_test` (object_modes.cc): Object mode is
//!   always allowed, and every other mode depends on the kind of object. A Husk
//!   mesh that carries a skeleton counts as both Blender's mesh and its
//!   armature, and Weight Paint also needs bones and geometry (Husk's own
//!   condition: with neither there is nothing to paint).
//! - `set` follows the checks at the top of `mode_set_ex` (object_modes.cc): with
//!   no active object only Object mode is possible, being in the mode already
//!   changes nothing, and a mode the object does not allow is refused. Going
//!   straight from one mode to another is allowed, as `mode_compat_set` leaves
//!   the old mode first.
//!
//! The mode is working state of the whole project, not of a scene file: it is
//! not saved and not part of undo. It belongs to the object that was active
//! when it was set (by id), so another object being active, or the object no
//! longer allowing the mode, ends it.

use super::{Object, ObjectKind, Project, PROJECT};

/// How the active object is being worked on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectMode {
    /// Placing, turning and scaling objects. Every object allows it.
    #[default]
    Object,
    /// Posing the bones of a rig, or of a mesh that carries a skeleton.
    Pose,
    /// Painting which bone moves which part of a mesh.
    WeightPaint,
}

impl ObjectMode {
    /// Every mode, in the order the mode menu lists them.
    pub const ALL: [ObjectMode; 3] = [
        ObjectMode::Object,
        ObjectMode::Pose,
        ObjectMode::WeightPaint,
    ];

    /// The name the bridge and the mode menu use.
    pub fn name(self) -> &'static str {
        match self {
            ObjectMode::Object => "object",
            ObjectMode::Pose => "pose",
            ObjectMode::WeightPaint => "weight_paint",
        }
    }

    /// The mode with this name, or None for any other name.
    pub fn from_name(name: &str) -> Option<ObjectMode> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// Whether `object` can be worked on in `mode`.
pub fn mode_compat_test(object: &Object, mode: ObjectMode) -> bool {
    if mode == ObjectMode::Object {
        return true;
    }
    let has_bones = !object.skeleton.bones.is_empty();
    match object.kind {
        ObjectKind::Mesh => match mode {
            ObjectMode::Pose => has_bones,
            ObjectMode::WeightPaint => has_bones && object.mesh.is_some(),
            ObjectMode::Object => true,
        },
        ObjectKind::Rig => mode == ObjectMode::Pose,
        ObjectKind::Camera(_) | ObjectKind::Light(_) => false,
    }
}

fn active_of(project: &Project) -> Option<&Object> {
    let scene = project.scenes.get(project.active_scene)?;
    scene.objects.get(scene.active_object)
}

/// The mode the active object is in: the stored mode if it was set on this
/// very object and the object still allows it, otherwise Object.
fn current(project: &Project) -> ObjectMode {
    match active_of(project) {
        Some(object)
            if project.mode != ObjectMode::Object
                && project.mode_object == object.id
                && mode_compat_test(object, project.mode) =>
        {
            project.mode
        }
        _ => ObjectMode::Object,
    }
}

/// The modes the active object allows, in menu order.
fn available(project: &Project) -> Vec<ObjectMode> {
    match active_of(project) {
        Some(object) => ObjectMode::ALL
            .into_iter()
            .filter(|mode| mode_compat_test(object, *mode))
            .collect(),
        None => vec![ObjectMode::Object],
    }
}

/// Puts the active object in `mode`. False, changing nothing, when the object
/// does not allow it.
fn set(project: &mut Project, mode: ObjectMode) -> bool {
    let Some(object) = active_of(project) else {
        return mode == ObjectMode::Object;
    };
    if current(project) == mode {
        return true;
    }
    if !mode_compat_test(object, mode) {
        return false;
    }
    let id = object.id;
    project.mode = mode;
    project.mode_object = id;
    true
}

/// The mode the active object is in.
pub fn active_mode() -> ObjectMode {
    let guard = PROJECT.lock().unwrap();
    current(&guard)
}

/// The modes the active object allows, in menu order.
pub fn available_modes() -> Vec<ObjectMode> {
    let guard = PROJECT.lock().unwrap();
    available(&guard)
}

/// Puts the active object in `mode` (see `set`).
pub fn set_mode(mode: ObjectMode) -> bool {
    let mut guard = PROJECT.lock().unwrap();
    set(&mut guard, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    fn plain() -> Object {
        Object::default()
    }

    fn rig() -> Object {
        Object {
            kind: ObjectKind::Rig,
            ..Object::default()
        }
    }

    /// A mesh with geometry and one bone.
    fn character() -> Object {
        let mut object = Object::default();
        object.skeleton.add_bone("root", None, glam::Vec3::ZERO);
        object.mesh = Some(std::sync::Arc::new(
            crate::geometry::mesh::MeshData::default(),
        ));
        object
    }

    fn camera() -> Object {
        Object {
            kind: ObjectKind::Camera(crate::scene::camera::Camera::default()),
            ..Object::default()
        }
    }

    fn lamp() -> Object {
        Object {
            kind: ObjectKind::Light(crate::scene::light::Light::point()),
            ..Object::default()
        }
    }

    fn project_with(objects: Vec<Object>, active: usize) -> Project {
        let mut scene = Scene::default();
        for object in objects {
            scene.push_object(object);
        }
        scene.active_object = active;
        Project {
            scenes: vec![scene],
            active_scene: 0,
            mode: ObjectMode::Object,
            mode_object: 0,
        }
    }

    #[test]
    fn object_mode_is_allowed_for_everything() {
        for object in [plain(), rig(), character(), camera(), lamp()] {
            assert!(mode_compat_test(&object, ObjectMode::Object));
        }
    }

    #[test]
    fn pose_needs_a_rig_or_a_skeleton() {
        assert!(!mode_compat_test(&plain(), ObjectMode::Pose));
        assert!(mode_compat_test(&rig(), ObjectMode::Pose));
        assert!(mode_compat_test(&character(), ObjectMode::Pose));
        assert!(!mode_compat_test(&camera(), ObjectMode::Pose));
        assert!(!mode_compat_test(&lamp(), ObjectMode::Pose));
    }

    #[test]
    fn weight_paint_needs_a_mesh_that_has_bones() {
        assert!(!mode_compat_test(&plain(), ObjectMode::WeightPaint));
        assert!(mode_compat_test(&character(), ObjectMode::WeightPaint));
        // Bones but no geometry: nothing to paint on.
        let mut no_mesh = character();
        no_mesh.mesh = None;
        assert!(!mode_compat_test(&no_mesh, ObjectMode::WeightPaint));
        // Geometry but no bones: nothing to weigh against.
        let mut no_bones = character();
        no_bones.skeleton = Default::default();
        assert!(!mode_compat_test(&no_bones, ObjectMode::WeightPaint));
        assert!(!mode_compat_test(&rig(), ObjectMode::WeightPaint));
    }

    #[test]
    fn the_mode_menu_lists_only_what_the_active_object_allows() {
        let mut project = project_with(vec![plain(), rig(), character()], 0);
        assert_eq!(available(&project), vec![ObjectMode::Object]);
        project.scenes[0].active_object = 1;
        assert_eq!(
            available(&project),
            vec![ObjectMode::Object, ObjectMode::Pose]
        );
        project.scenes[0].active_object = 2;
        assert_eq!(
            available(&project),
            vec![ObjectMode::Object, ObjectMode::Pose, ObjectMode::WeightPaint]
        );
    }

    #[test]
    fn a_mode_the_object_does_not_allow_is_refused_and_changes_nothing() {
        let mut project = project_with(vec![plain(), character()], 0);
        assert!(!set(&mut project, ObjectMode::Pose));
        assert!(!set(&mut project, ObjectMode::WeightPaint));
        assert_eq!(current(&project), ObjectMode::Object);
        assert_eq!(project.mode, ObjectMode::Object);
    }

    #[test]
    fn a_mode_belongs_to_the_object_it_was_set_on() {
        let mut project = project_with(vec![plain(), rig(), character()], 1);
        assert!(set(&mut project, ObjectMode::Pose));
        assert_eq!(current(&project), ObjectMode::Pose);
        // Another object, which would allow Pose too, is not in the mode.
        project.scenes[0].active_object = 2;
        assert_eq!(current(&project), ObjectMode::Object);
    }

    #[test]
    fn a_mode_ends_when_the_object_stops_allowing_it() {
        let mut project = project_with(vec![plain(), character()], 1);
        assert!(set(&mut project, ObjectMode::WeightPaint));
        assert_eq!(current(&project), ObjectMode::WeightPaint);
        project.scenes[0].objects[1].skeleton = Default::default();
        assert_eq!(current(&project), ObjectMode::Object);
    }

    #[test]
    fn with_no_active_object_only_object_mode_is_possible() {
        let mut project = Project {
            scenes: vec![Scene::default()],
            active_scene: 0,
            mode: ObjectMode::Object,
            mode_object: 0,
        };
        assert!(set(&mut project, ObjectMode::Object));
        assert!(!set(&mut project, ObjectMode::Pose));
        assert_eq!(available(&project), vec![ObjectMode::Object]);
        assert_eq!(current(&project), ObjectMode::Object);
    }

    #[test]
    fn modes_switch_directly_and_setting_the_current_mode_changes_nothing() {
        let mut project = project_with(vec![character()], 0);
        assert!(set(&mut project, ObjectMode::Pose));
        assert!(set(&mut project, ObjectMode::Pose));
        assert_eq!(current(&project), ObjectMode::Pose);
        // Straight from one mode to another, as Blender's mode menu does.
        assert!(set(&mut project, ObjectMode::WeightPaint));
        assert_eq!(current(&project), ObjectMode::WeightPaint);
        assert!(set(&mut project, ObjectMode::Object));
        assert_eq!(current(&project), ObjectMode::Object);
    }

    #[test]
    fn mode_names_round_trip() {
        for mode in ObjectMode::ALL {
            assert_eq!(ObjectMode::from_name(mode.name()), Some(mode));
        }
        assert_eq!(ObjectMode::from_name("sculpt"), None);
        assert_eq!(ObjectMode::from_name(""), None);
    }
}