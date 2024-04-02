@group(0) @binding(0)
var in_tex: texture_3d<u32>;

@group(0) @binding(1)
var<storage, read_write> dvo: array<u32>;

fn vmax(vec: vec3u) -> u32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn pack_octants(octants: array<bool, 8>) -> u32 {
    return
        u32(octants[0]) << 0u |
        u32(octants[1]) << 1u |
        u32(octants[2]) << 2u |
        u32(octants[3]) << 3u |
        u32(octants[4]) << 4u |
        u32(octants[5]) << 5u |
        u32(octants[6]) << 6u |
        u32(octants[7]) << 7u;
}

// because of workgroup size, in_tex size must be at least 4*4*4 * 2*2*2.
// 4*4*4 must respect the wgpu::Limits. (256 for webgpu).
// shader must be run n times to ensure synchronization. n = ???.
@compute @workgroup_size(4, 4, 4)
fn cs_main(
    @builtin(global_invocation_id) index: vec3u
) {
    var dim = textureDimensions(in_tex).x / 2u; // current width of the octree layer
    let dvo_depth = firstTrailingBit(dim); // depth = 0 for a 2^3 volume.
    var offset = ((1u << (3u * dvo_depth)) - 1u) / 7u; // offset in the dvo array of the current dvo depth layer. Computed as sum(2^(3i), i=0->dvo_depth)
    var indexer = vec3(dim * dim, dim, 1u);

    let i2 = index * 2u;
    let octants = array(
        textureLoad(in_tex, i2 + vec3(0u, 0u, 0u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(0u, 0u, 1u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(0u, 1u, 0u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(0u, 1u, 1u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(1u, 0u, 0u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(1u, 0u, 1u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(1u, 1u, 0u), 0).r != 0u,
        textureLoad(in_tex, i2 + vec3(1u, 1u, 1u), 0).r != 0u,
    );

    dvo[offset + dot(index, indexer)] = pack_octants(octants);

    // first, all threads work, then 1/8th of them, ... until depth=0
    let work_to_do = dvo_depth - min(dvo_depth, firstLeadingBit((vmax(index) << 1u) + 1u));

    for (var k = 0u; k < work_to_do; k++) {

        storageBarrier();

        let octants = array(
            dvo[offset + dot(i2 + vec3(0u, 0u, 0u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(0u, 0u, 1u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(0u, 1u, 0u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(0u, 1u, 1u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(1u, 0u, 0u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(1u, 0u, 1u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(1u, 1u, 0u), indexer)] != 0u,
            dvo[offset + dot(i2 + vec3(1u, 1u, 1u), indexer)] != 0u,
        );

        dim /= 2u;
        offset -= dim * dim * dim;
        indexer = vec3(dim * dim, dim, 1u);

        dvo[offset + dot(index, indexer)] = pack_octants(octants);
    }
}
