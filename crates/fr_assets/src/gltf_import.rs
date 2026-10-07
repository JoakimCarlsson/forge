//! Import of glTF 2.0 files, both `.gltf` with external or embedded data and `.glb`.
//!
//! Read: triangle primitives with positions, normals, tangents and the first
//! texture coordinate set; metallic-roughness materials with their five
//! textures and `KHR_materials_emissive_strength`; samplers; and the default
//! scene's node hierarchy, flattened to world transforms; and
//! `KHR_lights_punctual` lights, with their intensities passed through as
//! written. Skins are read only as a [`Skeleton`] of joint nodes (see
//! [`load_skeleton`]). Not read: vertex colours, further texture coordinate
//! sets, skinned meshes, weights, animations and cameras.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use fr_core::{
    Bone, DirectionalLight, Light, Mat4, PointLight, Skeleton, SkeletonError, SpotLight, Transform,
    Vec2, Vec3, Vec4,
};
use gltf::image::Format;
use gltf::mesh::Mode;

use crate::error::AssetError;
use crate::image::{Filter, ImageData, SamplerData, TextureData, Wrap};
use crate::material::{AlphaMode, MaterialData};
use crate::mesh::MeshData;
use crate::model::{ModelData, ModelPart};

/// The deepest node nesting accepted before the file is called malformed.
const MAX_NODE_DEPTH: usize = 256;

/// The glTF mesh primitives that became meshes: for each glTF mesh, the
/// `(mesh index, material index)` of each of its triangle primitives.
type PrimitiveTable = Vec<Vec<(usize, Option<usize>)>>;

/// Reads the glTF or GLB file at `path` into a [`ModelData`].
///
/// External buffers and images are resolved relative to the file; embedded ones
/// are decoded in place. Missing normals are generated from the triangles and
/// missing tangents from the texture coordinates.
///
/// # Errors
///
/// Returns [`AssetError`] when the file cannot be read or parsed, or when a
/// primitive has no positions or indexes outside its vertices.
pub fn load_gltf(path: impl AsRef<Path>) -> Result<ModelData, AssetError> {
    let path = path.as_ref();
    let (document, buffers, images) =
        gltf::import(path).map_err(|source| AssetError::import(path, source))?;

    let images = images
        .iter()
        .map(|data| convert_image(path, data))
        .collect::<Result<Vec<_>, _>>()?;
    let textures = document
        .textures()
        .map(|texture| TextureData {
            image: texture.source().index(),
            sampler: convert_sampler(&texture.sampler()),
        })
        .collect();
    let materials = document.materials().map(|m| convert_material(&m)).collect();

    let mut meshes = Vec::new();
    let mut table: PrimitiveTable = Vec::new();
    for mesh in document.meshes() {
        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            if let Some(data) = convert_primitive(path, &primitive, &buffers)? {
                primitives.push((meshes.len(), primitive.material().index()));
                meshes.push(data);
            }
        }
        table.push(primitives);
    }

    let mut placed = Placements::default();
    for root in root_nodes(&document) {
        collect_parts(path, &root, Mat4::IDENTITY, &table, 0, &mut placed)?;
    }

    Ok(ModelData {
        images,
        textures,
        materials,
        meshes,
        parts: placed.parts,
        lights: placed.lights,
    })
}

/// Reads the first skin of the glTF or GLB file at `path` as a [`Skeleton`], or
/// `None` when the file has no skin.
///
/// Only the document is parsed: buffers and images are not loaded.
///
/// # Errors
///
/// Returns [`AssetError`] when the file cannot be read or parsed, or when its
/// joints do not form a valid skeleton.
pub fn load_skeleton(path: impl AsRef<Path>) -> Result<Option<Skeleton>, AssetError> {
    let path = path.as_ref();
    let document = gltf::Gltf::open(path)
        .map_err(|source| AssetError::import(path, source))?
        .document;
    document
        .skins()
        .next()
        .map(|skin| {
            skeleton_from_skin(&document, &skin)
                .map_err(|error| AssetError::malformed(path, error.to_string()))
        })
        .transpose()
}

