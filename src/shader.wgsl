struct Camera {
    pos: vec3f,
    fov_y: f32,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

@group(0) @binding(0)
var<uniform> cam: Camera;

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
    hit_voxel: vec3i,
}

struct Intersect {
    t_min: f32,
    t_max: f32,
}

// intersection with a unit aabb cube with one corner at (0,0,0) and the other at (1,1,1).
// returns time to intersection. There was no intersection if t_min > t_max or t_max < 0.
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

const SVO_DEPTH = 6u;
var<private> SVO_INV_DIMS: array<f32, 21> = array(1.0, 1.0/2.0, 1.0/4.0, 1.0/8.0, 1.0/16.0, 1.0/32.0, 1.0/64.0, 1.0/128.0, 1.0/256.0, 1.0/512.0, 1.0/1024.0, 1.0/2048.0, 1.0/4096.0, 1.0/8192.0, 1.0/16384.0, 1.0/32768.0, 1.0/65536.0, 1.0/131072.0, 1.0/262144.0, 1.0/524288.0, 1.0/1048576.0);
const NO_HIT = CastResult(0u, vec3f(0.0), vec3i(0));

fn vmin(vec: vec3f) -> f32 {
    return min(min(vec.x, vec.y), vec.z);
}

fn vmax(vec: vec3f) -> f32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn raycast_svo_impl(ray_pos_: vec3f, ray_dir_: vec3f, t0: f32) -> CastResult {
    var svo_depth = SVO_DEPTH;
    var octant_width = f32(1 << svo_depth);
    var node_width = f32(2 << svo_depth);
    var ptr_stack = array<u32, (SVO_DEPTH + 1u)>(); // initialized with 0u
    var node_end_stack = array<vec3f, (SVO_DEPTH + 1u)>(); // initialized with 0u

    let ray_dir = abs(ray_dir_);
    let ray_pos = ray_pos_ * vec3f(ray_dir_ >= vec3f(0.0)) + (node_width - ray_pos_) * vec3f(ray_dir_ < vec3f(0.0));
    let mirror = (ray_dir_ < vec3f(0.0));
    let side_dt = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var t = t0;
    var node_end = node_width - ray_pos; // distance to the end of the current node (node = 8 octants)
    var node_mid = octant_width - ray_pos; // distance to the middle of the current node (mid-point between octants)
    var octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t))); // distance to the end of the current octant

    for (var i = 0; i < 1000; i++) {
        // outside octants, must pop the stack or finish raycast
        if t == vmin(node_end * side_dt) {
            if svo_depth == SVO_DEPTH { // completely out
                return NO_HIT;
            }
            else { // "pop" the recursion stack
                node_end = node_end_stack[svo_depth];
                svo_depth += 1u;
                octant_width = f32(1 << svo_depth);
                node_width = f32(2 << svo_depth);
                node_mid = node_end - octant_width;
                octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
            }
        }

        else {
            // let octant_pos = vec3i(octant_end_t == node_end_t & !mirror | octant_end_t != node_end_t & mirror);
            var octant_pos = vec3i((octant_end * side_dt + t) * 0.5 >= node_mid * side_dt & !mirror | (octant_end * side_dt + t) * 0.5 < node_mid * side_dt & mirror); // position inside the octants (0,0,0) to (1,1,1)
            // var octant_pos = vec3i(vec3f(t) >= node_mid_t & !mirror | vec3f(t) < node_mid_t & mirror); // position inside the octants (0,0,0) to (1,1,1)
            let octant_index = dot(octant_pos, vec3i(4, 2, 1)); // octant to index: x*4 + y*2 + z = 0b(xyz)
            let octant_ptr = svo[ptr_stack[svo_depth]].octants[octant_index];

            // octant is solid, time to "recurse"
            if octant_ptr != 0u {
                if svo_depth == 0u { // found a leaf
                    let t_max = vmin(octant_end * side_dt);
                    let pos = ray_pos_ + t * ray_dir_;
                    let ipos = vec3i(ray_pos_ + (t + t_max) * 0.5 * ray_dir_);
                    return CastResult(octant_ptr, pos, ipos);
                }
                else { // recurse, push current node to stack
                    svo_depth -= 1u;
                    ptr_stack[svo_depth] = octant_ptr;
                    node_end_stack[svo_depth] = node_end;
                    octant_width = f32(1 << svo_depth);
                    node_width = f32(2 << svo_depth);
                    node_end = octant_end;
                    node_mid = node_end - octant_width;
                    octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
                }
            }

            // octant is empty, move to the next
            else {
                let end_t = octant_end * side_dt;
                let incr_mask = end_t.xyz <= min(end_t.yzx, end_t.zxy); // which axis boundary is the closest
                let incr_axis = dot(vec3i(incr_mask), vec3i(0, 1, 2));

                if t > vmin(end_t) {
                    return CastResult(0u, vec3f(0.0, 1.0, 0.0), vec3i(0));
                }
                t = vmin(end_t);
                // octant_end_t += vec3f(incr_mask) * octant_width;
                octant_end = node_end * vec3f(incr_mask) + octant_end * vec3f(!incr_mask);
                // octant_end_t[incr_axis] = node_end_t[incr_axis];
            }
        }
    }

    // end of iteration
    // return NO_HIT;
    return CastResult(0u, vec3f(1.0, 0.0, 0.0), vec3i(0));
}

fn raycast_svo(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    let svo_width = f32(2 << SVO_DEPTH);
    let tr_pos = ray_pos / svo_width;
    var t = intersection(tr_pos, ray_dir);

    // no hit
    if t.t_min > t.t_max || t.t_max < 0.0 {
        return NO_HIT;
    }

    else {
        // TODO: 1000.0 avoids a numerical error, which idk where it comes from.
        // return raycast_svo_impl(ray_pos - 1000.0 * ray_dir, ray_dir, max(t.t_min, 0.0) * svo_width + 1000.0);
        return raycast_svo_impl(ray_pos, ray_dir, max(t.t_min, 0.0) * svo_width);
    }
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

    let res = raycast_svo(cam.pos, dir);

    if res.hit != 0u {
        let normal = cube_face_normal(res.hit_voxel, res.hit_pos);
        let albedo = palette[res.hit - 1u];
        return shade(albedo, cam.pos, res.hit_pos, normal);
    } else {
        return vec4f(res.hit_pos, 1.0);
    }
}
