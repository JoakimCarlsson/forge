//! The editing state of property rows and what the user's events do to the
//! document through them.
//!
//! A row is plain data, an [`InspectorRow`] whose `apply` closure turns an
//! edited value into a command. The widgets of [`rows`](super::rows) report
//! what the user did as a [`RowEvent`]; [`RowEditor::event`] turns it into the
//! gesture protocol of the document: a drag or a colour slider is
//! `begin_gesture`, any number of `preview` calls and one `commit_gesture`, so
//! it is one undo entry; a typed number, a chosen option or a ticked box is one
//! `execute`. The editor also owns which popup is open and which gesture is
//! running, so a panel keeps one [`RowEditor`] and nothing about individual
//! widgets. The line being typed lives in the app's text focus: a
//! [`TextSite`] names the field in the [`TextTarget`] it is given, and the
//! editor answers events that need the keyboard with an [`Effect`].

use fr_authoring::{AuthoringError, OpenDocument};
use fr_document::Guid;
use fr_ui::{Scroll, ScrollEvent, format_hex, format_number, parse_hex, parse_number};

use crate::app::TextTarget;
use crate::subsystems::{InspectorRow, RowValue};

/// The prefix of the names of the text fields of rows.
const SITE_PREFIX: &str = "row";

/// The separator of the parts of a field's name.
const SEPARATOR: char = '|';

/// What a row belongs to, which is how a panel finds the row again when an event
/// for it arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowOwner {
    /// The inspected node itself: its name, activity and transform.
    Node,
    /// One component of the inspected entity.
    Component(Guid),
    /// The settings of the scene being edited.
    Scene,
}

/// Names one row: its owner and its key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RowKey {
    /// What the row belongs to.
    pub owner: RowOwner,
    /// The row's key among the rows of its owner.
    pub key: String,
}

impl RowKey {
    /// The key of the row `key` of `owner`.
    pub fn new(owner: RowOwner, key: impl Into<String>) -> Self {
        Self {
            owner,
            key: key.into(),
        }
    }
}

/// What the user did to one row's widgets.
#[derive(Clone, Debug, PartialEq)]
pub enum RowEvent {
    /// A gesture begins; the colour sliders carry the colour they set at once.
    Begin(Option<RowValue>),
    /// The value is now this: previewed during a gesture, otherwise stored as
    /// one edit.
    Change(RowValue),
    /// A value was chosen from a popup: store it and close the popup.
    Pick(RowValue),
    /// The gesture ended and its last value stands.
    Commit,
    /// The gesture was abandoned.
    Cancel,
    /// A number or text was clicked: start typing into this slot of the row.
    StartText(usize),
    /// The pointer went down or dragged in the text being typed, at this
    /// character; the flag says whether to extend the selection.
    Caret(usize, bool),
    /// The row's popup was clicked: open it, or close it when open.
    TogglePopup,
    /// A press landed outside the row's popup.
    DismissPopup,
    /// The popup's list was scrolled.
    Scroll(ScrollEvent),
    /// The hex code of a colour popup was clicked: start typing it.
    HexEdit,
    /// The pointer went down or dragged in the hex code, at this character.
    HexCaret(usize, bool),
    /// The pointer went down or dragged in a picker's search, at this
    /// character.
    SearchCaret(usize, bool),
}

/// Which text of a row has the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    /// The number or text in a slot of the row: the axis of a vector, zero
    /// otherwise.
    Slot(usize),
    /// The hex code of the row's colour popup.
    Hex,
    /// The search line of the row's asset picker.
    Search,
}

/// A text field of a row: the row and which of its texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSite {
    /// The row.
    pub row: RowKey,
    /// Which of its texts.
    pub role: TextRole,
}

impl TextSite {
    /// The name the app's text focus knows the field by.
    pub fn target(&self) -> TextTarget {
        let owner = match self.row.owner {
            RowOwner::Node => "node".to_owned(),
            RowOwner::Component(id) => format!("component{SEPARATOR}{}", id.to_text()),
            RowOwner::Scene => "scene".to_owned(),
        };
        let role = match self.role {
            TextRole::Slot(slot) => format!("slot{SEPARATOR}{slot}"),
            TextRole::Hex => "hex".to_owned(),
            TextRole::Search => "search".to_owned(),
        };
        TextTarget::Inspector(format!(
            "{SITE_PREFIX}{SEPARATOR}{owner}{SEPARATOR}{}{SEPARATOR}{role}",
            self.row.key
        ))
    }

