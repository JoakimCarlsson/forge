//! The Hierarchy panel: the tree of the current document.
//!
//! The first row is the document itself with a mark when it has unsaved
//! changes; under it the entities and prefab instances in order. Rows select,
//! rename in place, carry onto each other to reparent, and open the context
//! menu. Every edit is a command; the panel only reads the document.

use std::collections::HashSet;

use fr_authoring::DocumentKind;
use fr_document::{Guid, NodeKind};
use fr_ui::{
    Div, DragEvent, DropHint, IconName, IconSize, ScrollEvent, Styled, Theme, div, h_flex, icon,
    scroll_area, text, tree_row, v_flex,
};

use super::hierarchy_rows::{Row, visible_rows};
use super::hierarchy_update::ROW_PREFIX;
use crate::app::fields::text_input;
use crate::app::{EditorState, Message, TextTarget};

/// What the user did in the Hierarchy.
#[derive(Clone, Debug)]
pub enum HierarchyMessage {
    /// A row was clicked; the held modifiers decide how it selects.
    Select(Guid),
    /// The empty area was clicked.
    ClearSelection,
    /// The chevron of a row was clicked; unset is the document's own row.
    ToggleExpand(Guid),
    /// A row was double clicked.
    BeginRename(Guid),
    /// A row is being carried.
    Drag {
        /// The node of the row.
        node: Guid,
        /// The stage of the drag.
        event: DragEvent,
    },
    /// The secondary button went down on a row, or on the empty area.
    Context(Option<Guid>),
    /// A Create menu entry was chosen, by its position in the registry's list.
    CreateEntry {
        /// The entry.
        index: usize,
        /// The parent of the new entity.
        parent: Guid,
    },
    /// Create Empty or Create Child was chosen.
    CreateEmpty {
        /// The parent of the new entity.
        parent: Guid,
    },
    /// The tree was scrolled.
    Scroll(ScrollEvent),
}

/// The key a node's row is recorded under in the rects.
pub fn row_key(node: Guid) -> String {
    if node.valid() {
        format!("{ROW_PREFIX}{}", node.to_text())
    } else {
        format!("{ROW_PREFIX}root")
    }
}

/// The icon of a row: its first component's, or a plain one.
fn row_icon(state: &EditorState, row: &Row) -> IconName {
    match (&row.kind, &row.first_component) {
        (NodeKind::Instance, _) => IconName::Box,
        (NodeKind::Entity, Some(kind)) => state.subsystems.icon(kind),
        (NodeKind::Entity, None) => IconName::CircleDot,
    }
}

/// The hint a row shows while a carried row is over it.
fn hint_for(state: &EditorState, node: Guid) -> Option<DropHint> {
    state
        .hierarchy
        .drag
        .and_then(|drag| drag.target)
        .filter(|target| target.node == node)
        .map(|target| target.hint)
}

/// The row of a node being renamed: the field where the label would be.
fn rename_row(state: &EditorState, theme: &Theme, row: &Row, node: Guid) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(6.0 + 4.0 * row.depth as f64)
        .pr(2)
        .gap(2)
        .items_center()
        .bg(theme.colors.surface_selected)
        .child(
            icon(row_icon(state, row))
                .size(IconSize::Medium)
                .color(theme.colors.text_muted),
        )
        .child(
            text_input(state, &TextTarget::HierarchyRename(node), &row.name)
                .bare()
                .w_full(),
        )
}

