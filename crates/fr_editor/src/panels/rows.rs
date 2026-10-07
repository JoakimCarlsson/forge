//! The widgets of one inspector row: the typed editor its [`RowValue`] needs,
//! next to its label.
//!
//! One function, [`row_view`], draws every row whatever component it comes
//! from: a checkbox for a boolean, a drag value for a number, three of them for
//! a vector, a colour field with its editor popup, a combo for an enum, a text
//! field for a string, an asset slot for an asset reference and plain text for
//! anything else. What the user does comes back as a [`RowEvent`] tagged with
//! the row's [`RowKey`], through the caller's `wrap` function.

use std::rc::Rc;

use fr_document::{AssetIndex, Guid};
use fr_ui::{
    ColorEvent, ComboEvent, Div, DragValueEvent, Styled, TextEdit, Theme, ToggleState, checkbox,
    color_field, combo, drag_value, field, h_flex, property_row, text, text_field, v_flex,
    vec3_row,
};

use super::asset_slot::{AssetSlot, Emit};
use super::row_editor::{
    PopupKind, RowEditor, RowEvent, RowKey, RowOwner, TextRole, Typing, precision, to_color,
    to_rgba, with_slot,
};
use crate::subsystems::{InspectorRow, RowValue};

/// Tags an event with the row it came from and wraps it as the caller's message.
pub type Wrap<M> = Rc<dyn Fn(RowKey, RowEvent) -> M>;

/// Looks up the name of an entity by its identity.
pub type NameOf<'a> = &'a dyn Fn(Guid) -> Option<String>;

/// What rows are drawn with.
pub struct RowEnv<'a> {
    /// The theme to paint with.
    pub theme: &'a Theme,
    /// What is typed and open.
    pub editor: &'a RowEditor,
    /// The assets of the project, which asset slots name and list.
    pub assets: &'a AssetIndex,
    /// The name of an entity, for rows that refer to one.
    pub names: NameOf<'a>,
    /// The text of a row that has the keyboard.
    pub typing: Option<Typing<'a>>,
    /// What a picker's search line shows while it does not have the keyboard.
    pub search: &'a TextEdit,
}

/// One row being drawn: where it is, what it says and where its events go.
struct RowCx<'a, M> {
    /// What rows are drawn with.
    env: &'a RowEnv<'a>,
    /// The row.
    row: &'a InspectorRow,
    /// The row's name.
    key: RowKey,
    /// Sends the row's events.
    emit: Emit<M>,
}

impl<M> RowCx<'_, M> {
    /// The text of this row with the keyboard, when it has the role `role`.
    fn typing(&self, role: TextRole) -> Option<&TextEdit> {
        self.env
            .typing
            .filter(|typing| typing.site.row == self.key && typing.site.role == role)
            .map(|typing| typing.edit)
    }

    /// The popup this row has open.
    fn popup(&self) -> Option<&PopupKind> {
        self.env
            .editor
            .popup()
            .filter(|popup| popup.row == self.key)
            .map(|popup| &popup.kind)
    }
}

/// The row `row` of `owner`: its label and the editor of its value.
pub fn row_view<M: Clone + 'static>(
    env: &RowEnv<'_>,
    owner: RowOwner,
    row: &InspectorRow,
    wrap: &Wrap<M>,
) -> Div<M> {
    let key = RowKey::new(owner, row.key.clone());
    let emit: Emit<M> = {
        let (wrap, key) = (wrap.clone(), key.clone());
        Rc::new(move |event| wrap(key.clone(), event))
    };
    let cx = RowCx {
        env,
        row,
        key,
        emit,
    };
    let control = if row.read_only {
        read_only(&cx)
    } else {
        editor(&cx)
    };
    property_row(env.theme, row.label.clone(), control)
}

