//! The root view: the picker, or the menu bar over the dock over the status
//! bar, with the document tabs, popups and questions painted on top.

use fr_color::Rgba;
use fr_math::{Point, Rect};
use fr_ui::{
    Div, DockPanel, IconName, Overlay, Styled, Theme, button, context_menu, div, dock_view, h_flex,
    overlay, popup_at, tab, tab_bar, text, toolbar_button, toolbar_group, v_flex,
};

use super::menus;
use super::message::{Action, Message, PromptAnswer};
use super::picker;
use super::state::{EditorState, PopupKind};
use super::status;
use crate::panels::registry::{self, ids};
use crate::panels::{console, hierarchy, hierarchy_menu, inspector, project};
use crate::viewport::Tool;

/// The dim wash behind a question.
const DIM: Rgba = Rgba::new(0.0, 0.0, 0.0, 0.45);

/// Where the last frame painted the open middle of the dock.
pub fn middle_rect(state: &EditorState) -> Option<Rect> {
    let path = state.dock.open_path()?;
    state.rects.get(&format!("group:{}", path.key()))
}

/// The rectangle of the 3D view inside the open middle: under the strip of
/// document tabs.
pub fn viewport_rect(state: &EditorState, theme: &Theme) -> Rect {
    let Some(middle) = middle_rect(state) else {
        return Rect::default();
    };
    let strip = if state.has_project() {
        theme.size.tab_bar
    } else {
        0.0
    };
    let height = (middle.size.height - strip).max(0.0);
    Rect::from_xywh(
        middle.left(),
        middle.top() + strip,
        middle.size.width,
        height,
    )
}

/// The panel of an id, with its tab.
fn panel(state: &EditorState, theme: &Theme, id: &str) -> DockPanel<Message> {
    let body = match id {
        ids::HIERARCHY => hierarchy::view(state, theme),
        ids::PROJECT => project::view(state, theme),
        ids::INSPECTOR => inspector::view(state, theme),
        ids::CONSOLE => console::view(state, theme),
        _ => div(),
    };
    let entry = registry::find(id);
    let title = entry.map_or(id, |entry| entry.title);
    let described = DockPanel::new(title, body).closable(true);
    match entry {
        Some(entry) => described.icon(entry.icon),
        None => described,
    }
}

/// The strip of open documents over the 3D view.
fn document_tabs(state: &EditorState, theme: &Theme, middle: Rect) -> Overlay<Message> {
    let current = state.document_id();
    let tabs = state
        .workspace
        .documents()
        .map(|(id, open)| {
            tab(open.title(), Message::SelectDocument(id))
                .active(Some(id) == current)
                .dirty(open.is_dirty())
                .on_close(Message::CloseDocument(id))
        })
        .collect();
    overlay(
        middle.origin,
        div()
            .w_px(middle.size.width)
            .child(tab_bar(theme, tabs, None, false)),
    )
}

/// The tools of the viewport in a group at its top left corner.
fn tool_overlay(state: &EditorState, theme: &Theme, rect: Rect) -> Overlay<Message> {
    let current = state.viewport.tool();
    let tools = [
        (IconName::Cursor, Tool::Select, "Select (Q)"),
        (IconName::Move, Tool::Move, "Move (W)"),
        (IconName::Rotate, Tool::Rotate, "Rotate (E)"),
        (IconName::Scale, Tool::Scale, "Scale (R)"),
    ];
    let buttons = tools
        .into_iter()
        .map(|(icon, tool, tip)| {
            toolbar_button(icon, Some(Message::Action(Action::SetTool(tool))))
                .active(current == tool)
                .tooltip(tip)
        })
        .collect();
    overlay(
        Point::new(rect.left() + 8.0, rect.top() + 8.0),
        toolbar_group(theme, buttons),
    )
}

/// The hint shown in the 3D view while no document is open.
fn empty_hint(theme: &Theme, rect: Rect) -> Overlay<Message> {
    let centre = Point::new(
        rect.left() + rect.size.width / 2.0 - 120.0,
        rect.top() + rect.size.height / 2.0 - 8.0,
    );
    overlay(
        centre,
        text("Open a scene from the Project panel")
            .text_sm()
            .color(theme.colors.text_subtle),
    )
}

/// The question about unsaved changes, over a dimmed window.
fn prompt_overlay(state: &EditorState, theme: &Theme) -> Option<Overlay<Message>> {
    let prompt = state.prompt.as_ref()?;
    let card = v_flex()
        .w_px(380.0)
        .p(5)
        .gap(4)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .blocks_pointer()
        .child(text(prompt.text.clone()).font_medium())
        .child(
            h_flex()
                .w_full()
                .gap(2)
                .justify_end()
                .child(button("Cancel", Message::Prompt(PromptAnswer::Cancel)))
                .child(button("Discard", Message::Prompt(PromptAnswer::Discard)))
                .child(button("Save", Message::Prompt(PromptAnswer::Save)).filled()),
        );
    let wash = v_flex()
        .w_full()
        .h_full()
        .bg(DIM)
        .items_center()
        .justify_center()
        .child(card);
    Some(popup_at(
        Point::default(),
        Message::Prompt(PromptAnswer::Cancel),
        wash,
    ))
}

/// The popup menu opened over the window, when one is.
fn popup_overlay(state: &EditorState, theme: &Theme) -> Option<Overlay<Message>> {
    let popup = state.popup.as_ref()?;
    let items = match &popup.kind {
        PopupKind::Hierarchy(node) => hierarchy_menu::items(state, *node),
    };
    Some(context_menu(
        theme,
        popup.origin,
        Message::CloseMenus,
        items,
    ))
}

/// The editor: menu bar, dock and status bar.
fn editor_view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let middle = middle_rect(state);
    let viewport = viewport_rect(state, theme);
    let dock = dock_view(&state.dock, &state.rects, Message::Dock)
        .drag(state.dock_drag.as_ref())
        .focused(state.focused_panel.as_deref())
        .build(theme, |id| panel(state, theme, id));
    let mut root = v_flex()
        .w_full()
        .h_full()
        .child(menus::view(state, theme))
        .child(v_flex().w_full().flex_1().child(dock))
        .child(status::view(state, theme));
    if let (Some(middle), true) = (middle, state.has_project()) {
        root = root.child(document_tabs(state, theme, middle));
        if state.workspace.is_empty() {
            root = root.child(empty_hint(theme, viewport));
        } else {
            root = root.child(tool_overlay(state, theme, viewport));
        }
    }
    root
}

/// The whole window's tree for this frame.
pub fn root(state: &EditorState, theme: &Theme) -> Div<Message> {
    let mut root = if state.picker.is_some() {
        picker::view(state, theme)
    } else {
        editor_view(state, theme)
    };
    if let Some(popup) = popup_overlay(state, theme) {
        root = root.child(popup);
    }
    if let Some(prompt) = prompt_overlay(state, theme) {
        root = root.child(prompt);
    }
    root
}