/// Turns the joint nodes of `skin` into a [`Skeleton`] in model space.
///
/// Bones keep the skin's joint order, except that a parent is moved ahead of its
/// first child when it comes later. A joint whose parent node is not a joint is a
/// root, and its bind pose includes every ancestor node above it. Names come
/// from the nodes, falling back to `bone{node index}`, and repeats get a
/// numeric suffix. Scale is dropped.
///
/// # Errors
///
/// Returns [`SkeletonError`] when the result is not a valid skeleton.
pub fn skeleton_from_skin(
    document: &gltf::Document,
    skin: &gltf::Skin<'_>,
) -> Result<Skeleton, SkeletonError> {
    let parents = node_parents(document);
    let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
    let joint_parent = |node: usize| {
        parents
            .get(&node)
            .copied()
            .filter(|parent| joints.contains(parent))
    };
    let order = parents_first(&joints, joint_parent);
    let slots: HashMap<usize, usize> = order
        .iter()
        .enumerate()
        .map(|(slot, &node)| (node, slot))
        .collect();
    let nodes: HashMap<usize, gltf::Node<'_>> =
        document.nodes().map(|node| (node.index(), node)).collect();
    let mut used = HashSet::new();
    let bones = order
        .iter()
        .filter_map(|node| nodes.get(node))
        .map(|node| {
            let parent = joint_parent(node.index()).and_then(|parent| slots.get(&parent).copied());
            Bone {
                name: unique_name(node, &mut used),
                parent,
                bind_local: bind_local(node, parent.is_some(), &parents, &nodes),
            }
        })
        .collect();
    Skeleton::new(bones)
}

