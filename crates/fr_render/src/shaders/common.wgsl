// Shared by every 3D shader: constants and the layout of the per-object storage.

const PI: f32 = 3.14159265358979;

// The per-object storage is two matrices per object: the model matrix, then
// the inverse transpose of it for normals.
fn model_index(instance: u32) -> u32 {
    return instance * 2u;
}

// The index of the normal matrix of the object at `instance`.
fn normal_index(instance: u32) -> u32 {
    return instance * 2u + 1u;
}
