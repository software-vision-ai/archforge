# archforge-er-ui

**Purpose.** ER's screens (`forge-ux/adr/0002`, "Notations are plugins"): what ER brings to the
shell through the shell's interfaces (`archforge-shell-api`), and nothing of the shell itself. Its
screens part, `ER`, gives the shell ER's notation (`NotationUi`: its schemas, tables, columns,
domains, enum types, sequences and views in Models and the search, Properties' bodies and editors
for a table and its columns, keys, check constraints, indexes and subtypes, a relationship's ends,
roles and foreign key, a schema's types and objects, its problems, refusals and words) and the ER
diagram's kind (`DiagramKind`: the Entity and Relationship tools and their gestures, a table
placed named in place and grown with its columns, a relationship drawn from table to table with
the cardinality popover's ends and its crow's-foot marks, a line dropped on empty canvas making a
new table, the entity menu's rows, and the mapping table a many-to-many line offers). Each screen
reaches the shell through the narrow context (`archforge_shell_api::cx`), never the whole shell,
and draws through the design, `ux`: the shell's interfaces' design, re-exported with the icons
only ER draws (`ux::icons`). Every text it says goes through the catalogues; the rows only ER's
screens name are its own, in `src/i18n/er/<code>.tsv`, embedded, handed to the shell with the
notation (`NotationUi::catalogues`), and checked at build time by the interfaces' row check
(`build.rs`). Its modules are private: the app registers `ER`, and behind the `snapshot` feature
`snapshot` lists what the screenshot tool, the app's tests and the shell's tests reach in them.
Its tests run against the shell's interfaces' test context (`archforge_shell_api::testing`,
`forge-ux/adr/0005`, "Tests follow the plugin"), without the shell: what ER's own code decides
(its refusal words) in `src/tests.rs`. Its window tests (the canvas, its tools and relationships,
Properties, ER's refusals that take a row of the shell's catalogue, and the shell asking both
notations) drive the whole window with the notations the app registers, so they are the app's:
`apps/archforge/tests/er_window/`, one module for each area beside `main.rs`.

**Layer.** 10, the notations' screens' layer: above the shell's interfaces (9), below the shell
(11). It takes `archforge-er` (7), ER's model, `archforge-shell-api` (9) and the crates below
them: `archforge-diagram` (6), `archforge-core` (5), `archforge-mof` (4) and `archforge-engine`
(3), with egui, serde_json and log. Builds for the portable target; holds no process or network
code (the gate's `layers` step scans it as it scans the shell).

**Public API.**

| Item | Signature |
|---|---|
| `ER` | `pub const ER: ScreensPart`: ER's screens part, its `NotationUi` with the ER diagram's kind, which the app registers (`static SCREENS: ScreensRegistry = ScreensRegistry::new(&[ER, UML])`) |
| `snapshot::{er_ui, er_kind, er_said, er_place, er_state, er_table, er_types, er_relation, er_indexes, er_objects, er_keys, er_checks, er_connect, icons}` | feature `snapshot`: the items of ER's modules the screenshot tool and the tests outside the crate reach, each under its module's name: ER's notation and kind, its refusal words, its tables' geometry, its state and menus, the editors they open with their rows, the column editor the app's journeys fill in (`er_table::{column_edit, column_edit_mut}`, over the notations' states), the Relationship tool's kinds and mapping offer, and the Entity tool's icon |
| `snapshot::word_lists` | feature `snapshot`: `pub fn word_lists() -> Vec<(&'static str, Vec<&'static str>)>`: every word a module hands `tr` through a table rather than as a literal, each list with the name of its module (`src/<name>.rs`), for the app's tests across every catalogue |

**Non-goals.** The shell's own screens, panels and canvas, which ER's screens are drawn inside;
UML's screens (`archforge-uml-ui`); the catalogues of the shell and of its interfaces; anything
that touches the file system or the operating system, which the host does through `Services`.
