---
name: archforge-design
description: "Use before designing or changing how Archforge looks or behaves: a screen, a panel, a tool, an editor, a gesture, a notation, a message or an icon. Holds the design decisions taken so far, so a change keeps to them or knowingly replaces one."
---

# How Archforge looks and behaves

The decisions taken so far about the interface, as rules. The archforge-ui skill says how to draw
them. A change that departs from one of these is a new decision for the owner: say so in
the change, and update this file in the same change.

Every rule here holds for every notation (ER now, UML and C4 next) unless it sits under "ER
notation" at the end. Many were first decided while building ER and are worded with ER's boxes and
lines as the example: read "table" as any notation's box (a class, a container) and "relationship"
as any of its lines. A notation adds its own section for what is its own, and builds on the shared
parts (the shell, the canvas and its gestures, routing, arrange, appearance, notes, the explorers,
search, Properties' panel and editors, skills, messages and saving) instead of making its own.

## Our own style
- Archforge has its own look. Never copy how classic modelling tools look or behave; reimagine a
  control in Archforge's visual language and show it to the owner before it is final.
- Build what the design draws, with honest empty states and never sample data; the behaviour is
  wired in after the shell. Shell parts are drawn from the design's components, never invented.
- Only what works is shown. A canvas tool that is not built is left out of the toolbar; another
  control whose feature is not built does nothing and says "Not available yet" on hover.
- Prefer icons to words wherever an icon is clear; the words go into the tooltip. Context menus,
  headings, field labels and sentences keep words.
- One meaning, one icon: a tool and a handle that do the same thing share their icon.
- Judge an icon at 1x as well as on a retina screen: dashes keep their gaps open, and every tool
  icon is drawn at its natural size so the tools look the same size side by side.

## Look
- Icons are thin strokes at one weight, 1.2 pt, at a standard size per role (canvas tools about 18,
  rail and chrome 15-17, rows and fields 13-15, chevrons 8); canvas glyphs scale with zoom.
- Every single-row control, bar and list row centres its parts on one middle line.
- Floating surfaces (the toolbar, popovers, menus) carry a soft shadow instead of an outline;
  lists and menus are opaque, on a layer nothing paints over, and open above their field when
  there is no room below.
- Text is Inter. Menus, tabs, tool labels and section titles are Regular, never bold; only the
  heading that names the selection and table names on the canvas are heavier. Eyebrow labels are
  small semibold capitals.
- Colours are role tokens with a Light and a Dark value; every screen exists in both, and the
  title strip's moon or sun switches between them, kept in the preferences.
- The selection is accent blue in every notation; each notation gives its boxes a default colour,
  and a style set on one box wins.

## Shell
- The title strip is 32 pt tall, clear of the macOS window buttons, and holds Home, + Open project
  and the theme switch; dragging it moves the window and a double click zooms it.
- The brand at the bar's left is the app icon's beaver and the wordmark ArchForge: Arch in the
  text colour, Forge in the icon's blue (a lighter blue in Dark).
- The project bar holds the brand, Home, the project tabs, the search field (Ctrl K), help and the
  avatar. Projects open as tabs, up to eight, each with its name and a close button; showing Home
  closes none of them. In a narrow window the tabs shrink, then turn into icons. A project whose
  changes cannot be written does not close; it stays open and says why.
- Diagrams and skills open as tabs inside the project; the + that makes a new diagram sits in
  that tab strip.
- The left rail holds Diagrams, Models and Versions, in that order (owner, 2026-11-04: the
  diagrams are found first); the right rail, between the canvas and
  the panel, holds Chat, Appearance, Properties and Skills, each with a shape of its own (a
  proposal for the owner to confirm): a rounded speech bubble with two lines, a palette with
  three dots of paint, three sliders, and a page of rules with a checklist. Chat's panel is the
  project's conversations with an agent (section Chat).
- Clicking the open column's own rail icon folds it; any rail icon opens a folded column on its
  content; the panel's collapse button folds the panel. Folded columns are kept in the preferences.
- The status bar says only what is true: the save state, the agent state, the location.
- On macOS the menu bar is the table of actions (`crates/ui-egui/src/actions.rs`), and its items, keys
  and Home's search rows share each action's availability. On Windows and Linux the same actions are
  an in-window menu bar in the title strip (`crates/ui-egui/src/menu_bar.rs`, set on by
  `Shell::in_window_menu`): the groups of its own table (File, Edit, Model and Help for now, others join by a
  row), popups 300 wide of rows 30 high and dividers 9, the rows lower down to a line of text where a
  long menu would leave the window, each row off saying why on hover. Choosing
  a row runs at the next frame's start, Esc closes, a press elsewhere closes, and while a menu is
  open the frame's keys are held from the window behind, as under a dialog. F10 with no modifier
  opens the first menu on its first live row and closes it again (Shift+F10 stays with the row
  menus); Left and Right go between the groups and Up and Down through the live rows, all wrapping;
  Enter or Space runs the row; Esc gives the keyboard focus back; a moving pointer takes the highlight.
- The Model menu (`crates/ui-egui/src/model_menu.rs`) arms the toolbar's own tools, so a row makes
  nothing by itself: its rows are the open diagram's notation's, then Note and Link note. Its group
  stays with no diagram, its rows off, so the bar doesn't move; the native menu is rebuilt when the
  rows change. A row is off while no diagram shows or the model can't change.
- Outside a packaged app the window and the Dock show the Archforge icon; a packaged macOS app
  keeps its bundle's light and dark icon.

## Home and projects
- Home fills the window on its own, without toolbar or status bar; the only menu bar it shows is
  the in-window one in the title strip, on Windows and Linux. Its welcome has no button: a project opens from the Open a project card or the title strip's + Open project.
- Archforge looks for Dolt when it starts. Without a Dolt it can keep projects in, Home shows a
  card in the banner's place (2026-11-03): an amber database tile, "Install Dolt to keep your
  projects" and why, Install Dolt (Dolt's installation page, in the browser) and Look again; it has
  no close. With Dolt found it goes, and the banner is back. The New project dialog's Dolt failure
  links to the same page.
- A start card for what cannot be done at all yet (Explore an example, Import a model) is left
  off Home, and the cards left share the row.
- Recent projects keep the last twelve (name, folder, last opened) across restarts; a row opens
  its project, or shows its tab when it is open already. Each keeps the view it was left in:
  its diagram and skill tabs in order, the one shown, and the canvas's pan and zoom; the
  row says the folder and "Last view: {diagram}", and opening it again resumes that view, a tab
  whose diagram is gone left out. The view is the window's preference, never the model's: resuming
  it changes nothing and joins no history. The explorer and the panel, and whether they are folded,
  are the window's for every project, kept as they were left. A row or card under the pointer shows a
  More button whose menu offers Open and Remove from recent projects; removing leaves the project
  where it is, asks nothing, and opening it again lists it again.
- Pin a folder picks a folder with the system's picker and pins it on Home, up to eight, kept in
  the preferences. A click on a pinned folder opens the project picker in it; its menu offers
  Open a project here and Unpin.
- Home's search field (or the command key with K) opens a list under it: the recent projects and
  the projects in the pinned folders (two levels down, looked for each time it opens, one search
  at a time), up to eight projects in all, the pinned folders and Home's actions, each worded for the state it is in. Every word typed must
  be found; the arrows move, Enter runs, Esc or a click elsewhere closes.
- The recent projects show as rows or as a gallery of cards in the page's grid (four columns
  where the width takes them, so two start cards line up with them), sorted by last opened
  (newest first) or by name; both choices are kept in the preferences.
- One project holds one model, named as the project. The New project dialog asks for a name and a
  location only; a taken or unfinished name is said in plain words under the field. Change… picks
  the location with the system's folder picker, titled Choose project location (Home's
  Pin a folder keeps its own title), and shows it in the field; a cancel keeps the
  folder before it, and a picker that cannot appear says why under the field. What it says
  a project holds is only what is built: ER and UML class diagrams, skills, agent access, undo
  and saves; nothing still to come is promised.
- Open project uses the system's folder picker; a damaged, newer or foreign project is refused
  with a dialog, and the project open already stays as it was.
- The shortest way to drawing: + in the diagram tab strip, Enter, a name, Enter. New diagram has
  two steps: first a card for each kind Archforge makes (a picture, its name, what it shows; never
  a card for a kind still to come), then the chosen kind's name and schema or package, with Back
  (which keeps what was typed), Cancel and Create; it makes a schema or package when the project
  has none, and nothing before Create. A Diagrams group's + skips to the second step.
- New things get default names (Schema 1, Diagram 1, Entity, Entity 2) with the name open for
  typing in place.

- In a project the top bar's search (or the command key with K, unless a text is being typed)
  finds the project's diagrams and elements (schemas, tables, columns, relationships, domains,
  enum types, sequences, views, skills, notes) by name or by where they live; every word must
  match, the name itself first, then names that start so. Diagrams are listed first, then
  elements, each saying what it is and where. A diagram opens in its tab; an element is selected,
  its row shows in the Models tree, and the canvas brings it to the centre on the open diagram,
  or on the first diagram that shows it, which opens.

- The bar's avatar shows the initials of the name entered in Settings (owner, 2026-11-01): the
  first and last words' first letters, or the first two letters of a single word; with no name,
  JD for John Doe. Archforge never reads the name from the operating system's account. The avatar
  and the command key with comma open Settings (see Settings and AI Agents below); Profile holds
  Your name, with the initials beside it changing as it is typed; Done, the close button, Enter in
  the name or Esc close it.
- The bar's Help button (and, on macOS, Archforge ▸ About Archforge…) opens About Archforge:
  what Archforge is, and Support Archforge with GitHub Sponsors and Ko-fi, each opening
  its provider's page in the browser. Only an approved HTTPS address of the provider opens; one
  the host has not set up says Not set up yet and takes no click. It says a contribution unlocks
  nothing; it never opens by itself, and it is modal, so Tab stays in it and nothing behind moves.

## Explorer
- Models shows the model's elements, a notation's own containers first; diagrams appear only in the
  Diagrams explorer.
- The Diagrams explorer groups diagrams by kind (Class diagrams, ER diagrams), a group for each kind
  Archforge supports, empty or not, with its count and a + that opens New diagram on that kind. A
  group is navigation alone: the model keeps each diagram once, where it was. A diagram's hover
  text and the footer say where its package or schema sits (project / package / subpackage). Never
  show a group for a kind that can't be made yet.
- Models starts folded: only the top-level containers (an ER model's schemas) show, and the user drills in. The button beside + opens
  every row (Expand all), or folds them all (Collapse all) once everything is open; a filter opens
  what it shows, and the button rests while one is typed. An element selected on the canvas or in
  Properties opens its schema and table once, so its row shows; folding them again sticks.
- The + adds what fits the selection, named in place: Enter confirms, Esc cancels.
- Dragging an element from Models onto the canvas places it; one already there is refused with a
  toast.
- Deleting from the explorer asks first when the element is on diagrams or takes other elements
  with it (for ER, keys).
- The Models and Diagrams trees scroll under their filter (wheel or trackpad, a thin thumb at the
  edge while they overflow); a row newly selected, on the canvas, in Properties or with the keys,
  is scrolled into view once, so the tree never fights the wheel.

## Canvas and tools
- The tools float at the bottom centre of the canvas: Select, Pan | the diagram kind's own tools
  (for ER: Entity, Relationship with its chevron) | Note, Link note | the kind's view toggles (for
  ER: Key status) and Grid | Arrange diagram. The toolbar is never hidden;
  the minimap folds to its button when both do not fit.
