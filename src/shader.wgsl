const DVO_DEPTH = 9u; // depth = 0 for a 2^3 volume.

struct Camera {
    pos: vec3f,
    fov_y: f32,
    size: vec2f,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

@group(0) @binding(0)
var<uniform> cam: Camera;

@group(1) @binding(0)
var<storage, read> dvo: array<u32>;

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

struct CastResult {
    hit: u32,
    hit_pos: vec3f,
    hit_normal: vec3f,
    iter: u32,
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

const NO_HIT = CastResult(0u, vec3f(0.0), vec3f(0.0), 0u);

fn vmin(vec: vec3f) -> f32 {
    return min(min(vec.x, vec.y), vec.z);
}

fn vmax(vec: vec3f) -> f32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn cmpmin(vec: vec3f) -> vec3<bool> {
    return vec.xyz <= min(vec.yzx, vec.zxy);
}

fn cmpmax(vec: vec3f) -> vec3<bool> {
    return vec.xyz >= max(vec.yzx, vec.zxy);
}

fn dvo_ptr(octant_coord: vec3u, dvo_depth: u32) -> u32 {
    let base_ptr = ((1u << 3u * dvo_depth) - 1u) / 7u;
    let w = 1u << dvo_depth;
    let octant_ptr = base_ptr + dot(octant_coord, vec3u(w * w, w, 1u));
    return octant_ptr;
}

fn raycast_dvo_impl(ray_pos_: vec3f, ray_dir_: vec3f, t0: f32) -> CastResult {
    var dvo_depth = 0u;
    var node_stack = array<u32, (DVO_DEPTH + 1u)>(); // initialized with 0u
    var node_end_stack = array<vec3f, (DVO_DEPTH + 1u)>(); // initialized with 0u

    // handle symmetries
    let ray_dir = abs(ray_dir_);
    let mirror = vec3u(ray_dir_ < vec3f(0.0));
    let ray_pos = ray_pos_ * vec3f(1u - mirror) + (f32(2 << DVO_DEPTH) - ray_pos_) * vec3f(mirror);
    let side_dt = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var t = t0;
    var node_end = f32(2 << DVO_DEPTH) - ray_pos; // distance to the end of the current node (node = 8 octants)
    var node_mid = f32(1 << DVO_DEPTH) - ray_pos; // distance to the middle of the current node (mid-point between octants)
    var octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t))); // distance to the end of the current octant
    var octant_coord = vec3u(0u);
    var node = dvo[0];

    var incr_mask = cmpmax(-ray_pos * side_dt);

    for (var i = 0u; i < 1000u; i++) {

        let octant_pos = vec3u(octant_end == node_end) ^ mirror;
        let octant_index = dot(octant_pos, vec3u(4u, 2u, 1u)); // octant to index: x*4 + y*2 + z = 0b(xyz)
        let octant_filled = extractBits(node, octant_index, 1u);

        // octant is solid, time to "recurse"
        if octant_filled != 0u {
            if dvo_depth == DVO_DEPTH { // found a leaf
                let pos = ray_pos_ + t * ray_dir_;
                let end_t = octant_end * side_dt;
                let normal = vec3f(incr_mask) * -sign(ray_dir_);
                return CastResult(1u, pos, normal, i);
            }
            else { // recurse, push current node to stack
                dvo_depth += 1u;
                octant_coord = (octant_coord * 2u) + octant_pos;
                node_end_stack[dvo_depth] = node_end;
                node = dvo[dvo_ptr(octant_coord, dvo_depth)];
                node_stack[dvo_depth] = node;
                node_end = octant_end;
                node_mid = node_end - f32(1 << (DVO_DEPTH - dvo_depth));
                octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
            }
        }

        // octant is empty, move to the next
        else {
            incr_mask = cmpmin(octant_end * side_dt); // which axis boundary is the closest
            let incr_axis = dot(vec3i(incr_mask), vec3i(0, 1, 2));

            t = vmin(octant_end * side_dt);
            // octant_end_t += vec3f(incr_mask) * octant_width;
            octant_end = node_end * vec3f(incr_mask) + octant_end * vec3f(!incr_mask);
            // octant_end_t[incr_axis] = node_end_t[incr_axis];
            
            // outside octants, must pop the stack or finish raycast
            while t == vmin(node_end * side_dt) {
                if dvo_depth == 0u { // completely out
                    // return NO_HIT;
                    return CastResult(0u, vec3f(0.0), vec3f(0.0), i);
                }
                else { // "pop" the recursion stack
                    octant_coord /= 2u;
                    node_end = node_end_stack[dvo_depth];
                    dvo_depth -= 1u;
                    node = node_stack[dvo_depth];
                    node_mid = node_end - f32(1 << (DVO_DEPTH - dvo_depth));
                    octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
                    // return CastResult(0u, vec3f(0.0, 1.0, 0.0), vec3f(0.0), i);
                }
            }
        }
    }

    // end of iteration
    // return NO_HIT;
    return CastResult(0u, vec3f(1.0, 0.0, 0.0), vec3f(0.0), 1000u);
}

fn raycast_dvo(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    let dvo_width = f32(2 << DVO_DEPTH);
    let tr_pos = ray_pos / dvo_width;
    var t = intersection(tr_pos, ray_dir);

    // no hit
    if t.t_min > t.t_max || t.t_max < 0.0 {
        return NO_HIT;
    }

    else {
        // TODO: 1000.0 avoids a numerical error, which idk where it comes from.
        // return raycast_dvo_impl(ray_pos - 1000.0 * ray_dir, ray_dir, max(t.t_min, 0.0) * dvo_width + 1000.0);
        return raycast_dvo_impl(ray_pos, ray_dir, max(t.t_min, 0.0) * dvo_width);
    }
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
