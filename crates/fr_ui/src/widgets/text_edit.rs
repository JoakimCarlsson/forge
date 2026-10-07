//! The editing state of one line of text, as a pure value.
//!
//! [`TextEdit`] holds the text, the caret and the selection, and changes them
//! the way a text box does: typing replaces the selection, backspace and delete
//! remove it or the character beside the caret, the arrows move the caret and
//! extend the selection with shift, and control A, C, X and V select everything
//! or ask for the clipboard. It knows nothing of windows or drawing, so the
//! caller feeds it the [`KeyEvent`]s it received and draws the result with a
//! [`field`](crate::field). Positions are counted in characters, not bytes.

use std::ops::Range;

use fr_input::{ButtonState, Key, KeyEvent, Modifiers};

/// What an edit asks the caller to do with the system clipboard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardRequest {
    /// Put this text on the clipboard.
    Copy(String),
    /// Put this text on the clipboard; it has already been removed from the line.
    Cut(String),
    /// Read the clipboard and hand its text to [`TextEdit::insert`].
    Paste,
}

/// What a key did to a [`TextEdit`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EditOutcome {
    /// Whether the key was one the edit understands, changed or not.
    pub handled: bool,
    /// Whether the text changed.
    pub changed: bool,
    /// What the key asks of the clipboard.
    pub clipboard: Option<ClipboardRequest>,
    /// Whether the key was enter.
    pub submitted: bool,
    /// Whether the key was escape.
    pub cancelled: bool,
}

impl EditOutcome {
    /// A key that was understood and changed nothing.
    fn handled() -> Self {
        Self {
            handled: true,
            ..Self::default()
        }
    }

    /// A key that was understood and changed the text.
    fn changed() -> Self {
        Self {
            handled: true,
            changed: true,
            ..Self::default()
        }
    }
}

/// One line of text with a caret and a selection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextEdit {
    /// The line.
    text: String,
    /// The caret, as a count of characters from the start.
    caret: usize,
    /// The other end of the selection, equal to the caret when nothing is selected.
    anchor: usize,
}

