import util/{ vmin, vmax, cmpmin, cmpmax };
import bindings/{ colors, dvo, svo };
import constants/{ OCTREE_DEPTH };
import ./util/{ intersection, Ray, Hit, no_hit, CastRes, mirror_coord };
import ./dvo/{ dvo_raycast_impl };
import ./svo/{ svo_raycast_impl };
import ./grid/{ grid_raycast_impl };

fn raycast_start(ray: Ray, res: ptr<function, CastRes>) {
    // svo_raycast_impl(ray, res);
    dvo_raycast_impl(ray, res);
}

fn svo_raycast_continue(ray: Ray, res: ptr<function, CastRes>) {
    dvo_raycast_impl(ray, res);
}

fn dvo_raycast_continue(ray: Ray, res: ptr<function, CastRes>) {
    grid_raycast_impl(ray, res);
}

fn grid_raycast_continue(ray: Ray, res: ptr<function, CastRes>) {
}

// exported function
fn raycast(ray_pos: vec3f, ray_dir: vec3f) -> Hit {
    let scene_width = f32(2u << OCTREE_DEPTH);
    let bounds = intersection(ray_pos, ray_dir, scene_width);

    // no hit;
    if bounds.t_min > bounds.t_max || bounds.t_max < 0.0 {
        return no_hit(0u);
    }

    var ray: Ray;

    // handle symmetries: ray is mirrored to go in the positive direction.
    // use mirror_coord() to find un-mirrored positions.
    // real position is needed when sampling the octree: in get_dvo_node() and is_octant_solid().
    ray.mirror = vec3u(ray_dir < vec3f(0.0));
    ray.pos = ray_pos * vec3f(1u - ray.mirror) + (f32(2u << OCTREE_DEPTH) - ray_pos) * vec3f(ray.mirror);
    ray.dir = abs(ray_dir);
    ray.inv_dir = 1.0 / ray.dir;
    ray.bounds = bounds;

    var res: CastRes;
    raycast_start(ray, &res);

    var hit: Hit;
    hit.iter = res.iter;
    hit.t = res.bounds.t_min;
    hit.pos = ray_pos + res.bounds.t_min * ray_dir;
    hit.voxel = mirror_coord(res.voxel, OCTREE_DEPTH + 1u, ray.mirror);
    hit.normal = res.normal * sign(ray_dir);
    return hit;
}
