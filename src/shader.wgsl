// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) pos: vec2f,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
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

    out.pos = verts[in_vertex_index];
    out.clip_position = vec4<f32>(out.pos, 0.0, 1.0);
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(in.pos * 0.5 + 0.5, 0.0, 1.0);
    // return vec4<f32>(0.3, 0.2, 0.1, 1.0);
}
