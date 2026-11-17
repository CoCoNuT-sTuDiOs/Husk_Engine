//! The Husk project file, version 2: the whole scene in one file.
//!
//! Layout: the four bytes `HUSK`, the format version (u32), the length of the
//! description (u64), the description (JSON), then the heavy data (meshes and
//! skin weights) as raw little-endian numbers. The description says where each
//! piece of heavy data starts, counted from the start of that last part.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::animation::channels::{Channels, Interpolation, Property, Value};
use crate::animation::keyframes::Timeline;
use crate::animation::pose::Pose;
use crate::geometry::mesh::MeshData;
use crate::rig::geodesic::STATIC_INFLUENCE_INDEX;
use crate::rig::skinning::VertexWeights;
use crate::rig::{Bone, Skeleton};
use crate::scene::{
    Object, ObjectKind, Parenting, Scene, camera, light, material, output, transform, world,
};

const MAGIC: &[u8; 4] = b"HUSK";

/// Bump this when the file layout changes in a way older readers cannot
/// follow, so such files are refused with a clear message.
const FORMAT_VERSION: u32 = 2;

/// Where a mesh sits in the heavy data: its points (3 numbers each), then its
/// normals (3 each), then its triangle indices (1 each).
#[derive(Serialize, Deserialize, Clone, Copy)]
struct MeshRef {
    offset: u64,
    vertices: u64,
    indices: u64,
}

