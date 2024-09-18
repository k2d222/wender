import constants/{ GRID_MAX_ITER, OCTREE_DEPTH };
import bindings/{ colors, dvo, svo };
import util/{ vmin, vmax, cmpmin, cmpmax };
import ./util/{ Ray, Hit, Node, CastRes, mirror_coord, is_voxel_solid };

fn grid_raycast_impl(ray: Ray, res: ptr<function, CastRes>) {
    let pos = ray.pos + (*res).bounds.t_min * ray.dir;
    var voxel_coord = vec3u(pos); // this may move the ray back 1 voxel, but whatever
    var voxel_t = (vec3f(voxel_coord) - ray.pos) * ray.inv_dir;

    for (var i = 0u; i < GRID_MAX_ITER && vmax(voxel_t) < (*res).bounds.t_max; i++) {
        let mirror_voxel = mirror_coord(voxel_coord, OCTREE_DEPTH + 1u, ray.mirror);
        if is_voxel_solid(mirror_voxel) {
            (*res).iter += i;
            (*res).bounds.t_min = vmax(voxel_t);
            (*res).normal = -vec3f(cmpmax(voxel_t));
            (*res).voxel = voxel_coord;
            return;
        }
        else {
            let incr_mask = cmpmin(voxel_t + ray.inv_dir); // find which axis boundary is the closest
            voxel_t += vec3f(incr_mask) * ray.inv_dir;
            voxel_coord += vec3u(incr_mask);
        }
    }

    // end of iteration
    (*res).bounds.t_min = -1.0;
    (*res).iter += GRID_MAX_ITER;
    return;
}

