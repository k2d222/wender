fn vmin(vec: vec3f) -> f32 {
    return min(min(vec.x, vec.y), vec.z);
}

fn vmax(vec: vec3f) -> f32 {
    return max(max(vec.x, vec.y), vec.z);
}

fn cmpmin(vec: vec3f) -> vec3<bool> {
    return vec.xyz <= min(vec.yzx, vec.zxy);
}

fn cmpmax(vec: vec3f) -> vec3<bool> {
    return vec.xyz >= max(vec.yzx, vec.zxy);
}
