//! The slot of an asset reference: the referenced asset's name, a button that
//! clears it and a picker listing the assets of the matching kinds.
//!
//! What a property accepts is decided by its key, since the schema carries no
//! kind: a key with `model` in it takes models, `prefab` prefabs and `scene`
//! scenes, and any other asset property takes models and prefabs.

use std::rc::Rc;

use fr_document::{AssetIndex, AssetKind, AssetReference, Guid, IndexedAsset};
use fr_ui::{
    Div, IconName, IconSize, Scroll, Styled, TextEdit, Theme, dropdown, h_flex, icon, icon_button,
    list_row, rule, scroll_area, text, text_field, v_flex,
};

use super::row_editor::RowEvent;
use crate::subsystems::RowValue;

/// How many rows of the picker show before it scrolls.
const VISIBLE_ROWS: f32 = 8.0;

/// The width of the picker.
const PICKER_WIDTH: f32 = 280.0;

/// Sends what the user does to the slot, already tagged with its row.
pub type Emit<M> = Rc<dyn Fn(RowEvent) -> M>;

/// The kinds of asset a property accepts, judged by its key.
pub fn kinds_for_key(key: &str) -> &'static [AssetKind] {
    let key = key.to_ascii_lowercase();
    if key.contains("model") {
        &[AssetKind::Model]
    } else if key.contains("prefab") {
        &[AssetKind::Prefab]
    } else if key.contains("scene") {
        &[AssetKind::Scene]
    } else {
        &[AssetKind::Model, AssetKind::Prefab]
    }
}

/// The icon of an asset kind.
pub fn kind_icon(kind: AssetKind) -> IconName {
    match kind {
        AssetKind::Model => IconName::Cube,
        AssetKind::Prefab => IconName::Box,
        AssetKind::Scene => IconName::Scene,
        AssetKind::Other => IconName::File,
    }
}

/// The file name at the end of a path.
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// What a reference is called: the name of the asset the index finds by its
/// identity, else the last known path, and whether the asset was found.
pub fn reference_label(index: &AssetIndex, reference: &AssetReference) -> (String, bool) {
    if !reference.asset.valid() {
        return ("None".to_owned(), true);
    }
    match index.find(reference.asset) {
        Some(asset) => (file_name(&asset.path).to_owned(), true),
        None if reference.last_known_path.is_empty() => {
            (format!("Missing {}", reference.asset.to_text()), false)
        }
        None => (
            format!("{} (missing)", file_name(&reference.last_known_path)),
            false,
        ),
    }
}

/// The assets of the kinds a property key accepts whose path contains `query`,
/// in path order.
pub fn choices<'a>(index: &'a AssetIndex, key: &str, query: &str) -> Vec<&'a IndexedAsset> {
    let kinds = kinds_for_key(key);
    let query = query.trim().to_lowercase();
    index
        .assets()
        .iter()
        .filter(|asset| kinds.contains(&AssetKind::of_path(&asset.path)))
        .filter(|asset| query.is_empty() || asset.path.to_lowercase().contains(&query))
        .collect()
}

/// The reference that picks an indexed asset.
fn reference_to(asset: &IndexedAsset) -> AssetReference {
    AssetReference {
        asset: asset.id,
        last_known_path: asset.path.clone(),
    }
}

/// The row of the picker that clears the reference.
fn none_row<M: Clone + 'static>(theme: &Theme, emit: &Emit<M>) -> Div<M> {
    list_row(
        theme,
        "None",
        false,
        emit(RowEvent::Pick(RowValue::Asset(AssetReference {
            asset: Guid::NONE,
            last_known_path: String::new(),
        }))),
    )
}

/// The rows of the picker: None, then every choice, the current one lit.
fn picker_rows<M: Clone + 'static>(
    theme: &Theme,
    choices: &[&IndexedAsset],
    current: &AssetReference,
    emit: &Emit<M>,
) -> Div<M> {
    let rows = choices.iter().map(|asset| {
        list_row(
            theme,
            asset.path.clone(),
            asset.id == current.asset,
            emit(RowEvent::Pick(RowValue::Asset(reference_to(asset)))),
        )
    });
    v_flex()
        .w_full()
        .child(none_row(theme, emit))
        .children(rows)
}