/// The editor of a row's value.
fn editor<M: Clone + 'static>(cx: &RowCx<'_, M>) -> Div<M> {
    match &cx.row.value {
        RowValue::Bool(value) => toggle(cx, *value),
        RowValue::Int(value) => number(cx, *value as f64),
        RowValue::Float(value) => number(cx, f64::from(*value)),
        RowValue::Vec3(value) | RowValue::EulerDegrees(value) => {
            vector(cx, value.to_array().map(f64::from))
        }
        RowValue::Color(color) => colour(cx, *color),
        RowValue::Text(value) => line(cx, value),
        RowValue::Enum { selected, options } => choice(cx, selected, options),
        RowValue::Asset(reference) => asset(cx, reference, false),
        RowValue::Summary(_) => read_only(cx),
    }
}

/// A row's value as text that cannot be edited.
fn read_only<M: Clone + 'static>(cx: &RowCx<'_, M>) -> Div<M> {
    let theme = cx.env.theme;
    let shown = match &cx.row.value {
        RowValue::Asset(reference) => return asset(cx, reference, true),
        RowValue::Summary(summary) => entity_name(cx, summary),
        RowValue::Bool(value) => value.to_string(),
        RowValue::Int(value) => value.to_string(),
        RowValue::Float(value) => format!("{value:.2}"),
        RowValue::Text(value) => value.clone(),
        RowValue::Enum { selected, options } => options
            .iter()
            .find(|option| option.key == *selected)
            .map_or_else(|| selected.clone(), |option| option.label.clone()),
        RowValue::Vec3(value) | RowValue::EulerDegrees(value) => {
            format!("{:.2}, {:.2}, {:.2}", value.x, value.y, value.z)
        }
        RowValue::Color(color) => fr_ui::format_hex(to_rgba(*color)),
    };
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .items_center()
        .child(
            text(shown)
                .text_sm()
                .color(theme.colors.text_muted)
                .flex_1(),
        )
}

/// A summary with an entity's identity replaced by its name when it has one.
fn entity_name<M>(cx: &RowCx<'_, M>, summary: &str) -> String {
    Guid::from_text(summary)
        .and_then(|id| (cx.env.names)(id))
        .unwrap_or_else(|| summary.to_owned())
}

/// A checkbox for a boolean.
fn toggle<M: Clone + 'static>(cx: &RowCx<'_, M>, value: bool) -> Div<M> {
    let state = if value {
        ToggleState::On
    } else {
        ToggleState::Off
    };
    h_flex()
        .w_full()
        .h_px(cx.env.theme.size.control)
        .items_center()
        .child(checkbox(
            state,
            (cx.emit)(RowEvent::Change(RowValue::Bool(!value))),
        ))
}

/// What a drag value's event means for the row, given the value it would store.
fn drag_event(event: DragValueEvent, slot: usize, change: impl Fn(f64) -> RowValue) -> RowEvent {
    match event {
        DragValueEvent::Begin => RowEvent::Begin(None),
        DragValueEvent::Change(value) => RowEvent::Change(change(value)),
        DragValueEvent::Commit => RowEvent::Commit,
        DragValueEvent::Cancel => RowEvent::Cancel,
        DragValueEvent::Edit => RowEvent::StartText(slot),
        DragValueEvent::Caret(index, extend) => RowEvent::Caret(index, extend),
    }
}

/// A drag value for an integer or a float.
fn number<M: Clone + 'static>(cx: &RowCx<'_, M>, value: f64) -> Div<M> {
    let emit = cx.emit.clone();
    let current = cx.row.value.clone();
    let mut control = drag_value(value, move |event| {
        emit(drag_event(event, 0, |number| {
            with_slot(&current, 0, number)
        }))
    })
    .precision(precision(cx.row))
    .step(f64::from(cx.row.step.unwrap_or(0.1)))
    .editing(cx.typing(TextRole::Slot(0)));
    if let RowValue::Int(_) = cx.row.value {
        control = control.integer();
    }
    if let Some((low, high)) = cx.row.range {
        control = control.range(f64::from(low), f64::from(high));
    }
    h_flex().w_full().child(control.flex_1())
}

