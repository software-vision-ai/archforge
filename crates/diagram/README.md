# archforge-diagram

**Purpose.** The diagram metamodel for every notation (decision): a `Diagram` of some
element shows model elements as `Node`s and relationships as `Edge`s, with `Label`s that name a text
slot of their element and `Style`s over the shipped style sheet; our own, in the spirit of OMG DI,
with UMLDI export a later task. Generated from `diagram.xmi` by `cargo xtask generate diagram`;
the rules, the generic commands, the cascades and the spatial index are written here.

**Layer.** 6, above the shared core (5), whose Model holds the diagrams, `archforge-mof` (4) and `archforge-engine` (3); pure,
builds for the portable target. Knows no notation: everything a view shows is typed Any, and a
view brings the policy through `DiagramView`. The project's view, `ProjectView`, answers it by
asking the notation that owns a diagram's kind or an element's class (`Notation`). The notations
(7) depend on it.

**Binding rules.** A node or a label carries no model text; it draws its element's, so a rename
shows everywhere at once. Coordinates are doubles in one logical unit, absolute in the diagram's
space (a child's too); the grid and the zoom live in the view. The MOF core enforces types, bounds,
containment and the derived union `ownedElement`; the rules here (`rules`) add: coordinates finite
and within ±1,000,000; widths and heights between 20 and 10,000; kinds, slot names and sheet style
names plain identifiers of at most 64 characters; a diagram's name at most 128 characters, a note's
text at most 4,096; colours `#rrggbb`; a line width between 0 and 100 (0: the sheet's); nodes nest
at most 16 deep; at most 256 waypoints and 16 labels on an edge; an edge's ends are nodes of its
own diagram; a node shows an element or carries text, never both. The commands add, with the
notation's policy: a diagram's kind is one the notation knows and its element one the kind is of; a
node's element is one the kind shows, once per diagram; an edge's element is one the kind connects,
once per diagram, and its ends show the relationship's ends in order (`DiagramView::ends`); an edge
without an element is an anchor and has a note at one end; a label's slot is one the element has.
A notation may require an element (`needs_element`) and keep a diagram to its scope (`may_show`,
such as a schema's own entities). Removing a model element removes, for it and everything it
contains, the default-diagram bindings they own (with no replacement, AR-577), the diagrams of
them, the edges that show them and the nodes that show them with the edges on those nodes
(`remove_views_of`); a notation calls it before it destroys an element. A
diagram goes with the skills on it, whether `diagram.remove` or its element's removal takes it.
A `DefaultDiagramBinding` names a package's default diagram (AR-552): a member of the core's Model,
whose `ownerElement` (typed Any) and `defaultDiagram` are both required and own nothing. Destroying
it leaves both alive; while it stands, the store refuses to destroy either. Its table,
`diagram_default_diagram_binding`, is a storage upgrade only: a project saved before it gets the
table through the project crate's `EARLIER_MODELS`, and no diagram kind is migrated.
`defaults::{bindings_of, bindings_to, bindings}` find the bindings naming an element as their owner,
naming a diagram as its default, and all of them, without judging them; UML's resolver says what a
binding may name (AR-575). `remove_views_of` takes the bindings it finds along (AR-577).
`diagram.remove` of a diagram a binding names is refused before anything changes, typed as
`Reason::DefaultDiagramOf` with the binding's owner, and with `BindingFault::WrongOwner` when that
owner is not the diagram's element (`defaults::diagram_removal_problem`), unless it names a
`replacement_diagram` on the `expected_revision` read (AR-640): another live diagram of the same
element and kind, to which one step retargets the binding, keeping its id, before the diagram goes;
an element with several bindings keeps the first by id and loses the others in that step, as UML's
`uml.package.default.set` mends them. The diagram itself is refused as `ReplacementIsRemoved`,
another element's diagram or another kind as `NotReplacement`, a replacement for a diagram that is no default as `ReplacementNotNeeded`, and a
revision absent with a replacement, or stale whenever given, as `StaleRevision`; a `WrongOwner`
binding is refused still. It answers `{removed, diagram, replacement, binding, bindings_removed,
created}`: `removed` the count of objects that went, `binding` the one kept, `bindings_removed` the
ids of the others, and `created` false, as no diagram is made (AR-397 adds one made in the same
step); the replacement and binding are null and no binding goes for a plain removal; a notation asks
`defaults::views_removal_problem` before it removes an element whose diagram a binding of an element
that stays names, judged against everything the removal takes, and `defaults::remove_bindings_of`
takes the bindings of all of it before any diagram goes, so the order of selection does not matter
(AR-578). A notation whose `Notation::binds_default` says so for a diagram's kind and its element's
class (UML: a package's class diagram) gets the element's first diagram of that kind bound as its
default as it is made: `defaults::new_diagram_bound`, which `diagram.add` calls, makes the diagram
and a binding beside it in its home as one change, but only while the element has no diagram of
that kind and no binding (`defaults::is_first`); an element with diagrams but no binding stays
without one, its default diagnosed as missing, never bound silently (AR-579). UML's
`uml.package.default.set` makes and retargets a binding (AR-580); Properties and the window's way to
replace a default as it is removed come later (AR-396, AR-397). A comparison lists a binding among the model's elements, not
the diagrams, worded by its package and diagram (`Sales → Sales overview`) under the kind word
Default diagram (AR-553).

**The metaclasses.** `Diagram` (kind, name, element, node, edge, style), `Node` (element, kind,
text, x, y, width, height, child, style), `Edge` (element, source, target, waypoint, label, style),
`Point` (x, y), `Label` (kind, at, dx, dy), `Style` (name, fill, stroke, text, line, routing,
avoid), `DefaultDiagramBinding` (ownerElement, defaultDiagram) and the enumerations `LabelPlace` (source, middle, target) and `Routing` (asDiagram,
straight, rightAngled, curved); `Element` abstract above them. A diagram's style is the look its
lines take unless their own says otherwise; a new diagram's lines run at right angles around the
tables.

**Public API.**

| Item | Signature |
|---|---|
| `DiagramView` | the view's policy, asked of the view: the open diagram and selection, `kinds`, `home`, `is_of`, `needs_element`, `binds_default`, `shows`, `may_show`, `connects`, `ends`, `text_slots`, `default_size` |
| `Notation`, `ProjectView` | what a notation answers for its own kinds and classes (the diagram policy, where skills sit, `remove`, `describe`); the view of a project holding several notations, which asks the right one (`of_kind`, `of_class`, `of_element`) and implements `DiagramView` and `SkillHost`: a diagram carries skills too and inherits those of the element it shows |
| `figure::{Card, Row, Line, End, EndMark, Text, Tone, CARD_HEADER, CARD_FIRST_ROW, CARD_ROW, COMPARTMENT_GAP, CARD_FOOT}` | what a notation draws, without a toolkit: a node's card (eyebrow, name, compartments of rows, `Card::height` the height that shows them all by the card layout the canvas draws by, its tone: tinted, plain, tabbed, a constraint note, braces or an n-ary association's diamond, and a template's signature rows, drawn in a dashed box over its top-right corner), an edge's line (end marks and texts, roles, dashes, status, a note); `Notation::card`, `line` and `slot_text` give them and the canvas draws them alike; `Notation::links` gives the nodes a node is tied to by dashed links worked out from the model, never stored (a constraint note's); `Notation::nested_in` the element a node's element is nested in, whose node the canvas ties it to by a line ending in ⊕, never stored (a nested classifier's owner) |
| `project::specs`, `project::{ELEMENT, NOTATION_ELEMENT}` | the shared commands: `view.select`, `element.remove` (each notation removes its own) and `element.describe`; the kinds their typed refusals name |
| `commands::specs` | `pub fn specs<V: DiagramView>() -> Vec<CommandSpec<Store, V>>`: the diagram commands for a notation's registry |
| `metamodel`, `universe_with`, `with_slabs` | the metamodel; a universe of it and others; the slabs joined to a `CompositeSlabs` |
| `remove_views_of`, `destroy_node_with_edges`, `views_of`, `edges_on`, `node_tree`, `diagram_of` | the cascades and the walks they rest on |
| `Grid`, `Hit`, `Rect`, `grid::Visible`, `grid::drawing_order` | `Grid::build(store, diagram)`, `Grid::build_with(store, diagram, behind)` (the top-level nodes `behind` takes drawn and hit under the others, `grid::drawing_order`), `hit(x, y)`, `sort_drawn(nodes)` (nodes put in the drawing order), `hit_keeping(x, y, keep)` (looking through the nodes `keep` leaves out), `visible(rect)`: the spatial index of one diagram, its lines routed as drawn |
| `arrange::arrange` | `arrange(sizes, lines) -> Vec<(f64, f64)>`: boxes in layers along their lines, crossings few, groups side by side, lone boxes in rows |
| `route`, `Way`, `route::{way_of, spreads}` | `route(way, from, to, at, via, boxes) -> Vec<(f64, f64)>`: a line straight, at right angles, around the other boxes, or curved; `way_of` reads an edge's routing (its own, else its diagram's); `spreads(store, diagram, rect)` places every line's ends along its boxes' sides, once per diagram |
| `rules::{problems, kind_problem, text_problem, colour_problem, coord_problem, size_problem, depth_of}` | the rules, for commands and for a whole store; `text_problem` and `kind_problem` typed as a `Problem` |
| the generated constants, structs, `LabelPlace`, `TABLES`, `JUNCTIONS` | as every generated metamodel |

Commands: `diagram.{add, rename, remove, place, note, set-text, move, resize, unplace, connect,
connect-all, disconnect, route, label, unlabel, style, arrange}`, `view.open-diagram`, and the queries
`diagram.{list, get}`. `diagram.move` takes the nodes with what they contain and the waypoints of
the edges between them, so a multi-node drag is one step (the engine's `coalesce` key joins the
steps of one drag); `diagram.style` sets a style as a whole and a call naming nothing removes it;
`diagram.connect-all` adds an edge for every relationship whose both ends the diagram shows and
which it does not show yet. A refusal of an id, an element of the wrong kind, a place, a size, a
name or a note's text, an element placed where it is not shown or shown already, a box on another
diagram or nested too deep, a note's link routed or a line bent too often, and a notation's elements
it does not remove carries its reason typed for an interface to say (AR-439); so does a kind word
that is no plain identifier or none the notations or a line's element take: a diagram's kind, a
box's kind, a label's slot and a style's name (AR-512); and a new diagram's element missing where
its kind needs one or of a class its kind cannot be of (AR-513).
The lists of `diagram.move`, `diagram.unplace` and `diagram.style` carry their reasons typed too: an
empty list, an item that is no string, no id, of no element or of the wrong kind, an item given
twice, and boxes of different diagrams moved together (AR-514).
The rules of `diagram.connect`, `diagram.link-note` and `diagram.label` carry their reasons typed as
well (AR-515), as the engine's `Reason::{NoRelationship, LacksEnd, NotAtEnd, NotLinkable,
TargetOtherDiagram, LinkUnlabelled, Labelled}` and reasons it had: a line between two elements that
names no relationship, one the diagram's kind does not connect (`NotShown`), one that lacks an end,
a box at either end that does not show that end, and one shown already (`ShownAlready`); a note's
link from a box that shows an element (`NotA` a `commands::NOTE`), to itself (`Itself`) or twice
(`Again`), both of the relationship `commands::NOTE_LINK`, to what no note links to, and onto another
diagram; a label on a note's link, and one of a text a label of the line shows already. A line's
labels are of distinct texts, never as many as `rules::MAX_LABELS`, and the engine refuses a place
other than `source`, `middle` or `target` before the command runs, so those two refusals stay
untyped.
The colours of `diagram.style` and the rules of `diagram.arrange` carry their reasons typed too
(AR-516), as the engine's `Reason::{NotAColour, ArrangedTooFar}` and `TooFew`: a fill, stroke or
text colour not of the form `#rrggbb` (`NotAColour` with its part), a diagram that shows no box
(`TooFew` of `commands::ARRANGED`), and boxes that, arranged, would land farther from the origin
than `rules::MAX_COORD`. The engine refuses a line width outside 0 to `rules::MAX_LINE` and a
routing none of the four before the command runs, so those two refusals stay untyped.
The shared commands carry theirs typed too (AR-517), of the kind `project::ELEMENT`: the lists of
`view.select` and `element.remove` with an item that is no string (`IdsExpected`), no id
(`IdEmpty`), of no element (`NoElement`) or given twice (`NamedTwice`), and an empty list to remove
(`TooFew`); `element.describe`'s id none or of no element likewise; an element of no notation the
project holds, as a diagram, removed or described (`NotA` a `project::NOTATION_ELEMENT`); and a
notation's elements it does not describe yet (`NotDescribable`). A list longer than the store's
limit stays untyped, as the engine's limit on a call's size refuses one that long first.

**Tests.** `tests/diagram.rs`, over the MOF core's toy metamodel with a test notation of classes and
generalizations: the registry and the metamodel; every command's round trip; refusals that name
their key and change nothing, with their reasons typed; the cascades on the store with the delta inverted back; the grid's
topmost node, nearest edge and viewport; the rules over hand-made data; the sweep; seeded random
sequences with the rules and the store's validation checked after each step and undo back to the
start; a default-diagram binding as a Model member read back from its rows under the same ids and
references, its creation undone and redone exactly, its ends required and owned by nothing, and
found from either end. `archforge-er/tests/diagram.rs` runs the commands with the ER policy.

**Known lacks.** The project crate persists diagrams; they do not load lazily. The style
sheet's contents and the drawing come with the canvas. Arrange is a layered layout without dummy boxes for long lines; routes are worked out when drawn, never
stored, and an avoiding route that finds no way round takes the plain right-angled one; `visible`
culls edges by their bounds, so a long diagonal edge is drawn in more viewports than it
crosses, and a viewport wider than 4,096 cells is answered by a scan of the items. The grid is
rebuilt from the store, not maintained incrementally.

**Non-goals.** Rendering, UMLDI export, storage, any notation's own rules.
