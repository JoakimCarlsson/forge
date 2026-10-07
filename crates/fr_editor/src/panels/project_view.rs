//! The Project panel's layout: the toolbar, the name prompt, the folder tree and
//! the file list, and the menus.

use fr_project::ProjectEntry;
use fr_ui::{
    Div, IconName, IconSize, MenuItem, Side, Styled, Theme, button, context_menu, dropdown, h_flex,
    icon, icon_button, menu, menu_entry, menu_separator, scroll_area, text, tree_row, v_flex,
};

use super::project::{CreatePrompt, MenuTarget, ProjectMessage, name_target, search_target};
use super::project_logic::{
    CreateKind, FolderRow, entry_icon, folder_rows, list_entries, name_of, opens_as_document,
};
use crate::app::fields::text_input;
use crate::app::{EditorState, Message};
use fr_document::AssetKind;

/// The width of the folder tree.
const TREE_WIDTH: f32 = 190.0;

/// Wraps a panel message as the app's.
fn send(message: ProjectMessage) -> Message {
    Message::Project(message)
}

/// The panel: its header, toolbar and the tree beside the list.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let panel = &state.project_panel;
    let mut root = v_flex().w_full().h_full();
    let Some(project) = state.project.as_ref() else {
        return root.child(
            v_flex().w_full().p(4).child(
                text("No project is open.")
                    .text_sm()
                    .color(theme.colors.text_subtle),
            ),
        );
    };
    let listing = panel.listing(Some(project));
    let query = filter_text(state);
    root = root.child(toolbar(state, theme));
    if let Some(prompt) = &panel.prompt {
        root = root.child(prompt_bar(state, theme, prompt));
    }
    let body = h_flex()
        .w_full()
        .flex_1()
        .child(tree(state, theme, &listing.entries))
        .child(list(state, theme, &listing.entries, &query));
    root = root.child(body);
    match &panel.menu {
        Some(open) => root.child(context_menu(
            theme,
            open.origin,
            send(ProjectMessage::CloseMenu),
            menu_items(state, &open.target),
        )),
        None => root,
    }
}

/// The search text: what is being typed, or what was last typed.
fn filter_text(state: &EditorState) -> String {
    if state.text.has(&search_target()) {
        state.text.edit.text().to_owned()
    } else {
        state.project_panel.filter.clone()
    }
}

/// The entries of the Create menu for the selected folder.
fn create_items() -> Vec<MenuItem<Message>> {
    CreateKind::ALL
        .into_iter()
        .map(|kind| {
            menu_entry(kind.label(), Some(send(ProjectMessage::BeginCreate(kind))))
                .icon(kind.icon())
        })
        .collect()
}

/// The entries of a context menu.
fn menu_items(state: &EditorState, target: &MenuTarget) -> Vec<MenuItem<Message>> {
    let mut items = Vec::new();
    if let MenuTarget::File(path) = target {
        match AssetKind::of_path(path) {
            kind if opens_as_document(kind) => items.push(
                menu_entry("Open", Some(send(ProjectMessage::Open(path.clone()))))
                    .icon(IconName::File),
            ),
            AssetKind::Model => items.push(
                menu_entry(
                    "Add to Scene",
                    state
                        .document()
                        .map(|_| send(ProjectMessage::Open(path.clone()))),
                )
                .icon(IconName::Cube),
            ),
            _ => {}
        }
        items.push(menu_separator());
    } else {
        items.extend(create_items());
        items.push(menu_separator());
    }
    items.push(menu_entry("Refresh", Some(send(ProjectMessage::Refresh))).icon(IconName::Refresh));
    items
}

/// The Create button, Refresh and the search field.
fn toolbar(state: &EditorState, theme: &Theme) -> Div<Message> {
    let panel = &state.project_panel;
    let create = button("+ Create", send(ProjectMessage::ToggleCreateMenu));
    let create = if panel.create_menu {
        v_flex().child(
            dropdown(create, menu(theme, create_items()))
                .on_dismiss(send(ProjectMessage::CloseMenu)),
        )
    } else {
        v_flex().child(create)
    };
    let query = filter_text(state);
    let search = text_input(state, &search_target(), &panel.filter)
        .placeholder("Search")
        .w_full();
    h_flex()
        .w_full()
        .h_px(theme.size.control + 8.0)
        .px(2)
        .gap(2)
        .items_center()
        .border_side(Side::Bottom, 1.0, theme.colors.border_variant)
        .child(create)
        .child(
            icon_button(theme, IconName::Refresh, send(ProjectMessage::Refresh)).tooltip("Refresh"),
        )
        .child(v_flex().flex_1().child(search))
        .when(!query.is_empty(), |bar| {
            bar.child(
                icon_button(theme, IconName::Close, send(ProjectMessage::ClearSearch))
                    .tooltip("Clear search"),
            )
        })
}

