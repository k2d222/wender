// this shader is a "module" supposed to be included.
// this module "exports" `raycast_dvo` and `intersection`
// this module "requires" `var<storage, read> dvo: array<u32>`
//
// constants below are overriden by the preprocessor.
// const DVO_DEPTH = 9u; // depth = 0 for a 2^3 volume.

// preproc_include(util.wgsl)

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
    node_stack[0] = dvo[0];

    // handle symmetries
    let ray_dir = abs(ray_dir_);
    let mirror = vec3u(ray_dir_ < vec3f(0.0));
    let ray_pos = ray_pos_ * vec3f(1u - mirror) + (f32(2 << DVO_DEPTH) - ray_pos_) * vec3f(mirror);
    let side_dt = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var t = t0;
    var node_end = f32(2 << DVO_DEPTH) - ray_pos; // distance to the end of the current node (node = 8 octants)
    var octant_end = mix(f32(1 << DVO_DEPTH) - ray_pos, node_end, step((f32(1 << DVO_DEPTH) - ray_pos) * side_dt, vec3f(t))); // distance to the end of the current octant
    var octant_coord = vec3u(0u);

    var incr_mask = cmpmax(-ray_pos * side_dt);

    for (var i = 0u; i < 1000u; i++) {

        let octant_pos = vec3u(octant_end == node_end) ^ mirror;
        let octant_index = dot(octant_pos, vec3u(4u, 2u, 1u)); // octant to index: x*4 + y*2 + z = 0b(xyz)
        let octant_filled = extractBits(node_stack[dvo_depth], octant_index, 1u);

        // octant is solid, time to "recurse"
        if octant_filled != 0u {
            if dvo_depth == DVO_DEPTH { // found a leaf
                let pos = ray_pos_ + t * ray_dir_;
                let end_t = octant_end * side_dt;
                let normal = vec3f(incr_mask) * -sign(ray_dir_);
                return CastResult(1u, pos, normal, i);
            }
            else { // recurse, push current node to stack
                node_end_stack[dvo_depth] = node_end;
                dvo_depth += 1u;
                octant_coord = (octant_coord * 2u) + octant_pos;
                node_stack[dvo_depth] = dvo[dvo_ptr(octant_coord, dvo_depth)];
                node_end = octant_end;
                let node_mid = node_end - f32(1 << (DVO_DEPTH - dvo_depth));
                octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
            }
        }

        // octant is empty, move to the next
        else {
            incr_mask = cmpmin(octant_end * side_dt); // which axis boundary is the closest
            let incr_axis = dot(vec3i(incr_mask), vec3i(0, 1, 2));

            t = vmin(octant_end * side_dt);
            octant_end = node_end * vec3f(incr_mask) + octant_end * vec3f(!incr_mask);
            
            // outside octants, must pop the stack or finish raycast
            while t == vmin(node_end * side_dt) {
                if dvo_depth == 0u { // completely out
                    // return NO_HIT;
                    return CastResult(0u, vec3f(0.0), vec3f(0.0), i);
                }
                else { // "pop" the recursion stack
                    dvo_depth -= 1u;
                    octant_coord /= 2u;
                    node_end = node_end_stack[dvo_depth];
                    let node_mid = node_end - f32(1 << (DVO_DEPTH - dvo_depth));
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
