struct Camera {
    pos: vec3f,
    fov_y: f32,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

@group(0) @binding(0)
var<uniform> cam: Camera;

const dim = vec3(256, 256, 256);

struct Voxels {
    data: array<array<array<u32, dim.z>, dim.y>, dim.x>,
}

@group(1) @binding(0)
var<storage, read> voxels: Voxels;

@group(1) @binding(1)
var<storage, read> palette: array<vec4f>;

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
    let diffuse_color = albedo.rgb;
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
    return vec4f(shading_color, 1.0);
}

struct CastResult {
    hit: bool,
    hit_voxel: vec3i,
    hit_pos: vec3f,
}

fn out_of_bounds_i(pos: vec3i) -> bool {
    return any(pos < vec3(0) | pos >= dim);
}

fn sample_voxel(ipos: vec3i) -> u32 {
    if out_of_bounds_i(ipos) {
        return 0u;
    } else {
        return voxels.data[ipos.x][ipos.y][ipos.z];
    }
}

// taken from https://www.shadertoy.com/view/4dX3zl
// copyright Lode Vandevenne (https://lodev.org/cgtutor/raycasting.html)
// copyright user "fb39ca4" on shadertoy (https://www.shadertoy.com/view/4dX3zl)
// licensed under cc-by-nc-sa
fn raycast_voxels(ray_pos: vec3f, ray_dir: vec3f, steps: i32) -> CastResult {
    var ipos = vec3i(floor(ray_pos)); // which voxel we're in
    let boundary_dt = abs(1.0 / ray_dir); // time to traverse 1 voxel in each x,y,z
    var next_side_dt = (sign(ray_dir) * (0.5 - fract(ray_pos)) + 0.5) * boundary_dt; // time to reach the next voxel boundary in each x,y,z
    let ray_sign = vec3i(sign(ray_dir));
    var i = 0;
    var t = 0.0;

    var res = CastResult(false, vec3i(0), vec3f(0.0));

    for (; i < steps; i++) {
        // // early return
        // if out_of_bounds_i(ipos) {
        //     return res;
        // }

        let voxel = sample_voxel(ipos);

        if voxel != 0u {
            res.hit = true;
            res.hit_voxel = ipos;
            res.hit_pos = ray_pos + t * ray_dir;
            return res;
        }
        
        let mask = next_side_dt.xyz <= min(next_side_dt.yzx, next_side_dt.zxy); // which boundary axis is the closest
        t = min(min(next_side_dt.x, next_side_dt.y), next_side_dt.z);
        next_side_dt += vec3f(mask) * boundary_dt; // increment the next boundary for the selected axis by 1 voxel.
        ipos += vec3i(mask) * ray_sign; // advance by 1 voxel along the selected axis
    }

    return res;
}

// is pos is on a cube surface, returns the normal of the corresponding cube face.
fn cube_face_normal(ipos: vec3i, pos: vec3f) -> vec3f {
    let off = pos - vec3f(ipos) - 0.5; // value between [-0.5, 0.5]
    let dist = abs(off);
    let max_dist = max(max(dist.x, dist.y), dist.z);
    return sign(off) * vec3f(dist == vec3f(max_dist));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dir = (cam.view_mat_inv * normalize(vec4f(
        in.pos.x * tan(cam.fov_y / 2.0) * cam.aspect,
        in.pos.y * tan(cam.fov_y / 2.0),
        1.0,
        0.0,
    ))).xyz;

    let res = raycast_voxels(cam.pos, dir, 2000);

    if res.hit {
        let normal = cube_face_normal(res.hit_voxel, res.hit_pos);
        let palette_index = sample_voxel(res.hit_voxel) - 1u;
        let albedo = palette[palette_index];
        return shade(albedo, cam.pos, res.hit_pos, normal);
    } else {
        // return vec4f(0.0, 0.0, 0.0, 1.0);
        return palette[0];
    }
}
