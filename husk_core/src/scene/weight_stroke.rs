//! The weight paint stroke in progress: from the mouse going down on the mesh to
//! it coming up. Working state, like the mode: it is not saved in a project file.
//!
//! A stroke begins on the active object, which must be in Weight Paint mode, have
//! weights and have the bone to paint. Beginning saves one undo step for the whole
//! stroke. Every step of the stroke paints the active object's weights through
//! `rig::brush::Stroke` and hands them to `upload` (the renderer). A stroke belongs
//! to the object it began on: if another object becomes active, steps do nothing.

use super::{mode, Project, RecoveringMutex, PROJECT};
use crate::rig::adjacency::Adjacency;
use crate::rig::brush::Stroke;
use crate::rig::paint::BlendMode;
use crate::rig::skinning::VertexWeights;

/// A stroke that has begun: the object it paints and the brush stroke itself.
struct Active {
    object_id: u64,
        stroke: Stroke,
    /// The neighbour lists of the mesh, for a Smooth stroke (None for a paint stroke).
    smooth: Option<Adjacency>,
}

static STROKE: RecoveringMutex<Option<Active>> = RecoveringMutex::new(None);

fn begin_in(
    project: &mut Project,
    bone: usize,
    blend: BlendMode,
    paint_weight: f32,
    strength: f32,
    accumulate: bool,
) -> Result<Active, String> {
    if mode::current(project) != mode::ObjectMode::WeightPaint {
        return Err(String::from("Switch to Weight Paint mode first"));
    }
    let scene_index = project.active_scene;
    let Some(scene) = project.scenes.get_mut(scene_index) else {
        return Err(String::from("There is no scene"));
    };
    let active = scene.active_object;
    let Some(object) = scene.objects.get(active) else {
        return Err(String::from("There is no active object"));
    };
    if bone >= object.skeleton.bones.len() {
        return Err(String::from("Select a bone first (click a joint)"));
    }
    let Some(weights) = object.weights.as_ref() else {
        return Err(String::from(
            "Compute the weights first (Rig > Compute weights)",
        ));
    };
    let vertex_count = match &object.mesh {
        Some(mesh) if mesh.positions.len() == weights.len() => weights.len(),
        _ => {
            return Err(String::from(
                "The weights do not fit the mesh; compute them again",
            ))
        }
    };
    let object_id = object.id;
    scene.push_undo(None);
    Ok(Active {
        object_id,
        smooth: None,
        stroke: Stroke::new(
            vertex_count,
            bone as u32,
            blend,
            paint_weight,
            strength,
            accumulate,
        ),
    })
}

fn step_in(
    project: &mut Project,
    active: &mut Active,
    centre: [f32; 3],
    radius: f32,
    upload: impl FnOnce(&[VertexWeights]),
) -> Option<usize> {
    let scene_index = project.active_scene;
    let scene = project.scenes.get_mut(scene_index)?;
    let index = scene.active_object;
    let object = scene.objects.get_mut(index)?;
    if object.id != active.object_id {
        return None;
    }
    let mesh = object.mesh.clone()?;
    let weights = object.weights.as_mut()?;
        let painted = match &active.smooth {
        Some(adjacency) => {
            active
                .stroke
                .smooth(weights, &mesh.positions, adjacency, centre, radius)
        }
        None => active
            .stroke
            .apply(weights, &mesh.positions, centre, radius),
    };
    if painted > 0 {
        upload(weights);
        scene.revision += 1;
    }
    Some(painted)
}

/// The neighbour lists of the active object's mesh, for a Smooth stroke.
fn adjacency_of_active(project: &Project) -> Option<Adjacency> {
    let scene = project.scenes.get(project.active_scene)?;
    let mesh = scene.objects.get(scene.active_object)?.mesh.as_ref()?;
    Some(Adjacency::build(&mesh.positions, &mesh.indices))
}

