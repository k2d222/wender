@group(1) @binding(0)
var<storage> svo: array<u32>;

@group(1) @binding(1)
var dvo: texture_3d<u32>;

@group(1) @binding(2)
var colors: texture_3d<f32>;

@group(1) @binding(3)
var linear_sampler: sampler;

@group(1) @binding(4)
var nearest_sampler: sampler;

