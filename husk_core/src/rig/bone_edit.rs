//! Editing a skeleton that already exists: moving a joint and deleting a bone,
//! and keeping everything that counts bones by number right afterwards.
//!
//! Modelled on Blender's Delete Selected Bone(s) (`ARMATURE_OT_delete`,
//! `armature_delete_selected_exec` in armature_edit.cc: it removes the bones and
//! their pose channels and forgets the active bone). No Blender code is copied.
//! Husk's bones are joints that carry their skin weights by number, so Husk has
//! rules of its own:
//! - the children of a deleted bone move up to its parent and stay where they are
//!   in space;
//! - its skin weights go to its parent (a root has none: they go to the influence
//!   that stays put, so nothing collapses to the origin);
//! - things that followed it follow its parent.
//! The child rule matches Blender's `ED_armature_ebone_remove_ex` (armature_edit.cc),
//! which the delete operator calls: the children of a deleted bone take its parent.


use glam::{Mat4, Quat, Vec3};

use crate::rig::geodesic::STATIC_INFLUENCE_INDEX;
use crate::rig::skinning::VertexWeights;
use crate::rig::Skeleton;

/// What deleting a bone did to the numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Removal {
    /// The number the deleted bone had.
    pub removed: usize,
    /// The bone it hung below, which takes over its children, weights and
    /// followers. None for a root.
    pub parent: Option<usize>,
}

impl Removal {
    /// The number a bone has now that it had `old` before: bones after the
    /// deleted one move down by one. None for the deleted bone itself.
    pub fn renumber(&self, old: usize) -> Option<usize> {
        if old == self.removed {
            None
        } else if old > self.removed {
            Some(old - 1)
        } else {
            Some(old)
        }
    }

    /// The bone something that followed bone `old` follows now: the same bone
    /// under its new number, or for the deleted bone its parent (None when it
    /// had none, so the thing follows the whole object).
    pub fn follow(&self, old: usize) -> Option<usize> {
        self.renumber(old).or(self.parent)
    }
}

/// Moves joint `index` to `position`, in the skeleton's own space, in the rest
/// pose. Its children stay where they are in space, so only this joint and the
/// bones that lead to it change. False, changing nothing, for an unknown joint
/// or a position that is not a number.
pub fn move_joint(skeleton: &mut Skeleton, index: usize, position: Vec3) -> bool {
    if index >= skeleton.bones.len() || !position.is_finite() {
        return false;
    }
    let world = skeleton.world_bind_matrices();
    let parent_world = match skeleton.bones[index].parent {
        Some(parent) => world[parent],
        None => Mat4::IDENTITY,
    };
    let local = parent_world.inverse().transform_point3(position);
    let moved = Mat4::from_scale_rotation_translation(
        skeleton.bones[index].local_scale,
        skeleton.bones[index].local_rotation,
        local,
    );
    let to_local = (parent_world * moved).inverse();
    if !local.is_finite() || !to_local.is_finite() {
        return false;
    }
    // Each child keeps its place in space: its position is worked out again
    // below the moved joint. Its own turn and scale are left as they were.
    let children: Vec<(usize, Vec3)> = (0..skeleton.bones.len())
        .filter(|&child| skeleton.bones[child].parent == Some(index))
        .map(|child| {
            let at = world[child].transform_point3(Vec3::ZERO);
            (child, to_local.transform_point3(at))
        })
        .collect();
    skeleton.bones[index].local_position = local;
    for (child, at) in children {
        skeleton.bones[child].local_position = at;
    }
    true
}

/// Deletes bone `index`. Its children hang below its parent and stay where they
/// are in space, and the bones after it move down one place. None, changing
/// nothing, for an unknown bone.
///
/// Whatever else counts bones by number (skin weights, the pose, animation keys,
/// objects that follow a bone) must be brought into step with the returned
/// `Removal`.
pub fn remove_bone(skeleton: &mut Skeleton, index: usize) -> Option<Removal> {
    if index >= skeleton.bones.len() {
        return None;
    }
    let world = skeleton.world_bind_matrices();
    let parent = skeleton.bones[index].parent;
    let parent_world = match parent {
        Some(parent) => world[parent],
        None => Mat4::IDENTITY,
    };
    let to_local = parent_world.inverse();
    let removed = skeleton.bones[index].clone();
    // A bone that is only moved (no turn, no scale) hands its children a plain
    // addition, which keeps their numbers exact.
    let plain = removed.local_rotation == Quat::IDENTITY && removed.local_scale == Vec3::ONE;

    for child in 0..skeleton.bones.len() {
        if skeleton.bones[child].parent != Some(index) {
            continue;
        }
        let bone = &mut skeleton.bones[child];
        bone.parent = parent;
        if plain {
            bone.local_position += removed.local_position;
        } else {
            let local = to_local * world[child];
            if local.is_finite() {
                let (scale, rotation, translation) = local.to_scale_rotation_translation();
                bone.local_position = translation;
                bone.local_rotation = rotation.normalize();
                bone.local_scale = scale;
            }
        }
    }

    skeleton.bones.remove(index);
    let removal = Removal {
        removed: index,
        parent,
    };
    for bone in skeleton.bones.iter_mut() {
        bone.parent = bone.parent.and_then(|parent| removal.renumber(parent));
    }
    Some(removal)
}

