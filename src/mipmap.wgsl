@group(0) @binding(0)
var in_tex: texture_3d<f32>;

@group(0) @binding(1)
var tex_sampler: sampler;

@group(0) @binding(2)
var out_tex: texture_storage_3d<rgba8unorm, write>;

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) index: vec3u, @builtin(num_workgroups) size: vec3u) {
    let filter_pos = vec3f(index) / vec3f(size);
    // let filtered = textureSample(in_tex, tex_sampler, filter_pos);
    // textureStore(out_tex, index, filtered);
}
