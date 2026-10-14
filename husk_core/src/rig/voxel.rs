use glam::Vec3;
use std::collections::VecDeque;

/// Empty cells kept around the mesh's bounding box so the flood fill that
/// finds the outside always has a clear path around the whole mesh.
const PADDING_CELLS: usize = 2;

const NEIGHBORS_6: [(isize, isize, isize); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// A regular 3D grid laid over a mesh's bounding box. Each cell is either
/// solid (on the mesh surface or enclosed by it) or empty (outside).
/// The geodesic weighting step flood-fills through the solid cells to
/// measure distance *through the body* between bones and vertices.
pub struct VoxelGrid {
    origin: Vec3,
    cell_size: f32,
    dims: [usize; 3],
    solid: Vec<bool>,
}

impl VoxelGrid {
    /// Voxelizes a triangle mesh. `cells_along_longest` sets the
    /// resolution: the mesh's longest side is split into that many
    /// cells. Returns None for an empty or flat-zero-size mesh.
    pub fn from_mesh(
        positions: &[[f32; 3]],
        indices: &[u32],
        cells_along_longest: usize,
    ) -> Option<VoxelGrid> {
        if positions.is_empty() || indices.len() < 3 || cells_along_longest == 0 {
            return None;
        }

        let mut bounds_min = Vec3::splat(f32::MAX);
        let mut bounds_max = Vec3::splat(f32::MIN);
        for position in positions {
            let p = Vec3::from(*position);
            bounds_min = bounds_min.min(p);
            bounds_max = bounds_max.max(p);
        }
        let extent = bounds_max - bounds_min;
        let longest = extent.max_element();
        if longest <= 0.0 {
            return None;
        }

        let cell_size = longest / cells_along_longest as f32;
        let origin = bounds_min - Vec3::splat(PADDING_CELLS as f32 * cell_size);
        let cells_for = |length: f32| (length / cell_size).ceil() as usize + 1 + 2 * PADDING_CELLS;
        let dims = [cells_for(extent.x), cells_for(extent.y), cells_for(extent.z)];
        let total = dims[0] * dims[1] * dims[2];

        let mut grid = VoxelGrid {
            origin,
            cell_size,
            dims,
            solid: Vec::new(),
        };

        // Step 1: mark every cell that any triangle touches (the shell).
        let half = Vec3::splat(cell_size * 0.5 * 1.0001);
        let mut surface = vec![false; total];
        for triangle in indices.chunks_exact(3) {
            let a = Vec3::from(positions[triangle[0] as usize]);
            let b = Vec3::from(positions[triangle[1] as usize]);
            let c = Vec3::from(positions[triangle[2] as usize]);

            let lo = grid.clamped_cell(a.min(b).min(c));
            let hi = grid.clamped_cell(a.max(b).max(c));
            for iz in lo[2]..=hi[2] {
                for iy in lo[1]..=hi[1] {
                    for ix in lo[0]..=hi[0] {
                        let cell = [ix, iy, iz];
                        let i = grid.index(cell);
                        if surface[i] {
                            continue;
                        }
                        if triangle_overlaps_cell(a, b, c, grid.cell_center(cell), half) {
                            surface[i] = true;
                        }
                    }
                }
            }
        }

        // Step 2: flood the outside from a padding corner, stopping at
        // the shell. Whatever the flood can't reach is solid.
        let mut exterior = vec![false; total];
        let mut queue: VecDeque<[usize; 3]> = VecDeque::new();
        exterior[0] = true;
        queue.push_back([0, 0, 0]);
        while let Some([x, y, z]) = queue.pop_front() {
            for &(dx, dy, dz) in NEIGHBORS_6.iter() {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                let nz = z as isize + dz;
                if nx < 0
                    || ny < 0
                    || nz < 0
                    || nx >= dims[0] as isize
                    || ny >= dims[1] as isize
                    || nz >= dims[2] as isize
                {
                    continue;
                }
                let neighbor = [nx as usize, ny as usize, nz as usize];
                let i = grid.index(neighbor);
                if exterior[i] || surface[i] {
                    continue;
                }
                exterior[i] = true;
                queue.push_back(neighbor);
            }
        }

        grid.solid = exterior.iter().map(|outside| !outside).collect();
        Some(grid)
    }

    pub fn dims(&self) -> [usize; 3] {
        self.dims
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Flat array index of a cell (x fastest, then y, then z).
    pub fn index(&self, cell: [usize; 3]) -> usize {
        cell[0] + self.dims[0] * (cell[1] + self.dims[1] * cell[2])
    }

    /// World-space center of a cell.
    pub fn cell_center(&self, cell: [usize; 3]) -> Vec3 {
        let cell_position = Vec3::new(cell[0] as f32, cell[1] as f32, cell[2] as f32);
        self.origin + (cell_position + Vec3::splat(0.5)) * self.cell_size
    }

    /// The cell containing a world-space point, or None if the point is
    /// outside the grid.
    pub fn cell_of(&self, point: Vec3) -> Option<[usize; 3]> {
        let relative = (point - self.origin) / self.cell_size;
        if relative.x < 0.0 || relative.y < 0.0 || relative.z < 0.0 {
            return None;
        }
        let cell = [relative.x as usize, relative.y as usize, relative.z as usize];
        if cell[0] >= self.dims[0] || cell[1] >= self.dims[1] || cell[2] >= self.dims[2] {
            None
        } else {
            Some(cell)
        }
    }

    /// True if the cell is on the mesh surface or enclosed by it.
    /// Cells outside the grid count as not solid.
    pub fn is_solid(&self, cell: [usize; 3]) -> bool {
        if cell[0] >= self.dims[0] || cell[1] >= self.dims[1] || cell[2] >= self.dims[2] {
            return false;
        }
        self.solid[self.index(cell)]
    }

    pub fn solid_count(&self) -> usize {
        self.solid.iter().filter(|is_solid| **is_solid).count()
    }

    /// The cell containing `point`, clamped into the grid.
    fn clamped_cell(&self, point: Vec3) -> [usize; 3] {
        let relative = (point - self.origin) / self.cell_size;
        [
            (relative.x.floor().max(0.0) as usize).min(self.dims[0] - 1),
            (relative.y.floor().max(0.0) as usize).min(self.dims[1] - 1),
            (relative.z.floor().max(0.0) as usize).min(self.dims[2] - 1),
        ]
    }
}

/// Exact triangle vs. axis-aligned box overlap (separating-axis test).
/// `half` is the box's half-size around `center`.
fn triangle_overlaps_cell(a: Vec3, b: Vec3, c: Vec3, center: Vec3, half: Vec3) -> bool {
    let v0 = a - center;
    let v1 = b - center;
    let v2 = c - center;
    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;

    // The three box-face axes: triangle's bounding box vs. the cell.
    let tri_min = v0.min(v1).min(v2);
    let tri_max = v0.max(v1).max(v2);
    if tri_min.x > half.x
        || tri_max.x < -half.x
        || tri_min.y > half.y
        || tri_max.y < -half.y
        || tri_min.z > half.z
        || tri_max.z < -half.z
    {
        return false;
    }

    // The triangle's own plane vs. the cell.
    let normal = e0.cross(e1);
    let plane_distance = normal.dot(v0);
    let plane_radius =
        half.x * normal.x.abs() + half.y * normal.y.abs() + half.z * normal.z.abs();
    if plane_distance.abs() > plane_radius {
        return false;
    }

    // The nine axes made from (box axis x triangle edge).
    for edge in [e0, e1, e2].iter() {
        for box_axis in [Vec3::X, Vec3::Y, Vec3::Z].iter() {
            let axis = box_axis.cross(*edge);
            // Skip edges nearly parallel to a box axis: the axis is noise.
            if axis.length_squared() <= 1e-10 * edge.length_squared() {
                continue;
            }
            let p0 = v0.dot(axis);
            let p1 = v1.dot(axis);
            let p2 = v2.dot(axis);
            let radius = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
            if p0.min(p1).min(p2) > radius || p0.max(p1).max(p2) < -radius {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed axis-aligned box as 8 vertices and 12 triangles.
    fn cube(min: Vec3, max: Vec3) -> (Vec<[f32; 3]>, Vec<u32>) {
        let positions = vec![
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [max.x, max.y, min.z],
            [min.x, max.y, min.z],
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
        ];
        let indices = vec![
            0, 1, 2, 0, 2, 3, // -z
            4, 5, 6, 4, 6, 7, // +z
            0, 1, 5, 0, 5, 4, // -y
            3, 2, 6, 3, 6, 7, // +y
            0, 3, 7, 0, 7, 4, // -x
            1, 2, 6, 1, 6, 5, // +x
        ];
        (positions, indices)
    }

    #[test]
    fn closed_cube_is_solid_inside_and_empty_outside() {
        let (positions, indices) = cube(Vec3::splat(-1.0), Vec3::splat(1.0));
        let grid = VoxelGrid::from_mesh(&positions, &indices, 20).unwrap();

        let inside = grid.cell_of(Vec3::ZERO).unwrap();
        assert!(grid.is_solid(inside));

        let outside = grid.cell_of(Vec3::new(1.15, 0.0, 0.0)).unwrap();
        assert!(!grid.is_solid(outside));

        assert!(!grid.is_solid([0, 0, 0]));

        let count = grid.solid_count();
        assert!((7_000..14_000).contains(&count), "solid count was {count}");
    }

    #[test]
    fn gap_between_two_cubes_stays_empty() {
        let (mut positions, mut indices) =
            cube(Vec3::new(-3.0, -1.0, -1.0), Vec3::new(-1.0, 1.0, 1.0));
        let (more_positions, more_indices) =
            cube(Vec3::new(1.0, -1.0, -1.0), Vec3::new(3.0, 1.0, 1.0));
        let offset = positions.len() as u32;
        positions.extend(more_positions);
        indices.extend(more_indices.iter().map(|i| i + offset));

        let grid = VoxelGrid::from_mesh(&positions, &indices, 60).unwrap();
        assert!(grid.is_solid(grid.cell_of(Vec3::new(-2.0, 0.0, 0.0)).unwrap()));
        assert!(grid.is_solid(grid.cell_of(Vec3::new(2.0, 0.0, 0.0)).unwrap()));
        assert!(!grid.is_solid(grid.cell_of(Vec3::ZERO).unwrap()));
    }

    #[test]
    fn cell_center_round_trips_through_cell_of() {
        let (positions, indices) = cube(Vec3::splat(-1.0), Vec3::splat(1.0));
        let grid = VoxelGrid::from_mesh(&positions, &indices, 20).unwrap();
        let center = grid.cell_center([10, 11, 12]);
        assert_eq!(grid.cell_of(center), Some([10, 11, 12]));
    }

    #[test]
    fn empty_mesh_gives_no_grid() {
        assert!(VoxelGrid::from_mesh(&[], &[], 20).is_none());
    }
}