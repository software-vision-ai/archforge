//! What a notation brings to the shell's explorers, search, problems and panels:
//! its rows in the Models tree and what `+` adds there, its renames and what a
//! removal takes with it, its entries in the project search, its problems, and the heading and the
//! body of Properties for its elements. The shells (the tree, the filter, the scroll, the search
//! list, the sections, the fields, the editors) stay shared; each answer is about the notation's
//! own elements, the classes of its metamodel.

use archforge_mof::{Obj, Store, Uuid};
use egui::Rect;

use crate::Shell;
use crate::explorer::{Row, Tree};
use crate::panels::Subject;
use crate::problems::Problem;
use crate::ux::{Icon, Palette};

/// Something `+` in the Models tree can add: its automation key and its word.
pub(crate) struct Offer {
    pub key: &'static str,
    pub word: String,
}

/// An element the tree added: its id and the rows it sits under, opened so it is seen.
pub(crate) struct Added {
    pub id: Uuid,
    pub parents: Vec<Uuid>,
}

/// An element as the project search lists it: its icon, its kind's place in the list (diagrams
/// 0, skills 6, notes 7), its name and what it is and where.
pub(crate) struct Entry {
    pub icon: &'static Icon,
    pub order: u8,
    pub label: String,
    pub detail: String,
}

/// How Properties names an element: the kind line under the heading, the word for the
/// selection, "This table", "On this table · {n}", and the heading's name.
pub(crate) struct Words {
    pub kind: String,
    pub word: String,
    pub this: String,
    pub on_this: String,
    pub name: String,
}

/// What a notation brings to the shell.
pub(crate) trait NotationUi: Sync {
    /// The index of the metamodel whose elements it answers for.
    fn metamodel(&self) -> u16;

    // The Models tree.

    /// Whether a member of the model is one of its top rows (ER's schemas).
    fn is_top(&self, store: &Store, member: Obj) -> bool;
    /// The rows of a top element: its own and, opened, those under it.
    fn tree(&self, store: &Store, top: Obj, t: &Tree<'_>) -> Vec<Row>;
    /// What `+` adds at `selection`, the first being the default; None is the model root.
    fn offers(&self, store: &Store, selection: Option<Obj>) -> Vec<Offer>;
    /// Adds what the offer `key` names at `selection` with a fresh default name; None when it has
    /// nothing to add there, a refusal in the user's words.
    fn add(&self, shell: &mut Shell, key: &str, selection: Option<Obj>) -> Option<Result<Added, String>>;
    /// The command that renames an element of `class` in place, and its key for the element.
    fn rename(&self, store: &Store, obj: Obj) -> Option<(&'static str, &'static str)>;
    /// What goes with `obj` when it is removed: relationships, keys and foreign keys.
    fn consequences(&self, store: &Store, obj: Obj) -> (usize, usize, usize) {
        let _ = (store, obj);
        (0, 0, 0)
    }
    /// The element that takes what `obj` holds when `obj` is removed (a nested package's
    /// parent), when it hands its contents up rather than taking them along.
    fn hands_up(&self, store: &Store, obj: Obj) -> Option<Obj> {
        let _ = (store, obj);
        None
    }
    /// Whether a drop of `dragged` on `target` is one the model refuses (a package into itself):
    /// its target is ringed in amber, not the accent.
    fn drop_refused(&self, store: &Store, dragged: Obj, target: Obj) -> bool {
        let _ = (store, dragged, target);
        false
    }
    /// The command, with its parameters, that a tree row `dragged` dropped on the row `target`
    /// runs (a class moved into a package); none when the drop means nothing there.
    fn drop_on(&self, store: &Store, dragged: Obj, target: Obj) -> Option<(&'static str, serde_json::Value)> {
        let _ = (store, dragged, target);
        None
    }
    /// Why Delete leaves `obj` in Models, in the user's words, when it does: by default Delete
    /// removes every element; one shown read-only (a UML literal, for now) stays, and Models says
    /// where it goes instead.
    fn kept(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// Why `obj`, which the model says carries no skill, carries none, in the user's words for the
    /// Skills panel ("Relationships carry no skills"); none takes the panel's general line.
    fn no_skills(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// Whether `obj` holds elements that go with it.
    fn has_children(&self, store: &Store, obj: Obj) -> bool {
        let _ = (store, obj);
        false
    }
    /// The rows of a top element that can open, for Expand all.
    fn foldable(&self, store: &Store, top: Obj) -> Vec<Obj>;
    /// The rows to open so the row of `obj` shows.
    fn parents(&self, store: &Store, obj: Obj) -> Vec<Obj>;

    // The project search.

    /// The search's entry for `obj`, when it lists it.
    fn entry(&self, store: &Store, obj: Obj) -> Option<Entry>;
    /// What the canvas brings into view for `obj` (a column's table).
    fn shown(&self, store: &Store, obj: Obj) -> Option<Obj> {
        let _ = store;
        Some(obj)
    }

    // Problems.

    /// Its problems in the store, each on the element the user would fix it on.
    fn problems(&self, store: &Store, out: &mut Vec<Problem>) {
        let _ = (store, out);
    }
    /// The element whose problems a selection of `obj` shows (a column's table).
    fn problem_holder(&self, store: &Store, obj: Obj) -> Obj {
        let _ = store;
        obj
    }

    // Properties.

    /// How Properties names `obj`.
    fn words(&self, store: &Store, obj: Obj) -> Option<Words>;
    /// The title of the first section, when it is not the selection's word (a column's panel is
    /// its table's).
    fn heading(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// Draws the body of Properties for `subject`, an element of its metamodel.
    #[allow(clippy::too_many_arguments, reason = "the body is drawn into the panel from its top, for its subject")]
    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject);
}

/// The notations the shell knows.
pub(crate) static UIS: &[&dyn NotationUi] = &[&crate::er_ui::ErUi, &crate::uml_ui::UmlUi];

/// The notation of `obj`, when the shell knows it.
pub(crate) fn of(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> Option<&'static dyn NotationUi> {
    let class = store.class_of(obj).ok()?;
    uis.iter().copied().find(|u| u.metamodel() == class.metamodel())
}
