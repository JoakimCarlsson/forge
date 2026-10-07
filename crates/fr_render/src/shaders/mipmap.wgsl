// Downsamples one mip level into the next with a linear filter.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Fragment {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Fragment(
        vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0),
        vec2<f32>(corner.x, 1.0 - corner.y),
    );
}

@fragment
fn fragment(input: Fragment) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, input.uv);
}
