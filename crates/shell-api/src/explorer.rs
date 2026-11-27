//! What the Models explorer's delete dialog asks about, as the narrow context asks for it
//! ([`crate::cx::Request::AskDelete`]).

use archforge_mof::Uuid;

/// A removal the dialog asks about: what goes, named, and what goes with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delete {
    pub ids: Vec<Uuid>,
    pub name: String,
    pub relationships: usize,
    pub keys: usize,
    pub foreign_keys: usize,
    /// The diagrams it disappears from: those that show it as a card or a line, each once
    /// (the shell's `editing::diagrams_showing`), not the diagrams that belong to it.
    pub diagrams: usize,
    /// The node the canvas asked from, when it did.
    pub node: Option<Uuid>,
    /// What goes with it by name, the dialog naming each: a UML classifier's nested classifiers
    /// (AR-132).
    pub along: Vec<String>,
    /// The stereotype applications that go with it and what it takes along, `«Entity» on Order`,
    /// the dialog naming each (AR-135).
    pub applications: Vec<String>,
    /// The package what it holds moves up to, a nested package's parent, and whether it holds
    /// anything: the dialog says so instead of saying what it holds goes with it, and the toast
    /// says where it went, as a removal without the dialog does.
    pub hands_to: Option<(String, bool)>,
}
