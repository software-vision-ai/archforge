---
name: archforge-design
description: "Use before designing or changing how Archforge looks or behaves: a screen, a panel, a tool, an editor, a gesture, a notation, a message or an icon. Holds the design decisions taken so far, so a change keeps to them or knowingly replaces one."
---

# How Archforge looks and behaves

The decisions taken so far about the interface, as rules. `skills/archforge-ui/SKILL.md` says how
to draw them. A change that departs from one of these is a new decision for the owner: say so in
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
- The left rail holds Models, Diagrams and Source Control; the right rail, between the canvas and
  the panel, holds Chat, Appearance, Properties and Skills, each with a shape of its own (AR-61,
  a proposal for the owner to confirm): a rounded speech bubble with two lines, a palette with
  three dots of paint, three sliders, and a page of rules with a checklist. Chat's panel says
  only Coming soon (owner, AR-63): no provider, message or history control until built-in chat is.
- Clicking the open column's own rail icon folds it; any rail icon opens a folded column on its
  content; the panel's collapse button folds the panel. Folded columns are kept in the preferences.
- The status bar says only what is true: the save state, the agent state, the location.
- A menu bar comes later; until then the search field and the designed chrome carry the commands.
- Outside a packaged app the window and the Dock show the Archforge icon; a packaged macOS app
  keeps its bundle's light and dark icon.

## Home and projects
- Home fills the window on its own, without menu bar, toolbar or status bar. Its welcome has no
  button: a project opens from the Open a project card or the title strip's + Open project.
- A start card for what cannot be done at all yet (Explore an example, Import a model) is left
  off Home, and the cards left share the row (AR-67).
- Recent projects keep the last twelve (name, folder, last opened) across restarts; a row opens
  its project, or shows its tab when it is open already. Each keeps the view it was left in
  (AR-59): its diagram and skill tabs in order, the one shown, and the canvas's pan and zoom; the
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
  the location with the system's folder picker and shows it in the field; a cancel keeps the
  folder before it, and a picker that cannot appear says why under the field (AR-66). What it says
  a project holds is only what is built: ER and UML class diagrams, skills, agent access, undo
  and saves (AR-67); nothing still to come is promised.
- Open project uses the system's folder picker; a damaged, newer or foreign project is refused
  with a dialog, and the project open already stays as it was.
- The shortest way to drawing: + in the diagram tab strip, a name, Enter. The New diagram dialog
  asks for a name, a kind and a schema, and makes a schema when the project has none.
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
  and the command key with comma open Settings, which for now holds Your name, with the initials
  beside it changing as it is typed; Done, the close button, Enter or Esc close it.
- The bar's Help button (and, on macOS, Archforge ▸ About Archforge…) opens About Archforge
  (AR-69): what Archforge is, and Support Archforge with GitHub Sponsors and Ko-fi, each opening
  its provider's page in the browser. Only an approved HTTPS address of the provider opens; one
  the host has not set up says Not set up yet and takes no click. It says a contribution unlocks
  nothing; it never opens by itself, and it is modal, so Tab stays in it and nothing behind moves.

## Explorer
- Models shows the model's elements, a notation's own containers first; diagrams appear only in the
  Diagrams explorer.
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
- Selecting is by click, Shift-click and marquee. A selected box shows handles and buttons beside
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
  body scrolls. A box's element and its members (a table and its columns, a class and its
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
- Generation is an agent's, never the window's (owner, AR-64): File → Generate… opens a handoff
  for the selected schema, table, class or diagram (else the open diagram): From, Output (SQL
  DDL with its dialect, which the export skills in effect suggest, or documentation), the export
  skills in effect with where each comes from and what overrides it, and the request to copy for
  the agent, naming the MCP resources it reads (`archforge://element/{id}`,
  `archforge://skills/effective/{id}/export`) and the model's revision. Its status line says
  whether agent access is on, that the request was copied, and when the model changed after;
  nothing in it shows output or claims the agent did anything. The dialog changes nothing.

## Messages
- Problems show as an orange badge on the box or line and in a Problems section of Properties,
  whose rows select the element.