/// Brings skin weights into step with a deleted bone: its weight goes to its
/// parent (or to the influence that stays put when it had none), and the bones
/// after it are renumbered. Every vertex keeps the total weight it had.
pub fn remap_weights(weights: &mut [VertexWeights], removal: &Removal) {
    let target = match removal.parent {
        Some(parent) => parent as u32,
        None => STATIC_INFLUENCE_INDEX,
    };
    let removed = removal.removed as u32;
    for vertex in weights.iter_mut() {
        for slot in 0..4 {
            if vertex.weights[slot] > 0.0 && vertex.joint_indices[slot] == removed {
                let moved = vertex.weights[slot];
                let existing = (0..4).find(|&other| {
                    other != slot
                        && vertex.weights[other] > 0.0
                        && vertex.joint_indices[other] == target
                });
                match existing {
                    Some(other) => {
                        vertex.weights[other] += moved;
                        vertex.weights[slot] = 0.0;
                    }
                    None => vertex.joint_indices[slot] = target,
                }
            }
        }
        for slot in 0..4 {
            if vertex.weights[slot] > 0.0 {
                let joint = vertex.joint_indices[slot];
                if joint != STATIC_INFLUENCE_INDEX && joint > removed {
                    vertex.joint_indices[slot] = joint - 1;
                }
            } else {
                vertex.joint_indices[slot] = 0;
                vertex.weights[slot] = 0.0;
            }
        }
    }
}

/// Takes the deleted bone's turn out of a list with one turn per bone (the pose,
/// or one animation key).
pub fn remap_rotations(rotations: &mut Vec<Quat>, removal: &Removal) {
    if removal.removed < rotations.len() {
        rotations.remove(removal.removed);
    }
}

/// Whether `byte` separates a name from a side letter: `.`, a space, `-` or `_`
/// (Blender's `is_char_sep`).
fn is_name_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b' ' | b'-' | b'_')
}

/// The first place `word` (in lower case) appears in `text`, ignoring case.
fn find_ignoring_case(text: &str, word: &str) -> Option<usize> {
    let (text, word) = (text.as_bytes(), word.as_bytes());
    if word.len() > text.len() {
        return None;
    }
    (0..=text.len() - word.len())
        .find(|&start| text[start..start + word.len()].eq_ignore_ascii_case(word))
}

/// The name with its side flipped (left becomes right and the other way round),
/// as Blender's `BLI_string_flip_side_name` does it with the numbers kept:
/// - a name shorter than 3 characters is left alone;
/// - a `.###` number at the end is set aside and put back after;
/// - a side letter after a separator at the end (`arm.L`, `leg_r`, `hand L`) or
///   before one at the start (`L_arm`, `r.hand`) swaps, keeping its case;
/// - for names longer than 5 characters, the word `left` or `right` at the very
///   start or end swaps (`Left_arm`, `armRIGHT`), keeping its case.
/// A name with none of these comes back unchanged.
pub fn flip_side_name(name: &str) -> String {
    if name.len() < 3 {
        return name.to_string();
    }
    let mut body = name;
    let mut number = "";
    if name.as_bytes()[name.len() - 1].is_ascii_digit() {
        if let Some(dot) = name.rfind('.') {
            if name
                .as_bytes()
                .get(dot + 1)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                body = &name[..dot];
                number = &name[dot..];
            }
        }
    }

    let bytes = body.as_bytes();
    let length = bytes.len();
    let mut prefix = body.to_string();
    let mut suffix = String::new();
    let mut replace: Option<&str> = None;
    let mut is_set = false;

    // A side letter after a separator, at the end.
    if length > 1 && is_name_separator(bytes[length - 2]) {
        is_set = true;
        match bytes[length - 1] {
            b'l' => {
                prefix.truncate(length - 1);
                replace = Some("r");
            }
            b'r' => {
                prefix.truncate(length - 1);
                replace = Some("l");
            }
            b'L' => {
                prefix.truncate(length - 1);
                replace = Some("R");
            }
            b'R' => {
                prefix.truncate(length - 1);
                replace = Some("L");
            }
            _ => is_set = false,
        }
    }

    // A side letter at the start, before a separator.
    if !is_set && bytes.get(1).is_some_and(|byte| is_name_separator(*byte)) {
        is_set = true;
        match bytes[0] {
            b'l' => replace = Some("r"),
            b'r' => replace = Some("l"),
            b'L' => replace = Some("R"),
            b'R' => replace = Some("L"),
            _ => is_set = false,
        }
        if is_set {
            suffix = body[1..].to_string();
            prefix.clear();
        }
    }

    // The word left or right at the very start or end.
    if !is_set && length > 5 {
        let right = find_ignoring_case(&prefix, "right");
        let left = find_ignoring_case(&prefix, "left");
        if let Some(index) = right.filter(|&index| index == 0 || index == length - 5) {
            let word = &prefix[index..index + 5];
            replace = Some(if word.starts_with('r') {
                "left"
            } else if word.as_bytes()[1] == b'I' {
                "LEFT"
            } else {
                "Left"
            });
            suffix = prefix[index + 5..].to_string();
            prefix.truncate(index);
        } else if let Some(index) = left.filter(|&index| index == 0 || index == length - 4) {
            let word = &prefix[index..index + 4];
            replace = Some(if word.starts_with('l') {
                "right"
            } else if word.as_bytes()[1] == b'E' {
                "RIGHT"
            } else {
                "Right"
            });
            suffix = prefix[index + 4..].to_string();
            prefix.truncate(index);
        }
    }

    format!("{prefix}{}{suffix}{number}", replace.unwrap_or(""))
}

