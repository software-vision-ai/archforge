# archforge-ui-egui

**Purpose.** The user interface: Home, the start screen, and the New project dialog over it,
drawn from the design with its own palette, type and icons (`ux`).

**Toolkit.** The only library crate that may use the toolkit (egui).

**Binding rules.** The shell rules, the localisation rules (`i18n/`), and the layering rule that
the shell never touches the file system: the host injects `Services`.

**Public API.**

| Item | Signature |
|---|---|
| `Shell::new` | `pub fn new(services: Services) -> Shell` |
| `Shell::logic` | `pub fn logic(&mut self, ctx: &egui::Context)` (once per frame, before drawing) |
| `Shell::ui` | `pub fn ui(&mut self, ui: &mut egui::Ui)` (draws the whole window) |
| `Shell::request_quit` | `pub fn request_quit(&mut self)` (closes the window at the next frame) |
| `Shell::widgets` | `pub fn widgets(&self) -> &[widgets::Entry]` (the controls of the last frame, by automation id) |
| `Shell::top_inset` | `pub top_inset: f32`, a field: the logical pixels at the top the host draws over (the macOS title bar) |
| `Services` | host functions: the installed fonts, the account's name, the folder new projects go in, the projects host |
| `ProjectHost` | `fn create(&self, folder: &Path, name: &str) -> Receiver<Result<Opened, CreateError>>` (the host creates on its own thread) |
| `Opened`, `CreateError` | an open project; why a creation failed, in the kinds the dialog explains |
| `UiState` | the stored appearance, language and Home's closed banner |
| `i18n::{tr, tr_ctx, trn, set_language}` | catalogue lookups at draw time |
| `offscreen::render` | `pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String>` (feature `snapshot`) |
| `offscreen::render_screen` | the same for a `Screen`: Home, or the New project dialog over it |
| `new_project::{open, name_problem, file_name}` | the New project dialog: open it, a name's problem in words, the file a name gives |
| `Folder` | a folder as the shell shows it: its path and its label |

**Non-goals.** The screens behind Home's controls: they come later.
