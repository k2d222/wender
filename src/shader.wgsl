struct Camera {
    pos: vec3f,
    fov_y: f32,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

@group(0) @binding(0)
var<uniform> cam: Camera;

const DIM = vec3(256, 256, 256);
const SVO_DEPTH = 8;

struct Voxels {
    data: array<array<array<u32, DIM.z>, DIM.y>, DIM.x>,
}

struct SvoNode {
    octants: array<u32, 8>,
}

@group(1) @binding(0)
var<storage, read> svo: array<SvoNode>;

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
    return any(pos < vec3(0) | pos >= DIM);
}

// intersection with a unit aabb cube with one corner at (0,0,0) and the other at (1,1,1).
// returns time to intersection (if intersects), or -1 (if no intersection)
fn intersects(ray_pos: vec3f, ray_dir: vec3f) -> f32 {
    let inv_dir = 1.0 / ray_dir;

    let t1 = (0.0 - ray_pos) * inv_dir;
    let t2 = (1.0 - ray_pos) * inv_dir;

    let a_min = min(t1, t2);
    let a_max = max(t1, t2);
    let t_min = max(max(a_min.x, a_min.y), a_min.z);
    let t_max = min(min(a_max.x, a_max.y), a_max.z);

    let intersects = t_min <= t_max;

    if intersects {
        return t_min;
    } else {
        return -1.0;
    }
}

// fn ray_octant(ray_pos: vec3f, cube_min)

fn raycast_svo(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    var res = CastResult(true, vec3i(0), ray_pos);

    var cube_min = vec3f(0.0);
    var cube_max = vec3f(256.0);
    var cube_extent = 256.0;
    var svo_index = 0u;

    for (var i = 0; i < 2; i++) {
        let tr_pos = (ray_pos - cube_min) / cube_extent;

        let t = intersects(tr_pos, ray_dir);

        // no hit
        if t < 0.0 {
            res.hit = false;
            return res;
        }

        // hit, test the octants
        else {
            let octant = step(vec3f(0.5), tr_pos + t * ray_dir); // vec of 1/0 identifying octants, from (0,0,0) to (1,1,1)
            let octant_index = u32(dot(octant, vec3f(4.0, 2.0, 1.0))); // octant to index: x*4 + y*2 + z = 0b(xyz)

            var node = svo[svo_index];
            let val = node.octants[octant_index];

            // this octant is air, but maybe another octant intersects
            if val == 0u {
                cube_extent /= 2.0;

                // test them all
                var min_t = -1.0;
                var min_j = 0u;
                for (var j = 0u; j < 8u; j++) {
                    let val = node.octants[j];
                    if val != 0u {
                        let octant = vec3f(f32(j >> 2u), f32((j & 2u) >> 1u), f32(j & 1u));
                        let tr_pos = (ray_pos - cube_min - cube_extent * octant) / cube_extent;
                        let t = intersects(tr_pos, ray_dir);
                        if t > 0.0 && (min_t < 0.0 || t < min_t) {
                            min_t = t;
                            min_j = j;
                        }
                    }
                }

                // only air
                if min_t < 0.0 {
                    res.hit = false;
                    return res;
                } else {
                    svo_index = node.octants[min_j];
                    let octant = vec3f(f32(min_j >> 2u), f32((min_j & 2u) >> 1u), f32(min_j & 1u));
                    cube_min += cube_extent * octant;
                    cube_max -= (1.0 - cube_extent) * octant;
                }
            }

            // found a hit in an octant, recurse
            else {
                svo_index = val;
                cube_extent /= 2.0;
                cube_min += cube_extent * octant;
                cube_max -= (1.0 - cube_extent) * octant;
            }
        }
    }

    // compute final hit
    let tr_pos = (ray_pos - cube_min) / cube_extent;
    let t = intersects(tr_pos, ray_dir);
    res.hit_pos = ray_pos + t * cube_extent * ray_dir;
    res.hit_voxel = vec3i(res.hit_pos + 0.01 * ray_dir);

    return res;
}

fn sample_svo_leaf(ipos: vec3i) -> u32 {
    return 1u;
}

// fn sample_voxel(ipos: vec3i) -> u32 {
//     if out_of_bounds_i(ipos) {
//         return 0u;
//     } else {
//         return voxels.data[ipos.x][ipos.y][ipos.z];
//     }
// }

// taken from https://www.shadertoy.com/view/4dX3zl
// copyright Lode Vandevenne (https://lodev.org/cgtutor/raycasting.html)
// copyright user "fb39ca4" on shadertoy (https://www.shadertoy.com/view/4dX3zl)
// licensed under cc-by-nc-sa
// fn raycast_voxels(ray_pos: vec3f, ray_dir: vec3f, steps: i32) -> CastResult {
//     var ipos = vec3i(floor(ray_pos)); // which voxel we're in
//     let boundary_dt = abs(1.0 / ray_dir); // time to traverse 1 voxel in each x,y,z
//     var next_side_dt = (sign(ray_dir) * (0.5 - fract(ray_pos)) + 0.5) * boundary_dt; // time to reach the next voxel boundary in each x,y,z
//     let ray_sign = vec3i(sign(ray_dir));
//     var i = 0;
//     var t = 0.0;

//     var res = CastResult(false, vec3i(0), vec3f(0.0));

//     for (; i < steps; i++) {
//         // // early return
//         // if out_of_bounds_i(ipos) {
//         //     return res;
//         // }

//         let voxel = sample_voxel(ipos);

//         if voxel != 0u {
//             res.hit = true;
//             res.hit_voxel = ipos;
//             res.hit_pos = ray_pos + t * ray_dir;
//             return res;
//         }
        
//         let mask = next_side_dt.xyz <= min(next_side_dt.yzx, next_side_dt.zxy); // which boundary axis is the closest
//         t = min(min(next_side_dt.x, next_side_dt.y), next_side_dt.z);
//         next_side_dt += vec3f(mask) * boundary_dt; // increment the next boundary for the selected axis by 1 voxel.
//         ipos += vec3i(mask) * ray_sign; // advance by 1 voxel along the selected axis
//     }

//     return res;
// }

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

    // let res = raycast_voxels(cam.pos, dir, 2000);
    let res = raycast_svo(cam.pos, dir);

    if res.hit {
        let normal = cube_face_normal(res.hit_voxel, res.hit_pos);
        // let palette_index = sample_svo(res.hit_voxel) - 1u;
        // let albedo = palette[palette_index];
        let albedo = vec4f(1.0);
        return shade(albedo, cam.pos, res.hit_pos, normal);
        // return vec4f(1.0);
    } else {
        return vec4f(0.0, 0.0, 0.0, 1.0);
        // return palette[0];
    }
}
