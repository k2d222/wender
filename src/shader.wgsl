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
    hit: u32,
    hit_pos: vec3f,
}

fn out_of_bounds_i(pos: vec3i) -> bool {
    return any(pos < vec3(0) | pos >= DIM);
}

struct Intersect {
    t_min: f32,
    t_max: f32,
}

// intersection with a unit aabb cube with one corner at (0,0,0) and the other at (1,1,1).
// returns time to intersection. There was no intersection if t_min > t_max.
fn intersection(ray_pos: vec3f, ray_dir: vec3f) -> Intersect {
    let inv_dir = 1.0 / ray_dir;

    let t1 = (0.0 - ray_pos) * inv_dir;
    let t2 = (1.0 - ray_pos) * inv_dir;

    let a_min = min(t1, t2);
    let a_max = max(t1, t2);
    let t_min = max(max(a_min.x, a_min.y), a_min.z);
    let t_max = min(min(a_max.x, a_max.y), a_max.z);

    return Intersect(t_min, t_max);
}

fn front_intersection(ray_pos: vec3f, ray_dir: vec3f) -> f32 {
    let inv_dir = 1.0 / ray_dir;

    let t1 = (0.0 - ray_pos) * inv_dir;
    let t2 = (1.0 - ray_pos) * inv_dir;

    let a_min = min(t1, t2);
    let t_min = max(max(a_min.x, a_min.y), a_min.z);

    return t_min;
}

fn back_intersection(ray_pos: vec3f, ray_dir: vec3f) -> f32 {
    let inv_dir = 1.0 / ray_dir;

    let t1 = (0.0 - ray_pos) * inv_dir;
    let t2 = (1.0 - ray_pos) * inv_dir;

    let a_max = max(t1, t2);
    let t_max = min(min(a_max.x, a_max.y), a_max.z);

    return t_max;
}

fn raycast_svo_0(svo_index: u32, t: f32, ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    return CastResult(svo_index, ray_pos + t * ray_dir);
}

#[recursive 9]
fn raycast_svo(svo_index: u32, _t: f32, ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    var t = _t;
    var node = svo[svo_index];

    // test all octants (max. 4 octants)
    for (var i = 0; i < 4; i++) {
        let octant = step(vec3f(0.5), ray_pos + t * ray_dir); // vec of 1/0 identifying octants, from (0,0,0) to (1,1,1)
        let octant_index = u32(dot(octant, vec3f(4.0, 2.0, 1.0))); // octant to index: x*4 + y*2 + z = 0b(xyz)
        let next_index = node.octants[octant_index];

        if next_index != 0u { // octant non-empty
            let tr_pos = (ray_pos - octant * 0.5) * 2.0;
            var recurse = raycast_svo(next_index, t * 2.0, tr_pos, ray_dir);

            if recurse.hit != 0u { // ray hit something recusively
                recurse.hit_pos = recurse.hit_pos * 0.5 + octant * 0.5;
                return recurse;
            }
        }

        // next octant
        let tr_pos = (ray_pos - octant * 0.5) * 2.0;
        t = back_intersection(tr_pos, ray_dir) * 0.5 + 0.00001;
    }

    // ray left the octants
    return CastResult(0u, vec3f(0.0));
}

fn raycast_svo(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    let tr_pos = ray_pos / 256.0;
    var t = intersection(tr_pos, ray_dir);

    // no hit
    if t.t_min > t.t_max || t.t_max < 0.0 {
        return CastResult(0u, vec3f(0.0));
    }

    else {
        var res = raycast_svo_8(0u, max(t.t_min, 0.0), tr_pos, ray_dir);
        res.hit_pos *= 256.0;
        return res;
    }
}

fn sample_svo_leaf(ipos: vec3i) -> u32 {
    return 1u;
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

    // let res = raycast_voxels(cam.pos, dir, 2000);
    let res = raycast_svo(cam.pos, dir);

    if res.hit != 0u {
        let hit_voxel = vec3i(floor(res.hit_pos + dir * 0.01));
        let normal = cube_face_normal(hit_voxel, res.hit_pos);
        // let palette_index = sample_svo(res.hit_voxel) - 1u;
        let albedo = palette[res.hit - 1u];
        // let albedo = vec4f(1.0);
        return shade(albedo, cam.pos, res.hit_pos, normal);
        // return res.col;
        // return vec4f(res.hit_pos, 1.0);
    } else {
        return vec4f(0.0, 0.0, 0.0, 1.0);
        // return palette[0];
    }
}