/// Where a set of skin weights sits in the heavy data: per vertex, four joint
/// indices then four weights.
#[derive(Serialize, Deserialize, Clone, Copy)]
struct WeightsRef {
    offset: u64,
    vertices: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum KindData {
    Mesh,
    Rig,
    Camera { fov_degrees: f32 },
    Light { sun: bool, color: [f32; 3], intensity: f32 },
}

#[derive(Serialize, Deserialize)]
struct MaterialData {
    base_color: [f32; 3],
    roughness: f32,
    metalness: f32,
}

#[derive(Serialize, Deserialize)]
struct ParentData {
    parent_id: u64,
    inverse: [f32; 16],
    bone: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct BoneData {
    name: String,
    parent: Option<usize>,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct KeyData {
    time: f32,
    rotations: Vec<[f32; 4]>,
    offset: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct ChannelKeyData {
    time: f32,
    value: Vec<f32>,
    interpolation: String,
}

#[derive(Serialize, Deserialize)]
struct ChannelData {
    property: String,
    keys: Vec<ChannelKeyData>,
}

#[derive(Serialize, Deserialize)]
struct ObjectData {
    id: u64,
    name: String,
    kind: KindData,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    material: MaterialData,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    parent: Option<ParentData>,
    #[serde(default)]
    bones: Vec<BoneData>,
    #[serde(default)]
    pose_rotations: Vec<[f32; 4]>,
    #[serde(default)]
    pose_offset: [f32; 3],
    #[serde(default)]
    keys: Vec<KeyData>,
    #[serde(default)]
    cycle_period: Option<f32>,
    #[serde(default)]
    travel_speed: f32,
    #[serde(default)]
    travel_velocity: [f32; 3],
    #[serde(default)]
    channels: Vec<ChannelData>,
    #[serde(default)]
    mesh: Option<MeshRef>,
    #[serde(default)]
    weights: Option<WeightsRef>,
}

#[derive(Serialize, Deserialize)]
struct WorldData {
    background: [f32; 3],
    ambient: f32,
    reflections: f32,
    background_top: [f32; 3],
    gradient: bool,
}

#[derive(Serialize, Deserialize)]
struct OutputData {
    width: u32,
    height: u32,
    fps: u32,
}

#[derive(Serialize, Deserialize)]
struct SceneData {
    active_object: usize,
    next_id: u64,
    objects: Vec<ObjectData>,
    /// Missing in a file means the defaults, so settings can be added later.
    #[serde(default)]
    world: Option<WorldData>,
    #[serde(default)]
    output: Option<OutputData>,
}

fn property_name(property: Property) -> &'static str {
    match property {
        Property::Position => "position",
        Property::Rotation => "rotation",
        Property::Scale => "scale",
        Property::Colour => "colour",
        Property::Roughness => "roughness",
        Property::Metalness => "metalness",
        Property::CameraFov => "camera_fov",
        Property::LightColour => "light_colour",
        Property::LightIntensity => "light_intensity",
    }
}

fn property_from_name(name: &str) -> Option<Property> {
    Some(match name {
        "position" => Property::Position,
        "rotation" => Property::Rotation,
        "scale" => Property::Scale,
        "colour" => Property::Colour,
        "roughness" => Property::Roughness,
        "metalness" => Property::Metalness,
        "camera_fov" => Property::CameraFov,
        "light_colour" => Property::LightColour,
        "light_intensity" => Property::LightIntensity,
        _ => return None,
    })
}

fn interpolation_name(interpolation: Interpolation) -> &'static str {
    match interpolation {
        Interpolation::Step => "step",
        Interpolation::Linear => "linear",
        Interpolation::Smooth => "smooth",
    }
}

fn interpolation_from_name(name: &str) -> Option<Interpolation> {
    Some(match name {
        "step" => Interpolation::Step,
        "linear" => Interpolation::Linear,
        "smooth" => Interpolation::Smooth,
        _ => return None,
    })
}

fn value_to_numbers(value: &Value) -> Vec<f32> {
    match value {
        Value::Scalar(v) => vec![*v],
        Value::Vector(v) => v.to_array().to_vec(),
        Value::Rotation(q) => q.to_array().to_vec(),
    }
}

/// The value a property takes, from saved numbers: four for a rotation, three
/// for a position, scale or colour, one for everything else.
fn numbers_to_value(property: Property, numbers: &[f32]) -> Option<Value> {
    if !numbers.iter().all(|n| n.is_finite()) {
        return None;
    }
    match (property, numbers.len()) {
        (Property::Rotation, 4) => Some(Value::Rotation(glam::Quat::from_slice(numbers))),
        (
            Property::Position | Property::Scale | Property::Colour | Property::LightColour,
            3,
        ) => Some(Value::Vector(glam::Vec3::from_slice(numbers))),
        (
            Property::Roughness
            | Property::Metalness
            | Property::CameraFov
            | Property::LightIntensity,
            1,
        ) => Some(Value::Scalar(numbers[0])),
        _ => None,
    }
}

fn finite3(values: [f32; 3]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// A normalized rotation, or None if the numbers are invalid.
fn unit_quat(values: [f32; 4]) -> Option<glam::Quat> {
    if !values.iter().all(|v| v.is_finite()) {
        return None;
    }
    let q = glam::Quat::from_array(values);
    if q.length_squared() < 1e-12 {
        None
    } else {
        Some(q.normalize())
    }
}

fn push_f32(blob: &mut Vec<u8>, value: f32) {
    blob.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(blob: &mut Vec<u8>, value: u32) {
    blob.extend_from_slice(&value.to_le_bytes());
}

fn write_mesh(blob: &mut Vec<u8>, mesh: &MeshData) -> Result<MeshRef, String> {
    if mesh.normals.len() != mesh.positions.len() {
        return Err("A mesh has a different number of normals and points".to_string());
    }
    let offset = blob.len() as u64;
    for point in &mesh.positions {
        for value in point {
            push_f32(blob, *value);
        }
    }
    for normal in &mesh.normals {
        for value in normal {
            push_f32(blob, *value);
        }
    }
    for index in &mesh.indices {
        push_u32(blob, *index);
    }
    Ok(MeshRef {
        offset,
        vertices: mesh.positions.len() as u64,
        indices: mesh.indices.len() as u64,
    })
}

fn write_weights(blob: &mut Vec<u8>, weights: &[VertexWeights]) -> WeightsRef {
    let offset = blob.len() as u64;
    for vertex in weights {
        for joint in vertex.joint_indices {
            push_u32(blob, joint);
        }
        for weight in vertex.weights {
            push_f32(blob, weight);
        }
    }
    WeightsRef {
        offset,
        vertices: weights.len() as u64,
    }
}

fn object_data(
    object: &Object,
    blob: &mut Vec<u8>,
    meshes: &mut HashMap<usize, MeshRef>,
) -> Result<ObjectData, String> {
    let kind = match object.kind {
        ObjectKind::Mesh => KindData::Mesh,
        ObjectKind::Rig => KindData::Rig,
        ObjectKind::Camera(lens) => KindData::Camera {
            fov_degrees: lens.fov_degrees,
        },
        ObjectKind::Light(lamp) => KindData::Light {
            sun: lamp.kind == light::LightKind::Sun,
            color: lamp.color,
            intensity: lamp.intensity,
        },
    };

    // Objects that share a mesh (a duplicate and its original) share one copy
    // of it in the file too.
    let mesh = match &object.mesh {
        Some(shared) => {
            let key = Arc::as_ptr(shared) as usize;
            let reference = match meshes.get(&key) {
                Some(reference) => *reference,
                None => {
                    let reference = write_mesh(blob, shared)?;
                    meshes.insert(key, reference);
                    reference
                }
            };
            Some(reference)
        }
        None => None,
    };

    // Weights are saved only when they match the mesh, so a stale set can
    // never make the file unreadable.
    let weights = match (&object.weights, &mesh) {
        (Some(list), Some(reference)) if list.len() as u64 == reference.vertices => {
            Some(write_weights(blob, list))
        }
        _ => None,
    };

    Ok(ObjectData {
        id: object.id,
        name: object.name.clone(),
        kind,
        translation: object.transform.translation.to_array(),
        rotation: object.transform.rotation.to_array(),
        scale: object.transform.scale.to_array(),
        material: MaterialData {
            base_color: object.material.base_color,
            roughness: object.material.roughness,
            metalness: object.material.metalness,
        },
        locked: object.locked,
        parent: object.parent.map(|link| ParentData {
            parent_id: link.parent_id,
            inverse: link.inverse.to_cols_array(),
            bone: link.bone,
        }),
        bones: object
            .skeleton
            .bones
            .iter()
            .map(|bone| BoneData {
                name: bone.name.clone(),
                parent: bone.parent,
                position: bone.local_position.to_array(),
                rotation: bone.local_rotation.to_array(),
                scale: bone.local_scale.to_array(),
            })
            .collect(),
        pose_rotations: object.pose.rotations.iter().map(|q| q.to_array()).collect(),
        pose_offset: object.pose.root_offset.to_array(),
        keys: object
            .timeline
            .keys
            .iter()
            .map(|key| KeyData {
                time: key.time,
                rotations: key.rotations.iter().map(|q| q.to_array()).collect(),
                offset: key.offset.to_array(),
            })
            .collect(),
        cycle_period: object.timeline.cycle_period,
        travel_speed: object.timeline.travel_speed,
        travel_velocity: object.timeline.travel_velocity.to_array(),
        channels: object
            .channels
            .channels
            .iter()
            .map(|channel| ChannelData {
                property: property_name(channel.property).to_string(),
                keys: channel
                    .keys
                    .iter()
                    .map(|key| ChannelKeyData {
                        time: key.time,
                        value: value_to_numbers(&key.value),
                        interpolation: interpolation_name(key.interpolation).to_string(),
                    })
                    .collect(),
            })
            .collect(),
        mesh,
        weights,
    })
}

/// Writes the whole scene to `path`. Returns a short summary, or the reason it
/// could not be written.
pub fn write_scene(path: &str, scene: &Scene) -> Result<String, String> {
    let mut blob: Vec<u8> = Vec::new();
    let mut meshes: HashMap<usize, MeshRef> = HashMap::new();
    let mut objects = Vec::with_capacity(scene.objects.len());
    for object in &scene.objects {
        objects.push(object_data(object, &mut blob, &mut meshes)?);
    }
    let data = SceneData {
        active_object: scene.active_object,
        next_id: scene.next_id,
        objects,
        world: Some(WorldData {
            background: scene.world.background,
            ambient: scene.world.ambient,
            reflections: scene.world.reflections,
            background_top: scene.world.background_top,
            gradient: scene.world.gradient,
        }),
        output: Some(OutputData {
            width: scene.output.width,
            height: scene.output.height,
            fps: scene.output.fps,
        }),
    };
    let json =
        serde_json::to_vec(&data).map_err(|e| format!("Could not encode the project: {e}"))?;

    let mut bytes = Vec::with_capacity(16 + json.len() + blob.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&blob);

    // Written beside the real file first and then moved into place, so a crash
    // while saving never destroys the previous save.
    let temporary = format!("{path}.saving");
    std::fs::write(&temporary, &bytes).map_err(|e| format!("Could not write {path}: {e}"))?;
    std::fs::rename(&temporary, path).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("Could not write {path}: {e}")
    })?;
    Ok(format!("Saved {} objects to {path}", scene.objects.len()))
}

fn damaged() -> String {
    "The project file is damaged: some of its data runs past the end of the file".to_string()
}

fn to_usize(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| damaged())
}

/// `length` bytes of the heavy data starting at `offset`, or an error when they
/// would run past its end.
fn blob_range(blob: &[u8], offset: u64, length: u64) -> Result<&[u8], String> {
    let start = to_usize(offset)?;
    let length = to_usize(length)?;
    start
        .checked_add(length)
        .and_then(|end| blob.get(start..end))
        .ok_or_else(damaged)
}

/// The number at position `unit` (counted in 4-byte units) of `bytes`.
fn f32_at(bytes: &[u8], unit: usize) -> f32 {
    f32::from_le_bytes(bytes[unit * 4..unit * 4 + 4].try_into().unwrap())
}

fn u32_at(bytes: &[u8], unit: usize) -> u32 {
    u32::from_le_bytes(bytes[unit * 4..unit * 4 + 4].try_into().unwrap())
}

fn read_mesh(blob: &[u8], reference: MeshRef, label: &str) -> Result<MeshData, String> {
    let vertices = to_usize(reference.vertices)?;
    let indices = to_usize(reference.indices)?;
    let length = vertices
        .checked_mul(24)
        .and_then(|points| indices.checked_mul(4).and_then(|tri| points.checked_add(tri)))
        .ok_or_else(damaged)?;
    let bytes = blob_range(blob, reference.offset, length as u64)?;

    let mut mesh = MeshData::default();
    mesh.positions.reserve(vertices);
    mesh.normals.reserve(vertices);
    mesh.indices.reserve(indices);
    for i in 0..vertices {
        mesh.positions.push([
            f32_at(bytes, i * 3),
            f32_at(bytes, i * 3 + 1),
            f32_at(bytes, i * 3 + 2),
        ]);
    }
    let normals_start = vertices * 3;
    for i in 0..vertices {
        mesh.normals.push([
            f32_at(bytes, normals_start + i * 3),
            f32_at(bytes, normals_start + i * 3 + 1),
            f32_at(bytes, normals_start + i * 3 + 2),
        ]);
    }
    let indices_start = vertices * 6;
    for i in 0..indices {
        mesh.indices.push(u32_at(bytes, indices_start + i));
    }

    if mesh.indices.len() % 3 != 0 {
        return Err(format!("{label} has a mesh that is not made of whole triangles"));
    }
    if mesh.indices.iter().any(|index| (*index as usize) >= vertices) {
        return Err(format!("{label} has a mesh with a triangle that points at a missing point"));
    }
    let finite = mesh
        .positions
        .iter()
        .chain(mesh.normals.iter())
        .all(|point| finite3(*point));
    if !finite {
        return Err(format!("{label} has a mesh with an invalid number"));
    }
    Ok(mesh)
}

fn read_weights(
    blob: &[u8],
    reference: WeightsRef,
    bone_count: usize,
    label: &str,
) -> Result<Vec<VertexWeights>, String> {
    let vertices = to_usize(reference.vertices)?;
    let length = vertices.checked_mul(32).ok_or_else(damaged)?;
    let bytes = blob_range(blob, reference.offset, length as u64)?;
    let mut weights = Vec::with_capacity(vertices);
    for vertex in 0..vertices {
        let base = vertex * 8;
        let mut joint_indices = [0u32; 4];
        let mut values = [0f32; 4];
        for slot in 0..4 {
            joint_indices[slot] = u32_at(bytes, base + slot);
            values[slot] = f32_at(bytes, base + 4 + slot);
        }
        for slot in 0..4 {
            if !values[slot].is_finite() || values[slot] < 0.0 {
                return Err(format!("{label}: vertex {vertex} has an invalid skin weight"));
            }
            if values[slot] > 0.0
                && (joint_indices[slot] as usize) >= bone_count
                && joint_indices[slot] != STATIC_INFLUENCE_INDEX
            {
                return Err(format!(
                    "{label}: vertex {vertex} is weighted to bone {}, but it only has {bone_count} bones",
                    joint_indices[slot]
                ));
            }
        }
        weights.push(VertexWeights {
            joint_indices,
            weights: values,
        });
    }
    Ok(weights)
}

fn object_from_data(
    index: usize,
    entry: &ObjectData,
    blob: &[u8],
    shared: &mut HashMap<(u64, u64, u64), Arc<MeshData>>,
) -> Result<Object, String> {
    let label = format!("Object {index} ('{}')", entry.name);

    // Placement and look.
    let rotation =
        unit_quat(entry.rotation).ok_or_else(|| format!("{label} has an invalid rotation"))?;
    if !finite3(entry.translation) || !finite3(entry.scale) {
        return Err(format!("{label} has an invalid placement"));
    }
    let placed = transform::Transform {
        translation: glam::Vec3::from_array(entry.translation),
        rotation,
        scale: transform::clamp_scale(glam::Vec3::from_array(entry.scale)),
    };
    let look = &entry.material;
    if !finite3(look.base_color) || !look.roughness.is_finite() || !look.metalness.is_finite() {
        return Err(format!("{label} has an invalid colour or finish"));
    }
    let surface = material::Material {
        base_color: look.base_color,
        roughness: look.roughness,
        metalness: look.metalness,
    }
    .clamped();

    let kind = match &entry.kind {
        KindData::Mesh => ObjectKind::Mesh,
        KindData::Rig => ObjectKind::Rig,
        KindData::Camera { fov_degrees } => {
            if !fov_degrees.is_finite() {
                return Err(format!("{label} has an invalid lens"));
            }
            ObjectKind::Camera(
                camera::Camera {
                    fov_degrees: *fov_degrees,
                }
                .clamped(),
            )
        }
        KindData::Light {
            sun,
            color,
            intensity,
        } => {
            if !finite3(*color) || !intensity.is_finite() {
                return Err(format!("{label} has an invalid light"));
            }
            ObjectKind::Light(
                light::Light {
                    kind: if *sun {
                        light::LightKind::Sun
                    } else {
                        light::LightKind::Point
                    },
                    color: *color,
                    intensity: *intensity,
                }
                .clamped(),
            )
        }
    };

    // Skeleton: parents must come before their children.
    let bone_count = entry.bones.len();
    let mut skeleton = Skeleton::default();
    for (i, bone) in entry.bones.iter().enumerate() {
        if let Some(parent) = bone.parent {
            if parent >= i {
                return Err(format!(
                    "{label}: bone {i} ('{}') is listed before its parent {parent}",
                    bone.name
                ));
            }
        }
        let Some(bone_rotation) = unit_quat(bone.rotation) else {
            return Err(format!("{label}: bone {i} ('{}') has an invalid rotation", bone.name));
        };
        if !finite3(bone.position) || !finite3(bone.scale) {
            return Err(format!("{label}: bone {i} ('{}') has an invalid number", bone.name));
        }
        skeleton.bones.push(Bone {
            name: bone.name.clone(),
            parent: bone.parent,
            local_position: glam::Vec3::from_array(bone.position),
            local_rotation: bone_rotation,
            local_scale: glam::Vec3::from_array(bone.scale),
        });
    }

    // Mesh and skin weights.
    let mesh = match entry.mesh {
        Some(reference) => {
            let key = (reference.offset, reference.vertices, reference.indices);
            let found = match shared.get(&key) {
                Some(found) => found.clone(),
                None => {
                    let read = Arc::new(read_mesh(blob, reference, &label)?);
                    shared.insert(key, read.clone());
                    read
                }
            };
            Some(found)
        }
        None => None,
    };
    let weights = match entry.weights {
        Some(reference) => {
            let vertices = mesh.as_ref().map_or(0, |m| m.positions.len()) as u64;
            if reference.vertices != vertices {
                return Err(format!(
                    "{label} has skin weights for a different number of vertices than its mesh"
                ));
            }
            Some(read_weights(blob, reference, bone_count, &label)?)
        }
        None => None,
    };

    // The pose it was left in.
    let mut pose = Pose::rest_for(&skeleton);
    if entry.pose_rotations.len() == bone_count {
        let mut rotations = Vec::with_capacity(bone_count);
        for values in &entry.pose_rotations {
            rotations.push(
                unit_quat(*values).ok_or_else(|| format!("{label} has an invalid pose"))?,
            );
        }
        pose.rotations = rotations;
    }
    if finite3(entry.pose_offset) {
        pose.root_offset = glam::Vec3::from_array(entry.pose_offset);
    }

    // The bone animation.
    if let Some(period) = entry.cycle_period {
        if !period.is_finite() || period <= 0.0 {
            return Err(format!("{label} has an invalid repeat length"));
        }
    }
    if !entry.travel_speed.is_finite() || !finite3(entry.travel_velocity) {
        return Err(format!("{label} has an invalid travel speed"));
    }
    let mut timeline = Timeline::default();
    for (i, key) in entry.keys.iter().enumerate() {
        if !key.time.is_finite() || !finite3(key.offset) {
            return Err(format!("{label}: key {i} has an invalid time or position"));
        }
        let mut rotations = Vec::with_capacity(key.rotations.len());
        for values in &key.rotations {
            rotations.push(
                unit_quat(*values).ok_or_else(|| format!("{label}: key {i} has an invalid rotation"))?,
            );
        }
        timeline.set_key_with_offset(key.time, rotations, glam::Vec3::from_array(key.offset));
    }
    timeline.cycle_period = entry.cycle_period;
    timeline.travel_speed = entry.travel_speed;
    timeline.travel_velocity = glam::Vec3::from_array(entry.travel_velocity);

    // The animation of everything else: placement, look, lens and light.
    let mut channels = Channels::default();
    for channel in &entry.channels {
        let property = property_from_name(&channel.property).ok_or_else(|| {
            format!(
                "{label} animates '{}', which this version of Husk does not know",
                channel.property
            )
        })?;
        for key in &channel.keys {
            let interpolation = interpolation_from_name(&key.interpolation)
                .ok_or_else(|| format!("{label} has an unknown kind of key"))?;
            let value = numbers_to_value(property, &key.value)
                .ok_or_else(|| format!("{label} has an invalid animation key"))?;
            if !channels.set_key(property, key.time, value, interpolation) {
                return Err(format!("{label} has an invalid animation key"));
            }
        }
    }

    let parent = match &entry.parent {
        Some(link) => {
            if link.inverse.iter().any(|v| !v.is_finite()) {
                return Err(format!("{label} has an invalid parent link"));
            }
            Some(Parenting {
                parent_id: link.parent_id,
                inverse: glam::Mat4::from_cols_array(&link.inverse),
                bone: link.bone,
            })
        }
        None => None,
    };

    Ok(Object {
        name: entry.name.clone(),
        transform: placed,
        material: surface,
        kind,
        skeleton,
        weights,
        pose,
        timeline,
        channels,
        id: entry.id,
        parent,
        locked: entry.locked,
        mesh,
        ..Object::default()
    })
}

fn scene_from_data(data: SceneData, blob: &[u8]) -> Result<Scene, String> {
    if data.objects.is_empty() {
        return Err("The project has no objects".to_string());
    }
    if data.active_object >= data.objects.len() {
        return Err("The project's active object does not exist".to_string());
    }

    let mut shared: HashMap<(u64, u64, u64), Arc<MeshData>> = HashMap::new();
    let mut objects = Vec::with_capacity(data.objects.len());
    for (index, entry) in data.objects.iter().enumerate() {
        objects.push(object_from_data(index, entry, blob, &mut shared)?);
    }

    // Every id is its own, and parent links point at real objects.
    let mut seen = HashSet::new();
    let mut highest = 0u64;
    for object in &objects {
        if object.id != 0 && !seen.insert(object.id) {
            return Err("Two objects in the project share one id".to_string());
        }
        highest = highest.max(object.id);
    }
    let parent_of = |objects: &[Object], link_id: u64| {
        objects
            .iter()
            .position(|other| other.id != 0 && other.id == link_id)
    };
    for (index, object) in objects.iter().enumerate() {
        let Some(link) = object.parent else {
            continue;
        };
        let Some(parent) = parent_of(&objects, link.parent_id) else {
            return Err(format!(
                "Object {index} ('{}') hangs below an object that is not in the project",
                object.name
            ));
        };
        if parent == index {
            return Err(format!("Object {index} ('{}') hangs below itself", object.name));
        }
        if let Some(bone) = link.bone {
            if bone >= objects[parent].skeleton.bones.len() {
                return Err(format!(
                    "Object {index} ('{}') follows a bone its parent does not have",
                    object.name
                ));
            }
        }
        // Walking up from the parent must never meet this object.
        let mut current = Some(parent);
        for _ in 0..=objects.len() {
            match current {
                Some(i) if i == index => {
                    return Err("The project's parent links form a loop".to_string());
                }
                Some(i) => {
                    current = objects[i]
                        .parent
                        .and_then(|up| parent_of(&objects, up.parent_id));
                }
                None => break,
            }
        }
    }

    let output = match &data.output {
        Some(saved) => output::Output {
            width: saved.width,
            height: saved.height,
            fps: saved.fps,
        }
        .clamped(),
        None => output::Output::default(),
    };
    let world = match &data.world {
        Some(saved) => {
            if !finite3(saved.background)
                || !finite3(saved.background_top)
                || !saved.ambient.is_finite()
                || !saved.reflections.is_finite()
            {
                return Err("The project's world settings are invalid".to_string());
            }
            world::World {
                background: saved.background,
                ambient: saved.ambient,
                reflections: saved.reflections,
                background_top: saved.background_top,
                gradient: saved.gradient,
            }
            .clamped()
        }
        None => world::World::default(),
    };

    Ok(Scene {
        objects,
        active_object: data.active_object,
        output,
        world,
        next_id: data.next_id.max(highest),
        ..Scene::default()
    })
}

/// Reads a project file, checking it carefully so a damaged or mismatched file
/// is refused with a reason instead of corrupting the scene.
pub fn read_scene(path: &str) -> Result<Scene, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {path}: {e}"))?;
    if bytes.first() == Some(&b'{') {
        return Err(
            "This project was saved by an older version of Husk (the first project format) and cannot be opened"
                .to_string(),
        );
    }
    if bytes.len() < 16 || &bytes[0..4] != MAGIC {
        return Err("This is not a Husk project file".to_string());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != FORMAT_VERSION {
        return Err(format!(
            "This project uses file format version {version}, but this build reads version {FORMAT_VERSION}"
        ));
    }
    let json_length = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let json_end = usize::try_from(json_length)
        .ok()
        .and_then(|length| length.checked_add(16))
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| {
            "The project file is damaged: its description runs past the end of the file".to_string()
        })?;
    let data: SceneData = serde_json::from_slice(&bytes[16..json_end])
        .map_err(|e| format!("This is not a Husk project file: {e}"))?;
    scene_from_data(data, &bytes[json_end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn temp_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("husk_scene_test_{}_{}.husk", std::process::id(), name))
            .to_string_lossy()
            .to_string()
    }

