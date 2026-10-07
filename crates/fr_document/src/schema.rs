//! The data half of a registered component type: its properties with defaults,
//! multiplicity, and the requirement and conflict rules between types.

use fr_math::Vec3;

use crate::guid::Guid;
use crate::property::{AssetReference, Color, PropertyType, PropertyValue};

/// The FNV-1a offset basis.
const FNV_OFFSET: u64 = 1_469_598_103_934_665_603;

/// The FNV-1a prime.
const FNV_PRIME: u64 = 1_099_511_628_211;

/// One option of an enum property.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumOption {
    /// The serialised key.
    pub key: String,
    /// The display label.
    pub label: String,
}

/// The description of one property of a component type.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySchema {
    /// The property key.
    pub key: String,
    /// The display label.
    pub label: String,
    /// The type of the value.
    pub kind: PropertyType,
    /// The value a new component starts with.
    pub default_value: PropertyValue,
    /// The options of an enum.
    pub options: Vec<EnumOption>,
    /// The type of the elements of an array.
    pub element_type: PropertyType,
    /// The most elements an array may hold; zero for no limit.
    pub max_items: u32,
    /// The least value of a number; equal to the maximum for no limit.
    pub minimum: f32,
    /// The greatest value of a number; equal to the minimum for no limit.
    pub maximum: f32,
    /// The editing step of a number.
    pub step: f32,
}

impl PropertySchema {
    /// A property of a type that starts at `default`, labelled with its key.
    pub fn new(key: &str, default: PropertyValue) -> Self {
        Self {
            key: key.to_owned(),
            label: key.to_owned(),
            kind: default.kind(),
            default_value: default,
            options: Vec::new(),
            element_type: PropertyType::Float,
            max_items: 0,
            minimum: 0.0,
            maximum: 0.0,
            step: 0.01,
        }
    }

    /// A boolean property.
    pub fn boolean(key: &str, default: bool) -> Self {
        Self::new(key, PropertyValue::Bool(default))
    }

    /// An integer property.
    pub fn integer(key: &str, default: i64) -> Self {
        Self::new(key, PropertyValue::Int(default))
    }

    /// A float property.
    pub fn float(key: &str, default: f32) -> Self {
        Self::new(key, PropertyValue::Float(default))
    }

    /// A text property.
    pub fn text(key: &str, default: &str) -> Self {
        Self::new(key, PropertyValue::String(default.to_owned()))
    }

    /// A vector property.
    pub fn vec3(key: &str, default: Vec3) -> Self {
        Self::new(key, PropertyValue::Vec3(default))
    }

    /// A colour property.
    pub fn color(key: &str, default: Color) -> Self {
        Self::new(key, PropertyValue::Color(default))
    }

    /// An unset asset reference property.
    pub fn asset(key: &str) -> Self {
        Self::new(key, PropertyValue::Asset(AssetReference::default()))
    }

    /// An unset entity reference property.
    pub fn entity(key: &str) -> Self {
        Self::new(key, PropertyValue::of_kind(PropertyType::Entity))
    }

    /// An enum property over `(key, label)` options that starts at `default`.
    pub fn enumeration(key: &str, options: &[(&str, &str)], default: &str) -> Self {
        Self {
            options: options
                .iter()
                .map(|(option, label)| EnumOption {
                    key: (*option).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            ..Self::new(key, PropertyValue::Enum(default.to_owned()))
        }
    }

    /// The same property limited to a range of values.
    pub fn range(mut self, minimum: f32, maximum: f32) -> Self {
        self.minimum = minimum;
        self.maximum = maximum;
        self
    }

    /// The same property with a display label.
    pub fn labelled(mut self, label: &str) -> Self {
        label.clone_into(&mut self.label);
        self
    }
}

/// Where a component type comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComponentSource {
    /// Registered by the engine.
    #[default]
    Engine,
    /// Registered by a game as a behaviour.
    Game,
}

/// The description of a component type.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentSchema {
    /// The registered type key.
    pub key: String,
    /// The display label.
    pub label: String,
    /// The category the editor files it under.
    pub category: String,
    /// The version properties are written against.
    pub schema_version: u32,
    /// Who registered the type.
    pub source: ComponentSource,
    /// Whether an entity may hold several.
    pub allow_multiple: bool,
    /// Whether the editor may remove it.
    pub removable: bool,
    /// Whether the data lives in the entity record itself, never as a component
    /// record: the type only describes it.
    pub intrinsic: bool,
    /// Types the same entity must also hold.
    pub required_types: Vec<String>,
    /// Types the same entity must not hold.
    pub conflicting_types: Vec<String>,
    /// The properties in display order.
    pub properties: Vec<PropertySchema>,
}

