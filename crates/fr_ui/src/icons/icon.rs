//! The icons the UI draws, and the element that places one.
//!
//! Each icon is one piece of vector artwork shipped with the crate, drawn at
//! whatever size the caller asks for and tinted from the theme. Artwork is
//! never coloured in the file: an icon in a disabled row and the same icon in
//! a selected one are one drawing, tinted twice.

use fr_color::Rgba;
use fr_math::{Rect, Size};
use fr_render::Svg;

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Style, Styled};

/// Reads one icon's artwork out of the crate's own assets.
macro_rules! include_icon {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icons/",
            $name,
            ".svg"
        ))
    };
}

/// One piece of artwork, at whatever size it is asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconName {
    /// The way down to what follows.
    ArrowDown,
    /// Go back to what came before.
    ArrowLeft,
    /// Go on to what follows.
    ArrowRight,
    /// The way up to what came before.
    ArrowUp,
    /// A block, cube or object.
    Box,
    /// A tick: chosen or done.
    Check,
    /// A section that is open.
    ChevronDown,
    /// A section that is closed.
    ChevronRight,
    /// The way back up.
    ChevronUp,
    /// A choice that is selected.
    CircleDot,
    /// A choice that is not selected.
    Circle,
    /// Close what the control is on.
    Close,
    /// Copy what the control is on.
    Copy,
    /// The pointer, for selecting.
    Cursor,
    /// Something shown.
    Eye,
    /// A file with a plus beside it.
    FileAdd,
    /// A file.
    File,
    /// A folder with a plus beside it.
    FolderAdd,
    /// A folder that is open.
    FolderOpen,
    /// A folder.
    Folder,
    /// A tree of parents and children.
    Hierarchy,
    /// A picture.
    Image,
    /// An informational notice.
    Info,
    /// Panels arranged in a window.
    Layout,
    /// Rows of items.
    List,
    /// Take one away.
    Minus,
    /// The remaining actions.
    More,
    /// Colours and materials.
    Palette,
    /// A pin that is in.
    PinFilled,
    /// Keep what the control is on.
    Pin,
    /// Add one.
    Plus,
    /// Read again.
    Refresh,
    /// Write to disk.
    Save,
    /// Look for something.
    Search,
    /// The preferences, as a gear.
    Settings,
    /// Light and the sky.
    Sun,
    /// Lines of text.
    Text,
    /// Put back as it was.
    Undo,
    /// Something to look at.
    Warning,
}