- Arrange diagram (owner, 2026-11-01) lays the whole diagram out in layers along its lines: the
  box a line starts at above the one it ends at, each layer ordered so lines cross
  little, groups with no line between them side by side, boxes with no line in rows below. It
  keeps the diagram's top left, drops dragged line routes, is one undo step, and shows the whole
  diagram after.
- The line under the pointer turns to the accent with a soft halo along its whole route, so it
  reads among the others.
- A line tool's button is double width, and its icon shows the two ends its next line will get;
  its chevron opens the kinds and ends it draws, and the choice stays for the next line.
- A placing or drawing tool hands back to Select after it has made one thing.
- Selecting is by click, Shift-click and marquee; Edit ▸ Select all (⌘A) with the canvas holding the
  keyboard takes what the open diagram draws, boxes nested in others included, while viewing a
  version as well, never while the project is held. A selected box shows handles and buttons beside
  it: connect, and the notation's add (for ER, add column). Dragging moves the whole selection as
  one undo step; arrow keys nudge. Two or more selected boxes show an arrange bar under them.
- Delete removes a box from the diagram only; Shift+Delete deletes its element from the model after
  a dialog that names what goes with it.
- Double-clicking a box's name renames it in place; double-clicking a member (a column, an
  attribute) opens its editor in Properties.
- Related boxes are shown by a focus chip above the canvas, the others faded, never as a drawn
  region.
- A box of another container (an ER table of another schema, a UML class of another package) is
  drawn faded with a dashed border and its container in its header, and stays editable.
- The minimap sits at the canvas's right edge; the zoom control is in its footer. Its viewport
  rectangle is dragged to move the view (a hand over it, a closed hand while dragging); a press
  elsewhere in the map centres the view there and the drag goes on from there. The map keeps its
  scale while a drag lasts, so it never shifts under the pointer.

## Lines
- Lines are drawn by dragging from box to box, never click by click. A line dragged from a box and
  dropped on empty canvas makes a new box there, joined by the line, as one undo step, the new
  box's name open for typing.
- A line's end texts (multiplicities, cardinalities) show beside the line near each end: under a
  level line (its name sits above), right of an upright one. A text is dragged anywhere and stays
  there relative to its end, as the line's label; a double click puts it back.
- A right-angled line's segment is dragged across (the pointer shows which way): a middle one
  slides, the first or last slides where the line meets its box. The line keeps its route from
  then on; a double click on it routes it anew.
- A generalization is a solid line with a hollow triangle at the general end, in every notation.
- An existing line's ends change in its tool's popover or in Properties.

## Notes
- The Note tool places a note on an amber tint, typed in place.
- A note links to a box or to a line with a thin dashed straight line; a link to a
  relationship ends on its line with a dot. A link is drawn from the selected note's handle or
  with the Link note tool, whose icon is that dashed line: press on a note, drag onto a box or a
  line. A link belongs to the diagram, can be selected and deleted on its own, and goes with what
  it joins.

## Properties and editors
- Properties shows model data, the same on every diagram; Appearance styles this diagram only;
  Skills is a tab of its own.
- The panel opens with the selection's name, its kind and a Description field, pinned while the
  body scrolls. Enter, leaving the field or selecting something else keeps the description on the
  element it was typed on, an empty field removes it; a UML
  element's is a comment it owns (AR-186), recognised by its shape: an owned comment annotating its
  owner alone, so such a comment from a file counts as the description (the first one; setting
  removes any others, clearing removes them all). An association's end takes none. A box's element and its members (a table and its columns, a class and its
  attributes) share one page, so moving between them keeps the scroll.
- Sections are divided by full-width lines, each headed by a regular title with its count
  ("Columns · 3") and an icon action such as +. Fields are filled and borderless with small muted
  labels above them. The panel carries no help or footer texts.
- Every list row (columns, keys, checks, indexes, types, sequences, views; attributes, operations) has an edit button that
  is always visible at its right end. Only that button, or a double click on the row, opens the
  row's editor. Selecting an element never opens an editor.
- A click on a row selects its element, so the Models tree and the canvas follow; selecting in the
  tree or on the canvas marks the row in Properties. The selection is always in step both ways.
- Selecting anything else closes open editors as Cancel would; coming back finds them closed.
- An editor commits nothing until Apply. Enter applies, Esc cancels, Apply is one undo step and
  closes the editor; a refusal shows under its field and keeps the editor open. Editor actions are
  icons: a red bin for Remove, a cross for Cancel, a check for Apply.
- A type menu lists the model's own types nearest the element first (for ER the schema's domains
  and enum types), then the built-in types.

## Appearance
- Appearance styles the selection on the diagram open in the centre, and on that diagram only.
  For now it holds Style (owner, 2026-10-31): a box (a table, a class), a note or another card takes Fill, Border
  and Text colour and a Border width; a line takes Line colour, Line width and Line routing. Text
  size comes later with its own model field; a line style is never offered, since dashed already
  means not identifying.
- Line routing (owner, 2026-11-01): Straight, Right-angled or Curved, the design's routing icons in
  a row of three under Line width. Route around tables, a switch under the row, shows only for
  Right-angled. The diagram sets how its lines run and a line may run its own way; with nothing
  selected the panel shows the diagram's routing. A new diagram's lines run right-angled around
  the boxes; a diagram made before keeps straight lines until changed.
- A right-angled line leaves and enters a box at right angles to its side; around boxes it keeps a
  margin from every box near it and takes the way with the fewest bends. A box's lines spread over
  the middle of each side, ordered by where their other box lies, so they leave side by side.
  Moving a box re-routes its lines as it moves.
- Each colour row starts with the diagram's own colour, drawn in the theme shown, which takes the
  row's own colour away; then the design's colours; then Your own colour, which opens a picker
  (a saturation and brightness field, a hue bar, the colour's code, Cancel and Apply). Nothing
  changes until Apply; Esc or a click outside cancels.
- A box or note with its own fill and no text colour of its own draws its text to read on that
  fill: dark on a light fill, light on a dark one, in either theme.
- Use diagram defaults takes the selection's own look away. Every change is one undo step for
  the whole selection.

## Skills
- A skill's text is Markdown, and it is written in the skill's own tab in the centre, beside the
  diagrams. The page shows the skill's name, where it sits, a formatting toolbar (bold, italic,
  heading, bulleted list, numbered list, code, link, quote) and a Write / Preview switch drawn as
  a pencil and an eye, their words on hover.
- Write edits the Markdown source itself, styled as it is typed: markers dimmed, headings larger,
  strong text semibold, emphasis slanted, code in purple, quotes and links in their colours. What
  is typed is what is kept. Preview draws the page the text makes.
- A toolbar button wraps the selection (bold, italic, code, link) or marks every line it touches
  (lists, quote); pressed again it takes the mark away. Heading goes one level deeper on each press,
  to three, then back to text. Cmd B, Cmd I and Cmd K work while writing.
- The text is kept a moment after typing pauses, when the tab closes, on Save and on quitting,
  each time one undo step. A text the model refuses keeps its tab, and its project, open, saying
  why.
- A link in Preview opens only after a confirmation that names its site and shows the whole
  address, and only when it is a web or mail address. Pictures in a skill are never loaded. A
  text too long or nested too deeply is shown plain in Write and not rendered in Preview.
- The Skills panel lists the skills: a click on a row opens the skill's tab; the bin that shows on
  a hovered row of the element's own skills removes it without opening it (undo brings it back);
  + makes a skill on the selected element and opens its tab with the name selected, ready to type.
  The + shows only where the model lets the selection carry a skill (the shell asks the same
  rule `skill.add` does); elsewhere a muted line under the section's title says so in the
  selection's words: "Relationships carry no skills", "Literals carry no skills", else "This
  element carries no skills" (AR-121).
- A new skill's text starts from a template shaped as an agent's skill, in the interface's
  language: a header between `---` lines with `name` (the name as a slug) and a `description`
  that says when to read it, the name as the `#` title, a line on what it is for, then Rules and
  Examples, each with a hint to write over. A rename carries into the header's name and the
  title while they still match the old name. Write shows the header in a quiet monospace face;
  Preview leaves it out and shows its description as the lead.
- A skill's tab holds, top to bottom: the name as the page's title (Enter or leaving the field
  keeps it, Esc puts it back) with the bin at its right, where it sits, then the toolbar and the
  text. Where a skill applies and when are not shown for now (owner, 2026-10-31): skills will be
  used in contexts still to come, and a fixed list such as Export misleads. The model keeps those
  fields; they come back when the contexts are known.
