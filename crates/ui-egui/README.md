# archforge-ui-egui

**Purpose.** The user interface: Home, the start screen, the New project dialog over it, and the
workspace once a project is open (`workspace`: the project bar, the activity rail and the Models
explorer over the session (`explorer`: each notation's rows, ER's schemas, tables, columns and
skills; `+` adds what the selection's notation offers, a name is typed in place, Delete asks when
other things go with it; `notation_ui::NotationUi` is what a notation brings to the tree, the
search, the problems and Properties, `er_ui` ER's, `uml_ui` UML's packages, classes, interfaces, enumerations, data types and primitive types, and
`uml_classifier` the class diagram's classifier tool with its menu of kinds),
the Diagrams explorer (`diagrams`: the model's folders and diagrams, the model behind the open
diagram, the New diagram dialog from its `+` and the tab strip's), the centre (`canvas`: the
diagram tabs, the open diagram drawn read-only from the diagram metamodel in the design's style
with each notation's cards and lines as it describes them (`archforge_diagram::figure`: ER's
table cards and crow's foot lines), notes, links and labels, pan and zoom, the minimap with the zoom control, the canvas
toolbar, whose tools after Select and Pan are the open diagram kind's (`kinds::DiagramKind`: its
tools, each placing or connecting, and the hooks the gestures run: place, connect, a line dropped
on empty canvas, rename in place, open at a card's row, its adorner, its menu, the live line;
`er_kind`: ER's); a click selects what it hits; `editing`: the Entity tool places a table named in place,
Select drags, resizes, marquees, nudges and arranges nodes, the entity menu removes from the
diagram or deletes from the model after the dialog, a node grows with its columns; `connect`: the
Relationship tool and the connect adorner drag a line from table to table with the cardinality
popover's ends, the relationship, its edge and its label as one step and the foreign key when the
parent has a key, lines coloured by key status, a mapping table offered for many to many, Delete
with a toast's Undo; `relation`: a relationship's Properties — Identifying, the ends with their
cardinality buttons and roles, the Foreign key switch that adds or removes the key with its
columns in one step, Points to, the key pairs with a child column chosen among those of the same
type, On delete / On update, More with Deferrable and Match, and the card for a parent without a
key with Add id column; `keys`: a table's keys with the key editor — name, the columns in key
order, Add a column, Nulls distinct — and the Problems section that lists what the rules find on
the selection; `checks` and `indexes`: a table's check constraints (name, expression) and indexes
(name, Unique, the columns in index order with a direction button and a nulls order from the row's
menu, an expression typed in place, Where and Method under More) with their editors; `problems`: those problems by element, badged on the canvas), the right rail between the
canvas and its panel with Properties (`panels`: a schema's name; a table's name, kind, columns
with the in-place column editor — type with its length or precision and scale, default,
generation with the expression of a computed column, collation under More — keys; a diagram's
name and schema; a description under every heading, the text DDL writes as a comment) and Skills (the skills on the
selection, the inherited ones, the skill editor) following the selection, the status bar), drawn
from the design with its own palette, type and icons (`ux`). Icons are drawn crisp: each is rasterised once per size into an alpha mask at the screen's own pixels, its
strokes a whole number of pixels on the pixel grid (`ux::raster`), and drawn at a standard size by
role with one stroke weight (`ux::icon_weighted`: chrome 15-17 pt, rows 13-15, small actions 10-14,
chevrons 8); rows, fields and bars centre their content on their middle line (`ux::text_mid`,
`ux::icon_mid`, `panels::field_inner`), and the macOS title strip is 32 pt tall with its controls
78 pt in, clear of the traffic lights. An open project has an in-memory ER session (`Shell::session`); nothing reaches the
project's database yet, and the status bar says so once there is something unsaved.

**Layer.** 9, over `archforge-notations` (8), `archforge-er` (7), `archforge-diagram` (6), `archforge-core` (5), `archforge-mof` (4), `archforge-engine` (3) and
`archforge-names`; the only library crate that may use the toolkit (egui). The shell never reads
the clock or the file system: the ids of a model's elements come from the host (`Services::ids`).

**Binding rules.** The shell rules, the localisation rules (`i18n/`), and the layering rule that
the shell never touches the file system: the host injects `Services`. Controls whose screens are
not built yet are drawn as designed, take no click and say "Not available yet" on hover; a panel
or explorer with nothing to show says so in one line, never with a help text.

**Public API.**

| Item | Signature |
|---|---|
| `Shell::new` | `pub fn new(services: Services) -> Shell` |
| `Shell::logic` | `pub fn logic(&mut self, ctx: &egui::Context)` (once per frame, before drawing) |
| `Shell::ui` | `pub fn ui(&mut self, ui: &mut egui::Ui)` (draws the whole window) |
| `Shell::request_quit` | `pub fn request_quit(&mut self)` (closes the window at the next frame) |
| `Shell::open_project`, `Shell::close_project` | `pub fn open_project(&mut self, opened: Opened)`, `pub fn close_project(&mut self)`: the workspace replaces Home and back |
| `Shell::project`, `Shell::session`, `Shell::session_error` | the open project, its session (made once per project through `archforge_notations::new_project_session`), why none could be made |
| `Shell::location` | `pub fn location(&self) -> Option<String>`: where the project lives, as the status bar says it |
| `state::{Explorer, Panel}` | the workspace's explorer and right panel, part of `UiState` |
| `Shell::attach_control` | `pub fn attach_control(&mut self, requests: Receiver<control::ControlRequest>)`: agent access on; the shell answers once per frame |
| `control::{ControlRequest, answer, answer_from, abandon, WAITING, TAKEN, ABANDONED, METHODS, READS, MAX_STEPS, MAX_REFERENCED, MAX_RESULTS}` | a request carries `state`: one given up (`abandon`) before the shell takes it is never run; it carries `origin` too, the label `archforge-cli mcp --origin` set, and `answer_from` puts it on the undo steps the request makes (`<origin>: <label>`), in the result and in `document.inspect` (`undo_origin`, `redo_origin`); `READS` names the methods that only read (`engine.commands`, `engine.describe`, `document.inspect`, `ui.inspect`, `app.saved`); a batch step's references bring in at most 1 MiB and its results at most 8 MiB in all; the control channel's methods (`engine.commands`, `engine.describe`, `engine.execute`, `engine.batch`, `engine.undo`, `engine.redo`, `document.inspect`, `ui.inspect`, `app.save`, `app.saved`) answered through the open session; no network code here |
| `Shell::widgets` | `pub fn widgets(&self) -> &[widgets::Entry]` (the controls of the last frame, by automation id) |
| `Shell::top_inset` | `pub top_inset: f32`, a field: the logical pixels at the top the host draws over (the macOS title bar) |
| `Services` | host functions: the installed fonts, the account's name, the folder new projects go in, the projects host, the id source for a model's elements, and the AI Agents services (`agents`, `agent_accounts`) |
| `ai_agents::{AgentHost, AgentAccounts, NotYet, Look, Keys, Program, Key, Store, KeyText, CodexAccount, AccountError, LoginWay, LoginEvent, AgentPrefs, Provider}` | Settings → AI Agents' host services: `AgentHost` finds the programs (`look`, at start) and reads and keeps the keys in the secure store (`read_keys`, first when the page needs them, never at launch; local, sends nothing); `AgentAccounts` sends the test message and runs Codex's ChatGPT sign-in (`NotYet` answers not available yet); every call answers on a channel from the host's own thread; `fake::Fake` (tests and feature `snapshot`) answers what it was set up with and counts the calls |
| `settings::{open, open_at, Section}` | Settings over the window, on Profile or AI Agents |
| `ProjectHost` | `fn create(&self, folder: &Path, name: &str) -> Receiver<Result<Opened, CreateError>>` (the host creates on its own thread) |
| `Opened`, `CreateError` | an open project; why a creation failed, in the kinds the dialog explains |
| `UiState` | the stored appearance, language, Home's closed banner, the workspace's explorer and panel and their widths |
| the title strip's controls | `strip.home`, `strip.open-project`, `strip.theme` on every screen; on the workspace `strip.home` closes the project |
| `i18n::{tr, tr_ctx, trn, set_language}` | catalogue lookups at draw time |
| `offscreen::render` | `pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String>` (feature `snapshot`) |
| `offscreen::render_screen` | the same for a `Screen`: Home, the New project dialog over it, the workspace, its first schema being named, a table's Properties, the Skills tab, a first diagram just created, or a diagram of two tables |
| `explorer::{State, add, remove_selection, rows, fresh_name, Row, Tree}` | the Models explorer's state and the actions the tests and the host drive |
| `panels::{State, draw, retain_valid, ColumnEdit, SkillEdit, Applies, Menu}` | the right panels' state and body; every change goes through `er.*` and `skill.*` commands; the Properties and Skills bodies scroll under the pinned heading and description, an editor that opens is brought into view, and a thin thumb shows while the content overflows (`widgets::Registry::reach` measures how far the controls go) |
| `diagrams::{State, rows, open_new, retain_valid, diagram_name, schema_of}` | the Diagrams explorer's state, its rows and the New diagram dialog |
| `canvas::{State, Tool, open, close_tab, retain_valid}` | the centre's state: the tabs, the view (pan, zoom), the tool, the toggles and the editing state; `open` shows a diagram through `view.open-diagram` |
| `editing::{add_entity, run_growing, content_height, diagrams_showing, NodeRename, Gesture, EntityMenu}` | canvas editing: a table added (and placed on an open diagram) as one step, a column command that grows the table's nodes with it, the gesture and menu state |
| `connect::{RelKind, MappingOffer, Toast, connect, offer_mapping, has_foreign_key, remove_selected_relationship}` | relationships drawn on the canvas: the ends a new line gets, the mapping table offer, the toast |
| `relation::{RoleEdit, retain_valid}` | a relationship's Properties state: a role being typed; the body itself is drawn by `panels` |
| `keys::{KeyEdit, KeyRow, rows, open}` | a table's keys and the key editor's state |
| `checks::{CheckEdit, CheckRow, rows, open}` | a table's check constraints and the check editor's state |
| `indexes::{IndexEdit, IndexRow, ColumnSpec, rows, open}` | a table's indexes and the index editor's state |
| `subtypes::{SubtypeRow, rows}` | a supertype's Subtypes section with Complete, Disjoint, Discriminator and Mapping (slice S6); the canvas's Generalization tool draws a subtype line with a hollow triangle at the supertype |
| `objects::{ObjectKind, ObjectEdit, ObjectRow, sequence_rows, view_rows, open, kind_of}` | a schema's sequences and views with their editors (slice S7); the Models tree lists them under the schema, a table row drags onto the canvas to be placed with its lines, and a table of another schema draws dimmed and dashed with its schema named |
| `types::{TypeKind, TypeEdit, TypeRow, rows, open, kind_of}` | a schema's domains and enum types with the domain and enum editors (slice S5); the column editor's type menu lists them before the base types |
| `panels::ColumnEdit::linked` | the column a key column follows, shown as a link mark on its row and on the canvas, with Unlink in its editor (slice S4); a pure mapping table's Properties offer to collapse it |
| `problems::{Problem, all, by_element, of}` | the problems the shell shows, on the element the user would fix them on |
| `new_project::{open, name_problem, file_name}` | the New project dialog: open it, a name's problem in words, the file a name gives |
| `Folder` | a folder as the shell shows it: its path and its label |

**Non-goals.** The screens behind Home's other controls, the Source Control explorer's contents,
the Agent and Appearance panels, relationships and foreign keys in Properties, key editing beyond
key editing past name, columns and Nulls distinct, the drawing
tools' menus, folders and renaming in the Diagrams tree: their own rows.
