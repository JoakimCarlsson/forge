//! The dock drawn: groups of tabs, sashes between them, and carrying a tab.
//!
//! [`dock_view`] turns a [`DockTree`] into an element. For every panel in the
//! tree it asks the caller for a [`DockPanel`]: a title, an icon and the contents
//! the caller builds from its own state. Everything the user does comes back as
//! a [`DockEvent`] in the caller's message, which the caller hands to
//! [`DockTree::apply`], together with the [`Rects`] the view recorded while
//! painting: the bounds of every group, bar and tab, and of the open leaf under
//! the key `group:` and the path of the leaf, whose contents the caller draws
//! itself.

use std::sync::Arc;

use fr_math::{Point, Rect, Size};
use fr_render::Quad;

use crate::div::{Div, h_flex, v_flex};
use crate::dock::tree::{DockNode, DockPath, DockSide, DockTree};
use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::icons::IconName;
use crate::overlay::overlay;
use crate::rects::{Rects, placed};
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::{Tab, split, tab, tab_bar};

/// The narrowest a pane is dragged to between two sashes.
const MIN_PANE: f32 = 96.0;

/// How much of a group's edge, as a fraction, counts as a drop on that side.
const EDGE_ZONE: f32 = 0.25;

/// Thickness of the line that marks where a tab would be inserted in a bar.
const INSERT_LINE: f32 = 2.0;

/// How far the dragged tab's label is held from the pointer.
const GHOST_OFFSET: f32 = 12.0;

/// One panel, as the caller describes it to the view.
pub struct DockPanel<M> {
    /// What the tab says.
    title: String,
    /// The icon before the title.
    icon: Option<IconName>,
    /// The panel's contents, drawn when it is the one showing in its group.
    body: Box<dyn Element<M>>,
    /// Whether the panel has unsaved changes.
    dirty: bool,
    /// Whether the tab has a close control.
    closable: bool,
}

impl<M> DockPanel<M> {
    /// A panel called `title` showing `body`.
    pub fn new(title: impl Into<String>, body: impl IntoElement<M>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            body: body.into_element(),
            dirty: false,
            closable: true,
        }
    }

    /// Returns this panel's tab showing `icon`.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Returns this panel's tab marked as having unsaved changes when `dirty`.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Returns this panel's tab with a close control when `closable`.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// What the user did to the dock.
#[derive(Clone, Debug, PartialEq)]
pub enum DockEvent {
    /// The tab of panel `id` was clicked.
    Select {
        /// The panel.
        id: String,
    },
    /// The close control of panel `id` was clicked.
    Close {
        /// The panel.
        id: String,
    },
    /// A sash was dragged.
    Resize {
        /// The split it divides.
        path: DockPath,
        /// The fraction of the split its first child now takes.
        share: f32,
        /// The stage of the drag.
        phase: DragPhase,
    },
    /// The tab of panel `id` is being carried.
    DragTab {
        /// The panel.
        id: String,
        /// The drag.
        event: DragEvent,
    },
}

/// Where a carried tab would land if let go of.
#[derive(Clone, Debug, PartialEq)]
pub enum DockDrop {
    /// In a group's bar, at this tab position.
    Tab {
        /// The group.
        group: DockPath,
        /// The position among its tabs.
        index: usize,
    },
    /// Into a group, at the end of its tabs.
    Into {
        /// The group.
        group: DockPath,
    },
    /// Into a new group on one side of a group or of the open leaf.
    Split {
        /// The group or open leaf.
        group: DockPath,
        /// The side of it.
        side: DockSide,
    },
}

/// A tab being carried: which, where the pointer is and where it would land.
#[derive(Clone, Debug, PartialEq)]
pub struct DockDrag {
    /// The panel being carried.
    pub id: String,
    /// Where the pointer is.
    pub position: Point,
    /// Where the tab would land if let go of now.
    pub drop: Option<DockDrop>,
    /// The area to mark as where it would land.
    pub hint: Option<Rect>,
}

/// What [`DockTree::apply`] did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockChange {
    /// The layout did not change.
    None,
    /// The layout changed in the middle of a drag; there is more to come.
    Live,
    /// The layout changed and is at rest: the moment to keep it.
    Settled,
}

