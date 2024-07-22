// this shader is a "module" supposed to be included.
// 
// this module "exports":
// fn raycast_octree(ray_pos: vec3f, ray_dir: vec3f) -> CastResult
// 
// this module "requires":
// const OCTREE_DEPTH: u32; // depth = 0 for a 2^3 volume.
// fn octree_root() -> u32
// fn octree_node(octant_coord: vec3u, octree_depth: u32) -> u32

// preproc_include(util.wgsl)

struct CastResult {
    hit: u32,
    pos: vec3f,
    normal: vec3f,
    voxel:vec3u,
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

const MAX_ITER = 200u;

fn no_hit(iter: u32) -> CastResult {
    return CastResult(0u, vec3f(0.0), vec3f(0.0), vec3u(0u), iter);
}

fn raycast_octree_impl(ray_pos_: vec3f, ray_dir_: vec3f, t0: f32) -> CastResult {
    var octree_depth = 0u;
    var node_stack = array<u32, (OCTREE_DEPTH + 1u)>(); // initialized with 0u
    var node_end_stack = array<vec3f, (OCTREE_DEPTH + 1u)>(); // initialized with 0u
    node_stack[0] = octree_root();

    // handle symmetries
    let ray_dir = abs(ray_dir_);
    let mirror = vec3u(ray_dir_ < vec3f(0.0));
    let ray_pos = ray_pos_ * vec3f(1u - mirror) + (f32(2 << OCTREE_DEPTH) - ray_pos_) * vec3f(mirror);
    let side_dt = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var t = t0;
    var node_end = f32(2 << OCTREE_DEPTH) - ray_pos; // distance to the end of the current node (node = 8 octants)
    var octant_end = mix(f32(1 << OCTREE_DEPTH) - ray_pos, node_end, step((f32(1 << OCTREE_DEPTH) - ray_pos) * side_dt, vec3f(t))); // distance to the end of the current octant
    var octant_coord = vec3u(0u);

    var incr_mask = cmpmax(-ray_pos * side_dt);

    for (var i = 0u; i < MAX_ITER; i++) {

        let octant_pos = vec3u(octant_end == node_end) ^ mirror;
        let octant_index = dot(octant_pos, vec3u(4u, 2u, 1u)); // octant to index: x*4 + y*2 + z = 0b(xyz)
        let octant_filled = extractBits(node_stack[octree_depth], octant_index, 1u);

        // octant is solid, time to "recurse"
        if octant_filled != 0u {
            if octree_depth == OCTREE_DEPTH { // found a leaf
                let pos = ray_pos_ + t * ray_dir_;
                let normal = vec3f(incr_mask) * -sign(ray_dir_);
                let voxel = (octant_coord * 2u) + octant_pos;
                return CastResult(1u, pos, normal, voxel, i);
            }
            else { // recurse, push current node to stack
                node_end_stack[octree_depth] = node_end;
                octree_depth += 1u;
                octant_coord = (octant_coord * 2u) + octant_pos;
                node_stack[octree_depth] = octree_node(octant_coord, octree_depth);
                node_end = octant_end;
                let node_mid = node_end - f32(1 << (OCTREE_DEPTH - octree_depth));
                octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
            }
        }

        // octant is empty, move to the next
        else {
            incr_mask = cmpmin(octant_end * side_dt); // which axis boundary is the closest
            let incr_axis = dot(vec3i(incr_mask), vec3i(0, 1, 2));

            // t = vmin(octant_end * side_dt);
            // octant_end = node_end * vec3f(incr_mask) + octant_end * vec3f(!incr_mask);
            t = (octant_end * side_dt)[incr_axis];
            octant_end[incr_axis] = node_end[incr_axis];
            
            // outside octants, must pop the stack or finish raycast
            while t == vmin(node_end * side_dt) {
                if octree_depth == 0u { // completely out
                    return no_hit(i);
                }
                else { // "pop" the recursion stack
                    octree_depth -= 1u;
                    octant_coord /= 2u;
                    node_end = node_end_stack[octree_depth];
                    let node_mid = node_end - f32(1 << (OCTREE_DEPTH - octree_depth));
                    octant_end = mix(node_mid, node_end, step(node_mid * side_dt, vec3f(t)));
                }
            }
        }
    }

    // end of iteration
    return no_hit(MAX_ITER);
}

fn raycast_octree(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    let octree_width = f32(2 << OCTREE_DEPTH);
    let tr_pos = ray_pos / octree_width;
    var t = intersection(tr_pos, ray_dir);

    // no hit
    if t.t_min > t.t_max || t.t_max < 0.0 {
        return no_hit(0u);
    }

    else {
        // TODO: 1000.0 avoids a numerical error, which idk where it comes from.
        // return raycast_octree_impl(ray_pos - 1000.0 * ray_dir, ray_dir, max(t.t_min, 0.0) * octree_width + 1000.0);
        return raycast_octree_impl(ray_pos, ray_dir, max(t.t_min, 0.0) * octree_width);
    }
}