impl Default for ComponentSchema {
    /// An engine type at schema version one that may be removed and holds no
    /// properties.
    fn default() -> Self {
        Self {
            key: String::new(),
            label: String::new(),
            category: String::new(),
            schema_version: 1,
            source: ComponentSource::Engine,
            allow_multiple: false,
            removable: true,
            intrinsic: false,
            required_types: Vec::new(),
            conflicting_types: Vec::new(),
            properties: Vec::new(),
        }
    }
}

impl ComponentSchema {
    /// The description of a property.
    pub fn find_property(&self, key: &str) -> Option<&PropertySchema> {
        self.properties.iter().find(|property| property.key == key)
    }
}

/// The component types an authoring or play session knows, in registration
/// order.
#[derive(Clone, Debug, Default)]
pub struct SchemaSet {
    /// The types in registration order.
    types: Vec<ComponentSchema>,
}

impl SchemaSet {
    /// A set holding no types.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Registers a type, replacing the description of a type with the same key
    /// in place.
    pub fn add(&mut self, schema: ComponentSchema) {
        match self
            .types
            .iter_mut()
            .find(|existing| existing.key == schema.key)
        {
            Some(existing) => *existing = schema,
            None => self.types.push(schema),
        }
    }

    /// The description of a type.
    pub fn find(&self, key: &str) -> Option<&ComponentSchema> {
        self.types.iter().find(|schema| schema.key == key)
    }

    /// Every registered type in registration order.
    pub fn types(&self) -> &[ComponentSchema] {
        &self.types
    }

    /// Removes every type from a source.
    pub fn remove_source(&mut self, source: ComponentSource) {
        self.types.retain(|schema| schema.source != source);
    }

    /// A short digest of the types a document is written against: keys,
    /// versions, multiplicity, property keys and types and enum options,
    /// independent of registration order.
    pub fn fingerprint(&self) -> String {
        let mut lines: Vec<String> = self
            .types
            .iter()
            .map(|schema| {
                let multiplicity = if schema.allow_multiple { "m" } else { "s" };
                let mut line = format!("{}:{}:{multiplicity}", schema.key, schema.schema_version);
                for property in &schema.properties {
                    line.push_str(&format!("|{}={}", property.key, property.kind.key()));
                    for option in &property.options {
                        line.push(',');
                        line.push_str(&option.key);
                    }
                }
                line
            })
            .collect();
        lines.sort();
        hex_digest(lines.iter().map(String::as_str))
    }
}

/// The FNV-1a digest of lines, each followed by a newline, as 16 hexadecimal
/// digits.
fn hex_digest<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut hash = FNV_OFFSET;
    for line in lines {
        for byte in line.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= u64::from(b'\n');
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    /// The document loads; the data is inert or ignored.
    Warning,
    /// The document breaks a rule.
    #[default]
    Error,
}

/// A problem found in a document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnostic {
    /// How serious the problem is.
    pub severity: Severity,
    /// What is wrong.
    pub message: String,
    /// The entity concerned, unset when none.
    pub entity: Guid,
    /// The component concerned, unset when none.
    pub component: Guid,
}

/// Whether any diagnostic is an error.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
