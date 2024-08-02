#import "util.wgsl"::{ vmin, vmax, cmpmin, cmpmax }
#import "bindings.wgsl"::{ colors, dvo }

// this shader is a "module" supposed to be included.
// 
// this module "exports":
// fn raycast(ray_pos: vec3f, ray_dir: vec3f) -> CastResult
// 
// this module "requires":
// const #OCTREE_DEPTH: u32; // depth = 0 for a 2^3 volume: depth = log2(n) - 1.
// const #OCTREE_MAX_ITER: u32 // max number of hit tests in the octree per ray.
// const #GRID_DEPTH: u32; // depth = 0 for a 2^3 volume: depth = log2(n) - 1.
// const #GRID_MAX_ITER: u32 // max number of hit tests in the octree per ray.
// fn get_node(octant_coord: vec3u, octree_depth: u32) -> u32
// fn is_octant_solid(node: u32, octant: vec3u) -> bool
// fn is_voxel_solid(voxel_coord: vec3u) -> bool

// preproc_include(util.wgsl)

// virtual functions do not work yet.
// virtual fn get_node(octant_coord: vec3u, octree_depth: u32) -> u32 { return 0u; }

// virtual fn is_octant_solid(node: u32, octant: vec3u) -> bool { return true; }

// virtual fn is_voxel_solid(voxel_coord: vec3u) -> bool { return true; }

// provide functions to access the dvo, so octree can use it in an agnostic way.
fn get_node(octant_coord: vec3u, octree_depth: u32) -> u32 {
    return textureLoad(dvo, octant_coord, i32(textureNumLevels(dvo) - 1u - octree_depth)).r; // BUG: the cast to i32 is a bug in naga afaik
}

fn is_octant_solid(node: u32, octant: vec3u) -> bool {
    let octant_index = dot(octant, vec3u(4u, 2u, 1u));
    let is_solid = bool(extractBits(node, octant_index, 1u));
    return is_solid;
}

fn is_voxel_solid(voxel_coord: vec3u) -> bool {
    let albedo = textureLoad(colors, voxel_coord, 0);
    return any(albedo != vec4f(0.0));
    // let node_coord = voxel_coord / 2u;
    // let octant = voxel_coord - node_coord * 2u;
    // let node = textureLoad(dvo, voxel_coord, 0).r; // BUG: the cast to i32 is a bug in naga afaik
    // return is_octant_solid(node, octant);
}

