//! Rows of a tree and of a plain list.
//!
//! A tree row draws one line of a hierarchy: its indent, the chevron that
//! expands or collapses it, an icon and a label, lit when selected. It answers
//! to a click, a double click, the secondary button, the chevron, and a drag
//! that carries it somewhere. Where a carried row would land on another is shown
//! as a [`DropHint`] the caller passes to the row under the pointer; the caller
//! finds that row from the bounds the rows recorded in a [`Rects`], and
//! [`drop_zone`] says which part of it the pointer is over.

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Rect, Size};
use fr_render::Quad;

use crate::div::{Div, h_flex, v_flex};
use crate::drag::DragEvent;
use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::icons::{IconName, IconSize, icon};
use crate::rects::Rects;
use crate::style::{Length, Style, Styled};
use crate::text::text;
use crate::theme::Theme;

/// How far each level of the tree is indented, in logical pixels.
const INDENT: f32 = 16.0;

/// The side of the column the chevron sits in, in logical pixels.
const CHEVRON_SLOT: f32 = 16.0;

/// Thickness of the line that marks a drop before or after a row.
const DROP_LINE: f32 = 2.0;

/// The fraction of a row's height at its top and bottom that counts as before and after.
const EDGE_ZONE: f32 = 0.25;

/// Where a carried row would land relative to the row it is over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropHint {
    /// Just before the row, as its sibling.
    Before,
    /// Inside the row, as its child.
    Onto,
    /// Just after the row, as its sibling.
    After,
}

/// Which part of `row` the height `y` falls in: the top quarter is before, the
/// bottom quarter after, and the middle onto the row itself.
pub fn drop_zone(row: Rect, y: f32) -> DropHint {
    let into = ((y - row.top()) / row.size.height.max(1.0)).clamp(0.0, 1.0);
    if into < EDGE_ZONE {
        DropHint::Before
    } else if into > 1.0 - EDGE_ZONE {
        DropHint::After
    } else {
        DropHint::Onto
    }
}

/// What one tree row is made of, before the theme is known.
struct RowSpec<M> {
    /// How deep in the tree the row is.
    depth: usize,
    /// What the row says.
    label: String,
    /// The icon before the label.
    icon: Option<IconName>,
    /// The tint of the icon, when it is not the quiet default.
    icon_color: Option<Rgba>,
    /// Whether the row can expand, and whether it is open.
    expanded: Option<bool>,
    /// Whether the row is selected.
    selected: bool,
    /// Whether the row is drawn dimmed, as something hidden or disabled is.
    dimmed: bool,
    /// What clicking the row sends.
    click: Option<M>,
    /// What clicking the chevron sends.
    toggle: Option<M>,
    /// What clicking the row twice sends.
    double_click: Option<M>,
    /// What the secondary button on the row sends.
    secondary: Option<M>,
    /// What carrying the row sends.
    drag: Option<Arc<dyn Fn(DragEvent) -> M>>,
    /// Where to leave the row's bounds, and under which key.
    record: Option<(Rects, String)>,
    /// Something drawn at the right end of the row.
    trailing: Option<Box<dyn Element<M>>>,
}

/// One row of a tree: indent, chevron, icon and label.
pub struct TreeRow<M> {
    /// What the row is made of, until it is measured.
    spec: Option<RowSpec<M>>,
    /// The row built against the theme, once it is measured.
    built: Option<Div<M>>,
    /// Where a carried row would land on this one, drawn over the row.
    drop: Option<DropHint>,
    /// How the row is sized.
    style: Style,
}

/// A row `depth` levels down, saying `label`.
pub fn tree_row<M>(depth: usize, label: impl Into<String>) -> TreeRow<M> {
    TreeRow {
        spec: Some(RowSpec {
            depth,
            label: label.into(),
            icon: None,
            icon_color: None,
            expanded: None,
            selected: false,
            dimmed: false,
            click: None,
            toggle: None,
            double_click: None,
            secondary: None,
            drag: None,
            record: None,
            trailing: None,
        }),
        built: None,
        drop: None,
        style: Style {
            width: Length::Full,
            ..Style::default()
        },
    }
}

impl<M> TreeRow<M> {
    /// Edits the spec the row is still made of.
    fn with(mut self, edit: impl FnOnce(&mut RowSpec<M>)) -> Self {
        if let Some(spec) = self.spec.as_mut() {
            edit(spec);
        }
        self
    }

    /// Returns this row showing `icon` before its label.
    pub fn icon(self, icon: IconName) -> Self {
        self.with(|spec| spec.icon = Some(icon))
    }

    /// Returns this row's icon tinted `color`.
    pub fn icon_color(self, color: Rgba) -> Self {
        self.with(|spec| spec.icon_color = Some(color))
    }

    /// Returns this row expandable, open when `expanded`, its chevron sending `toggle`.
    pub fn expandable(self, expanded: bool, toggle: M) -> Self {
        self.with(|spec| {
            spec.expanded = Some(expanded);
            spec.toggle = Some(toggle);
        })
    }

    /// Returns this row lit as selected when `selected`.
    pub fn selected(self, selected: bool) -> Self {
        self.with(|spec| spec.selected = selected)
    }

    /// Returns this row drawn dimmed when `dimmed`.
    pub fn dimmed(self, dimmed: bool) -> Self {
        self.with(|spec| spec.dimmed = dimmed)
    }

    /// Returns this row marked as the place a carried row would land, when there is a hint.
    pub fn drop_hint(mut self, hint: Option<DropHint>) -> Self {
        self.drop = hint;
        self
    }