/// The name a mirrored copy of a bone gets: the name with its side flipped (see
/// `flip_side_name`). A name with no side to flip gets `.R` added. That last part
/// is Husk's own rule.
fn mirrored_name(name: &str) -> String {
    let flipped = flip_side_name(name);
    if flipped != name {
        flipped
    } else {
        format!("{name}.R")
    }
}


/// What a bone is called when it is given no name. Blender uses its own
/// `animrig::bone_default_name`, whose value is in a file not read yet, so this
/// one is Husk's choice for now.
pub const DEFAULT_BONE_NAME: &str = "Bone";

/// The most characters a bone name can have. This is Husk's own limit.
pub const MAX_BONE_NAME_CHARS: usize = 64;

/// Splits a trailing number off a name, as Blender's `BLI_string_split_name_number`
/// does: the part before the last `delim`, and the number after it, when
/// everything after the last `delim` is digits that fit in an `i32`. Any other
/// name comes back whole, with 0.
fn split_name_number(name: &str, delim: char) -> (&str, i32) {
    let Some(at) = name.rfind(delim) else {
        return (name, 0);
    };
    let digits = &name[at + delim.len_utf8()..];
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return (name, 0);
    }
    match digits.parse::<i32>() {
        Ok(number) => (&name[..at], number),
        Err(_) => (name, 0),
    }
}

/// `wanted` when no bone has that name. Otherwise the same name with a number,
/// counting up from the number it already ends in (`.001` when it has none) until
/// no bone has it: `arm.R` becomes `arm.R.001`, and `arm.R.001` becomes
/// `arm.R.002` (Blender's `BLI_uniquename_cb` with a `.`). An empty name becomes
/// `DEFAULT_BONE_NAME` first.
fn unique_name(skeleton: &Skeleton, wanted: &str) -> String {
    let name = if wanted.is_empty() {
        DEFAULT_BONE_NAME
    } else {
        wanted
    };
    let taken = |other: &str| skeleton.bones.iter().any(|bone| bone.name == other);
    if !taken(name) {
        return name.to_string();
    }
    let (left, mut number) = split_name_number(name, '.');
    loop {
        number = number.saturating_add(1);
        let candidate = if left.is_empty() {
            format!(".{number:03}")
        } else {
            format!("{left}.{number:03}")
        };
        if !taken(&candidate) {
            return candidate;
        }
    }
}

/// Renames bone `index` to `wanted`, the way Blender's `ED_armature_bone_rename`
/// treats the name itself: the same name changes nothing, and any other is made
/// unique (see `unique_name`; the bone's own old name counts as taken, as in
/// Blender). Husk keeps names to `MAX_BONE_NAME_CHARS` characters. Bones are
/// numbered everywhere else (weights, pose, keys, parts that follow a bone), so
/// nothing else needs to change. Returns the name the bone has now and whether
/// that is a change, or None, changing nothing, for an unknown bone.
pub fn rename_bone(skeleton: &mut Skeleton, index: usize, wanted: &str) -> Option<(String, bool)> {
    if index >= skeleton.bones.len() {
        return None;
    }
    let wanted: String = wanted.chars().take(MAX_BONE_NAME_CHARS).collect();
    if skeleton.bones[index].name == wanted {
        return Some((wanted, false));
    }
    let name = unique_name(skeleton, &wanted);
    let changed = skeleton.bones[index].name != name;
    skeleton.bones[index].name = name.clone();
    Some((name, changed))
}