- A fact the model's target cannot enforce (for ER, the database; say a one-or-more end) is a
  notice, never a refusal. It says in the model's words what is wrong, why, and which choice
  avoids it.
- A refusal shows where the action happened: under the field, under the box's name or under the
  tree. Removals that take other things with them show a toast with Undo.

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

## UML notation
What is UML's own, beside the shared rules above.
- A class diagram is of a package. New diagram offers Class diagram beside ER diagram; its second
  field is Package, with New package as ER has New schema.
- A class is a plain card: the surface's white with the plain border, its eyebrow CLASS, its name,
  then two compartments under dividing lines, attributes and operations, each shown even when
  empty. An abstract class's name, and an abstract operation, are slanted.
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
- The class diagram's toolbar is Class, Relationship with its chevron, Note and Link note. A
  class is added only with the Class tool, never by a click in Select.
- The Relationship tool's face is a sample of the kind it draws; its chevron opens a menu of
  Association, Directed association, Aggregation, Composition, Generalization and Dependency, each
  a row with its sample line, and a pick closes it. A line is drawn from the whole to the part, from
  the specific class to the general one, from the client to the supplier; dropped on empty canvas
  it makes a new class there, named in place. A refusal (a cycle, a class on itself) is said at the
  canvas's top and draws nothing.
- A new association is a plain line: no roles, no multiplicities, navigable to neither end (owner
  decision); a directed one navigates to its second class; an association owns both its ends, so drawing lines never changes a class's
  attributes (owner decision).
- A right click on a class opens its menu: Open in Models, Add attribute, Remove from this diagram
  and Delete from the model…, which asks first, naming the relationships and diagrams that go with
  it, as a table's does. Delete removes the selected cards from the diagram; Shift+Delete asks to
  delete from the model.
- A selected class-diagram line shows a round handle at each end; one dragged onto another class
  moves that end there, its lines on every diagram following. Delete removes the line's
  relationship with a toast and Undo.
- Packages nest as deep as needed. The Models tree shows a package, then the packages it holds,
  then its classes, and under an open class its attributes (with their type) and operations
  (`name()`, with their return type), every level folded until opened; Expand all opens packages
  and classes. `+` adds a class or a package inside the selected package (or the package of what is
  selected); with nothing selected, a top-level package.
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
  changed). A dependency shows Name, Client and Supplier; a generalization Specific and General.
- An end's Multiplicity is an editable select (owner, 2026-11-02): its text takes typing, its
  chevron (or Alt+Down) opens `0..1`, `1`, `0..*`, `1..*` and `*` with the current one ticked, the
  hint "Or type a number or range" and Clear. A preset writes on its pick; typing writes on Enter
  or leaving the field; empty text or Clear takes the multiplicity away, and an empty field shows a
  muted dash that is never a value. While the menu is open the arrows move, Enter picks and Escape
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
- A type menu offers None, the built-in types (Integer, String, Boolean, Real, UnlimitedNatural)
  the model has no type of the same name for, then the model's own types by name. A built-in type
  is made in the package Types the first time it is used (owner decision).
- A selected class's second adorner is Add attribute. A class dragged from Models lands sized to
  show its members, with its lines to the classes the diagram shows, as one undo step.
- Members are typed on the card (AR-62, a proposal for the owner to confirm): a selected class
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
- A card never hides a member silently (2026-11-02, AR-57). A new class is as tall as a row in
  each compartment. Applying a member in Properties, added or edited, grows every card of its class
  that would hide one, on every diagram, in the same undo step and keeping its width; a card is
  never shrunk. A card made shorter by hand stays so until a member of its class is next applied;
  the rows it cannot show are said in one muted row, `+ n more members`, in the last place that
  fits, whichever compartment they are in; with no row's place left, a row above the card's foot
  while that is under the header, and on a card shorter still at the right of its eyebrow's line.
  Below the detail zoom every card shows only its name.

On every line: a role, and a label at the line's middle, sit above a level stretch of the line and
beside an upright one, never on the line or its mark.

## Not decided yet
The menu bar's structure; how a colour chosen in Appearance and the key-status colour combine;
packages; closing a project whose changes cannot be written; whether resolving a relationship
moves its note links. Ask the owner before building any of them.
