// The depth-only vertex stage the shadow maps are rendered with: the same
// per-object storage as the mesh pipeline, seen from a light.

@group(0) @binding(0) var<uniform> light_view_proj: mat4x4<f32>;
@group(0) @binding(1) var<storage, read> objects: array<mat4x4<f32>>;

@vertex
fn vertex(
    @location(0) position: vec3<f32>,
    @builtin(instance_index) instance: u32,
) -> @builtin(position) vec4<f32> {
    let model = objects[model_index(instance)];
    return light_view_proj * (model * vec4<f32>(position, 1.0));
}
