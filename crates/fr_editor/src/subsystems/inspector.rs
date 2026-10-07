//! The rows an inspector shows for a component, as plain data.
//!
//! A panel renders each [`InspectorRow`] with an editor chosen by its
//! [`RowValue`] and sends the command its `apply` closure returns for the edited
//! value; it never needs to know the component type. A subsystem contributes the
//! rows of its own type through [`InspectorContribution`]; every other
//! component is shown by [`generic_rows`] over the schema.

use std::fmt;
use std::rc::Rc;

use fr_authoring::{Command, PropertyRow, property_rows};
use fr_document::{
    AssetReference, Color, ComponentRecord, EntityRecord, EnumOption, Guid, PropertyType,
    PropertyValue, SchemaSet,
};
use fr_math::{Mat3, Quat, Vec3};
use fr_transform::Transform;

/// How close to straight up or down the pitch may get before the yaw and roll
/// can no longer be told apart.
const GIMBAL_LIMIT: f32 = 0.9999;

/// One choice of an enum row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowOption {
    /// The stable key stored in the document.
    pub key: String,
    /// The label shown.
    pub label: String,
}

/// The value of a row and so the kind of editor it needs.
#[derive(Clone, Debug, PartialEq)]
pub enum RowValue {
    /// A checkbox.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A number.
    Float(f32),
    /// Three numbers.
    Vec3(Vec3),
    /// A rotation as Euler angles in degrees, applied as yaw around Y, then pitch around X, then roll around Z.
    EulerDegrees(Vec3),
    /// A colour in linear channels.
    Color(Color),
    /// A line of text.
    Text(String),
    /// One of a fixed set of options.
    Enum {
        /// The key of the selected option.
        selected: String,
        /// Every option in display order.
        options: Vec<RowOption>,
    },
    /// A reference to an asset.
    Asset(AssetReference),
    /// A value shown as text that cannot be edited here, such as a reference
    /// to an entity or a list.
    Summary(String),
}

impl RowValue {
    /// The row value of a property value; `options` are the choices of an enum
    /// property.
    pub fn from_property(value: &PropertyValue, options: &[EnumOption]) -> Self {
        match value {
            PropertyValue::Bool(value) => Self::Bool(*value),
            PropertyValue::Int(value) => Self::Int(*value),
            PropertyValue::Float(value) => Self::Float(*value),
            PropertyValue::String(value) => Self::Text(value.clone()),
            PropertyValue::Enum(selected) => Self::Enum {
                selected: selected.clone(),
                options: options
                    .iter()
                    .map(|option| RowOption {
                        key: option.key.clone(),
                        label: option.label.clone(),
                    })
                    .collect(),
            },
            PropertyValue::Vec3(value) => Self::Vec3(*value),
            PropertyValue::Color(value) => Self::Color(*value),
            PropertyValue::Rotation(value) => Self::EulerDegrees(euler_degrees(*value)),
            PropertyValue::Asset(value) => Self::Asset(value.clone()),
            PropertyValue::Transform(_) => Self::Summary("transform".to_owned()),
            PropertyValue::Entity(value) => Self::Summary(if value.entity.valid() {
                value.entity.to_text()
            } else {
                "none".to_owned()
            }),
            PropertyValue::Component(value) => Self::Summary(if value.component.valid() {
                value.component.to_text()
            } else {
                "none".to_owned()
            }),
            PropertyValue::Array(items) => Self::Summary(format!("{} items", items.len())),
        }
    }

    /// The property value this row value stores in a property of type `kind`;
    /// none when the value does not fit the type.
    pub fn to_property(&self, kind: PropertyType) -> Option<PropertyValue> {
        match (self, kind) {
            (Self::Bool(value), PropertyType::Bool) => Some(PropertyValue::Bool(*value)),
            (Self::Int(value), PropertyType::Int) => Some(PropertyValue::Int(*value)),
            (Self::Float(value), PropertyType::Float) => Some(PropertyValue::Float(*value)),
            (Self::Int(value), PropertyType::Float) => Some(PropertyValue::Float(*value as f32)),
            (Self::Float(value), PropertyType::Int) => {
                Some(PropertyValue::Int(value.round() as i64))
            }
            (Self::Text(value), PropertyType::String) => Some(PropertyValue::String(value.clone())),
            (Self::Enum { selected, .. }, PropertyType::Enum) => {
                Some(PropertyValue::Enum(selected.clone()))
            }
            (Self::Vec3(value), PropertyType::Vec3) => Some(PropertyValue::Vec3(*value)),
            (Self::Color(value), PropertyType::Color) => Some(PropertyValue::Color(*value)),
            (Self::EulerDegrees(value), PropertyType::Rotation) => {
                Some(PropertyValue::Rotation(quat_from_degrees(*value)))
            }
            (Self::Asset(value), PropertyType::Asset) => Some(PropertyValue::Asset(value.clone())),
            _ => None,
        }
    }
}

