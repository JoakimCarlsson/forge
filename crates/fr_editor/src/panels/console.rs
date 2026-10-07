//! The Console panel: the log of the editor, the build and the game.

use std::collections::VecDeque;

use fr_color::Rgba;
use fr_ui::{
    Div, IconName, Scroll, ScrollEvent, Styled, Theme, div, h_flex, icon_button, scroll_area, text,
    v_flex,
};

use crate::app::{EditorState, Message};

/// The most lines the console keeps; older ones are dropped.
pub const MAX_LINES: usize = 2000;

/// The height of one line of the log, in logical pixels.
const LINE_HEIGHT: f32 = 18.0;

/// How many lines beyond the visible ones are built, above and below.
const OVERSCAN: usize = 2;

/// What a console line is about, which decides its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    /// An ordinary message of the editor.
    Info,
    /// Something that works but needs attention.
    Warning,
    /// Something that failed.
    Error,
    /// A line the game or the build printed.
    Game,
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    /// What the line is about.
    pub level: ConsoleLevel,
    /// The text, without a line ending.
    pub text: String,
}

/// What the user did in the Console.
#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleMessage {
    /// Empties the log.
    Clear,
    /// The log was scrolled.
    Scroll(ScrollEvent),
}

/// The log and how it is shown.
#[derive(Debug)]
pub struct ConsoleState {
    /// The lines, oldest first.
    lines: VecDeque<ConsoleLine>,
    /// How far the log is scrolled.
    pub scroll: Scroll,
    /// Whether the view follows the newest line.
    pub auto_scroll: bool,
}

impl Default for ConsoleState {
    /// An empty log that follows its end.
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            scroll: Scroll::new(),
            auto_scroll: true,
        }
    }
}

impl ConsoleState {
    /// Adds a line, dropping the oldest beyond [`MAX_LINES`].
    pub fn push(&mut self, level: ConsoleLevel, text: impl Into<String>) {
        self.lines.push_back(ConsoleLine {
            level,
            text: text.into(),
        });
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }

    /// The lines, oldest first.
    pub fn lines(&self) -> &VecDeque<ConsoleLine> {
        &self.lines
    }

    /// Empties the log.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll.scroll_to(0.0);
        self.auto_scroll = true;
    }

    /// Applies a scroll; scrolling up stops following the end and reaching the
    /// end again resumes it.
    pub fn apply_scroll(&mut self, event: ScrollEvent) {
        self.scroll.apply(event);
        self.auto_scroll = self.scroll.offset() >= self.scroll.max_offset() - 1.0;
    }
}

/// The colour a line of a level is drawn in.
fn level_color(theme: &Theme, level: ConsoleLevel) -> Rgba {
    match level {
        ConsoleLevel::Info => theme.colors.text,
        ConsoleLevel::Warning => theme.colors.warning,
        ConsoleLevel::Error => theme.colors.danger,
        ConsoleLevel::Game => theme.colors.text_muted,
    }
}

/// One line as a row.
fn line_row(theme: &Theme, line: &ConsoleLine) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(LINE_HEIGHT)
        .px(2)
        .items_center()
        .overflow_hidden()
        .child(
            text(line.text.clone())
                .text_sm()
                .font_mono()
                .color(level_color(theme, line.level)),
        )
}

/// A blank block as tall as `lines` lines, standing for the ones not built.
fn spacer(lines: usize) -> Div<Message> {
    div().w_full().h_px(lines as f32 * LINE_HEIGHT)
}

/// The panel's contents: a bar with Clear and the log under it.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let console = &state.console;
    if console.auto_scroll {
        console.scroll.scroll_to(f32::MAX);
    }
    let total = console.lines().len();
    let viewport_lines = (console.scroll.viewport_height() / LINE_HEIGHT).ceil() as usize + 1;
    let first = ((console.scroll.offset() / LINE_HEIGHT) as usize)
        .saturating_sub(OVERSCAN)
        .min(total);
    let last = (first + viewport_lines + 2 * OVERSCAN).min(total);
    let rows = console
        .lines()
        .iter()
        .skip(first)
        .take(last - first)
        .map(|line| line_row(theme, line));
    let body = v_flex()
        .w_full()
        .child(spacer(first))
        .children(rows)
        .child(spacer(total - last));
    let bar = h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(2)
        .gap(2)
        .items_center()
        .justify_end()
        .child(
            text(format!("{total} lines"))
                .text_xs()
                .color(theme.colors.text_subtle),
        )
        .child(icon_button(
            theme,
            IconName::Trash,
            Message::Console(ConsoleMessage::Clear),
        ));
    v_flex().w_full().h_full().child(bar).child(
        v_flex().w_full().flex_1().child(
            scroll_area(&console.scroll, body)
                .on_scroll(|event| Message::Console(ConsoleMessage::Scroll(event))),
        ),
    )
}

/// Applies a message of the Console.
pub fn update(state: &mut EditorState, message: ConsoleMessage) {
    match message {
        ConsoleMessage::Clear => state.console.clear(),
        ConsoleMessage::Scroll(event) => state.console.apply_scroll(event),
    }
}