    /// The field a text focus name stands for; none for a name that is not a
    /// row's.
    pub fn from_target(target: &TextTarget) -> Option<Self> {
        let TextTarget::Inspector(name) = target else {
            return None;
        };
        let mut parts = name.split(SEPARATOR);
        if parts.next()? != SITE_PREFIX {
            return None;
        }
        let owner = match parts.next()? {
            "node" => RowOwner::Node,
            "scene" => RowOwner::Scene,
            "component" => RowOwner::Component(Guid::from_text(parts.next()?)?),
            _ => return None,
        };
        let key = parts.next()?.to_owned();
        let role = match parts.next()? {
            "slot" => TextRole::Slot(parts.next()?.parse().ok()?),
            "hex" => TextRole::Hex,
            "search" => TextRole::Search,
            _ => return None,
        };
        Some(Self {
            row: RowKey::new(owner, key),
            role,
        })
    }
}

/// A text of a row being typed, as the widgets are told.
#[derive(Clone, Copy, Debug)]
pub struct Typing<'a> {
    /// The field.
    pub site: &'a TextSite,
    /// The line.
    pub edit: &'a fr_ui::TextEdit,
}

/// What an event asks of the app's text focus.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Nothing.
    None,
    /// Give the keyboard to this field, showing this text selected.
    Begin(TextSite, String),
    /// Take the keyboard from a hex code or search line whose popup closed.
    End,
    /// Move the caret of the field that has the keyboard.
    Caret(usize, bool),
}

/// What a row's popup shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKind {
    /// The options of an enum.
    Combo,
    /// A colour editor.
    Color,
    /// The assets of a kind.
    Asset,
}

/// The one popup a row has open.
#[derive(Clone, Debug)]
pub struct RowPopup {
    /// The row that opened it.
    pub row: RowKey,
    /// What it shows.
    pub kind: PopupKind,
}

/// What is being gestured and shown open for a set of rows.
#[derive(Clone, Debug, Default)]
pub struct RowEditor {
    /// The popup that is open.
    popup: Option<RowPopup>,
    /// The row whose gesture is open in the document.
    gesture: Option<RowKey>,
    /// How far the open popup's list is scrolled.
    popup_scroll: Scroll,
}

impl RowEditor {
    /// The open popup, if any.
    pub fn popup(&self) -> Option<&RowPopup> {
        self.popup.as_ref()
    }

    /// The scroll of the open popup's list.
    pub fn popup_scroll(&self) -> &Scroll {
        &self.popup_scroll
    }

    /// Whether a gesture is open in the document.
    pub fn gesturing(&self) -> bool {
        self.gesture.is_some()
    }

    /// Forgets the open popup, and abandons a gesture in `document`.
    ///
    /// # Errors
    ///
    /// When a gesture cannot be cancelled.
    pub fn reset(&mut self, document: Option<&mut OpenDocument>) -> Result<(), AuthoringError> {
        self.popup = None;
        let gesture = self.gesture.take();
        match (gesture, document) {
            (Some(_), Some(document)) => document.cancel_gesture(),
            _ => Ok(()),
        }
    }

    /// Applies an event to `row`, which is named by `key`, and says what the
    /// text focus should do about it.
    ///
    /// # Errors
    ///
    /// When the document refuses the command or the gesture.
    pub fn event(
        &mut self,
        document: &mut OpenDocument,
        row: &InspectorRow,
        key: RowKey,
        event: RowEvent,
    ) -> Result<Effect, AuthoringError> {
        match event {
            RowEvent::Begin(initial) => {
                self.gesture = Some(key);
                document.begin_gesture(format!("Edit {}", row.label))?;
                if let Some(value) = initial {
                    document.preview(&row.command(value))?;
                }
                Ok(Effect::None)
            }
            RowEvent::Change(value) => self.store(document, row, &key, value),
            RowEvent::Pick(value) => {
                self.popup = None;
                self.store(document, row, &key, value)?;
                Ok(Effect::End)
            }
            RowEvent::Commit => self.finish(document, true),
            RowEvent::Cancel => self.finish(document, false),
            event => Ok(self.interact(row, key, event)),
        }
    }

    /// Stores a value: previewed while the row's own gesture is open, as one
    /// undo entry otherwise.
    fn store(
        &mut self,
        document: &mut OpenDocument,
        row: &InspectorRow,
        key: &RowKey,
        value: RowValue,
    ) -> Result<Effect, AuthoringError> {
        let command = row.command(value);
        if self.gesture.as_ref() == Some(key) {
            document.preview(&command)?;
        } else {
            document.execute(&command)?;
        }
        Ok(Effect::None)
    }

    /// Ends the open gesture, keeping its result or restoring the document.
    fn finish(
        &mut self,
        document: &mut OpenDocument,
        keep: bool,
    ) -> Result<Effect, AuthoringError> {
        if self.gesture.take().is_none() {
            return Ok(Effect::None);
        }
        if keep {
            document.commit_gesture()?;
        } else {
            document.cancel_gesture()?;
        }
        Ok(Effect::None)
    }

