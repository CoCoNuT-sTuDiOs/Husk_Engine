struct BackgroundUniform {
    top: vec4<f32>,
    bottom: vec4<f32>,
    // x: the pixel row where the top colour is. y: how many pixel rows it
    // takes to reach the bottom colour.
    span: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> background: BackgroundUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

// One triangle that covers the whole screen.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(corners[index], 0.0, 1.0);
    return out;
}

// A different tiny number for every pixel, between 0 and 1.
fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let rows = max(background.span.y, 1.0);
    let t = clamp((position.y - background.span.x) / rows, 0.0, 1.0);
    let colour = mix(background.top.rgb, background.bottom.rgb, t);
    // A very slight grain, under one step of 8-bit colour, so a smooth
    // gradient does not show bands.
    let grain = (hash(position.xy) - 0.5) / 255.0;
    return vec4<f32>(colour + vec3<f32>(grain), 1.0);
}