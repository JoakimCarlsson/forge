// Metallic-roughness shading: Cook-Torrance with a GGX distribution, a
// height-correlated Smith visibility term and Schlick's Fresnel, an analytic environment BRDF and the
// ACES filmic tonemap. All in linear light.

const MIN_ROUGHNESS: f32 = 0.045;

// Everything the lighting needs to know about one shaded point.
struct Surface {
    position: vec3<f32>,
    normal: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
    emissive: vec3<f32>,
}

// The GGX normal distribution at half-vector alignment `n_dot_h`.
fn distribution_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Smith's height-correlated masking-shadowing divided by 4 n.l n.v.
fn visibility_smith_ggx(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(v + l, 1e-5);
}

// Schlick's approximation of Fresnel reflectance from `f0` at `v_dot_h`.
fn fresnel_schlick(f0: vec3<f32>, v_dot_h: f32) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
}

// Karis' analytic fit of the split-sum environment BRDF: how much of a
// uniform environment a surface of `f0` and `roughness` reflects.
fn environment_brdf(f0: vec3<f32>, n_dot_v: f32, roughness: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// A surface prepared for lighting: everything the BRDF needs that does not
// depend on the light.
struct Shading {
    n: vec3<f32>,
    v: vec3<f32>,
    n_dot_v: f32,
    roughness: f32,
    alpha: f32,
    f0: vec3<f32>,
    diffuse_color: vec3<f32>,
}

// Prepares `surface` for lighting as seen from `eye`.
fn prepare_shading(surface: Surface, eye: vec3<f32>) -> Shading {
    let n = surface.normal;
    let v = normalize(eye - surface.position);
    let roughness = clamp(surface.roughness, MIN_ROUGHNESS, 1.0);
    return Shading(
        n,
        v,
        max(dot(n, v), 1e-4),
        roughness,
        roughness * roughness,
        mix(vec3<f32>(0.04), surface.albedo, surface.metallic),
        surface.albedo * (1.0 - surface.metallic),
    );
}

// The radiance reflected towards the viewer from light of colour `radiance`
// arriving along `l`, the unit vector towards the light.
fn light_response(s: Shading, l: vec3<f32>, radiance: vec3<f32>) -> vec3<f32> {
    let n_dot_l = max(dot(s.n, l), 0.0);
    let h = normalize(l + s.v);
    let n_dot_h = clamp(dot(s.n, h), 0.0, 1.0);
    let v_dot_h = clamp(dot(s.v, h), 0.0, 1.0);
    let fresnel = fresnel_schlick(s.f0, v_dot_h);
    let specular = fresnel
        * distribution_ggx(n_dot_h, s.alpha)
        * visibility_smith_ggx(s.n_dot_v, n_dot_l, s.alpha);
    let diffuse = (vec3<f32>(1.0) - fresnel) * s.diffuse_color / PI;
    return (diffuse + specular) * radiance * n_dot_l;
}

// Narkowicz's fit of the ACES filmic curve, mapping linear radiance to 0..1.
fn tonemap_aces(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}