    /// The events that only change what is open or typed.
    fn interact(&mut self, row: &InspectorRow, key: RowKey, event: RowEvent) -> Effect {
        match event {
            RowEvent::StartText(slot) => Effect::Begin(
                TextSite {
                    row: key,
                    role: TextRole::Slot(slot),
                },
                initial_text(row, slot),
            ),
            RowEvent::Caret(index, extend)
            | RowEvent::HexCaret(index, extend)
            | RowEvent::SearchCaret(index, extend) => Effect::Caret(index, extend),
            RowEvent::TogglePopup => self.toggle_popup(row, key),
            RowEvent::DismissPopup => {
                self.popup = None;
                Effect::End
            }
            RowEvent::Scroll(scroll) => {
                self.popup_scroll.apply(scroll);
                Effect::None
            }
            RowEvent::HexEdit => match &row.value {
                RowValue::Color(color) => Effect::Begin(
                    TextSite {
                        row: key,
                        role: TextRole::Hex,
                    },
                    format_hex(to_rgba(*color)),
                ),
                _ => Effect::None,
            },
            _ => Effect::None,
        }
    }

    /// Opens the popup the row's value needs, or closes it when it is open.
    fn toggle_popup(&mut self, row: &InspectorRow, key: RowKey) -> Effect {
        let open = self.popup.as_ref().is_some_and(|popup| popup.row == key);
        self.popup_scroll.scroll_to(0.0);
        self.popup = if open {
            None
        } else {
            popup_kind(&row.value).map(|kind| RowPopup {
                row: key.clone(),
                kind,
            })
        };
        match (&self.popup, open) {
            (
                Some(RowPopup {
                    kind: PopupKind::Asset,
                    ..
                }),
                _,
            ) => Effect::Begin(
                TextSite {
                    row: key,
                    role: TextRole::Search,
                },
                String::new(),
            ),
            _ => Effect::End,
        }
    }
}

/// The popup a value is edited in, when it has one.
fn popup_kind(value: &RowValue) -> Option<PopupKind> {
    match value {
        RowValue::Enum { .. } => Some(PopupKind::Combo),
        RowValue::Color(_) => Some(PopupKind::Color),
        RowValue::Asset(_) => Some(PopupKind::Asset),
        _ => None,
    }
}

/// How many decimals a row's numbers are written with.
pub fn precision(row: &InspectorRow) -> usize {
    match (&row.value, row.step) {
        (RowValue::Int(_), _) => 0,
        (_, Some(step)) if step < 0.01 => 3,
        _ => 2,
    }
}

/// The number of slot `slot` of a row's value, when it has numbers.
fn slot_number(value: &RowValue, slot: usize) -> Option<f64> {
    match value {
        RowValue::Int(value) => Some(*value as f64),
        RowValue::Float(value) => Some(f64::from(*value)),
        RowValue::Vec3(vector) | RowValue::EulerDegrees(vector) => {
            Some(f64::from(vector.to_array()[slot.min(2)]))
        }
        _ => None,
    }
}

/// The text a slot starts being typed with.
fn initial_text(row: &InspectorRow, slot: usize) -> String {
    match (&row.value, slot_number(&row.value, slot)) {
        (RowValue::Text(text), _) => text.clone(),
        (_, Some(number)) => format_number(number, precision(row)),
        _ => String::new(),
    }
}

/// The colour value a hex code stands for; none when it is not a hex code.
pub fn hex_value(text: &str) -> Option<RowValue> {
    parse_hex(text).map(|color| RowValue::Color(to_color(color)))
}

/// The value that typing `text` into slot `slot` of a row makes, held inside the
/// row's range; none when the text is not what the row takes.
pub fn text_value(row: &InspectorRow, slot: usize, text: &str) -> Option<RowValue> {
    if let RowValue::Text(_) = row.value {
        return Some(RowValue::Text(text.to_owned()));
    }
    let mut number = parse_number(text)?;
    if let Some((low, high)) = row.range {
        number = number.clamp(f64::from(low), f64::from(high));
    }
    Some(with_slot(&row.value, slot, number))
}

/// `value` with slot `slot` set to `number`.
pub fn with_slot(value: &RowValue, slot: usize, number: f64) -> RowValue {
    let replaced = |vector: fr_math::Vec3| {
        let mut parts = vector.to_array();
        parts[slot.min(2)] = number as f32;
        fr_math::Vec3::from_array(parts)
    };
    match value {
        RowValue::Int(_) => RowValue::Int(number.round() as i64),
        RowValue::Float(_) => RowValue::Float(number as f32),
        RowValue::Vec3(vector) => RowValue::Vec3(replaced(*vector)),
        RowValue::EulerDegrees(vector) => RowValue::EulerDegrees(replaced(*vector)),
        other => other.clone(),
    }
}

/// A colour of the document as the colour the interface draws.
pub fn to_rgba(color: fr_document::Color) -> fr_ui::Rgba {
    fr_ui::Rgba::new(color.r, color.g, color.b, color.a)
}

/// A colour of the interface as the colour the document stores.
pub fn to_color(color: fr_ui::Rgba) -> fr_document::Color {
    fr_document::Color::rgba(color.r, color.g, color.b, color.a)
}