/// The row of one listed node.
fn node_row(state: &EditorState, theme: &Theme, row: &Row, selected: bool) -> Div<Message> {
    let node = row.node;
    if state.hierarchy.renaming == Some(node) {
        return rename_row(state, theme, row, node);
    }
    let mut line = tree_row(row.depth, row.name.clone())
        .icon(row_icon(state, row))
        .selected(selected)
        .dimmed(!row.active)
        .drop_hint(hint_for(state, node))
        .on_click(Message::Hierarchy(HierarchyMessage::Select(node)))
        .on_double_click(Message::Hierarchy(HierarchyMessage::BeginRename(node)))
        .on_secondary_click(Message::Hierarchy(HierarchyMessage::Context(Some(node))))
        .on_drag(move |event| Message::Hierarchy(HierarchyMessage::Drag { node, event }))
        .recorded(&state.rects, row_key(node));
    if row.kind == NodeKind::Instance {
        line = line.icon_color(theme.colors.accent);
    }
    if row.has_children {
        line = line.expandable(
            row.expanded,
            Message::Hierarchy(HierarchyMessage::ToggleExpand(node)),
        );
    }
    div().w_full().child(line)
}

/// The row of the document itself.
fn document_row(state: &EditorState, theme: &Theme, collapsed: bool) -> Div<Message> {
    let Some(open) = state.document() else {
        return div();
    };
    let kind_icon = match open.document().kind() {
        DocumentKind::Scene => IconName::Scene,
        DocumentKind::Prefab => IconName::Box,
    };
    let mark = if open.is_dirty() { " \u{2022}" } else { "" };
    let label = format!("{}{mark}", open.title());
    let line = tree_row(0, label)
        .icon(kind_icon)
        .icon_color(theme.colors.text)
        .drop_hint(hint_for(state, Guid::NONE))
        .expandable(
            !collapsed,
            Message::Hierarchy(HierarchyMessage::ToggleExpand(Guid::NONE)),
        )
        .on_click(Message::Hierarchy(HierarchyMessage::ClearSelection))
        .on_secondary_click(Message::Hierarchy(HierarchyMessage::Context(None)))
        .recorded(&state.rects, row_key(Guid::NONE));
    div().w_full().child(line)
}

/// The filter field with its search icon.
fn filter_bar(state: &EditorState, theme: &Theme) -> Div<Message> {
    h_flex()
        .w_full()
        .px(2)
        .py(1)
        .gap(2)
        .items_center()
        .child(
            icon(IconName::Search)
                .size(IconSize::Medium)
                .color(theme.colors.text_muted),
        )
        .child(text_input(
            state,
            &TextTarget::HierarchyFilter,
            &state.hierarchy.filter,
        ))
}

/// The panel's contents.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let Some(open) = state.document() else {
        return v_flex().w_full().h_full().p(4).child(
            text("No document open")
                .text_sm()
                .color(theme.colors.text_subtle),
        );
    };
    let id = state.document_id();
    let empty = HashSet::new();
    let collapsed = id
        .and_then(|id| state.hierarchy.collapsed_in(id))
        .unwrap_or(&empty);
    let root_collapsed = collapsed.contains(&Guid::NONE);
    let rows = if root_collapsed {
        Vec::new()
    } else {
        visible_rows(
            open.document().content(),
            collapsed,
            &state.hierarchy.filter,
        )
    };
    let listed = rows.len() + 1;
    let filler = (state.hierarchy.scroll.viewport_height() - listed as f32 * theme.size.row)
        .max(theme.size.row);
    let mut column = v_flex()
        .w_full()
        .child(document_row(state, theme, root_collapsed));
    for row in &rows {
        column = column.child(node_row(
            state,
            theme,
            row,
            open.selection().contains(row.node),
        ));
    }
    column = column.child(
        div()
            .w_full()
            .h_px(filler)
            .on_click(Message::Hierarchy(HierarchyMessage::ClearSelection))
            .on_secondary_click(Message::Hierarchy(HierarchyMessage::Context(None))),
    );
    v_flex()
        .w_full()
        .h_full()
        .child(filter_bar(state, theme))
        .child(
            v_flex().w_full().flex_1().child(
                scroll_area(&state.hierarchy.scroll, column)
                    .on_scroll(|event| Message::Hierarchy(HierarchyMessage::Scroll(event))),
            ),
        )
}
