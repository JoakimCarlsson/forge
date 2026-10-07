//! The contribution for the transform of a node.

use std::rc::Rc;

use fr_authoring::Command;
use fr_document::Guid;
use fr_document::builtin_schema::TRANSFORM;
use fr_transform::Transform;
use fr_ui::IconName;

use crate::subsystems::{
    CreateEntry, InspectorContext, InspectorContribution, InspectorRow, RowValue, Subsystem,
    SubsystemRegistry, no_change, quat_from_degrees,
};

/// The group of the entries the transform subsystem adds to the Create menu.
const BASIC_CATEGORY: &str = "Basic";

/// The editing step of a position, in world units.
const POSITION_STEP: f32 = 0.1;

/// The editing step of a rotation, in degrees.
const ROTATION_STEP: f32 = 1.0;

/// The editing step of a scale.
const SCALE_STEP: f32 = 0.05;

/// Adds the transform subsystem to the registry.
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(TransformSubsystem));
}

/// Contributes the transform of a node, which is not a component record.
struct TransformSubsystem;

impl Subsystem for TransformSubsystem {
    /// The transform's type key.
    fn kind(&self) -> &'static str {
        TRANSFORM
    }

    /// The move icon.
    fn icon(&self) -> IconName {
        IconName::Move
    }

    /// The Empty entry.
    fn create_entries(&self) -> Vec<CreateEntry> {
        vec![CreateEntry::new(
            "Empty",
            BASIC_CATEGORY,
            IconName::Hierarchy,
            "Entity",
        )]
    }

    /// The specialised rows.
    fn inspector(&self) -> Option<&dyn InspectorContribution> {
        Some(&TransformRows)
    }
}

/// The position, rotation and scale rows of a node.
struct TransformRows;

impl InspectorContribution for TransformRows {
    /// Position, rotation as Euler degrees and scale, each storing the whole
    /// transform with one part replaced.
    fn rows(&self, context: &InspectorContext<'_>) -> Vec<InspectorRow> {
        let (node, base) = (context.node, context.transform);
        vec![
            transform_row(
                node,
                base,
                ("position", "Position"),
                RowValue::Vec3(base.translation),
                POSITION_STEP,
                |base, value| match value {
                    RowValue::Vec3(translation) => Some(base.with_translation(*translation)),
                    _ => None,
                },
            ),
            transform_row(
                node,
                base,
                ("rotation", "Rotation"),
                RowValue::EulerDegrees(crate::subsystems::euler_degrees(base.rotation)),
                ROTATION_STEP,
                |base, value| match value {
                    RowValue::EulerDegrees(degrees) => {
                        Some(base.with_rotation(quat_from_degrees(*degrees)))
                    }
                    _ => None,
                },
            ),
            transform_row(
                node,
                base,
                ("scale", "Scale"),
                RowValue::Vec3(base.scale),
                SCALE_STEP,
                |base, value| match value {
                    RowValue::Vec3(scale) => Some(base.with_scale(*scale)),
                    _ => None,
                },
            ),
        ]
    }
}

/// A row that edits one part of a node's transform: `update` returns the
/// transform with the edited value stored, or none when the value does not fit.
fn transform_row(
    node: Guid,
    base: Transform,
    (key, label): (&str, &str),
    value: RowValue,
    step: f32,
    update: fn(Transform, &RowValue) -> Option<Transform>,
) -> InspectorRow {
    InspectorRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
        range: None,
        step: Some(step),
        read_only: false,
        apply: Rc::new(move |edited| match update(base, &edited) {
            Some(transform) => Command::SetTransform { node, transform },
            None => no_change(),
        }),
    }
}
