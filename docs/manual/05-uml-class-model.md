# A first UML class model

This chapter builds a small class model: a class *Customer* that places many *Order*s. It can live
in the same project as the ER model of the previous chapter, or in a new one.

Every UML model has one **root package**, named after the project, that holds everything without
another home: the classes and packages you add without choosing a package, and the first class
diagram. You don't need to set it up. Archforge makes it with your first class diagram (or your
first package), and you can rename it like any package.

## 1. Make a class diagram

1. Choose the **+** at the start of the centre's tab strip.
2. In the **New diagram** dialog choose the **Class diagram** card and **Continue**. On the second
   step, **Package** shows your project's name: the root package, made when you choose **Create**.
   Name the diagram *Commerce* and choose **Create**.

The root package is the top row of **Models**, with what it holds below it. Once it exists, the
**Package** field of a later class diagram offers it and every other package, and **New package**
makes a new package inside the root, named like the diagram.

## 2. Add classes

1. Choose the **Class** tool in the toolbar at the bottom, then click on the canvas. A class
   appears with its name ready to type: type *Customer* and press Enter. (The tool places the kind
   its chevron chose; a new window starts at **Class**.)
2. Do the same for *Order*.

A class is a card with its name and two compartments under it: attributes, then operations.

## 3. Type attributes and operations on the card

1. Select *Customer* and press ⌘↩ (Ctrl Enter), or choose the **+** at the right of its attributes
   compartment. A draft row opens on the card, ringed in blue.
2. Type the name *email*, press Tab, and type the type *String*. Press Enter: the attribute is
   added as `email : String`, and a new draft opens for the next one. Press Escape to stop.
3. Press ⇧⌘↩ (Ctrl Shift Enter) for an operation: type *placeOrder*, Tab, its return type *Order*,
   and Enter.

<!-- picture: workspace-inline-member light | Typing an attribute on the card: its name, then its type -->

Built-in types (*Integer*, *String*, *Boolean*, *Real*, *UnlimitedNatural*) are made in a package
*Types* the first time you use one. For visibility, multiplicity, *static*, *abstract* and an
operation's parameters, use the row's **⋯**, which opens the member's editor in **Properties**.
**Properties** of a class lists its **Attributes** and **Operations**, each row with its edit
button.

<!-- picture: workspace-class-members light | A class's attributes and operations in Properties -->

## 4. Draw an association

1. Open the chevron of the **Relationship** tool and choose **Association**. (It also offers
   directed association, aggregation, composition, generalization and dependency, and below a
   line, interface realization.)
2. Press on *Customer* and drag onto *Order*, then let go.

A new association is a plain line: no roles, no multiplicities, navigable to neither end.

## 5. Set multiplicity and navigability

Select the line. **Properties** shows the association's **Name** and a section for each end,
*End at Customer* and *End at Order*:

1. Name the association *places*.
2. Under *End at Order*, type the role *orders*, choose the multiplicity `0..*` from the
   **Multiplicity** menu (or type a number or range, such as `1..5`), and tick **Navigable**.
3. Under *End at Customer*, choose the multiplicity `1`.

<!-- picture: workspace-association light | An association's ends in Properties: role, multiplicity, navigable and aggregation -->

The line now shows an open arrow at *Order*, the end it navigates to, with the role and the
multiplicities beside its ends. **Aggregation** makes the line a shared aggregation (a hollow
diamond) or a composition (a filled one) at the whole's end.

## 6. Add an interface

An interface names what a class offers, without saying how: its attributes and operations, all
public.

1. Open the chevron beside the **Class** tool and choose **Interface** (or use the arrow keys and
   Enter). The tool now shows the interface's lollipop, and its tooltip says *Interface*; it keeps
   that kind for this window until you choose another.
2. Click on the canvas, type *Payable* and press Enter. Its card says `«interface»` above its name.
3. Add its members as for a class: on the card with ⌘↩ and ⇧⌘↩, or in **Properties**.

