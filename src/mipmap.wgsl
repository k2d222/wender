@group(0) @binding(0)
var in_tex: texture_storage_3d<rgba8unorm, read>;

@group(0) @binding(1)
var out_tex: texture_storage_3d<rgba8unorm, write>;

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) index: vec3u, @builtin(num_workgroups) size: vec3u) {
    // let filter_pos = vec3f(index) / vec3f(size);
    // let filtered = textureSample(in_tex, tex_sampler, filter_pos);
    // unfortunately I cannot use a sampler in a compute shader (wgsl limitation),
    // and I cannot use a 3D texture as a render target (to use a fragment shader),
    // at least until https://github.com/gfx-rs/wgpu/issues/6040 lands.
    // so we are doing manual linear filtering here.
    // let filtered = textureLoad(coords)
    let t = 1.0 / 8.0;
    let filtered = textureLoad(in_tex, index * 2u + vec3u(0u, 0u, 0u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(0u, 0u, 1u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(0u, 1u, 0u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(0u, 1u, 1u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(1u, 0u, 0u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(1u, 0u, 1u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(1u, 1u, 0u)) * t
                 + textureLoad(in_tex, index * 2u + vec3u(1u, 1u, 1u)) * t;
    textureStore(out_tex, index, filtered);
}
