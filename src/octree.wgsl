// this shader is a "module" supposed to be included.
// 
// this module "exports":
// fn raycast_octree(ray_pos: vec3f, ray_dir: vec3f) -> CastResult
// 
// this module "requires":
// const OCTREE_DEPTH: u32; // depth = 0 for a 2^3 volume.
// fn octree_node(octant_coord: vec3u, octree_depth: u32) -> u32

// preproc_include(util.wgsl)

struct CastResult {
    pos: vec3f,
    normal: vec3f,
    voxel:vec3u,
    iter: u32,
    hit: bool,
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
    return CastResult(vec3f(0.1), vec3f(0.0), vec3u(0u), iter, false);
}

fn pack_octant(octant: vec3u) -> u32 {
    return 8u | dot(octant, vec3u(4u, 2u, 1u));
}
fn unpack_octant(index: u32) -> vec3u {
    return vec3u((index & 4u) >> 2u, (index & 2u) >> 1u, (index & 1u) >> 0u);
}

fn is_solid(node: u32, octant: vec3u) -> bool {
    let octant_index = pack_octant(octant) & 7u;
    return bool(extractBits(node, octant_index, 1u));
}

fn mirror_coord(node_coord: vec3u, depth: u32, mirror: vec3u) -> vec3u {
    let mirror_node_coord = mirror * ((1u << depth) - node_coord - 1u) + (1u - mirror) * node_coord;
    return mirror_node_coord;
}

// return a packed list of next octants
fn next_solid_octants(node_coord: vec3u, depth: u32, ray_pos: vec3f, side_dt: vec3f, mirror: vec3u) -> u32 {
    let voxels_per_octant = 1u << OCTREE_DEPTH - depth;
    let node_start_t = (vec3f((node_coord * 2u + vec3u(0u)) * voxels_per_octant) - ray_pos) * side_dt;
    let node_mid_t   = (vec3f((node_coord * 2u + vec3u(1u)) * voxels_per_octant) - ray_pos) * side_dt;
    var octant = vec3u(node_mid_t < vec3f(0.0) || vec3f(vmax(node_start_t)) > node_mid_t);

    var incr_t = node_mid_t + vec3f(octant) * side_dt * f32(voxels_per_octant);

    var next_octants = 0u;
    var next_ptr = 0u;

    let mirror_node_coord = mirror_coord(node_coord, depth, mirror);
    let node = octree_node(mirror_node_coord, depth);

    if is_solid(node, octant ^ mirror) {
        next_octants = pack_octant(octant);
        next_ptr++;
    }

    for (var i = 0u; i < 3u; i++) {
        let incr_mask = vec3u(cmpmin(incr_t)); // find which axis boundary is the closest
        incr_t += vec3f(incr_mask) * side_dt * f32(voxels_per_octant);
        if dot(octant, incr_mask) != 0u {
            // exited the node
            break;
        }
        octant += incr_mask;
        if is_solid(node, octant ^ mirror) {
            next_octants = (pack_octant(octant) << (next_ptr * 4u)) | next_octants;
            next_ptr++;
        }
    }

    return next_octants;
}

fn raycast_octree_impl(ray_pos_: vec3f, ray_dir_: vec3f) -> CastResult {
    var depth = 0u;
    var octants_stack = array<u32, (OCTREE_DEPTH + 1u)>();

    // handle symmetries: ray is mirrored to go in the positive direction, use (coord ^ mirror) to find real positions.
    // real position is only needed when sampling the octee: in octree_node() and is_solid.
    let ray_dir = abs(ray_dir_);
    let mirror = vec3u(ray_dir_ < vec3f(0.0));
    let ray_pos = ray_pos_ * vec3f(1u - mirror) + (f32(2 << OCTREE_DEPTH) - ray_pos_) * vec3f(mirror);
    let side_dt = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var node_coord = vec3u(0u);
    var next_octants = next_solid_octants(node_coord, depth, ray_pos, side_dt, mirror);

    for (var i = 0u; i < MAX_ITER; i++) {
        if next_octants == 0u { // exited node, pop this octree level
            if depth == 0u { // completely out
                return no_hit(i);
            }
            else { // "pop" the recursion stack
                depth -= 1u;
                next_octants = octants_stack[depth];
                node_coord /= 2u;
            }
        }
        else { // found a solid octant, push next octree level
            let octant_index = next_octants & 7u;
            next_octants >>= 4u;

            if depth == OCTREE_DEPTH { // found a leaf
                node_coord = node_coord * 2u + unpack_octant(octant_index);
                let node_start_t = (vec3f(node_coord) - ray_pos) * side_dt;
                let t = vmax(node_start_t);
                let pos = ray_pos_ + ray_dir_ * t;
                let normal = vec3f(cmpmax(node_start_t)) * -sign(ray_dir_);
                let voxel = mirror_coord(node_coord, depth + 1u, mirror);
                return CastResult(pos, normal, voxel, i, true);
            }
            else { // recurse, push current node to stack
                octants_stack[depth] = next_octants;
                depth += 1u;
                node_coord = node_coord * 2u + unpack_octant(octant_index);
                next_octants = next_solid_octants(node_coord, depth, ray_pos, side_dt, mirror);
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
        return raycast_octree_impl(ray_pos, ray_dir);
    }
}
