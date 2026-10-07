//! How much of a colour a wash, a thumb or a halo carries.
//!
//! A translucent overlay is read against whatever it covers, so how strong it
//! has to be to register is a property of the theme and not of the screen
//! drawing it.

/// The alphas the translucent parts of the window are drawn at.
#[derive(Clone, Copy, Debug)]
pub struct Emphasis {
    /// How far a filled control is lifted towards the text colour on hover.
    pub hover_lift: f32,
}

impl Emphasis {
    /// The emphases every theme uses.
    pub const DEFAULT: Self = Self { hover_lift: 0.16 };
}
