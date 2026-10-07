// Output encoding: the target's own format decides whether sRGB is applied here.

// Whether the render target stores linear values and the shader encodes sRGB
// itself. The renderer substitutes the placeholder when it builds the module:
// off for an sRGB target, on for the plain one the swapchain uses.
const encode_srgb: bool = ENCODE_SRGB;

// The sRGB encoding of linear `color`.
fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let low = color * 12.92;
    let high = 1.055 * pow(color, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, color <= vec3<f32>(0.0031308));
}

// Turns display-referred linear `color` into what the target stores.
fn encode_output(color: vec3<f32>) -> vec3<f32> {
    let clamped = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(clamped, linear_to_srgb(clamped), encode_srgb);
}
