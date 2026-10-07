//! The panels the editor offers, by the ids the dock layout names them with.

use fr_ui::{DockNode, DockTree, IconName};

/// The ids of the panels, as the dock layout file spells them.
pub mod ids {
    /// The Hierarchy panel.
    pub const HIERARCHY: &str = "hierarchy";
    /// The Project panel.
    pub const PROJECT: &str = "project";
    /// The Inspector panel.
    pub const INSPECTOR: &str = "inspector";
    /// The Console panel.
    pub const CONSOLE: &str = "console";
}

/// The side of the window a panel is docked on by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelSlot {
    /// The column left of the viewport.
    Left,
    /// The column right of the viewport.
    Right,
    /// The strip under the viewport.
    Bottom,
}

/// One panel the editor can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelEntry {
    /// The id the layout names the panel by.
    pub id: &'static str,
    /// The title on its tab and in the View menu.
    pub title: &'static str,
    /// The icon on its tab.
    pub icon: IconName,
    /// Where it is docked by default.
    pub slot: PanelSlot,
}

/// Every panel, in the order of the View menu.
pub const PANELS: [PanelEntry; 4] = [
    PanelEntry {
        id: ids::HIERARCHY,
        title: "Hierarchy",
        icon: IconName::Hierarchy,
        slot: PanelSlot::Left,
    },
    PanelEntry {
        id: ids::PROJECT,
        title: "Project",
        icon: IconName::Folder,
        slot: PanelSlot::Left,
    },
    PanelEntry {
        id: ids::INSPECTOR,
        title: "Inspector",
        icon: IconName::Settings,
        slot: PanelSlot::Right,
    },
    PanelEntry {
        id: ids::CONSOLE,
        title: "Console",
        icon: IconName::List,
        slot: PanelSlot::Bottom,
    },
];

/// The entry of a panel id.
pub fn find(id: &str) -> Option<&'static PanelEntry> {
    PANELS.iter().find(|entry| entry.id == id)
}

/// The layout the editor starts with: Hierarchy above Project on the left, the
/// Inspector on the right, the Console under the open middle.
pub fn default_layout() -> DockTree {
    let middle = DockNode::split(
        fr_ui::Axis::Vertical,
        0.74,
        DockNode::Open,
        DockNode::tabs(&[ids::CONSOLE]),
    );
    let rest = DockNode::split(
        fr_ui::Axis::Horizontal,
        0.76,
        middle,
        DockNode::tabs(&[ids::INSPECTOR]),
    );
    let left = DockNode::split(
        fr_ui::Axis::Vertical,
        0.55,
        DockNode::tabs(&[ids::HIERARCHY]),
        DockNode::tabs(&[ids::PROJECT]),
    );
    DockTree::new(DockNode::split(fr_ui::Axis::Horizontal, 0.2, left, rest))
}

/// Makes a layout loaded from disk fit the registry: panels the editor does
/// not know are dropped, and a layout without the open middle for the
/// viewport is replaced by the default.
pub fn reconcile(mut layout: DockTree) -> DockTree {
    let unknown: Vec<String> = layout
        .panel_ids()
        .into_iter()
        .filter(|id| find(id).is_none())
        .map(str::to_owned)
        .collect();
    for id in unknown {
        layout.close(&id);
    }
    if layout.open_path().is_none() || !layout.is_valid() {
        return default_layout();
    }
    layout
}

/// Shows a panel that is hidden, in the group of another panel of its slot
/// when one is showing, and hides it when it is showing.
pub fn toggle(layout: &mut DockTree, id: &str) {
    if layout.contains(id) {
        layout.close(id);
        return;
    }
    let Some(entry) = find(id) else {
        return;
    };
    let neighbour = PANELS
        .iter()
        .filter(|other| other.slot == entry.slot && other.id != id)
        .find_map(|other| layout.group_of(other.id));
    match neighbour {
        Some(group) => {
            layout.add_to_group(id, &group);
        }
        None => {
            layout.show(id);
        }
    }
}