/// The joint that mirrors joint `index`: the one whose name is `index`'s name
/// with its side flipped (`arm.L` and `arm.R`), as Blender's
/// `ED_armature_ebone_get_mirrored` finds it. None for a name with no side to
/// flip, for an unknown joint, or when no joint has the flipped name.
pub fn mirror_twin(skeleton: &Skeleton, index: usize) -> Option<usize> {
    let name = &skeleton.bones.get(index)?.name;
    let flipped = flip_side_name(name);
    if flipped == *name {
        return None;
    }
    skeleton.bones.iter().position(|bone| bone.name == flipped)
}

/// `move_joint`, and with `mirror` on the twin of the joint (see `mirror_twin`)
/// goes to the other side of the centre at once with its X flipped: Blender's
/// X-Axis Mirror, which copies the moved point to the mirrored bone with its X
/// negated (`ED_armature_ebone_transform_mirror_update`). The twin's children stay
/// where they are, as for any moved joint. False, changing nothing, when the
/// joint cannot be moved.
pub fn move_joint_mirrored(
    skeleton: &mut Skeleton,
    index: usize,
    position: Vec3,
    mirror: bool,
) -> bool {
    if !move_joint(skeleton, index, position) {
        return false;
    }
    if mirror {
        if let Some(twin) = mirror_twin(skeleton, index) {
            if twin != index {
                move_joint(skeleton, twin, Vec3::new(-position.x, position.y, position.z));
            }
        }
    }
    true
}

