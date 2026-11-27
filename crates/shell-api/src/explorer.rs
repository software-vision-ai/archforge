//! What the Models explorer's delete dialog asks about, as the narrow context asks for it
//! ([`crate::cx::Request::AskDelete`]).

use archforge_mof::Uuid;

/// A removal the dialog asks about: what goes, named, and what goes with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delete {
    pub ids: Vec<Uuid>,
    pub name: String,
    /// What goes with it, as its notation counts and words it, one finished sentence each, in the
    /// order the dialog lists them ([`NotationUi::consequences`]).
    ///
    /// [`NotationUi::consequences`]: crate::notation_ui::NotationUi::consequences
    pub consequences: Vec<String>,
    /// The diagrams it disappears from: those that show it as a card or a line, each once
    /// (the shell's `editing::diagrams_showing`), not the diagrams that belong to it.
    pub diagrams: usize,
    /// The node the canvas asked from, when it did.
    pub node: Option<Uuid>,
    /// The name of the element what it holds moves up to (the parent of an element nested in
    /// another), and whether it holds anything: the dialog says so instead of saying what it holds
    /// goes with it, and the toast says where it went, as a removal without the dialog does.
    pub hands_to: Option<(String, bool)>,
}