    /// Returns this row sending `message` when it is clicked.
    pub fn on_click(self, message: M) -> Self {
        self.with(|spec| spec.click = Some(message))
    }

    /// Returns this row sending `message` when it is clicked twice.
    pub fn on_double_click(self, message: M) -> Self {
        self.with(|spec| spec.double_click = Some(message))
    }

    /// Returns this row sending `message` when the secondary button goes down on it.
    pub fn on_secondary_click(self, message: M) -> Self {
        self.with(|spec| spec.secondary = Some(message))
    }

    /// Returns this row carried by the pointer, reporting through `on_drag`.
    pub fn on_drag(self, on_drag: impl Fn(DragEvent) -> M + 'static) -> Self {
        self.with(|spec| spec.drag = Some(Arc::new(on_drag)))
    }

    /// Returns this row leaving its bounds in `rects` under `key` as it paints.
    pub fn recorded(self, rects: &Rects, key: impl Into<String>) -> Self {
        self.with(|spec| spec.record = Some((rects.clone(), key.into())))
    }

    /// Returns this row with `element` at its right end.
    pub fn trailing(self, element: impl IntoElement<M>) -> Self {
        self.with(|spec| spec.trailing = Some(element.into_element()))
    }
}

impl<M> Styled for TreeRow<M> {
    /// How the row is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

/// Builds the row `spec` describes against `theme`.
fn build_row<M: Clone + 'static>(theme: &Theme, spec: RowSpec<M>) -> Div<M> {
    let colors = theme.colors;
    let background = if spec.selected {
        colors.surface_selected
    } else {
        Rgba::TRANSPARENT
    };
    let (label_color, icon_color) = match (spec.dimmed, spec.selected) {
        (true, _) => (colors.text_subtle, colors.text_subtle),
        (false, true) => (colors.text, colors.text_muted),
        (false, false) => (colors.text, colors.text_subtle),
    };
    let chevron = chevron_slot(theme, spec.expanded, spec.toggle);
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(1.0)
        .pr(2)
        .gap(1)
        .items_center()
        .bg(background)
        .hover_bg(if spec.selected {
            colors.surface_selected
        } else {
            colors.surface_hover
        })
        .when_some(spec.click, Div::on_click)
        .when_some(spec.double_click, Div::on_double_click)
        .when_some(spec.secondary, Div::on_secondary_click)
        .when_some(spec.drag, |row, drag| row.on_drag(move |event| drag(event)))
        .when_some(spec.record, |row, (rects, key)| row.recorded(&rects, key))
        .child(v_flex().w_px(spec.depth as f32 * INDENT))
        .child(chevron)
        .when_some(spec.icon, |row, name| {
            row.child(
                icon(name)
                    .size(IconSize::Medium)
                    .color(spec.icon_color.unwrap_or(icon_color)),
            )
        })
        .child(text(spec.label).text_sm().color(label_color).flex_1())
        .when_some(spec.trailing, |row, trailing| row.child(trailing))
}

/// The column the chevron sits in: the chevron when the row expands, else empty.
fn chevron_slot<M: Clone + 'static>(
    theme: &Theme,
    expanded: Option<bool>,
    toggle: Option<M>,
) -> Div<M> {
    let slot = v_flex()
        .size_px(CHEVRON_SLOT)
        .items_center()
        .justify_center()
        .rounded(theme.radius.sm);
    let Some(expanded) = expanded else {
        return slot;
    };
    let name = if expanded {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    slot.hover_bg(theme.colors.surface_active)
        .when_some(toggle, Div::on_click)
        .child(
            icon(name)
                .size(IconSize::Small)
                .color(theme.colors.text_muted),
        )
}

impl<M: Clone + 'static> Element<M> for TreeRow<M> {
    /// How the row is sized: the width it is offered by default.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Builds the row against the theme and measures it.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        if let Some(spec) = self.spec.take() {
            self.built = Some(build_row(cx.theme, spec));
        }
        match self.built.as_mut() {
            Some(row) => row.measure(available, cx),
            None => Size::zero(),
        }
    }

    /// Paints the row, then the mark of where a carried row would land on it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        if let Some(row) = self.built.as_mut() {
            row.paint(bounds, cx);
        }
        let Some(hint) = self.drop else {
            return;
        };
        let theme = *cx.theme();
        let accent = theme.colors.accent;
        match hint {
            DropHint::Before => cx.quad(Quad::filled(
                Rect::from_xywh(bounds.left(), bounds.top(), bounds.size.width, DROP_LINE),
                accent,
            )),
            DropHint::After => cx.quad(Quad::filled(
                Rect::from_xywh(
                    bounds.left(),
                    bounds.bottom() - DROP_LINE,
                    bounds.size.width,
                    DROP_LINE,
                ),
                accent,
            )),
            DropHint::Onto => cx.quad(
                Quad::filled(bounds, theme.colors.drop_target)
                    .corner_radius(theme.radius.sm)
                    .border(1.0, accent),
            ),
        }
    }
}

/// A plain row of a list saying `label`, lit when `selected`, sending `message` when clicked.
pub fn list_row<M: Clone + 'static>(
    theme: &Theme,
    label: impl Into<String>,
    selected: bool,
    message: M,
) -> Div<M> {
    let background = if selected {
        theme.colors.surface_selected
    } else {
        Rgba::TRANSPARENT
    };
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(2.5)
        .gap(2)
        .items_center()
        .bg(background)
        .hover_bg(if selected {
            theme.colors.surface_selected
        } else {
            theme.colors.surface_hover
        })
        .on_click(message)
        .child(text(label).text_sm().flex_1())
}
