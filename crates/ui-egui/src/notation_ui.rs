//! What a notation brings to the shell's explorers, search, problems and panels: its rows in the
//! Models tree and what `+` adds there, how a row dropped on another runs, its renames, what a
//! removal takes with it and what is said of it, what Delete and a chord of its own on the canvas do to its selected
//! elements, what it draws over the canvas whatever diagram shows and what a problem badge that
//! offers a way out does, its entries in the project search, its problems, the heading and the body of Properties for its elements and which share a page,
//! its commands' refusals in the user's words, the words for the kinds of its elements, and the
//! limits of their names and descriptions, and the state its own editors, menus and drawings keep
//! between frames. The shells (the tree, the filter, the scroll, the search list, the sections, the
//! fields, the editors) stay shared; each answer is about the notation's own elements, the classes
//! of its metamodel.

use std::any::Any;

use archforge_mof::{ClassId, Obj, Store, Uuid};
use egui::Rect;

pub(crate) use archforge_shell_api::notation_ui::{Added, Entry, Inside, Layer, Offer, Overlaid, Removal, Words};

use crate::Shell;
use crate::explorer::{Row, Tree};
use crate::panels::{Editor, MenuId, OpenEditor, Subject};
use crate::problems::Problem;
use crate::ux::Palette;

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
    fn add_inside(&self, shell: &mut Shell, key: &str, obj: Obj) -> Option<Result<Uuid, String>> {
        let _ = (shell, key, obj);
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
    fn run_drop(&self, shell: &mut Shell, cmd: &'static str, params: serde_json::Value) -> Result<(), String> {
        crate::panels::run(shell, cmd, params).map(|_| ())
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
    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject);

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
    /// ([`crate::i18n::tables`]), as the shared screens look some of them up too.
    fn catalogues(&self) -> &'static [crate::i18n::Rows];
    /// The classes of its metamodel it creates, the only ones of it that get a kind word of their
    /// own ([`crate::model_words::kind_label`]); the rest of its classes say Element.
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
    /// The most characters its commands take in the name of one of its elements.
    fn name_limit(&self) -> usize;
    /// The most characters its elements take in a description.
    fn description_limit(&self) -> usize;

    // The canvas's keys on its own selected elements: asked of every notation, whatever diagram
    // the canvas shows, as the selection may hold elements it does not draw (an agent's, Models').

    /// Delete on the canvas, with none of the diagram's boxes selected and Shift held or not
    /// (`shift`): removes one of its own selected elements its own way and gives true; false
    /// leaves the key to the next notation and then to the shared removals.
    fn canvas_delete(&self, shell: &mut Shell, shift: bool) -> bool;
    /// Reads a chord of its own on the selection, once the canvas has the keys and changes are
    /// allowed (UML's ⌥⌘G); true when it took the keys this frame, false by default.
    fn canvas_keys(&self, shell: &mut Shell, ui: &egui::Ui) -> bool {
        let _ = (shell, ui);
        false
    }

    // What it draws over the canvas: asked of every notation, whatever diagram the canvas shows,
    // as what it has open may outlive the diagram it was opened on; each closes its own there.

    /// Draws what of its own goes over the canvas `on` at `layer`, from its own state: the drafts,
    /// rows, menus and offers it has open; nothing at a layer it has nothing for.
    fn canvas_overlay(&self, shell: &mut Shell, ui: &mut egui::Ui, layer: Layer, on: &Overlaid<'_>);
    /// What a problem badge on one of its elements says on hover when the badge offers a way out
    /// of the problem, as the element's figure says it does (ER's many-to-many relationship: "Many
    /// to many · add a mapping table"); none leaves the badge a plain one, its problems on hover
    /// and no click.
    fn resolve_hint(&self) -> Option<String> {
        None
    }
    /// A click on such a badge on its element `element`: ER opens the mapping table offer for the
    /// relationship; nothing by default.
    fn resolve(&self, shell: &mut Shell, element: Uuid) {
        let _ = (shell, element);
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
pub(crate) trait NotationState: Any {
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
    /// its card shows ([`crate::panels::reveal`]).
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
pub(crate) struct NotationStates(Vec<Box<dyn NotationState>>);

impl NotationStates {
    /// Each notation of `uis` with a fresh state ([`NotationUi::new_state`]).
    pub(crate) fn new(uis: &[&'static dyn NotationUi]) -> NotationStates {
        NotationStates(uis.iter().map(|ui| ui.new_state()).collect())
    }

    /// The state of type `T`, a notation's own; none while that notation is not among the shell's.
    pub(crate) fn own<T: NotationState>(&self) -> Option<&T> {
        self.0.iter().find_map(|s| <dyn Any>::downcast_ref::<T>(&**s))
    }

    /// The state of type `T` to change, a notation's own; none while that notation is not among
    /// the shell's.
    pub(crate) fn own_mut<T: NotationState>(&mut self) -> Option<&mut T> {
        self.0.iter_mut().find_map(|s| <dyn Any>::downcast_mut::<T>(&mut **s))
    }

    /// Drops what the store no longer has, in every notation's state.
    pub(crate) fn retain_valid(&mut self, store: &Store, canvas: &crate::canvas::State) {
        self.0.iter_mut().for_each(|s| s.retain_valid(store, canvas));
    }

    /// Whether any notation has something typed, drawn or an editor open.
    pub(crate) fn editing(&self) -> bool {
        self.0.iter().any(|s| s.editing())
    }

    /// Every notation's open Properties editors, in the order of the notations.
    pub(crate) fn open_editors(&self) -> Vec<(Editor, OpenEditor)> {
        self.0.iter().flat_map(|s| s.open_editors()).collect()
    }

    /// Closes every notation's Properties editors as the selection moves on.
    pub(crate) fn close_editors(&mut self, menu: &mut Option<MenuId>) {
        self.0.iter_mut().for_each(|s| s.close_editors(menu));
    }

    /// Whether something of any notation's open on the canvas holds the keys.
    pub(crate) fn holds_keys(&self) -> bool {
        self.0.iter().any(|s| s.holds_keys())
    }

    /// Whether the menu open in the shell's slot, if any, is a notation's that takes the keys
    /// ([`NotationState::menu_takes_keys`]), as its owner answers.
    pub(crate) fn menu_takes_keys(&self, menu: Option<MenuId>) -> bool {
        menu.is_some_and(|m| self.0.iter().any(|s| s.menu_takes_keys(m)))
    }

    /// Ends every notation's drawing under way, as the project is set aside.
    pub(crate) fn set_aside(&mut self) {
        self.0.iter_mut().for_each(|s| s.set_aside());
    }

    /// Closes what of each notation's own a press on the canvas closes.
    pub(crate) fn canvas_pressed(&mut self) {
        self.0.iter_mut().for_each(|s| s.canvas_pressed());
    }

    /// Closes what of each notation's own Escape on the canvas ends.
    pub(crate) fn canvas_escape(&mut self) {
        self.0.iter_mut().for_each(|s| s.canvas_escape());
    }

    /// The row typed in place on a card, as the first notation with one answers
    /// ([`NotationState::card_draft`]).
    pub(crate) fn card_draft(&self, store: &Store) -> Option<(Obj, usize, Option<usize>)> {
        self.0.iter().find_map(|s| s.card_draft(store))
    }
}

/// What the notations say of the removal of `objs`, each asked of its own elements alone
/// ([`NotationUi::before_removal`]), in the order of the notations; an element of none, such as a
/// diagram, says nothing.
pub(crate) fn before_removal(uis: &[&'static dyn NotationUi], store: &Store, objs: &[Obj]) -> Removal {
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
pub(crate) fn say_after(shell: &mut Shell, after: Option<String>, lead: &str) {
    let Some(after) = after else { return };
    let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
    let text = match shell.canvas.toast.take() {
        Some(t) if t.until > shell.frames => format!("{} · {after}", t.text),
        _ => format!("{lead} · {after}"),
    };
    shell.canvas.toast = Some(crate::canvas::Toast { text, until: shell.frames.saturating_add(400), undo_label });
}

/// Draws every notation's overlays at `layer` over the canvas `on`, in the order of the notations
/// ([`NotationUi::canvas_overlay`]).
pub(crate) fn overlays(shell: &mut Shell, ui: &mut egui::Ui, layer: Layer, on: &Overlaid<'_>) {
    let uis = shell.notation_uis;
    for notation in uis {
        notation.canvas_overlay(shell, ui, layer, on);
    }
}

/// The notations the shell knows.
pub(crate) static UIS: &[&dyn NotationUi] = &[&crate::er_ui::ErUi, &crate::uml_ui::UmlUi];

/// What an opening does to a project's model before its session starts: every notation of `UIS`
/// puts its own layout right (`NotationUi::open_repair`), in the order of the notations. The
/// shell opens, restores and compares models with it, and hands it to the host for a restore. It
/// reads `UIS` itself rather than a shell's `notation_uis`: the host and the comparisons' threads
/// take it as a plain function, and one opening must repair a model the same way wherever it runs.
pub fn open_repair(store: &mut Store) {
    for notation in UIS {
        notation.open_repair(store);
    }
}

/// The notation of `obj`, when the shell knows it.
pub(crate) fn of(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> Option<&'static dyn NotationUi> {
    of_class(uis, store.class_of(obj).ok()?)
}

/// The notation whose metamodel holds `class`, when the shell knows it.
pub(crate) fn of_class(uis: &[&'static dyn NotationUi], class: ClassId) -> Option<&'static dyn NotationUi> {
    uis.iter().copied().find(|u| u.metamodel() == class.metamodel())
}

/// The most characters a name field takes for `obj`, as its rename command does: a diagram's as
/// the diagrams' rules allow, so its field never takes a name `diagram.rename` refuses, an
/// element's as its notation says ([`NotationUi::name_limit`]); for anything else, the most any
/// name takes ([`longest_name`]).
pub(crate) fn name_limit(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> usize {
    if store.class_of(obj).ok() == Some(archforge_diagram::DIAGRAM) {
        return archforge_diagram::rules::MAX_NAME;
    }
    of(uis, store, obj).map_or_else(|| longest_name(uis), |ui| ui.name_limit())
}

/// The most characters a name field takes for the element or diagram `id` of the shell's model
/// ([`name_limit`]); for one not there, the most any name takes ([`longest_name`]).
pub(crate) fn name_limit_of(shell: &Shell, id: Uuid) -> usize {
    let uis = shell.notation_uis;
    shell.session.as_ref().and_then(|s| s.doc().by_uuid(id).map(|o| name_limit(uis, s.doc(), o))).unwrap_or_else(|| longest_name(uis))
}

/// The most characters any name takes, a diagram's or an element's of any notation: the filter of
/// Models takes as much, so any name can be typed into it whole.
pub(crate) fn longest_name(uis: &[&'static dyn NotationUi]) -> usize {
    uis.iter().map(|ui| ui.name_limit()).fold(archforge_diagram::rules::MAX_NAME, usize::max)
}

/// The most characters the description of `obj` takes, as its notation says
/// ([`NotationUi::description_limit`]); for an element of none, the most any of them takes.
pub(crate) fn description_limit(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> usize {
    of(uis, store, obj).map_or_else(|| uis.iter().map(|ui| ui.description_limit()).max().unwrap_or_default(), |ui| ui.description_limit())
}

/// What a sentence such as "Delete {name} from the model?" calls `obj`, one rule for every path
/// that asks (AR-216): its name; without one, its notation's words for it
/// ([`NotationUi::unnamed`]); else its kind's word, as Properties says it.
pub(crate) fn label(uis: &[&'static dyn NotationUi], store: &Store, obj: Obj) -> String {
    let own = archforge_core::name_of(store, obj);
    if !own.is_empty() {
        return own;
    }
    let ui = of(uis, store, obj);
    ui.and_then(|u| u.unnamed(store, obj)).or_else(|| ui.and_then(|u| u.words(store, obj)).map(|w| w.word)).unwrap_or_default()
}

/// An unnamed relationship by its ends, as its line runs: `CardPayment → Payment`, as the
/// generalization set's members and the project search's relationships read.
pub(crate) fn ends_label(from: &str, to: &str) -> String {
    format!("{from} → {to}")
}