/// Maps each node that is somebody's child to its parent.
fn node_parents(document: &gltf::Document) -> HashMap<usize, usize> {
    document
        .nodes()
        .flat_map(|node| {
            node.children()
                .map(|child| (child.index(), node.index()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `joints` in their own order with every joint placed after its parent joint.
fn parents_first(joints: &[usize], joint_parent: impl Fn(usize) -> Option<usize>) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::with_capacity(joints.len());
    for &joint in joints {
        let mut chain = Vec::new();
        let mut current = Some(joint);
        while let Some(node) = current.filter(|node| !order.contains(node)) {
            if chain.contains(&node) || chain.len() > joints.len() {
                break;
            }
            chain.push(node);
            current = joint_parent(node);
        }
        order.extend(chain.into_iter().rev());
    }
    order
}

/// A name for `node` that is not in `used`, which then includes it.
fn unique_name(node: &gltf::Node<'_>, used: &mut HashSet<String>) -> String {
    let base = node
        .name()
        .map_or_else(|| format!("bone{}", node.index()), str::to_owned);
    let mut name = base.clone();
    let mut suffix = 1;
    while !used.insert(name.clone()) {
        name = format!("{base}.{suffix:03}");
        suffix += 1;
    }
    name
}

/// The bind pose of a joint `node`: its own local transform when its parent is a
/// joint, else that composed with every ancestor node above it.
fn bind_local(
    node: &gltf::Node<'_>,
    parent_is_joint: bool,
    parents: &HashMap<usize, usize>,
    nodes: &HashMap<usize, gltf::Node<'_>>,
) -> Transform {
    let mut matrix = Mat4::from_cols_array_2d(&node.transform().matrix());
    let mut current = node.index();
    while let Some(ancestor) = parents
        .get(&current)
        .filter(|_| !parent_is_joint)
        .and_then(|parent| nodes.get(parent))
    {
        matrix = Mat4::from_cols_array_2d(&ancestor.transform().matrix()) * matrix;
        current = ancestor.index();
    }
    Transform::from_matrix(matrix).with_scale(Vec3::ONE)
}

/// What the node hierarchy places: mesh parts and lights.
#[derive(Default)]
struct Placements {
    /// The mesh parts, in model space.
    parts: Vec<ModelPart>,
    /// The lights, in model space.
    lights: Vec<Light>,
}

/// The nodes of the default scene, else of the first scene, else those that
/// no other node lists as a child.
fn root_nodes(document: &gltf::Document) -> Vec<gltf::Node<'_>> {
    if let Some(scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        return scene.nodes().collect();
    }
    let children: Vec<usize> = document
        .nodes()
        .flat_map(|node| {
            node.children()
                .map(|child| child.index())
                .collect::<Vec<_>>()
        })
        .collect();
    document
        .nodes()
        .filter(|node| !children.contains(&node.index()))
        .collect()
}

/// Adds the parts and lights of `node` and its descendants, whose parent sits at `parent`.
fn collect_parts(
    path: &Path,
    node: &gltf::Node<'_>,
    parent: Mat4,
    table: &PrimitiveTable,
    depth: usize,
    placed: &mut Placements,
) -> Result<(), AssetError> {
    if depth > MAX_NODE_DEPTH {
        return Err(AssetError::malformed(
            path,
            "the node hierarchy is too deep",
        ));
    }
    let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        for &(mesh, material) in &table[mesh.index()] {
            placed.parts.push(ModelPart {
                name: node.name().map(str::to_owned),
                mesh,
                material,
                transform: Transform::from_matrix(world),
            });
        }
    }
    if let Some(light) = node.light() {
        placed
            .lights
            .push(convert_light(&light).placed(&Transform::from_matrix(world)));
    }
    for child in node.children() {
        collect_parts(path, &child, world, table, depth + 1, placed)?;
    }
    Ok(())
}

/// Reads a light as it sits at its node's origin, shining down the node's negative Z axis.
fn convert_light(light: &gltf::khr_lights_punctual::Light<'_>) -> Light {
    use gltf::khr_lights_punctual::Kind;
    let color = Vec3::from_array(light.color());
    let intensity = light.intensity();
    let range = light.range().unwrap_or(0.0);
    match light.kind() {
        Kind::Directional => Light::Directional(DirectionalLight {
            direction: Vec3::NEG_Z,
            color,
            intensity,
            ..DirectionalLight::default()
        }),
        Kind::Point => Light::Point(PointLight {
            color,
            intensity,
            range,
            ..PointLight::default()
        }),
        Kind::Spot {
            inner_cone_angle,
            outer_cone_angle,
        } => Light::Spot(SpotLight {
            direction: Vec3::NEG_Z,
            color,
            intensity,
            range,
            inner_angle: inner_cone_angle,
            outer_angle: outer_cone_angle,
            ..SpotLight::default()
        }),
    }
}

/// Reads one primitive into a mesh, or `None` when it is not a triangle list,
/// strip or fan.
fn convert_primitive(
    path: &Path,
    primitive: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
) -> Result<Option<MeshData>, AssetError> {
    if !matches!(
        primitive.mode(),
        Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
    ) {
        return Ok(None);
    }
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|data| &data.0[..]));
    let positions: Vec<Vec3> = reader
        .read_positions()
        .ok_or_else(|| AssetError::malformed(path, "a primitive has no positions"))?
        .map(Vec3::from)
        .collect();
    let count = positions.len();

    let sequence = || (0..count as u32).collect::<Vec<_>>();
    let raw_indices = reader
        .read_indices()
        .map_or_else(sequence, |indices| indices.into_u32().collect());
    if raw_indices.iter().any(|&index| index as usize >= count) {
        return Err(AssetError::malformed(
            path,
            "a primitive indexes outside its vertices",
        ));
    }

    let normals: Option<Vec<Vec3>> = reader
        .read_normals()
        .map(|normals| normals.map(Vec3::from).collect())
        .filter(|normals: &Vec<Vec3>| normals.len() == count);
    let tangents: Option<Vec<Vec4>> = reader
        .read_tangents()
        .map(|tangents| tangents.map(Vec4::from).collect())
        .filter(|tangents: &Vec<Vec4>| tangents.len() == count);
    let uvs: Option<Vec<Vec2>> = reader
        .read_tex_coords(0)
        .map(|uvs| uvs.into_f32().map(Vec2::from).collect())
        .filter(|uvs: &Vec<Vec2>| uvs.len() == count);

    let mut mesh = MeshData {
        positions,
        normals: normals.clone().unwrap_or_default(),
        tangents: tangents.clone().unwrap_or_default(),
        uvs: uvs.unwrap_or_else(|| vec![Vec2::ZERO; count]),
        indices: triangulate(primitive.mode(), &raw_indices),
    };
    if normals.is_none() {
        mesh.compute_normals();
    }
    if tangents.is_none() {
        mesh.compute_tangents();
    }
    Ok(Some(mesh))
}

/// Turns the indices of a triangle list, strip or fan into a triangle list.
fn triangulate(mode: Mode, indices: &[u32]) -> Vec<u32> {
    match mode {
        Mode::TriangleStrip => indices
            .windows(3)
            .enumerate()
            .flat_map(|(index, w)| {
                if index % 2 == 0 {
                    [w[0], w[1], w[2]]
                } else {
                    [w[1], w[0], w[2]]
                }
            })
            .collect(),
        Mode::TriangleFan => indices
            .windows(2)
            .skip(1)
            .flat_map(|w| [indices[0], w[0], w[1]])
            .collect(),
        _ => indices[..indices.len() - indices.len() % 3].to_vec(),
    }
}