    fn triangle() -> MeshData {
        let mut mesh = MeshData::default();
        mesh.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        mesh.normals = vec![[0.0, 0.0, 1.0]; 3];
        mesh.indices = vec![0, 1, 2];
        mesh
    }

    /// Object 0 is the empty starting object; 1 is a rigged body; 2 is a copy
    /// of the body's mesh parented to it; 3 is a camera.
    fn sample_scene() -> Scene {
        let mut scene = Scene::default();
        scene.push_object(Object::default());

        let mesh = Arc::new(triangle());
        let mut body = Object::default();
        body.name = String::from("Body");
        body.mesh = Some(mesh.clone());
        body.transform.translation = Vec3::new(1.0, 2.0, 3.0);
        body.material.roughness = 0.9;
        body.skeleton.add_bone("root", None, Vec3::ZERO);
        body.skeleton.add_bone("arm", Some(0), Vec3::new(0.0, 1.0, 0.0));
        body.weights = Some(vec![
            VertexWeights {
                joint_indices: [0, 1, STATIC_INFLUENCE_INDEX, STATIC_INFLUENCE_INDEX],
                weights: [0.5, 0.5, 0.0, 0.0],
            };
            3
        ]);
        body.timeline
            .set_key_with_offset(0.0, vec![Quat::IDENTITY; 2], Vec3::ZERO);
        body.timeline.set_key_with_offset(
            1.0,
            vec![Quat::from_rotation_z(0.5), Quat::IDENTITY],
            Vec3::new(1.0, 0.0, 0.0),
        );
        body.key_channels(0.5);
        scene.push_object(body);

        let mut eye = Object::default();
        eye.name = String::from("Eye");
        eye.mesh = Some(mesh);
        scene.push_object(eye);
        assert!(scene.set_parent(2, Some(1)));

        let mut lens = Object::default();
        lens.kind = ObjectKind::Camera(camera::Camera { fov_degrees: 50.0 });
        scene.push_object(lens);

        scene.active_object = 1;
        scene.output = output::Output {
            width: 1280,
            height: 720,
            fps: 24,
        };
        scene
    }

