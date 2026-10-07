//! The dock layout as plain data: a tree of splits and groups of tabs.
//!
//! A [`DockTree`] says which panels exist, how they are grouped into tabs and
//! how the groups divide the window. It knows nothing of what a panel shows:
//! a panel is a name. The tree has three kinds of node: a [`Split`](DockNode::Split)
//! dividing its space between two children, a [`Tabs`](DockNode::Tabs) group
//! holding panels one of which shows, and [`Open`](DockNode::Open), the one
//! leaf that shows nothing so that whatever is drawn behind the window, a 3D
//! view for instance, shows through it.
//!
//! A node is addressed by a [`DockPath`], the choices of first or second child
//! taken from the root. Every operation that changes the layout leaves the tree
//! tidy: no group is empty, and a split that lost a child is replaced by the
//! child that is left.

use crate::style::Axis;

/// The narrowest and widest share a split keeps for its first child.
const SHARE_LIMITS: (f32, f32) = (0.05, 0.95);

/// The share a new split gives to the group that was dropped into it.
const NEW_GROUP_SHARE: f32 = 0.5;

/// A node of the dock tree, found by walking from the root.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct DockPath(Vec<bool>);

impl DockPath {
    /// The path of the root.
    pub fn root() -> Self {
        Self::default()
    }

    /// The path of the first child, or of the second when `second` is set.
    pub fn child(&self, second: bool) -> Self {
        let mut steps = self.0.clone();
        steps.push(second);
        Self(steps)
    }

    /// The path of the parent, or nothing for the root.
    pub fn parent(&self) -> Option<Self> {
        let mut steps = self.0.clone();
        steps.pop()?;
        Some(Self(steps))
    }

    /// How deep the node is: the number of choices from the root.
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    /// The choices from the root, `false` for the first child and `true` for the second.
    pub fn steps(&self) -> &[bool] {
        &self.0
    }

    /// The path written as letters, `a` for the first child and `b` for the
    /// second, which is how a node is named in a [`Rects`](crate::Rects).
    pub fn key(&self) -> String {
        self.0
            .iter()
            .map(|second| if *second { 'b' } else { 'a' })
            .collect()
    }

