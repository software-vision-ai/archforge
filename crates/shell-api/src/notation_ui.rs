//! What a notation brings to the shell, and the data types the interfaces hand between them, which
//! name no shell. `NotationUi` answers for a notation's own elements in the shell's explorers,
//! search, problems and panels: its rows in the Models tree and what `+` adds there, how a row
//! dropped on another runs, its renames, what a removal takes with it and what is said of it, what
//! Delete and a chord of its own on the canvas do to its selected elements, what it draws over the
//! canvas whatever diagram shows and what a problem badge that offers a way out does, its entries in
//! the project search, its problems, the heading and the body of Properties for its elements and
//! which share a page, its commands' refusals in the user's words, the words for the kinds of its
//! elements and Chat's counts of them, and the limits of their names and descriptions; `NotationState` is what its own
//! editors, menus and drawings keep between frames, `NotationStates` the shell's one of each, and
//! the free functions ask the notations the shell registered (`of`, the limits, `label`, what a
//! removal says). The shells (the tree, the filter, the scroll, the search list, the sections, the
//! fields, the editors) stay shared; each answer is about the notation's own elements, the classes
//! of its metamodel.
//!
//! The data types: what `+` in Models adds, what an addition gives back, what New inside ▸ offers,
//! a row of the Models tree and what a notation's rows are made with (the filter, the rows opened,
//! the skills by subject), a project search entry, a problem, Properties' words and the selection
//! they describe, what Chat counts an element as, what is said of a removal, an overlay's layer and
//! the canvas it is drawn over, a Properties editor's card and where it edits, and a menu of the
//! shell's one open-menu slot (`archforge_ui_egui::notation_ui`, `explorer`, `problems` and
//! `panels` re-export them under their old paths).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};

use archforge_mof::{ClassId, Obj, Store, Uuid};
use egui::Rect;

use crate::canvas::View;
use crate::cx::{Ctx, CtxRef};
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