/// Three drag values for a vector or Euler angles.
fn vector<M: Clone + 'static>(cx: &RowCx<'_, M>, values: [f64; 3]) -> Div<M> {
    let emit = cx.emit.clone();
    let current = cx.row.value.clone();
    let editing = match cx.env.typing {
        Some(Typing {
            site:
                super::row_editor::TextSite {
                    row,
                    role: TextRole::Slot(slot),
                },
            edit,
        }) if *row == cx.key => Some((*slot, edit)),
        _ => None,
    };
    vec3_row(
        cx.env.theme,
        values,
        f64::from(cx.row.step.unwrap_or(0.1)),
        editing,
        move |axis, event| {
            emit(drag_event(event, axis, |number| {
                with_slot(&current, axis, number)
            }))
        },
    )
}

/// A colour field with its editor popup.
fn colour<M: Clone + 'static>(cx: &RowCx<'_, M>, color: fr_document::Color) -> Div<M> {
    let emit = cx.emit.clone();
    let open = matches!(cx.popup(), Some(PopupKind::Color));
    let hex = cx.typing(TextRole::Hex);
    color_field(cx.env.theme, to_rgba(color), open, hex, move |event| {
        emit(match event {
            ColorEvent::Toggle => RowEvent::TogglePopup,
            ColorEvent::Dismiss => RowEvent::DismissPopup,
            ColorEvent::Begin(color) => RowEvent::Begin(Some(RowValue::Color(to_color(color)))),
            ColorEvent::Change(color) => RowEvent::Change(RowValue::Color(to_color(color))),
            ColorEvent::Commit => RowEvent::Commit,
            ColorEvent::Cancel => RowEvent::Cancel,
            ColorEvent::HexEdit => RowEvent::HexEdit,
            ColorEvent::HexCaret(index, extend) => RowEvent::HexCaret(index, extend),
        })
    })
}

/// A text field that starts typing when clicked.
fn line<M: Clone + 'static>(cx: &RowCx<'_, M>, value: &str) -> Div<M> {
    let (press, drag) = (cx.emit.clone(), cx.emit.clone());
    let control = match cx.typing(TextRole::Slot(0)) {
        Some(edit) => text_field(edit, true)
            .on_press(move |index| press(RowEvent::Caret(index, false)))
            .on_drag(move |index| drag(RowEvent::Caret(index, true))),
        None => field(value, 0, false).on_press(move |_| press(RowEvent::StartText(0))),
    };
    v_flex().w_full().child(control.w_full())
}

/// A combo box of an enum's options.
fn choice<M: Clone + 'static>(
    cx: &RowCx<'_, M>,
    selected: &str,
    options: &[crate::subsystems::RowOption],
) -> Div<M> {
    let labels: Vec<&str> = options.iter().map(|option| option.label.as_str()).collect();
    let index = options.iter().position(|option| option.key == selected);
    let all = options.to_vec();
    let emit = cx.emit.clone();
    let open = matches!(cx.popup(), Some(PopupKind::Combo));
    combo(
        cx.env.theme,
        &labels,
        index,
        open,
        Some(cx.env.editor.popup_scroll()),
        move |event| {
            emit(match event {
                ComboEvent::Toggle => RowEvent::TogglePopup,
                ComboEvent::Dismiss => RowEvent::DismissPopup,
                ComboEvent::Scroll(scroll) => RowEvent::Scroll(scroll),
                ComboEvent::Select(chosen) => match all.get(chosen) {
                    Some(option) => RowEvent::Pick(RowValue::Enum {
                        selected: option.key.clone(),
                        options: all.clone(),
                    }),
                    None => RowEvent::DismissPopup,
                },
            })
        },
    )
}

/// An asset slot.
fn asset<M: Clone + 'static>(
    cx: &RowCx<'_, M>,
    reference: &fr_document::AssetReference,
    read_only: bool,
) -> Div<M> {
    let open = matches!(cx.popup(), Some(PopupKind::Asset));
    let focused = cx.typing(TextRole::Search);
    AssetSlot {
        theme: cx.env.theme,
        index: cx.env.assets,
        key: &cx.row.key,
        reference,
        open,
        search: focused.unwrap_or(cx.env.search),
        search_focused: focused.is_some(),
        scroll: cx.env.editor.popup_scroll(),
        read_only,
    }
    .view(&cx.emit)
}