/// The popup of the slot: a search line over the scrolling list.
fn picker<M: Clone + 'static>(
    theme: &Theme,
    choices: &[&IndexedAsset],
    current: &AssetReference,
    (search, focused): (&TextEdit, bool),
    scroll: &Scroll,
    emit: &Emit<M>,
) -> Div<M> {
    let search_emit = emit.clone();
    let scroll_emit = emit.clone();
    let height = (choices.len() as f32 + 1.0).min(VISIBLE_ROWS) * theme.size.row;
    v_flex()
        .w_px(PICKER_WIDTH)
        .p(2)
        .gap(2)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .blocks_pointer()
        .child(
            text_field(search, focused)
                .placeholder("Search")
                .on_press(move |index| search_emit(RowEvent::SearchCaret(index, false)))
                .on_drag({
                    let emit = emit.clone();
                    move |index| emit(RowEvent::SearchCaret(index, true))
                })
                .w_full(),
        )
        .child(rule(theme))
        .child(
            scroll_area(scroll, picker_rows(theme, choices, current, emit))
                .on_scroll(move |event| scroll_emit(RowEvent::Scroll(event)))
                .h_px(height),
        )
}

/// What the slot's box shows: the kind's icon and the asset's name.
fn slot_box<M: Clone + 'static>(
    theme: &Theme,
    index: &AssetIndex,
    reference: &AssetReference,
    open: bool,
    emit: &Emit<M>,
) -> Div<M> {
    let (label, found) = reference_label(index, reference);
    let kind = index
        .find(reference.asset)
        .map_or(AssetKind::Other, |asset| AssetKind::of_path(&asset.path));
    let color = if found {
        theme.colors.text
    } else {
        theme.colors.text_subtle
    };
    let border = if open {
        theme.colors.border_focused
    } else {
        theme.colors.border
    };
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(2.5)
        .gap(2)
        .items_center()
        .bg(theme.colors.surface_input)
        .hover_bg(theme.colors.surface_hover)
        .border_1(border)
        .rounded(theme.radius.md)
        .on_click(emit(RowEvent::TogglePopup))
        .tooltip(reference.last_known_path.clone())
        .child(
            icon(kind_icon(kind))
                .size(IconSize::Small)
                .color(theme.colors.text_muted),
        )
        .child(text(label).text_sm().color(color).flex_1())
}

/// What an asset slot needs to be drawn.
pub struct AssetSlot<'a> {
    /// The theme to paint with.
    pub theme: &'a Theme,
    /// The assets of the project.
    pub index: &'a AssetIndex,
    /// The property key, which decides the kinds the picker lists.
    pub key: &'a str,
    /// The reference shown.
    pub reference: &'a AssetReference,
    /// Whether the picker is open.
    pub open: bool,
    /// The picker's search line.
    pub search: &'a TextEdit,
    /// Whether the search line has the keyboard.
    pub search_focused: bool,
    /// How far the picker's list is scrolled.
    pub scroll: &'a Scroll,
    /// Whether the slot only shows its reference.
    pub read_only: bool,
}

impl AssetSlot<'_> {
    /// The slot: the box that opens the picker and the clear button.
    pub fn view<M: Clone + 'static>(&self, emit: &Emit<M>) -> Div<M> {
        let theme = self.theme;
        let anchor = slot_box(
            theme,
            self.index,
            self.reference,
            self.open && !self.read_only,
            emit,
        );
        let mut line = h_flex().w_full().gap(1).items_center();
        line = match self.open && !self.read_only {
            true => {
                let found = choices(self.index, self.key, self.search.text());
                let panel = picker(
                    theme,
                    &found,
                    self.reference,
                    (self.search, self.search_focused),
                    self.scroll,
                    emit,
                );
                line.child(
                    v_flex()
                        .flex_1()
                        .child(dropdown(anchor, panel).on_dismiss(emit(RowEvent::DismissPopup))),
                )
            }
            false => line.child(v_flex().flex_1().child(anchor)),
        };
        if self.reference.asset.valid() && !self.read_only {
            line = line.child(
                icon_button(
                    theme,
                    IconName::Close,
                    emit(RowEvent::Pick(RowValue::Asset(AssetReference::default()))),
                )
                .tooltip("Clear"),
            );
        }
        line
    }
}
