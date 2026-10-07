//! Choosing the node under the pointer, and what a click does to the
//! selection.

use fr_authoring::Selection;
use fr_document::Guid;
use fr_input::Modifiers;
use fr_math::Ray3d;

use crate::preview::NodeBounds;

/// The node whose bounds the ray enters first.
pub fn pick(bounds: &[NodeBounds], ray: &Ray3d) -> Option<Guid> {
    bounds
        .iter()
        .filter_map(|found| found.intersect(ray).map(|distance| (found.node, distance)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(node, _)| node)
}

/// Applies a click on `hit`, which is none for empty space, to the selection:
/// a plain click selects the node or clears the selection; control toggles the
/// node and shift adds it, and neither clears anything. Returns whether the
/// selection changed.
pub fn apply_click(selection: &mut Selection, hit: Option<Guid>, modifiers: Modifiers) -> bool {
    let before = selection.clone();
    match (hit, modifiers.control || modifiers.logo, modifiers.shift) {
        (Some(node), true, _) => selection.toggle(node),
        (Some(node), false, true) => selection.extend(node),
        (Some(node), false, false) => selection.set(node),
        (None, false, false) => selection.clear(),
        (None, _, _) => {}
    }
    *selection != before
}
