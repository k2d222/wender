import package::bindings::{ colors };

struct Intersect {
    t_min: f32,
    t_max: f32,
}

struct Ray {
    pos: vec3f,
    dir: vec3f,
    inv_dir: vec3f,
    bounds: Intersect,
    mirror: vec3u,
}

struct Hit {
    pos: vec3f,
    voxel: vec3u,
    normal: vec3f,
    iter: u32,
    t: f32,
}

struct Node {
    value: u32,
}

struct CastRes {
    iter: u32,
    voxel: vec3u,
    normal: vec3f,
    bounds: Intersect,
}

fn is_octant_solid(node: Node, octant: vec3u) -> bool {
    let octant_index = dot(octant, vec3u(4u, 2u, 1u));
    let is_solid = bool(extractBits(node.value, octant_index, 1u));
    return is_solid;
}

fn is_voxel_solid(voxel_coord: vec3u) -> bool {
    let albedo = textureLoad(colors, voxel_coord, 0);
    return any(albedo != vec4f(0.0));
    // let node_coord = voxel_coord / 2u;
    // let octant = voxel_coord - node_coord * 2u;
    // let node = textureLoad(dvo, voxel_coord, 0).r;
    // return is_octant_solid(node, octant);
}

fn is_hit(hit: Hit) -> bool {
    return hit.t >= 0.0;
}

fn is_res(res: CastRes) -> bool {
    return res.bounds.t_min >= 0.0;
}

fn no_hit(iter: u32) -> Hit {
    var hit: Hit;
    hit.t = -1.0;
    return hit;
}

fn no_res(iter: u32, res: ptr<function, CastRes>)  {
    (*res).iter += iter;
    (*res).bounds.t_min = -1.0;
}

// intersection with a unit aabb cube with one corner at (0,0,0) and the other at (1,1,1).
// returns time to intersection. There was no intersection if t_min > t_max or t_max < 0.
fn unit_intersection(ray_pos: vec3f, ray_dir: vec3f) -> Intersect {
    let inv_dir = 1.0 / ray_dir;

    let t1 = (0.0 - ray_pos) * inv_dir;
    let t2 = (1.0 - ray_pos) * inv_dir;

    let a_min = min(t1, t2);
    let a_max = max(t1, t2);
    let t_min = max(max(a_min.x, a_min.y), a_min.z);
    let t_max = min(min(a_max.x, a_max.y), a_max.z);

    return Intersect(t_min, t_max);
}

fn intersection(ray_pos: vec3f, ray_dir: vec3f, box_size: f32) -> Intersect {
    let tr_pos = ray_pos / box_size;
    var t = unit_intersection(tr_pos, ray_dir);
    t.t_min *= box_size;
    t.t_max *= box_size;
    return t;
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

