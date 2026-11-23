struct SceneUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    base_color: vec4<f32>,
    material: vec4<f32>,
    camera_pos: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> scene: SceneUniform;

@group(1) @binding(0)
var<storage, read> bone_matrices: array<mat4x4<f32>>;

// The same bones as dual quaternions, for Preserve volume: for every bone a real
// quaternion, then a dual quaternion, each as (x, y, z, w).
@group(1) @binding(1)
var<storage, read> bone_dual_quats: array<vec4<f32>>;

struct GpuLight {
    position: vec4<f32>,
    direction: vec4<f32>,
    color: vec4<f32>,
};

struct LightsUniform {
    count: vec4<u32>,
    items: array<GpuLight, 8>,
};
@group(2) @binding(0)
var<uniform> lights: LightsUniform;

// The world: x is how strong the soft studio light on surfaces is, y how
// strong the studio reflections are.
@group(3) @binding(0)
var<uniform> world: vec4<f32>;
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) joint_indices: vec4<u32>,
    @location(3) weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) bone_weight: f32,
};


struct DualQuatSkin {
    position: vec3<f32>,
    normal: vec3<f32>,
};

// Turns `v` by the unit quaternion `q`, held as (x, y, z, w).
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// Dual quaternion skinning: the bones' dual quaternions are blended by weight
// instead of their matrices, so a bent or twisted limb keeps its thickness.
//
// Ported from Blender (SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All
// rights reserved., SPDX-License-Identifier: GPL-2.0-or-later): `add_weighted_dq_dq`,
// `normalize_dq` and `mul_v3m3_dq` in BLI math_rotation, and the rule of
// `armature_deform` that a vertex with a total weight under 0.0001 is not moved.
// The same blend is in dual_quat.rs, which has the tests.
fn dual_quat_skin(
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
    normal: vec3<f32>,
) -> DualQuatSkin {
    var real = vec4<f32>(0.0);
    var dual = vec4<f32>(0.0);
    var total = 0.0;
    for (var slot = 0u; slot < 4u; slot = slot + 1u) {
        let weight = weights[slot];
        if (weight <= 0.0) {
            continue;
        }
        let bone = joints[slot];
        var r = bone_dual_quats[bone * 2u];
        var d = bone_dual_quats[bone * 2u + 1u];
        // Turn a quaternion that points away from what is added so far round.
        if (dot(r, real) < 0.0) {
            r = -r;
            d = -d;
        }
        real = real + r * weight;
        dual = dual + d * weight;
        total = total + weight;
    }

    var skin: DualQuatSkin;
    skin.position = position;
    skin.normal = normal;
    let length_squared = dot(real, real);
    if (total < 0.0001 || length_squared < 1e-12) {
        return skin;
    }
    let rotation = real / sqrt(length_squared);
    let translation = 2.0
        * (dual.xyz * real.w - real.xyz * dual.w - cross(dual.xyz, real.xyz))
        / length_squared;
    skin.position = quat_rotate(rotation, position) + translation;
    skin.normal = quat_rotate(rotation, normal);
    return skin;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let skin_matrix =
        bone_matrices[in.joint_indices.x] * in.weights.x +
        bone_matrices[in.joint_indices.y] * in.weights.y +
        bone_matrices[in.joint_indices.z] * in.weights.z +
        bone_matrices[in.joint_indices.w] * in.weights.w;

    var skinned_position = skin_matrix * vec4<f32>(in.position, 1.0);
        var skinned_normal = skin_matrix * vec4<f32>(in.normal, 0.0);

    // With Preserve volume on, the bones' dual quaternions are blended instead
    // of their matrices.
    if (scene.material.w > 0.5) {
        let skin = dual_quat_skin(in.joint_indices, in.weights, in.position, in.normal);
        skinned_position = vec4<f32>(skin.position, 1.0);
        skinned_normal = vec4<f32>(skin.normal, 0.0);
    }

out.clip_position = scene.view_proj * scene.model * skinned_position;
    out.world_normal = (scene.normal_matrix * skinned_normal).xyz;
    out.world_position = (scene.model * skinned_position).xyz;
    // How much of the bone being shown (if any) moves this vertex: the sum of
    // the weights of the slots that name that bone.
    let shown = max(i32(round(scene.material.z)) - 1, 0);
    let named = in.joint_indices == vec4<u32>(u32(shown));
    out.bone_weight = dot(select(vec4<f32>(0.0), vec4<f32>(1.0), named), in.weights);
    return out;
}


