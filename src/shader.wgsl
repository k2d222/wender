// this module "requires":
// const OCTREE_DEPTH: u32; // depth = 0 for a 2^3 volume: depth = log2(n) - 1.
// const OCTREE_MAX_ITER: u32 // max number of hit tests in the octree per ray.
// const GRID_DEPTH: u32;
// const GRID_MAX_ITER: u32;
// const MSAA_LEVEL: u32 // msaa with 2^n probes, 0 to disable
// const DEBUG_DISPLAY: u32 // display ray complexity instead of color

@group(0) @binding(0)
var<uniform> cam: Camera;

@group(0) @binding(1)
var<uniform> lights: Lights;

@group(1) @binding(0)
var dvo: texture_3d<u32>;

@group(1) @binding(1)
var colors: texture_storage_3d<rgba8unorm, read>;

// provide functions to access the dvo, so octree can use it in an agnostic way.
fn octree_node(octant_coord: vec3u, dvo_depth: u32) -> u32 {
    return textureLoad(dvo, octant_coord, i32(textureNumLevels(dvo) - 1u - dvo_depth)).r; // BUG: the cast to i32 is a bug in naga afaik
}

fn is_octant_solid(node: u32, octant: vec3u) -> bool {
    let octant_index = dot(octant, vec3u(4u, 2u, 1u));
    let is_solid = bool(extractBits(node, octant_index, 1u));
    return is_solid;
}

fn is_voxel_solid(voxel_coord: vec3u) -> bool {
    let node_coord = voxel_coord / 2u;
    let octant = voxel_coord - node_coord * 2u;
    let node = textureLoad(dvo, voxel_coord, 0).r; // BUG: the cast to i32 is a bug in naga afaik
    return is_octant_solid(node, octant);
}

// preproc_include(octree.wgsl)

struct Camera {
    pos: vec3f,
    fov_y: f32,
    size: vec2f,
    aspect: f32,
    view_mat_inv: mat4x4f,
}

struct Lights {
    sun_dir: vec3f,
}

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
    let ambient_color = albedo.rgb * 0.01;
    let diffuse_color = pow(albedo.rgb, vec3f(2.2));
    let specular_color = vec3f(1.0, 1.0, 1.0) * 0.1;
    let shininess = 16.0;

    let view_dir = normalize(view_pos - hit_pos);
    let light_dir = lights.sun_dir;
    let half_vector = normalize(light_dir + view_dir);

    let res = raycast_octree(hit_pos + light_dir * 0.001, light_dir);

    var ambient_term = ambient_color;
    var diffuse_term = max(dot(hit_normal, light_dir), 0.0) * diffuse_color;
    var specular_term = pow(max(dot(hit_normal, half_vector), 0.0), shininess) * specular_color;

    if res.hit {
        ambient_term *= 2.0;
        diffuse_term *= 0.1;
        specular_term *= 0.1;
    }

    var shading_color = ambient_term + diffuse_term + specular_term;

    if !res.hit && res.iter == OCTREE_MAX_ITER {
        shading_color = vec3f(1.0, 1.0, 0.0);
    }

    return vec4f(saturate(shading_color), 1.0);
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

    let res = raycast_octree(cam.pos, ray_dir);

    // display ray complexity
    if DEBUG_DISPLAY == 1u {
        let complexity = f32(res.iter) / f32(OCTREE_MAX_ITER);
        if res.iter == OCTREE_MAX_ITER {
            return vec4f(0.0, 0.0, 1.0, 1.0);
        }
        else if res.hit {
            return vec4f(complexity, 0.0, 0.0, 1.0);
        }
        else {
            return vec4f(0.0, complexity, 0.0, 1.0);
        }
    }

    // display depth buffer
    else if DEBUG_DISPLAY == 2u {
        let max_t = f32(1u << textureNumLevels(dvo));
        var depth = 1.0 - saturate(res.t / max_t);
        depth = pow(depth, 2.0); // just to give more contrast to higher values
        return vec4f(vec3f(depth), 1.0);
    }

    if res.hit {
        let albedo = textureLoad(colors, res.voxel);
        var col = shade(albedo, cam.pos, res.pos, res.normal);
        // return col;

        // MSAA
        for (var i = 0u; i < MSAA_LEVEL * 2u; i++) {
            for (var j = 0u; j < MSAA_LEVEL * 2u; j++) {
                let pos = (2.0 * (vec2f(f32(i), f32(j)) - f32(MSAA_LEVEL)) - 1.0) / (4.0 * f32(MSAA_LEVEL * MSAA_LEVEL) - 1.0);
                let jitter = pos / cam.size;
                let ray_dir = cam_ray_dir(in.pos + jitter);
                let res = raycast_octree(cam.pos, ray_dir);
                let albedo = textureLoad(colors, res.voxel);
                col += shade(albedo, cam.pos, res.pos, res.normal);
            }
        }

        return col / (1.0 + f32(MSAA_LEVEL * MSAA_LEVEL * 4u));
    }

    else {
        var col = res.pos;
        return vec4f(col, 1.0);
    }
}