/// The dock as the caller's state, rects and message turn it into an element.
pub struct DockView<'a, M> {
    /// The layout to draw.
    tree: &'a DockTree,
    /// Where the bounds of what is painted are recorded.
    rects: &'a Rects,
    /// The tab being carried, if one is.
    drag: Option<&'a DockDrag>,
    /// The panel the keyboard goes to, whose group's tab is lit.
    focused: Option<&'a str>,
    /// Builds the caller's message for what the user does.
    on_event: Arc<dyn Fn(DockEvent) -> M>,
}

/// A dock of `tree`, recording what it paints in `rects` and sending `on_event`.
pub fn dock_view<'a, M>(
    tree: &'a DockTree,
    rects: &'a Rects,
    on_event: impl Fn(DockEvent) -> M + 'static,
) -> DockView<'a, M> {
    DockView {
        tree,
        rects,
        drag: None,
        focused: None,
        on_event: Arc::new(on_event),
    }
}

impl<'a, M: Clone + 'static> DockView<'a, M> {
    /// Returns this view showing the tab being carried and where it would land.
    pub fn drag(mut self, drag: Option<&'a DockDrag>) -> Self {
        self.drag = drag;
        self
    }

    /// Returns this view lighting the tab of the group holding panel `id`.
    pub fn focused(mut self, id: Option<&'a str>) -> Self {
        self.focused = id;
        self
    }

    /// Builds the dock, asking `panel` for every panel in the tree.
    ///
    /// Clears the recorded rects first: what is there afterwards is what the
    /// frame painted.
    pub fn build(self, theme: &Theme, mut panel: impl FnMut(&str) -> DockPanel<M>) -> Div<M> {
        self.rects.clear();
        let root = self.node(theme, self.tree.root(), DockPath::root(), &mut panel);
        let mut dock = v_flex().w_full().h_full().child(root);
        if let Some(drag) = self.drag {
            let title = panel(&drag.id).title;
            dock = dock
                .child(DropMark {
                    rect: drag.hint,
                    insert: matches!(drag.drop, Some(DockDrop::Tab { .. })),
                })
                .child(overlay(
                    drag.position.offset(GHOST_OFFSET, GHOST_OFFSET),
                    ghost(theme, title),
                ));
        }
        dock
    }

    /// Builds the node at `path`.
    fn node(
        &self,
        theme: &Theme,
        node: &DockNode,
        path: DockPath,
        panel: &mut dyn FnMut(&str) -> DockPanel<M>,
    ) -> Box<dyn Element<M>> {
        match node {
            DockNode::Split { axis, share, a, b } => {
                let on_event = self.on_event.clone();
                let split_path = path.clone();
                let first = self.node(theme, a, path.child(false), panel);
                let second = self.node(theme, b, path.child(true), panel);
                split(*axis)
                    .min_extent(MIN_PANE)
                    .child(*share, first)
                    .child(1.0 - *share, second)
                    .on_resize(move |resize| {
                        on_event(DockEvent::Resize {
                            path: split_path.clone(),
                            share: resize.before / (resize.before + resize.after).max(f32::EPSILON),
                            phase: resize.phase,
                        })
                    })
                    .into_element()
            }
            DockNode::Tabs { ids, active } => self.group(theme, &path, ids, *active, panel),
            DockNode::Open => placed(
                self.rects,
                format!("group:{}", path.key()),
                v_flex().w_full().h_full(),
            )
            .into_element(),
        }
    }

    /// Builds the group of tabs at `path`: its bar and the panel showing.
    fn group(
        &self,
        theme: &Theme,
        path: &DockPath,
        ids: &[String],
        active: usize,
        panel: &mut dyn FnMut(&str) -> DockPanel<M>,
    ) -> Box<dyn Element<M>> {
        let focused = self
            .focused
            .is_some_and(|id| ids.iter().any(|candidate| candidate == id));
        let mut tabs = Vec::with_capacity(ids.len());
        let mut showing = None;
        for (index, id) in ids.iter().enumerate() {
            let described = panel(id);
            tabs.push(self.tab_for(id, &described, index == active));
            if index == active {
                showing = Some(described.body);
            }
        }
        let bar =
            tab_bar(theme, tabs, None, focused).recorded(self.rects, format!("bar:{}", path.key()));
        let body = v_flex()
            .w_full()
            .flex_1()
            .bg(theme.colors.background)
            .overflow_hidden()
            .when_some(showing, |body, content| body.child(content));
        v_flex()
            .w_full()
            .h_full()
            .bg(theme.colors.background)
            .recorded(self.rects, format!("group:{}", path.key()))
            .child(bar)
            .child(body)
            .into_element()
    }

    /// Builds the tab of panel `id`.
    fn tab_for(&self, id: &str, described: &DockPanel<M>, active: bool) -> Tab<M> {
        let on_event = self.on_event.clone();
        let carried = id.to_owned();
        let drag_events = on_event.clone();
        let mut built = tab(
            &described.title,
            on_event(DockEvent::Select { id: id.to_owned() }),
        )
        .active(active)
        .dirty(described.dirty)
        .on_drag(move |event| {
            drag_events(DockEvent::DragTab {
                id: carried.clone(),
                event,
            })
        })
        .recorded(self.rects, format!("tab:{id}"));
        if let Some(icon) = described.icon {
            built = built.icon(icon);
        }
        if described.closable {
            built = built.on_close(on_event(DockEvent::Close { id: id.to_owned() }));
        }
        built
    }
}

