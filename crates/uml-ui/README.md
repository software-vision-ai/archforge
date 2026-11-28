# archforge-uml-ui

**Purpose.** UML's screens (`forge-ux/adr/0002`, "Notations are plugins"): what UML brings to the
shell through the shell's interfaces (`archforge-shell-api`), and nothing of the shell itself. Its
screens part, `UML`, gives the shell UML's notation (`NotationUi`: its packages, classes,
interfaces, enumerations, data types and primitive types in Models and the search, Properties'
bodies and editors for members, ends, constraints, qualifiers, template parameters and bindings,
nested classifiers, generalization sets, profiles and the stereotypes applied, its problems,
refusals and words) and the class diagram's kind (`DiagramKind`: the classifier and Relationship
tools and their gestures, its cards, lines, n-ary associations, adornments and overlays, and
package navigation with its way back). Each screen reaches the shell through the narrow context
(`archforge_shell_api::cx`), never the whole shell, and draws through the design, `ux`: the
shell's interfaces' design, re-exported with the icons only UML draws (`ux::icons`). Every text
it says goes through the catalogues; the rows only UML's screens name are its own, in
`src/i18n/uml/<code>.tsv`, embedded, handed to the shell with the notation
(`NotationUi::catalogues`), and checked at build time by the interfaces' row check
(`build.rs`). Its modules are private: the app registers `UML`, and behind the `snapshot`
feature `snapshot` lists what the screenshot tool, the app's tests (those across every catalogue
and UML's window tests) and the shell's tests reach in them. Its tests run against the shell's
interfaces' test context (`archforge_shell_api::testing`, `forge-ux/adr/0005`, "Tests follow the
plugin"), without the shell: what UML's own code draws or decides (its refusal words, its hooks,
its Properties bodies and editors) in `src/tests.rs`, drawn in both appearances. Its window tests
(the canvas, its tools and tabs, and relationships; Properties, Models, profiles and stereotypes,
Chat, the menus, the rail's panels and the views of a version) drive the whole window with the
notations the app registers, so they are the app's: `apps/archforge/tests/uml_window/`, one module
for each area beside `main.rs`.

**Layer.** 10, the notations' screens' layer: above the shell's interfaces (9), below the shell
(11). It takes `archforge-uml` (7), UML's model, `archforge-shell-api` (9) and the crates below
them: `archforge-diagram` (6), `archforge-core` (5), `archforge-mof` (4) and `archforge-engine`
(3), with egui and serde_json. Behind its `snapshot` feature, never shipped, it also takes
`archforge-notations` (8), whose store and session the sample builders make the sample model on.
Builds for the portable target; holds no process or network code (the gate's `layers` step scans
it as it scans the shell).

**Public API.**

| Item | Signature |
|---|---|
| `UML` | `pub const UML: ScreensPart`: UML's screens part, its `NotationUi` with the class diagram's kind, which the app registers (`static SCREENS: ScreensRegistry = ScreensRegistry::new(&[ER, UML])`) |
| `snapshot::{uml_ui, uml_state, uml_classifier, uml_panel, uml_inline, uml_relate, uml_nary, uml_sets, uml_constraints, uml_qualifiers, uml_templates, uml_bindings, uml_nesting, uml_profiles, uml_stereotypes, uml_package, uml_package_nav, uml_nav_route, icons}` | feature `snapshot`: the items of UML's modules the screenshot tool and the shell's tests reach, each under its module's name: the sample builders over the narrow context (`uml_ui::open_sample`, `put_payment_kinds`, `uml_profiles::put_profiles` and the rest), UML's state and menus, the editors they open and the items their tests name |
| `snapshot::word_lists` | feature `snapshot`: `pub fn word_lists() -> Vec<(&'static str, Vec<&'static str>)>`: every word a module hands `tr` through a table rather than as a literal, each list with the name of its module (`src/<name>.rs`), for the app's tests across every catalogue |

**Non-goals.** The shell's own screens, panels and canvas, which UML's screens are drawn inside;
ER's screens (`archforge-er-ui`); the catalogues of the shell and of its interfaces; anything that touches the file
system or the operating system, which the host does through `Services`.
