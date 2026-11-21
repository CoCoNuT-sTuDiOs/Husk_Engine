//! Which faces touch a vertex: the neighbourhood the weight Smooth brush
//! averages over.
//!
//! Ported from Blender (the weight paint stroke code, `do_wpaint_brush_blur`;
//! SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All rights reserved.,
//! SPDX-License-Identifier: GPL-2.0-or-later): a vertex is averaged with every
//! corner of every face around it, so it counts once for each of its faces, a
//! vertex across an edge counts twice and one across a corner once.
//!
//! Husk differs in one way. glTF splits a vertex along a seam into several
//! copies at the same position. Copies at exactly the same position are welded
//! into one group, and a group's faces are the faces of all its copies, so a
//! seam does not cut the surface in two when averaging and the copies come out
//! the same.

use std::collections::HashMap;

/// For every vertex of one mesh, the corners of the triangles around it.
pub struct Adjacency {
    /// For each vertex, the group of vertices at its exact position.
    group_of: Vec<u32>,
    /// Where each group's corners start in `corner_list` (one extra at the end).
    offsets: Vec<u32>,
    /// For each group, the three corners (vertex numbers) of every triangle
    /// that touches it, one triangle after another.
    corner_list: Vec<u32>,
}

/// Whether a triangle only names vertices that exist.
fn names_real_vertices(triangle: &[u32], vertex_count: usize) -> bool {
    triangle
        .iter()
        .all(|&vertex| (vertex as usize) < vertex_count)
}

impl Adjacency {
    /// Builds the lists for a mesh of `positions` and triangle `indices` (three
    /// per triangle). A triangle that names a vertex that does not exist is
    /// skipped.
    pub fn build(positions: &[[f32; 3]], indices: &[u32]) -> Adjacency {
        let count = positions.len();
        let mut group_of = vec![0u32; count];
        let mut group_count = 0u32;
        let mut seen: HashMap<[u32; 3], u32> = HashMap::with_capacity(count);
        for (vertex, position) in positions.iter().enumerate() {
            // Adding 0.0 turns -0.0 into 0.0, so both zeros are the same key.
            let key = [
                (position[0] + 0.0).to_bits(),
                (position[1] + 0.0).to_bits(),
                (position[2] + 0.0).to_bits(),
            ];
            let group = *seen.entry(key).or_insert_with(|| {
                group_count += 1;
                group_count - 1
            });
            group_of[vertex] = group;
        }
        let group_count = group_count as usize;

        // Count first, so every group's corners sit side by side in one array.
        let mut offsets = vec![0u32; group_count + 1];
        for triangle in indices.chunks_exact(3) {
            if !names_real_vertices(triangle, count) {
                continue;
            }
            for &vertex in triangle {
                offsets[group_of[vertex as usize] as usize + 1] += 3;
            }
        }
        for group in 0..group_count {
            offsets[group + 1] += offsets[group];
        }

        let mut next = offsets.clone();
        let mut corner_list = vec![0u32; offsets[group_count] as usize];
        for triangle in indices.chunks_exact(3) {
            if !names_real_vertices(triangle, count) {
                continue;
            }
            for &vertex in triangle {
                let group = group_of[vertex as usize] as usize;
                let at = next[group] as usize;
                corner_list[at..at + 3].copy_from_slice(triangle);
                next[group] += 3;
            }
        }

        Adjacency {
            group_of,
            offsets,
            corner_list,
        }
    }

    /// How many vertices the mesh had when this was built.
    pub fn vertex_count(&self) -> usize {
        self.group_of.len()
    }

    /// The corners of every triangle around `vertex` (and around its copies at
    /// the same position), three for each triangle. Empty for a vertex that no
    /// triangle touches and for one that does not exist.
    pub fn corners(&self, vertex: usize) -> &[u32] {
        let Some(&group) = self.group_of.get(vertex) else {
            return &[];
        };
        let start = self.offsets[group as usize] as usize;
        let end = self.offsets[group as usize + 1] as usize;
        &self.corner_list[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> [[f32; 3]; 4] {
        [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
    }

    #[test]
    fn a_single_triangle_gives_each_corner_that_triangle() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let adjacency = Adjacency::build(&positions, &[0, 1, 2]);
        assert_eq!(adjacency.vertex_count(), 3);
        for vertex in 0..3 {
            assert_eq!(adjacency.corners(vertex), &[0, 1, 2]);
        }
    }

    #[test]
    fn two_triangles_sharing_an_edge_give_the_shared_corners_both_triangles() {
        let adjacency = Adjacency::build(&square(), &[0, 1, 2, 0, 2, 3]);
        assert_eq!(adjacency.corners(0), &[0, 1, 2, 0, 2, 3]);
        assert_eq!(adjacency.corners(1), &[0, 1, 2]);
        assert_eq!(adjacency.corners(2), &[0, 1, 2, 0, 2, 3]);
        assert_eq!(adjacency.corners(3), &[0, 2, 3]);
    }

    #[test]
    fn copies_at_the_same_position_share_one_list_so_a_seam_does_not_cut_the_surface() {
        // The same square, but the second triangle has its own copies of the
        // corners it shares: vertices 4, 5 and 6 sit where 0, 2 and 3 are.
        let mut positions = square().to_vec();
        positions.push([0.0, 0.0, 0.0]);
        positions.push([1.0, 1.0, 0.0]);
        positions.push([0.0, 1.0, 0.0]);
        let adjacency = Adjacency::build(&positions, &[0, 1, 2, 4, 5, 6]);
        // Vertex 0 and its copy 4 see the faces of both.
        assert_eq!(adjacency.corners(0), &[0, 1, 2, 4, 5, 6]);
        assert_eq!(adjacency.corners(4), adjacency.corners(0));
        // So do 3 and its copy 6, though only 6 is in a triangle.
        assert_eq!(adjacency.corners(3), adjacency.corners(6));
        assert_eq!(adjacency.corners(3), &[4, 5, 6]);
    }

    #[test]
    fn negative_zero_and_zero_are_the_same_position() {
        let positions = [[0.0, 0.0, 0.0], [-0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let adjacency = Adjacency::build(&positions, &[0, 2, 1]);
        assert_eq!(adjacency.corners(0), adjacency.corners(1));
        assert_eq!(adjacency.corners(0).len(), 6);
    }

    #[test]
    fn bad_indices_lonely_vertices_and_unknown_vertices_are_ignored() {
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [5.0, 5.0, 5.0],
        ];
        // The second triangle points at vertices that do not exist, and the
        // last index has no triangle to belong to.
        let adjacency = Adjacency::build(&positions, &[0, 1, 2, 7, 8, 9, 5]);
        assert_eq!(adjacency.corners(0), &[0, 1, 2]);
        // Vertex 3 is in no triangle.
        assert!(adjacency.corners(3).is_empty());
        assert!(adjacency.corners(99).is_empty());
        // A mesh with nothing in it is fine.
        let empty = Adjacency::build(&[], &[]);
        assert_eq!(empty.vertex_count(), 0);
        assert!(empty.corners(0).is_empty());
    }
}