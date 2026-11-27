//! What the Appearance panel keeps between frames that the right panels' state holds
//! ([`crate::panels::State::picker`]): the colour picker open on a row, and the row's group.

use crate::canvas::hex;
use crate::ux::{self, Colour};

/// What a colour row sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Fill,
    Border,
    Text,
    Line,
}

impl Group {
    /// Its word in the automation ids of its row's swatches (`fill`, `border`, `text`, `line`).
    pub fn key(self) -> &'static str {
        match self {
            Group::Fill => "fill",
            Group::Border => "border",
            Group::Text => "text",
            Group::Line => "line",
        }
    }
}

/// The colour picker open on a row: the colour being chosen as hue, saturation and brightness
/// (so the hue stays while the colour is grey), and its code as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Picker {
    pub group: Group,
    pub hsv: [f32; 3],
    pub code: String,
}

impl Picker {
    /// The picker for `group`, open on `colour`: its hue, saturation and brightness, and its code.
    pub fn new(group: Group, colour: Colour) -> Self {
        Picker { group, hsv: ux::to_hsv(colour), code: hex(colour) }
    }

    /// The colour chosen so far.
    pub fn colour(&self) -> Colour {
        ux::from_hsv(self.hsv[0], self.hsv[1], self.hsv[2])
    }
}