/// Starts a stroke on the active object that paints `bone` with `paint_weight`
/// (0 to 1) at `strength` (0 to 1). Saves one undo step for the whole stroke.
/// With `smooth` on, the stroke smooths the weights toward their neighbours
/// instead of painting (`paint_weight` and `blend` are then not used).
/// Returns why not when the stroke cannot start.
pub fn begin(
    bone: usize,
    blend: BlendMode,
    paint_weight: f32,
    strength: f32,
    accumulate: bool,
        smooth: bool,
    ) -> Result<(), String> {
    let started = {
        let mut guard = PROJECT.lock().unwrap();
                let mut started = begin_in(&mut guard, bone, blend, paint_weight, strength, accumulate)?;
        if smooth {
            started.smooth = Some(
                adjacency_of_active(&guard)
                    .ok_or_else(|| String::from("The mesh has no faces to smooth over"))?,
            );
        }
        started
    };
    *STROKE.lock().unwrap() = Some(started);
    Ok(())
}

/// One touch of the brush: its centre in the active object's own space and its
/// radius in the same units. `upload` is given the weights when any vertex was
/// painted. Returns how many vertices were painted, or None when no stroke is in
/// progress on the active object.
pub fn step(
    centre: [f32; 3],
    radius: f32,
    upload: impl FnOnce(&[VertexWeights]),
) -> Option<usize> {
    let mut stroke = STROKE.lock().unwrap();
    let active = stroke.as_mut()?;
    let mut guard = PROJECT.lock().unwrap();
    step_in(&mut guard, active, centre, radius, upload)
}