/// The small label held by the pointer while a tab is carried.
fn ghost<M: Clone + 'static>(theme: &Theme, title: String) -> Div<M> {
    h_flex()
        .h_px(theme.size.control)
        .px(3)
        .items_center()
        .bg(theme.colors.surface_active)
        .border_1(theme.colors.border_focused)
        .rounded(theme.radius.md)
        .child(text(title).text_sm())
}

/// The mark over where a carried tab would land, drawn on a layer of its own.
struct DropMark {
    /// The area to mark, when there is one.
    rect: Option<Rect>,
    /// Whether the mark is the line of an insertion rather than a wash.
    insert: bool,
}

impl<M> Element<M> for DropMark {
    /// Takes no room.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::zero()
    }

    /// Paints the wash, or the insertion line, over everything.
    fn paint(&mut self, _bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let Some(rect) = self.rect else {
            return;
        };
        let theme = *cx.theme();
        cx.push_layer();
        if self.insert {
            cx.quad(Quad::filled(rect, theme.colors.accent));
        } else {
            cx.quad(
                Quad::filled(rect, theme.colors.drop_target)
                    .corner_radius(theme.radius.sm)
                    .border(1.0, theme.colors.accent),
            );
        }
        cx.pop_layer();
    }
}

/// Where a tab of `id` carried to `at` would land, and the area that marks it.
fn drop_at(tree: &DockTree, rects: &Rects, id: &str, at: Point) -> Option<(DockDrop, Rect)> {
    let mut targets = tree.groups();
    targets.extend(tree.open_path());
    for group in targets {
        let key = group.key();
        let Some(whole) = rects.get(&format!("group:{key}")) else {
            continue;
        };
        if !whole.contains(at) {
            continue;
        }
        let bar = rects.get(&format!("bar:{key}"));
        if let Some(bar) = bar.filter(|bar| bar.contains(at)) {
            return tab_drop(tree, rects, group, bar, at.x);
        }
        let body = match bar {
            Some(bar) => Rect::from_xywh(
                whole.left(),
                bar.bottom(),
                whole.size.width,
                (whole.bottom() - bar.bottom()).max(0.0),
            ),
            None => whole,
        };
        return body_drop(tree, id, group, body, at);
    }
    None
}

/// The drop into the bar of `group` at horizontal position `x`.
fn tab_drop(
    tree: &DockTree,
    rects: &Rects,
    group: DockPath,
    bar: Rect,
    x: f32,
) -> Option<(DockDrop, Rect)> {
    let (ids, _) = tree.tabs_at(&group)?;
    let mut index = ids.len();
    let mut line = ids
        .last()
        .and_then(|last| rects.get(&format!("tab:{last}")))
        .map_or(bar.left(), |last| last.right());
    for (position, id) in ids.iter().enumerate() {
        let Some(rect) = rects.get(&format!("tab:{id}")) else {
            continue;
        };
        if x < rect.left() + rect.size.width / 2.0 {
            index = position;
            line = rect.left();
            break;
        }
    }
    let mark = Rect::from_xywh(
        line - INSERT_LINE / 2.0,
        bar.top(),
        INSERT_LINE,
        bar.size.height,
    );
    Some((DockDrop::Tab { group, index }, mark))
}