    /// The path written by [`DockPath::key`], when `key` is one.
    pub fn from_key(key: &str) -> Option<Self> {
        key.chars()
            .map(|letter| match letter {
                'a' => Some(false),
                'b' => Some(true),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(Self)
    }
}

/// Which side of a group a panel is dropped against to split it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSide {
    /// The new group goes to the left.
    Left,
    /// The new group goes to the right.
    Right,
    /// The new group goes above.
    Top,
    /// The new group goes below.
    Bottom,
}

impl DockSide {
    /// The axis a split on this side divides along.
    pub fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Top | Self::Bottom => Axis::Vertical,
        }
    }

    /// Whether the new group is the first child of the split.
    fn is_first(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// One node of the dock tree.
#[derive(Clone, Debug, PartialEq)]
pub enum DockNode {
    /// Two children dividing the space along an axis.
    Split {
        /// The axis the children are placed along: horizontal is side by side.
        axis: Axis,
        /// The fraction of the space the first child takes.
        share: f32,
        /// The first child: left or top.
        a: Box<DockNode>,
        /// The second child: right or bottom.
        b: Box<DockNode>,
    },
    /// A group of panels shown as tabs.
    Tabs {
        /// The panels, in tab order.
        ids: Vec<String>,
        /// The index of the one showing.
        active: usize,
    },
    /// The leaf that shows nothing, so what is behind the window shows through.
    Open,
}

impl DockNode {
    /// A split of `a` and `b` along `axis`, `a` taking the fraction `share`.
    pub fn split(axis: Axis, share: f32, a: Self, b: Self) -> Self {
        Self::Split {
            axis,
            share,
            a: Box::new(a),
            b: Box::new(b),
        }
    }

    /// A group of the panels `ids`, the first showing.
    pub fn tabs(ids: &[&str]) -> Self {
        Self::Tabs {
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            active: 0,
        }
    }

    /// The node `path` leads to below this one.
    fn at(&self, path: &[bool]) -> Option<&Self> {
        let Some((step, rest)) = path.split_first() else {
            return Some(self);
        };
        match self {
            Self::Split { a, b, .. } => if *step { b } else { a }.at(rest),
            _ => None,
        }
    }

    /// The node `path` leads to below this one, to change.
    fn at_mut(&mut self, path: &[bool]) -> Option<&mut Self> {
        let Some((step, rest)) = path.split_first() else {
            return Some(self);
        };
        match self {
            Self::Split { a, b, .. } => if *step { b } else { a }.at_mut(rest),
            _ => None,
        }
    }

    /// Calls `visit` with every node below and including this one, with its path.
    fn walk<'a>(&'a self, path: DockPath, visit: &mut impl FnMut(&DockPath, &'a Self)) {
        visit(&path, self);
        if let Self::Split { a, b, .. } = self {
            a.walk(path.child(false), visit);
            b.walk(path.child(true), visit);
        }
    }

    /// Drops what is empty, returning whether this node is itself empty.
    ///
    /// A group without panels is removed, and a split left with one child
    /// becomes that child.
    fn tidy(&mut self) -> bool {
        match self {
            Self::Tabs { ids, active } => {
                *active = (*active).min(ids.len().saturating_sub(1));
                ids.is_empty()
            }
            Self::Open => false,
            Self::Split { a, b, .. } => {
                let (a_empty, b_empty) = (a.tidy(), b.tidy());
                match (a_empty, b_empty) {
                    (true, true) => true,
                    (true, false) => {
                        let survivor = std::mem::replace(b.as_mut(), Self::Open);
                        *self = survivor;
                        false
                    }
                    (false, true) => {
                        let survivor = std::mem::replace(a.as_mut(), Self::Open);
                        *self = survivor;
                        false
                    }
                    (false, false) => false,
                }
            }
        }
    }
}

/// The layout of a dock: panels in groups of tabs, groups in splits.
#[derive(Clone, Debug, PartialEq)]
pub struct DockTree {
    /// The root of the layout.
    root: DockNode,
}

impl DockTree {
    /// A tree rooted at `root`, tidied.
    pub fn new(root: DockNode) -> Self {
        let mut tree = Self { root };
        tree.tidy();
        tree
    }

    /// A tree rooted at `root`, as it is.
    pub(super) fn raw(root: DockNode) -> Self {
        Self { root }
    }

    /// The root node.
    pub fn root(&self) -> &DockNode {
        &self.root
    }

    /// The node `path` leads to.
    pub fn node(&self, path: &DockPath) -> Option<&DockNode> {
        self.root.at(path.steps())
    }

    /// Every panel in the tree, in layout order.
    pub fn panel_ids(&self) -> Vec<&str> {
        let mut found = Vec::new();
        self.root.walk(DockPath::root(), &mut |_, node| {
            if let DockNode::Tabs { ids, .. } = node {
                found.extend(ids.iter().map(String::as_str));
            }
        });
        found
    }

    /// Whether panel `id` is in the tree.
    pub fn contains(&self, id: &str) -> bool {
        self.group_of(id).is_some()
    }

    /// The path of the group holding panel `id`.
    pub fn group_of(&self, id: &str) -> Option<DockPath> {
        let mut found = None;
        self.root.walk(DockPath::root(), &mut |path, node| {
            if let DockNode::Tabs { ids, .. } = node
                && ids.iter().any(|candidate| candidate == id)
            {
                found = Some(path.clone());
            }
        });
        found
    }

    /// The path of every group of tabs, in layout order.
    pub fn groups(&self) -> Vec<DockPath> {
        let mut found = Vec::new();
        self.root.walk(DockPath::root(), &mut |path, node| {
            if matches!(node, DockNode::Tabs { .. }) {
                found.push(path.clone());
            }
        });
        found
    }

    /// The path of the open leaf, if the tree has one.
    pub fn open_path(&self) -> Option<DockPath> {
        let mut found = None;
        self.root.walk(DockPath::root(), &mut |path, node| {
            if matches!(node, DockNode::Open) {
                found = Some(path.clone());
            }
        });
        found
    }

    /// The panels of the group at `path` and the index of the one showing.
    pub fn tabs_at(&self, path: &DockPath) -> Option<(&[String], usize)> {
        match self.node(path)? {
            DockNode::Tabs { ids, active } => Some((ids, *active)),
            _ => None,
        }
    }

    /// The panel showing in the group at `path`.
    pub fn active(&self, path: &DockPath) -> Option<&str> {
        let (ids, active) = self.tabs_at(path)?;
        ids.get(active).map(String::as_str)
    }

    /// Makes panel `id` the one showing in its group; false when it is not in the tree.
    pub fn activate(&mut self, id: &str) -> bool {
        let Some(path) = self.group_of(id) else {
            return false;
        };
        if let Some(DockNode::Tabs { ids, active }) = self.root.at_mut(path.steps())
            && let Some(position) = ids.iter().position(|candidate| candidate == id)
        {
            *active = position;
        }
        true
    }

    /// Gives the first child of the split at `path` the fraction `share`.
    pub fn set_share(&mut self, path: &DockPath, share: f32) -> bool {
        match self.root.at_mut(path.steps()) {
            Some(DockNode::Split { share: current, .. }) => {
                *current = share.clamp(SHARE_LIMITS.0, SHARE_LIMITS.1);
                true
            }
            _ => false,
        }
    }

    /// Removes panel `id` from the tree; false when it is not in it.
    pub fn close(&mut self, id: &str) -> bool {
        if !self.take_panel(id) {
            return false;
        }
        self.tidy();
        true
    }

    /// Moves panel `id` into the group at `target`, at tab `index` or at the end,
    /// and makes it the one showing; false when nothing could be moved.
    pub fn move_tab(&mut self, id: &str, target: &DockPath, index: Option<usize>) -> bool {
        if !self.contains(id) || self.tabs_at(target).is_none() {
            return false;
        }
        let from = self.group_of(id);
        let before = self
            .tabs_at(target)
            .and_then(|(ids, _)| ids.iter().position(|candidate| candidate == id));
        self.take_panel(id);
        let Some(DockNode::Tabs { ids, active }) = self.root.at_mut(target.steps()) else {
            return false;
        };
        let mut at = index.unwrap_or(ids.len());
        if from.as_ref() == Some(target)
            && let Some(old) = before
            && at > old
        {
            at -= 1;
        }
        let at = at.min(ids.len());
        ids.insert(at, id.to_owned());
        *active = at;
        self.tidy();
        true
    }

    /// Moves panel `id` into a new group beside the node at `target`, on `side`
    /// of it; false when nothing could be moved.
    ///
    /// The target may be a group or the open leaf. A group holding only `id`
    /// cannot be split by it.
    pub fn split_tab(&mut self, id: &str, target: &DockPath, side: DockSide) -> bool {
        if !self.contains(id) {
            return false;
        }
        match self.node(target) {
            Some(DockNode::Tabs { ids, .. }) if ids.len() == 1 && ids[0] == id => return false,
            Some(DockNode::Tabs { .. } | DockNode::Open) => {}
            _ => return false,
        }
        self.take_panel(id);
        let Some(node) = self.root.at_mut(target.steps()) else {
            return false;
        };
        let old = std::mem::replace(node, DockNode::Open);
        let fresh = DockNode::Tabs {
            ids: vec![id.to_owned()],
            active: 0,
        };
        let (a, b) = if side.is_first() {
            (fresh, old)
        } else {
            (old, fresh)
        };
        *node = DockNode::split(side.axis(), NEW_GROUP_SHARE, a, b);
        self.tidy();
        true
    }

    /// Makes panel `id` appear: showing it if it is in the tree, and otherwise
    /// adding it to the first group, or beside the open leaf when there is none.
    pub fn show(&mut self, id: &str) -> bool {
        if self.activate(id) {
            return true;
        }
        if let Some(group) = self.groups().first() {
            return self.add_to_group(id, group);
        }
        let Some(open) = self.open_path() else {
            self.root = DockNode::tabs(&[id]);
            return true;
        };
        let Some(node) = self.root.at_mut(open.steps()) else {
            return false;
        };
        let old = std::mem::replace(node, DockNode::Open);
        *node = DockNode::split(Axis::Horizontal, 0.75, old, DockNode::tabs(&[id]));
        true
    }

    /// Adds a panel `id` that is not yet in the tree to the group at `target`,
    /// showing it; false when it is already in the tree or `target` is not a group.
    pub fn add_to_group(&mut self, id: &str, target: &DockPath) -> bool {
        if self.contains(id) {
            return false;
        }
        match self.root.at_mut(target.steps()) {
            Some(DockNode::Tabs { ids, active }) => {
                ids.push(id.to_owned());
                *active = ids.len() - 1;
                true
            }
            _ => false,
        }
    }

    /// Replaces the layout with a copy of `layout`.
    pub fn reset(&mut self, layout: &Self) {
        *self = layout.clone();
    }

    /// Whether the tree is sound: shares inside their limits, no panel twice,
    /// no empty group, an active tab that exists and at most one open leaf.
    pub fn is_valid(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut sound = true;
        let mut opens = 0;
        self.root.walk(DockPath::root(), &mut |_, node| match node {
            DockNode::Split { share, .. } => {
                sound &= (SHARE_LIMITS.0..=SHARE_LIMITS.1).contains(share);
            }
            DockNode::Tabs { ids, active } => {
                sound &= !ids.is_empty() && *active < ids.len();
                sound &= ids
                    .iter()
                    .all(|id| !id.is_empty() && seen.insert(id.as_str()));
            }
            DockNode::Open => opens += 1,
        });
        sound && opens <= 1
    }

    /// Takes panel `id` out of its group, leaving the tree untidy.
    fn take_panel(&mut self, id: &str) -> bool {
        let Some(path) = self.group_of(id) else {
            return false;
        };
        let Some(DockNode::Tabs { ids, active }) = self.root.at_mut(path.steps()) else {
            return false;
        };
        let Some(position) = ids.iter().position(|candidate| candidate == id) else {
            return false;
        };
        ids.remove(position);
        if position < *active || *active >= ids.len() {
            *active = active.saturating_sub(1);
        }
        true
    }

    /// Tidies the whole tree; an empty root becomes an empty group.
    fn tidy(&mut self) {
        if self.root.tidy() {
            self.root = DockNode::Tabs {
                ids: Vec::new(),
                active: 0,
            };
        }
    }
}
