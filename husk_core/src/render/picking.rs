pub struct Ray {
    pub origin: glam::Vec3,
    pub direction: glam::Vec3,
}

impl Ray {
    /// Builds a world-space ray from a screen-space click position, given
    /// the viewport size and the camera's combined view-projection matrix.
    /// Not covered by this step's test yet verified once wired to a real
    /// camera and real screen coordinates.
    pub fn from_screen(
        screen_x: f32,
        screen_y: f32,
        viewport_width: f32,
        viewport_height: f32,
        view_proj: glam::Mat4,
    ) -> Ray {
        let ndc_x = (screen_x / viewport_width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / viewport_height) * 2.0;

        let inverse_view_proj = view_proj.inverse();

        let near_point = inverse_view_proj.project_point3(glam::Vec3::new(ndc_x, ndc_y, 0.0));
        let far_point = inverse_view_proj.project_point3(glam::Vec3::new(ndc_x, ndc_y, 1.0));

        Ray {
            origin: near_point,
            direction: (far_point - near_point).normalize(),
        }
    }
}

/// Result of a successful ray-mesh intersection.
pub struct RayHit {
    pub point: glam::Vec3,
    pub distance: f32,
}

/// Finds the closest point where `ray` intersects any triangle in the
/// mesh (given as raw positions + indices), using the Moller-Trumbore
/// ray-triangle intersection algorithm — a standard, published algorithm,
/// not proprietary engine logic.
pub fn ray_mesh_intersection(
    ray: &Ray,
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<RayHit> {
    let mut closest: Option<RayHit> = None;

    for triangle in indices.chunks_exact(3) {
        let v0 = glam::Vec3::from(positions[triangle[0] as usize]);
        let v1 = glam::Vec3::from(positions[triangle[1] as usize]);
        let v2 = glam::Vec3::from(positions[triangle[2] as usize]);

        if let Some(t) = ray_triangle_intersection(ray, v0, v1, v2) {
            if closest.as_ref().map_or(true, |hit| t < hit.distance) {
                closest = Some(RayHit {
                    point: ray.origin + ray.direction * t,
                    distance: t,
                });
            }
        }
    }

    closest
}

/// Finds where `ray` first enters the mesh and where it last leaves it,
/// and returns the point halfway between them: the middle of whatever
/// the ray passes through, not its front surface. If the ray only
/// crosses the mesh once, the entry point comes back unchanged.
pub fn ray_mesh_midpoint(
    ray: &Ray,
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<glam::Vec3> {
    let mut nearest: Option<f32> = None;
    let mut farthest: Option<f32> = None;

    for triangle in indices.chunks_exact(3) {
        let v0 = glam::Vec3::from(positions[triangle[0] as usize]);
        let v1 = glam::Vec3::from(positions[triangle[1] as usize]);
        let v2 = glam::Vec3::from(positions[triangle[2] as usize]);

        if let Some(t) = ray_triangle_intersection(ray, v0, v1, v2) {
            if nearest.map_or(true, |n| t < n) {
                nearest = Some(t);
            }
            if farthest.map_or(true, |f| t > f) {
                farthest = Some(t);
            }
        }
    }

    let (near_t, far_t) = (nearest?, farthest?);
    Some(ray.origin + ray.direction * ((near_t + far_t) * 0.5))
}

fn ray_triangle_intersection(ray: &Ray, v0: glam::Vec3, v1: glam::Vec3, v2: glam::Vec3) -> Option<f32> {
    const EPSILON: f32 = 1e-6;

    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if a.abs() < EPSILON {
        return None; // Ray is parallel to the triangle.
    }

    let f = 1.0 / a;
    let s = ray.origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);
    if t > EPSILON {
        Some(t)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two square faces facing each other: a front quad at z = 1 and a
    /// back quad at z = -1, each spanning x, y in [-1, 1].
    fn two_parallel_quads() -> (Vec<[f32; 3]>, Vec<u32>) {
        let positions = vec![
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
        ];
        let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
        (positions, indices)
    }

    fn ray_down_z() -> Ray {
        Ray {
            origin: glam::Vec3::new(0.3, 0.2, 5.0),
            direction: glam::Vec3::new(0.0, 0.0, -1.0),
        }
    }

    #[test]
    fn midpoint_lands_between_front_and_back_surface() {
        let (positions, indices) = two_parallel_quads();
        let mid = ray_mesh_midpoint(&ray_down_z(), &positions, &indices).unwrap();
        assert!((mid - glam::Vec3::new(0.3, 0.2, 0.0)).length() < 1e-4);
    }

    #[test]
    fn single_crossing_falls_back_to_entry_point() {
        let (positions, indices) = two_parallel_quads();
        // First six indices = the front quad only.
        let mid = ray_mesh_midpoint(&ray_down_z(), &positions, &indices[..6]).unwrap();
        assert!((mid - glam::Vec3::new(0.3, 0.2, 1.0)).length() < 1e-4);
    }

    #[test]
    fn miss_returns_none() {
        let (positions, indices) = two_parallel_quads();
        let ray = Ray {
            origin: glam::Vec3::new(5.0, 5.0, 5.0),
            direction: glam::Vec3::new(0.0, 0.0, -1.0),
        };
        assert!(ray_mesh_midpoint(&ray, &positions, &indices).is_none());
    }
}