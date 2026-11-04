# The workspace

An open project fills the window with its workspace.

<!-- picture: workspace-diagram light | The workspace: Models on the left, a diagram in the centre, Properties on the right -->

- **The bar at the top** holds Home, the open projects' tabs, the search field, Help and your
  avatar (which opens Settings, where you enter your name).
- **The left rail** switches the left column between **Diagrams**, at the top, and **Models**. (Source Control
  is planned and not available yet.)
- **The centre** shows the open diagrams and skills, each in a tab. The **+** at the start of the
  tab strip makes a new diagram.
- **The right rail** opens the right panel on **Chat**, **Appearance**, **Properties** or
  **Skills**. Clicking the rail icon of the open column folds it away; clicking any icon opens it
  again.
- **The status bar** at the bottom says whether the project is saved, and where the project is.

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
