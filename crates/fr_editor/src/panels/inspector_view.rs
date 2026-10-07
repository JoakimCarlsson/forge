//! The Inspector's layout: the node header, the transform, one collapsible
//! section per component, the scene settings and the Add Component menu.
//!
//! Nothing here knows a component type: a section's icon comes from the
//! subsystem registry, its rows from [`inspector_rows`](super::inspector_rows)
//! and its editors from [`rows`](super::rows).

use std::rc::Rc;

use fr_authoring::{ComponentChoice, ComponentSection, DocumentKind, OpenDocument};
use fr_authoring::{addable_components, component_sections};
use fr_document::builtin_schema::TRANSFORM;
use fr_document::{EntityRecord, Guid};
use fr_ui::{
    Div, IconName, IconSize, SCROLLBAR_GUTTER, STEP, Styled, TextEdit, Theme, ToggleState, button,
    checkbox, dropdown, h_flex, icon, icon_button, menu, menu_entry, menu_separator, scroll_area,
    text, text_field, v_flex,
};

use super::inspector::{InspectorMenu, InspectorMessage};
use super::inspector_rows::{Node, component_rows, header_rows, transform_rows};
use super::row_editor::{RowOwner, TextSite, Typing};
use super::rows::{RowEnv, Wrap, row_view};
use crate::app::{EditorState, Message};
use crate::subsystems::InspectorRow;

/// How many rows of the Add Component list show before it scrolls.
const VISIBLE_CHOICES: usize = 12;

/// The right padding that keeps rows clear of the scroll bar, in spacing steps.
const GUTTER: f32 = 2.0 + SCROLLBAR_GUTTER / STEP;

/// The width of the Add Component list.
const ADD_WIDTH: f32 = 260.0;

/// The panel: its header and the scrolling body.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let body = match state.document() {
        Some(document) => content(state, theme, document),
        None => hint(theme, "Open a scene or prefab to inspect it."),
    };
    v_flex().w_full().h_full().child(
        v_flex().w_full().flex_1().child(
            scroll_area(&state.inspector_panel.scroll, body)
                .on_scroll(|event| Message::Inspector(InspectorMessage::Scroll(event))),
        ),
    )
}

/// A line of muted text filling the panel's width.
fn hint(theme: &Theme, message: &str) -> Div<Message> {
    v_flex()
        .w_full()
        .p(4)
        .child(text(message).text_sm().color(theme.colors.text_subtle))
}

/// What the panel shows for a document: its primary node, its settings or a hint.
fn content(state: &EditorState, theme: &Theme, document: &OpenDocument) -> Div<Message> {
    let listing = state.project_panel.listing(state.project.as_ref());
    let content = document.document().content();
    let names = |id: Guid| {
        content
            .find_entity(id)
            .map(|entity| entity.name.clone())
            .or_else(|| {
                content
                    .find_instance(id)
                    .map(|instance| instance.name.clone())
            })
    };
    let site = state.text.target.as_ref().and_then(TextSite::from_target);
    let typing = site.as_ref().map(|site| Typing {
        site,
        edit: &state.text.edit,
    });
    let search = TextEdit::new(state.inspector_panel.search.clone());
    let env = RowEnv {
        theme,
        editor: &state.inspector_panel.editor,
        assets: &listing.index,
        names: &names,
        typing,
        search: &search,
    };
    let wrap: Wrap<Message> =
        Rc::new(|key, event| Message::Inspector(InspectorMessage::Row(key, event)));
    let panel = Panel {
        state,
        theme,
        env: &env,
        wrap,
        document,
    };
    match document
        .selection()
        .primary()
        .and_then(|id| Node::find(document, id))
    {
        Some(node) => panel.node(node),
        None if document.document().kind() == DocumentKind::Scene => panel.scene_settings(),
        None => hint(theme, "Select an entity to inspect it."),
    }
}

/// What the sections are drawn with.
struct Panel<'a> {
    /// The editor.
    state: &'a EditorState,
    /// The theme.
    theme: &'a Theme,
    /// What rows are drawn with.
    env: &'a RowEnv<'a>,
    /// Sends the rows' events.
    wrap: Wrap<Message>,
    /// The current document.
    document: &'a OpenDocument,
}