/// What a tool row of Chat counts one of a notation's elements as ([`NotationUi::chat_kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    /// Not counted: a part of another element, as a description's comment is.
    Part,
    /// A line, summed with the relationships of every notation: "2 lines".
    Line,
    /// A kind of its own, by its key: summed with the elements of the same key and said in its
    /// words ([`NotationUi::chat_count`]).
    Own(&'static str),
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

/// What a notation brings to the shell.
pub trait NotationUi: Sync {
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
    fn add(&self, cx: &mut Ctx, key: &str, selection: Option<Obj>) -> Option<Result<Added, String>>;
    /// The command that renames an element of `class` in place, and its key for the element.
    fn rename(&self, store: &Store, obj: Obj) -> Option<(&'static str, &'static str)>;
    /// Why `obj` cannot be renamed `name`, in the user's words, checked before the rename command
    /// runs (a UML literal's name another literal has); none lets the command decide.
    fn rename_refusal(&self, store: &Store, obj: Obj, name: &str) -> Option<String> {
        let _ = (store, obj, name);
        None
    }
    /// What goes with `obj` when it is removed: relationships, keys and foreign keys.
    fn consequences(&self, store: &Store, obj: Obj) -> (usize, usize, usize) {
        let _ = (store, obj);
        (0, 0, 0)
    }
    /// What the Models context menu's New inside ▸ offers on `obj`; None for no such row.
    fn inside(&self, store: &Store, obj: Obj) -> Option<Inside> {
        let _ = (store, obj);
        None
    }
    /// Makes what New inside ▸ `key` names inside `obj`, its name open for typing in Models.
    fn add_inside(&self, cx: &mut Ctx, key: &str, obj: Obj) -> Option<Result<Uuid, String>> {
        let _ = (cx, key, obj);
        None
    }
    /// What goes with `obj` when it is removed, by name, for the dialog to name each: a UML
    /// classifier's nested classifiers (AR-132).
    fn goes_along(&self, store: &Store, obj: Obj) -> Vec<String> {
        let _ = (store, obj);
        Vec::new()
    }
    /// The stereotype applications that go when `obj` is removed, by name, for the delete
    /// dialog (AR-135); none by default.
    fn applications_along(&self, store: &Store, obj: Obj) -> Vec<String> {
        let _ = (store, obj);
        Vec::new()
    }
    /// Why `obj` cannot be removed, in the user's words, checked before the removal or its dialog
    /// (UML's package Types while it keeps the built-in types); none lets the command decide.
    fn remove_refusal(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// The element that takes what `obj` holds when `obj` is removed (a nested package's
    /// parent), when it hands its contents up rather than taking them along.
    fn hands_up(&self, store: &Store, obj: Obj) -> Option<Obj> {
        let _ = (store, obj);
        None
    }
    /// What it says of the removal of `objs`, elements of its own, worked out before the removal
    /// runs ([`Removal`]); an empty one when it has nothing to say. Only the words: whether they may
    /// go and what goes with them are asked above.
    fn before_removal(&self, store: &Store, objs: &[Obj]) -> Removal;
    /// The toast once a line's element of its own was removed from the model, `what` being its
    /// kind's word (`Generalization`), with what [`before_removal`] worked out.
    ///
    /// [`before_removal`]: NotationUi::before_removal
    fn removed_text(&self, what: &str, removal: &Removal) -> String;
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
    /// Runs the command `cmd` with `params` that a tree row dropped on another gives ([`drop_on`]),
    /// as one undo step: by default as any command runs; a notation may run it its own way (UML
    /// moves a package as Properties' Parent package does, with a toast offering Undo). A refusal
    /// comes back in the user's words, for the tree to say.
    ///
    /// [`drop_on`]: NotationUi::drop_on
    fn run_drop(&self, cx: &mut Ctx, cmd: &'static str, params: serde_json::Value) -> Result<(), String> {
        crate::panels::run_cx(cx, cmd, params).map(|_| ())
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
    /// What a sentence calls `obj` when it has no name of its own (AR-216): a relationship by its
    /// ends as its line runs, `CardPayment → Payment`; none leaves its kind's word ([`label`]).
    fn unnamed(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// The title of the first section, when it is not the selection's word (a column's panel is
    /// its table's).
    fn heading(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// The element `obj` shares a page with, as the right panel decides whether to keep its scroll:
    /// selecting `obj` after that element, or that element after `obj`, in the same panel keeps it
    /// where it was scrolled (a column shares its table's, whose Properties it shows); by default
    /// its own, none when it has none. It changes only the scroll, never what a panel draws.
    fn page(&self, store: &Store, obj: Obj) -> Option<Obj> {
        let _ = store;
        Some(obj)
    }
    /// Draws the body of Properties for `subject`, an element of its metamodel.
    #[allow(clippy::too_many_arguments, reason = "the body is drawn into the panel from its top, for its subject")]
    fn body(&self, cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject);

    // Refusals: each is said by the one notation whose commands give it; one the engine, the
    // diagrams or both notations give is the shell's own ([`crate::panels::said`]).

    /// A typed refusal only its own commands give, in the user's words; none for any other reason
    /// or for a kind of element the catalogues don't name.
    fn said(&self, reason: &archforge_engine::Reason) -> Option<String> {
        let _ = reason;
        None
    }
    /// Why one of its own commands is not available now, in the user's words, by the reason its
    /// availability check gives; none for any other reason.
    fn disabled(&self, reason: &str) -> Option<String> {
        let _ = reason;
        None
    }
    /// Whether its rename command `cmd` may empty the name, which leaves the element unnamed (a
    /// relationship's); false for any other command.
    fn may_be_unnamed(&self, cmd: &str) -> bool {
        let _ = cmd;
        false
    }

    // Words and limits: each about its own elements; the shell keeps those of the core, the
    // diagrams and both notations.

    /// Its own catalogues, one per language in the order of `LANGUAGES`: the rows only its own
    /// screens name. The shell registers them with its own before its first lookup
    /// (the shell's `i18n::tables`), as the shared screens look some of them up too.
    fn catalogues(&self) -> &'static [crate::i18n::Rows];
    /// The classes of its metamodel it creates, the only ones of it that get a kind word of their
    /// own (the shell's `model_words::kind_label`); the rest of its classes say Element.
    fn created_classes(&self) -> &'static [ClassId];
    /// Its own word for `class` where the metamodel's name is not what the interface says (ER's
    /// Entity is a Table); none takes the metamodel's name.
    fn kind_word(&self, class: ClassId) -> Option<String> {
        let _ = class;
        None
    }
    /// A kind of element as a refusal names it, with its article where the language has one (`a
    /// table`), for a kind only its own commands name; none for any other kind.
    fn a_kind(&self, kind: &str) -> Option<String> {
        let _ = kind;
        None
    }
    /// The kind word of `obj` when File → Generate… can be about it, as the agent's resources read
    /// it (a schema, a table, a class); none for anything else.
    fn scope_word(&self, store: &Store, obj: Obj) -> Option<String> {
        let _ = (store, obj);
        None
    }
    /// What a tool row of Chat counts an element of its table `table` as (the generated table's
    /// name, as the row keeps it): a part of another element, a line, or a kind of its own; none
    /// for a table not its own and for one counted as an element.
    fn chat_kind(&self, table: &str) -> Option<ChatKind> {
        let _ = table;
        None
    }
    /// Chat's words for `n` elements of its own kind `kind`, a key its [`NotationUi::chat_kind`]
    /// gives, with the count in them; none for a key not its own.
    fn chat_count(&self, kind: &str, n: u64) -> Option<String> {
        let _ = (kind, n);
        None
    }
    /// The most characters its commands take in the name of one of its elements.
    fn name_limit(&self) -> usize;
    /// The most characters its elements take in a description.
    fn description_limit(&self) -> usize;

    // The canvas's keys on its own selected elements: asked of every notation, whatever diagram
    // the canvas shows, as the selection may hold elements it does not draw (an agent's, Models').

    /// Delete on the canvas, with none of the diagram's boxes selected and Shift held or not
    /// (`shift`): removes one of its own selected elements its own way and gives true; false
    /// leaves the key to the next notation and then to the shared removals.
    fn canvas_delete(&self, cx: &mut Ctx, shift: bool) -> bool;
    /// Reads a chord of its own on the selection, once the canvas has the keys and changes are
    /// allowed (UML's ⌥⌘G); true when it took the keys this frame, false by default.
    fn canvas_keys(&self, cx: &mut Ctx, ui: &egui::Ui) -> bool {
        let _ = (cx, ui);
        false
    }

    // What it draws over the canvas: asked of every notation, whatever diagram the canvas shows,
    // as what it has open may outlive the diagram it was opened on; each closes its own there.

    /// Draws what of its own goes over the canvas `on` at `layer`, from its own state: the drafts,
    /// rows, menus and offers it has open; nothing at a layer it has nothing for.
    fn canvas_overlay(&self, cx: &mut Ctx, ui: &mut egui::Ui, layer: Layer, on: &Overlaid<'_>);
    /// What a problem badge on one of its elements says on hover when the badge offers a way out
    /// of the problem, as the element's figure says it does (ER's many-to-many relationship: "Many
    /// to many · add a mapping table"); none leaves the badge a plain one, its problems on hover
    /// and no click.
    fn resolve_hint(&self) -> Option<String> {
        None
    }
    /// A click on such a badge on its element `element`: ER opens the mapping table offer for the
    /// relationship; nothing by default.
    fn resolve(&self, cx: &mut Ctx, element: Uuid) {
        let _ = (cx, element);
    }

    // As a project opens.

    /// Puts right, in `store`, a project's model read as it opens and before its session starts, a
    /// layout of its own elements that depends on what its screens draw: ER grows a table box an
    /// earlier version left too short for its columns. No undo step; it is written with the next
    /// change. Nothing by default, for a notation whose boxes show what they hold at any size.
    fn open_repair(&self, store: &mut Store) {
        let _ = store;
    }

    // Its own state.

    /// A fresh state for its own editors, menus and drawings, as a window or a project opened
    /// anew starts with: nothing open, its tools at their first choice.
    fn new_state(&self) -> Box<dyn NotationState>;
}

/// What a notation keeps between frames for its own editors, menus and drawings: the shell holds
/// it as it holds its own, parks it with the project it belongs to, and asks it only these
/// questions; the notation's own screens reach their state through [`NotationStates::own`].
pub trait NotationState: Any {
    /// Drops what the store no longer has: an editor whose element is gone, a drawing on a diagram
    /// that is no longer the one `canvas` shows.
    fn retain_valid(&mut self, store: &Store, canvas: &crate::canvas::State) {
        let _ = (store, canvas);
    }
    /// Whether something is being typed or drawn, or an editor card is open: the fields that only
    /// show what is stored are not counted.
    fn editing(&self) -> bool {
        false
    }
    /// Its Properties editors that are open, each by its card, with where it edits and the refusal
    /// its card shows (the shell's `panels::reveal_cx`).
    fn open_editors(&self) -> Vec<(Editor, OpenEditor)> {
        Vec::new()
    }
    /// Closes its Properties editors as the selection moves on, as Cancel would, except one opened
    /// this frame and not yet shown; one of its own menus open in the shell's one `menu` slot that
    /// belongs to what it closes goes with it, and another owner's menu stays.
    fn close_editors(&mut self, menu: &mut Option<MenuId>) {
        let _ = menu;
    }
    /// Whether something of its own open on the canvas holds the keys: a row typed in place or a
    /// menu.
    fn holds_keys(&self) -> bool {
        false
    }
    /// Whether `menu`, open in the shell's one menu slot, is one of its own that takes typing,
    /// Backspace, the arrows and Enter (UML's Stereotypes picker), so the canvas and the Models tree
    /// leave those keys to it; false for any other menu, another owner's included.
    fn menu_takes_keys(&self, menu: MenuId) -> bool {
        let _ = menu;
        false
    }
    /// Ends a drawing under way as the project it is on is set aside: it never outlives that
    /// project.
    fn set_aside(&mut self) {}
    /// Closes what of its own a press on the canvas closes, whatever diagram it shows (UML's menus
    /// of generalization lines and of an n-ary association); nothing by default.
    fn canvas_pressed(&mut self) {}
    /// Closes what of its own Escape on the canvas ends, whatever diagram it shows (ER's mapping
    /// table offer); nothing by default.
    fn canvas_escape(&mut self) {}
    /// The row typed in place on a card, as the canvas draws the card with it: the card's element,
    /// its compartment, and the row counted across the compartments (none for a new one); none
    /// while no row of its own is typed, and by default.
    fn card_draft(&self, store: &Store) -> Option<(Obj, usize, Option<usize>)> {
        let _ = store;
        None
    }
}

/// The state of each notation the shell knows, one each, in the order of the notations.
pub struct NotationStates(Vec<Box<dyn NotationState>>);

impl NotationStates {
    /// Each notation of `uis` with a fresh state ([`NotationUi::new_state`]).
    pub fn new(uis: &[&'static dyn NotationUi]) -> NotationStates {
        NotationStates(uis.iter().map(|ui| ui.new_state()).collect())
    }

    /// The state of type `T`, a notation's own; none while that notation is not among the shell's.
    pub fn own<T: NotationState>(&self) -> Option<&T> {
        self.0.iter().find_map(|s| <dyn Any>::downcast_ref::<T>(&**s))
    }

    /// The state of type `T` to change, a notation's own; none while that notation is not among
    /// the shell's.
    pub fn own_mut<T: NotationState>(&mut self) -> Option<&mut T> {
        self.0.iter_mut().find_map(|s| <dyn Any>::downcast_mut::<T>(&mut **s))
    }

    /// Drops what the store no longer has, in every notation's state.
    pub fn retain_valid(&mut self, store: &Store, canvas: &crate::canvas::State) {
        self.0.iter_mut().for_each(|s| s.retain_valid(store, canvas));
    }

    /// Whether any notation has something typed, drawn or an editor open.
    pub fn editing(&self) -> bool {
        self.0.iter().any(|s| s.editing())
    }

    /// Every notation's open Properties editors, in the order of the notations.
    pub fn open_editors(&self) -> Vec<(Editor, OpenEditor)> {
        self.0.iter().flat_map(|s| s.open_editors()).collect()
    }

    /// Closes every notation's Properties editors as the selection moves on.
    pub fn close_editors(&mut self, menu: &mut Option<MenuId>) {
        self.0.iter_mut().for_each(|s| s.close_editors(menu));
    }

    /// Whether something of any notation's open on the canvas holds the keys.
    pub fn holds_keys(&self) -> bool {
        self.0.iter().any(|s| s.holds_keys())
    }

    /// Whether the menu open in the shell's slot, if any, is a notation's that takes the keys
    /// ([`NotationState::menu_takes_keys`]), as its owner answers.
    pub fn menu_takes_keys(&self, menu: Option<MenuId>) -> bool {
        menu.is_some_and(|m| self.0.iter().any(|s| s.menu_takes_keys(m)))
    }

    /// Ends every notation's drawing under way, as the project is set aside.
    pub fn set_aside(&mut self) {
        self.0.iter_mut().for_each(|s| s.set_aside());
    }

    /// Closes what of each notation's own a press on the canvas closes.
    pub fn canvas_pressed(&mut self) {
        self.0.iter_mut().for_each(|s| s.canvas_pressed());
    }

    /// Closes what of each notation's own Escape on the canvas ends.
    pub fn canvas_escape(&mut self) {
        self.0.iter_mut().for_each(|s| s.canvas_escape());
    }

    /// The row typed in place on a card, as the first notation with one answers
    /// ([`NotationState::card_draft`]).
    pub fn card_draft(&self, store: &Store) -> Option<(Obj, usize, Option<usize>)> {
        self.0.iter().find_map(|s| s.card_draft(store))
    }
}

/// What the notations say of the removal of `objs`, each asked of its own elements alone
/// ([`NotationUi::before_removal`]), in the order of the notations; an element of none, such as a
/// diagram, says nothing.
pub fn before_removal(uis: &[&'static dyn NotationUi], store: &Store, objs: &[Obj]) -> Removal {
    let mut out = Removal::default();
    for ui in uis {
        let own: Vec<Obj> = objs.iter().copied().filter(|o| store.class_of(*o).is_ok_and(|c| c.metamodel() == ui.metamodel())).collect();
        if own.is_empty() {
            continue;
        }
        let said = ui.before_removal(store, &own);
        out.went.extend(said.went);
        out.after = match (out.after, said.after) {
            (Some(first), Some(next)) => Some(format!("{first} · {next}")),
            (first, next) => first.or(next),
        };
    }
    out
}

/// Says what a removal that went through changed beyond what it removed (`after`, from
/// [`Removal::after`]): after the toast it made while that toast still shows, or in a toast of
/// its own led by `lead` (`Payment deleted`), with Undo; nothing when it changed nothing else.
pub fn say_after_cx(cx: &mut Ctx, after: Option<String>, lead: &str) {
    let Some(after) = after else { return };
    let undo_label = cx.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
    let text = match cx.canvas.toast.take() {
        Some(t) if t.until > cx.frames => format!("{} · {after}", t.text),
        _ => format!("{lead} · {after}"),
    };
    cx.canvas.toast = Some(crate::canvas::Toast { text, until: cx.frames.saturating_add(400), undo_label });
}

/// The notation of `obj`, when the shell knows it.
pub fn of(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> Option<&'static dyn NotationUi> {
    of_class(uis, store.class_of(obj).ok()?)
}

/// The notation whose metamodel holds `class`, when the shell knows it.
pub fn of_class(uis: &[&'static dyn NotationUi], class: ClassId) -> Option<&'static dyn NotationUi> {
    uis.iter().copied().find(|u| u.metamodel() == class.metamodel())
}

/// The most characters a name field takes for `obj`, as its rename command does: a diagram's as
/// the diagrams' rules allow, so its field never takes a name `diagram.rename` refuses, an
/// element's as its notation says ([`NotationUi::name_limit`]); for anything else, the most any
/// name takes ([`longest_name`]).
pub fn name_limit(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> usize {
    if store.class_of(obj).ok() == Some(archforge_diagram::DIAGRAM) {
        return archforge_diagram::rules::MAX_NAME;
    }
    of(uis, store, obj).map_or_else(|| longest_name(uis), |ui| ui.name_limit())
}

/// The most characters a name field takes for the element or diagram `id` of the shell's model
/// ([`name_limit`]); for one not there, the most any name takes ([`longest_name`]).
pub fn name_limit_of_cx(cx: &CtxRef, id: Uuid) -> usize {
    let uis = cx.notation_uis;
    cx.session.as_ref().and_then(|s| s.doc().by_uuid(id).map(|o| name_limit(uis, s.doc(), o))).unwrap_or_else(|| longest_name(uis))
}

/// The most characters any name takes, a diagram's or an element's of any notation: the filter of
/// Models takes as much, so any name can be typed into it whole.
pub fn longest_name(uis: &[&'static dyn NotationUi]) -> usize {
    uis.iter().map(|ui| ui.name_limit()).fold(archforge_diagram::rules::MAX_NAME, usize::max)
}

/// The most characters the description of `obj` takes, as its notation says
/// ([`NotationUi::description_limit`]); for an element of none, the most any of them takes.
pub fn description_limit(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> usize {
    of(uis, store, obj).map_or_else(|| uis.iter().map(|ui| ui.description_limit()).max().unwrap_or_default(), |ui| ui.description_limit())
}

/// What a sentence such as "Delete {name} from the model?" calls `obj`, one rule for every path
/// that asks (AR-216): its name; without one, its notation's words for it
/// ([`NotationUi::unnamed`]); else its kind's word, as Properties says it.
pub fn label(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> String {
    let own = archforge_core::name_of(store, obj);
    if !own.is_empty() {
        return own;
    }
    let ui = of(uis, store, obj);
    ui.and_then(|u| u.unnamed(store, obj)).or_else(|| ui.and_then(|u| u.words(store, obj)).map(|w| w.word)).unwrap_or_default()
}

/// An unnamed relationship by its ends, as its line runs: `CardPayment → Payment`, as the
/// generalization set's members and the project search's relationships read.
pub fn ends_label(from: &str, to: &str) -> String {
    format!("{from} → {to}")
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
