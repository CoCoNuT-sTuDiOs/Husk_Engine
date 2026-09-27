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