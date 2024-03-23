@group(0) @binding(0)
var in_tex: texture_3d<u32>;

@group(0) @binding(1)
var<storage, read_write> dvo: array<u32>;

fn vmax(vec: vec3u) -> u32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn dvo_ptr(index: u32) -> u32 {
    let node = dvo[index];
    return u32(node != 0u) * index;
}

@compute @workgroup_size(1)
fn cs_main(
    @builtin(global_invocation_id) index: vec3u
) {
    let vox_dim = textureDimensions(in_tex).x;
    var dim = vox_dim / 2u; // current width of the octree layer
    let dvo_depth = firstTrailingBit(dim); // depth = 0 for a 2^3 volume.
    var offset = ((1u << (3u * dvo_depth)) - 1u) / 7u; // offset in the dvo array of the current dvo depth layer. Computed as sum(2^(3i), i=0->dvo_depth)
    var indexer = vec3(dim * dim, dim, 1u);

    let i2 = index * 2u;
    let octants = array(
        textureLoad(in_tex, i2 + vec3(0u, 0u, 0u), 0).r,
        textureLoad(in_tex, i2 + vec3(0u, 0u, 1u), 0).r,
        textureLoad(in_tex, i2 + vec3(0u, 1u, 0u), 0).r,
        textureLoad(in_tex, i2 + vec3(0u, 1u, 1u), 0).r,
        textureLoad(in_tex, i2 + vec3(1u, 0u, 0u), 0).r,
        textureLoad(in_tex, i2 + vec3(1u, 0u, 1u), 0).r,
        textureLoad(in_tex, i2 + vec3(1u, 1u, 0u), 0).r,
        textureLoad(in_tex, i2 + vec3(1u, 1u, 1u), 0).r,
    );

    dvo[offset + dot(index, indexer)] =
        u32(octants[0] != 0u) << 0u |
        u32(octants[1] != 0u) << 1u |
        u32(octants[2] != 0u) << 2u |
        u32(octants[3] != 0u) << 3u |
        u32(octants[4] != 0u) << 4u |
        u32(octants[5] != 0u) << 5u |
        u32(octants[6] != 0u) << 6u |
        u32(octants[7] != 0u) << 7u;
    // dvo[offset + dot(index, indexer)] = 0x69u;

    let work_to_do = dvo_depth - min(dvo_depth, firstLeadingBit((vmax(index) << 1u) + 1u));

    // if work_to_do != 0u && any(index != vec3(0u)) {
    //     svo[0u] = SvoNode(array(1u, 2u, 3u, 4u, 5u, 6u, 7u, 8u));
    // }

    for (var k = 0u; k < work_to_do; k++) {
        storageBarrier();

        let octants = array(
            dvo_ptr(offset + dot(i2 + vec3(0u, 0u, 0u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(0u, 0u, 1u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(0u, 1u, 0u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(0u, 1u, 1u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(1u, 0u, 0u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(1u, 0u, 1u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(1u, 1u, 0u), indexer)),
            dvo_ptr(offset + dot(i2 + vec3(1u, 1u, 1u), indexer)),
        );

        dim /= 2u;
        offset -= dim * dim * dim;
        indexer = vec3(dim * dim, dim, 1u);
        dvo[offset + dot(index, indexer)] = 
            u32(octants[0] != 0u) << 0u |
            u32(octants[1] != 0u) << 1u |
            u32(octants[2] != 0u) << 2u |
            u32(octants[3] != 0u) << 3u |
            u32(octants[4] != 0u) << 4u |
            u32(octants[5] != 0u) << 5u |
            u32(octants[6] != 0u) << 6u |
            u32(octants[7] != 0u) << 7u;
        // dvo[offset + dot(index, indexer)] = 0xFFu;
    }
}