- Generation is an agent's, never the window's (owner): File → Generate… opens a handoff
  for the selected schema, table, class or diagram (else the open diagram): From, Output (SQL
  DDL with its dialect, which the export skills in effect suggest, or documentation), the export
  skills in effect with where each comes from and what overrides it, and the request to copy for
  the agent, naming the MCP resources it reads (`archforge://element/{id}`,
  `archforge://skills/effective/{id}/export`) and the model's revision. Its status line says
  whether the person turned agent access on for this start (never Chat's), that the request was
  copied, and when the model changed after; nothing in it shows output or claims the agent did anything. The dialog changes nothing.

## Settings and AI Agents
- Settings is an 880-wide modal dialog (owner, 2026-11-06, AR-204): its title and close button on
  top, its sections listed on the left as nav items with an icon (Profile with the person, AI
  Agents with the agent mark), the section's page on the right, scrolling when the window is
  short, and Done alone in the footer. A choice takes effect as it is made; Tab stays in the
  dialog and the window behind takes no keys.
- AI Agents (page 07 of the design, frames A1-A8): the default agent select on top, a card per
  agent (the neutral hexagon agent mark beside its name, never a provider's logo; a status badge
  at the card's right), then Check connection and Send test message, each with its note beside
  it, and the status badge under them.
- Archforge never touches the system's credential store at launch, so it never asks for access
  to it unless agents are used (owner, 2026-11-06): only the programs are looked for when it
  starts. The keys are read, on the host's thread, the first time the AI Agents page (or Chat)
  needs them, and kept until Look again, Check connection, a save or a removal, or a restart;
  that keeping is the keys' alone, Codex's account is not kept that way. Codex's account is read
  when a Codex conversation first starts, and again each time the AI Agents page shows, on Check
  connection and on a Codex conversation's "Sign-in expired" card (AR-222); never at launch
  (owner, 2026-11-06), as reading it starts the codex program; a plain launch runs only the
  programs' version look.
  Opening the page sends nothing over the network; those first local reads are all it does. Check connection is free
  and local (the programs, their versions, the keys or sign-in); Send test message carries its
  cost note and goes to the default agent's provider. The badge says when it was checked as a
  time ago ("Checked just now", "5 min ago"), not a clock time, as the window has no time zone.
- Claude uses an API key only: typed into a hidden field, saved to the system's secure store and
  nowhere else, then shown masked to its last four characters with Replace and Remove. Its model
  select waits until a key is saved. A key the provider refused is outlined in the danger colour
  with why under it; the refusal comes from a test message, since Check connection sends nothing.
- Codex has a segmented control, Sign in with ChatGPT or API key. The codex program keeps the
  ChatGPT sign-in itself; Archforge starts it, shows its device code inline (the code with Copy;
  under it the sign-in page's address as selectable text with Copy, only on the sign-in host, else
  "Not an address Archforge opens" in the danger colour (AR-223, decided by UX); Open sign-in page,
  the wait and Cancel) and shows who is signed in with Sign out. A sign-in not
  finished within 15 minutes, or cancelled by the codex program, ends with a line saying so and
  Sign in again (AR-203; the design draws no such state, decided by UX); Cancel ends it silently.
  Who is signed in is read again each time the page opens, on Check connection and on a Codex
  conversation's "Sign-in expired" card. It shows expired once the codex program reads it so or a
  Codex conversation finds it so (that card's word stands until a sign-in or a sign-out); until
  then it may still say signed in. An expired sign-in the codex program names no one for keeps
  the email known before, "The ChatGPT sign-in for {email} has expired." (AR-222). On an API key
  the saved key shows as a row (key icon, "ends in" its last four) with Replace and Remove.
- A missing program shows why, the maker's install command with Copy, Look again (primary) and
  Installation guide; a program newer than tested is accepted with an amber badge, "Not yet
  tested with this version".
- Without a secure store (Linux without a Secret Service) an amber notice says keys can't be
  saved and why in the system's words, the key fields and Codex's API key way are off, and
  nothing falls back; ChatGPT sign-in still works. The store's answer holds until Archforge
  restarts, so the notice says to start one and restart, and offers no Look again (a departure
  from frame A7, which could not keep its promise). On Windows the page says AI Agents are not available on this system yet.
- What is kept in the preferences: the default agent, each agent's model and Codex's way of
  signing in. Never a key, nor any part of one. There is no switch for Chat (owner, AR-237): Chat
  is ready when the default agent is; an earlier build's switch is skipped when read.
- The codex program keeps a ChatGPT sign-in in its own files in Archforge's private `CODEX_HOME`
  (`agents/codex-home`, mode 0700), on every system, never in the system keychain or Secret
  Service (owner, 2026-11-08, AR-236; it replaces the 2026-11-06 rule "the keyring where the
  secure store opens"): the codex program runs with a private `HOME`, where no keychain is
  found. The note "keeps its ChatGPT sign-in in its own files" shows once Codex is signed in and
  only where no secure store opened, as there it tells why the sign-in works without one.
- Agent access for Chat (AR-203) opens only while a conversation holds it, and closes with the
  last conversation and on quit; asking a conversation to end revokes its token at once. With no
  switch for Chat (AR-237), asking a question is the opt-in: a conversation's program and its
  token start only when the person sends one in it, read-only unless Allow changes. The
  status bar's Agent item (page 07, the dot and text beside the location) says which: an accent
  dot and "Chat is using the model" while a conversation holds access, a purple dot and
  "Agent access off" otherwise, one at a time. It is a control (pointer, Tab, Enter, its id
  `workspace.status.agent`); either state opens Settings on AI Agents, as the design's agent setup
  overlay does.

## Chat
- The Chat panel (page 07, frames B1 to B7, AR-205) is built from the design's Chat components:
  question bubbles in the accent tint, the agent's name over its turn, tool rows (an eye for a
  read, a pencil for a change), replies at 13 on 19, the green card of a turn's changes, the amber
  or danger notice of a failure, the context chips, the composer with the provider badge and Send
  (Stop while a reply runs), the read-only switch, and the history. Its header holds the history
  and New conversation (New is hidden while no agent is set up).
- No agent set up (B1) shows until the default agent is ready; Configure agent opens Settings on
  AI Agents. While the keys are first read it draws nothing.
- Opening the panel the first time in a project starts a new conversation (B2); it stays while
  the project is open. One read back from the history opens read-only: Allow changes is off for
  every conversation until turned on in it. Choosing in the history the conversation already
  shown does the same: Allow changes goes off as when turned off by hand, its program ends, and
  the next question starts it again read-only (AR-226, decided by UX).
- A question is kept in the project before the agent gets it; the composer empties only then. A
  refusal (a saved key in the question, a project that cannot be written, a question over 64 KiB)
  keeps the draft and says why in the danger colour above the chips. Enter sends, Shift+Enter
  starts a line; the composer grows from two lines to six, then scrolls. Send is drawn ready, as
  the design has it, and sends nothing while the composer is empty.
- While a reply runs, typing works; Enter, the badge, the switch, New conversation and the history
  wait and say "Stop the reply first".
- The context chips (the project, the open diagram, the selection; several selected elements are
  one chip, "{n} elements") go with a question as a line after it, by name and id; a removed chip
  stays removed for that conversation.
- A tool row is worded from what Archforge's control channel answered for the conversation's
  origin: Read, Changed, Added or Removed and what it was about, with what was read or the aspect
  changed as its detail; a refused or failed call keeps its words with "refused" or "failed" and an
  amber mark. Looking up commands and the window's controls makes no row. A request from a
  conversation's token runs only while that conversation's turn runs in the project shown, never
  while it still runs or ends elsewhere; the window knows a conversation's request by the key the
  control channel bound to its token, never by a label (AR-230), so another project's conversation
  of the same label neither stands in for it nor blocks it, and a conversation's token the window
  does not know may not act under a label it knows (one whose label it does not know either is let
  through, as no such token is live). The person's own token is known by the label it names. It
  undoes or redoes only a step of the turn running, never the person's, another conversation's or
  one of its own earlier turns; while its turn runs, its own undo takes back the turn's latest step
  so far.
- A turn's changes are one step of the project's undo history, `Chat N: Turn n`, unless another
  change (the person's or another conversation's) lands while the turn runs: that ends the step,
  the turn's next change starts another, and the turn is several steps. Their card follows the
  turn's last tool row. Undo this turn's changes is offered while a turn of one step is on top of
  the history; otherwise the card says which (the model was changed during the turn, or after it,
  the undo history no longer holds it, or the project was closed since). Undone, the card says so,
  in one step only for a turn of one, and redo brings it back.
- Stop keeps the reply so far, marked cut off ("Stopped. Nothing was changed.", or "Stopped." over
  the changes' card); a program that has not stopped within ten seconds is ended, and the turn
  with it. Send again runs a failed turn again in place, its question kept once and its chips sent
  again; a turn that changed the model is not sent again. After a lost connection before any
  answer the question goes back to the composer when that is empty, its bubble hidden, and Send
  again sends what the composer holds; otherwise it stays in its bubble.
- A running turn whose program reports it retries the provider shows, at the end of the turn, an
  amber look-again icon and "No answer from {agent}'s provider yet. Trying again, attempt {n} of
  {max}…" (the attempt and the most only as far as the program says them), with Stop as ever; any
  text, tool call or end from the provider takes it away (AR-224, decided by UX).
- Failures the design does not draw: a missing program, a refused key, no key and an expired
  sign-in are danger cards with Open Settings; a start refused while the conversation's folder is
  being removed is an amber "Not started yet", "This conversation is being removed. Try again in
  a moment.", with Send again (AR-225, decided by UX); anything else is an amber "No answer" with
  Send again. A rate limit without a reset time names none. Model not available offers Send again once
  another model is chosen in the badge.
- The provider badge names the agent and the model asked for ("Claude · Sonnet"), the version once
  the program names it ("Claude · Sonnet 4.5"); the program's default shows the agent alone until
  then. Its menu lists the conversation's agent's models, both agents' before the first question.
  A conversation that has not started follows Settings' default; once started its agent and model
  change only in its badge, never because Settings changed. A changed model or switch ends the
  idle program, and the next question starts it again where it was.
- The history lists the project's conversations newest first under "Saved with {project}": the
  title, then when, the agent and how many messages; the open one is marked. Times use the local
  offset the host gives ("Today 14:02", "Yesterday", "3 Nov"), months short and translated; without
  an offset a time says UTC.
- A reply's links open only through the link confirmation, and only a web address with a host and
  no user part; an image shows its alt text, HTML its plain text.
- Leaving a project's tab stops its running turn; closing the project ends its conversation.
- Deleting a conversation (B7a to B7c, AR-217): the bin (quiet icon button, the bin icon) shows on
  the row the pointer is on, or the keyboard is on while focus marks show, and the title shortens
  for it; its tooltip and accessible name are "Delete conversation", the tooltip with ⌫ (Delete off
  macOS). The row's menu (right-click at the pointer, flipped left near the window's edge; the menu
  key or ⇧F10 under the row) holds Open ↵ (Enter off macOS), a line, Delete conversation ⌫. ⌫ or
  Delete on the focused row asks as the bin does. The confirmation, 440 wide on the scrim: "Delete
  this conversation?", then "{title} · {n} messages with {agent}, {age}" (age "today",
  "yesterday" or "3 Nov"), "It is removed from this project's conversation list." and "It stays in
  the project's saved history: anyone with a copy of the project can still find it there.", and
  "Its reply in progress stops first." while it runs; Cancel and Delete (filled danger), the focus
  starting on Cancel, Escape cancelling with the focus back on the row. Delete stops a running
  turn first (its reply so far kept), opens the next row's conversation when it was open (else the
  one above, else a new one, B2), and moves the focus to the next row, else the one above, else
  New conversation. The toast at the centre's foot, clear of the minimap, surface-coloured text on
  an ink fill: "{title} deleted · still in the project's saved history" with Undo (the hover accent),
  eight seconds after the pointer and the focus last left it; Undo is in the tab order. Undo
  brings it back in its place, with the keyboard on its row. ⌘Z never deletes or restores one.
  The private folder of its program goes when the toast closes without Undo, when Undo fails,
  when its project closes or when Archforge quits; a copy of the project open beside it showing
  that conversation keeps it. While the folder is being removed the conversation does not start:
  its turn says "Not started yet" (AR-225), the folder is kept for it, and Send again starts it once the removal
  is done. While a history row's menu or the confirmation is open, the canvas and ⌘Z take no keys.
  While the list cannot be read again (chat writing off) the rows read before stay, and the bin
  and the item are disabled saying "The
  conversations in this project cannot be changed: {why}"; an unreadable row's bin says "This
  conversation cannot be read". Failures say "This conversation could not be deleted: {why}" or
  "… could not be brought back: {why}" under the history. While a toast is up, a new
  conversation never takes the deleted one's label.
- Focus rings show only while the keyboard was used last: a key pressed shows them, a pointer
  pressed hides them (AR-217).

## Messages
- Problems show as an orange badge on the box or line and in a Problems section of Properties,
  whose rows select the element.
- A fact the model's target cannot enforce (for ER, the database; say a one-or-more end) is a
  notice, never a refusal. It says in the model's words what is wrong, why, and which choice
  avoids it.
- A refusal shows where the action happened: under the field, under the box's name or under the
  tree. Removals that take other things with them show a toast with Undo. The toast keeps clear
  of the minimap: at the canvas's foot, left of the minimap while there is room beside it,
  else above it; its text wraps to three lines rather than running under Undo.
- A step between versions or a way out of current work that is refused or fails says why in a
  raised card with an amber dot, centred over the toolbar and clear of the minimap and of a toast
  with Undo (frame V2.4); it wraps and grows upwards, takes no clicks, and goes with the next
  action. The notices of drawing and editing stay the amber caption at the canvas's top, and the
  details, Compare and a change keep a refused step as a caption under their way back.

## Saving
- Every edit is written to the project on its own a moment after it settles, so a crash loses
  nothing. Save makes a version; "Unsaved changes" means edits not yet in a version.
- Every edit is a command that can be undone; a drag, an Apply or an arrange is one step.
- Unsaved changes show as a dot before the project's name, in the window title and on its tab.
- Closing a project with unsaved changes, or quitting, asks "Save the changes to {name}?":
  Save (primary, right), Cancel, and Don't save apart on the left (owner decision). Save closes
  once the save is done; Don't save puts the project back to its last save. Quitting asks about
  each unsaved project in turn, showing it; Cancel stops quitting.
- A project whose changes cannot be written stays open and says why; closing it again offers
  Keep open or Close without writing, and Close without writing asks once more ("Close and lose
  changes", in the danger colour) before the changes not yet written are lost; what was written
  stays, unsaved, for the next opening (owner decision). Save waits for a save already running and
  asks again when edits came in meanwhile.

## ER notation
What is ER's own, beside the shared rules above.
- Tables are orange by default, and a style set on one table wins.
- Models starts at the schemas: schema, then its domains, enum types, sequences and views, then
  its tables and their columns.
- Key marks (PK, FK, FKn) sit in a column at the left of each table row; a column that follows
  another column shows a link mark whose tooltip names the column it follows.
- Crow's foot. Each end draws two marks: the outer one the most (a bar for one, a foot for many),
  the inner one the fewest (a circle for zero, a bar for one). Exactly one is two bars.
- An identifying relationship's line is solid; a non-identifying one is dashed. With Key status
  on, a line is green when its foreign key is on and orange when it is off.
- Drawing a relationship never blocks: without a parent key, or when either table has no columns
  at all, the line is drawn with its foreign key off; one dropped on empty canvas makes its new
  table with the foreign key off.
- Cardinality is chosen in one place, the popover from the Relationship tool's chevron, with
  glyphs, not words.
- Key columns a foreign key adds grow every box of their table to show them, in the same undo
  step; deleting a relationship takes its foreign key and those key columns with it. A project
  opens with every table box at least tall enough for its columns.
- Many-to-many offers a mapping table; a mapping table that holds only its two keys offers to
  collapse back into a line.
- ER has no generalization tool. Subtypes are added from the supertype's Properties.
- Foreign keys come only from drawing relationships; a table's Properties has no foreign key
  section.
- Ticking Primary key ticks Not null, and unticking Not null unticks Primary key. A parent end's
  cardinality and its key columns' nullability move together.
- A linked column follows the name and type of the column it points at; any direct edit unlinks
  it, and the editor offers Unlink.
- A line's own colour shows while Key status is off; while it is on, the panel offers Show line
  colours, which turns it off. A line's own width shows always.
- Names are told apart as SQL does, compared without case (AR-209): a schema is named once in the
  project; tables, domains, enum types, sequences and views share one name per schema; a column is
  named once in its table, a relationship once in its schema, a domain's check once in its domain;
  keys, foreign keys, checks and indexes share one constraint name per schema. Elements of
  different kinds may share a name (a schema and its table, a table and its column). A clash is
  refused ("there is a schema named shop already", "two checks are named positive").
  `archforge_er::names::kind` is the one rule the project's check as it opens asks, and the
  commands keep it, so a project the commands made always opens again.

## UML notation
What is UML's own, beside the shared rules above.
- A class diagram is of a package. New diagram offers a Class diagram card beside the ER diagram
  card; its second step has Package, with New package as ER has New schema.
- Every UML model has one root package (owner, 2026-11-04): a plain package named after the
  project, marked with the tag `archforge.root`, the top row of Models. It holds what has no other
  home: a package or class added without one, the first class diagram (the Package field names the
  root while it is still to be made, and Create makes it). It cannot be moved or removed; removing
  a package hands its contents up, in the end into the root. A project opened without one gets it,
  its top-level packages moved in, as one undoable step.
- Properties' Owner (a classifier's, AR-132) and Parent package selects show a package by its path from the root,
  `Commerce platform › Orders`, every ancestor named however deep. Too long for the field, the
  path's front folds into `…` (`… › Orders`) so the package's own name shows; a name too long alone
  is cut at its end, as any name is. The whole path is the select's tooltip and accessible name.
  Their menus are as wide as the longest path needs inside the window, and a row still too long
  folds from its front the same way, the whole path its tooltip; so does a type menu's note that
  tells same-named types apart by their package (AR-191).
- A package on a class diagram (AR-95, owner decisions 2026-11-04) is a tabbed card. Its tab
  (AR-113) holds the name in 12 px SemiBold, 22 px high with 12 px padding, never narrower than
  56 px nor wider than the card (a longer name ends in "…", the tab's hover shows it whole); it
  follows a rename as it is typed, and the rename field is the tab. The tab is part of the card for
  click, drag and double click. The card's body is a thumbnail of the package's own diagram on the
  page tint, 10 px in from the card, fitted and enlarged at most 50 %: mini cards with a name bar,
  a header line and a bar per row, nested packages with a tab, notes amber, and the lines; "Empty
  diagram" when it shows nothing; below 50 % zoom the name centred instead. Only a package the
  diagram's package holds goes on it. Every package placed on a canvas has its own class diagram,
  made with it in one undo step (an existing one is reused); renaming the package renames a diagram
  still named like it. Selected, its card shows Open; ⌘↓ opens it too, and zooming in until it
  covers 60 % of the canvas does; each opens the package's diagram at 100 %, its content centred
  (AR-110). Zooming out stays in a package's diagram down to the least zoom, 10 %; reaching it
  opens the parent's diagram at 100 %, the package left centred; at the root, 10 % stays put with a
  notice ("This is the top package: zooming out goes no further"). The path bar at the canvas's top
  left (root to here, ancestors as buttons) and ⌘↑ go back to the parent as it was left. One level
  per gesture: a switch waits for the zoom to rest 0.3 s and for 0.6 s to pass since the last; a
  switch fades in over 0.2 s. A double click renames, never navigates.
- A package moves (AR-102) by dragging its card onto another package's card (ringed, with a
  "Move … into …" tag while held) or onto an ancestor in the path bar, its row in Models onto
  another package's, or Properties' Parent package (the keyboard's way; never itself or one it
  holds). It goes with what it holds and its own diagram, onto its new home's diagram, its card
  leaving diagrams of a package that no longer holds it; one undo step, with a toast offering Undo.
  Into itself or a package it holds is refused with the reason; Models rings such a row in amber.
- A class is a plain card: the surface's white with the plain border, its eyebrow CLASS, its name,
  then two compartments under dividing lines, attributes and operations, each shown even when
  empty. Every compartment is padded as the card is: its first row as far under its line as under
  the header's, its last as far over the next line as over the card's foot (AR-191), so a new
  class is 240 × 146. An abstract class's name, and an abstract operation, are slanted; a static
  member is underlined.
- An enumeration (AR-121) is the classifier tool's third kind, after Interface, its face and menu
  icon a short list (Tool · Enumeration), placed and named in place as `Enumeration 1`. Its card is
  plain, `«enumeration»` in its eyebrow kept as written, and has one compartment: its literals in
  their order, bare names, one empty row while it has none. It takes no attribute or operation, and
  ⇧⌘↩ adds nothing to it. Properties shows Name and Package, then `Literals · n`, or "No literals
  yet"; Models lists it under its package with its literals below it, and its `+` offers
  Enumeration beside Class and Interface.
- Literals are edited (AR-122) as a class's members are. A selected enumeration's card shows a `+`
  at the right of its Literals compartment, even while it has none; it, the second adorner (Add
  literal), the menu's Add literal and ⌘↩ open a draft row at the end, ringed in the accent, with
  the name alone and its ⋯ to Properties. Enter writes the literal as one undo step and opens the
  next draft; Escape drops the draft; a click away writes a draft with a name and drops an empty
  one. A double click on a literal's row edits it in place there. Names are told apart within their
  enumeration whatever the case (`Paid` and `paid` clash): "There is a literal named Paid already."
  sits under the row, the text kept and the model untouched. Properties' `Literals · n` has a `+`
  and a pencil per row (a double click opens it too); the literal editor holds Name, then Remove,
  Move up and Move down at its foot on the left, Cancel and Apply on the right; Apply or Enter
  writes one undo step, Cancel or Escape leaves. A literal applied grows every card of its
  enumeration that would hide it, in the same step. In Models a literal's row renames in place (a
  second click, F2) and Delete removes it; `+` on a selected enumeration or literal offers Literal
  first, added at the end and named in place (`Literal 1`); a clashing name there is refused in
  the same words. An enumeration holds at most 256 literals, as an ER enum type does ("An
  enumeration holds at most 256 literals."). A card lists at most 500 rows and counts the rest in
  its `+ n more` row; on such a card a new member opens in Properties' editor, not as a draft row.
  Literals carry no skills.
- An enumeration is a data type: no association of any kind starts or ends at one, nor does an
  association's end move onto one; the tool says "An enumeration is a data type and joins no
  association" at the canvas's top and draws nothing. An enumeration specializes only an
  enumeration and only an enumeration specializes one; a generalization dragged out of one onto
  empty canvas makes an enumeration. A dependency may touch it; no realization starts or ends at
  one.
- A data type (AR-123) is the classifier tool's fourth kind, after Enumeration, its face and menu
  icon a pair of braces (Tool · Data type), placed and named in place as `DataType 1`. It is UML's
  DataType itself, never an enumeration or a primitive type, though the metamodel makes those data
  types too: each kind is told by its own metaclass. A skill for every data type reaches neither:
  an element is aimed at by its most specific kind, and a primitive type is a skill kind of its own
  (AR-124), so Integer, String and the model's own primitive types stay out.
  Its card is plain, `«dataType»` in its eyebrow kept as written, over attributes and operations
  edited on the card and in Properties as a class's are (draft rows, ⌘↩ and ⇧⌘↩, the adorner, the
  menu's row), any visibility allowed. Properties shows Name, Package and Description, then
  `Attributes · n` and `Operations · n`, with no Abstract; Models lists it under its package with
  its members below, its `+` offers Data type after Enumeration, and it drags onto a diagram or a
  package. It is offered once in every type menu and shares its package's names with the classes,
  interfaces and enumerations there.
- A data type joins no association, as an enumeration does not: the tool says "A data type joins no
  association" at the canvas's top (an enumeration keeps its own "An enumeration is a data type and
  joins no association"). A data type specializes only a data type and only a data type specializes
  one ("A data type specializes only a data type", "Only a data type specializes a data type"); a
  generalization dragged out of one onto empty canvas makes a data type. Between a data type and an
  enumeration the enumeration's rule is said. No realization starts at one, since only a class
  implements; a dependency may touch it. It carries skills, aimed at as "every data type".
- A primitive type (AR-124) is the classifier tool's fifth kind, after Data type, its face and menu
  icon a plain box with a small ring at its middle (Tool · Primitive type), placed and named in
  place as `PrimitiveType 1`. It is UML's PrimitiveType, a value of the model's own with no members:
  its card is plain, `«primitive»` in its eyebrow over its name and no compartment, a header's
  height (240 × 66) when placed or dragged from Models. Nothing adds a member to it: no adorner but
  Connect, no compartment `+`, no menu row, and ⌘↩, ⇧⌘↩ and a double click open nothing. Properties
  shows Name, Package and Description and no member section; Models lists it under its package
  with nothing below it, its `+` offers Primitive type after Data type, and it drags onto a diagram
  or a package. It shares its package's names with the other classifiers. It joins no association
  ("A primitive type joins no association"), specializes only a primitive type and only a
  primitive type specializes one ("A primitive type specializes only a primitive type", "Only a
  primitive type specializes a primitive type"), a generalization dragged out of one onto empty
  canvas makes a primitive type; no realization starts at one; a dependency may touch it. It
  carries skills, aimed at as "every primitive type".
- The built-in types (Integer, String, Boolean, Real, UnlimitedNatural) are primitive types made in
  the package Types the first time one is used (owner decision), and they stay as they were (AR-124
  decisions, the design being silent): they stay out of Models and the search, so Types shows only
  what the user put there; they are never renamed, moved or removed (the next use would make one
  again under a new id), specialize nothing, and carry no skill
  (the Skills panel says "Built-in types carry no skills"; a package-wide skill with no kind still
  reaches them). In a package named Types their names, whatever the case, are kept for them, so no
  classifier of the user's own takes one there. A built-in type's name always means the one in
  Types, never a type of the user's own named alike, and is never made twice.
- Names in one owner are told apart by kind, as UML does (owner decision, 2026-11-06, AR-208),
  compared without case: every classifier (class, interface, enumeration, data type, primitive
  type, association class) is one kind; a package, a property (attribute, association end,
  qualifier), an operation, a parameter, a literal and a generalization set are each a kind of
  their own; relationships, constraints and diagrams carry their names as labels and never clash.
  Two of one kind named alike are refused ("there is a package named Billing already", "the class
  has an operation named total already"); a package and a class, or an attribute and an operation,
  may share a name. `archforge_uml::names::kind` is the one rule: the commands ask it and the
  project's check as it opens asks it, so a project the commands made always opens again.
  Operations are not overloaded: two of one name are refused whatever their parameters.
- The package Types is the one named exactly Types directly under the root package (at the top
  level in a project without a root yet); a package named Types anywhere else is ordinary. Nothing
  is stored to mark it. While it holds built-in types it keeps its name and its place and is not
  removed, nor is any package holding one; an empty Types, or one with only the user's own types,
  goes as any package: Models says "Types keeps the built-in types, so it stays while it holds them" under the
  tree, and the canvas says it at its top, with no dialog. A package becomes Types (renamed, moved,
  or handed up by its parent's removal) only while there is no Types and only when it holds no
  classifier with a built-in type's name. Where an older project's Types holds a classifier named
  like a built-in type in another case or kind, the built-in type is not made beside it: the
  member's command is refused, naming that classifier and asking for it to be renamed. Models
  counts in Types only what it shows, so a Types holding only built-in types has no rows.
- An attribute reads `+ name : Type [0..1]`, an operation `+ name(param : Type) : Return`; the
  visibility sign only when one is set, the multiplicity only when it is not 1. A bound pair of
  none to many is written `*`.
- An association end a class owns is the line's role, not a row of the card.
- An association shows an open arrow at every end it navigates to, both ends when it navigates both
  ways (owner decision). A diamond sits at the whole, in place of an arrow there: hollow for a
  shared aggregation, filled for a composition. The whole's end is never navigable (owner
  decision): an end that starts to aggregate makes the other end not navigable, and Properties
  shows no Navigable on the whole's end.
  Its name sits at the middle, its roles and multiplicities at their ends.
- A generalization draws to a hollow triangle at the general class, in the muted ink, as ER's.
- A dependency is dashed, with an open arrow at its supplier.
- An interface realization is dashed in the muted ink, with a hollow triangle at its interface: it
  is neither a generalization (solid) nor a dependency (an open arrow).
- An association class (AR-125) is one element, a line and a card: the line an association's, the
  card a class's with `ASSOCIATION CLASS` in its eyebrow, above the line's middle, joined to it by a
  dashed tie in the muted ink (the accent while selected). The tie is worked out from what is drawn,
  never stored, so it follows the card and the line as either moves.
- The class diagram's toolbar is Class, Relationship with its chevron, Note and Link note. A
  class is added only with the Class tool, never by a click in Select.
- The Relationship tool's face is a sample of the kind it draws; its chevron opens a menu of
  Association, Directed association, Aggregation, Composition, Generalization and Dependency, then
  after a separator Interface realization and Association class (its sample a small card over the
  line's middle), then after another separator N-ary association (AR-128; its sample a small
  diamond on the line with a leg down) and Template binding (AR-131), in that order, each a row with its sample line, the current kind tinted and ticked at the
  row's right as the classifier tool's menu ticks its own (AR-191), and a pick closes it.
  A line is drawn from the whole to the part, from the specific class to the general one, from the
  client to the supplier, from the implementing class to its interface; dropped on empty canvas it
  makes a new class there, named in place, and for a realization a new interface. A refusal (a
  cycle, a class on itself) is said at the canvas's top and draws nothing.
- Interface realization: only a class starts one; from anything else the tool starts no line and
  says so at the canvas's top. A drop on anything but an interface, or a second realization from
  the same class to the same interface, is refused there and draws nothing; several classes may
  realize one interface. Its tooltip and accessible name say it runs from the implementing class to
  its interface. An association class implements as the class it is.
- Association class: drawn between two classes (an association class is one) as an association is,
  its line and its card above the line's middle made as one element and one undo step, the card's
  name (a fresh `AssociationClass n`) open for typing. Dropped on empty canvas it makes a new class
  there, named in place, with the line to it and the card. From or to an interface, an
  enumeration, a data type or a primitive type it is refused at the canvas's top ("An association
  class joins only classes") and draws nothing; nothing specializes an association class. Its
  tooltip and accessible name say it joins two classes with a card of its own. A click on its line
  or its card selects the one element, both drawn selected. Remove from this diagram, or Delete,
  takes the line and the card away together (`diagram.unplace` takes the edges of the same
  element on that diagram with its node); Delete on its line never deletes it from the model, card
  or no card, and Delete from the model asks first, from its line alone too. A line without its card (its classes placed on
  a diagram after it was drawn, or `connect-all`) and a card without its line (an end class not
  shown) are both kept states; a line without its card shows the name at its middle, as an
  association's. Its attributes and its ends' roles share its names: a clash either way is refused
  under the card's row or in Properties. Dragged from Models it
  places its card, with its line where the diagram shows both its classes. It renames and moves
  as the classifier it is; its name is one of its package's classifiers' names.
- A new association is a plain line: no roles, no multiplicities, navigable to neither end (owner
  decision); a directed one navigates to its second class; an association owns both its ends, so drawing lines never changes a class's
  attributes (owner decision).
- A right click on a class opens its menu: Open in Models, Add attribute, Remove from this diagram
  and Delete from the model…, which asks first, naming the relationships and diagrams that go with
  it, as a table's does. Delete removes the selected cards from the diagram; Shift+Delete asks to
  delete from the model.
- A selected class-diagram line shows a round handle at each end, on the line a few points clear
  of the end's mark as drawn (arrowhead, triangle or diamond), round a bend near the end too, so
  the mark shows whole; no square is drawn on such a tip. An end without a mark keeps its handle
  on the end. On a short line each handle stops half a hit box short of the middle, and where a
  line bends back so the two would still meet on screen they are placed together, apart, giving
  up as little of a mark's clearance as they must, so each takes its own press (AR-191). One
  dragged onto another class moves that end there, its lines on every diagram following. A
  realization's class end moves onto another class and its interface end onto another interface,
  with the same refusals as drawing one: a drop elsewhere, or onto an interface the class realizes
  already, is said at the canvas's top and changes nothing. Delete removes the line's relationship
  with a toast and Undo.
- Packages nest as deep as needed. The Models tree shows a package, then the packages it holds,
  then its classes, and under an open class its attributes (with their type) and operations
  (`name()`, with their return type), every level folded until opened; Expand all opens packages
  and classes. `+` adds a class or a package inside the selected package (or the package of what is
  selected); with nothing selected, a package in the root package.
- A class moves to another package by dragging its row onto a package's row (or onto a class of
  that package), the target ringed while held over it, or with the Package select beside Name in
  its Properties (packages named by their path, `sales › orders`). A package row drags onto
  another package the same way.
- A class of another package on a class diagram is an outsider: dimmed, its package named in the
  eyebrow, as ER draws a table of another schema.
- Removing a nested package needs no dialog: what it held moves up to its parent and the toast
  says so with Undo (owner decision, every notation). A top-level package asks, as anything that
  takes its contents along does.
- An association's Properties: Name, its From and To classes read only with "Drag a line end onto
  another class.", then a section per end, "End at {class}": Role (written on Enter or leaving the
  field) and Multiplicity, Navigable and Aggregation (None, Shared, Composite; written when
  changed). A dependency shows Name, Client and Supplier; a generalization Specific and General; an
  interface realization Implementing class and Realized interface, with no name. An association
  class shows an association's Name, From and To and its ends' sections, then Attributes · n and
  Operations · n with a class's editors; its members are typed on its card as a class's are.
- An end's Multiplicity is an editable select (owner, 2026-11-02): its text takes typing, its
  chevron (or Alt+Down) opens `0..1`, `1`, `*` and `1..*` with the current one ticked, the hint
  "Or type a number or range" and Clear. None to many is offered once, as `*`, the way it is
  written (AR-191); `0..*` typed still reads as it and shows as `*`. A preset writes on its pick;
  typing writes on Enter or leaving the field; empty text or Clear takes the multiplicity away, and
  an empty field shows a muted dash that is never a value. While the menu is open the arrows move, Enter picks and Escape
  closes it; with it closed, Escape brings back the value written. Text that does not read stays in
  the field, ringed in the danger colour with the reason under it, and the model keeps its value.
- A multiplicity reads as a whole number (`0`, `2`), a range `lower..upper` with lower up to upper,
  `lower..*`, or `*` for `0..*`, each bound at most 1,000,000; whitespace around it is dropped and
  anything else inside it is refused with its reason, never coerced (owner, 2026-11-02). Attributes
  and parameters read theirs the same way.
- Its own icons: the Class tool and a class in the tree are a box in three compartments (a
  table's box has a column line instead); a package is UML's tabbed box; a class diagram's tab
  shows two class boxes joined by a generalization arrow.
- A class's Properties: its name, Abstract, then Attributes · n and Operations · n, each row read as
  the card shows it, with the columns' row-and-editor pattern (+ adds, the pencil or a double
  click edits, Apply or Enter writes, Cancel or Escape leaves). A selected member shows its class's
  panel, its row marked. The attribute editor holds Visibility (narrow) and Name, Type and
  Multiplicity, and Static; the operation editor Visibility and Name, Return type, Static and
  Abstract, then its parameters, each on two lines: its name with its ✕, then its type and its
  multiplicity (narrow, `1` as its hint). Remove and Move up and down sit at the editor's foot on
  the left, Cancel and Apply on the right.
- No subclasses (final) (AR-137, design page 08, frames G1 and G3) sits in a classifier's
  Properties after Abstract, for every kind but a built-in type (an association class needs none:
  nothing specializes one), its explanation under it ("Nothing may specialise Payment. Shown as
  {final} after its name."), which also describes it; No redefinition (leaf) follows it. Ticked,
  the card shows `{final}` after the name in the small secondary style, and a generalization drawn
  to it, or a general end dragged onto it, is refused at the canvas's top ("PricingService is final:
  nothing may specialise it. Clear "No subclasses" in its Properties first."). Ticked while
  something specializes it, the box stays clear, nothing is written, and a refusal on the danger
  tint under it names those classifiers, with Select the n generalizations under it. Agents use
  `uml.classifier.set`.
- No redefinition (leaf) (AR-138, design page 08, frames G1, G2 and G4) follows No subclasses in a
  classifier's Properties (`workspace.properties.element.isLeaf`), its explanation under it ("A
  subclass could not redefine Payment where it is inherited. Shown as {leaf}."), and sits in the
  attribute editor under Unique and in the operation editor under Static and Abstract
  (`workspace.properties.member.isLeaf`, "A subclass could not redefine this member. Shown as
  {leaf} after it."); the explanation describes the box. Ticked, `{leaf}` follows the classifier's
  name in the same braces as final (`{final, leaf}`) and ends a member's row (first in a property's
  braces). Ticked on an element something redefines, the box stays clear, nothing is written, and a
  refusal on the danger tint under it names what does ("CardPayment::amount redefines it. Remove
  that redefinition first."). Redefines lists a leaf property last with a muted `{leaf}`; picking
  it changes nothing and the reason sits under the field. A classifier's box writes at once; a
  member's with its editor's Apply. Agents use `uml.classifier.set`, the attribute and operation
  commands and `uml.association.set-end`, each with `leaf`.
- Generalization sets (AR-129, design page 08, frames B1 to B4) have no tool. Generalizations
  selected (Shift-click their lines) are grouped from their right-click menu: its heading `3
  GENERALIZATIONS · TO PAYMENT`, then Create generalization set… and Add to set ▸ a set of their
  class (one row a set, `none for Payment yet` when there is none, `already in it`), then Remove
  from this diagram and Delete from the model (`workspace.canvas.contextMenu.generalizationSet.create`,
  `.add.<i>`). The shortcut is ⌥⌘G (Ctrl Alt G elsewhere): the design's ⇧⌘G is File ▸ Generate…'s
  already, so the owner is to confirm this one. A new set takes a fresh name (`GeneralizationSet
  1`), is selected, and its name field in Properties takes the keyboard. Several elements selected
  with a generalization among them show Properties' selection panel (`3 generalizations`,
  `Selection · all specialise Payment`) with Create set from selection
  (`workspace.properties.selection.createGeneralizationSet`) and Add to … buttons. Generalizations
  of different classes, or anything else selected with them, leave the actions listed but disabled,
  the short reason at the row's right (`parents differ`, `no shared parent`), the whole reason on
  hover, as the description for screen readers and on the danger tint under the menu and the
  buttons ("CardPayment specialises Payment and Voucher specialises Product. A set groups
  generalizations of one class."). On the canvas a set is a dashed line square across its member
  lines, a step towards the general class for each further set of the same lines, and its label
  `name {complete, disjoint}` beside the middle member, on the general class's side, on every
  diagram that shows a member line; muted, the accent with a ring while selected. A click on the
  label, or its row in Models (`name` then a muted `generalization set`, after its general class),
  selects it; dragging the label moves it on that diagram alone (a node of the kind
  `uml.generalization-set.label`, nested in the general class's node). A label takes a press only
  where it is drawn: on a diagram that shows none of its set's lines its node keeps the place but
  is looked through, never hit. A set's Properties (section
  Set): Name and General class (read only), Completeness (Complete | Incomplete) and Disjointness
  (Disjoint | Overlapping) as segmented choices the arrow keys switch, what they mean
  ("Shown beside the lines as {complete, disjoint}: every Payment is exactly one of these kinds."),
  then Members · n with `+` (only generalizations to its general class), a pencil (select the
  generalization) and a cross (take it out) per row. A generalization's Properties add
  Generalization sets · n, each set with its braces, `+` adding it to another set or starting one.
  Reconnecting a member is refused at the canvas's top and on the danger tint in its Properties
  ("CardPayment → Payment is in method and channel, which group the specialisations of Payment.
  Remove it from those sets before reconnecting it to Product."), the drag snapping back. Deleting
  a member takes it out of its sets in the same step, a set left with no member going too, the
  toast naming it; Delete on a selected set removes the set alone, its generalizations kept.
- Qualified associations (AR-127, design page 08, frames A1 and A6): qualifiers belong to the end
  where the box is drawn, the qualified end, the one at the class that picks (the Order end of
  Order to OrderLine); the other end's multiplicity counts what one qualifier value picks and is
  edited on that end. In Properties each end's part ends with `Qualifiers · n`, its `+`
  (`workspace.properties.end.<i>.qualifiers.add`) opening the row editor: Name and Type (the
  shared type menu) side by side, Multiplicity under them, a line saying what it picks ("One order
  and one lineNo pick 0..1 line."), the bin and the moves at its foot on the left, Cancel and Apply
  on the right; rows read `lineNo : Integer` with their pencil, ids
  `workspace.properties.end.<i>.qualifier.<k>` and `….<k>.name`, `.type`, `.multiplicity`,
  `.remove`, `.up`, `.down`, `.cancel`, `.apply` (a new one's `k` is the count). A qualified end
  says "Qualified end: the qualifier box is drawn here, against Order." under its title; the
  other end's multiplicity field is labelled Per qualifier value with "Counted per qualifier
  value of the other end; shown next to OrderLine." under it. As built, the ends keep their two
  sections (End at …) rather than the design's Ends switch; n-ary associations (AR-128) list their
  ends as rows with an editor each, as frame A4 draws, so no switch is built. A name another qualifier of the end has, whatever the case, is refused in a tinted
  box under the fields with the name ringed ("This end already has a qualifier named lineNo. Pick
  another name."), the editor open and nothing written. On the canvas the box is drawn in the
  surface, framed in the line's ink, against the side of the class's card the line leaves, rows in
  model order; the line, its end mark, role, multiplicity and end handle start from its outer
  side (`workspace.canvas.edge.<id>.qualifiers.<i>`). Selecting a qualifier shows its
  association's panel, its row marked. Qualifiers are never attributes or roles: cards and
  Attributes leave them out. Agents use `uml.qualifier.add`, `.set` and `.move`, and
  `element.remove`.
- A type menu offers None, every built-in type (Integer, String, Boolean, Real, UnlimitedNatural),
  then the model's own types by name, every one of them: the menu scrolls, the arrow keys reach any
  row, and only the rows in sight are laid out (AR-123). A built-in type is made in the package
  Types the first time it is used (owner decision). Where several entries share a name, whatever the
  case, each has a muted note after it saying where it is from: "built-in", or its package's path
  below the root package; the note joins the row's accessible name, and the menu widens to show the
  noted rows whole, kept inside the window (AR-124). A type of the user's own named like a built-in
  one never hides it. Typed on a card, a name several types share is refused ("Several types are
  named Integer; pick one from the menu.").
- A selected class's second adorner is Add attribute. A class dragged from Models lands sized to
  show its members, with its lines to the classes the diagram shows, as one undo step.
- Members are typed on the card (a proposal for the owner to confirm): a selected class
  shows a + at the right of each compartment; it, the adorner, the menu's Add attribute and ⌘↩
  open a draft row at the end of the attributes (⇧⌘↩ and the operations' + at the end of the
  operations), and a double click on a member's row opens that row. The row is ringed in the
  accent: the name where a row's text starts, then the type (an operation's return type) with
  its menu. Tab moves between the two; Enter writes the member as one undo step through
  Properties' command and opens the next draft in the same compartment; Escape drops the draft; a
  click away writes a draft with a name and drops an empty one. A refusal sits under the row, the
  text kept and the model untouched. The row's ⋯ takes what was typed to Properties' editor for
  visibility, multiplicity, static, abstract and parameters. Below the detail zoom, or without
  the class's card on the open diagram, Properties' editor opens instead.
- A card never hides a member silently (2026-11-02). A new class is as tall as a row in
  each compartment. Adding or editing a member, from Properties, the card itself, the control
  channel or Chat, grows every card of its class that would hide one, on every diagram, in the same
  undo step and keeping its width: the member's command grows it (AR-241); a card is never shrunk.
  A card made shorter by hand stays so until a member of its class is next added or edited (a
  move, which hides no row, leaves it);
  the rows it cannot show are said in one muted row, `+ n more members`, in the last place that
  fits, whichever compartment they are in; with no row's place left, a row above the card's foot
  while that is under the header, and on a card shorter still at the right of its eyebrow's line.
  Below the detail zoom every card shows only its name.
- N-ary associations (AR-128, design page 08, frames A2 to A5): one association of three ends or
  more, drawn as a hollow diamond (the page-02 N-ary diamond, 44 × 28, its name above it) with a
  leg to the card of each end's class; the diamond is a node of the association and each leg an
  edge of its end, so nothing new is stored. Drawing: pick N-ary association, then click each class
  in turn; the diamond appears with the second end at the middle of the cards clicked, a dashed leg
  follows the pointer, and the page-02 Gesture hint at the canvas's top counts the ends ("N-ary
  association · 2 ends — click a class for the next end", then Enter, Backspace and Esc in muted
  words); Properties shows the draft ("New n-ary association", Ends so far). Enter, or a double
  click on the last class, finishes at three ends or more as one undo step and selects it;
  Backspace drops the last end; Esc, another tool or kind, a click on empty canvas or another tab
  cancels and leaves nothing. A class clicked twice has two ends. New ends are named after their
  class (`customer`, `customer2` for a second end there) and start at `*`; the association gets
  no name until one is typed. A click on the diamond or any leg selects the one association, the
  diamond and every leg drawn in the accent. Properties: Name and its package (read only: nothing
  moves an association yet), then `Ends · n` with `+` and a row per end, `customer : Customer [*]`
  with its pencil; the row editor has Role and Type, Multiplicity, Navigable, Ordered and Unique,
  the line "Ends of an n-ary association are not aggregations…", the bin at its foot and Cancel and
  Apply (one undo step); a role another end has, whatever the case, is refused in a tinted box in
  the editor. The design's arrows that reorder ends are left out: nothing reorders them yet. Add
  end (`+`, or the diamond's or a leg's right-click menu: Add end, Reconnect end, Remove from this
  diagram, Delete from the model…) waits for a class click; dragging a leg's class end, or picking
  another Type, moves that end; a leg is drawn on each diagram that shows the diamond and its class. Removing one of three ends asks in
  Properties (frame A5): "Two ends would remain", Keep 3 ends (focused, as Esc) or Make it
  binary, which turns it into a plain association between the two classes left, a named line in the
  diamond's place wherever both are shown, as one undo step; with four ends or more the bin removes
  at once. Delete takes the diamond and its legs off this diagram; Delete from the model asks first.
  Its ends are never aggregations, as UML has only binary aggregations. Agents use
  `uml.association.{add-nary, add-end, remove-end, reconnect-end}` and `uml.association.set-end`
  with an end's place.
- Constraints (AR-133, page 08 section E): a classifier's or a package's Properties end with
  `Constraints · n`, its `+` opening the editor with the selected element as the one constrained
  element; each row reads `name {expression}` with its pencil (a double click opens it too). The
  editor holds Name and Language (an editable select: Plain text when empty, OCL, English, or any
  label typed), the Expression in a field of several lines (Enter is a new line there), the
  Constrained elements as chips, each with its ✕ while more than one is left, and a `+` that offers
  every package, classifier, member and relationship of the model; then, while a class diagram is
  open, On this diagram: Linked note | Braces, and the lines "Stored as written…" and "Long or
  multi-element constraints read best as a linked note…". Bin, Move up and Move down sit at its
  foot on the left, Cancel and Apply on the right. Apply is one undo step: the constraint and how
  the open diagram shows it (a new one is shown there always; an existing one where it is shown or
  when its form was picked). A new constraint's form follows its text until picked: braces for one
  line of at most 40 characters about one element, else a note. Stored, never evaluated: the
  expression and the language are kept exactly as typed. The only refusals are an empty or blank
  expression ("Type an expression. A constraint without one states no rule.", in a tinted box under
  the chips, the field ringed) and an element outside the model; braces on a diagram that shows
  none of the constraint's elements are refused under the switch. On the canvas, braces are
  `{expression}` in purple, nested in the node of the first constrained element the diagram shows
  (a member's: its classifier's), so they move with it; a constraint note is the page-02
  Constraint note: the surface, a purple border and folded corner, `CONSTRAINT · NOT EVALUATED` in
  its purple eyebrow, its name, its expression between braces, tied to its elements' cards by
  dashed purple links worked out from the model, never stored. A plain note stays amber and
  linked by hand. A double click on braces or a note opens the editor; Delete takes them off the
  diagram only; selecting a constraint shows its context's panel, its row marked. Deleting a
  constrained element drops it from the constraint in the same undo step and the toast says so
  ("Payment deleted · paidInFull now constrains Order only"); a constraint left naming nothing goes
  with it and the toast names it. The content is the model's, shared by every diagram; the form and
  the place are each diagram's.
- Nested classifiers (AR-132, design page 08, D1 to D4; owner decisions 2026-11-06). A class, an
  interface or an association class owns classifiers through UML's nestedClassifier: a class, an
  interface, an enumeration or a data type (the design's four kinds; a primitive type and an
  association class stay in a package, and an enumeration, a data type and a primitive type own
  none). A row's context menu in Models (right click, or ⇧F10; `workspace.explorer.contextMenu.*`)
  holds New inside {name} ▸ (its submenu headed NEW INSIDE {NAME}: Class, Interface, Enumeration,
  Data type, then Package disabled, "not inside a class"; on an owner of none the row is disabled
  with the reason), Rename F2, Show in diagram, a line, and Delete from model; the arrows walk it,
  → opens the submenu, Enter picks, Escape closes. A pick makes the classifier as one undo step,
  named afresh and open for typing in Models. Models lists it under its owner before the owner's
  members, a muted `nested` after its name. Properties: Package becomes Owner on every classifier
  (`workspace.properties.class.owner`), a package or a classifier; its menu (OWNER) lists the
  owners that may take it with their kind at the right and the current one ticked, then, after a
  line, those that may not, muted with a short reason (itself, owns none, `{kind} {name} already
  holds that name` naming the clashing element by its kind, for a package owner too, not inside a
  class) and the whole reason on hover and as their description; one nested in it is never
  listed. A pick is `uml.classifier.set-owner`, one step, keeping identity, members, relationships
  and type references. A nested classifier's kind line reads `{kind} · nested in {owner} · model, in
  every diagram`, and an explanation sits under Owner. A class's or an interface's panel has
  Nested classifiers · n after No redefinition, its `+` offering the same kinds, its rows selecting.
  Names: a nested classifier's differs, whatever the case, from the other classifiers its owner
  holds, by the one rule of names in an owner (owner decision 2026-11-06, AR-208): an attribute
  or an operation of the owner may share it, so the sample has the design's D2 `priority :
  Priority` beside nested Priority. On the
  canvas a nested card whose owner's card is on the diagram is tied to it by a solid line in the
  muted ink (the accent while either is selected), straight where the cards face each other, else
  one elbow crossing as few cards as it can, ending in ⊕ just outside the owner's side; it is
  worked out from the model, never stored nor selected, and overlapping cards draw none. Where the
  owner is not shown, the card reads `Order::Priority`. A card over another is never ownership. It
  is placed by dragging its row from Models; the classifier tool keeps placing package members.
  Deleting an owner from the model asks first, the dialog naming what it nests (`{n} nested
  classifiers go with it: Order::Priority.`), and takes them along with their relationships and
  cards in one step.
- Templates (AR-130, design page 08, C1, C4 and C5). A class, an interface or a data type
  becomes a template with its first parameter: `Template parameters · n` in its Properties, after
  Nested classifiers, its `+` (`workspace.properties.class.templateParameters.add`) opening the
  row editor; rows read `Key : DataType = Integer` with their pencil (a double click opens it too),
  ids `workspace.properties.class.templateParameter.<k>` and `….<k>.name`, `.kind`, `.constraint`,
  `.default`, `.remove`, `.up`, `.down`, `.cancel`, `.apply`. The editor holds Name and Kind side
  by side, Constrained to (Any, or a classifier of the kind) and Default (None, or one of the kind
  that fits the constraint) under them, a line saying what an argument must be ("Arguments for Key
  must be data types. Bindings that leave it out use Integer. Renaming or reordering keeps every
  binding."), the bin and the moves at its foot on the left, Cancel and Apply on the right. The
  Kind menu, headed PARAMETER KIND, lists Class, Interface, Data type, Enumeration and Primitive
  type, the current one ticked, then after a line Value and Operation, disabled with "not yet" at
  their right and the reason on hover, never taken (`….kind.<kind>`). A name another parameter has,
  whatever the case, and a default that does not fit are refused in a tinted box under the fields,
  the field ringed. Removing a parameter a binding uses is refused under the rows on the danger
  tint ("OrderRepository binds Key -> Integer. Change or remove that binding first, then remove
  Key."), its row marked, with Show the binding (`….showBinding`), and so is a template's last
  parameter while any binding names the template ("OrderRepository binds Repository, and Key is
  its last parameter. …"). Show the binding selects the binding itself (AR-131), its line and its
  Properties, and the line under it says so. The card shows the
  signature in a dashed box in the secondary text colour on the surface, over its top-right corner
  (28 past its right side, 24 above its top), one parameter a row in the caption style, drawn over
  the selection (`workspace.canvas.node.<id>.signature`), on every diagram, at the detail zoom.
  Models writes a muted `template` after its name; its kind line reads `Class · template, 2
  parameters · model, in every diagram`. A parameter's classifier types its template's own members
  alone: their type menus list its parameters first, after None, each noted `template parameter`.
  Agents use `uml.template-parameter.add`, `.set`, `.move` and `.list`, and `element.remove`.
- Template bindings (AR-131, design page 08, A2, C2 and C3). Template binding is the Relationship
  menu's group after Association class, under a separator (`workspace.relate.template-binding`,
  its sample a dashed line with an open arrow under a tiny `«bind»`), after N-ary association,
  which opens that group (AR-128). It is dragged from the bound class to the template; releasing it on a template
  commits nothing: Properties shows the draft, `New template binding` over `Drawing · not in the
  model until you apply`, Binding with Bound element and Template read only, `Substitutions · n`
  with one row per parameter (its name strong, its kind muted after it, its selector at the right,
  `workspace.properties.templateBinding.substitution.<k>.argument`), each listing only what fits
  the parameter's kind and constraint, Default (Integer) first where the parameter has a default,
  a parameter on its default showing it with a `default` tag and a parameter without one `Choose a
  class`. Cancel and Apply are the design's text buttons (`….cancel`, `….apply`), the help under
  them. The draft's line is dashed in the accent colour from card to card with an open arrow until
  Apply, which adds the binding, its line and its `name` label as one undo step and selects it. A
  parameter still needing an argument refuses Apply on the danger tint (`T needs an argument: pick
  a class. Key can keep its default.`, `….refusal`), its selector ringed, the draft kept. Cancel,
  Escape, another selection or another diagram drops the draft, nothing written. Drawn, the line is
  dashed in its own colour with an open arrow at the template and reads `«bind» <T -> Order, Key ->
  Integer>` at its middle; the text writes `->`, which Inter draws as an arrow, as every message
  about bindings writes it. Its Properties are titled Template binding, kind line `Binding ·
  OrderRepository binds Repository · model, in every diagram`, the section Binding with Bound
  element and Template selects (`….bound`, `….template`), then the substitutions, each pick one
  undo step; another template opens the draft over the binding, applied by Apply. Models writes a
  muted `binds Repository` after the bound class. A drop that may not bind (another kind, the
  template itself, a second binding of one template, a cycle) is said at the canvas's top; empty
  canvas makes nothing. The binding's end drags onto another template only when that one needs no
  argument, else the canvas points to Properties. Deleting a classifier a binding uses as an
  argument is refused, naming the binding. Agents use `uml.template-binding.add`, `.set` and
  `.list`, `uml.relationship.reconnect` and `element.remove`.
- Profiles and stereotypes (AR-134, design page 08, F1, F2, F5, F6; owner decisions 2026-11-06).
  A profile sits in a package beside its packages, never in a classifier, made with Models' row
  menu New ▸ Profile on a package or the root (New ▸ lists Package and Profile, a line, then the
  classifier kinds; ids `workspace.explorer.contextMenu.newInside.{package, profile, class, …}`),
  named in place, `profile` muted after its row; a profile's name is a package's kind. New ▸
  Stereotype on a profile, or the `+` under its Properties' Stereotypes · n
  (`workspace.properties.profile.stereotypes.add`), makes a stereotype, its row muted `extends
  Class, …` (or `extends nothing yet`). A stereotype's Properties: Name and Profile
  (`workspace.properties.stereotype.profile`) side by side, Extends · n with `+`
  (`workspace.properties.stereotype.extends.add`, its menu the metaclasses Archforge draws by UML's
  names, Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization,
  Package, then Activity, Actor, Artifact, Component, Interaction, Node, StateMachine and UseCase
  disabled with `not drawn by Archforge yet`), each row's pencil opening an editor with the
  metaclass, the bin, Cancel and Apply; then Properties · n through the member editor, whose type
  menu offers only primitive types, enumerations and data types; a name starting `base_` is kept
  for the extensions. A package's Properties (the root's too) have Applied profiles · n
  (`workspace.properties.package.profiles.add`, its menu every profile, those applied ticked and
  disabled), each row's pencil opening Unapply (`workspace.properties.package.profileEditor.unapply`)
  and Cancel; the package's row says `applies Persistence` after its name. Storage (owner decision
  2026-11-06): everything is UML's own Profile, Stereotype, Extension, ExtensionEnd and
  ProfileApplication; the metaclasses are Classes of a hidden UML library, a second, unnamed UML
  Model at the project's top level known by UML's address `http://www.omg.org/spec/UML/20161101`,
  each made the first time a stereotype extends it and never twice, never shown in Models, type
  menus, the search or `uml.type.list`, and never changed; no `Profile::metaclassReference` is
  stored (the rows reader refuses an import its composite superset lists too), the extensions
  naming the metaclasses. A stereotype application (AR-135, owner decision 2026-11-06) is an
  InstanceSpecification classified by the stereotype with Slots for its values and a core Link
  from it to its element; one function finds them, and while any exists, unapplying a profile they
  need, deleting a stereotype or a profile, dropping an extension they need, moving a stereotype to
  another profile, deleting a property whose values are set, and moving or deleting what they sit
  in out of the profile's scope, or deleting a package that takes a used profile along, are
  refused, the uses counted under the section in a tinted box
  (`Persistence is in use in Commerce: «Entity» on Order and Customer, «Id» on 2 elements,
  «Reference» on places. Remove those first.`, two of one name counted, not named twice) with Show
  the {n} uses selecting them; deleting a used element itself removes its applications with it,
  the delete dialog naming them (AR-135).
  Open profile diagram (`workspace.properties.profile.diagram`, `uml.profile.diagram`) opens the
  profile's class diagram, made the first time, placing what it lacks below what it shows: «metaclass» cards in a
  row, «stereotype» cards under them, joined by extension lines, solid, a filled triangle at the
  metaclass; a stereotype's row dragged onto it comes with its metaclasses. A selected metaclass's
  Properties say it is UML's own and change nothing.
- Stereotypes applied (AR-135, design page 08, F3 and F4; owner decisions 2026-11-06). The
  Properties of every element a stereotype may extend (a classifier, a package, an association,
  a generalization; an attribute or an operation selected on its class's panel, the section then
  `Stereotypes of id · n`) have `Stereotypes · n`, after No redefinition and before the other
  sections, shown only once the model has a stereotype (only what works is shown). Its `+`
  (`workspace.properties.stereotypes.add`) opens the picker headed `APPLY TO ORDER`: every
  stereotype of the model, those that apply first (their profile at the right), one applied ticked
  on the accent tint, then after a line the rest, muted and disabled with the short reason at the
  right (`properties only`, `classes and interfaces only`, `extends nothing yet`, `Logging not
  applied`), the whole reason on hover and as the description; typing filters it (`Filter: ent`
  under the heading), the arrows walk what applies, Enter applies (the first that applies when none
  is reached), Escape or a press elsewhere closes; rows `….add.<stereotypeId>`. Each applied
  stereotype is a row, `«Entity»` semibold and its profile muted, a bin at its right
  (`workspace.properties.stereotypes.applied.<id>.remove`) taking it off with its values, its
  definition kept; then a line per property, its name at the left and its control at the right
  (`….applied.<id>.value.<propertyId>`): a select for a Boolean or an enumeration holding one value,
  its first row `Default (false)` (or `No value`) clearing it; a field for any other value, several
  separated by commas, written on Enter or leaving it, emptied to clear it. Unset, the control shows
  the property's default muted with a bordered `default` tag (a muted dash without one); a value set
  has no tag even when it equals the default. A value that does not read stays in its field, ringed
  in the danger colour, the reason under it (`many is no value of rows. A whole number, such as 42
  or -7.`, `….refusal`), nothing written; so are too many or too few values for the multiplicity
  and a unique property's value twice. Under the rows, "Values you set are stored on Order; …", and
  while a class diagram is open the check Show stereotype values on this diagram
  (`workspace.properties.stereotypes.showValues`). On a card the stereotypes replace the kind's
  word (`«Entity»` for CLASS, `«Entity, Auditable»` for two, in the profiles' order); a kind written
  as a keyword keeps it first (`«interface, Entity»`, a decision of ours, the design silent: the
  kind stays readable); an outsider reads `«Entity» · Commerce`. A member's row, on the card and in
  Properties, starts with its stereotypes (`«Id» + id : Integer`); a line's sit above its name in
  the caption style (`«Reference»`), at the middle where it has no name; an n-ary association's
  above its diamond's name, and an n-ary end's at its leg's middle (AR-128). A template binding
  shows none: no stereotype extends its metaclass yet (it is not among those offered). Models writes them muted
  after the name (`«Entity» · template`). With the diagram's check on, a card lists, under its
  members, a compartment per stereotype with a value set: `«Entity»`, then `table = "orders"`, a
  String's in quotes; a property left to its default is not listed; turning it on, and setting a
  value while it is on, grows the cards that would hide one, in the same undo step. Storage (owner
  decision 2026-11-06, with ours where it was silent): the InstanceSpecification sits at the
  project's top level beside the hidden library, so moving or handing up its element never moves
  it; a Slot exists only for a value set, so a default is never stored as a value; values are
  ValueSpecifications read by the property's type as a default is, an enumeration's an
  InstanceValue referring to the literal; the diagram's choice is kept by a core tag, as the root package's mark is: the
  one tag named `archforge.stereotypeValues` lists the diagrams that show their values, and goes
  when the last is turned off. Nothing is lost unseen: a property whose values are set is not removed, retyped or
  given bounds a value breaks, and a literal a value names is not removed, each refused naming the
  value. Deleting an element its stereotypes are applied to asks first, from Models too, and the
  dialog names the applications going with it and with what it takes along (`3 stereotype
  applications go with it: «Entity» on Order, «Id» on id, «Reference» on
  places.`), as it names nested classifiers (decision of ours, the design silent); so does a
  member's Remove in its editor when applications would go with it, writing nothing until
  confirmed, and a nested package carrying one, its dialog saying `What it holds moves to
  Commerce.` and its toast where it went. While the picker is open it takes the keys: Backspace
  edits its filter, never deletes the selection. A binary association's end has a section of its own in its part of Properties, after
  its qualifiers (`Stereotypes of customer · n`, `Stereotypes of this end · n` without a role; ids
  `workspace.properties.end.<i>.stereotypes.*`, no diagram check); an n-ary association's ends have no section
  yet and take stereotypes through `uml.stereotype.apply` with the end's id, which
  `uml.association.add-nary` and `uml.relationship.list` give. Agents use `uml.stereotype.apply`,
  `.unapply`, `.set-value`, `.show-values`, `.list-applied` and `.applicable`.

On every line: a role, and a label at the line's middle, sit above a level stretch of the line and
beside an upright one, never on the line or its mark.

### Structure for 0.12.0 (owner review 2026-11-05)

The design is page 06 of the design file, in light and dark. Placing a classifier, editing a
member and drawing a relationship stay three separate gestures:

- Placing a classifier. The Class tool becomes the classifier tool: an icon button with a chevron
  at its right, in Class's place. The chevron opens the kinds, each with its icon and name: Class,
  Interface, Enumeration, Data type, Primitive type. The current kind is marked; arrow keys move,
  Enter picks, Escape closes, and picking arms the tool. Its face and tooltip name the kind. A click
  on the canvas places one in the diagram's package, its name open for typing, and the tool goes
  back to Select. The kind stays for the next placement while the window lives; a new window starts
  at Class. Only kinds that work are listed. Models' `+` does not use the tool's kind: it offers
  each kind that works as a row of its own (Class, Interface, …) in the selected package. In Models
  every classifier is a row under its package with its kind's icon; an interface's and a data
  type's attributes and operations sit below it as a class's do, an enumeration's literals below
  it, and a primitive type has none. Dragging any classifier from Models places it, as a class is
  placed.
- Cards. A class keeps `CLASS` in its eyebrow. The other kinds show their UML keyword there instead:
  `«interface»`, `«enumeration»`, `«dataType»`, `«primitive»`. An interface and a data type have
  attributes and operations as a class has. An enumeration has one compartment, its literals in
  their order. A primitive type has no compartment: its card shows the `«primitive»` eyebrow and
  its name.
- Literals are members of their enumeration, never boxes of their own. They are added with the `+`
  of the Literals compartment, or ⌘↩, as a draft row with a name only; Enter writes one and opens
  the next. A double click on a literal's row on the card edits it in place, as a member typed on
  the card is (the UML card rule), not in Properties: this replaces, for literals, the shared rule
  that a double click on a member opens its editor in Properties. Properties lists them under `Literals · n`; an
  editor there has Name, Remove, Move up, Move down, Cancel and Apply. Names are told apart within
  their enumeration compared without case, so `Paid` and `paid` clash; a refusal sits under the row
  and leaves the model as it was.
- Drawing relationships. The Relationship menu gains, after a separator, Interface realization and
  Association class.
  - Interface realization is drawn from the implementing class to its interface: a dashed line in
    the muted ink, with a hollow triangle at the interface. Only a class or an association class
    implements, as in UML, where an implementing classifier is behaviored; from an interface, an
    enumeration, a data type or a primitive type the tool starts no line. Dropped on empty canvas it
    makes a new interface there, named in place, with the line, as one undo step, as the
    Relationship tool makes a class. A drop on any other element than an interface, or a second
    realization from the same class to the same interface, is refused at the canvas's top and draws
    nothing; several classes may realize one interface.
  - An association class is drawn between two classes as an association is; dropped on empty
    canvas it makes a new class there, named in place, as an association does. The line and a card
    above its middle appear as one element and one undo step, the card's name open for typing and
    its eyebrow `ASSOCIATION CLASS`. The card moves on its own; a dashed tie joins it to the line's
    middle. Selecting the line or the card selects the one element, and Properties shows its ends,
    attributes and operations. Remove from this diagram takes the line and the card away; Delete
    from the model deletes the element. It is the one class not added with the classifier tool:
    Models' `+` does not offer it, and it is made only by drawing. In Models it is a row under its
    package; dragging it onto a diagram places its card, with its line where both classes are
    shown.
- Property modifiers live in the attribute editor and the association end's editor, never in a tool
  of their own: Default value; Static, Derived, Read only, Ordered and Unique; and Subsets and
  Redefines. Subsets offers the other properties of the property's context and of its general
  classifiers whose type the property's type conforms to, whose upper bound is not below the
  property's, and whose name differs from the property's (UML 2.5.1, 9.9.17,
  `subsetting_context_conforms`, `subsetting_rules` and `subsetted_property_names`). An attribute's
  context is its classifier; an association end's context is the classifiers at the association's
  other ends, so an end offers their properties. Redefines
  offers only properties of the general classifiers (`redefined_property_inherited`) with which the
  property is consistent (`Property::isConsistentWith`): a conforming type, a multiplicity whose
  lower and upper bounds both lie within the target's, and composite aggregation wherever the
  target is composite (a composite property may redefine one that is not). These are
  the user's own attributes and ends: each keeps its subsets and redefines in its own Property
  fields, so an association's end offers both as an attribute does. The generator's drop of
  subsets on association-owned ends concerns only UML's own metamodel, not these. On a card a derived attribute starts with `/`, a
  default follows `=`, and braces list what differs from the usual (`{readOnly}`, `{ordered}`,
  `{nonunique}`, `{subsets name}`, `{redefines name}`); unique is the usual and is not said. An
  association end follows the same order beside its line: `/` before the role when derived, the
  multiplicity, `= default` after it, then the braces, as in `/items 0..* = none {ordered}`. A
  static attribute's or operation's row is underlined (AR-136) on every card that lists it, and in
  Properties' rows: the line runs under its glyphs, the ellipsis of a cut row too, in the text's
  ink and as thick as the text is large; a static abstract operation is underlined and slanted. An
  association end's static has no mark yet beside its line.
  As built (AR-126): the attribute editor keeps the design's layout under Type and Multiplicity
  (Default value; Static and Derived, Read only and Ordered, Unique; Subsets and Redefines side by
  side, a muted line under a select with nothing to offer), and one Apply writes it all. An
  enumeration-typed attribute picks its default from a select of its literals. The card writes a
  derived attribute as the design frame does, `+ / total : Money = 0`; several braces share one pair,
  comma-separated (`{readOnly, ordered, subsets parts}`); a String's default shows in double
  quotes. An end's modifiers sit under a Modifiers disclosure in its section, closed by default
  with a summary beside it, with their own Cancel and Apply, since the end's other fields are
  written as they change; its role and multiplicity stay where they were beside the line, the role
  taking the `/` and the multiplicity the default and the braces. A refusal sits under its field in
  the user's words; a type the kept default does not fit is refused under Type, naming the default.
  The selects offer one property each; a property of the model that subsets or redefines several
  shows them all, and picking one replaces them. A default that is a literal goes with the literal
  when it is removed, as a property's type does. A long end text is drawn further along its line so
  it clears the card, and clear of the end's arrowhead, diamond or triangle. For agents, a field
  `uml.attribute.set` or `uml.association.set-end` is not given keeps its value; clearing is said
  explicitly. Every target is checked against the property's new shape, so a multiplicity, type or
  aggregation change that breaks a kept target is refused; only a target the rules no longer
  allowed already is tolerated, so it never blocks an unrelated edit. Moving an association's end
  drops, in the same step, what the moved association's own two ends no longer subset or redefine,
  and the canvas says which; other properties that subset or redefine those ends are not looked at
  (they keep their entries, tolerated as above until edited).

## Not decided yet
The menu bar's structure; how a colour chosen in Appearance and the key-status colour combine;
packages; closing a project whose changes cannot be written; whether resolving a relationship
moves its note links. Ask the owner before building any of them.
