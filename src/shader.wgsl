@group(0) @binding(0)
var<uniform> cam: Camera;

@group(1) @binding(0)
var<storage, read> dvo: array<u32>;

@group(1) @binding(1)
var<storage, read> palette: array<vec4f>;

// preproc_include(octree.wgsl)

struct Camera {
    pos: vec3f,
    fov_y: f32,
    size: vec2f,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

struct VertexInput {
    @location(0) position: vec2f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) pos: vec2f,
}

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

fn shade(albedo: vec4f, view_pos: vec3f, hit_pos: vec3f, hit_normal: vec3f) -> vec4f {
    // return vec4f(pos / 16.0 * 0.9 + 0.1, 1.0);

    let ambient_color = vec3f(1.0, 1.0, 1.0) * 0.0;
    let diffuse_color = pow(albedo.rgb, vec3f(2.2));
    let specular_color = vec3f(1.0, 1.0, 1.0) * 0.1;
    let shininess = 16.0;

    let view_dir = normalize(view_pos - hit_pos);
    let light_dir = view_dir;
    let light_dist = length(view_pos - hit_pos);
    let half_vector = normalize(light_dir + view_dir);
    // var sun_light_dir = normalize(vec3f(1.0, -1.0, 1.0));

    let ambient_term = ambient_color;
    let diffuse_term = max(dot(hit_normal, light_dir), 0.0) * diffuse_color;
    let specular_term = pow(max(dot(hit_normal, half_vector), 0.0), shininess) * specular_color;

    let shading_color = ambient_term + diffuse_term + specular_term;
    return vec4f(saturate(shading_color), 1.0);
}

// is pos is on a cube surface, returns the normal of the corresponding cube face.
fn cube_face_normal(ipos: vec3i, pos: vec3f) -> vec3f {
    let off = pos - vec3f(ipos) - 0.5; // value between [-0.5, 0.5]
    let dist = abs(off);
    let max_dist = max(max(dist.x, dist.y), dist.z);
    return sign(off) * vec3f(dist == vec3f(max_dist));
}

fn cam_ray_dir(pos: vec2f) -> vec3f {
    return (cam.view_mat_inv * normalize(vec4f(
        pos.x * tan(cam.fov_y / 2.0) * cam.aspect,
        pos.y * tan(cam.fov_y / 2.0),
        1.0,
        0.0,
    ))).xyz;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let ray_dir = cam_ray_dir(in.pos);

    let res = raycast_dvo(cam.pos, ray_dir);

    if res.hit != 0u {
        let albedo = palette[res.hit - 1u];
        var col = shade(albedo, cam.pos, res.hit_pos, res.hit_normal);
        // return col;

        let msaa_level = 1;

        // MSAA
        for (var i = 0; i < msaa_level * 2; i++) {
            for (var j = 0; j < msaa_level * 2; j++) {
                let pos = (2.0 * (vec2f(f32(i), f32(j)) - f32(msaa_level)) - 1.0) / (4.0 * f32(msaa_level * msaa_level) - 1.0);
                let jitter = pos / cam.size;
                let ray_dir = cam_ray_dir(in.pos + jitter);
                let res = raycast_dvo(cam.pos, ray_dir);
                let albedo = palette[res.hit - 1u];
                col += shade(albedo, cam.pos, res.hit_pos, res.hit_normal);
            }
        }

        return col / (1.0 + f32(msaa_level * msaa_level * 4));
    }

    else {
        var col = res.hit_pos;
        // col.b += f32(res.iter) / 100.0;
        // col.g += f32(res.iter) / 200.0;
        return vec4f(col, 1.0);
    }
}
