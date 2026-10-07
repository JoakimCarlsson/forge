//! How much of a colour a wash, a thumb or a halo carries.
//!
//! A translucent overlay is read against whatever it covers, so how strong it
//! has to be to register is a property of the theme and not of the screen
//! drawing it.

/// The alphas the translucent parts of the window are drawn at.
#[derive(Clone, Copy, Debug)]
pub struct Emphasis {
    /// The wash over selected text.
    pub selection: f32,
    /// A scrollbar thumb at rest.
    pub scrollbar: f32,
    /// A scrollbar thumb under the pointer or being dragged.
    pub scrollbar_active: f32,
    /// How much of its colour a disabled control keeps.
    pub disabled: f32,
    /// How far a filled control is lifted towards the text colour on hover.
    pub hover_lift: f32,
}

impl Emphasis {
    /// The emphases every theme uses.
    pub const DEFAULT: Self = Self {
        selection: 0.3,
        scrollbar: 0.35,
        scrollbar_active: 0.6,
        disabled: 0.5,
        hover_lift: 0.16,
    };
}
