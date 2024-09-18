import util/{ vmin, vmax, cmpmin, cmpmax };
import bindings/{ colors, dvo, svo };
import constants/{ OCTREE_DEPTH, DVO_DEPTH, GRID_DEPTH, DVO_MAX_ITER };
import ./util/{ Ray, Hit, Node, CastRes, is_octant_solid, is_res, mirror_coord, pack_octant, unpack_octant, no_res };
import ./raycast/{ dvo_raycast_continue as continue_raycast };

// this module "requires":
// OCTREE_DEPTH: u32; // depth = 0 for a 2^3 volume: depth = log2(n) - 1.
// DVO_DEPTH: u32; // depth = 0 for a 2^3 volume: depth = log2(n) - 1.
// DVO_MAX_ITER: u32 // max number of hit tests in the octree per ray.

// dvo: a 3d texture of format R8Uint (or R32Uint if unsupported).
// the 8 LSB bits are the solid state of the corresponding octant index.
// octant to index: index = dot(octant, (4, 2, 1));

fn dvo_node(octant_coord: vec3u, octree_depth: u32) -> Node {
    return Node(textureLoad(dvo, octant_coord, i32(textureNumLevels(dvo) - 1u - octree_depth)).r); // BUG: the cast to i32 is a bug in naga afaik
}

// raycast a node to find the intersected octants.
// returns a packed list of octants positions in a single u32:
// the 4 lsb bits are the packed first octant hit, the next 4 bits the 2nd, etc.
fn dvo_visit_octants(node_coord: vec3u, depth: u32, ray: Ray) -> u32 {
    let voxels_per_octant = 1u << (OCTREE_DEPTH - depth);
    let node_start_t = (vec3f((node_coord * 2u + 0u) * voxels_per_octant) - ray.pos) * ray.inv_dir;
    let node_mid_t   = (vec3f((node_coord * 2u + 1u) * voxels_per_octant) - ray.pos) * ray.inv_dir;
    var octant = vec3u((node_mid_t < vec3f(0.0)) | (vec3f(vmax(node_start_t)) > node_mid_t));

    var incr_t = node_mid_t + vec3f(octant) * ray.inv_dir * f32(voxels_per_octant);

    var next_octants = 0u;
    var next_ptr = 0u;

    let mirror_node_coord = mirror_coord(node_coord, depth, ray.mirror);
    let node = dvo_node(mirror_node_coord, depth);

    if is_octant_solid(node, octant ^ ray.mirror) {
        next_octants = pack_octant(octant);
        next_ptr++;
    }

    for (var i = 0u; i < 3u; i++) {
        let incr_mask = vec3u(cmpmin(incr_t)); // find which axis boundary is the closest
        incr_t += vec3f(incr_mask) * ray.inv_dir * f32(voxels_per_octant);
        if dot(octant, incr_mask) != 0u {
            // exited the node
            break;
        }
        octant += incr_mask;
        if is_octant_solid(node, octant ^ ray.mirror) {
            next_octants = (pack_octant(octant) << (next_ptr * 4u)) | next_octants;
            next_ptr++;
        }
    }

    return next_octants;
}

fn dvo_raycast_impl(ray: Ray, res: ptr<function, CastRes>) {
    var depth = 0u;
    var octants_stack = array<u32, DVO_DEPTH>();
    var node_coord = vec3u(0u);
    var next_octants = dvo_visit_octants(node_coord, depth, ray);

    for (var i = 0u; i < DVO_MAX_ITER; i++) {
        if next_octants == 0u { // exited node, pop this octree level
            if depth == 0u { // completely out
                no_res(i, res);
                return;
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

            if depth == DVO_DEPTH { // found a leaf
                let octant_coord = node_coord * 2u + unpack_octant(octant_index);
                let voxels_per_octant = vec3u(1u << GRID_DEPTH);
                let octant_start_t = (vec3f((octant_coord + 0u) * voxels_per_octant) - ray.pos) * ray.inv_dir;
                let octant_end_t   = (vec3f((octant_coord + 1u) * voxels_per_octant) - ray.pos) * ray.inv_dir;
                // let t = max(vmax(octant_start_t), 0.0);
                (*res).iter = i;
                (*res).bounds.t_min = max(vmax(octant_start_t), 0.0);
                (*res).bounds.t_max = vmin(octant_end_t);
                (*res).normal = -vec3f(cmpmax(octant_start_t));
                (*res).voxel = octant_coord;

                continue_raycast(ray, res);

                if is_res(*res) {
                    return;
                }
            }
            else { // recurse, push current node to stack
                octants_stack[depth] = next_octants;
                depth += 1u;
                node_coord = node_coord * 2u + unpack_octant(octant_index);
                next_octants = dvo_visit_octants(node_coord, depth, ray);
            }
        }
    }

    // end of iteration
    no_res(DVO_MAX_ITER, res);
}