/// Mirrors joint `index` and every joint below it across the plane x = 0 of the
/// skeleton's own space (left becomes right). The copy of `index` hangs from the
/// same parent as `index`; every other copy hangs from the copy of its parent.
/// Copies are plain joints (no turn, no scale), named by `mirrored_name` and made
/// unique. Returns the numbers of the copies, the copy of `index` first. None,
/// changing nothing, for an unknown joint.
pub fn symmetrize(skeleton: &mut Skeleton, index: usize) -> Option<Vec<usize>> {
    let count = skeleton.bones.len();
    if index >= count {
        return None;
    }
    // The joint and everything below it.
    let mut inside = vec![false; count];
    inside[index] = true;
    loop {
        let mut grew = false;
        for bone in 0..count {
            if inside[bone] {
                continue;
            }
            if let Some(parent) = skeleton.bones[bone].parent {
                if inside[parent] {
                    inside[bone] = true;
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }

    let world = skeleton.world_bind_matrices();
    let mut copy_of: Vec<Option<usize>> = vec![None; count];
    let mut made = Vec::new();
    for bone in (0..count).filter(|&bone| inside[bone]) {
        let at = world[bone].transform_point3(Vec3::ZERO);
        let mirrored_at = Vec3::new(-at.x, at.y, at.z);
        let parent = if bone == index {
            skeleton.bones[bone].parent
        } else {
            skeleton.bones[bone].parent.and_then(|parent| copy_of[parent])
        };
        let parent_world = match parent {
            Some(parent) => skeleton.world_bind_matrices()[parent],
            None => Mat4::IDENTITY,
        };
        let name = unique_name(skeleton, &mirrored_name(&skeleton.bones[bone].name));
        let local = parent_world.inverse().transform_point3(mirrored_at);
        let new_index = skeleton.add_bone(&name, parent, local);
        copy_of[bone] = Some(new_index);
        made.push(new_index);
    }
    Some(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    /// 0 root, 1 a (above root), 2 b (above a), 3 side (beside root), 4 tip
    /// (above b). In space: a (0,1,0), b (0,2,0), side (1,0,0), tip (0,3,0).
    fn tree() -> Skeleton {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let a = skeleton.add_bone("a", Some(root), Vec3::new(0.0, 1.0, 0.0));
        let b = skeleton.add_bone("b", Some(a), Vec3::new(0.0, 1.0, 0.0));
        skeleton.add_bone("side", Some(root), Vec3::new(1.0, 0.0, 0.0));
        skeleton.add_bone("tip", Some(b), Vec3::new(0.0, 1.0, 0.0));
        skeleton
    }

    /// Where the bone with this name is in space, at rest.
    fn at(skeleton: &Skeleton, name: &str) -> Vec3 {
        let index = skeleton
            .bones
            .iter()
            .position(|bone| bone.name == name)
            .expect(name);
        skeleton.world_bind_matrices()[index].transform_point3(Vec3::ZERO)
    }

    fn names(skeleton: &Skeleton) -> Vec<&str> {
        skeleton.bones.iter().map(|bone| bone.name.as_str()).collect()
    }

    fn parents(skeleton: &Skeleton) -> Vec<Option<usize>> {
        skeleton.bones.iter().map(|bone| bone.parent).collect()
    }

    fn total(vertex: &VertexWeights) -> f32 {
        vertex.weights.iter().sum()
    }

    /// How much of the vertex's weight sits on `bone`.
    fn on(vertex: &VertexWeights, bone: u32) -> f32 {
        (0..4)
            .filter(|&slot| vertex.joint_indices[slot] == bone && vertex.weights[slot] > 0.0)
            .map(|slot| vertex.weights[slot])
            .sum()
    }

    #[test]
    fn moving_a_joint_moves_only_it_and_its_children_stay_where_they_are() {
        let mut skeleton = tree();
        assert!(move_joint(&mut skeleton, 1, Vec3::new(0.5, 1.5, 0.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.5, 1.5, 0.0)));
        // Below it, nothing moved in space.
        assert!(close(at(&skeleton, "b"), Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
        // Beside and above it, nothing moved either.
        assert!(close(at(&skeleton, "root"), Vec3::ZERO));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn moving_the_root_leaves_every_other_joint_in_place() {
        let mut skeleton = tree();
        assert!(move_joint(&mut skeleton, 0, Vec3::new(1.0, 2.0, 3.0)));
        assert!(close(at(&skeleton, "root"), Vec3::new(1.0, 2.0, 3.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn an_unknown_joint_or_a_position_that_is_not_a_number_moves_nothing() {
        let mut skeleton = tree();
        assert!(!move_joint(&mut skeleton, 99, Vec3::ONE));
        assert!(!move_joint(&mut skeleton, 1, Vec3::new(f32::NAN, 0.0, 0.0)));
        assert!(!move_joint(&mut skeleton, 1, Vec3::new(f32::INFINITY, 0.0, 0.0)));
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn deleting_a_middle_bone_hands_its_children_to_its_parent_in_place() {
        let mut skeleton = tree();
        let removal = remove_bone(&mut skeleton, 1).expect("bone 1 exists");
        assert_eq!(
            removal,
            Removal {
                removed: 1,
                parent: Some(0)
            }
        );
        assert_eq!(names(&skeleton), vec!["root", "b", "side", "tip"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0), Some(0), Some(1)]);
        assert!(close(at(&skeleton, "b"), Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn deleting_a_turned_bone_keeps_its_children_in_place_and_turned() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let a = skeleton.add_bone("a", Some(root), Vec3::new(0.0, 1.0, 0.0));
        skeleton.bones[a].local_rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        skeleton.add_bone("c", Some(a), Vec3::new(1.0, 0.0, 0.0));
        // Turned a quarter, a carries c from one unit along its own X to (0, 2, 0).
        assert!(close(at(&skeleton, "c"), Vec3::new(0.0, 2.0, 0.0)));
        let turned_before = skeleton.world_bind_matrices()[2].transform_vector3(Vec3::X);

        assert!(remove_bone(&mut skeleton, a).is_some());
        assert_eq!(names(&skeleton), vec!["root", "c"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0)]);
        assert!(close(at(&skeleton, "c"), Vec3::new(0.0, 2.0, 0.0)));
        let turned_after = skeleton.world_bind_matrices()[1].transform_vector3(Vec3::X);
        assert!(close(turned_before, turned_after));
    }

    #[test]
    fn deleting_the_root_makes_its_children_roots_in_place() {
        let mut skeleton = tree();
        let removal = remove_bone(&mut skeleton, 0).expect("bone 0 exists");
        assert_eq!(
            removal,
            Removal {
                removed: 0,
                parent: None
            }
        );
        assert_eq!(names(&skeleton), vec!["a", "b", "side", "tip"]);
        assert_eq!(parents(&skeleton), vec![None, Some(0), None, Some(1)]);
        assert!(close(at(&skeleton, "a"), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn mirroring_a_joint_copies_it_across_the_centre_with_the_same_parent() {
        let mut skeleton = tree();
        // "side" (bone 3) is at (1, 0, 0) below the root.
        let copies = symmetrize(&mut skeleton, 3).expect("bone 3 exists");
        assert_eq!(copies, vec![5]);
        assert_eq!(skeleton.bones[5].parent, Some(0));
        assert_eq!(skeleton.bones[5].name, "side.R");
        assert!(close(at(&skeleton, "side.R"), Vec3::new(-1.0, 0.0, 0.0)));
        // The original did not move.
        assert!(close(at(&skeleton, "side"), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn mirroring_a_joint_takes_everything_below_it_along() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let arm = skeleton.add_bone("arm.L", Some(root), Vec3::new(1.0, 0.0, 0.0));
        skeleton.add_bone("hand.L", Some(arm), Vec3::new(1.0, 0.5, 0.25));
        skeleton.add_bone("head", Some(root), Vec3::new(0.0, 2.0, 0.0));

        let copies = symmetrize(&mut skeleton, arm).expect("the arm exists");
        // Two copies, the arm and the hand. The head is not below the arm.
        assert_eq!(copies.len(), 2);
        assert_eq!(skeleton.bones.len(), 6);
        assert_eq!(skeleton.bones[copies[0]].name, "arm.R");
        assert_eq!(skeleton.bones[copies[1]].name, "hand.R");
        assert_eq!(skeleton.bones[copies[0]].parent, Some(root));
        assert_eq!(skeleton.bones[copies[1]].parent, Some(copies[0]));
        assert!(close(at(&skeleton, "arm.R"), Vec3::new(-1.0, 0.0, 0.0)));
        assert!(close(at(&skeleton, "hand.R"), Vec3::new(-2.0, 0.5, 0.25)));
        // The originals stay where they were.
        assert!(close(at(&skeleton, "hand.L"), Vec3::new(2.0, 0.5, 0.25)));
    }

    #[test]
    fn mirroring_works_in_space_even_when_the_parent_is_turned() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        skeleton.bones[root].local_rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let arm = skeleton.add_bone("arm", Some(root), Vec3::new(1.0, 1.0, 0.0));
        // The root turns (1, 1, 0) a quarter: the arm is at (-1, 1, 0) in space.
        assert!(close(at(&skeleton, "arm"), Vec3::new(-1.0, 1.0, 0.0)));

        let copies = symmetrize(&mut skeleton, arm).expect("the arm exists");
        assert_eq!(copies.len(), 1);
        assert!(close(at(&skeleton, "arm.R"), Vec3::new(1.0, 1.0, 0.0)));
    }

    #[test]
    fn a_mirrored_name_swaps_the_side_or_adds_one_and_is_never_taken_twice() {
        assert_eq!(mirrored_name("arm.L"), "arm.R");
        assert_eq!(mirrored_name("arm.R"), "arm.L");
        assert_eq!(mirrored_name("leg_L"), "leg_R");
        assert_eq!(mirrored_name("leg_R"), "leg_L");
        assert_eq!(mirrored_name("spine"), "spine.R");

        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let arm = skeleton.add_bone("arm.L", Some(root), Vec3::new(1.0, 0.0, 0.0));
        let first = symmetrize(&mut skeleton, arm).unwrap()[0];
        let second = symmetrize(&mut skeleton, arm).unwrap()[0];
        assert_eq!(skeleton.bones[first].name, "arm.R");
        assert_eq!(skeleton.bones[second].name, "arm.R.001");
        assert!(symmetrize(&mut skeleton, 99).is_none());
        assert_eq!(skeleton.bones.len(), 4);
    }

    #[test]
    fn flipping_a_name_follows_blenders_rules() {
        for (name, flipped) in [
            ("arm.L", "arm.R"),
            ("arm.R", "arm.L"),
            ("leg_l", "leg_r"),
            ("hand R", "hand L"),
            ("L_arm", "R_arm"),
            ("r.hand", "l.hand"),
            ("arm.L.001", "arm.R.001"),
            ("Left_arm", "Right_arm"),
            ("LEFT_arm", "RIGHT_arm"),
            ("arm_RIGHT", "arm_LEFT"),
            ("armLeft", "armRight"),
            ("rightarm", "leftarm"),
            ("myleft", "myright"),
        ] {
            assert_eq!(flip_side_name(name), flipped, "{name}");
        }
        // Nothing to flip, or too short to have a side: the name stays.
        for name in [
            "spine", "Bone.001", "head", ".R", "L", "left", "Right", "arm_left_x", "L.001",
        ] {
            assert_eq!(flip_side_name(name), name, "{name}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number_the_way_blender_counts_it() {
        let mut skeleton = Skeleton::default();
        for name in ["arm.R", "arm.R.001", "x.5", "x.", ".5", "Bone"] {
            skeleton.add_bone(name, None, Vec3::ZERO);
        }
        assert_eq!(unique_name(&skeleton, "free"), "free");
        // .001 is taken as well, so it goes on to .002.
        assert_eq!(unique_name(&skeleton, "arm.R"), "arm.R.002");
        // A number it already ends in is counted up, not added to.
        assert_eq!(unique_name(&skeleton, "arm.R.001"), "arm.R.002");
        assert_eq!(unique_name(&skeleton, "x.5"), "x.006");
        // A dot with nothing after it is not a number.
        assert_eq!(unique_name(&skeleton, "x."), "x..001");
        // Nothing before the number: only the number is left.
        assert_eq!(unique_name(&skeleton, ".5"), ".006");
        // An empty name becomes the default name, which is taken here.
        assert_eq!(unique_name(&skeleton, ""), "Bone.001");
        assert_eq!(unique_name(&Skeleton::default(), ""), "Bone");
    }

    #[test]
    fn renaming_a_bone_changes_only_its_name_and_a_taken_name_gets_a_number() {
        let mut skeleton = tree();
        let world_before = skeleton.world_bind_matrices();
        let parents_before = parents(&skeleton);

        assert_eq!(
            rename_bone(&mut skeleton, 1, "upper_arm"),
            Some((String::from("upper_arm"), true))
        );
        assert_eq!(skeleton.bones[1].name, "upper_arm");
        // The same name changes nothing.
        assert_eq!(
            rename_bone(&mut skeleton, 1, "upper_arm"),
            Some((String::from("upper_arm"), false))
        );
        // A name another bone has gets a number.
        assert_eq!(
            rename_bone(&mut skeleton, 2, "tip"),
            Some((String::from("tip.001"), true))
        );
        // An empty name becomes the default name.
        assert_eq!(
            rename_bone(&mut skeleton, 3, ""),
            Some((String::from("Bone"), true))
        );
        // A name that is too long is held to the limit.
        let long = "x".repeat(100);
        let (given, _) = rename_bone(&mut skeleton, 4, &long).expect("bone 4 exists");
        assert_eq!(given.chars().count(), MAX_BONE_NAME_CHARS);
        // An unknown bone changes nothing.
        assert_eq!(rename_bone(&mut skeleton, 99, "x"), None);

        // Only names changed: not the parents, not where anything is.
        assert_eq!(parents(&skeleton), parents_before);
        assert_eq!(skeleton.world_bind_matrices(), world_before);
    }

    #[test]
    fn a_joints_twin_is_the_one_with_the_flipped_name() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let left = skeleton.add_bone("arm.L", Some(root), Vec3::new(1.0, 1.0, 0.0));
        let right = skeleton.add_bone("arm.R", Some(root), Vec3::new(-1.0, 1.0, 0.0));
        let lonely = skeleton.add_bone("leg_L", Some(root), Vec3::new(0.5, -1.0, 0.0));
        assert_eq!(mirror_twin(&skeleton, left), Some(right));
        assert_eq!(mirror_twin(&skeleton, right), Some(left));
        // No side in the name, or no joint with the flipped name, or no such joint.
        assert_eq!(mirror_twin(&skeleton, root), None);
        assert_eq!(mirror_twin(&skeleton, lonely), None);
        assert_eq!(mirror_twin(&skeleton, 99), None);
    }

    #[test]
    fn moving_a_joint_with_mirror_on_moves_its_twin_to_the_other_side() {
        let mut skeleton = Skeleton::default();
        let root = skeleton.add_bone("root", None, Vec3::ZERO);
        let left = skeleton.add_bone("arm.L", Some(root), Vec3::new(1.0, 1.0, 0.0));
        let right = skeleton.add_bone("arm.R", Some(root), Vec3::new(-1.0, 1.0, 0.0));
        skeleton.add_bone("hand.R", Some(right), Vec3::new(-1.0, 0.0, 0.0));
        // hand.R is at (-2, 1, 0).

        // Mirror off: only the joint that was moved changes.
        assert!(move_joint_mirrored(&mut skeleton, left, Vec3::new(2.0, 2.0, 0.5), false));
        assert!(close(at(&skeleton, "arm.L"), Vec3::new(2.0, 2.0, 0.5)));
        assert!(close(at(&skeleton, "arm.R"), Vec3::new(-1.0, 1.0, 0.0)));

        // Mirror on: the twin follows with its X flipped, and its child stays put.
        assert!(move_joint_mirrored(&mut skeleton, left, Vec3::new(3.0, 1.5, 0.25), true));
        assert!(close(at(&skeleton, "arm.L"), Vec3::new(3.0, 1.5, 0.25)));
        assert!(close(at(&skeleton, "arm.R"), Vec3::new(-3.0, 1.5, 0.25)));
        assert!(close(at(&skeleton, "hand.R"), Vec3::new(-2.0, 1.0, 0.0)));

        // A joint with no twin just moves, and a joint that does not exist changes nothing.
        assert!(move_joint_mirrored(&mut skeleton, root, Vec3::new(0.0, 0.5, 0.0), true));
        assert!(close(at(&skeleton, "arm.R"), Vec3::new(-3.0, 1.5, 0.25)));
        assert!(!move_joint_mirrored(&mut skeleton, 99, Vec3::ONE, true));
    }

    #[test]
    fn deleting_a_bone_that_does_not_exist_changes_nothing() {
        let mut skeleton = tree();
        assert!(remove_bone(&mut skeleton, 5).is_none());
        assert_eq!(skeleton.bones.len(), 5);
        assert!(close(at(&skeleton, "tip"), Vec3::new(0.0, 3.0, 0.0)));
    }

    #[test]
    fn the_numbering_follows_the_deleted_bone_and_the_turns_lose_its_slot() {
        let removal = Removal {
            removed: 2,
            parent: Some(1),
        };
        assert_eq!(removal.renumber(0), Some(0));
        assert_eq!(removal.renumber(2), None);
        assert_eq!(removal.renumber(3), Some(2));
        assert_eq!(removal.follow(2), Some(1));
        assert_eq!(removal.follow(3), Some(2));
        assert_eq!(removal.follow(0), Some(0));

        let root = Removal {
            removed: 0,
            parent: None,
        };
        assert_eq!(root.follow(0), None);
        assert_eq!(root.follow(2), Some(1));

        let mut rotations = vec![
            Quat::IDENTITY,
            Quat::from_rotation_x(1.0),
            Quat::from_rotation_y(1.0),
        ];
        remap_rotations(
            &mut rotations,
            &Removal {
                removed: 1,
                parent: Some(0),
            },
        );
        assert_eq!(rotations, vec![Quat::IDENTITY, Quat::from_rotation_y(1.0)]);
        // A list that is shorter than the bone numbers is left alone.
        let mut short = vec![Quat::IDENTITY];
        remap_rotations(
            &mut short,
            &Removal {
                removed: 5,
                parent: Some(0),
            },
        );
        assert_eq!(short.len(), 1);
    }

    #[test]
    fn the_weight_of_a_deleted_bone_goes_to_its_parent_and_the_total_never_changes() {
        let removal = Removal {
            removed: 1,
            parent: Some(0),
        };
        let mut vertices = vec![
            // Bone 1 with bone 2: bone 1's share goes to bone 0.
            VertexWeights {
                joint_indices: [1, 2, 0, 0],
                weights: [0.6, 0.4, 0.0, 0.0],
            },
            // Bone 0 is already there: the two shares are added.
            VertexWeights {
                joint_indices: [0, 1, 2, 0],
                weights: [0.3, 0.5, 0.2, 0.0],
            },
            // All four slots full: bone 1's slot is the one that is freed.
            VertexWeights {
                joint_indices: [1, 2, 3, 4],
                weights: [0.25, 0.25, 0.25, 0.25],
            },
            // Nothing to do with bone 1.
            VertexWeights {
                joint_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ];
        remap_weights(&mut vertices, &removal);

        assert!((on(&vertices[0], 0) - 0.6).abs() < 1e-6);
        assert!((on(&vertices[0], 1) - 0.4).abs() < 1e-6);

        assert!((on(&vertices[1], 0) - 0.8).abs() < 1e-6);
        assert!((on(&vertices[1], 1) - 0.2).abs() < 1e-6);

        for (bone, share) in [(0u32, 0.25f32), (1, 0.25), (2, 0.25), (3, 0.25)] {
            assert!((on(&vertices[2], bone) - share).abs() < 1e-6);
        }

        assert!((on(&vertices[3], 0) - 1.0).abs() < 1e-6);
        for vertex in &vertices {
            assert!((total(vertex) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_weight_of_a_deleted_root_goes_to_the_influence_that_stays_put() {
        let removal = Removal {
            removed: 0,
            parent: None,
        };
        let mut vertices = vec![
            VertexWeights {
                joint_indices: [0, 2, 0, 0],
                weights: [0.7, 0.3, 0.0, 0.0],
            },
            // The static influence is already there: the shares are added.
            VertexWeights {
                joint_indices: [0, STATIC_INFLUENCE_INDEX, 0, 0],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
        ];
        remap_weights(&mut vertices, &removal);

        assert!((on(&vertices[0], STATIC_INFLUENCE_INDEX) - 0.7).abs() < 1e-6);
        assert!((on(&vertices[0], 1) - 0.3).abs() < 1e-6);
        assert!((on(&vertices[1], STATIC_INFLUENCE_INDEX) - 1.0).abs() < 1e-6);
        for vertex in &vertices {
            assert!((total(vertex) - 1.0).abs() < 1e-6);
        }
    }
}