struct CastResult {
    pos: vec3f,
    normal: vec3f,
    voxel:vec3u,
    iter: u32,
    t: f32,
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

fn no_hit(iter: u32) -> CastResult {
    return CastResult(vec3f(0.1), vec3f(0.0), vec3u(0u), iter, 0.0, false);
}

// pack a vec3 of 0 or 1 (octant position), as 0b_1xyz
fn pack_octant(octant: vec3u) -> u32 {
    return 8u | dot(octant, vec3u(4u, 2u, 1u));
}

// unpack 0b_1xyz (x,y,z are bits) into vec3(x, y, z)
fn unpack_octant(index: u32) -> vec3u {
    return vec3u((index & 4u) >> 2u, (index & 2u) >> 1u, (index & 1u) >> 0u);
}

// flip coordinates based on the mirror mask
fn mirror_coord(node_coord: vec3u, depth: u32, mirror: vec3u) -> vec3u {
    let mirror_node_coord = mirror * ((1u << depth) - node_coord - 1u) + (1u - mirror) * node_coord;
    return mirror_node_coord;
}

// raycast a node to find the intersected octants.
// returns a packed list of octants positions in a single u32:
// the 4 lsb bits are the packed first octant hit, the next 4 bits the 2nd, etc.
fn next_solid_octants(node_coord: vec3u, depth: u32, ray_pos: vec3f, inv_dir: vec3f, mirror: vec3u) -> u32 {
    let voxels_per_octant = 1u << #OCTREE_DEPTH - depth;
    let node_start_t = (vec3f((node_coord * 2u + 0u) * voxels_per_octant) - ray_pos) * inv_dir;
    let node_mid_t   = (vec3f((node_coord * 2u + 1u) * voxels_per_octant) - ray_pos) * inv_dir;
    var octant = vec3u(node_mid_t < vec3f(0.0) | vec3f(vmax(node_start_t)) > node_mid_t);

    var incr_t = node_mid_t + vec3f(octant) * inv_dir * f32(voxels_per_octant);

    var next_octants = 0u;
    var next_ptr = 0u;

    let mirror_node_coord = mirror_coord(node_coord, depth, mirror);
    let node = get_node(mirror_node_coord, depth);

    if is_octant_solid(node, octant ^ mirror) {
        next_octants = pack_octant(octant);
        next_ptr++;
    }

    for (var i = 0u; i < 3u; i++) {
        let incr_mask = vec3u(cmpmin(incr_t)); // find which axis boundary is the closest
        incr_t += vec3f(incr_mask) * inv_dir * f32(voxels_per_octant);
        if dot(octant, incr_mask) != 0u {
            // exited the node
            break;
        }
        octant += incr_mask;
        if is_octant_solid(node, octant ^ mirror) {
            next_octants = (pack_octant(octant) << (next_ptr * 4u)) | next_octants;
            next_ptr++;
        }
    }

    return next_octants;
}

fn raycast_octree_impl(ray_pos_: vec3f, ray_dir_: vec3f) -> CastResult {
    var depth = 0u;
    var octants_stack = array<u32, (#OCTREE_DEPTH - #GRID_DEPTH)>();

    // handle symmetries: ray is mirrored to go in the positive direction.
    // use mirror_coord() to find un-mirrored positions.
    // real position is needed when sampling the octree: in get_node() and is_octant_solid().
    let ray_dir = abs(ray_dir_);
    let mirror = vec3u(ray_dir_ < vec3f(0.0));
    let ray_pos = ray_pos_ * vec3f(1u - mirror) + (f32(2u << #OCTREE_DEPTH) - ray_pos_) * vec3f(mirror);
    let inv_dir = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    var node_coord = vec3u(0u);
    var next_octants = next_solid_octants(node_coord, depth, ray_pos, inv_dir, mirror);

    for (var i = 0u; i < #OCTREE_MAX_ITER; i++) {
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

            if depth == #OCTREE_DEPTH - #GRID_DEPTH { // found a leaf
                let octant_coord = node_coord * 2u + unpack_octant(octant_index);
                let voxels_per_octant = vec3u(1u << #GRID_DEPTH);
                let octant_start_t = (vec3f((octant_coord + 0u) * voxels_per_octant) - ray_pos) * inv_dir;
                let octant_end_t   = (vec3f((octant_coord + 1u) * voxels_per_octant) - ray_pos) * inv_dir;
                let t = max(vmax(octant_start_t), 0.0);

                if #GRID_DEPTH == 0u {
                    let pos = ray_pos_ + ray_dir_ * t;
                    let normal = vec3f(cmpmax(octant_start_t)) * -sign(ray_dir_);
                    let voxel = mirror_coord(octant_coord, depth + 1u, mirror);
                    return CastResult(pos, normal, voxel, i, t, true);
                }

                let res = raycast_grid_impl(ray_pos, ray_dir, t, octant_end_t, mirror);

                if res.hit {
                    let t = res.t;
                    let pos = ray_pos_ + ray_dir_ * t;
                    let normal = res.normal * -sign(ray_dir_);
                    return CastResult(pos, normal, res.voxel, i + res.iter, t, true);
                }
            }
            else { // recurse, push current node to stack
                octants_stack[depth] = next_octants;
                depth += 1u;
                node_coord = node_coord * 2u + unpack_octant(octant_index);
                next_octants = next_solid_octants(node_coord, depth, ray_pos, inv_dir, mirror);
            }
        }
    }

    // end of iteration
    return no_hit(#OCTREE_MAX_ITER);
}

fn raycast_grid_impl(ray_pos: vec3f, ray_dir: vec3f, t: f32, max_t: vec3f, mirror: vec3u) -> CastResult {
    let inv_dir = 1.0 / ray_dir; // time to traverse 1 voxel in each x,y,z

    let pos = ray_pos + t * ray_dir;
    var voxel_coord = vec3u(pos); // this may move the ray back 1 voxel, but whatever
    var voxel_t = (vec3f(voxel_coord) - ray_pos) * inv_dir;

    for (var i = 0u; i < #GRID_MAX_ITER && all(voxel_t < max_t); i++) {
        let mirror_voxel_coord = mirror_coord(voxel_coord, #OCTREE_DEPTH + 1u, mirror);
        if is_voxel_solid(mirror_voxel_coord) {
            let pos = vec3f(0.0); // ray_pos + ray_dir * t;
            let normal = vec3f(cmpmax(voxel_t));
            let t = vmax(voxel_t);
            return CastResult(pos, normal, mirror_voxel_coord, i, t, true);
        }
        else {
            let incr_mask = cmpmin(voxel_t + inv_dir); // find which axis boundary is the closest
            voxel_t += vec3f(incr_mask) * inv_dir;
            voxel_coord += vec3u(incr_mask);
        }
    }

    // end of iteration
    return no_hit(#GRID_MAX_ITER);
}

// exported function
fn raycast(ray_pos: vec3f, ray_dir: vec3f) -> CastResult {
    let scene_width = f32(2u << #OCTREE_DEPTH);
    let tr_pos = ray_pos / scene_width;
    var t = intersection(tr_pos, ray_dir);

    // no hit
    if t.t_min > t.t_max || t.t_max < 0.0 {
        return no_hit(0u);
    }

    else {
        let res = raycast_octree_impl(ray_pos, ray_dir);
        return res;
    }
}
