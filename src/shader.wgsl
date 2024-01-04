struct Camera {
    pos: vec3f,
    fov_y: f32,
    view_mat_inv: mat4x4f,
}

@group(0) @binding(0)
var<uniform> cam: Camera;

struct Blocks {
    data: array<array<array<u32, 16>, 16>, 16>,
}

@group(1) @binding(0)
var<storage, read> blocks: Blocks;

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

fn shade(pos: vec3f, normal: vec3f) -> vec4f {
    return vec4f(1.0);
}

struct CastResult {
    hit: bool,
    hit_voxel: vec3i,
    hit_face: vec3f,
}

fn out_of_bounds_i(pos: vec3i) -> bool {
    return any(pos < 0 | pos >= 16);
}

fn sample_block(ipos: vec3i) -> u32 {
    if out_of_bounds_i(ipos) {
        return 0u;
    } else {
        return blocks.data[ipos.x][ipos.y][ipos.z];
    }
}

// taken from https://www.shadertoy.com/view/4dX3zl
// copyright Lode Vandevenne (https://lodev.org/cgtutor/raycasting.html)
// copyright user "fb39ca4" on shadertoy (https://www.shadertoy.com/view/4dX3zl)
// licensed under cc-by-nc-sa
fn raycast_voxels(ray_pos: vec3f, ray_dir: vec3f, steps: i32) -> CastResult {
    var ipos = vec3i(ray_pos); // which voxel we're in
    let delta_dist = abs(1.0 / ray_dir);
    let ray_step = vec3i(sign(ray_dir));
    var side_dist = (sign(ray_dir) * (vec3f(ipos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist; // distance to the next voxel boundary in each x,y,z
    var i = 0;

    var res = CastResult(false, vec3i(0), vec3f(0.0));

    // raycast forward until we meet empty space
    // this allows to see though the volume when the camera is inside
    for (; i < steps; i++) {
        // if out_of_bounds_i(ipos) {
        //     return res;
        // }

        let block = sample_block(ipos);

        if (block == 0u) {
            break;
        }
        
        let mask = side_dist.xyz <= min(side_dist.yzx, side_dist.zxy);
        side_dist += vec3f(mask) * delta_dist;
        ipos += vec3i(mask) * ray_step;
    }

    // now the proper raycast
    for (; i < steps; i++) {
        // if out_of_bounds_i(ipos) {
        //     return res;
        // }

        let block = sample_block(ipos);

        if (block != 0u) {
            res.hit = true;
            res.hit_voxel = ipos;
            res.hit_face = vec3f(ipos); // TODO
            return res;
        }
        
        let mask = side_dist.xyz <= min(side_dist.yzx, side_dist.zxy);
        side_dist += vec3f(mask) * delta_dist;
        ipos += vec3i(mask) * ray_step;
    }

    return res;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dir = (cam.view_mat_inv * normalize(vec4f(
        in.pos.xy * tan(cam.fov_y / 2.0),
        1.0,
        0.0,
    ))).xyz;

    let res = raycast_voxels(cam.pos, dir, 100);

    if res.hit {
        return shade(vec3f(res.hit_voxel), vec3f(0.0, 1.0, 0.0));
    } else {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }

    // let t = -cam.pos.z / dir.z;

    // let hit = cam.pos.xy + t * dir.xy;

    // if all(-1.0 <= hit & hit <= 1.0) {
    //     return vec4f(1.0);
    // } else {
    //     return vec4f(0.0, 0.0, 0.0, 1.0);
    // }
}
