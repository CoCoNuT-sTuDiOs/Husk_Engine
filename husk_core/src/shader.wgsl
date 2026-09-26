struct SceneUniform {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    base_color: vec4<f32>,
    material: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> scene: SceneUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = scene.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = (scene.model * vec4<f32>(in.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let l = normalize(-scene.light_dir.xyz);

    let roughness = scene.material.x;
    let metalness = scene.material.y;

    let diffuse_strength = max(dot(n, l), 0.0);
    let ambient = 0.1;
    let diffuse = diffuse_strength * (1.0 - metalness);

    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = normalize(l + view_dir);
    let spec_power = mix(8.0, 128.0, 1.0 - roughness);
    let specular_strength =
        pow(max(dot(n, half_dir), 0.0), spec_power) * mix(0.1, 1.0, metalness);

    let lit_color = scene.base_color.rgb * (ambient + diffuse) * scene.light_color.rgb
        + scene.light_color.rgb * specular_strength;

    return vec4<f32>(lit_color, 1.0);
}