/// Ends the stroke.
pub fn end() {
    *STROKE.lock().unwrap() = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::mesh::MeshData;
    use crate::rig::paint::weight_of;
    use crate::scene::mode::ObjectMode;
    use crate::scene::{Object, Scene};

    fn on_bone_one() -> VertexWeights {
        VertexWeights {
            joint_indices: [1, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Three bones and three vertices in a row, all weighted to bone 1.
    fn character() -> Object {
        let mut object = Object::default();
        object.skeleton.add_bone("a", None, glam::Vec3::ZERO);
        object.skeleton.add_bone("b", None, glam::Vec3::ZERO);
        object.skeleton.add_bone("c", None, glam::Vec3::ZERO);
        let mut mesh = MeshData::default();
        mesh.positions = vec![[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [3.0, 0.0, 0.0]];
        object.mesh = Some(std::sync::Arc::new(mesh));
        object.weights = Some(vec![on_bone_one(); 3]);
        object
    }

    fn project_in_weight_paint(objects: Vec<Object>, active: usize) -> Project {
        let mut scene = Scene::default();
        for object in objects {
            scene.push_object(object);
        }
        scene.active_object = active;
        let id = scene.objects[active].id;
        Project {
            scenes: vec![scene],
            active_scene: 0,
            mode: ObjectMode::WeightPaint,
            mode_object: id,
        }
    }

    #[test]
    fn a_stroke_needs_weight_paint_mode_a_real_bone_and_weights_that_fit_the_mesh() {
        let mut project = project_in_weight_paint(vec![character()], 0);
        project.mode = ObjectMode::Object;
        assert!(begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false).is_err());
        project.mode = ObjectMode::WeightPaint;
        assert!(begin_in(&mut project, 9, BlendMode::Mix, 1.0, 1.0, false).is_err());
        project.scenes[0].objects[0].weights = None;
        assert!(begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false).is_err());
        project.scenes[0].objects[0].weights = Some(vec![on_bone_one(); 2]);
        assert!(begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false).is_err());
        // None of the refusals left an undo step behind.
        assert!(project.scenes[0].undo_stack.is_empty());
    }

    #[test]
    fn beginning_saves_one_undo_step_and_a_touch_paints_and_uploads_the_weights() {
        let mut project = project_in_weight_paint(vec![character()], 0);
        let mut active = begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false)
            .ok()
            .expect("the stroke begins");
        assert_eq!(project.scenes[0].undo_stack.len(), 1);
        let revision = project.scenes[0].revision;

        let mut uploaded = 0;
        let painted = step_in(&mut project, &mut active, [0.0; 3], 1.0, |_| uploaded += 1);
        assert_eq!(painted, Some(2));
        assert_eq!(uploaded, 1);
        assert!(project.scenes[0].revision > revision);

        let weights = project.scenes[0].objects[0].weights.as_ref().unwrap();
        assert!((weight_of(&weights[0], 2) - 1.0).abs() < 1e-5);
        assert!((weight_of(&weights[1], 2) - 0.5).abs() < 1e-5);
        assert_eq!(weights[2].weights, on_bone_one().weights);
        // Still one undo step: the touch did not save another.
        assert_eq!(project.scenes[0].undo_stack.len(), 1);
    }

    #[test]
    fn a_touch_out_of_reach_paints_and_uploads_nothing() {
        let mut project = project_in_weight_paint(vec![character()], 0);
        let mut active = begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false)
            .ok()
            .expect("the stroke begins");
        let mut uploaded = 0;
        let painted = step_in(
            &mut project,
            &mut active,
            [10.0, 0.0, 0.0],
            1.0,
            |_| uploaded += 1,
        );
        assert_eq!(painted, Some(0));
        assert_eq!(uploaded, 0);
    }

    #[test]
    fn a_stroke_belongs_to_the_object_it_began_on() {
        let mut project = project_in_weight_paint(vec![character(), character()], 0);
        let mut active = begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false)
            .ok()
            .expect("the stroke begins");
        project.scenes[0].active_object = 1;
        let mut uploaded = 0;
        let painted = step_in(&mut project, &mut active, [0.0; 3], 1.0, |_| uploaded += 1);
        assert_eq!(painted, None);
        assert_eq!(uploaded, 0);
        for object in &project.scenes[0].objects {
            let weights = object.weights.as_ref().unwrap();
            assert_eq!(weights[0].weights, on_bone_one().weights);
        }
    }

    #[test]
    fn undo_brings_back_the_weights_from_before_the_stroke() {
        let mut project = project_in_weight_paint(vec![character()], 0);
        let mut active = begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false)
            .ok()
            .expect("the stroke begins");
        step_in(&mut project, &mut active, [0.0; 3], 1.0, |_| {});
        assert!(project.scenes[0].undo());
        let weights = project.scenes[0].objects[0].weights.as_ref().unwrap();
        for vertex in weights {
            assert_eq!(vertex.joint_indices, on_bone_one().joint_indices);
            assert_eq!(vertex.weights, on_bone_one().weights);
        }
    }

    #[test]
    fn a_smooth_stroke_pulls_a_vertex_toward_the_average_of_the_faces_around_it() {
        // Four triangles around vertex 0, which has 0.9 of bone 2; the ring has none.
        let mixed = |bone_two: f32| VertexWeights {
            joint_indices: [2, 1, 0, 0],
            weights: [bone_two, 1.0 - bone_two, 0.0, 0.0],
        };
        let mut object = character();
        let mut mesh = MeshData::default();
        mesh.positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ];
        mesh.indices = vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1];
        object.mesh = Some(std::sync::Arc::new(mesh));
        object.weights = Some(vec![
            mixed(0.9),
            mixed(0.0),
            mixed(0.0),
            mixed(0.0),
            mixed(0.0),
        ]);
        let mut project = project_in_weight_paint(vec![object], 0);
        let mut active = begin_in(&mut project, 2, BlendMode::Mix, 1.0, 1.0, false)
            .ok()
            .expect("the stroke begins");
        active.smooth = adjacency_of_active(&project);
        assert!(active.smooth.is_some());

        let painted = step_in(&mut project, &mut active, [0.0; 3], 0.5, |_| {});
        assert_eq!(painted, Some(1));
        let weights = project.scenes[0].objects[0].weights.as_ref().unwrap();
        assert!((weight_of(&weights[0], 2) - 0.3).abs() < 1e-5);
        assert!(weight_of(&weights[1], 2).abs() < 1e-5);
    }

    #[test]
    fn blend_mode_names_round_trip() {
        for mode in BlendMode::ALL {
            assert_eq!(BlendMode::from_name(mode.name()), Some(mode));
        }
        assert_eq!(BlendMode::from_name("multiply"), None);
    }
}