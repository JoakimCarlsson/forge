// Shadow lookups: three cascades for the sun and one layer per spot light or
// cube face of a point light, filtered with a 3x3 grid of hardware-filtered
// comparison taps. Surfaces are pushed along their normal by a few shadow
// texels first, which removes acne without the light leaking at contact points.

@group(0) @binding(3) var cascade_map: texture_depth_2d_array;
@group(0) @binding(4) var local_map: texture_depth_2d_array;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;

const SUN_NORMAL_OFFSET: f32 = 2.5;
const LOCAL_NORMAL_OFFSET: f32 = 3.0;
const LOCAL_DEPTH_BIAS: f32 = 0.00005;

// Component `index` of `v`, indexed through a copy of just the vector.
fn component(v: vec4<f32>, index: u32) -> f32 {
    return v[index];
}

// The fraction of light reaching `depth` at `uv` of `layer` in `map`, `texel` being one texel in uv units.
fn filter_shadow(map: texture_depth_2d_array, layer: i32, uv: vec2<f32>, depth: f32, texel: f32) -> f32 {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || depth > 1.0 {
        return 1.0;
    }
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            sum += textureSampleCompareLevel(map, shadow_sampler, uv + offset, layer, depth);
        }
    }
    return sum / 9.0;
}

// How much of the sun reaches `position`, whose view-space distance is `view_depth`.
fn sun_shadow(position: vec3<f32>, normal: vec3<f32>, n_dot_l: f32, view_depth: f32) -> f32 {
    var cascade = 3u;
    for (var i = 0u; i < 3u; i++) {
        if view_depth < component(lights.cascade_splits, i) {
            cascade = i;
            break;
        }
    }
    if cascade >= 3u {
        return 1.0;
    }
    let slope = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0));
    let offset = normal * component(lights.cascade_texel, cascade) * SUN_NORMAL_OFFSET * (0.5 + slope);
    let clip = lights.cascades[cascade] * vec4<f32>(position + offset, 1.0);
    let uv = clip.xy * vec2<f32>(0.5, -0.5) + 0.5;
    let texel = 1.0 / f32(textureDimensions(cascade_map).x);
    return filter_shadow(cascade_map, i32(cascade), uv, clip.z, texel);
}

// The cube face of a point light that `from_light`, the vector from the light, falls in.
fn cube_face(from_light: vec3<f32>) -> i32 {
    let a = abs(from_light);
    if a.x >= a.y && a.x >= a.z {
        return select(1, 0, from_light.x > 0.0);
    }
    if a.y >= a.z {
        return select(3, 2, from_light.y > 0.0);
    }
    return select(5, 4, from_light.z > 0.0);
}

// How much of local light `index` reaches `position`, `distance` away from it.
fn local_shadow(index: u32, position: vec3<f32>, normal: vec3<f32>, n_dot_l: f32, distance: f32) -> f32 {
    let first = i32(lights.local[index].shadow.x);
    if first < 0 {
        return 1.0;
    }
    let is_spot = lights.local[index].color.w > 0.5;
    let from_light = position - lights.local[index].position_range.xyz;
    let face = select(cube_face(from_light), 0, is_spot);
    let slope = sqrt(max(1.0 - n_dot_l * n_dot_l, 0.0));
    let offset = normal * lights.local[index].shadow.y * distance * LOCAL_NORMAL_OFFSET * (0.5 + slope);
    let clip = lights.local_shadows[first + face] * vec4<f32>(position + offset, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    let texel = 1.0 / f32(textureDimensions(local_map).x);
    return filter_shadow(local_map, first + face, uv, ndc.z - LOCAL_DEPTH_BIAS, texel);
}
