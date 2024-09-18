import bindings/{ colors, linear_sampler, nearest_sampler };
import raycast/util/{ intersection };
import constants/{ SHADOW_MAX_ITER, SHADOW_CONE_ANGLE, OCTREE_DEPTH };

fn conetrace(ray_pos: vec3f, ray_dir: vec3f, tan_angle: f32, start_dist: f32, max_dist: f32) -> vec4f {
    var res = vec4f(0.0);

    let dist_incr = 0.5;
    var dist = start_dist;
    let size = vec3f(textureDimensions(colors, 0u));

    for (var i = 0u; i < SHADOW_MAX_ITER && dist <= max_dist; i++) {
        let pos = ray_pos + ray_dir * dist;
        let radius = tan_angle * dist;
        let sample = textureSampleLevel(colors, linear_sampler, pos / size, log2(radius));
        // let sample = textureSampleLevel(colors, colors_sampler, pos / size, 0.0);
        // this integration is incorrect because it does not take step size into account
        res = res + (1.0 - res.a) * sample;
        // dist += dist_incr;
        dist += radius + dist_incr;

        if res.a >= 1.0 {
            break;
        }
    }

    return saturate(res);
}

fn cone_spread(cone_angle: f32) -> f32 {
    return 2.0 * tan(cone_angle / 2.0 / 180.0 * 3.1415);
}

fn trace_shadow(ray_pos: vec3f, ray_dir: vec3f, start_dist: f32) -> f32 {
    let scene_width = f32(2u << OCTREE_DEPTH);
    let bounds = intersection(ray_pos, ray_dir, scene_width);

    let shadow_spread = cone_spread(f32(SHADOW_CONE_ANGLE));
    let sample = conetrace(ray_pos, ray_dir, shadow_spread, start_dist, bounds.t_max);
    return sample.a;
}

fn trace_ao(hit_pos: vec3f, hit_normal: vec3f) -> f32 {
    let pos = hit_pos + hit_normal * 0.5;
    let size = vec3f(textureDimensions(colors, 0u));
    let sample = textureSampleLevel(colors, linear_sampler, pos / size, 0.0);
    return sample.a;
}
