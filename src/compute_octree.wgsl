import constants/{ texture_storage_read, texture_storage_write };

@group(0) @binding(0)
var voxels: texture_storage_read;

@group(0) @binding(1)
var dvo: texture_storage_write;

fn pack_octants(octants: array<bool, 8>) -> u32 {
    return
        (u32(octants[0]) << 0u) |
        (u32(octants[1]) << 1u) |
        (u32(octants[2]) << 2u) |
        (u32(octants[3]) << 3u) |
        (u32(octants[4]) << 4u) |
        (u32(octants[5]) << 5u) |
        (u32(octants[6]) << 6u) |
        (u32(octants[7]) << 7u);
}

// shader must be run dvo_depth times, for each depth level.
@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) index: vec3u) {
    let i2 = index * 2u;
    let octants = array(
        textureLoad(voxels, i2 + vec3(0u, 0u, 0u)).r != 0u,
        textureLoad(voxels, i2 + vec3(0u, 0u, 1u)).r != 0u,
        textureLoad(voxels, i2 + vec3(0u, 1u, 0u)).r != 0u,
        textureLoad(voxels, i2 + vec3(0u, 1u, 1u)).r != 0u,
        textureLoad(voxels, i2 + vec3(1u, 0u, 0u)).r != 0u,
        textureLoad(voxels, i2 + vec3(1u, 0u, 1u)).r != 0u,
        textureLoad(voxels, i2 + vec3(1u, 1u, 0u)).r != 0u,
        textureLoad(voxels, i2 + vec3(1u, 1u, 1u)).r != 0u,
    );

    let value = pack_octants(octants);
    textureStore(dvo, index, vec4(value));
}
