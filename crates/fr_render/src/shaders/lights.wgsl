// The frame's camera and lights, and the loop that adds every light's
// contribution to a surface: directional lights, then point and spot lights
// with inverse-square falloff, a smooth range window and a smooth cone edge,
// then the sky-and-ground ambient term.

struct Camera {
    view_proj: mat4x4<f32>,
    // The world position in xyz and the exposure multiplier in w.
    position: vec4<f32>,
    // The unit view direction in xyz.
    forward: vec4<f32>,
}

struct DirectionalLight {
    // The unit vector from the surface towards the light, in xyz.
    direction: vec4<f32>,
    // The linear colour times the illuminance, in rgb.
    color: vec4<f32>,
}

struct LocalLight {
    // The world position in xyz and the range in w, zero for unlimited.
    position_range: vec4<f32>,
    // The linear colour times the intensity in rgb; w is 1 for a spot light.
    color: vec4<f32>,
    // The unit direction a spot light points in xyz and its cone scale in w.
    direction: vec4<f32>,
    // The first shadow layer in x, or -1 for none; the size of one shadow
    // texel at unit distance in y; the cone offset in z.
    shadow: vec4<f32>,
}

struct Lights {
    // The linear sky and ground colours times the ambient intensity.
    sky: vec4<f32>,
    ground: vec4<f32>,
    // The number of directional lights in x, of local lights in y, and one
    // plus the index of the directional light casting shadows, or zero, in z.
    counts: vec4<u32>,
    // The far view distance of each cascade.
    cascade_splits: vec4<f32>,
    // The world size of one texel of each cascade.
    cascade_texel: vec4<f32>,
    cascades: array<mat4x4<f32>, 3>,
    directional: array<DirectionalLight, 4>,
    local: array<LocalLight, 16>,
    local_shadows: array<mat4x4<f32>, 12>,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> lights: Lights;
@group(0) @binding(2) var<storage, read> objects: array<mat4x4<f32>>;

const MIN_DISTANCE_SQUARED: f32 = 0.0001;

// Inverse-square falloff of light at squared distance `distance2`, faded to
// zero smoothly at `range` unless that is zero.
fn distance_attenuation(distance2: f32, range: f32) -> f32 {
    var attenuation = 1.0 / max(distance2, MIN_DISTANCE_SQUARED);
    if range > 0.0 {
        let ratio = distance2 / (range * range);
        let window = clamp(1.0 - ratio * ratio, 0.0, 1.0);
        attenuation *= window * window;
    }
    return attenuation;
}

// The smooth falloff across a spot light's cone, given the cosine of the angle off its axis.
fn cone_attenuation(index: u32, cos_angle: f32) -> f32 {
    let t = clamp(cos_angle * lights.local[index].direction.w + lights.local[index].shadow.z, 0.0, 1.0);
    return t * t;
}

// The ambient colour arriving from the side `direction` points to.
fn hemisphere(direction: vec3<f32>) -> vec3<f32> {
    return mix(lights.ground.rgb, lights.sky.rgb, direction.y * 0.5 + 0.5);
}

// The radiance leaving `surface` towards the camera.
fn shade(surface: Surface) -> vec3<f32> {
    let s = prepare_shading(surface, camera.position.xyz);
    let view_depth = dot(surface.position - camera.position.xyz, camera.forward.xyz);

    var radiance = vec3<f32>(0.0);
    for (var i = 0u; i < min(lights.counts.x, 4u); i++) {
        let l = lights.directional[i].direction.xyz;
        let n_dot_l = dot(s.n, l);
        if n_dot_l <= 0.0 {
            continue;
        }
        var visibility = 1.0;
        if lights.counts.z == i + 1u {
            visibility = sun_shadow(surface.position, s.n, n_dot_l, view_depth);
        }
        radiance += light_response(s, l, lights.directional[i].color.rgb * visibility);
    }

    for (var i = 0u; i < min(lights.counts.y, 16u); i++) {
        let to_light = lights.local[i].position_range.xyz - surface.position;
        let distance2 = dot(to_light, to_light);
        let distance = sqrt(distance2);
        let l = to_light / max(distance, 1e-4);
        let n_dot_l = dot(s.n, l);
        if n_dot_l <= 0.0 {
            continue;
        }
        var attenuation = distance_attenuation(distance2, lights.local[i].position_range.w);
        if lights.local[i].color.w > 0.5 {
            attenuation *= cone_attenuation(i, dot(lights.local[i].direction.xyz, -l));
        }
        if attenuation <= 0.0 {
            continue;
        }
        let visibility = local_shadow(i, surface.position, s.n, n_dot_l, distance);
        radiance += light_response(s, l, lights.local[i].color.rgb * attenuation * visibility);
    }

    let reflected = reflect(-s.v, s.n);
    let diffuse_light = hemisphere(s.n);
    let specular_light = mix(hemisphere(reflected), diffuse_light, s.roughness);
    let ambient = surface.occlusion
        * (diffuse_light * s.diffuse_color + specular_light * environment_brdf(s.f0, s.n_dot_v, s.roughness));
    return radiance + ambient + surface.emissive;
}
