//! A scene editor mock-up: an icon rail, a settings panel and an object list.

use fr_engine::ui::{
    Div, IconName, IconSize, Styled, Theme, ToggleState, checkbox, h_flex, icon, text, v_flex,
};
use fr_engine::{App, Frame};

/// The width of the settings panel, in logical pixels.
const PANEL_WIDTH: f32 = 224.0;

/// The width of the object list, in logical pixels.
const LIST_WIDTH: f32 = 168.0;

/// The number of objects the list shows.
const OBJECTS: usize = 10;

/// The checkboxes of the panel, grouped under their headings.
const GROUPS: [(&str, &[&str]); 3] = [
    (
        "Developer material",
        &["Face measurements", "Receive scene lighting"],
    ),
    ("Snapping", &["Snap", "Snap to other objects"]),
    ("Guides", &["Show the grid", "Light markers", "Wireframe"]),
];

/// The line counts the segmented row offers.
const LINES: [&str; 4] = ["240", "480", "720", "Full"];

/// The tools on the rail, top to bottom.
const RAIL: [IconName; 6] = [
    IconName::Cursor,
    IconName::Box,
    IconName::Palette,
    IconName::Sun,
    IconName::Layout,
    IconName::Hierarchy,
];

/// What the controls of the window send.
#[derive(Clone, Copy, Debug)]
enum Message {
    /// Picks the tool at the index on the rail.
    Tool(usize),
    /// Flips the option at the index.
    Toggle(usize),
    /// Picks the segment at the index.
    Segment(usize),
    /// Selects the object at the index.
    Select(usize),
}

/// The state the window is drawn from.
struct Editor {
    /// The tool picked on the rail.
    tool: usize,
    /// Whether each option is ticked.
    options: [bool; 7],
    /// The segment picked in the row of line counts.
    segment: usize,
    /// The object selected in the list.
    selected: usize,
}

/// A translucent panel `width` pixels wide holding `title` and `body`.
fn panel(theme: &Theme, width: f32, title: &str, body: Div<Message>) -> Div<Message> {
    v_flex()
        .w_px(width)
        .p(3)
        .gap(2)
        .bg(theme.colors.surface)
        .rounded(theme.radius.lg)
        .border_1(theme.colors.border_variant)
        .child(
            text(title)
                .text_lg()
                .font_semibold()
                .color(theme.colors.text),
        )
        .child(body)
}

/// A small heading over a group of rows.
fn heading(theme: &Theme, label: &str) -> Div<Message> {
    v_flex().pt(2).child(
        text(label)
            .text_sm()
            .font_semibold()
            .color(theme.colors.text_muted),
    )
}

impl Editor {
    /// The square buttons down the left edge.
    fn rail(&self, theme: &Theme) -> Div<Message> {
        v_flex()
            .gap(1)
            .children(RAIL.iter().enumerate().map(|(index, name)| {
                let active = index == self.tool;
                let tint = if active {
                    theme.colors.text_on_accent
                } else {
                    theme.colors.text_muted
                };
                v_flex()
                    .size_px(36.0)
                    .items_center()
                    .justify_center()
                    .rounded(theme.radius.md)
                    .border_1(theme.colors.border)
                    .bg(if active {
                        theme.colors.accent
                    } else {
                        theme.colors.surface
                    })
                    .hover_bg(if active {
                        theme.colors.accent_hover
                    } else {
                        theme.colors.surface_hover
                    })
                    .on_click(Message::Tool(index))
                    .child(icon(*name).size(IconSize::Large).color(tint))
            }))
    }

    /// One checkbox beside its label.
    fn option(&self, theme: &Theme, index: usize, label: &str) -> Div<Message> {
        let state = if self.options[index] {
            ToggleState::On
        } else {
            ToggleState::Off
        };
        h_flex()
            .gap(2)
            .items_center()
            .h_px(theme.size.row)
            .child(checkbox(state, Message::Toggle(index)))
            .child(text(label).color(theme.colors.text))
    }

    /// The headed groups of checkboxes.
    fn groups(&self, theme: &Theme) -> Div<Message> {
        let mut column = v_flex().gap(0.5);
        let mut index = 0;
        for (title, labels) in GROUPS {
            column = column.child(heading(theme, title));
            for label in labels {
                column = column.child(self.option(theme, index, label));
                index += 1;
            }
        }
        column
    }

    /// The row of line counts, one of which is picked.
    fn segments(&self, theme: &Theme) -> Div<Message> {
        h_flex()
            .w_full()
            .rounded(theme.radius.md)
            .overflow_hidden()
            .border_1(theme.colors.border_variant)
            .children(LINES.iter().enumerate().map(|(index, label)| {
                let picked = index == self.segment;
                v_flex()
                    .flex_1()
                    .h_px(theme.size.field)
                    .items_center()
                    .justify_center()
                    .bg(if picked {
                        theme.colors.surface_selected
                    } else {
                        theme.colors.surface_input
                    })
                    .hover_bg(theme.colors.surface_hover)
                    .on_click(Message::Segment(index))
                    .child(text(*label).color(theme.colors.text))
            }))
    }

    /// The left panel: headed groups and the segmented row.
    fn scene(&self, theme: &Theme) -> Div<Message> {
        let body = v_flex()
            .gap(1)
            .child(self.groups(theme))
            .child(heading(theme, "Look"))
            .child(text("Lines").text_sm().color(theme.colors.text_muted))
            .child(self.segments(theme));
        panel(theme, PANEL_WIDTH, "Scene", body)
    }

    /// The right panel: one row per object, the selected one lifted.
    fn objects(&self, theme: &Theme) -> Div<Message> {
        let rows = v_flex().gap(0.5).children((0..OBJECTS).map(|index| {
            let selected = index == self.selected;
            h_flex()
                .w_full()
                .h_px(theme.size.row + 6.0)
                .px(2)
                .gap(2)
                .items_center()
                .rounded(theme.radius.md)
                .bg(if selected {
                    theme.colors.surface_selected
                } else {
                    theme.colors.surface
                })
                .hover_bg(theme.colors.surface_hover)
                .on_click(Message::Select(index))
                .child(
                    icon(IconName::Box)
                        .size(IconSize::Medium)
                        .color(theme.colors.text_muted),
                )
                .child(text(format!("Block {}", index + 1)).color(theme.colors.text))
        }));
        panel(theme, LIST_WIDTH, "Objects", rows)
    }
}

impl App for Editor {
    type Message = Message;

    /// Keeps no per-frame state.
    fn update(&mut self, _frame: &Frame) {}

    /// Applies the message a control sent.
    fn message(&mut self, message: Message) {
        match message {
            Message::Tool(index) => self.tool = index,
            Message::Toggle(index) => self.options[index] = !self.options[index],
            Message::Segment(index) => self.segment = index,
            Message::Select(index) => self.selected = index,
        }
    }

    /// Lays out the rail, the scene panel and the object list.
    fn view(&self, theme: &Theme) -> Div<Message> {
        h_flex()
            .w_full()
            .h_full()
            .p(2)
            .gap(2)
            .items_start()
            .child(self.rail(theme))
            .child(self.scene(theme))
            .child(v_flex().flex_1())
            .child(self.objects(theme))
    }
}

fn main() -> std::process::ExitCode {
    let editor = Editor {
        tool: 5,
        options: [true, true, true, true, false, true, false],
        segment: 2,
        selected: 3,
    };
    match fr_engine::run("forge ui", editor) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
