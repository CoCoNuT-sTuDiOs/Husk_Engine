struct GridUniform {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    // x: how far the camera is from what it looks at.
    params: vec4<f32>,
    // The colour of the plain grid lines (red, green, blue).
    line_color: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> grid: GridUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

// One triangle that covers the whole screen.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let corner = corners[index];
    var out: VertexOutput;
    out.clip_position = vec4<f32>(corner, 0.0, 1.0);
    out.ndc = corner;
    return out;
}

// How much of a pixel the lines of a grid with cells `spacing` wide cover
// at ground position `p`: 1 on a line, 0 between lines. Lines stay about
// one pixel wide whatever the distance, and fade out where the cells get
// smaller than a few pixels (which would only shimmer).
fn grid_level(p: vec2<f32>, spacing: f32) -> f32 {
    let cell = p / spacing;
    let step = fwidth(cell);
    let distance_to_line = abs(fract(cell - 0.5) - 0.5) / max(step, vec2<f32>(0.00001));
    let line = 1.0 - min(min(distance_to_line.x, distance_to_line.y), 1.0);
    let too_small = 1.0 - smoothstep(0.2, 0.5, max(step.x, step.y));
    return line * too_small;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // The ray through this pixel, from the near plane to the far plane.
    let near_h = grid.inverse_view_proj * vec4<f32>(in.ndc, 0.0, 1.0);
    let far_h = grid.inverse_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let near = near_h.xyz / near_h.w;
    let far = far_h.xyz / far_h.w;
    let direction = far - near;

    // Where it meets the ground plane, y = 0.
    let parallel = abs(direction.y) < 0.000001;
    let safe_y = select(direction.y, 0.000001, parallel);
    let t = -near.y / safe_y;
    let hit = near + direction * t;
    let p = hit.xz;

    // Three grids, ten times apart, that change with how far the camera is:
    // the finest fades out and the coarsest fades in as the camera moves
    // away, so the spacing steps smoothly and never pops.
    let decade = log2(max(grid.params.x, 0.01)) / log2(10.0);
    let level = floor(decade);
    let blend = decade - level;
    let spacing = pow(10.0, level);
    let fine = grid_level(p, spacing * 0.1) * 0.18 * (1.0 - blend);
    let medium = grid_level(p, spacing) * mix(0.35, 0.18, blend);
    let coarse = grid_level(p, spacing * 10.0) * 0.35 * blend;
    let lines = max(fine, max(medium, coarse));

    // The two axes through the origin: X in red, Z in blue.
    let width = fwidth(p);
    let x_axis = 1.0 - clamp(abs(p.y) / max(width.y * 1.5, 0.00001), 0.0, 1.0);
    let z_axis = 1.0 - clamp(abs(p.x) / max(width.x * 1.5, 0.00001), 0.0, 1.0);
    let axis = max(x_axis, z_axis);
    let axis_colour = select(
        vec3<f32>(0.30, 0.45, 0.90),
        vec3<f32>(0.90, 0.30, 0.30),
        x_axis >= z_axis,
    );

    // Fading out towards the horizon.
    let distance_to_camera = length(hit - grid.camera_pos.xyz);
    let fade = 1.0 - smoothstep(0.25, 1.0, distance_to_camera / (grid.params.x * 12.0));

    let colour = mix(grid.line_color.rgb, axis_colour, axis);
    let alpha = max(lines, axis * 0.8) * fade;

    // The depth of the ground point, so objects hide the grid behind them
    // and the grid shows where it passes through them.
    let clip = grid.view_proj * vec4<f32>(hit, 1.0);
    let depth = clip.z / clip.w;
    let valid = !parallel && t >= 0.0 && clip.w > 0.0 && depth >= 0.0 && depth <= 1.0;
    if (!valid) {
        discard;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(colour, alpha);
    out.depth = depth;
    return out;
}