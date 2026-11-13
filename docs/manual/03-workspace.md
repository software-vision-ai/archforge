# The workspace

An open project fills the window with its workspace.

<!-- picture: workspace-diagram light | The workspace: Models on the left, a diagram in the centre, Properties on the right -->

- **The bar at the top** holds Home, the open projects' tabs, the search field, Help and your
  avatar (which opens Settings, where you enter your name; it is saved with each version, so
  anyone given a copy of the project can read it).
- **The left rail** switches the left column between **Diagrams**, at the top, **Models** and **Versions**,
  which lists what has changed since the last version and the project's history, newest first.
  **Save version…** there saves what changed as a new version:
  [Versions](09-versions.md#save-a-version) goes through it step by step.
  **Branches**, above it, lists the project's branches, each with when its latest version was saved;
  it stays folded while the project has only `main`. The branch you are on says *Current*, and a dot
  marks a branch with unsaved changes to the model (conversations don't count). Click another
  branch, press Enter on it, or choose **Switch** from its **More** menu to switch to it; ↑ and ↓
  move between branches. If your work has unsaved changes, Archforge first asks whether to save a
  version and switch, or switch and leave them on this branch for when you come back. Branches
  can't be merged yet. A project with more than 200 branches lists the first 200 by name, and says
  so under the list.
  The **+** in the Branches header, there even while the section is folded, opens **New branch**:
  a branch keeps its own versions, unsaved work and conversations, and the new one starts from
  your current work, unsaved changes included, which the branch you are on keeps as well. Type a
  name of at most 60 characters, on one line, not starting or ending with a space; a name another
  branch has is refused under the field, and **Create** stays off until the name can
  be used. **Create** (or Enter) makes the branch and switches to it; while Archforge makes it the
  dialog stays up. If it can't be made, the reason shows under the field and you stay where you
  were: change the name or try again. If Archforge can't tell whether it was made, look at the
  Branches list before trying again. If the switch to the new branch can't start, the reason is
  shown as for any switch, and the branch stays in the list for you to switch to later.
  **Start a branch here…**, in a version's details and as the branch button beside **Back to
  current work** in the banner of a version you are viewing, opens the same dialog for a branch that
  starts from that version instead. Your unsaved changes stay on the branch you are on, and viewing
  ends on the new branch once it is made. It isn't offered for a version that can't be read.
  **Rename…** in a branch's **More** menu renames it, the branch you are on too; `main` keeps its
  name. The name is checked as for a new branch, and **Rename** stays off while it is unchanged. The
  branch keeps its conversations, versions and unsaved work, and you stay on it. If it can't be
  renamed, the reason shows under the field and the name stays. If Archforge can't tell whether it
  was renamed, the dialog names the name you asked for and offers only that one: rename it to that
  name again to find out.
  **Delete…** in a branch's **More** menu deletes it; `main` and the branch you are on can't be
  deleted. The dialog counts what goes with the branch: the versions only on it, its conversations,
  and whether it holds unsaved changes. **Delete branch** waits for that count, or for word that it
  can't be read. The branch leaves the project's list, but its contents may remain in the project
  folder's storage, and there is no Undo. If it can't be deleted, the dialog says why and the branch
  stays. If Archforge can't tell whether it was deleted, delete it again to find out.
  Once a project has a second branch, the version chip in the bar at the top names the branch you
  are on, before *Current work*, and the status bar says *On* and its name before whether your work
  is saved. The chip's list then starts with that branch: choose it, or press ⇧⌘B (Ctrl Shift B),
  to list the branches there, each with when its latest version was saved (the first 200, saying so
  under them, as above), type to narrow them, and choose one, or move to it with ↑ and ↓ and press
  Enter, to switch to it as above. In a narrower window the chip shortens the branch's name first,
  then leaves out *Current work*, then shows only its icon, with the whole label when you point at
  it.
  Clicking a version in the history opens its details in the centre, and **View this version**
  there shows it read-only: [Look back at a version](09-versions.md#look-back-at-a-version) goes
  through History, viewing an earlier version and coming back to your current work.
  **Compare…** opens the Compare page in the centre, with this version as Before and your current
  work as After; the button between them swaps the two, and each select lists your current work
  and this branch's versions, reading further ones as you reach the end. What differs between the
  two is not shown there yet.
  **Restore this version…**, beside Compare… in the details and the arrow beside **Back to current
  work** in the banner of a version you are viewing, makes the content of that version a new version
  on top of the history, named *Restored the version of* and its date. Nothing is rewound or removed
  and later versions stay in History. A question names the version and says all this first, and
  that unsaved work is first kept as a version called *Before restoring* and the date and time, as
  History shows them, for example *Before restoring 8 Nov 15:42*. Undo starts fresh after a
  restore. If it doesn't finish, the same question says what happened and whether your work is as
  it was, with **Retry**, unless the version turned out to be the same as your current work. When
  another question is open at that moment, this one follows once that one is answered and the project shows again; until then Restore is off in your other
  open projects and says why. When Archforge can't tell whether it
  finished, the status bar says so instead. Restore isn't offered for the last version while nothing is
  unsaved, which is what you have now, or for a version that can't be read.
  While Chat is still changing the model, Archforge asks first: **Stop and go on** ends its reply,
  keeping what it has written so far, and **Keep working** leaves everything as it was.
  Clicking a model element under Working changes opens it in the centre: its name, whether it was
  added, changed or removed, where it is, and the version it is compared with, or that there is
  no version to compare with yet. Two cards side by side, **Before** (the last version) and
  **After** (current work), give the element's kind, name and values. A name or value that differs
  is marked with − on the Before card and + on the After card as well as a tint. An element with
  no name goes by what it joins or says, such as *Customer → Order*, and those words following a
  rename of *Customer* aren't marked as a change to it. A side the element isn't on says *Nothing here yet* or *Removed*. A summary above the cards counts the values,
  members and relationships that differ and says the first three values in words, or that the element was added
  or removed. Below the values the cards group the element's attributes or columns, operations, literals,
  other members, relationships and owner under headings, marked the same way when one came, went or changed; a
  long list scrolls with the cards. A relationship between two elements is listed on both their cards,
  whoever owns it, by its role at the far end or its own name, else its kind, and what it joins to,
  or *Not available on this side* when nothing is set there, such as `lines → OrderLine [1..*]` or `Generalization → Shape`; an association's end that a class
  owns is listed there rather than among its attributes. Renaming what it joins to isn't marked as a
  change to it. Under the cards, **Related change** names the other model elements connected to this one
  that also differ in this comparison: what it or its members refer to, what refers to them, and
  its owner. **Go to** and the name opens that element against the same version.
  The project's name at the top closes it.
  Clicking a diagram, or a box or line of one, under Working changes opens that diagram the same
  way. Two cards above it keep the model and the layout apart: **Model** counts the elements its
  boxes and lines draw that were added, changed or removed and names them, and **Layout** counts
  the boxes placed, moved, changed or taken off and the lines drawn, changed or taken off. When only
  one of them changed, the other card says so, such as *The layout didn't change* and *Only the
  model changed.* An empty diagram that was added or removed says *This diagram was added* or
  *This diagram was removed*. Under the cards are **Before** and **After** side by side, each drawn as it was
  on its side and both at one zoom, so a box that stayed is in the same place on both. A box that differs has an outline, a
  line that differs is drawn over in colour, and each says *Added*, *Changed* or *Removed*. A side
  the diagram isn't on says so. The two sides move together: drag or scroll either to pan both,
  pinch or scroll with ⌘ (Ctrl) held to zoom both, or click a side (or Tab to it) and use the arrow keys,
  **+** and **−**, and **0** to fit both again; clicking the diagram in the list again also fits it.
  When the element a marked box or line draws also differs, its *Added*, *Changed* or *Removed*
  opens that element's change.
- **The centre** shows the open diagrams and skills, each in a tab. The **+** at the start of the
  tab strip makes a new diagram (see *New diagram* below).
- **The right rail** opens the right panel on **Chat**, **Appearance**, **Properties** or
  **Skills**. Clicking the rail icon of the open column folds it away; clicking any icon opens it
  again.
- **The status bar** at the bottom says whether the project is saved, and where the project is.

On Windows and Linux the strip at the very top, on Home as well, also holds the **File**, **Edit**,
**Model**, **Versions** and **Help** menus; on macOS those commands are in the system's menu bar. A click on a menu's name
opens it or moves to it. A row that cannot act now is greyed out and says why when you point at
it, and a row with a shortcut shows it, such as `Ctrl+Shift+G`. Esc closes the menu, and so does a
click elsewhere; while one is open, keys go to neither the window behind it nor the shortcuts.
F10 opens the first menu from the keyboard and closes it again. The Left and Right arrows go between
the menus and the Up and Down arrows go through the rows that can act, both wrapping around at the
ends. Enter or Space runs the row, and Esc puts the keyboard focus back where it was.

**Model** holds what the diagram tools make. Choosing a row picks that tool on the canvas's toolbar,
as a click on the tool does: click the canvas to place the element, or drag between two elements to
draw the relationship. **Note** and **Link note** are always listed; the rest follow the open
diagram: **Entity** and **Relationship** on an ER diagram; on a class diagram the kinds of the
classifier tool, **Package**, and every kind of the Relationship tool. While no diagram shows, or
the diagram can't be changed, the rows are greyed out.

**Versions** holds **Save version…** (⇧⌘S, `Ctrl+Shift+S`), **History** (⇧⌘H), which opens the list
of versions, **Compare…**, **Switch branch** (⇧⌘B), **Older version** (⌘[), **Newer version** (⌘]) and
**Back to current work**. Compare… sets the version whose details show, or else the newest, against
your current work; Newer version needs a version showing. Save version… needs changes to save. While you look at an earlier version,
History gives way to Back to current work, which takes the same keys; Switch branch needs a second
branch. While the project is busy, saving or switching branch, every row waits.

## New diagram

The **+** of the tab strip, or of **Diagrams**, opens **New diagram** in two steps.

1. **What the diagram shows.** A card for each kind of diagram Archforge makes: **ER diagram**
   (tables, their columns and keys, and their relationships) and **Class diagram** (classes with
   their attributes and operations, and the lines between them). Choose a card, with a click or
   the arrow keys, and **Continue** (or press Enter, or double-click the card). Other kinds get
   their card once Archforge supports them.

   <!-- picture: workspace-new-diagram-kind light | New diagram's first step: an ER diagram card and a Class diagram card, Class diagram chosen -->

2. **Its name and what it is of.** The diagram's name, and the schema or package it shows: the
   one selected, else the first there is, else a new one named like the diagram (for a class
   diagram, the project's root package). **Create** makes the diagram and opens it in its tab.

   <!-- picture: workspace-new-diagram-details light | New diagram's second step for a class diagram: its name and the Package it is of -->

**Back** returns to the cards with what you typed kept; choosing another kind there gives it that
kind's schema or package. **Cancel**, or Escape, closes the dialog and makes nothing: nothing is
made until **Create**. A group's **+** in **Diagrams** has chosen the kind already, so it opens on
the second step.

## Diagrams and Models

**Diagrams** lists the project's diagrams, grouped by kind: **Class diagrams** and **ER diagrams**,
each with how many it holds. Choose a diagram to open it in its tab; a group's arrow folds it, and
its **+** makes a new diagram of that kind. A class diagram of a nested package sits in Class
diagrams like any other: hold the pointer over it to see where its package is, and the foot of the
explorer says the same for the open diagram (for example *Commerce platform / Sales / Orders*). The
filter field narrows the list to the diagrams whose name holds what you type, or to a whole group
when you type its name. Grouping only arranges the list: each diagram stays where it is in the
model. Other kinds of diagram get their own group once Archforge supports them.

<!-- picture: workspace-diagrams light | The Diagrams explorer: class diagrams and ER diagrams in their groups, a nested package's diagram open -->

**Models** lists the model's elements: an ER model's schemas with their tables and columns, a UML
model's packages with their classes, attributes and operations. It starts folded; open a row with
its arrow, or open every row with the button beside **+**. The filter field narrows the tree to
what matches. **+** adds what fits the selection, with its name ready to type: press Enter to keep
it, Escape to cancel. If a name is refused, the reason shows under it and the field keeps what you
typed, still ready for typing: correct it and press Enter, or press Escape to keep the old name.

An element exists once in the model and can appear on many diagrams. Drag an element from Models
onto a diagram to show it there.

## Properties and Appearance

**Properties** shows the selected element's model data: its name, its kind, a description, and
sections such as a table's **Columns · 4** or a class's **Attributes · 2**. These are the same on
every diagram. Each row has an edit button (a pencil) at its right end; it opens the row's editor,
which changes nothing until you apply it with the check at its foot (or Enter). The cross (or
Escape) cancels, and the red bin removes the row.

Type a new name in the **Name** field, or a description in the **Description** field, and press
Enter, or leave the field, to keep it; press Escape to give up what you typed and get back what
was there, with nothing changed. Empty the **Description** field to remove the description. A
name can't be empty: an emptied name is refused before anything changes, and the old name stays
(a relationship or a diagram may be left unnamed). Each change is one undo step. When a change is
refused, the reason shows at the foot of Properties until you select something else. A description
or an end's role you leave by selecting something else is kept then; if that is refused, the reason
names the element you typed it on, and a description stays in that element's field. In a UML model the
description is kept as a comment the element owns: it goes with the element when you remove it, and an agent
reading the element reads it too.

**Appearance** styles the selection on the open diagram only: fill, border and text colours, the
border width, and for lines their colour, width and routing (straight, right-angled or curved).

## Selecting and the canvas

- Click a box or a line to select it; Shift-click adds to the selection; drag on empty canvas to
  select everything in a rectangle.
- Drag selected boxes to move them; the arrow keys nudge them.
- Double-click a box's name to rename it in place.
- The tools float at the bottom of the diagram: **Select**, **Pan**, the diagram's own tools,
  **Note** and **Link note**, view toggles, and **Arrange diagram**, which lays the whole diagram
  out tidily in one step.
- The **minimap** at the bottom right shows the whole diagram; drag its rectangle to move the
  view, and use its footer to zoom. **Whole diagram** fits everything in the window.

## Search

The search field in the bar (⌘K, Ctrl K) finds the project's diagrams and elements by name or by
where they live. Every word you type must match. Choose a diagram to open it; choose an element to
select it, show its row in Models and bring it to the centre of a diagram that shows it.

<!-- picture: workspace-search light | Search finds diagrams and elements by name -->

## Undo

Every change is one step you can undo with ⌘Z (Ctrl Z) and redo with ⇧⌘Z (Ctrl Shift Z or Ctrl Y), however it was
made: on the canvas, in Properties, or by an agent. A drag, an Apply and an Arrange diagram are each
one step. **Edit ▸ Undo** and **Edit ▸ Redo** do the same, and while you are typing in a field they
act on that field's text instead of the model. Undo is not the same as going back to a saved version:
[Versions](09-versions.md#autosave-a-saved-version-and-undo) says how they differ.

## Removing from a diagram versus deleting from the model

These are different, and Archforge keeps them apart:

- **Delete** (or Backspace) with a box selected **removes it from this diagram only**. The element
  stays in the model, in Models, and on any other diagram that shows it.
- **⇧Delete**, or **Delete from the model…** in a box's right-click menu, **deletes the element from
  the model**, from every diagram. Archforge asks first and names what goes with it, such as a
  table's keys or a class's relationships.
- Deleting a row in **Models** deletes the element from the model; Archforge asks first when the
  element is on a diagram or takes other elements with it.

When a removal takes other things with it, a message at the bottom of the canvas says what
happened, with **Undo** to take it back.

<!-- picture: workspace-toast light | After a removal, Undo is one click away -->

Next: [A first ER model](04-er-model.md).