/// The drop onto the body of `group`: a side to split, or into the group.
fn body_drop(
    tree: &DockTree,
    id: &str,
    group: DockPath,
    body: Rect,
    at: Point,
) -> Option<(DockDrop, Rect)> {
    let alone = tree
        .tabs_at(&group)
        .is_some_and(|(ids, _)| ids.len() == 1 && ids[0] == id);
    let fx = (at.x - body.left()) / body.size.width.max(1.0);
    let fy = (at.y - body.top()) / body.size.height.max(1.0);
    let side = if fx < EDGE_ZONE {
        Some(DockSide::Left)
    } else if fx > 1.0 - EDGE_ZONE {
        Some(DockSide::Right)
    } else if fy < EDGE_ZONE {
        Some(DockSide::Top)
    } else if fy > 1.0 - EDGE_ZONE {
        Some(DockSide::Bottom)
    } else {
        None
    };
    match side {
        Some(_) if alone => None,
        Some(side) => Some((DockDrop::Split { group, side }, half_of(body, side))),
        None if alone || tree.tabs_at(&group).is_none() => None,
        None => Some((DockDrop::Into { group }, body)),
    }
}

/// The half of `body` on `side`.
fn half_of(body: Rect, side: DockSide) -> Rect {
    let (width, height) = (body.size.width, body.size.height);
    match side {
        DockSide::Left => Rect::from_xywh(body.left(), body.top(), width / 2.0, height),
        DockSide::Right => {
            Rect::from_xywh(body.left() + width / 2.0, body.top(), width / 2.0, height)
        }
        DockSide::Top => Rect::from_xywh(body.left(), body.top(), width, height / 2.0),
        DockSide::Bottom => {
            Rect::from_xywh(body.left(), body.top() + height / 2.0, width, height / 2.0)
        }
    }
}

impl DockTree {
    /// Applies what the user did to the dock, using the bounds the view recorded
    /// in `rects` to find where a carried tab was let go, and keeping what is
    /// being carried in `drag`.
    ///
    /// Selecting, closing, resizing and dropping change the layout; carrying a
    /// tab only updates `drag`, which the view reads to show where it would land.
    /// The result says whether the layout changed and, if so, whether it is at
    /// rest, which is the moment to keep it.
    pub fn apply(
        &mut self,
        event: DockEvent,
        rects: &Rects,
        drag: &mut Option<DockDrag>,
    ) -> DockChange {
        match event {
            DockEvent::Select { id } => settled(self.activate(&id)),
            DockEvent::Close { id } => settled(self.close(&id)),
            DockEvent::Resize { path, share, phase } => {
                if !self.set_share(&path, share) {
                    return DockChange::None;
                }
                match phase {
                    DragPhase::Ended => DockChange::Settled,
                    _ => DockChange::Live,
                }
            }
            DockEvent::DragTab { id, event } => self.apply_drag(id, event, rects, drag),
        }
    }

    /// Applies one event of a carried tab.
    fn apply_drag(
        &mut self,
        id: String,
        event: DragEvent,
        rects: &Rects,
        drag: &mut Option<DockDrag>,
    ) -> DockChange {
        let target = drop_at(self, rects, &id, event.position);
        match event.phase {
            DragPhase::Began | DragPhase::Moved => {
                let (drop, hint) = match target {
                    Some((drop, hint)) => (Some(drop), Some(hint)),
                    None => (None, None),
                };
                *drag = Some(DockDrag {
                    id,
                    position: event.position,
                    drop,
                    hint,
                });
                DockChange::None
            }
            DragPhase::Cancelled => {
                *drag = None;
                DockChange::None
            }
            DragPhase::Ended => {
                *drag = None;
                let Some((drop, _)) = target else {
                    return DockChange::None;
                };
                let moved = match drop {
                    DockDrop::Tab { group, index } => self.move_tab(&id, &group, Some(index)),
                    DockDrop::Into { group } => self.move_tab(&id, &group, None),
                    DockDrop::Split { group, side } => self.split_tab(&id, &group, side),
                };
                settled(moved)
            }
        }
    }
}

/// [`DockChange::Settled`] when `changed`, else nothing.
fn settled(changed: bool) -> DockChange {
    if changed {
        DockChange::Settled
    } else {
        DockChange::None
    }
}
