//! The data types the interfaces `NotationUi` and `NotationState` hand between the shell and a
//! notation, which name no shell: what `+` in Models adds, what an addition gives back, what New
//! inside ▸ offers, a row of the Models tree and what a notation's rows are made with (the filter,
//! the rows opened, the skills by subject), a project search entry, a problem, Properties' words
//! and the selection they describe, what is said of a removal, an overlay's layer and the canvas it is drawn over, a Properties
//! editor's card and where it edits, and a menu of the shell's one open-menu slot. The traits
//! themselves stay in the shell until they no longer hand a notation the whole `Shell`
//! (`archforge_ui_egui::notation_ui`, `explorer`, `problems` and `panels` re-export these under
//! their old paths).

use std::collections::{BTreeMap, BTreeSet};

use archforge_mof::{Obj, Store, Uuid};
use egui::Rect;

use crate::canvas::View;
use crate::i18n::tr;
use crate::search::Typed;
use crate::ux::{self, Icon, Palette};

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

/// The selection the panels describe.
#[derive(Debug, Clone, PartialEq)]
pub struct Subject {
    pub obj: Option<Obj>,
    pub id: Option<Uuid>,
    pub class: Option<archforge_mof::ClassId>,
    pub name: String,
    /// The kind line under the heading and the word for the selection.
    pub kind: String,
    pub word: String,
    /// "This table": the selection itself, as the applies-to choice and the own skill rows say it.
    pub this: String,
    /// "On this table · {n}": the title of the own skills section.
    pub on_this: String,
}

impl Subject {
    /// The subject `o` of `class`, as its words name it.
    pub fn of(o: Obj, id: impl Into<Option<Uuid>>, class: impl Into<Option<archforge_mof::ClassId>>, words: Words) -> Subject {
        let Words { kind, word, this, on_this, name } = words;
        Subject { obj: Some(o), id: id.into(), class: class.into(), name, kind, word, this, on_this }
    }
    /// The model root, which the project's name names.
    pub fn root(project: &str) -> Subject {
        Subject {
            obj: None,
            id: None,
            class: None,
            name: project.to_string(),
            kind: tr("Model · the project's root"),
            word: tr("Model"),
            this: tr("This model"),
            on_this: tr("On this model · {n}"),
        }
    }
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

/// One row of the tree as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: Uuid,
    pub obj: Obj,
    pub depth: u8,
    /// What the row is, as its notation names it (`schema`, `table`, `column`, `skill`), and its
    /// icon, drawn muted for a row under another's (a column).
    pub kind: &'static str,
    pub icon: &'static ux::Icon,
    pub muted: bool,
    pub label: String,
    /// A column's type, after its name.
    pub detail: Option<String>,
    /// What the row is, muted after its name (a UML generalization set's `generalization set`).
    pub note: Option<String>,
    pub expandable: bool,
    pub expanded: bool,
    /// Whether the row drags onto the canvas (a table).
    pub draggable: bool,
}

/// What a notation's rows are made with: the filter, the rows opened and the skills by subject.
pub struct Tree<'a> {
    store: &'a Store,
    filter: Typed,
    expanded: &'a BTreeSet<Uuid>,
    skills: BTreeMap<Obj, Vec<Obj>>,
    none: Vec<Obj>,
}

impl<'a> Tree<'a> {
    /// The rows' makings for `store`: the Models filter as typed, trimmed ([`Typed::trimmed`]),
    /// the rows opened, and the skills by the element they sit on, read once.
    pub fn new(store: &'a Store, filter: &str, expanded: &'a BTreeSet<Uuid>) -> Tree<'a> {
        Tree { store, filter: Typed::trimmed(filter), expanded, skills: skills_by_subject(store), none: Vec::new() }
    }
}

impl Tree<'_> {
    /// Whether a filter narrows the rows: then every row with a match under it is open.
    pub fn filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Whether a name holds the filter, whatever its capitals and accents.
    pub fn matches(&self, label: &str) -> bool {
        self.filter.in_any(&[label])
    }

    /// Whether the row of `obj` is open.
    pub fn open(&self, obj: Obj) -> bool {
        self.filtering() || self.store.uuid(obj).ok().is_some_and(|id| self.expanded.contains(&id))
    }

    /// The skills that sit on `obj`.
    pub fn skills(&self, obj: Obj) -> &[Obj] {
        self.skills.get(&obj).unwrap_or(&self.none)
    }

    /// A row of `obj`, closed, without a detail.
    pub fn row(&self, obj: Obj, depth: u8, kind: &'static str, icon: &'static ux::Icon, label: String) -> Option<Row> {
        let id = self.store.uuid(obj).ok()?;
        Some(Row { id, obj, depth, kind, icon, muted: false, label, detail: None, note: None, expandable: false, expanded: false, draggable: false })
    }

    /// The rows of the skills on `obj` whose names hold the filter.
    pub fn skill_rows(&self, obj: Obj, depth: u8) -> Vec<Row> {
        self.skills(obj)
            .iter()
            .filter_map(|s| {
                let label = archforge_core::skills::name_of(self.store, *s);
                if self.matches(&label) { self.row(*s, depth, "skill", &ux::icons::SKILL, label) } else { None }
            })
            .collect()
    }
}

/// The skills by the element they sit on, read once per frame.
fn skills_by_subject(store: &Store) -> BTreeMap<Obj, Vec<Obj>> {
    let mut out: BTreeMap<Obj, Vec<Obj>> = BTreeMap::new();
    for s in archforge_core::skills::skills(store) {
        if let Some(subject) = archforge_core::skills::subject_of(store, s) {
            out.entry(subject).or_default().push(s);
        }
    }
    out
}