impl IconName {
    /// The artwork this icon is drawn from.
    pub fn svg(self) -> Svg {
        match self {
            Self::ArrowDown => Svg::new("arrow_down", include_icon!("arrow_down")),
            Self::ArrowLeft => Svg::new("arrow_left", include_icon!("arrow_left")),
            Self::ArrowRight => Svg::new("arrow_right", include_icon!("arrow_right")),
            Self::ArrowUp => Svg::new("arrow_up", include_icon!("arrow_up")),
            Self::Box => Svg::new("box", include_icon!("box")),
            Self::Check => Svg::new("check", include_icon!("check")),
            Self::ChevronDown => Svg::new("chevron_down", include_icon!("chevron_down")),
            Self::ChevronRight => Svg::new("chevron_right", include_icon!("chevron_right")),
            Self::ChevronUp => Svg::new("chevron_up", include_icon!("chevron_up")),
            Self::CircleDot => Svg::new("circle_dot", include_icon!("circle_dot")),
            Self::Circle => Svg::new("circle", include_icon!("circle")),
            Self::Close => Svg::new("close", include_icon!("close")),
            Self::Copy => Svg::new("copy", include_icon!("copy")),
            Self::Cursor => Svg::new("cursor", include_icon!("cursor")),
            Self::Eye => Svg::new("eye", include_icon!("eye")),
            Self::FileAdd => Svg::new("file_add", include_icon!("file_add")),
            Self::File => Svg::new("file", include_icon!("file")),
            Self::FolderAdd => Svg::new("folder_add", include_icon!("folder_add")),
            Self::FolderOpen => Svg::new("folder_open", include_icon!("folder_open")),
            Self::Folder => Svg::new("folder", include_icon!("folder")),
            Self::Hierarchy => Svg::new("hierarchy", include_icon!("hierarchy")),
            Self::Image => Svg::new("image", include_icon!("image")),
            Self::Info => Svg::new("info", include_icon!("info")),
            Self::Layout => Svg::new("layout", include_icon!("layout")),
            Self::List => Svg::new("list", include_icon!("list")),
            Self::Minus => Svg::new("minus", include_icon!("minus")),
            Self::More => Svg::new("more", include_icon!("more")),
            Self::Palette => Svg::new("palette", include_icon!("palette")),
            Self::PinFilled => Svg::new("pin_filled", include_icon!("pin_filled")),
            Self::Pin => Svg::new("pin", include_icon!("pin")),
            Self::Plus => Svg::new("plus", include_icon!("plus")),
            Self::Refresh => Svg::new("refresh", include_icon!("refresh")),
            Self::Save => Svg::new("save", include_icon!("save")),
            Self::Search => Svg::new("search", include_icon!("search")),
            Self::Settings => Svg::new("settings", include_icon!("settings")),
            Self::Sun => Svg::new("sun", include_icon!("sun")),
            Self::Text => Svg::new("text", include_icon!("text")),
            Self::Undo => Svg::new("undo", include_icon!("undo")),
            Self::Warning => Svg::new("warning", include_icon!("warning")),
        }
    }
}

/// How large an icon is drawn, in the steps the UI uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconSize {
    /// 12px: inside a tab or a row of text.
    ///
    /// The artwork is drawn on a grid of sixteen, so anything below
    /// [`IconSize::Medium`] rasterizes its strokes across two pixels rather
    /// than into one: a size for an icon that has to fit.
    XSmall,
    /// 14px: a smaller icon, where one at its own size would crowd its row.
    #[default]
    Small,
    /// 16px: the artwork's own size, which is the size it is sharpest at.
    Medium,
    /// 20px: an icon that stands alone, as on a rail.
    Large,
}

impl IconSize {
    /// The side of the square this size draws inside.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::XSmall => 12.0,
            Self::Small => 14.0,
            Self::Medium => 16.0,
            Self::Large => 20.0,
        }
    }
}

/// One icon, sized from the icon scale and coloured from the theme.
pub struct Icon {
    /// Which artwork to draw.
    name: IconName,
    /// How large to draw it.
    size: IconSize,
    /// The colour to tint it, or the theme's quietest text colour when unset.
    color: Option<Rgba>,
    /// How the box is sized.
    style: Style,
    /// Clockwise rotation around the icon's centre, in radians.
    rotation: f32,
}

/// An icon of `name` at the default size, in the theme's subtle text colour.
pub fn icon(name: IconName) -> Icon {
    Icon {
        name,
        size: IconSize::default(),
        color: None,
        style: Style::default(),
        rotation: 0.0,
    }
}

impl Icon {
    /// Returns this icon drawn at `size`.
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Returns this icon tinted `color`.
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Returns this icon rotated clockwise by `radians` around its centre.
    pub fn rotate(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// The square this icon occupies, centred in `bounds`.
    fn square(&self, bounds: Rect) -> Rect {
        let side = self.size.pixels();
        Rect::from_xywh(
            (bounds.left() + (bounds.size.width - side) / 2.0).round(),
            (bounds.top() + (bounds.size.height - side) / 2.0).round(),
            side,
            side,
        )
    }
}

impl Styled for Icon {
    /// How the box is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for Icon {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the square the artwork is drawn inside.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let side = self.size.pixels();
        Size::new(side, side)
    }

    /// Paints the artwork centred in `bounds`.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let color = self.color.unwrap_or(cx.theme().colors.text_subtle);
        let square = self.square(bounds);
        cx.rotated_icon(square, self.name.svg(), color, self.rotation);
    }
}