    #[test]
    fn a_saved_scene_opens_the_same() {
        let path = temp_path("round_trip");
        let scene = sample_scene();
        write_scene(&path, &scene).unwrap();
        let loaded = read_scene(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded.objects.len(), 4);
        assert_eq!(loaded.active_object, 1);
        assert_eq!(
            (loaded.output.width, loaded.output.height, loaded.output.fps),
            (1280, 720, 24)
        );

        let body = &loaded.objects[1];
        assert_eq!(body.name, "Body");
        assert_eq!(body.transform, scene.objects[1].transform);
        assert_eq!(body.material, scene.objects[1].material);
        assert_eq!(body.skeleton.bones.len(), 2);
        assert_eq!(body.skeleton.bones[1].parent, Some(0));
        let mesh = body.mesh.as_ref().expect("the body keeps its mesh");
        assert_eq!(mesh.positions, triangle().positions);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        assert_eq!(body.weights.as_ref().unwrap().len(), 3);
        assert_eq!(body.timeline.keys.len(), 2);
        assert!(!body.channels.is_empty());

        // The copy shares one mesh with the body and is still parented to it.
        let eye = &loaded.objects[2];
        assert!(Arc::ptr_eq(eye.mesh.as_ref().unwrap(), mesh));
        assert_eq!(loaded.parent_of(2), Some(1));

        assert_eq!(
            loaded.objects[3].kind,
            ObjectKind::Camera(camera::Camera { fov_degrees: 50.0 })
        );
        assert!(loaded.objects[0].mesh.is_none());
        // New objects never take an id that is already in use.
        let highest = loaded.objects.iter().map(|object| object.id).max().unwrap();
        assert!(loaded.next_id >= highest);
    }

    #[test]
    fn files_that_are_not_scenes_or_are_damaged_are_refused() {
        assert!(read_scene("no/such/file.husk").is_err());

        let path = temp_path("refused");
        std::fs::write(&path, "not a project").unwrap();
        assert!(read_scene(&path).is_err());

        // A version 1 project was a JSON file.
        std::fs::write(&path, "{\"version\":1}").unwrap();
        let reason = read_scene(&path)
            .err()
            .expect("an old project must be refused");
        assert!(reason.contains("older"), "{reason}");

        // A file cut short is refused instead of being read past its end.
        write_scene(&path, &sample_scene()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();
        assert!(read_scene(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}