const PI: f32 = 3.14159265;

// How much of the microscopic surface faces the halfway direction: tight
// for smooth surfaces, broad for rough ones (the GGX curve).
fn distribution_ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// How much the surface's own bumps block the light (Smith with Schlick).
fn geometry_schlick(n_dot_x: f32, k: f32) -> f32 {
    return n_dot_x / (n_dot_x * (1.0 - k) + k);
}

// How much light a surface reflects instead of scattering: little head-on
// for non-metals, almost all of it at grazing angles.
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - cos_theta, 5.0);
}

// One soft light panel as seen in a direction: 1 inside it, 0 outside, with
// a soft edge.
fn softbox(dir: vec3<f32>, centre: vec3<f32>, size: f32, softness: f32) -> f32 {
    let c = dot(dir, normalize(centre));
    let edge = cos(size);
    return smoothstep(edge - softness, edge + softness, c);
}

// A made-up photo studio to reflect: a soft sky above, a dark floor below,
// a key light panel and a rim light panel. Rougher surfaces see the panels
// as wider, dimmer glows.
fn studio_environment(dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    let horizon = vec3<f32>(0.62, 0.64, 0.68);
    let zenith = vec3<f32>(0.95, 0.97, 1.0);
    let ground = vec3<f32>(0.12, 0.12, 0.13);
    var colour = mix(horizon, zenith, smoothstep(0.0, 0.9, dir.y));
    colour = mix(colour, ground, smoothstep(0.0, 0.45, -dir.y));

    let softness = mix(0.03, 0.6, roughness * roughness);
    let dimming = 1.0 / (1.0 + 5.0 * softness);
    colour += softbox(dir, vec3<f32>(-0.55, 0.7, 0.45), 0.32, softness)
        * 3.0 * dimming * vec3<f32>(1.0, 0.98, 0.95);
    colour += softbox(dir, vec3<f32>(0.75, 0.35, -0.6), 0.25, softness)
        * 2.0 * dimming * vec3<f32>(0.85, 0.92, 1.0);
    return colour;
}

// What one light does to the surface: a soft diffuse part (rougher
// surfaces soften the edge of the shading) and a highlight. `l` points
// from the surface towards the light, `radiance` is the light's colour
// times its strength where it arrives.
fn direct_light(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    radiance: vec3<f32>,
    albedo: vec3<f32>,
    roughness: f32,
    metalness: f32,
    f0: vec3<f32>,
) -> vec3<f32> {
    let h = normalize(l + v);
    let n_dot_v = max(dot(n, v), 0.0001);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);

    let fresnel = fresnel_schlick(v_dot_h, f0);

    let wrap = roughness * 0.5;
    let wrapped = max((dot(n, l) + wrap) / (1.0 + wrap), 0.0);
    let k_diffuse = (vec3<f32>(1.0) - fresnel) * (1.0 - metalness);
    let diffuse_direct = albedo * k_diffuse * wrapped * radiance;

    let a = roughness * roughness;
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let geometry = geometry_schlick(n_dot_v, k) * geometry_schlick(n_dot_l, k);
    let specular_direct = distribution_ggx(n_dot_h, a) * geometry * fresnel * PI
        / (4.0 * n_dot_v) * radiance;

    return diffuse_direct + specular_direct;
}

// Squeezes bright values towards 1 instead of cutting them off, so
// several lights add up without turning flat white. Everything below the
// knee is left exactly as it was.
fn soft_clip(c: vec3<f32>) -> vec3<f32> {
    let knee = 0.8;
    let over = max(c - vec3<f32>(knee), vec3<f32>(0.0));
    let shoulder = vec3<f32>(knee)
        + (1.0 - knee) * (vec3<f32>(1.0) - exp(-over / (1.0 - knee)));
    return select(shoulder, c, c <= vec3<f32>(knee));
}

