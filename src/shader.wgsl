struct Camera {
    mvp: mat4x4f,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) pos: vec2f,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    vert: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    var verts = array(
        vec2f(-1.0, -1.0) * vec2f(0.95),
        vec2f( 1.0, -1.0) * vec2f(0.95),
        vec2f( 1.0,  1.0) * vec2f(0.95),

        vec2f(-1.0, -1.0) * vec2f(0.95),
        vec2f( 1.0,  1.0) * vec2f(0.95),
        vec2f(-1.0,  1.0) * vec2f(0.95),
    );

    out.pos = verts[index];
    out.pos = vert.position;
    out.clip_position = camera.mvp * vec4f(out.pos, 0.0, 1.0);
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(in.pos * 0.5 + 0.5, 0.0, 1.0);
    // return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}
