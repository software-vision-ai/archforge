---
name: archforge-design
description: "Use before designing or changing how Archforge looks or behaves: a screen, a panel, a tool, an editor, a gesture, a notation, a message or an icon. Holds the design decisions taken so far, so a change keeps to them or knowingly replaces one."
---

# How Archforge looks and behaves

The decisions taken so far about the interface, as rules. `skills/archforge-ui/SKILL.md` says how
to draw them. A change that departs from one of these is a new decision for the owner: say so in
the change, and update this file in the same change.

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
- Tables are orange by default, the selection is accent blue, and a style set on one table wins.

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
  the panel, holds Agent, Appearance, Properties and Skills.
- Clicking the open column's own rail icon folds it; any rail icon opens a folded column on its
  content; the panel's collapse button folds the panel. Folded columns are kept in the preferences.
- The status bar says only what is true: the save state, the agent state, the location.
- A menu bar comes later; until then the search field and the designed chrome carry the commands.
- Outside a packaged app the window and the Dock show the Archforge icon; a packaged macOS app
  keeps its bundle's light and dark icon.

## Home and projects
- Home fills the window on its own, without menu bar, toolbar or status bar. Its welcome has no
  button: a project opens from the Open a project card or the title strip's + Open project.
- A start card for what cannot be done at all yet (Import a model) is left off Home, and the
  cards left share the row.
- Recent projects keep the last twelve (name, folder, last opened) across restarts; a row opens
  its project, or shows its tab when it is open already. A row or card under the pointer shows a
  More button whose menu offers Open and Remove from recent projects; removing leaves the project
  where it is, asks nothing, and opening it again lists it again.
- Pin a folder picks a folder with the system's picker and pins it on Home, up to eight, kept in
  the preferences. A click on a pinned folder opens the project picker in it; its menu offers
  Open a project here and Unpin.
- Home's search field (or the command key with K) opens a list under it: the recent projects and
  the projects in the pinned folders (two levels down, looked for each time it opens, one search
  at a time), up to eight projects in all, the pinned folders and Home's actions, each worded for the state it is in. Every word typed must
  be found; the arrows move, Enter runs, Esc or a click elsewhere closes.
- The recent projects show as rows or as a gallery of cards standing in the start cards' columns,
  sorted by last opened (newest first) or by name; both choices are kept in the preferences.
- One project holds one model, named as the project. The New project dialog asks for a name and a
  location only; a taken or unfinished name is said in plain words under the field.
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

## Explorer
- Models starts at the schemas: schema, then its domains, enum types, sequences and views, then
  its tables and their columns. Diagrams appear only in the Diagrams explorer.
- Models starts folded: only the schemas show, and the user drills in. The button beside + opens
  every row (Expand all), or folds them all (Collapse all) once everything is open; a filter opens
  what it shows, and the button rests while one is typed. An element selected on the canvas or in
  Properties opens its schema and table once, so its row shows; folding them again sticks.
- The + adds what fits the selection, named in place: Enter confirms, Esc cancels.
- Dragging a table from Models onto the canvas places it; a table already there is refused with a
  toast.
- Deleting from the explorer asks first when the element is on diagrams or has keys.
- The Models and Diagrams trees scroll under their filter (wheel or trackpad, a thin thumb at the
  edge while they overflow); a row newly selected, on the canvas, in Properties or with the keys,
  is scrolled into view once, so the tree never fights the wheel.

## Canvas and tools
- The tools float at the bottom centre of the canvas: Select, Pan | Entity, Relationship with its
  chevron, Note, Link note | Key status and Grid | Arrange diagram. The toolbar is never hidden;
  the minimap folds to its button when both do not fit.
- Arrange diagram (owner, 2026-11-01) lays the whole diagram out in layers along its lines: the
  table a relationship starts at above the one it ends at, each layer ordered so lines cross
  little, groups with no line between them side by side, tables with no line in rows below. It
  keeps the diagram's top left, drops dragged line routes, is one undo step, and shows the whole
  diagram after.
- The line under the pointer turns to the accent with a soft halo along its whole route, so it
  reads among the others.
- The Relationship tool's button is double width, and its icon shows the two ends its next line
  will get.
- A placing or drawing tool hands back to Select after it has made one thing.
- Selecting is by click, Shift-click and marquee. A selected table shows handles and two buttons
  beside it: connect and add column. Dragging moves the whole selection as one undo step; arrow
  keys nudge. Two or more selected tables show an arrange bar under them.
- Delete removes a table from the diagram only; Shift+Delete deletes it from the model after a
  dialog that names what goes with it.
- Double-clicking a column on the canvas opens its editor in Properties.
- Key marks (PK, FK, FKn) sit in a column at the left of each table row; a column that follows
  another column shows a link mark whose tooltip names the column it follows.
- Related tables are shown by a focus chip above the canvas, the other tables faded, never as a
  drawn region.
- A table of another schema is drawn faded with a dashed border and its schema in its header, and
  stays editable.
- The minimap sits at the canvas's right edge; the zoom control is in its footer. Its viewport
  rectangle is dragged to move the view (a hand over it, a closed hand while dragging); a press
  elsewhere in the map centres the view there and the drag goes on from there. The map keeps its
  scale while a drag lasts, so it never shifts under the pointer.

## Relationships and notation
- Crow's foot. Each end draws two marks: the outer one the most (a bar for one, a foot for many),
  the inner one the fewest (a circle for zero, a bar for one). Exactly one is two bars.
- An identifying relationship's line is solid; a non-identifying one is dashed. With Key status
  on, a line is green when its foreign key is on and orange when it is off.
