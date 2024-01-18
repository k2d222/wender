struct SvoNode {
    octants: array<u32, 8>,
}

@group(0) @binding(0)
var in_tex: texture_3d<u32>;

@group(0) @binding(1)
var<storage, read_write> svo: array<SvoNode>;

@group(0) @binding(2)
var<storage, read_write> svo_depth: u32;

fn vmax(vec: vec3u) -> u32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn svo_ptr(index: u32) -> u32 {
    let node = svo[index];
    let full = is_full(node);
    return u32(full) * index;
}

fn is_full(node: SvoNode) -> bool {
    let full =
        node.octants[0] != 0u ||
        node.octants[1] != 0u ||
        node.octants[2] != 0u ||
        node.octants[3] != 0u ||
        node.octants[4] != 0u ||
        node.octants[5] != 0u ||
        node.octants[6] != 0u ||
        node.octants[7] != 0u;
    return full;
}

@compute @workgroup_size(1)
fn cs_main(
    @builtin(global_invocation_id) index: vec3u
) {
    let vox_dim = textureDimensions(in_tex).x;
    var dim = vox_dim / 2u; // curent width of the octree layer
    let svo_depth = firstTrailingBit(dim); // depth = 1 for a 2^3 volume.
    var offset = ((1u << (3u * svo_depth)) - 1u) / 7u;
    var indexer = vec3(dim * dim, dim, 1u);

    // if vmax(index) >= dim {
    //     return;
    // }

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

    svo[offset + dot(index, indexer)] = SvoNode(octants);

    let work_to_do = svo_depth - min(svo_depth, firstLeadingBit((vmax(index) << 1u) + 1u));

    // if work_to_do != 0u && any(index != vec3(0u)) {
    //     svo[0u] = SvoNode(array(1u, 2u, 3u, 4u, 5u, 6u, 7u, 8u));
    // }

    for (var k = 0u; k < work_to_do; k++) {
        storageBarrier();

        let octants = array(
            svo_ptr(offset + dot(i2 + vec3(0u, 0u, 0u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(0u, 0u, 1u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(0u, 1u, 0u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(0u, 1u, 1u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(1u, 0u, 0u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(1u, 0u, 1u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(1u, 1u, 0u), indexer)),
            svo_ptr(offset + dot(i2 + vec3(1u, 1u, 1u), indexer)),
        );

        dim /= 2u;
        offset -= dim * dim * dim;
        indexer = vec3(dim * dim, dim, 1u);
        svo[offset + dot(index, indexer)] = SvoNode(octants);
    }
}