impl Panel<'_> {
    /// The rows of an owner, each with its editor.
    fn rows(&self, owner: RowOwner, rows: &[InspectorRow]) -> Div<Message> {
        v_flex().w_full().gap(0.5).children(
            rows.iter()
                .map(|row| row_view(self.env, owner, row, &self.wrap)),
        )
    }

    /// Whether a section is collapsed.
    fn collapsed(&self, id: Guid) -> bool {
        self.state.inspector_panel.collapsed.contains(&id)
    }

    /// The frame around a header and, when expanded, its rows.
    fn frame(&self, header: Div<Message>, body: Option<Div<Message>>) -> Div<Message> {
        let theme = self.theme;
        v_flex()
            .w_full()
            .bg(theme.colors.surface)
            .border_1(theme.colors.border_variant)
            .rounded(theme.radius.md)
            .child(header)
            .when_some(body, |frame, body| {
                frame.child(v_flex().w_full().p(2).child(body))
            })
    }

    /// The clickable title of a section: chevron, icon and label.
    fn title(&self, id: Guid, icon_name: IconName, label: &str) -> Div<Message> {
        let theme = self.theme;
        let chevron = if self.collapsed(id) {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };
        h_flex()
            .flex_1()
            .h_full()
            .gap(2)
            .items_center()
            .on_click(Message::Inspector(InspectorMessage::ToggleSection(id)))
            .child(
                icon(chevron)
                    .size(IconSize::Small)
                    .color(theme.colors.text_muted),
            )
            .child(
                icon(icon_name)
                    .size(IconSize::Small)
                    .color(theme.colors.text_muted),
            )
            .child(text(label.to_owned()).text_sm().font_medium().flex_1())
    }

    /// The header bar of a section around its parts.
    fn header(&self, parts: Div<Message>) -> Div<Message> {
        parts
            .w_full()
            .h_px(self.theme.size.row)
            .px(2)
            .gap(1)
            .items_center()
    }

    /// The whole inspector of a node.
    fn node(&self, node: Node<'_>) -> Div<Message> {
        let selected = self.document.selection().len();
        let mut column = v_flex()
            .w_full()
            .p(2)
            .pr(GUTTER)
            .gap(2)
            .child(self.node_title(node, selected))
            .child(self.rows(RowOwner::Node, &header_rows(node, selected > 1)))
            .child(self.transform_section(node));
        if let Some(entity) = node.entity() {
            for section in component_sections(entity, self.document.document().schemas()) {
                column = column.child(self.component_section(entity, &section));
            }
            column = column.child(self.add_component(entity));
        }
        column
    }

    /// The line naming what is inspected.
    fn node_title(&self, node: Node<'_>, selected: usize) -> Div<Message> {
        let theme = self.theme;
        let (glyph, kind) = match node {
            Node::Entity(_) => (IconName::Cube, "Entity"),
            Node::Instance(_) => (IconName::Box, "Prefab Instance"),
        };
        let label = if selected > 1 {
            format!("{selected} selected")
        } else {
            kind.to_owned()
        };
        h_flex()
            .w_full()
            .gap(2)
            .items_center()
            .child(icon(glyph).color(theme.colors.text_muted))
            .child(text(label).text_sm().color(theme.colors.text_muted))
    }

    /// The Transform section of a node.
    fn transform_section(&self, node: Node<'_>) -> Div<Message> {
        let id = node.id();
        let glyph = self.state.subsystems.icon(TRANSFORM);
        let body = (!self.collapsed(id)).then(|| {
            self.rows(
                RowOwner::Node,
                &transform_rows(&self.state.subsystems, self.document, node),
            )
        });
        self.frame(
            self.header(h_flex().child(self.title(id, glyph, "Transform"))),
            body,
        )
    }

    /// The section of one component.
    fn component_section(&self, entity: &EntityRecord, section: &ComponentSection) -> Div<Message> {
        let id = section.component;
        let glyph = self.state.subsystems.icon(&section.kind);
        let body = (!self.collapsed(id)).then(|| {
            let record = entity
                .components
                .iter()
                .find(|component| component.id == id);
            let rows = record
                .map(|record| component_rows(&self.state.subsystems, self.document, entity, record))
                .unwrap_or_default();
            self.rows(RowOwner::Component(id), &rows)
        });
        let parts = h_flex()
            .child(self.title(id, glyph, &section.label))
            .child(checkbox(
                if section.enabled {
                    ToggleState::On
                } else {
                    ToggleState::Off
                },
                Message::Inspector(InspectorMessage::SetEnabled {
                    component: id,
                    enabled: !section.enabled,
                }),
            ))
            .child(self.remove_button(id, &section.removal))
            .child(self.section_menu(id, section));
        self.frame(self.header(parts), body)
    }

    /// The Remove button, greyed out with the reason when the rules forbid it.
    fn remove_button(&self, id: Guid, removal: &Result<(), String>) -> Div<Message> {
        let theme = self.theme;
        match removal {
            Ok(()) => icon_button(
                theme,
                IconName::Trash,
                Message::Inspector(InspectorMessage::Remove(id)),
            )
            .tooltip("Remove"),
            Err(reason) => v_flex()
                .size_px(theme.size.icon_control)
                .items_center()
                .justify_center()
                .tooltip(reason.clone())
                .child(
                    icon(IconName::Trash)
                        .size(IconSize::Medium)
                        .color(theme.colors.text_subtle),
                ),
        }
    }

    /// The menu button of a section header and, when open, its menu.
    fn section_menu(&self, id: Guid, section: &ComponentSection) -> Div<Message> {
        let theme = self.theme;
        let menu_id = InspectorMenu::Section(id);
        let anchor = icon_button(
            theme,
            IconName::More,
            Message::Inspector(InspectorMessage::ToggleMenu(menu_id)),
        );
        let wrapped = v_flex();
        if self.state.inspector_panel.menu != Some(menu_id) {
            return wrapped.child(anchor);
        }
        let message = |inner: InspectorMessage| Some(Message::Inspector(inner));
        let remove_label = match &section.removal {
            Ok(()) => "Remove Component".to_owned(),
            Err(reason) => format!("Remove Component ({reason})"),
        };
        let toggle_label = if section.enabled { "Disable" } else { "Enable" };
        let collapse_label = if self.collapsed(id) {
            "Expand"
        } else {
            "Collapse"
        };
        let items = vec![
            menu_entry(
                toggle_label,
                message(InspectorMessage::SetEnabled {
                    component: id,
                    enabled: !section.enabled,
                }),
            ),
            menu_entry(collapse_label, message(InspectorMessage::ToggleSection(id))),
            menu_separator(),
            menu_entry(
                remove_label,
                section
                    .removal
                    .is_ok()
                    .then_some(Message::Inspector(InspectorMessage::Remove(id))),
            ),
        ];
        wrapped.child(
            dropdown(anchor, menu(theme, items))
                .on_dismiss(Message::Inspector(InspectorMessage::CloseMenu)),
        )
    }

    /// The Add Component button and, when open, its searchable list.
    fn add_component(&self, entity: &EntityRecord) -> Div<Message> {
        let toggle = Message::Inspector(InspectorMessage::ToggleMenu(InspectorMenu::AddComponent));
        let anchor = button("Add Component", toggle).w_full();
        let wrapped = v_flex().w_full();
        if self.state.inspector_panel.menu != Some(InspectorMenu::AddComponent) {
            return wrapped.child(anchor);
        }
        wrapped.child(
            dropdown(anchor, self.add_list(entity))
                .match_width()
                .on_dismiss(Message::Inspector(InspectorMessage::CloseMenu)),
        )
    }

    /// The search line over the grouped list of component types.
    fn add_list(&self, entity: &EntityRecord) -> Div<Message> {
        let theme = self.theme;
        let target = crate::app::TextTarget::Inspector("add-search".to_owned());
        let focused = self.state.text.has(&target);
        let stored = TextEdit::new(self.state.inspector_panel.search.clone());
        let edit = if focused {
            &self.state.text.edit
        } else {
            &stored
        };
        let choices = matching_choices(
            addable_components(entity, self.document.document().schemas()),
            edit.text(),
        );
        let groups = group_by_category(&choices);
        let lines: usize = groups.iter().map(|(_, items)| items.len() + 1).sum();
        let height = lines.clamp(1, VISIBLE_CHOICES) as f32 * theme.size.row;
        let list = v_flex()
            .w_full()
            .children(groups.iter().map(|(category, items)| {
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .h_px(theme.size.row)
                            .px(2.5)
                            .items_center()
                            .child(
                                text(category.clone())
                                    .text_xs()
                                    .color(theme.colors.text_subtle),
                            ),
                    )
                    .children(items.iter().map(|choice| self.choice_row(choice)))
            }));
        v_flex()
            .w_px(ADD_WIDTH)
            .p(2)
            .gap(2)
            .bg(theme.colors.surface)
            .border_1(theme.colors.border)
            .rounded(theme.radius.lg)
            .blocks_pointer()
            .child(
                text_field(edit, focused)
                    .placeholder("Search")
                    .on_press(|index| {
                        Message::Inspector(InspectorMessage::SearchCaret(index, false))
                    })
                    .on_drag(|index| Message::Inspector(InspectorMessage::SearchCaret(index, true)))
                    .w_full(),
            )
            .child(
                scroll_area(&self.state.inspector_panel.menu_scroll, list)
                    .on_scroll(|event| Message::Inspector(InspectorMessage::MenuScroll(event)))
                    .h_px(height),
            )
    }

    /// One component type of the list: enabled to add it, dimmed with the
    /// reason when it cannot be added.
    fn choice_row(&self, choice: &ComponentChoice) -> Div<Message> {
        let theme = self.theme;
        let glyph = self.state.subsystems.icon(&choice.kind);
        let row = h_flex()
            .w_full()
            .h_px(theme.size.row)
            .px(2.5)
            .gap(2)
            .items_center();
        match &choice.availability {
            Ok(()) => row
                .hover_bg(theme.colors.surface_hover)
                .on_click(Message::Inspector(InspectorMessage::AddComponent(
                    choice.kind.clone(),
                )))
                .child(
                    icon(glyph)
                        .size(IconSize::Small)
                        .color(theme.colors.text_muted),
                )
                .child(text(choice.label.clone()).text_sm().flex_1()),
            Err(reason) => row
                .tooltip(reason.clone())
                .child(
                    icon(glyph)
                        .size(IconSize::Small)
                        .color(theme.colors.text_subtle),
                )
                .child(
                    text(choice.label.clone())
                        .text_sm()
                        .color(theme.colors.text_subtle),
                )
                .child(
                    text(reason.clone())
                        .text_xs()
                        .color(theme.colors.text_subtle)
                        .flex_1(),
                ),
        }
    }

    /// The scene settings, shown while nothing is selected in a scene.
    fn scene_settings(&self) -> Div<Message> {
        let theme = self.theme;
        let rows = super::inspector_scene::scene_rows(self.document);
        let header = self.header(
            h_flex()
                .child(
                    icon(IconName::Scene)
                        .size(IconSize::Small)
                        .color(theme.colors.text_muted),
                )
                .child(text("Scene Settings").text_sm().font_medium().flex_1()),
        );
        v_flex()
            .w_full()
            .p(2)
            .pr(GUTTER)
            .gap(2)
            .child(self.frame(header, Some(self.rows(RowOwner::Scene, &rows))))
            .child(hint(theme, "Select an entity to inspect it."))
    }
}

/// The choices whose label contains `query`, ignoring case.
fn matching_choices(choices: Vec<ComponentChoice>, query: &str) -> Vec<ComponentChoice> {
    let query = query.trim().to_lowercase();
    choices
        .into_iter()
        .filter(|choice| query.is_empty() || choice.label.to_lowercase().contains(&query))
        .collect()
}

/// The choices grouped by category, in the order the categories first appear.
fn group_by_category(choices: &[ComponentChoice]) -> Vec<(String, Vec<&ComponentChoice>)> {
    let mut groups: Vec<(String, Vec<&ComponentChoice>)> = Vec::new();
    for choice in choices {
        match groups
            .iter_mut()
            .find(|(category, _)| *category == choice.category)
        {
            Some((_, items)) => items.push(choice),
            None => groups.push((choice.category.clone(), vec![choice])),
        }
    }
    groups
}
