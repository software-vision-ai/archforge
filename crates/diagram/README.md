# archforge-diagram

**Purpose.** The diagram metamodel for every notation (decision gm2.21): a `Diagram` of some
element shows model elements as `Node`s and relationships as `Edge`s, with `Label`s that name a text
slot of their element and `Style`s over the shipped style sheet; our own, in the spirit of OMG DI,
with UMLDI export a later task. Generated from `diagram.xmi` by `cargo xtask generate diagram`;
the rules, the generic commands, the cascades and the spatial index are written here.

**Layer.** 5, beside the shared core, above `archforge-mof` (4) and `archforge-engine` (3); pure,
builds for the portable target. Knows no notation: everything a view shows is typed Any, and a
notation's view type brings the policy through `DiagramView`. The notations (6) depend on it.

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
contains, the diagrams of them, the edges that show them and the nodes that show them with the
edges on those nodes (`remove_views_of`); a notation calls it before it destroys an element.

**The metaclasses.** `Diagram` (kind, name, element, node, edge, style), `Node` (element, kind,
text, x, y, width, height, child, style), `Edge` (element, source, target, waypoint, label, style),
`Point` (x, y), `Label` (kind, at, dx, dy), `Style` (name, fill, stroke, text, line, routing,
avoid) and the enumerations `LabelPlace` (source, middle, target) and `Routing` (asDiagram,
straight, rightAngled, curved); `Element` abstract above them. A diagram's style is the look its
lines take unless their own says otherwise; a new diagram's lines run at right angles around the
tables (archforge-4ym).

**Public API.**

| Item | Signature |
|---|---|
| `DiagramView` | the notation's policy: the open diagram and selection, `kinds`, `home`, `is_of`, `needs_element`, `shows`, `may_show`, `connects`, `ends`, `text_slots`, `default_size` |
| `commands::specs` | `pub fn specs<V: DiagramView>() -> Vec<CommandSpec<Store, V>>`: the diagram commands for a notation's registry |
| `metamodel`, `universe_with`, `with_slabs` | the metamodel; a universe of it and others; the slabs joined to a `CompositeSlabs` |
| `remove_views_of`, `destroy_node_with_edges`, `views_of`, `edges_on`, `node_tree`, `diagram_of` | the cascades and the walks they rest on |
| `Grid`, `Hit`, `Rect`, `grid::Visible` | `Grid::build(store, diagram)`, `hit(x, y)`, `visible(rect)`: the spatial index of one diagram, its lines routed as drawn |
| `arrange::arrange` | `arrange(sizes, lines) -> Vec<(f64, f64)>`: boxes in layers along their lines, crossings few, groups side by side, lone boxes in rows |
| `route`, `Way`, `route::{way_of, spreads}` | `route(way, from, to, at, via, boxes) -> Vec<(f64, f64)>`: a line straight, at right angles, around the other boxes, or curved; `way_of` reads an edge's routing (its own, else its diagram's); `spreads(store, diagram, rect)` places every line's ends along its boxes' sides, once per diagram |
| `rules::{problems, kind_problem, text_problem, colour_problem, coord_problem, size_problem, depth_of}` | the rules, for commands and for a whole store |
| the generated constants, structs, `LabelPlace`, `TABLES`, `JUNCTIONS` | as every generated metamodel |

Commands: `diagram.{add, rename, remove, place, note, set-text, move, resize, unplace, connect,
connect-all, disconnect, route, label, unlabel, style, arrange}`, `view.open-diagram`, and the queries
`diagram.{list, get}`. `diagram.move` takes the nodes with what they contain and the waypoints of
the edges between them, so a multi-node drag is one step (the engine's `coalesce` key joins the
steps of one drag); `diagram.style` sets a style as a whole and a call naming nothing removes it;
`diagram.connect-all` adds an edge for every relationship whose both ends the diagram shows and
which it does not show yet.

**Tests.** `tests/diagram.rs`, over the MOF core's toy metamodel with a test notation of classes and
generalizations: the registry and the metamodel; every command's round trip; refusals that name
their key and change nothing; the cascades on the store with the delta inverted back; the grid's
topmost node, nearest edge and viewport; the rules over hand-made data; the sweep; seeded random
sequences with the rules and the store's validation checked after each step and undo back to the
start. `archforge-er/tests/diagram.rs` runs the commands with the ER policy.

**Known lacks.** Nothing persists a diagram until gm2.31, where diagrams load lazily. The style
sheet's contents and the drawing come with the canvas (gm2.10). Arrange is a layered layout without dummy boxes for long lines; routes are worked out when drawn, never
stored, and an avoiding route that finds no way round takes the plain right-angled one; `visible`
culls edges by their bounds, so a long diagonal edge is drawn in more viewports than it
crosses, and a viewport wider than 4,096 cells is answered by a scan of the items. The grid is
rebuilt from the store, not maintained incrementally.

**Non-goals.** Rendering, UMLDI export, storage, any notation's own rules.
