# archforge-ui-egui

**Purpose.** The user interface: Home, the start screen, the New project dialog over it, and the
workspace once a project is open (`workspace`: the project bar, the activity rail and the Models
explorer over the session (`explorer`: each notation's rows, ER's schemas, tables, columns and
skills; `+` adds what the selection's notation offers, a name is typed in place, Delete asks when
other things go with it; `notation_ui::NotationUi` is what a notation brings to the tree (it and
the data types it hands across are `archforge-shell-api`'s `notation_ui`, re-exported under their
old paths), the search, the problems and Properties, how a row dropped on another runs, what it says of a
removal of its own elements, worked out before it runs and said in the toast after
(`notation_ui::Removal`: the shared removals ask the element's notation), what Delete and a
chord of its own on the canvas do to its selected elements, what it draws over the canvas
whatever diagram shows, each layer at its one place in the canvas's order (`notation_ui::Layer`:
drafts, rows, menus, offers), what a problem badge that offers a way out says and does, and what an
opening puts right in its own layout before the session starts (`open_repair`, which runs every
notation's in turn), `er_ui` ER's, `uml_ui` UML's packages, classes, interfaces, enumerations, data types and primitive types;
`notation_ui::NotationState` (`archforge-shell-api`'s too) is what a notation keeps between frames for its own editors, menus,
drawings and tools, `er_state` ER's and `uml_state` UML's, which the shell holds, parks with the
project, starts fresh with each project opened and each version shown, and asks only through that
trait: whether something is being edited, which editors are open, what to drop, what to close,
whether something of its own holds the canvas's keys (an open menu holds them without being an
edit), whether a menu of its own takes the keys, to end a drawing under way when its project
is set aside, what of its own a press or Escape on the canvas closes, and the row typed in place
on a card that the card is drawn with; the shell keeps one open-menu slot, which holds a `panels::MenuId` (`archforge-shell-api`'s
`notation_ui::MenuId`, re-exported under its old path): the shell's own
menus (Home's, the Versions panel's) in their scope, each notation's by its metamodel in its own,
with a key and an index, so a notation opens, recognises and closes only its own (`er_state::menus`,
`uml_state::menus`); and
`uml_classifier` the class diagram's classifier tool with its menu of kinds),
the Diagrams explorer (`diagrams`: the model's folders and diagrams, the model behind the open
diagram, the New diagram dialog from its `+` and the tab strip's, a diagram row's menu and Delete
key, which delete a diagram that is no package's default), the centre (`canvas`: the
diagram tabs, the open diagram drawn read-only from the diagram metamodel in the design's style
with each notation's cards and lines as it describes them (`archforge_diagram::figure`: ER's
table cards and crow's foot lines; its geometry that names no shell, the view's mapping and where lines, marks, handles and end texts sit, is `archforge-shell-api`'s `canvas`), notes, links and labels, pan and zoom, the minimap with the zoom control, the canvas
toolbar, whose tools after Select and Pan are the open diagram kind's (`kinds::DiagramKind`, the
interface in `archforge-shell-api`'s `kinds` with the data types it hands across, what one frame's
drawing reads (`canvas::Drawing`) and what a press asks for (`editing::Action`), re-exported under
their old paths, while the shell keeps the kinds it knows (`kinds::KINDS`, ER's and UML's) and
asks them through thin forwarders over the whole shell (`kinds::kind_of`, `kinds::active_kind`,
`kinds::popover_takes_keys`): its tools, each placing or connecting, and the hooks the gestures run: place, connect, a line dropped
on empty canvas, rename in place, open at a card's row, its adorner, the rows its element menu adds
with what each does, the live line, a
press and a right click it reads first, the keys a drawing under way takes, whether its popover's
menu takes the keys, what Select all takes in beside the boxes and lines, and where a label of
its own figures is let go; and
the hooks the canvas runs as it draws: before and after a diagram is drawn, where a line starts,
what goes with a line, the figures of no one node or line, and the hint bar, through which UML
draws its association classes' ties and names, its generalization sets, its n-ary legs and hint,
and runs its package navigation, whose state is UML's own; and its rows of the Model menu, as
data (`kinds::ModelRow`: the row's id, its word, the tool it arms and the choice of the tool's
popover it makes first), from which `model_menu` makes the menu's entries, Note and Link note after them;
`er_kind`: ER's, whose entity menu adds a table's related tables, new column and collapse); a click
selects what it hits; `editing`, shared by every kind: Select drags, resizes, marquees, nudges and
arranges nodes, the element menu opens in Models, hands the element to an agent, removes it from
the diagram or deletes it from the model after the dialog; `er_place`: the Entity tool places a
table named in place, a relationship dropped on empty canvas makes a new table, a table dragged
from Models lands with its lines, and a table's boxes grow with its columns; `connect`: the
Relationship tool and the connect adorner drag a line from table to table with the cardinality
popover's ends, the relationship, its edge and its label as one step and the foreign key when the
parent has a key, lines coloured by key status, a mapping table offered for many to many, Delete
with a toast's Undo; `relation`: a relationship's Properties — Identifying, the ends with their
cardinality buttons and roles, the Foreign key switch that adds or removes the key with its
columns in one step, Points to, the key pairs with a child column chosen among those of the same
type, On delete / On update, More with Deferrable and Match, and the card for a parent without a
key with Add id column; `er_table`: a table's Properties — name, kind, the columns with the
in-place column editor (type with its length or precision and scale, default, generation with the
expression of a computed column, collation under More), then its keys, checks, indexes, subtypes
and problems; `keys`: a table's keys with the key editor — name, the columns in key
order, Add a column, Nulls distinct; `checks` and `indexes`: a table's check constraints (name, expression) and indexes
(name, Unique, the columns in index order with a direction button and a nulls order from the row's
menu, an expression typed in place, Where and Method under More) with their editors; `problems`: those problems by element, badged on the canvas), the right rail between the
canvas and its panel with Properties (`panels`: the body the selection's notation draws, a scroll kept
while the selection moves within what its notation calls one page (a column and its table), a
diagram's name and schema,
the Problems section that lists what the rules find on the selection, and a description under
the heading of every model element (a diagram's heading has none): an ER element's is the text DDL
writes as its comment, a UML element's a Comment it owns) and Skills (the skills on the
selection, the inherited ones, the skill editor) following the selection, the status bar), drawn
from the design with its own palette, type and icons (`ux`, the design of `archforge-shell-api` re-exported with the shell's app icon, its fonts and the icons only ER or only UML draws); its colours are `ux::Colour` values, which only `ux` makes. Icons are drawn crisp: each is rasterised once per size into an alpha mask at the screen's own pixels, its
strokes a whole number of pixels on the pixel grid (`ux::raster`), and drawn at the standard size
each icon carries, by role, with one stroke weight (`ux::Icon::standard_size`, drawn by
`ux::icon_weighted`: chrome 15-17 pt, rows 13-15, small actions 10-14, chevrons 8); rows, fields
and bars centre their content on their middle line (`ux::text_mid`,
`ux::icon_mid`, `panels::field_inner`), and the macOS title strip is 32 pt tall with its controls
78 pt in, clear of the traffic lights. An open project has an in-memory ER session (`Shell::session`); nothing reaches the
project's database yet, and the status bar says so once there is something unsaved. A notation's
screens and the shared controls reach the shell through the narrow context (`archforge-shell-api`'s
`cx`, re-exported crate-private under `cx`, where the shell builds it; `forge-ux/adr/0002`): `cx::Ctx`, the parts they change, each borrowed apart by `Shell::cx` (the
session, the panels' and the canvas's state, the automation registry, the notations and each
one's own state, the problems, the frame count, the menu bar's hold on an inline editor, whether a
version shows), its read-only twin `cx::CtxRef` (`Shell::cx_ref`: the session, the canvas's
state, the notations and their states, whether a version shows), and `cx::Request`, what lies
outside it, which the shell carries out once the call returns, in order, each once: the Models
tree's rows opened over a selected element, the centre leaving a version's details for what was
opened (`canvas::open` opens at once), a panel of the right side shown, an explorer of the left
side shown, a new element's name opened for typing in Models, and the delete dialog opened on a
removal. ER's screens take the context, and so do UML's leaf screens (`package_nav`,
`uml_classifier`, `uml_bindings`, `uml_constraints`, `uml_nesting`, `uml_sets`, `uml_templates`),
its panel screens (`uml_panel`, `uml_relate`, `uml_profiles`, `uml_stereotypes`, `uml_nary`,
`uml_qualifiers`) and its member typed in place on a card (`uml_inline`). The interfaces hand a
notation the context, not the shell: each method of `NotationUi` and `DiagramKind` that reaches
the shell takes `&mut cx::Ctx`, or `&cx::CtxRef` where it only reads (a tool's face and hint,
where a line starts, the live line, a press and a right click), and so do a row of an element's
menu (`kinds::MenuRow::run`) and what a kind makes of a press (`editing::Action::Own`). The shell
builds the context at each call (`Shell::cx`, `Shell::cx_ref`) and carries out its requests once
the method returns, so ER's and UML's kinds and Properties (`er_kind`, `er_ui`, `uml_ui`) build
none. The controls that take it end in `_cx` (`panels::section_cx`, `panels::control_cx`,
`menu_bar::ends_cx`, `notation_ui::say_after_cx` and the rest); their old names over the whole
shell forward to them for the shell's own screens, and `home::control` stays the shell's own way
to `panels::control_cx`. The shared controls (the `_cx` controls of `panels`, from `section_cx`
and `control_cx` to `name_field_cx` and `reveal_cx`, with their geometry `PAD`, `FIELD`, `ROW` and
`PATH_SEP`), the problems a selection shows (`problems::of`) and the key presses they take
(`actions::{Keys, take, take_alone, held_exactly}`) are `archforge-shell-api`'s, re-exported
crate-private under their old paths where the shell uses them (`actions::Keys` public, as
`actions::Action` names it). The refusal words (`panels::refusal`, `panels::said`, `panels::a_kind`
and `engine_said::disabled`, `archforge-shell-api`'s) ask the notations they are handed for their
own: the context's or the shell's (`notation_uis`), and the app's (`notation_ui::UIS`) where a
notation's code holds neither (its own refusal words, its problems and the checks that take only
the store).

**Layer.** 11, the top of the interface's layers, above the shell's interfaces
(`archforge-shell-api`, 9) and the notations' screens (10); over `archforge-notations` (8),
`archforge-er` (7), `archforge-diagram` (6), `archforge-core` (5), `archforge-mof` (4),
`archforge-engine` (3) and `archforge-names`; with the shell's interfaces, the only library crates that may use the toolkit (egui). The shell never reads
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
| `state::{Explorer, Panel}` | the workspace's explorer and right panel, part of `UiState`; `archforge-shell-api`'s `state`, re-exported under their old paths, as the narrow context's requests name them |
| `Shell::attach_control` | `pub fn attach_control(&mut self, requests: Receiver<control::ControlRequest>, generation: control::SessionGeneration)`: the requests of every opening of agent access (the person's own and Chat's); the shell answers once per frame, as each request's token allows |
| `Shell::set_own_agent_access` | `pub fn set_own_agent_access(&mut self, on: bool)`: whether the person turned agent access on for this start, which File ▸ Generate… reports; the queue and Chat's access don't change it |
| `control::{ControlRequest, SessionGeneration, answer, answer_from, answer_request, abandon, only_reads, Access, ACCESS, REVOKED, VIEWING, CHANGED, HELD, WAITING, TAKEN, ABANDONED, METHODS, READS, MAX_STEPS, MAX_REFERENCED, MAX_RESULTS}` | a request carries its token's role (`read_only`) and liveness (`live`): `answer_request` refuses a revoked token's request with `REVOKED` and, for a read-only token, anything that may change (`only_reads`: `ACCESS` classifies each method once; `engine.execute` and `engine.batch` read only when every command they run is a query by the engine's own test; a batch past `MAX_STEPS` is refused before its steps are looked at); a request carries `state`: one given up (`abandon`) before the shell takes it is never run; it carries `origin` too, the label `archforge-cli mcp --origin` set (a conversation's token gives its own, bound when it was granted), and `conversation`, the key the channel bound that token to, by which the chat gate knows the conversation, never by its label (AR-230); `answer_from` puts the origin on the undo steps the request makes (`<origin>: <label>`), in the result and in `document.inspect` (`undo_origin`, `redo_origin`); `READS` names the methods that only read (`engine.commands`, `engine.describe`, `document.inspect`, `ui.inspect`, `app.saved`); a batch step's references bring in at most 1 MiB and its results at most 8 MiB in all; the control channel's methods (`engine.commands`, `engine.describe`, `engine.execute`, `engine.batch`, `engine.undo`, `engine.redo`, `document.inspect`, `ui.inspect`, `app.save`, `app.saved`) answered through the open session; no network code here |
| `Shell::widgets` | `pub fn widgets(&self) -> &[widgets::Entry]` (the controls of the last frame, by automation id) |
| `widgets` | the automation registry of `archforge-shell-api`, re-exported under the path the shell's code and its tests name it by |
| `canvas::self_loop` | `pub fn self_loop(rect: Rect, z: f32) -> Vec<Pos2>`: a self relationship's loop on a card's right side, from the canvas geometry of `archforge-shell-api`, which the shell re-exports in `canvas` under the paths its code names it by |
| `ux` | the design of `archforge-shell-api` (colours, type, icons and the helpers that draw them), re-exported under the path the shell's code and its tests name it by, with what needs the shell: `ux::app_icon_rgba` and `ux::scale_rgba` (the app icon built in), the installing of the design's fonts, and in `ux::icons` the icons only ER or only UML draws |
| `Shell::top_inset` | `pub top_inset: f32`, a field: the logical pixels at the top the host draws over (the macOS title bar) |
| `Shell::in_window_menu` | `pub in_window_menu: bool`, a field: whether the window draws its own menu bar in the title strip, set by the application everywhere but macOS |
| `actions::VERSIONS_MENU` | `[&[&str]; 4]`: the Versions menu's rows by section, on every system; `actions::{label, available, run}` answer for them |
| `actions::FILE_MENU` | `[&[&str]; 5]`: the File menu's rows by section, the same groups on every system, Settings… and Exit last; read through `actions::file_menu` |
| `actions::file_menu` | `fn(&Shell) -> Vec<Vec<&'static str>>`: File's rows on this system, by section: the native bar leaves out Settings… and Exit, which its application menu holds, and no section is left empty |
| `actions::file_title` | `fn(&Shell) -> String`: the File menu's title on this system, the native bar's own word where a language has one (German Ablage, Datei in the window's bar) |
| `actions::ticked` | `fn(&Shell, &str) -> Option<bool>`: whether a row that shows and hides something is ticked now (`view_menu::ticked`: the minimap, an explorer or a panel shows); `None` for a row with no tick |
| `view_menu::VIEW_MENU` | `[&[&str]; 4]`: the View menu's rows by section, on every system, with their ids beside it; `actions::{label, available, run}` answer for them |
| `model_menu::sections` | `fn(&Shell) -> Vec<Vec<&'static str>>`: the ids of the Model menu's rows now, by section: the rows the open diagram's kind answers as data (`DiagramKind::menu_rows`), then Note and Link note; `actions::{label, available, run}` answer for them, through the entries `model_menu` makes from that data |
| `Services` | host functions: the installed fonts, the account's name, the folder new projects go in, the projects host, the id source for a model's elements, the AI Agents and Chat host (`agents`), the project's kept conversations (`chats`) and the time with the local offset Chat shows times in (`clock`) |
| `ai_agents::{AgentHost, Look, Keys, Program, Key, Store, KeyText, CodexAccount, AccountError, LoginWay, LoginEvent, AgentPrefs, Provider, need_account, reread_account, sign_in_expired, approved_browser_page}` | AI Agents' and Chat's one host service, `AgentHost`: it finds the programs (`look`, at start), reads and keeps the keys in the secure store (`read_keys`, first when the page needs them, never at launch), sends the test message, reads Codex's account (`codex_account_read`: first when a Codex conversation starts, `need_account`, and again each time the page shows, on Check connection and on a Codex conversation's Sign-in expired card, `reread_account` and `sign_in_expired`; never at launch), runs its ChatGPT sign-in (a browser page opens only on `approved_browser_page`) and sign-out, and runs Chat's conversations (`connect`, `start`, `send`, `stop`, `end`) and removes a deleted one's private folder (`forget`, its program ended first, a bounded wait); every call answers at once, on a channel from the host's own thread; `CodexAccount`'s and `LoginEvent`'s `Debug` show an email, a code or an address by length only; `fake::Fake` (tests and feature `snapshot`) answers what it was set up with and counts the calls |
| `chat::{ChatStart, ChatRefusal, ChatEvent, TurnEnd, ChatFailure, FailureKind, HostEvent, EventSink, Ticket, start, send, stop, end, take_events, valid_id, check_id, MAX_SESSIONS, MAX_WAITING, MAX_RUNNING}` | Chat's conversations as the shell knows them (`ChatFailure` carries a rate limit's `reset`): an id is a conversation's lowercase UUID, checked before the host is asked; at most `MAX_SESSIONS` (64) kept, each with at most `MAX_WAITING` (4096) events waiting, past which an event is dropped and the first drop logged, an end never dropped; the host's events and agent access's changes (`HostEvent::Access`) come on one channel with a repaint; `Shell::chats.holds_access()` drives the status bar's Agent item (`workspace.status.agent`), which opens Settings on AI Agents |
| `chat_store::{ChatStore, Write, Answer, Refusal, Listed, Row, Stored, ToolFacts, Anchor, next_origin, …}` | where Chat's conversations are kept: the host's service over the open project (AR-206), every write on the project's writer and a question checked against the kept API keys before it is written; `Write::Delete` answers with the commit before the deletion (`Answer::Deleted`) and `Write::Restore` brings the conversation back from it (AR-217); a new conversation's label skips those `Write::Create` names; `fake::Memory` (tests and feature `snapshot`) keeps them in memory by the same rules |
| `chat_panel::{Panel, Chat, Turn, Item, send, stop, retry, set_allow_changes, choose, new_conversation, toggle_history, open, undo_turn, set_up, chips, context_line, QUESTION_BYTES, REPLY_BYTES, MAX_HELD, MAX_ROWS}` | the Chat panel of a project (`Shell::chat_panel`, parked with its tab): a question kept before the agent gets it, a turn's edits one undo step of the history (`Chat N: Turn n`) unless another change lands during the turn, its card undoing a turn of one step while it is on top, tool rows worded from the control channel's answers for the conversation's origin, Stop keeping the reply so far, Send again running a failed turn in place, no silent switch of agent or model; leaving the tab stops a running turn, closing the project ends its program; drawn by `chat_view` (page 07, B1 to B7), whose conversation follows the keyboard and scrolls by Page Up, Page Down, Home and End while it (a Tab stop of its own) or one of its links or buttons has it, and whose history follows the keyboard, the page keys moving it through the rows |
| `chat_markdown::{render, plain, web_link}`, `chat_words::{Clock, when, clock_time, date_and_time, seconds_of, badge, named_model, tool_words, kinds}` | a reply rendered from its Markdown at 13 on 19 (an image as its alt text, HTML as plain text, a link only to a web address that shows what it is, through the confirmation); times local with short translated months, the provider badge, a tool row's words |
| `settings::{open, open_at, Section}` | Settings over the window, on Profile or AI Agents |
| `ProjectHost` | `fn create(&self, folder: &Path, name: &str) -> Receiver<Result<Opened, CreateError>>` (the host creates on its own thread) |
| `Opened`, `CreateError` | an open project; why a creation or an opening failed, in the kinds the dialog explains; `UnsupportedKinds` names a project's diagram kinds Archforge does not support, worded on Home and in the status bar |
| `UiState` | the stored appearance, language, Home's closed banner, the workspace's explorer and panel and their widths |
| the title strip's controls | `strip.home`, `strip.open-project`, `strip.theme` on every screen; on the workspace `strip.home` closes the project |
| `i18n::{tr, tr_ctx, trn, set_language}` | catalogue lookups at draw time: the translations machinery of `archforge-shell-api`, re-exported under the path the shell's code and its tests name it by, with the shell's catalogues (`i18n::CATALOGUES`, `src/i18n/<code>.tsv`) and each notation's (`NotationUi::catalogues`: ER's in `src/i18n/er/`, UML's in `src/i18n/uml/`, the rows only that notation's screens name), which `Shell::new` registers as it sets up the notations it knows, so nothing the shell's interfaces say is looked up before them, and `tr`, `tr_ctx` and `trn` before their first lookup |
| `model_words::{words, property_label, literal_label, kind_label}` | the labels for what the metamodel names, looked up in the catalogues on the interface thread: every card property and literal of the model area, the kinds the notations create, in each notation's own word (a table, a column, a subtype), and `Element` for the rest |
| `offscreen::render` | `pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String>` (feature `snapshot`) |
| `offscreen::render_screen` | the same for a `Screen`: Home, the New project dialog over it, the workspace, its first schema being named, a table's Properties, the Skills tab, a first diagram just created, or a diagram of two tables |
| `explorer::{State, ContextMenu, add, remove_selection, rows, fresh_name, Row, Tree}` | the Models explorer's state (a row's context menu among it, AR-132) and the actions the tests and the host drive; `Row`, a row of the tree as drawn, and `Tree`, what a notation's rows are made with, are `archforge-shell-api`'s `notation_ui::Row` and `notation_ui::Tree`, re-exported under their old paths |
| `panels::{State, draw, retain_valid, SkillEdit, Applies, MenuId}` | the right panels' state and body, each notation asked what of its own ends as the selection moves on, whichever panel shows (`NotationUi::selection_moved`), and in Properties whether it draws its own panel in place of the selection's (`NotationUi::properties_in_place`); every change to the model goes through the shared commands (`diagram.rename`, `element.describe`), the notations' commands and the `skill.*` commands, and `view.select` selects the element a row names: a problem row whose problem is on another element than the one shown, and a row a Properties list shows (`select_element`); the Properties and Skills bodies scroll under the pinned heading and description, an editor that opens or is refused and a control the keyboard moves to are brought into view, Page Up, Page Down, Home and End scroll the body while one of its controls has the keyboard, and a thin thumb shows while the content overflows (`widgets::Registry::reach` measures how far the controls go, `laid_out` says where each is, in view or not); `State` (with what it holds: `NameEdit`, `DescriptionDraft`, `ReachedRows`, `MenuOrdinals`, and `appearance::Picker`) is `archforge-shell-api`'s `panels::State` and `MenuId` its `notation_ui::MenuId`, re-exported under their old paths |
| `diagrams::{State, RowMenu, rows, open_new, retain_valid, diagram_name, schema_of}` | the Diagrams explorer's state, its rows, a row's menu and the New diagram dialog |
| `versions::State` | the Versions explorer's state: what it has read, how its sections are folded, and what the keyboard has just moved to, which its list brings into view |
| `save_version::Dialog` | the Save version dialog: the description typed, the revision its summary is of, and how its save went |
| `canvas::{State, Tool, Toast, open, close_tab, retain_valid}` | the centre's state: the tabs, the view (pan, zoom), the tool, the toggles and the editing state (what a notation types, draws and opens there is its own state); the toast with Undo at the canvas's foot; `open` shows a diagram through `view.open-diagram`; `State`, `Tool` and `Toast` are `archforge-shell-api`'s `canvas::State`, `canvas::Tool` and `canvas::Toast`, re-exported under their old paths |
| `editing::{diagrams_showing, NodeRename, Gesture, EntityMenu}` | canvas editing: how many diagrams show an element, the name typed in place, the gesture and the element menu's state; `NodeRename`, `Gesture` and `EntityMenu` are `archforge-shell-api`'s `editing` types, re-exported under their old paths |
| `er_place::{add_entity, run_growing, content_height, fit_tables}` | ER's tables on its diagrams: a table added (and placed on an open diagram) as one step, a column command that grows the table's nodes with it, the height that shows a table's columns, and the boxes grown to show them when a project opens (ER's `NotationUi::open_repair`) |
| `open_repair` | what an opening does to a project's model before its session starts: every notation's own layout put right (`NotationUi::open_repair`, nothing by default), in the order of the notations, without an undo step; the shell opens, restores and compares models with it and hands it to the host for a restore |
| `connect::{RelKind, MappingOffer, connect, offer_mapping, has_foreign_key, remove_selected_relationship}` | relationships drawn on the canvas: the ends a new line gets, the mapping table offer |
| `relation::RoleEdit` | a relationship's Properties state, ER's own: a role being typed; the body itself is drawn by `panels` |
| `keys::{KeyEdit, KeyRow, rows, open}` | a table's keys and the key editor's state |
| `checks::{CheckEdit, CheckRow, rows, open}` | a table's check constraints and the check editor's state |
| `indexes::{IndexEdit, IndexRow, ColumnSpec, rows, open}` | a table's indexes and the index editor's state |
| `subtypes::{SubtypeRow, rows}` | a supertype's Subtypes section with Complete, Disjoint, Discriminator and Mapping (slice S6); the canvas's Generalization tool draws a subtype line with a hollow triangle at the supertype |
| `objects::{ObjectKind, ObjectEdit, ObjectRow, sequence_rows, view_rows, open, kind_of}` | a schema's sequences and views with their editors (slice S7); the Models tree lists them under the schema, a table row drags onto the canvas to be placed with its lines, and a table of another schema draws dimmed and dashed with its schema named |
| `types::{TypeKind, TypeEdit, TypeRow, rows, open, kind_of}` | a schema's domains and enum types with the domain and enum editors (slice S5); the column editor's type menu lists them before the base types |
| `er_table::{ColumnEdit, column_edit, column_edit_mut}` | a table's Properties, ER's own: the column editor's state, which is ER's state and which `column_edit` reads for the app's journeys |
| `er_table::ColumnEdit::linked` | the column a key column follows, shown as a link mark on its row and on the canvas, with Unlink in its editor (slice S4); a pure mapping table's Properties offer to collapse it |
| `problems::{Problem, all, by_element}` | the problems the shell shows, on the element the user would fix them on; `Problem` is `archforge-shell-api`'s `notation_ui::Problem`, re-exported under its old path, and what a selection shows is its `problems::of` |
| `new_project::{open, name_problem, file_name}` | the New project dialog: open it, a name's problem in words, the file a name gives |
| `Folder` | a folder as the shell shows it: its path and its label |
| `Picked` | how a folder pick ended (the folder, nothing, or why no picker could appear), sent once on the channel `Services::pick_project` and `Services::pick_folder` answer with; the shell takes it on a later frame, one pick at a time, with New project, Open project (Home's Open card, title-strip button and pinned folders, which act only while Open can), Home's recent projects, the search's projects and pinned folders, Pin a folder and Create project off while it waits, no other opening starting meanwhile, and no Open project pick starting while an opening runs or waits for a hold, nor any other folder pick while an opening waits for a hold, which a pick waiting would turn away; closing New project lets its Change… pick go, a quit asked for keeps the answer waiting until it is called off and lets the pick go once it goes ahead, a pick let go of is withdrawn from the host (`Services::withdraw_pick`), so a picker for it not yet shown never shows, while one already on screen stays until the user closes it, its answer unused, and the host shows a pick asked meanwhile once it closes, and a host gone without answering is a cancel; `Picked::at_once` is a pick already answered |

**Non-goals.** The screens behind Home's other controls, opening a version from the Versions explorer,
the Agent and Appearance panels, relationships and foreign keys in Properties, key editing beyond
key editing past name, columns and Nulls distinct, the drawing
tools' menus, folders and renaming in the Diagrams tree: their own rows.