/// Reads one glTF material, whose texture slots become texture indices.
fn convert_material(material: &gltf::Material<'_>) -> MaterialData<usize> {
    let pbr = material.pbr_metallic_roughness();
    let emissive_strength = material.emissive_strength().unwrap_or(1.0);
    let alpha_mode = match material.alpha_mode() {
        gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
        gltf::material::AlphaMode::Mask => AlphaMode::Mask(material.alpha_cutoff().unwrap_or(0.5)),
        gltf::material::AlphaMode::Blend => AlphaMode::Blend,
    };
    MaterialData {
        name: material.name().map(str::to_owned),
        base_color: Vec4::from(pbr.base_color_factor()),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        emissive: Vec3::from(material.emissive_factor()) * emissive_strength,
        normal_scale: material.normal_texture().map_or(1.0, |t| t.scale()),
        occlusion_strength: material.occlusion_texture().map_or(1.0, |t| t.strength()),
        alpha_mode,
        double_sided: material.double_sided(),
        base_color_texture: pbr.base_color_texture().map(|info| info.texture().index()),
        metallic_roughness_texture: pbr
            .metallic_roughness_texture()
            .map(|info| info.texture().index()),
        normal_texture: material.normal_texture().map(|info| info.texture().index()),
        occlusion_texture: material
            .occlusion_texture()
            .map(|info| info.texture().index()),
        emissive_texture: material
            .emissive_texture()
            .map(|info| info.texture().index()),
    }
}

/// Reads one glTF sampler.
fn convert_sampler(sampler: &gltf::texture::Sampler<'_>) -> SamplerData {
    use gltf::texture::{MagFilter, MinFilter};
    SamplerData {
        magnify: match sampler.mag_filter() {
            Some(MagFilter::Nearest) => Filter::Nearest,
            Some(MagFilter::Linear) | None => Filter::Linear,
        },
        minify: match sampler.min_filter() {
            Some(
                MinFilter::Nearest
                | MinFilter::NearestMipmapNearest
                | MinFilter::NearestMipmapLinear,
            ) => Filter::Nearest,
            _ => Filter::Linear,
        },
        wrap_u: convert_wrap(sampler.wrap_s()),
        wrap_v: convert_wrap(sampler.wrap_t()),
    }
}

/// Reads one glTF wrapping mode.
fn convert_wrap(mode: gltf::texture::WrappingMode) -> Wrap {
    match mode {
        gltf::texture::WrappingMode::ClampToEdge => Wrap::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => Wrap::MirroredRepeat,
        gltf::texture::WrappingMode::Repeat => Wrap::Repeat,
    }
}

/// The channel count and the byte width of one channel of `format`.
fn layout(format: Format) -> (usize, usize) {
    match format {
        Format::R8 => (1, 1),
        Format::R8G8 => (2, 1),
        Format::R8G8B8 => (3, 1),
        Format::R8G8B8A8 => (4, 1),
        Format::R16 => (1, 2),
        Format::R16G16 => (2, 2),
        Format::R16G16B16 => (3, 2),
        Format::R16G16B16A16 => (4, 2),
        Format::R32G32B32FLOAT => (3, 4),
        Format::R32G32B32A32FLOAT => (4, 4),
    }
}

/// One channel of a decoded image as an 8-bit value.
fn channel_to_u8(format: Format, bytes: &[u8]) -> u8 {
    match layout(format).1 {
        1 => bytes[0],
        2 => (u16::from_ne_bytes([bytes[0], bytes[1]]) >> 8) as u8,
        _ => {
            let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        }
    }
}

/// Expands a decoded glTF image to four 8-bit channels.
///
/// A single channel is replicated to grey; a missing green or blue becomes zero
/// and a missing alpha becomes opaque.
fn convert_image(path: &Path, data: &gltf::image::Data) -> Result<ImageData, AssetError> {
    let (channels, width) = layout(data.format);
    let texel = channels * width;
    let expected = data.width as usize * data.height as usize * texel;
    if data.width == 0 || data.height == 0 || data.pixels.len() < expected {
        return Err(AssetError::malformed(path, "an image has too few pixels"));
    }
    let mut pixels = Vec::with_capacity(data.width as usize * data.height as usize * 4);
    for source in data.pixels[..expected].chunks_exact(texel) {
        let channel =
            |index: usize| channel_to_u8(data.format, &source[index * width..(index + 1) * width]);
        let rgba = match channels {
            1 => [channel(0), channel(0), channel(0), 255],
            2 => [channel(0), channel(1), 0, 255],
            3 => [channel(0), channel(1), channel(2), 255],
            _ => [channel(0), channel(1), channel(2), channel(3)],
        };
        pixels.extend_from_slice(&rgba);
    }
    Ok(ImageData {
        width: data.width,
        height: data.height,
        pixels,
    })
}