/// A rotation as Euler angles in degrees: pitch around X, yaw around Y and roll
/// around Z, composed as yaw, then pitch, then roll.
pub fn euler_degrees(rotation: Quat) -> Vec3 {
    let matrix = Mat3::from_quat(rotation);
    let element = |row: usize, column: usize| matrix.col(column)[row];
    let pitch = (-element(1, 2)).clamp(-1.0, 1.0).asin();
    let (yaw, roll) = if element(1, 2).abs() < GIMBAL_LIMIT {
        (
            element(0, 2).atan2(element(2, 2)),
            element(1, 0).atan2(element(1, 1)),
        )
    } else {
        ((-element(2, 0)).atan2(element(0, 0)), 0.0)
    };
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

/// The rotation of Euler angles in degrees, the inverse of [`euler_degrees`].
pub fn quat_from_degrees(degrees: Vec3) -> Quat {
    Quat::from_rotation_y(degrees.y.to_radians())
        * Quat::from_rotation_x(degrees.x.to_radians())
        * Quat::from_rotation_z(degrees.z.to_radians())
}

/// One row of an inspector: what to show and the command an edit becomes.
#[derive(Clone)]
pub struct InspectorRow {
    /// The property key, or the field name for a row that is not a property.
    pub key: String,
    /// The label shown.
    pub label: String,
    /// The current value.
    pub value: RowValue,
    /// The least and greatest value of a number, when limited.
    pub range: Option<(f32, f32)>,
    /// The editing step of a number.
    pub step: Option<f32>,
    /// Whether the row only shows its value.
    pub read_only: bool,
    /// Turns an edited value into the command that stores it.
    pub apply: Rc<dyn Fn(RowValue) -> Command>,
}

impl InspectorRow {
    /// The command that stores `value` through this row.
    pub fn command(&self, value: RowValue) -> Command {
        (self.apply)(value)
    }
}

impl fmt::Debug for InspectorRow {
    /// Shows the data of the row; the closure has nothing to show.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InspectorRow")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("value", &self.value)
            .field("range", &self.range)
            .field("step", &self.step)
            .field("read_only", &self.read_only)
            .finish_non_exhaustive()
    }
}

/// What a contribution is asked to build rows for: one component of a node, or
/// the node's own transform when `component` is none.
#[derive(Clone, Copy)]
pub struct InspectorContext<'a> {
    /// The node being inspected.
    pub node: Guid,
    /// The entity record, when the node is an entity.
    pub entity: Option<&'a EntityRecord>,
    /// The component whose rows are wanted; none for the transform.
    pub component: Option<&'a ComponentRecord>,
    /// The transform of the node relative to its parent.
    pub transform: Transform,
    /// The component schemas of the document.
    pub schemas: &'a SchemaSet,
}

/// A specialised inspector for one component type, contributed by a subsystem.
/// It is data only: the rows carry the commands, a panel carries the editors.
pub trait InspectorContribution {
    /// The rows of the component or transform described by `context`.
    fn rows(&self, context: &InspectorContext<'_>) -> Vec<InspectorRow>;
}

/// A command that changes nothing, for an edit that cannot be stored.
pub fn no_change() -> Command {
    Command::Batch {
        label: "No change".to_owned(),
        commands: Vec::new(),
    }
}

/// The row of one schema property of a component.
pub fn property_row(entity: Guid, component: Guid, row: &PropertyRow) -> InspectorRow {
    let kind = row.kind;
    let key = row.key.clone();
    let apply_key = key.clone();
    let read_only = matches!(
        row.value,
        PropertyValue::Transform(_)
            | PropertyValue::Entity(_)
            | PropertyValue::Component(_)
            | PropertyValue::Array(_)
    );
    InspectorRow {
        key,
        label: row.label.clone(),
        value: RowValue::from_property(&row.value, row.options()),
        range: row.range(),
        step: row.step(),
        read_only,
        apply: Rc::new(move |value| match value.to_property(kind) {
            Some(value) => Command::SetProperty {
                entity,
                component,
                key: apply_key.clone(),
                value,
            },
            None => no_change(),
        }),
    }
}

/// The rows of a component taken from its schema, one per property in schema
/// order, which is what every component without a contribution is shown with.
pub fn generic_rows(
    entity: Guid,
    component: &ComponentRecord,
    schemas: &SchemaSet,
) -> Vec<InspectorRow> {
    property_rows(component, schemas)
        .iter()
        .map(|row| property_row(entity, component.id, row))
        .collect()
}