In **Models**, the **+** of a package offers **Class** and **Interface** each on a row of its own,
and you can drag an interface from Models onto a diagram as you drag a class. A package's classes
and interfaces share its names: an interface cannot be named like a class beside it. An interface is
offered as a type wherever you choose an attribute's or a parameter's type.

The **Relationship** tool draws associations and dependencies to and from interfaces. A
generalization joins an interface to the interface it specializes; a class never specializes an
interface, and an interface specializes nothing but an interface. Such a drag is refused at the
canvas's top, saying why, and draws nothing.

<!-- picture: workspace-interface light | An interface, Payable, with its members, the interface that specializes it and the class associated with it -->

## 7. Realize an interface

A class that offers everything an interface names *realizes* it: it implements the interface.

1. Open the chevron of the **Relationship** tool and choose **Interface realization**, below the
   line in the menu. The tool now shows a dashed line with a hollow triangle at its right, and its
   tooltip says it runs from the implementing class to its interface.
2. Press on the class, *Order*, and drag onto the interface, *Payable*, then let go.

The realization is a dashed line in the muted ink, with a hollow triangle at the interface: it is
neither a generalization (a solid line to its triangle) nor a dependency (a dashed line to an open
arrow). Drawing it is one step you can undo. Several classes may realize the same interface, and
a class may realize several interfaces.

Only a class implements an interface. From an interface, the tool draws nothing, and a drop on
anything but an interface, or a second realization of an interface the class realizes already,
is refused: the reason shows at the canvas's top and nothing is drawn. Let go on empty canvas and
a new interface appears there, its name ready to type, with the realization to it, as one step.

Select the line: **Properties** shows its **Implementing class** and its **Realized interface**.
Drag either end of the selected line onto another class or interface to move it; the same rules
apply. Delete removes the realization, and removing its class or its interface takes it along.

<!-- picture: workspace-realization light | Order and Invoice each realizing the interface Payable, Order's realization selected with its implementing class and realized interface in Properties -->

## 8. Add an enumeration

An enumeration is a type whose values are a fixed list of names, its *literals*: an order's status
is *Placed*, *Paid*, *Shipped* or *Cancelled*.

1. Open the chevron beside the classifier tool and choose **Enumeration**, below **Interface**. The
   tool now shows a short list, and its tooltip says *Enumeration*.
2. Click on the canvas, type *OrderStatus* and press Enter. Its card says `«enumeration»` above
   its name and has one compartment, for its literals; while it has none, the compartment is
   empty.

An enumeration has no attributes or operations of its own: its card has no **+**, its menu offers
no new member, and ⌘↩ adds none. Select it to see it in **Properties**: under its **Name** and
**Package**, **Literals** lists its literals in their order, or says it has none yet. In **Models**
an enumeration is a row under its package with its literals below it. Adding, renaming,
reordering and removing literals comes in a later version; for now they are shown, not edited.

In **Models**, the **+** of a package offers **Enumeration** beside **Class** and **Interface**,
and you can drag an enumeration onto a diagram or onto another package's row as you drag a class.
It shares its package's names with the classes and interfaces there. An enumeration is offered as a
type wherever you choose an attribute's, a parameter's or a return type, and what it types keeps it
when you rename it or move it to another package.

An enumeration is a data type, not something whose instances you link: the **Relationship** tool
draws no association to or from one, and says so at the canvas's top. A generalization joins an
enumeration only to another enumeration, and a dependency may lead to one. Dragged out of an
enumeration onto empty canvas, a generalization makes a new enumeration there.

<!-- picture: workspace-enumeration light | The enumeration OrderStatus with its four literals, selected with its literals in Properties and under it in Models, an enumeration Priority with none yet, and the class Order whose attributes they type -->

## The result