/// One problem, on the element it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub element: Obj,
    pub text: String,
}

/// A Properties editor's card, which `reveal` brings into view: several can be open at once, and
/// each card answers only for its own editor. It is named by the metamodel of the notation whose
/// editor it is and the card's own name there, so two notations' cards never meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Editor {
    pub notation: u16,
    pub card: &'static str,
}

/// Where an open editor edits: the item, or none for one being added, and the list it belongs to (a
/// kind of member, type, object or key, or an association's end), so a move to either is new.
pub type Place = (Option<Uuid>, usize);

/// An open editor: where it edits, and the refusal its card shows.
pub type OpenEditor = Option<(Place, Option<String>)>;

/// Whose a menu is: the shell's own (Home's, the Versions panel's), or a notation's, by the index
/// of its metamodel, so two owners' menus never meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Shell,
    Notation(u16),
}

/// A menu of the shell's one open-menu slot (`State::menu`): whose it is, which of its owner's
/// menus by key, and which one of a kind shown more than once (a key pair's, an association end's;
/// 0 for a menu shown once). Only its owner opens, recognises and closes it: the shell keeps its
/// own beside the screens they belong to, each notation its own beside its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuId {
    pub scope: Scope,
    pub key: &'static str,
    pub index: usize,
}

impl MenuId {
    /// The shell's own menu `key`, the first of its kind.
    pub const fn shell(key: &'static str) -> MenuId {
        MenuId { scope: Scope::Shell, key, index: 0 }
    }

    /// The menu `key` of the notation whose metamodel is `notation`, the first of its kind.
    pub const fn of(notation: u16, key: &'static str) -> MenuId {
        MenuId { scope: Scope::Notation(notation), key, index: 0 }
    }

    /// The menu of the same owner and key, number `index` of its kind.
    pub const fn at(self, index: usize) -> MenuId {
        MenuId { index, ..self }
    }

    /// Which one of `kind` it is, when it is one of them: the same owner's menu of the same key,
    /// whatever its index; none for any other menu.
    pub fn index_in(self, kind: MenuId) -> Option<usize> {
        (self.scope == kind.scope && self.key == kind.key).then_some(self.index)
    }
}

#[cfg(test)]
mod tests {
    use archforge_core::{FOLDER, MODEL, SKILL, model, named_element, skill};
    use archforge_mof::{CompositeSlabs, Value};

    use super::*;

    /// A store of the core alone: the model, two folders and three skills, two of them on the
    /// first folder and one on the whole model.
    fn store() -> (Store, [Obj; 2], [Obj; 3]) {
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            Uuid::v7(1_761_670_800_000, r)
        });
        let slabs = CompositeSlabs::new().with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default())).unwrap();
        let mut st = Store::new(archforge_core::universe_with(Vec::new()).unwrap(), Box::new(slabs), ids);
        let m = st.create(MODEL).unwrap();
        let named = |st: &mut Store, class, name: &str, list| {
            let o = st.create(class).unwrap();
            st.set(o, named_element::NAME, Value::str(name)).unwrap();
            st.push(m, list, Value::Ref(o)).unwrap();
            o
        };
        let folders = [named(&mut st, FOLDER, "Sales", model::MEMBER), named(&mut st, FOLDER, "Billing", model::MEMBER)];
        let skills = [
            named(&mut st, SKILL, "Review", model::SKILL),
            named(&mut st, SKILL, "Audit", model::SKILL),
            named(&mut st, SKILL, "Style", model::SKILL),
        ];
        for s in &skills[..2] {
            st.set(*s, skill::SUBJECT, Value::Ref(folders[0])).unwrap();
        }
        (st, folders, skills)
    }

    /// The tree's makings: the skills by the element they sit on, the rows opened, and the filter
    /// as typed into Models, folded and trimmed as one piece, so the spaces around it go and a
    /// space inside it stays.
    #[test]
    fn a_tree_is_made_with_the_skills_by_subject_the_rows_opened_and_the_filter_trimmed() {
        let (st, [sales, billing], [review, audit, _style]) = store();
        let none = BTreeSet::new();
        let t = Tree::new(&st, "", &none);
        assert!(!t.filtering());
        assert_eq!((t.skills(sales), t.skills(billing)), (&[review, audit][..], &[][..]));
        assert!(!t.open(sales) && !t.open(billing));
        let labels = |t: &Tree<'_>| t.skill_rows(sales, 2).into_iter().map(|r| (r.label, r.kind, r.depth)).collect::<Vec<_>>();
        assert_eq!(labels(&t), [("Review".to_string(), "skill", 2), ("Audit".to_string(), "skill", 2)]);

        let opened = BTreeSet::from([st.uuid(sales).unwrap()]);
        let t = Tree::new(&st, "", &opened);
        assert!(t.open(sales) && !t.open(billing));

        let t = Tree::new(&st, " \u{200B} REV ", &none);
        assert!(t.filtering() && t.open(billing), "a filter opens every row");
        assert_eq!(labels(&t), [("Review".to_string(), "skill", 2)]);
        assert!(!Tree::new(&st, "\u{200B} ", &none).filtering(), "spaces alone are no filter");
        assert!(labels(&Tree::new(&st, "rev iew", &none)).is_empty(), "one piece, not each word apart");
    }
}