impl TextEdit {
    /// An edit holding `text`, with the caret after it.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.chars().count();
        Self {
            text,
            caret: end,
            anchor: end,
        }
    }

    /// An edit holding `text` with all of it selected, as a field is entered.
    pub fn selected(text: impl Into<String>) -> Self {
        let mut edit = Self::new(text);
        edit.select_all();
        edit
    }

    /// The line.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How many characters the line has.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Whether the line is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The caret, as a count of characters from the start.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected characters, when any are selected.
    pub fn selection(&self) -> Option<Range<usize>> {
        (self.caret != self.anchor)
            .then(|| self.caret.min(self.anchor)..self.caret.max(self.anchor))
    }

    /// The selected text, when any is selected.
    pub fn selected_text(&self) -> Option<String> {
        let range = self.selection()?;
        Some(
            self.text
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect(),
        )
    }

    /// Replaces the line with `text`, the caret after it and nothing selected.
    pub fn set_text(&mut self, text: impl Into<String>) {
        *self = Self::new(text);
    }

    /// Selects the whole line, the caret at its end.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len();
    }

    /// Moves the caret to `index`, extending the selection from where it was
    /// when `extend` is set and dropping it otherwise.
    pub fn set_caret(&mut self, index: usize, extend: bool) {
        self.caret = index.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    /// Replaces the selection, or inserts at the caret, with `text`; returns
    /// whether the line changed. Line breaks are dropped.
    pub fn insert(&mut self, text: &str) -> bool {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() && self.selection().is_none() {
            return false;
        }
        self.remove_selection();
        let at = self.byte_of(self.caret);
        self.text.insert_str(at, &clean);
        self.caret += clean.chars().count();
        self.anchor = self.caret;
        true
    }

    /// Removes the selection, or the character before the caret.
    pub fn backspace(&mut self) -> bool {
        if self.remove_selection() {
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        self.anchor = self.caret - 1;
        self.remove_selection()
    }

    /// Removes the selection, or the character after the caret.
    pub fn delete(&mut self) -> bool {
        if self.remove_selection() {
            return true;
        }
        if self.caret >= self.len() {
            return false;
        }
        self.anchor = self.caret + 1;
        self.remove_selection()
    }

    /// Moves the caret one character left; without `extend` a selection
    /// collapses to its start instead.
    pub fn move_left(&mut self, extend: bool) {
        match (self.selection(), extend) {
            (Some(range), false) => self.set_caret(range.start, false),
            _ => self.set_caret(self.caret.saturating_sub(1), extend),
        }
    }

    /// Moves the caret one character right; without `extend` a selection
    /// collapses to its end instead.
    pub fn move_right(&mut self, extend: bool) {
        match (self.selection(), extend) {
            (Some(range), false) => self.set_caret(range.end, false),
            _ => self.set_caret(self.caret + 1, extend),
        }
    }

    /// Moves the caret to the start of the previous word.
    pub fn move_word_left(&mut self, extend: bool) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut at = self.caret;
        while at > 0 && !chars[at - 1].is_alphanumeric() {
            at -= 1;
        }
        while at > 0 && chars[at - 1].is_alphanumeric() {
            at -= 1;
        }
        self.set_caret(at, extend);
    }

    /// Moves the caret to the end of the next word.
    pub fn move_word_right(&mut self, extend: bool) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut at = self.caret;
        while at < chars.len() && !chars[at].is_alphanumeric() {
            at += 1;
        }
        while at < chars.len() && chars[at].is_alphanumeric() {
            at += 1;
        }
        self.set_caret(at, extend);
    }

    /// Moves the caret to the start of the line.
    pub fn home(&mut self, extend: bool) {
        self.set_caret(0, extend);
    }

    /// Moves the caret to the end of the line.
    pub fn end(&mut self, extend: bool) {
        self.set_caret(self.len(), extend);
    }

    /// Applies a key the caller received while this edit had the keyboard.
    ///
    /// Keys that produce text insert it, so the caller should not also feed the
    /// same text to [`TextEdit::insert`]. Releases and keys the edit does not
    /// know come back unhandled.
    pub fn handle_key(&mut self, event: &KeyEvent) -> EditOutcome {
        if event.state != ButtonState::Pressed {
            return EditOutcome::default();
        }
        let modifiers = event.modifiers;
        match &event.key {
            Key::Character(text) if modifiers.control || modifiers.logo => {
                self.handle_shortcut(text)
            }
            Key::Character(text) if !modifiers.alt => self.typed(text),
            Key::Space => self.typed(" "),
            Key::Backspace => self.changed_by(Self::backspace),
            Key::Delete => self.changed_by(Self::delete),
            Key::ArrowLeft => self.moved(modifiers, Self::move_left, Self::move_word_left),
            Key::ArrowRight => self.moved(modifiers, Self::move_right, Self::move_word_right),
            Key::Home => self.moved(modifiers, Self::home, Self::home),
            Key::End => self.moved(modifiers, Self::end, Self::end),
            Key::Enter => EditOutcome {
                submitted: true,
                ..EditOutcome::handled()
            },
            Key::Escape => EditOutcome {
                cancelled: true,
                ..EditOutcome::handled()
            },
            _ => EditOutcome::default(),
        }
    }

    /// The control and command shortcuts: select all, copy, cut and paste.
    fn handle_shortcut(&mut self, text: &str) -> EditOutcome {
        match text.to_lowercase().as_str() {
            "a" => {
                self.select_all();
                EditOutcome::handled()
            }
            "c" => EditOutcome {
                clipboard: self.selected_text().map(ClipboardRequest::Copy),
                ..EditOutcome::handled()
            },
            "x" => {
                let cut = self.selected_text().map(ClipboardRequest::Cut);
                let changed = cut.is_some() && self.remove_selection();
                EditOutcome {
                    changed,
                    clipboard: cut,
                    ..EditOutcome::handled()
                }
            }
            "v" => EditOutcome {
                clipboard: Some(ClipboardRequest::Paste),
                ..EditOutcome::handled()
            },
            _ => EditOutcome::default(),
        }
    }

    /// Inserts typed text, reporting whether anything was inserted.
    fn typed(&mut self, text: &str) -> EditOutcome {
        if self.insert(text) {
            EditOutcome::changed()
        } else {
            EditOutcome::handled()
        }
    }

    /// Runs `change` and reports whether it changed the line.
    fn changed_by(&mut self, change: fn(&mut Self) -> bool) -> EditOutcome {
        if change(self) {
            EditOutcome::changed()
        } else {
            EditOutcome::handled()
        }
    }

    /// Moves the caret with `step`, or with `word` while control is held, and
    /// extends the selection while shift is.
    fn moved(
        &mut self,
        modifiers: Modifiers,
        step: fn(&mut Self, bool),
        word: fn(&mut Self, bool),
    ) -> EditOutcome {
        let motion = if modifiers.control { word } else { step };
        motion(self, modifiers.shift);
        EditOutcome::handled()
    }

    /// Removes the selected characters, reporting whether there were any.
    fn remove_selection(&mut self) -> bool {
        let Some(range) = self.selection() else {
            return false;
        };
        let start = self.byte_of(range.start);
        let end = self.byte_of(range.end);
        self.text.replace_range(start..end, "");
        self.caret = range.start;
        self.anchor = range.start;
        true
    }

    /// The byte offset of the character at `index`, the length of the line past its end.
    fn byte_of(&self, index: usize) -> usize {
        self.text
            .char_indices()
            .nth(index)
            .map_or(self.text.len(), |(at, _)| at)
    }
}