/// The inline question for the name of a new entry.
fn prompt_bar(state: &EditorState, theme: &Theme, prompt: &CreatePrompt) -> Div<Message> {
    let folder = if prompt.folder.is_empty() {
        "Assets"
    } else {
        prompt.folder.as_str()
    };
    let title = text(format!(
        "New {} in {folder}",
        prompt.kind.label().to_lowercase()
    ))
    .text_sm()
    .color(theme.colors.text_muted);
    let entry = h_flex()
        .w_full()
        .gap(2)
        .items_center()
        .child(
            v_flex().flex_1().child(
                text_input(state, &name_target(), &prompt.name)
                    .placeholder("Name")
                    .w_full(),
            ),
        )
        .child(button("Create", send(ProjectMessage::ConfirmCreate)).filled())
        .child(button("Cancel", send(ProjectMessage::CancelCreate)));
    v_flex()
        .w_full()
        .px(2)
        .py(2)
        .gap(1)
        .bg(theme.colors.surface_selected)
        .child(title)
        .child(entry)
}

/// One row of the folder tree.
fn folder_element(state: &EditorState, row: &FolderRow) -> Div<Message> {
    let panel = &state.project_panel;
    let label = name_of(&row.path).to_owned();
    let glyph = if row.expanded {
        IconName::FolderOpen
    } else {
        IconName::Folder
    };
    let entry = tree_row(row.depth, label)
        .icon(glyph)
        .selected(panel.folder == row.path)
        .on_click(send(ProjectMessage::SelectFolder(row.path.clone())))
        .on_secondary_click(send(ProjectMessage::OpenMenu(MenuTarget::Folder(
            row.path.clone(),
        ))));
    let entry = if row.has_folders {
        entry.expandable(
            row.expanded,
            send(ProjectMessage::ToggleFolder(row.path.clone())),
        )
    } else {
        entry
    };
    v_flex().w_full().child(entry)
}

/// The folder tree: the asset root and the folders below it.
fn tree(state: &EditorState, theme: &Theme, entries: &[ProjectEntry]) -> Div<Message> {
    let panel = &state.project_panel;
    let root = tree_row(0, "Assets")
        .icon(IconName::FolderOpen)
        .selected(panel.folder.is_empty())
        .on_click(send(ProjectMessage::SelectFolder(String::new())))
        .on_secondary_click(send(ProjectMessage::OpenMenu(MenuTarget::Folder(
            String::new(),
        ))));
    let rows = folder_rows(entries, &panel.expanded);
    let body = v_flex()
        .w_full()
        .child(root)
        .children(rows.iter().map(|row| folder_element(state, row)));
    v_flex()
        .w_px(TREE_WIDTH)
        .h_full()
        .border_side(Side::Right, 1.0, theme.colors.border_variant)
        .child(
            scroll_area(&panel.tree_scroll, body)
                .on_scroll(|event| send(ProjectMessage::ScrollTree(event))),
        )
}

/// Whether an open document is the file at a path, and has unsaved changes.
fn is_dirty(state: &EditorState, relative: &str) -> bool {
    let absolute = super::project_logic::absolute_of(state.workspace.asset_root(), relative);
    state
        .workspace
        .find_by_path(&absolute)
        .and_then(|id| state.workspace.get(id))
        .is_some_and(|open| open.is_dirty())
}

/// One row of the file list.
fn list_element(
    state: &EditorState,
    theme: &Theme,
    entry: &ProjectEntry,
    searching: bool,
) -> Div<Message> {
    let panel = &state.project_panel;
    let path = entry.path.clone();
    let open = send(ProjectMessage::Open(path.clone()));
    let (click, secondary) = if entry.is_directory {
        (
            send(ProjectMessage::SelectFolder(path.clone())),
            MenuTarget::Folder(path.clone()),
        )
    } else {
        (
            send(ProjectMessage::SelectFile(path.clone())),
            MenuTarget::File(path.clone()),
        )
    };
    let label = if searching { &path } else { name_of(&path) };
    let mut row = tree_row(0, label.to_owned())
        .icon(entry_icon(entry))
        .selected(panel.file.as_deref() == Some(path.as_str()))
        .on_click(click)
        .on_double_click(open)
        .on_secondary_click(send(ProjectMessage::OpenMenu(secondary)));
    if !entry.is_directory && is_dirty(state, &path) {
        row = row.trailing(
            icon(IconName::CircleDot)
                .size(IconSize::Small)
                .color(theme.colors.accent),
        );
    }
    v_flex().w_full().child(row)
}

/// The file list: the folders and files of the selected folder, or the matches
/// of the search.
fn list(state: &EditorState, theme: &Theme, entries: &[ProjectEntry], query: &str) -> Div<Message> {
    let panel = &state.project_panel;
    let shown = list_entries(entries, &panel.folder, query);
    let searching = !query.trim().is_empty();
    let body = v_flex()
        .w_full()
        .children(
            shown
                .iter()
                .map(|entry| list_element(state, theme, entry, searching)),
        )
        .when(shown.is_empty(), |body| {
            body.child(
                v_flex().w_full().p(3).child(
                    text(if searching {
                        "Nothing matches the search."
                    } else {
                        "This folder is empty."
                    })
                    .text_sm()
                    .color(theme.colors.text_subtle),
                ),
            )
        });
    v_flex()
        .flex_1()
        .h_full()
        .on_secondary_click(send(ProjectMessage::OpenMenu(MenuTarget::Background)))
        .child(
            scroll_area(&panel.list_scroll, body)
                .on_scroll(|event| send(ProjectMessage::ScrollList(event))),
        )
}
