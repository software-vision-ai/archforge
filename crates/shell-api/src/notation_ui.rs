//! The data types the interface `NotationUi` hands between the shell and a notation, which name no
//! shell: what `+` in Models adds, what an addition gives back, what New inside ▸ offers, a project
//! search entry, Properties' words, what is said of a removal, and an overlay's layer and the
//! canvas it is drawn over. The trait itself stays in the shell until it no longer hands a
//! notation the whole `Shell` (`archforge_ui_egui::notation_ui` re-exports these under their old
//! paths).

use std::collections::BTreeMap;

use archforge_mof::{Obj, Uuid};
use egui::Rect;

use crate::canvas::View;
use crate::ux::{Icon, Palette};

/// Something `+` in the Models tree can add: its automation key and its word.
pub struct Offer {
    pub key: &'static str,
    pub word: String,
}

/// An element the tree added: its id and the rows it sits under, opened so it is seen.
pub struct Added {
    pub id: Uuid,
    pub parents: Vec<Uuid>,
}

/// What the Models context menu's New inside ▸ offers on an element (AR-132): its row's word, the
/// submenu's heading, the kinds it makes (key, word, icon), the rows shown disabled with their
/// reason (key, word, reason), and why the row is disabled when the element owns none.
pub struct Inside {
    pub word: String,
    pub heading: String,
    pub kinds: Vec<(&'static str, String, &'static Icon)>,
    pub refused: Vec<(&'static str, String, String)>,
    pub why_not: Option<String>,
    /// Where a line parts the kinds, when not before the disabled rows: after a package's
    /// Package and Profile (AR-134, F2).
    pub split: Option<usize>,
}

/// An element as the project search lists it: its icon, its kind's place in the list (diagrams
/// 0, skills 6, notes 7), its name and what it is and where.
pub struct Entry {
    pub icon: &'static Icon,
    pub order: u8,
    pub label: String,
    pub detail: String,
}

/// How Properties names an element: the kind line under the heading, the word for the
/// selection, "This table", "On this table · {n}", and the heading's name.
pub struct Words {
    pub kind: String,
    pub word: String,
    pub this: String,
    pub on_this: String,
    pub name: String,
}

/// What a notation works out about a removal of its own elements before it runs, while they are
/// still in the store, for what is said once it went through (`NotationUi::before_removal`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Removal {
    /// What goes with the elements that the toast of a line's removal names after the element's
    /// own words (`NotationUi::removed_text`): UML's generalization sets a line alone made up.
    pub went: Vec<String>,
    /// What it says after the toast of any removal, of what else the removal changes: UML's
    /// constraints that name what goes (`say_after`); none when it changes nothing else.
    pub after: Option<String>,
}

/// Where among the canvas's own overlays a notation's go (`NotationUi::canvas_overlay`), each
/// layer drawn at its one place in the canvas's order, every notation asked in turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Over the lines being drawn and a line's end being moved, under the marquee and the
    /// adorners: a line not in the model yet (UML's template binding's draft).
    Drafts,
    /// Over the name typed in a node, under a note's text being typed: a row typed in place on a
    /// card (UML's member row).
    Rows,
    /// Over the element menu, before the canvas reads its keys: its own menus and what goes with
    /// them (UML's menu of generalization lines, then an n-ary association's draft, its legs' end
    /// handles and its menu).
    Menus,
    /// Over what the diagram's kind draws over the canvas and the fade of a switch, under the
    /// toast: a card offered over the canvas (ER's mapping table offer).
    Offers,
}

/// The canvas an overlay is drawn over: the palette, the canvas's rectangle, its view, the diagram
/// it shows and where a gesture under way previews nodes, on screen.
pub struct Overlaid<'a> {
    pub p: &'a Palette,
    pub r: Rect,
    pub view: View,
    pub d: Obj,
    pub overrides: &'a BTreeMap<Uuid, Rect>,
}
