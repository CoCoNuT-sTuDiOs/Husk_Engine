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

    /// The same ray seen from inside a placed object: its origin and
    /// direction moved into the object's own space by the inverse of
    /// `model`. The direction is NOT re-normalized, so a distance t along
    /// this ray is the same t along the original, and hits on different
    /// objects stay comparable.
    pub fn to_local(&self, model: glam::Mat4) -> Ray {
        let inverse = model.inverse();
        Ray {
            origin: inverse.transform_point3(self.origin),
            direction: inverse.transform_vector3(self.direction),
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

pub fn pick_nearest(
    ray: &Ray,
    placed: &[(glam::Mat4, &[[f32; 3]], &[u32])],
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (index, (model, positions, indices)) in placed.iter().enumerate() {
        // Moving the ray into the mesh's own space keeps t the same along
        // it, so hits on different meshes can be compared directly.
        let local = ray.to_local(*model);
        if let Some(hit) = ray_mesh_intersection(&local, positions, indices) {
            if best.map_or(true, |(_, nearest)| hit.distance < nearest) {
                best = Some((index, hit.distance));
            }
        }
    }
    best.map(|(index, _)| index)
}


/// Projects a scene point to screen pixels (origin at the top left) for a
/// viewport of `width` x `height` pixels. None if the point is behind the
/// camera.
pub fn project_to_screen(
    view_proj: glam::Mat4,
    width: f32,
    height: f32,
    point: glam::Vec3,
) -> Option<(f32, f32)> {
    let clip = view_proj * point.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc_x = clip.x / clip.w;
    let ndc_y = clip.y / clip.w;
    Some((
        (ndc_x * 0.5 + 0.5) * width,
        (0.5 - ndc_y * 0.5) * height,
    ))
}

/// Where along an axis line the ray passes closest to it: the t for which
/// `axis_origin + axis_dir * t` (with `axis_dir` as a unit vector) is the
/// point of the axis nearest the ray. Dragging a gizmo arrow uses the
/// difference between two of these, so the object follows the cursor along
/// the axis. None when the ray runs parallel to the axis (there is no
/// single nearest point) or the axis has no direction.
pub fn closest_param_on_axis(
    ray: &Ray,
    axis_origin: glam::Vec3,
    axis_dir: glam::Vec3,
) -> Option<f32> {
    let axis = axis_dir.normalize_or_zero();
    if axis == glam::Vec3::ZERO {
        return None;
    }
    let to_axis = axis_origin - ray.origin;
    let b = axis.dot(ray.direction);
    let c = ray.direction.dot(ray.direction);
    let d = axis.dot(to_axis);
    let e = ray.direction.dot(to_axis);
    // axis . axis is 1, so the usual denominator a*c - b*b is c - b*b.
    let denominator = c - b * b;
    if denominator <= 1e-6 * c {
        return None;
    }
    Some((b * e - c * d) / denominator)
}

/// Where to draw one gizmo axis: the screen position of `origin`, and of an
/// arrow tip exactly `pixels` away from it, in the direction the axis
/// points on screen. None if the origin is behind the camera. An axis
/// pointing straight at the camera has no direction on screen, so its tip
/// lands on the origin.
pub fn axis_screen_ends(
    view_proj: glam::Mat4,
    width: f32,
    height: f32,
    origin: glam::Vec3,
    axis_dir: glam::Vec3,
    pixels: f32,
) -> Option<((f32, f32), (f32, f32))> {
    let start = project_to_screen(view_proj, width, height, origin)?;
    let step = origin + axis_dir.normalize_or_zero() * 0.01;
    let toward = project_to_screen(view_proj, width, height, step).unwrap_or(start);
    let (dx, dy) = (toward.0 - start.0, toward.1 - start.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1e-3 {
        return Some((start, start));
    }
    Some((
        start,
        (start.0 + dx / length * pixels, start.1 + dy / length * pixels),
    ))
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

    fn one_quad() -> (Vec<[f32; 3]>, Vec<u32>) {
        let (positions, indices) = two_parallel_quads();
        (positions, indices[..6].to_vec())
    }


    fn test_camera() -> glam::Mat4 {
        let projection =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let view = glam::Mat4::look_at_rh(
            glam::Vec3::new(0.0, 0.0, 5.0),
            glam::Vec3::ZERO,
            glam::Vec3::Y,
        );
        projection * view
    }

    #[test]
    fn closest_param_on_axis_follows_the_cursor_along_the_axis() {
        // A ray coming straight down from above x = 2.
        let down = Ray {
            origin: glam::Vec3::new(2.0, 5.0, 0.0),
            direction: glam::Vec3::new(0.0, -1.0, 0.0),
        };
        let t = closest_param_on_axis(&down, glam::Vec3::ZERO, glam::Vec3::X).unwrap();
        assert!((t - 2.0).abs() < 1e-5);

        // Against an axis that starts at x = 1, the same ray is 1 along it.
        let t = closest_param_on_axis(&down, glam::Vec3::new(1.0, 0.0, 0.0), glam::Vec3::X)
            .unwrap();
        assert!((t - 1.0).abs() < 1e-5);

        // Height and depth off the axis do not matter, only where along it.
        let deep = Ray {
            origin: glam::Vec3::new(3.0, 1.0, 4.0),
            direction: glam::Vec3::new(0.0, 0.0, -1.0),
        };
        let t = closest_param_on_axis(&deep, glam::Vec3::ZERO, glam::Vec3::X).unwrap();
        assert!((t - 3.0).abs() < 1e-5);

        // A slanted ray that crosses the axis at x = 5.
        let slanted = Ray {
            origin: glam::Vec3::new(0.0, 0.0, 5.0),
            direction: glam::Vec3::new(1.0, 0.0, -1.0).normalize(),
        };
        let t = closest_param_on_axis(&slanted, glam::Vec3::ZERO, glam::Vec3::X).unwrap();
        assert!((t - 5.0).abs() < 1e-4);

        // The axis direction need not be a unit vector: t is still measured
        // in units of the normalised direction.
        let t = closest_param_on_axis(&down, glam::Vec3::ZERO, glam::Vec3::X * 4.0).unwrap();
        assert!((t - 2.0).abs() < 1e-5);
    }

    #[test]
    fn closest_param_on_axis_has_no_answer_for_parallel_rays_or_a_zero_axis() {
        let along_x = Ray {
            origin: glam::Vec3::new(0.0, 1.0, 0.0),
            direction: glam::Vec3::X,
        };
        assert!(closest_param_on_axis(&along_x, glam::Vec3::ZERO, glam::Vec3::X).is_none());
        assert!(closest_param_on_axis(&ray_down_z(), glam::Vec3::ZERO, glam::Vec3::ZERO)
            .is_none());
    }

    #[test]
    fn project_to_screen_puts_the_centre_in_the_middle_and_hides_what_is_behind() {
        let vp = test_camera();
        let (x, y) = project_to_screen(vp, 800.0, 800.0, glam::Vec3::ZERO).unwrap();
        assert!((x - 400.0).abs() < 1e-3 && (y - 400.0).abs() < 1e-3);
        // Up in the scene is up on the screen, which is a smaller y.
        let (_, above) = project_to_screen(vp, 800.0, 800.0, glam::Vec3::Y).unwrap();
        assert!(above < 400.0);
        let behind = glam::Vec3::new(0.0, 0.0, 10.0);
        assert!(project_to_screen(vp, 800.0, 800.0, behind).is_none());
    }

    #[test]
    fn axis_ends_are_drawn_the_asked_number_of_pixels_long() {
        let vp = test_camera();
        let origin = glam::Vec3::ZERO;

        let (start, x_tip) =
            axis_screen_ends(vp, 800.0, 800.0, origin, glam::Vec3::X, 100.0).unwrap();
        assert!((start.0 - 400.0).abs() < 1e-3);
        assert!((x_tip.0 - 500.0).abs() < 1e-2 && (x_tip.1 - 400.0).abs() < 1e-2);

        let (_, y_tip) =
            axis_screen_ends(vp, 800.0, 800.0, origin, glam::Vec3::Y, 100.0).unwrap();
        assert!((y_tip.0 - 400.0).abs() < 1e-2 && (y_tip.1 - 300.0).abs() < 1e-2);

        // An axis pointing straight at the camera has no direction on screen.
        let (start, z_tip) =
            axis_screen_ends(vp, 800.0, 800.0, origin, glam::Vec3::Z, 100.0).unwrap();
        assert_eq!(start, z_tip);

        let behind = glam::Vec3::new(0.0, 0.0, 10.0);
        assert!(axis_screen_ends(vp, 800.0, 800.0, behind, glam::Vec3::X, 100.0).is_none());
    }

    #[test]
    fn to_local_keeps_distances_along_the_ray() {
    let model = glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -3.0))
            * glam::Mat4::from_scale(glam::Vec3::splat(2.0));
        let ray = ray_down_z();
        let local = ray.to_local(model);
        let t = 2.5;
        let world_point = ray.origin + ray.direction * t;
        let back = model.transform_point3(local.origin + local.direction * t);
        assert!((back - world_point).length() < 1e-4);
    }

    #[test]
    fn pick_nearest_chooses_the_closer_placed_mesh() {
    let (positions, indices) = one_quad();
        // The quad sits at z = 1 as built; moved by -3 it sits at z = -2.
        let at_z1 = glam::Mat4::IDENTITY;
        let at_z_minus2 = glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -3.0));
        let placed = [
            (at_z1, positions.as_slice(), indices.as_slice()),
            (at_z_minus2, positions.as_slice(), indices.as_slice()),
        ];
        assert_eq!(pick_nearest(&ray_down_z(), &placed), Some(0));
        // The same two meshes in the other order: the closer one is now entry 1.
        let swapped = [placed[1], placed[0]];
        assert_eq!(pick_nearest(&ray_down_z(), &swapped), Some(1));
    }

    #[test]
    fn pick_nearest_respects_scale_and_translation() {
        let (positions, indices) = one_quad();
        let behind = glam::Mat4::IDENTITY; // quad at z = 1
        // Scaled to half size and moved to z = 2 + 0.5 = 2.5: closer to the
        // ray's start at z = 5, and still wide enough to catch (0.3, 0.2).
        let in_front = glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 2.0))
            * glam::Mat4::from_scale(glam::Vec3::splat(0.5));
        let placed = [
            (behind, positions.as_slice(), indices.as_slice()),
            (in_front, positions.as_slice(), indices.as_slice()),
        ];
        assert_eq!(pick_nearest(&ray_down_z(), &placed), Some(1));
    }

    #[test]
    fn pick_nearest_returns_none_when_every_mesh_is_missed() {
        let (positions, indices) = one_quad();
        let far_left = glam::Mat4::from_translation(glam::Vec3::new(-10.0, 0.0, 0.0));
        let placed = [(far_left, positions.as_slice(), indices.as_slice())];
        assert!(pick_nearest(&ray_down_z(), &placed).is_none());
        assert!(pick_nearest(&ray_down_z(), &[]).is_none());
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