// The weight colour ramp: blue at 0, then cyan, green, yellow, and red at 1.
//
// Ported from Blender (SPDX-FileCopyrightText: 2001-2002 NaN Holding BV. All
// rights reserved., SPDX-License-Identifier: GPL-2.0-or-later):
// `BKE_defvert_weight_to_rgb` in deform.cc, the ramp the weight paint overlay
// uses by default.
fn weight_to_rgb(weight: f32) -> vec3<f32> {
    let blend = weight / 2.0 + 0.5;
    if (weight <= 0.25) {
        return vec3<f32>(0.0, blend * weight * 4.0, blend);
    }
    if (weight <= 0.5) {
        return vec3<f32>(0.0, blend, blend * (1.0 - (weight - 0.25) * 4.0));
    }
    if (weight <= 0.75) {
        return vec3<f32>(blend * (weight - 0.5) * 4.0, blend, 0.0);
    }
    return vec3<f32>(blend, blend * (1.0 - (weight - 0.75) * 4.0), 0.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(scene.camera_pos.xyz - in.world_position);

    // The weight view: the weights of one bone as colours, shaded only enough
    // (a light from the camera) to show the shape of the surface. As in
    // Blender's weight overlay, the colour is never allowed to go fully black.
    if (scene.material.z > 0.5) {
        let shade = abs(dot(n, v)) * 0.9 + 0.1;
        let ramp = weight_to_rgb(clamp(in.bone_weight, 0.0, 1.0));
        return vec4<f32>(max(ramp, vec3<f32>(0.005)) * shade, 1.0);
    }

    let albedo = scene.base_color.rgb;
    let roughness = clamp(scene.material.x, 0.05, 1.0);
    let metalness = clamp(scene.material.y, 0.0, 1.0);

    let n_dot_v = max(dot(n, v), 0.0001);

    // Non-metals reflect about 4 percent head-on; metals reflect their colour.
    let f0 = mix(vec3<f32>(0.04), albedo, metalness);

    // With no lights in the scene, the light that follows the camera shines.
    // Once there are lights, they replace it.
    var direct = vec3<f32>(0.0);
    if (lights.count.x == 0u) {
        let l = normalize(-scene.light_dir.xyz);
        direct = direct_light(n, v, l, scene.light_color.rgb, albedo, roughness, metalness, f0);
    } else {
        for (var i = 0u; i < lights.count.x; i = i + 1u) {
            let light = lights.items[i];
            var l = normalize(-light.direction.xyz);
            var radiance = light.color.rgb;
            // A point light shines from where it sits and fades with the
            // square of the distance (held above 0.1, as in the scene code).
            if (light.position.w > 0.5) {
                let to_light = light.position.xyz - in.world_position;
                let dist = max(length(to_light), 0.1);
                l = to_light / dist;
                radiance = radiance / (dist * dist);
            }
            direct = direct + direct_light(n, v, l, radiance, albedo, roughness, metalness, f0);
        }
    }

    // The studio around the object: soft light on the surface, and its
    // reflection, which is what makes metal look like metal.
    let sky_amount = n.y * 0.5 + 0.5;
    let ambient_light = mix(vec3<f32>(0.20, 0.20, 0.22), vec3<f32>(0.55, 0.57, 0.62), sky_amount);
    let diffuse_ambient = albedo * (1.0 - metalness) * ambient_light * 0.6 * world.x;
    let reflected = reflect(-v, n);
    let environment = studio_environment(reflected, roughness);
    let f_environment = f0
        + (max(vec3<f32>(1.0 - roughness), f0) - f0) * pow(1.0 - n_dot_v, 5.0);
    let specular_ambient = environment * f_environment * world.y;
    
    let lit_color = soft_clip(direct + diffuse_ambient + specular_ambient);
    return vec4<f32>(lit_color, 1.0);
}