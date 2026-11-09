# The workspace

An open project fills the window with its workspace.

<!-- picture: workspace-diagram light | The workspace: Models on the left, a diagram in the centre, Properties on the right -->

- **The bar at the top** holds Home, the open projects' tabs, the search field, Help and your
  avatar (which opens Settings, where you enter your name; it is saved with each version, so
  anyone given a copy of the project can read it).
- **The left rail** switches the left column between **Diagrams**, at the top, **Models** and **Versions**,
  which lists the project's history, newest first.
- **The centre** shows the open diagrams and skills, each in a tab. The **+** at the start of the
  tab strip makes a new diagram (see *New diagram* below).
- **The right rail** opens the right panel on **Chat**, **Appearance**, **Properties** or
  **Skills**. Clicking the rail icon of the open column folds it away; clicking any icon opens it
  again.
- **The status bar** at the bottom says whether the project is saved, and where the project is.

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
it, Escape to cancel.

An element exists once in the model and can appear on many diagrams. Drag an element from Models
onto a diagram to show it there.

## Properties and Appearance

**Properties** shows the selected element's model data: its name, its kind, a description, and
sections such as a table's **Columns · 4** or a class's **Attributes · 2**. These are the same on
every diagram. Each row has an edit button (a pencil) at its right end; it opens the row's editor,
which changes nothing until you apply it with the check at its foot (or Enter). The cross (or
Escape) cancels, and the red bin removes the row.

Type a description in the **Description** field and press Enter, or leave the field, to keep it;
empty the field to remove it. Each change is one undo step. In a UML model the description is
kept as a comment the element owns: it goes with the element when you remove it, and an agent
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
one step.

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
