// The forward mesh pipeline: per-object transforms come from the storage
// buffer indexed by the instance, materials from group 1.

struct Material {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    metallic: f32,
    roughness: f32,
    normal_scale: f32,
    occlusion_strength: f32,
    alpha_cutoff: f32,
    // 0 opaque, 1 mask, 2 blend.
    alpha_mode: u32,
    padding: vec2<u32>,
}

@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var base_color_texture: texture_2d<f32>;
@group(1) @binding(2) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(3) var normal_texture: texture_2d<f32>;
@group(1) @binding(4) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(5) var emissive_texture: texture_2d<f32>;
@group(1) @binding(6) var base_color_sampler: sampler;
@group(1) @binding(7) var metallic_roughness_sampler: sampler;
@group(1) @binding(8) var normal_sampler: sampler;
@group(1) @binding(9) var occlusion_sampler: sampler;
@group(1) @binding(10) var emissive_sampler: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

@vertex
fn vertex(input: Vertex, @builtin(instance_index) instance: u32) -> Fragment {
    let world = objects[model_index(instance)] * vec4<f32>(input.position, 1.0);
    return Fragment(
        camera.view_proj * world,
        world.xyz,
        (objects[normal_index(instance)] * vec4<f32>(input.normal, 0.0)).xyz,
        vec4<f32>((objects[model_index(instance)] * vec4<f32>(input.tangent.xyz, 0.0)).xyz, input.tangent.w),
        input.uv,
    );
}

// The shading normal: the interpolated one bent by the normal texture and
// flipped to face the viewer on a back face.
fn shading_normal(input: Fragment, front: bool, tangent_normal: vec3<f32>) -> vec3<f32> {
    let side = select(-1.0, 1.0, front);
    let n = normalize(input.normal) * side;
    let t = normalize(input.tangent.xyz - n * dot(n, input.tangent.xyz));
    let b = cross(n, t) * input.tangent.w;
    let scaled = vec3<f32>(tangent_normal.xy * material.normal_scale, tangent_normal.z);
    return normalize(t * scaled.x + b * scaled.y + n * scaled.z);
}

@fragment
fn fragment(input: Fragment, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let base = textureSample(base_color_texture, base_color_sampler, input.uv) * material.base_color;
    let metal_rough = textureSample(metallic_roughness_texture, metallic_roughness_sampler, input.uv);
    let tangent_normal = textureSample(normal_texture, normal_sampler, input.uv).xyz * 2.0 - 1.0;
    let occlusion = textureSample(occlusion_texture, occlusion_sampler, input.uv).r;
    let emissive = textureSample(emissive_texture, emissive_sampler, input.uv).rgb;

    if material.alpha_mode == 1u && base.a < material.alpha_cutoff {
        discard;
    }

    let surface = Surface(
        input.world_position,
        shading_normal(input, front, tangent_normal),
        base.rgb,
        clamp(material.metallic * metal_rough.b, 0.0, 1.0),
        clamp(material.roughness * metal_rough.g, 0.0, 1.0),
        1.0 + material.occlusion_strength * (occlusion - 1.0),
        emissive * material.emissive.rgb,
    );

    let radiance = shade(surface) * camera.position.w;
    let alpha = select(1.0, base.a, material.alpha_mode == 2u);
    return vec4<f32>(encode_output(tonemap_aces(radiance)), alpha);
}