- Relationships are drawn by dragging from table to table, never click by click, and drawing never
  blocks: without a parent key, or when either table has no columns at all, the line is drawn
  with its foreign key off.
- A relationship dragged from a table and dropped on empty canvas makes a new table there, joined
  by the line with its foreign key off, as one undo step, the new table's name open for typing.
- A line's cardinalities show as text beside the line near each end: under a level line (its name
  sits above), right of an upright one. A text is dragged anywhere and stays there relative to its
  end, as the line's label; a double click puts it back.
- A right-angled line's segment is dragged across (the pointer shows which way): a middle one
  slides, the first or last slides where the line meets its table. The line keeps its route from
  then on; a double click on it routes it anew.
- Cardinality is chosen in one place, the popover from the Relationship tool's chevron, with
  glyphs, not words; the choice stays for the next line. An existing line's ends change there or
  in Properties.
- Key columns a foreign key adds grow every box of their table to show them, in the same undo
  step; deleting a relationship takes its foreign key and those key columns with it. A project
  opens with every table box at least tall enough for its columns.
- Many-to-many offers a mapping table; a mapping table that holds only its two keys offers to
  collapse back into a line.
- ER has no generalization tool. Subtypes are added from the supertype's Properties and drawn as a
  solid line with a hollow triangle at the supertype.

## Notes
- The Note tool places a note on an amber tint, typed in place.
- A note links to a table or to a relationship with a thin dashed straight line; a link to a
  relationship ends on its line with a dot. A link is drawn from the selected note's handle or
  with the Link note tool, whose icon is that dashed line: press on a note, drag onto a table or a
  line. A link belongs to the diagram, can be selected and deleted on its own, and goes with what
  it joins.

## Properties and editors
- Properties shows model data, the same on every diagram; Appearance styles this diagram only;
  Skills is a tab of its own.
- The panel opens with the selection's name, its kind and a Description field, pinned while the
  body scrolls. A table and its columns share one page, so moving between them keeps the scroll.
- Sections are divided by full-width lines, each headed by a regular title with its count
  ("Columns · 3") and an icon action such as +. Fields are filled and borderless with small muted
  labels above them. The panel carries no help or footer texts.
- Every list row (columns, keys, checks, indexes, types, sequences, views) has an edit button that
  is always visible at its right end. Only that button, or a double click on the row, opens the
  row's editor. Selecting an element never opens an editor.
- A click on a row selects its element, so the Models tree and the canvas follow; selecting in the
  tree or on the canvas marks the row in Properties. The selection is always in step both ways.
- Selecting anything else closes open editors as Cancel would; coming back finds them closed.
- An editor commits nothing until Apply. Enter applies, Esc cancels, Apply is one undo step and
  closes the editor; a refusal shows under its field and keeps the editor open. Editor actions are
  icons: a red bin for Remove, a cross for Cancel, a check for Apply.
- Foreign keys come only from drawing relationships; a table's Properties has no foreign key
  section.
- Ticking Primary key ticks Not null, and unticking Not null unticks Primary key. A parent end's
  cardinality and its key columns' nullability move together.
- A linked column follows the name and type of the column it points at; any direct edit unlinks
  it, and the editor offers Unlink.
- The column type menu lists the schema's domains and enum types, then the built-in types.

## Appearance
- Appearance styles the selection on the diagram open in the centre, and on that diagram only.
  For now it holds Style (owner, 2026-10-31): a table, a note or another card takes Fill, Border
  and Text colour and a Border width; a line takes Line colour, Line width and Line routing. Text
  size comes later with its own model field; a line style is never offered, since dashed already
  means not identifying.
- Line routing (owner, 2026-11-01): Straight, Right-angled or Curved, the design's routing icons in
  a row of three under Line width. Route around tables, a switch under the row, shows only for
  Right-angled. The diagram sets how its lines run and a line may run its own way; with nothing
  selected the panel shows the diagram's routing. A new diagram's lines run right-angled around
  the tables; a diagram made before keeps straight lines until changed.
- A right-angled line leaves and enters a table at right angles to its side; around tables it
  keeps a margin from every table near it and takes the way with the fewest bends. A table's lines
  spread over the middle of each side, ordered by where their other table lies, so they leave
  side by side. Moving a table re-routes its lines as it moves.
- Each colour row starts with the diagram's own colour, drawn in the theme shown, which takes the
  row's own colour away; then the design's colours; then Your own colour, which opens a picker
  (a saturation and brightness field, a hue bar, the colour's code, Cancel and Apply). Nothing
  changes until Apply; Esc or a click outside cancels.
- A table or note with its own fill and no text colour of its own draws its text to read on that
  fill: dark on a light fill, light on a dark one, in either theme.
- A line's own colour shows while Key status is off; while it is on, the panel offers Show line
  colours, which turns it off. A line's own width shows always.
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

## Messages
- Problems show as an orange badge on the table or line and in a Problems section of Properties,
  whose rows select the element.
- A fact the database cannot enforce, such as a one-or-more end, is a notice, never a refusal. It
  says in the model's words what is wrong, why, and which choice avoids it.
- A refusal shows where the action happened: under the field, under the table's name or under the
  tree. Removals that take other things with them show a toast with Undo.

## Saving
- Every edit is written to the project on its own a moment after it settles, so a crash loses
  nothing. Save makes a version; "Unsaved changes" means edits not yet in a version.
- Every edit is a command that can be undone; a drag, an Apply or an arrange is one step.

## Not decided yet
The menu bar's structure; how a colour chosen in Appearance and the key-status colour combine;
packages; closing a project whose changes cannot be written; whether resolving a relationship
moves its note links. Ask the owner before building any of them.