The root package holds two classes and an interface. *Customer* has an attribute and an
operation, and a navigable association *places* leads to many *Order*s. *Order* realizes the
interface *Payable*, a dashed line with a hollow triangle at *Payable*. In Models, open the root
package (your project's name), then a class or the interface, to see its attributes and
operations.

<!-- picture: workspace-class-diagram light | A class diagram of a small commerce model -->

Drag a class's row onto another package's row in Models to move it there; a class of another
package on a diagram is drawn dimmed, its package named above its name. Removing a package hands
what it holds up to the package around it, in the end the root package, which itself stays.
A project made before the root package existed gets one when it opens: its top-level packages move
into it, as one step you can undo.

## Packages on a diagram, and the diagram inside each

A package can sit on a class diagram beside classes, drawn as a card with a tab, like a folder.
The tab holds the package's name and is as wide as the name needs, up to the card's width; a
longer name ends in "…", and pointing at the tab shows it whole. Every package on a diagram leads
to a class diagram of its own, and the card shows that diagram in small: its classes and packages
where they sit there, and the lines between them. When you change that diagram, the card follows.
A package whose diagram has nothing on it says *Empty diagram*. Zoomed out below 50 %, the card
shows the package's name in its middle instead, as every card does.

The tab is part of the card: click it to select the package, drag it to move the card, and double
click it to rename the package there. The tab grows as you type.

- **Add one** with the **Package** tool of the toolbar: click where it goes and type its name. The
  package goes into the diagram's package, and its own diagram, named like it, is made with it.
  Both are one step you can undo. A package dragged from Models onto the diagram of the package
  that holds it lands there too, and gets its diagram if it has none yet.
- **Go in.** Select the package and choose **Open** on its card, or press ⌘↓ (Ctrl+↓). Or zoom in
  on it, with a pinch, the scroll wheel with ⌘ (Ctrl), the zoom buttons, or + while the canvas has
  the keyboard. Once the package covers most of the view, its diagram opens. However you go in,
  the package's diagram opens at 100 %, what it shows centred. A double click still renames the
  package, as it does a class.
- **Come back by zooming out.** A package's diagram stays open while you zoom out, down to the
  least zoom, 10 %. Reach 10 % and its parent's diagram opens, at 100 %, with the package you were
  in at its centre. Zoom out to 10 % again to go up another level. At your project's root there is
  no parent: the diagram stays at 10 %, and a note under the path bar says so.
- **Come back by the path bar.** The path bar at the canvas's top left names the packages from
  your project's root down to the one open. Choose one of them, or press ⌘↑ (Ctrl+↑) for the
  parent; it comes back as you left it.

One gesture moves one level: after a switch, keep zooming and you stay where you landed. Pause,
then zoom again to go on.
Renaming a package renames its diagram too, as long as the diagram still carries the package's
name.

<!-- picture: workspace-packages light | A class diagram of the root package: Customer and two packages, Sales selected with its Open and its own diagram in small, Billing's diagram empty -->

<!-- picture: workspace-inside-package dark | Inside Sales: its classes, its own package Payments, and the path bar back to the root -->

<!-- picture: workspace-package-top light | The root's diagram zoomed out to 10 %: the note under the path bar says there is no package above -->

### Move a package

A package moves into another package with everything it holds and its own diagram. Four ways
lead there:

- **On a diagram**, drag its card onto another package's card. While you hold it there, that
  card is ringed and a label says what the drop does (*Move Sales into Billing*). Let go, and the
  package leaves this diagram for the diagram of its new home, and a toast offers **Undo**.
- **Up the path bar**: drag a card onto one of the packages named in the path bar, such as your
  project's root, to move it up there.
- **In Models**, drag its row onto another package's row, or onto a class there.
- **From the keyboard**, select it, and in **Properties** choose its **Parent package**. The menu
  lists every package it may go into, never itself nor one it holds.

A package can't go into itself or into a package it holds. In Models, such a row is ringed in
amber while you hold the package over it, and the drop is refused with the reason. Let go anywhere
else and the package only moves on the diagram; the model is not changed. Each move is one step
you can undo, and the project keeps it once saved.

<!-- picture: workspace-package-parent light | Properties for the package Sales: its Parent package menu lists the root and Billing -->

Next: [Skills](06-skills.md).
