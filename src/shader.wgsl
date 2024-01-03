struct Camera {
    pos: vec3f,
    fov_y: f32,
    view_mat_inv: mat4x4f,
};

@group(0) @binding(0)
var<uniform> cam: Camera;

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

    out.pos = vert.position;
    out.clip_position = vec4f(out.pos, 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dir = cam.view_mat_inv * normalize(vec4f(
        in.pos.xy * tan(cam.fov_y / 2.0),
        1.0,
        0.0,
    ));

    let t = -cam.pos.z / dir.z;

    let hit = cam.pos.xy + t * dir.xy;

    if all(-1.0 <= hit & hit <= 1.0) {
        return vec4f(1.0);
    } else {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }
}
