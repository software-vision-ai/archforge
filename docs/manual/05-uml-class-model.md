# A first UML class model

Archforge models a software system. A model has several aspects: the previous chapter drew one,
the system's database. This chapter draws another, the classes of its code, on a UML class
diagram. A class diagram shows the things the software keeps track of, what each one knows and
does, and how they are connected.

You build one small example from start to end: a shop's customers, the orders they place, an
order's status and its total in money. Each step adds one kind of element. It says what the
element is for, with an everyday example first and its UML name second, where you make it, where
you change it, and what you see afterwards.

Use the project you made in [Getting started](02-getting-started.md). The tables of the
[previous chapter](04-er-model.md) can stay: the class model sits beside them in the same project.

## The tools you will use

You make and change a class model in four places. Each step below says which one to use.

- **The classifier tool** sits in the toolbar at the bottom of a class diagram, after **Select**
  (the arrow) and **Pan** (the hand). It places a new card on the canvas. Its small chevron opens
  the kinds menu: **Class**, **Interface**, **Enumeration**, **Data type** and **Primitive type**.
  UML calls all of these *classifiers*. The tool shows the icon of the kind it places (for a
  class, a small card with lines across it), and its tooltip names that kind. Archforge keeps the
  kind you choose until you choose another; when you start Archforge again, it is **Class**.
- **The Relationship tool** sits after the **Package** tool (the folder), and shows the line it
  draws. It draws a line from one card to another. Its chevron opens a menu of kinds of line.
- **Models**, in the left column, lists the model as a tree: its packages, the cards in them, and
  their members, whether or not a diagram shows them. Lines are not listed there; select them on
  a diagram. Its **+** adds an element next to the selected one, with its name ready to type.
- **Properties**, in the right column, shows the selected element: its name, its package, and
  sections such as **Attributes**, **Operations** or **Literals**. Each row there has a pencil
  that opens its editor (see [The workspace](03-workspace.md#properties-and-appearance)).

A selected card shows two small square buttons beside it. The one at its top right is
**Connect**: drag from it onto another card to draw the Relationship tool's current kind of line.
The one at its bottom right adds a member, such as an attribute.

After it places a card or draws a line, a tool hands back to **Select**.

Keys are written for macOS, with Windows and Linux in brackets: ⌘↩ (Ctrl Enter) means the
Command key and Return on a Mac, and the Control key and Enter elsewhere.

## 1. Make a class diagram

1. Choose the **+** at the start of the centre's tab strip. The **New diagram** dialog opens on
   its first step, a card for each kind of diagram.
2. Choose the **Class diagram** card, then **Continue**. The second step asks for a **Name** and
   a **Package**.
3. In **Name**, type *Commerce*.
4. Leave **Package** as it is. It shows your project's name: this is the *root package*.
5. Choose **Create**.

An empty diagram opens in a tab named *Commerce*. The path bar at the canvas's top left shows the
package the diagram belongs to: your project's name. In **Models**, the top row has the same
name. That row is the root package: every class model has exactly one, and it holds everything
that you don't put in a package of your own. Archforge makes it with your first class diagram, so
you never set it up. **Properties** calls it *Model · the project's root*.

What you make in this chapter is part of the *model*: it exists once, whichever diagrams show it.
That is why **Properties** describes a class as *Class · model, in every diagram*: a change there
shows on every diagram that has the class.

## 2. Add the classes Customer and Order

A *class* describes one kind of thing your software keeps track of, such as a customer or an
order. Each real customer of your shop is one *instance* of the class *Customer*.

1. Choose the classifier tool. Its tooltip says **Class**. (If it shows another kind, open its
   chevron and choose **Class**.)
2. Click on the canvas, towards the left. A card appears with its name ready to type.
3. Type *Customer* and press Enter.
4. Choose the classifier tool again and click to the right of *Customer*.
5. Type *Order* and press Enter.

Each class is a card with *CLASS* above its name and two empty compartments under it: the upper
one for attributes, the lower one for operations. **Models** lists *Customer* and *Order* under
the root package. (The other kinds you make later show a UML keyword in « » above their name
instead, such as `«enumeration»`.)

To rename a class, double-click its name on the card, or change **Name** in **Properties**.

## 3. Say what a class knows and does: attributes and operations

An *attribute* is a fact that every instance holds, such as an order's number. An *operation* is
something an instance can do, such as paying.

1. Select *Customer*.
2. Press ⌘↩ (Ctrl Enter). A draft row opens in the attributes compartment, ringed in blue. (A
   selected card also shows a **+** at the right of each compartment, and the **Add attribute**
   button at its bottom right; either does the same.)
3. Type the name *name*.
4. Press Tab to move to the type.
5. Type *String*.
6. Press Enter. The row reads `+ name : String`, and a new draft row opens under it.
7. Press Escape to close the empty draft.
8. Select *Order* and add the attribute *number* of type *Integer* in the same way.

<!-- picture: workspace-guide-classes light | Order selected with a draft row open on its card: the name number, then the type Integer; Customer beside it with its attribute name : String -->

9. With *Order* still selected, press ⇧⌘↩ (Ctrl Shift Enter). A draft row opens in the
   operations compartment.
10. Type *pay*, press Tab, type the return type *Boolean*, and press Enter.
11. Press Escape.

*Order*'s card now reads `+ number : Integer` and `+ pay() : Boolean`. The `+` means the member
is public: other classes can use it.

*String*, *Integer*, *Boolean*, *Real* and *UnlimitedNatural* are built-in types. Archforge keeps
them in a package named *Types*, which it made the first time you used one. You see *Types* in
**Models**, but not the types inside it.

Select a class to edit it in **Properties**. It shows a **Description**, the **Name** and
**Package**, **Abstract** (tick it for a class that has no instances of its own, only more
specific classes), **No subclasses (final)** (see
[Forbidding subclasses: final](#forbidding-subclasses-final)) and **No redefinition (leaf)** (see
[Forbidding redefinition: leaf](#forbidding-redefinition-leaf)). Then come **Attributes** and
**Operations**, each with a **+** and a pencil on every row. The pencil opens the member's editor. There you set what the card row cannot hold, such
as the visibility (who may use the member) and an operation's parameters. Apply the change with
the check at the editor's foot, or Enter; cancel it with the cross, or Escape. The red bin
removes the member. On a draft row, the **⋯** at its right opens the same editor.

The editor's **Static** marks a member that belongs to the class itself, not to each instance. Its
row is underlined on every card that shows the class, and in **Properties**; clear **Static** and
the underline goes. An operation's **Abstract** slants its row, so a static abstract operation is
both underlined and slanted. **No redefinition (leaf)** writes `{leaf}` after the member's row.

## 4. Connect Customer and Order: an association

A customer places orders. An *association* says that instances of two classes are linked like
this. Its ends say how many instances take part (the *multiplicity*) and which way you can follow
the link (*navigability*).

1. Open the chevron beside the **Relationship** tool. The menu lists the kinds of line, the
   current one highlighted.
2. Choose **Association**.

<!-- picture: workspace-guide-relate-kinds light | The Relationship tool's menu open over Customer and Order: Association highlighted, then the other kinds of line, and Interface realization and Association class below a line -->

3. Press on *Customer*, drag onto *Order*, and let go.

A plain line joins the two cards. It is selected, and **Properties** shows it: its **Name**, its
two classes under **From** and **To**, and a section for each end, **End at Customer** and
**End at Order**. A new association has no roles and no multiplicities, and can be followed in
neither direction. Each end also has **Aggregation** and **Modifiers**; leave them as they are.

4. In **Name**, type *places* and press Enter.
5. Under **End at Order**, type *orders* in **Role** and press Enter. A role is the name an end
   has as seen from the other class: a customer's *orders*.
6. Under **End at Order**, open **Multiplicity** and choose `*`. A star means any number,
   including none.
7. Under **End at Order**, tick **Navigable**.
8. Under **End at Customer**, open **Multiplicity** and choose `1`.

<!-- picture: workspace-guide-association light | The association places selected between Customer and Order: an open arrow at Order with the role orders and *, 1 at Customer; Properties shows its name and both ends -->

The line now has an open arrow at *Order*, the end you can navigate to. (While the line is
selected, each end's handle sits on the line just past the end's mark, so the arrow shows whole.)
*places* sits at its middle, *orders* and `*` at *Order*'s end, and `1` at *Customer*'s end. Read it as: one customer
places any number of orders, and from a customer you can find their orders.

The **Multiplicity** menu offers `0..1`, `1`, `*` and `1..*`, and also accepts a number or a range
you type, such as `1..5`. `0..*` means the same as `*`, so the menu offers it once, as `*`; typed,
it is shown as `*` too.

The menu's other kinds of line, for later:

- **Directed association**: an association drawn already navigable towards its second end.
- **Aggregation** and **Composition**: an association between a whole and its parts, drawn with
  a hollow or a filled diamond at the whole. In a composition the parts belong to that one whole.
  To change an association into either, set **Aggregation** to **Shared** or **Composite** on the
  part's end in **Properties**; the diamond then shows at the other end, the whole's.
- **Generalization**: one class is a more specific kind of another, as a card payment is a kind
  of payment. It is drawn as a solid line with a hollow triangle at the general class.
- **Dependency**: one element uses another, drawn as a dashed line with an open arrow.
- **Interface realization** and **Association class**: steps 9 and 10.

## 5. Give an order a status: an enumeration

An order is always *Placed*, *Paid*, *Shipped* or *Cancelled*, and nothing else. A type whose
values come from a fixed list of names is an *enumeration*. The names in the list are its
*literals*.

### Make the enumeration

1. Open the chevron beside the classifier tool. The kinds menu opens, with **Class**, the current
   kind, ticked.

<!-- picture: workspace-guide-classifier-kinds light | The classifier tool's kinds menu open: Class ticked, Enumeration highlighted below Interface, then Data type and Primitive type -->

2. Choose **Enumeration**. (You can also move with the arrow keys and press Enter.) The tool now
   shows a short list, and its tooltip says **Enumeration**.
3. Click on the canvas below *Order*.
4. Type *OrderStatus* and press Enter.

The card says `«enumeration»` above its name. It has one compartment, for its literals, which is
empty for now. An enumeration has no attributes or operations: its literals are its members.

### Add the literals inside the enumeration

You add an enumeration's values on its own card, or in its own **Literals** section.

1. Select *OrderStatus*.
2. Press ⌘↩ (Ctrl Enter). A draft row opens at the end of its compartment. (The **+** at the
   right of the compartment, and the **Add literal** button at the card's bottom right, do the
   same.)
3. Type *Placed* and press Enter. The literal is added, and a new draft row opens under it.
4. Type *Paid* and press Enter.
5. Type *Shipped* and press Enter.
6. Type *Cancelled* and press Enter.
7. Press Escape to close the empty draft.

<!-- picture: workspace-guide-literals light | OrderStatus selected below Order, with Placed, Paid and Shipped on its card and a draft row with Cancelled typed at its end; Properties lists its literals, each with a pencil -->

The card shows the four literals in order. **Properties** lists them under **Literals · 4**, and
**Models** shows them under *OrderStatus*.

### Use the enumeration as a type

1. Select *Order*.
2. Press ⌘↩.
3. Type *status*, press Tab, type *OrderStatus*, and press Enter.
4. Press Escape.

*Order*'s card now reads `+ status : OrderStatus`. Every class, interface, enumeration, data type
and primitive type you make is offered wherever you choose a type: an attribute's, a parameter's
or an operation's return type.

### Change the literals

- **Rename**: double-click a literal on the card and type the new name. Two literals of one
  enumeration cannot share a name, whatever the case: typing *paid* beside *Paid* says "There is
  a literal named Paid already." under the row and changes nothing.
- **Edit, move or remove**: in **Properties**, choose a literal's pencil, or double-click its
  row. Its editor holds its **Name**, the bin that removes it, and arrows that move it up or
  down. Apply with the check or Enter; cancel with the cross or Escape.
- **In Models**: with *OrderStatus* or one of its literals selected, **+** offers **Literal**
  first. Click a selected literal's row again, or press F2, to rename it; Delete removes it.

A literal stays the same literal when you rename or move it, so whatever refers to it keeps
working.

## 6. Hold an amount of money: a data type

An order's total is an amount of money, such as 5 euros. Two amounts of 5 euros are the same
value, wherever they appear. An order is different: it stays the same order when its status
changes. A value without an identity of its own is a *data type*. Like a class, it can have
attributes and operations.

1. Open the classifier tool's chevron and choose **Data type**. The tool now shows a pair of
   braces.
2. Click on the canvas to the right of *OrderStatus*, type *Money*, and press Enter.
3. With *Money* selected, press ⌘↩, type *amount*, press Tab, type *Real*, and press Enter.
4. Type *currency*, press Tab, type *String*, and press Enter.
5. Press Escape.
6. Select *Order*, press ⌘↩, and add *total* of type *Money*. Press Escape.

<!-- picture: workspace-guide-data-type light | The data type Money selected, its attributes amount and currency on its card and in Properties; Order's total typed Money, and OrderStatus beside it -->

The card says `«dataType»` above its name and has two compartments, as a class has.
**Properties** shows its **Description**, **Name** and **Package**, then **Attributes** and
**Operations**. A data type has no **Abstract**.

## 7. Name a plain value: a primitive type

An email address is text, but not just any text. A *primitive type* gives a plain, single value
its own name. It has nothing inside it: no attributes and no operations. The built-in types, such
as *String* and *Integer*, are primitive types too; this one is your own.

1. Open the classifier tool's chevron and choose **Primitive type**. The tool now shows a small
   box with a ring inside.
2. Click on the canvas below *Customer*, type *Email*, and press Enter.
3. Select *Customer*, press ⌘↩, and add *email* of type *Email*. Press Escape.

<!-- picture: workspace-guide-primitive-type light | The primitive type Email selected below Customer, its card its name alone; Customer's email typed Email; Properties shows Email's name, package and description -->

The card says `«primitive»` above its name and has no compartment. Since there is nothing to add,
a selected primitive type shows no **+** and no **Add attribute** button, and ⌘↩ does nothing.
**Properties** shows its **Description**, **Name** and **Package**, and nothing more.

## 8. Promise an operation: an interface

Orders can be paid, and later other things might be too, such as invoices. An *interface* is a
promise: it names what a class offers, without saying how. The interface *Payable* promises the
operation *pay*.

1. Open the classifier tool's chevron and choose **Interface**. The tool now shows a small circle
   on a stem.
2. Click on the canvas above *Order*, type *Payable*, and press Enter.
3. With *Payable* selected, press ⇧⌘↩ (Ctrl Shift Enter), type *pay*, press Tab, type
   *Boolean*, and press Enter.
4. Press Escape.

The card says `«interface»` above its name. It has the same two compartments as a class, and you
add members to them in the same ways. All of an interface's members are public.

## 9. Keep the promise: interface realization

*Order* already has the operation *pay* that *Payable* names. To say that *Order* keeps
*Payable*'s promise, draw an *interface realization* from the class to the interface.

1. Open the chevron beside the **Relationship** tool and choose **Interface realization**, below
   the line in the menu. The tooltip now says it runs from the implementing class to its
   interface.
2. Press on *Order*, drag onto *Payable*, and let go.

<!-- picture: workspace-guide-realization light | Order's realization of Payable selected, a dashed line from Order up to Payable; Properties shows its implementing class Order and realized interface Payable -->

A dashed line joins them, with a hollow triangle at *Payable*, the end handle just past it while
the line is selected. It is selected, and **Properties** shows its **Implementing class**,
*Order*, and its **Realized interface**, *Payable*. A realization has no name, so the top of
**Properties** shows only its kind. It differs from a generalization (a solid line to a triangle)
and from a dependency (a dashed line to an open arrow).

Only a class, or an association class (step 10), realizes an interface. A class can realize
several interfaces, and several classes can realize one interface. If you let go on empty canvas
instead, a new interface appears there with its name ready to type, already realized.

## 10. Give a link facts of its own: an association class

An order holds products, and for each product it holds a quantity. The quantity belongs neither
to the order nor to the product: it belongs to the link between them. An *association class* is
such a link: an association that is also a class, with attributes of its own.

First add the class *Product*:

1. Open the classifier tool's chevron and choose **Class**.
2. Click on the canvas to the right of *Order*, type *Product*, and press Enter.
3. With *Product* selected, press ⌘↩ and add *name* of type *String*. Press Escape.

Then draw the association class:

4. Open the chevron beside the **Relationship** tool and choose **Association class**, the last
   row.
5. Press on *Order*, drag onto *Product*, and let go. A line joins them, and a card appears above
   the line's middle with its name ready to type.
6. Type *OrderLine* and press Enter.
7. With *OrderLine* selected, press ⌘↩ and add *quantity* of type *Integer*. Press Escape.
8. In **Properties**, under **End at Order**, choose the **Multiplicity** `*`.
9. Under **End at Product**, choose the **Multiplicity** `*`.

<!-- picture: workspace-guide-association-class light | OrderLine selected: its card with quantity : Integer above the line between Order and Product, tied to the line's middle by a dashed line; Properties shows its name, its classes Order and Product, and its ends -->

The card says *ASSOCIATION CLASS* above its name, and a dashed tie joins it to the middle of the
line. The line, the card and the tie are one element: click either the line or the card to select
it. **Properties** shows its **Description** and **Name**, its two classes (**From** and **To**,
which you cannot change here), a section for each end as an association has, and then
**Attributes** and **Operations** as a class has.

Read it as: an order holds any number of products, a product can be in any number of orders, and
for each order and product an *OrderLine* says how many.

You can drag the card anywhere; the tie follows. An association class joins only classes. It is
made only by drawing, so the **+** of **Models** does not offer it.

## 11. Fix a value or set a starting value: property modifiers

An attribute can say more than its name and type. An order's number never changes once it is
given: it is *read only*. A new order starts as *Placed*: that is its status's *default value*.
These extra facts are *property modifiers*.

1. Select *Order*.
2. In **Properties**, under **Attributes**, choose the pencil on *number*. Its editor opens.
3. Tick **Read only**.
4. Choose the check at the editor's foot, or press Enter.
5. Choose the pencil on *status*.
6. Open **Default value** and choose *Placed*. The menu lists *OrderStatus*'s literals.
7. Choose the check, or press Enter.

<!-- picture: workspace-guide-modifiers light | Order selected, its card reading number : Integer {readOnly} and status : OrderStatus = Placed; Properties shows the attribute editor on status, its Default value Placed, the flags, Subsets and Redefines -->

*Order*'s card now reads `+ number : Integer {readOnly}` and `+ status : OrderStatus = Placed`.
**Unique** is ticked in the editor because it is on unless you turn it off; it matters only for
an attribute that holds several values. The editor holds more modifiers: **Static**,
**Derived**, **Ordered**, **Unique**, **Subsets** and **Redefines**.
[Property modifiers](#property-modifiers) in the reference explains each one.

## 12. State a rule: constraints

Some facts are rules rather than values: an order's number is always above zero, and an order
that is paid has a total above zero. Such a rule is a *constraint*. Archforge keeps its text and
the elements it is about, exactly as you write it, and never evaluates it: nothing is checked
against the model. Write it in any language you like, OCL, plain English or another.

1. Select *Order*.
2. In **Properties**, choose **+** beside **Constraints**. The editor opens with *Order* among the
   **Constrained elements**.
3. Type *positiveNumber* as the **Name** and `number > 0` as the **Expression**. In **Language**,
   pick **OCL**, or type any other language; left empty, it is plain text.
4. Choose the check at the editor's foot. A short rule shows in braces under *Order*'s card:
   `{number > 0}`.
5. Choose **+** again. Name it *paidInFull*, and type a longer rule on two lines:
   `status = OrderStatus::Paid` and `implies total.amount > 0`.
6. Choose **+** beside the chips of **Constrained elements** and pick *OrderStatus*.
7. Under **On this diagram**, choose **Linked note**, then the check.

<!-- picture: workspace-guide-constraints light | Order selected: {number > 0} in braces under its card, and the constraint note paidInFull, its eyebrow CONSTRAINT · NOT EVALUATED, linked by dashed lines to Order and OrderStatus; Properties shows Constraints · 2 with the editor open on paidInFull, its language OCL, its expression on two lines, Order and OrderStatus as chips and Linked note chosen -->

The note says *CONSTRAINT · NOT EVALUATED*, and dashed lines join it to *Order* and
*OrderStatus*. It looks different from a plain note on purpose: a plain note is your remark on the
diagram, a constraint is part of the model.

A constraint belongs to the model, so every diagram shares its text; how it is shown is each
diagram's own choice. [Constraints](#constraints) in the reference says more.

## The result

Your model now has three classes, an enumeration, a data type, a primitive type, an interface, an
association, an association class and two constraints, all in the root package:

- *Customer* places any number of *Order*s, and has an *email* of the type *Email*.
- *Order* has a read-only *number*, a *status* that starts as *Placed*, and a *total* in *Money*.
  It realizes *Payable*. Its number is always above zero, and a paid order has a total above
  zero.
- *OrderLine* links an order to its products and holds each product's *quantity*.

<!-- picture: workspace-guide-result light | The finished Commerce diagram: Customer, Order and Product in a row, Payable and OrderLine above, Email, OrderStatus and Money below; every element listed in Models -->

The picture has **Properties** folded away, so the whole diagram fits: click the open column's
icon on the right rail to fold it, and any icon there to open it again. In **Models**, open a
row's arrow to see its attributes, operations or literals.

Since you began editing, the status bar at the bottom has said *Unsaved changes · ⌘S saves*.
Archforge writes each change to the project on its own, a moment after you stop editing, so what
it has written is kept even if Archforge closes unexpectedly. If a write fails, the status bar
says *Changes could not be written to the project* instead. Saving is a separate step that keeps
a version of the project as it is now. Press ⌘S (Ctrl S): the status bar then says *Saved* (see
[Getting started](02-getting-started.md#saving)).

## Which element do I need?

| You want to describe | Use | Example |
|---|---|---|
| A thing your software keeps track of, which stays itself while its facts change | Class | *Order*, *Customer* |
| A promise of operations that several classes can keep | Interface | *Payable* |
| A value chosen from a fixed list of names | Enumeration | *OrderStatus* |
| A value made of parts, with no identity of its own | Data type | *Money* |
| A single plain value with a name of its own | Primitive type | *Email* |

Between two classes:

- Use an **association** when the link itself has no facts: a customer *places* orders.
- Use an **association class** when the link has facts of its own: the *quantity* of a product in
  an order.

If you are unsure between a class and a data type, ask: are two with equal facts the same thing?
Two amounts of 5 euros are the same value, so *Money* is a data type. Two customers with the same
name are still two customers, so *Customer* is a class.

## Organise the model in packages

So far everything sits in the root package. A *package* groups elements, like a folder. A large
model is easier to read when each area of the system has its own package, such as *Sales* and
*Billing*. The pictures in this part show a separate example project, whose root diagram holds
*Customer* and the packages *Sales* and *Billing*.

### Packages on a diagram, and the diagram inside each

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
  on it, with a pinch, the scroll wheel with ⌘ (Ctrl), the zoom buttons, or the + key after
  clicking on empty canvas. Once the package covers most of the view, its diagram opens. However you go in,
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
then zoom again to go on. Renaming a package renames its diagram too, as long as the diagram
still carries the package's name.

<!-- picture: workspace-packages light | A class diagram of the root package: Customer and two packages, Sales selected with its Open and its own diagram in small, Billing's diagram empty -->

<!-- picture: workspace-inside-package dark | Inside Sales: its classes, its own package Payments, and the path bar back to the root -->

<!-- picture: workspace-package-top light | The root's diagram zoomed out to 10 %: the note under the path bar says there is no package above -->

### Move an element into a package

Drag a classifier's row in **Models** onto a package's row to move it there, or pick the package
as its **Owner** in **Properties** (see [Nested classifiers](#nested-classifiers)). A classifier of
another package on a diagram is drawn dimmed, its package named above its name. Removing a
package hands what it holds up to the package around it, in the end the root package, which
itself stays.

### Move a package

A package moves into another package with everything it holds and its own diagram. Four ways
lead there:

- **On a diagram**, drag its card onto another package's card. While you hold it there, that
  card is ringed and a label says what the drop does (*Move Sales into Billing*). Let go, and the
  package leaves this diagram for the diagram of its new home, and a message at the bottom of the
  canvas offers **Undo**.
- **Up the path bar**: drag a card onto one of the packages named in the path bar, such as your
  project's root, to move it up there.
- **In Models**, drag its row onto another package's row, or onto a class there.
- **In Properties**, select it and choose its **Parent package**. The menu
  lists every package it may go into, never itself nor one it holds.

A package can't go into itself or into a package it holds. In Models, such a row is ringed in
amber while you hold the package over it, and the drop is refused with the reason. A card you
drag on a diagram and let go anywhere but on a package only changes its place on the diagram; the
model is not changed. Each move is one step
you can undo, and the project keeps it once saved.

<!-- picture: workspace-package-parent light | Properties for the package Sales: its Parent package menu lists the root and Billing -->

## Reference

The walkthrough above shows the usual way to do each thing. This part collects the other ways and
the rules, to look up when you need them.

### Property modifiers

A few words first. A *property* is an attribute or an association's end: both have a name, a
type and a multiplicity. A class *specializes* another when a generalization runs from it to the
other. A *composite* end is a part's end whose **Aggregation** is **Composite**; the filled
diamond is drawn at the other end, the whole's. A multiplicity's *upper bound* is its largest
number: 5 in `1..5`, and no limit in `*`. On a card, an attribute's multiplicity stands in
brackets after its type, as in `phones : String [1..*]`.

An attribute's modifiers are in its editor in **Properties**, under **Type** and
**Multiplicity**:

- **Default value** is typed as text and read by the attribute's type:
  - an *Integer* takes a whole number, such as 42;
  - a *Real* takes a number, such as 2.5;
  - a *Boolean* takes true or false;
  - an *UnlimitedNatural* takes a whole number of 0 or more, or `*`;
  - a *String* takes any text;
  - an enumeration offers its literals to choose from. If the literal is renamed later, the
    default follows it; if the literal is removed, the default goes with it;
  - any other type, or no type, takes the text as written.
- **Static**: the attribute belongs to the class as a whole, not to each instance. The card
  underlines its row.
- **Derived**: the value is worked out from other values, such as a total from the lines.
- **Read only**: the value cannot change once set.
- **No redefinition (leaf)**: no property of a more specific class may redefine this one. See
  [Forbidding redefinition: leaf](#forbidding-redefinition-leaf).
- **Ordered** and **Unique** matter only when the attribute holds more than one value, that is,
  when its multiplicity's upper bound is above 1. You can set them either way, but the card shows
  them only then. **Unique** is on unless you turn it off.
- **Subsets** names another property whose values include all of this one's values. It offers
  only the properties this attribute may name: other properties of its class, and of the classes
  its class specializes, whose type this attribute's type is or specializes, which allow at least
  as many values, and which have another name.
- **Redefines** names a property of a class its class specializes that this one replaces. It
  offers only properties of those classes whose multiplicity holds this attribute's, and that are
  composite only if this attribute is too. A property marked **No redefinition (leaf)** is listed
  last with `{leaf}` beside it; choosing it changes nothing and the reason shows under
  **Redefines**.
- When **Subsets** or **Redefines** has nothing to offer, a muted line under it says so.

The check (or Enter) writes every change in the editor as one step you can undo; the cross (or
Escape) leaves the model as it was. A value that does not fit is refused under its field, and the
editor stays open. This includes a new **Type** that the kept default does not fit: the reason
shows under **Type**, so a default is never dropped without you choosing so. Change or clear the
default first, then the type.

The card writes modifiers the UML way:

- `/` before the name of a derived attribute;
- `=` and the default after the multiplicity; a String's default is in double quotes;
- in braces, what differs from the usual: `{leaf}`, `{readOnly}`, `{ordered}`, `{nonunique}`,
  `{subsets name}` and `{redefines name}`. Unique is the usual, so it is not written;
- a static attribute's or operation's whole row underlined, as `create() : Order` in the picture.

An association's end has the same modifiers, under **Modifiers** in its section of
**Properties**. Closed, the line beside **Modifiers** sums them up. Open it to edit them, then use
its own check and cross. The role, multiplicity, navigability and aggregation above it are written
as before, and keep the modifiers. An end's **Subsets** and **Redefines** offer the properties of
the class at the other end of the association, and of the classes that class specializes. Beside
the line, the end reads in the same order: `/` before a derived end's role, then its
multiplicity, `= default` and the braces, as in `/items` with `* = none {ordered}`.

The picture shows a separate, fuller example in a package *Orders*, where *OrderLine* is a plain
class rather than the walkthrough's association class: a derived *total*, an ordered list of
*lines*, and an association end with modifiers.

<!-- picture: workspace-modifiers light | The class Order on its card with id {readOnly}, a derived total defaulting to 0, status defaulting to Placed and ordered lines, and the static operation create() : Order underlined, its end at OrderLine reading /items with * = none {ordered}; Order selected with the attribute editor open on total, showing Default value, the flags, and Subsets and Redefines -->

### Moving a line's end

Select a line, then drag one of its ends onto another card to move that end. The same rules apply
as when you draw the line. When you move an end of an association or an association class, any
**Subsets** or **Redefines** entry that can no longer reach its property from there is dropped in
the same step. The canvas names what was dropped, and you can undo it. For an interface
realization, drag the class end onto another class, or the interface end onto another interface.
A generalization in a generalization set keeps its ends: take it out of its sets first (see
[Generalization sets](#generalization-sets)).

### What each line can join

When a line is not allowed, nothing is drawn and the reason shows at the top of the canvas.

| Line | Can join |
|---|---|
| Association, directed association, aggregation, composition | Classes and interfaces. Never an enumeration, a data type or a primitive type. |
| Generalization | Two elements of the same kind: a class with a class, an interface with an interface, an enumeration with an enumeration, a data type with a data type, a primitive type with a primitive type. A class never specializes an interface. Nothing specializes an association class, nor an element marked **No subclasses (final)**. |
| Dependency | Any two classifiers, including enumerations, data types and primitive types. |
| Interface realization | From a class (or an association class) to an interface, once per pair. |
| Association class | Two classes, never the same class at both ends. |

When you let go on empty canvas, the line makes a new card there, its name ready to type: a new
interface for an interface realization, a new class for an association class, and for a
generalization a new element of the kind it started from.

### Names, types and packages

- All classifiers of one package share its names: a class, an interface, an enumeration, a data
  type, a primitive type and an association class in the same package need different names.
- Names are told apart by kind, as UML does, and compared without case. Two elements of one kind
  in one owner need different names: two classifiers, two packages, two attributes, two
  operations, two literals, two parameters or two generalization sets. Elements of different kinds
  may share a name: a package may hold a package *Billing* beside a class *Billing*, and a class
  may have an attribute *total* and an operation *total()*. Relationships, constraints and
  diagrams may be named alike, since their names are labels.
- An association class's attributes and its ends' roles share its own names. A clash is refused
  under the attribute's row, or in **Properties** for a role.
- Every class, interface, enumeration, data type and primitive type you made is offered once
  wherever you choose a type. Associations, association classes included, are not types: the type
  menu does not list them, and a type typed on a card does not find them. What a type types keeps
  it when you rename it or move it to another package, and loses it when you delete it, until you
  undo.
- If one of your types has the same name as a built-in type (say you made your own *Integer*),
  the type menu lists both with a muted note: *built-in* beside the built-in one, the package
  beside yours. Typed by name on a card, a name that several types share is refused until you pick
  one from the menu.
- In the package *Types*, the built-in names are kept for the built-in types. *Types* keeps its
  name and its place directly under the root package while it holds built-in types, and
  **Delete** on it in **Models** says so instead of removing it. The built-in types do not appear
  in **Models** or in the search.
- A project made before the root package existed gets one when it opens: its top-level packages
  move into it, as one step you can undo.
- **Models**' **+**, with a package or an element in it selected, offers **Class**, **Interface**,
  **Enumeration**, **Data type**, **Primitive type** and **Package**, and **Literal** first at an
  enumeration. Drag a row from **Models** onto a diagram to show it there, or onto another
  package's row to move it.

### Forbidding subclasses: final

Tick **No subclasses (final)** in the **Properties** of a class, an interface, an enumeration, a
data type or a primitive type of your own when nothing may be a more specific kind of it. Its card
then shows `{final}` after its name, on every diagram. It is a separate setting from **Abstract**,
and from a later **No redefinition (leaf)**: it only stops other elements from specializing this
one, and the element itself may still specialize another.

While it is ticked, a generalization drawn to the element is refused, and so is dragging the
general end of an existing generalization onto it: the line is not drawn and the top of the
canvas says *PricingService is final: nothing may specialise it. Clear "No subclasses" in its
Properties first.*

You can only tick it while nothing specializes the element yet. Otherwise the box stays clear,
nothing changes, and **Properties** names the elements that specialize it, with a button that
selects their generalizations. Delete those generalizations from the model, then tick the box
again. Clearing the box later allows new generalizations again and changes nothing else. Each
change is one step you can undo, and the project keeps it once saved.

<!-- picture: workspace-final light | PricingService with {final} after its name on its card; Payment selected, No subclasses refused in its Properties: it names its subclass CardPayment and offers to select the generalization -->

### Forbidding redefinition: leaf

A more specific class may *redefine* what it inherits: an attribute of its own that replaces one
of the general class's, named under **Redefines**. Tick **No redefinition (leaf)** when nothing may
redefine an element. It is in the **Properties** of a class, an interface, an enumeration, a data
type or a primitive type of your own, right after **No subclasses (final)**, and in the editor of
an attribute and of an operation. It is a separate setting from final: a leaf class may still be
specialized; only what it holds may not be redefined.

The element then shows `{leaf}`: after a class's name (`{final, leaf}` when both are ticked), and
after an attribute's or an operation's row, as in `+ total() : Money {leaf}`, on every diagram.
An association's end shows it beside its line too.

While it is ticked, **Redefines** lists the property last with `{leaf}` beside it, and choosing it
is refused under the field: *amount is leaf: nothing may redefine it. Clear "No redefinition" on
it first.*

You can only tick it while nothing redefines the element yet. Otherwise the box stays clear,
nothing changes, and the reason under it names what redefines it, such as *CardPayment::amount
redefines it. Remove that redefinition first.* Set that attribute's **Redefines** to **None**, then
tick the box again. Clearing the box later allows redefinitions again and changes nothing else.
A class's box is written at once; a member's is written with the rest of its editor when you apply
it. Each change is one step you can undo, and the project keeps it once saved.

<!-- picture: workspace-leaf light | Order's total() : Money {leaf} and PricingService {final, leaf} on their cards; Payment selected with the attribute editor open on amount, No redefinition (leaf) refused: CardPayment::amount redefines it -->

### Generalization sets

A *generalization set* groups generalizations to one class and says two things about them: whether
every instance of that class is one of the more specific kinds (**Complete**, or **Incomplete**)
and whether an instance may be of several of them at once (**Disjoint**, or **Overlapping**). In a
payment model, *method* groups the generalizations from *CardPayment*, *BankTransfer* and
*WalletPayment* to *Payment* and is `{complete, disjoint}`: every payment is made exactly one of
those ways.

There is no tool for it. Select the generalizations, clicking their lines with Shift held, then
right-click one of them:

- **Create generalization set…** (⌥⌘G, or Ctrl Alt G on Windows and Linux) groups them in a new set
  named *GeneralizationSet 1*, as one step you can undo. The set is selected and its name is ready
  to type in **Properties**.
- **Add to set ▸ method** adds them to a set the class has already. **Add to set** says *none for
  Payment yet* when it has none.

The same actions are buttons in **Properties** when several generalizations are selected
(**Create set from selection**), so the keyboard reaches them too. When the generalizations
specialise different classes, or something else is selected with them, the actions stay listed but
disabled, and the reason shows under the menu and on hover, such as *CardPayment specialises
Payment and Voucher specialises Product. A set groups generalizations of one class.*

On the diagram a set is a dashed line across its members' lines, with its name and
`{complete, disjoint}` beside it, on every diagram that shows them. Click the label, or the set's
row in **Models**, to select it. Drag the label to move it: where it sits is that diagram's own,
and it moves with the general class's card.

A set's **Properties** hold its **Name**, its **General class**, **Completeness** and
**Disjointness** (each two choices; the arrow keys switch them) and its **Members**: **+** offers
only generalizations to the general class, the pencil opens a member, and the cross takes it out
of the set. A generalization's own **Properties** list the sets it is in under **Generalization
sets**; one generalization may be in several sets, and **+** there adds it to another or starts a
new one.

- **Reconnecting** a generalization that is in a set is refused: the line's end snaps back and the
  canvas and its **Properties** say *CardPayment → Payment is in method and channel, which group
  the specialisations of Payment. Remove it from those sets before reconnecting it to Product.*
- **Deleting a generalization** takes it out of its sets in the same step you can undo. A set left
  with no member is deleted with it, and the toast names it.
- **Deleting the set** (Delete with it selected, or in **Models**) keeps its generalizations; only
  the grouping goes.

<!-- picture: workspace-generalization-sets light | Payment above CardPayment, BankTransfer and WalletPayment; the set method selected, its dashed line across their three generalizations and its label method {complete, disjoint} beside it; Properties shows its name, general class Payment, Complete and Disjoint chosen, and its three members -->

### Qualified associations

A *qualifier* picks out the instances at the far end of an association by a value. An order has
many lines, but an order and a line number pick out at most one line: the association *contains*
from *Order* to *OrderLine* is qualified at the *Order* end by `lineNo : Integer`, and its
*OrderLine* end's multiplicity is `0..1`.

Select the association and find the end the qualifier belongs to in **Properties**: the end at
the class that does the picking, here **End at Order**. Under its fields, **Qualifiers** lists that
end's qualifiers. Press **+** to add one: the editor takes its **Name**, its **Type**, chosen from
the same list of types an attribute uses, and its **Multiplicity** (empty means one). A line under
the fields says what it picks, such as *One order and one lineNo pick 0..1 line.* **Apply** (or
Enter) adds it as one step you can undo; **Cancel** or Escape leaves the model as it was. The pencil
on a row, or a double click, opens a qualifier again; in its editor the bin removes it and the
arrows move it up and down the list.

- **Where it is drawn.** The qualifier box sits against the qualified end's class, between the
  class and the line, its rows in the order of the list, on every diagram that shows the line. The
  end's role and multiplicity are written beside the box.
- **The other end's multiplicity** counts the instances one qualifier value picks. Its field is
  labelled **Per qualifier value** while the other end is qualified, and a line under it says so.
- **Qualifiers are not attributes or roles.** A class's card and its **Attributes** never list
  them, and the end's role stays its own.
- **What is refused.** A name another qualifier of the same end has, whatever the case: *This end
  already has a qualifier named lineNo. Pick another name.* The reason shows in the editor, the
  editor stays open, and nothing reaches the model or the undo history. A qualifier needs a name,
  and its multiplicity must read as one does elsewhere.
- **Renaming or reconnecting** the association keeps its qualifiers: they belong to their end and
  move with it. Deleting the association deletes them with it.

<!-- picture: workspace-qualifiers light | Order above OrderLine joined by the composition contains; the qualifier box lineNo : Integer against Order's card, order and 1 beside it, line and 0..1 at OrderLine; Properties shows End at Order with Qualifiers · 1 and the editor open on lineNo, and End at OrderLine with Per qualifier value 0..1 -->

<!-- picture: workspace-qualifier-refused light | The same, a second qualifier lineNo : String applied on the Order end and refused: the name ringed and the reason in a tinted box under the fields -->

### Constraints

A constraint is owned by a class, an interface, an enumeration, a data type, your own primitive
type, an association class or a package: its *context*, the element whose **Properties** list it
under **Constraints**. It names the elements it is about, its *constrained elements*: any package,
classifier, attribute, operation, literal or relationship of the model, in the order you give.

- **Stored, never evaluated.** The expression and the language are kept exactly as typed, spaces
  and line breaks included. Nothing reads them for meaning.
- **What is refused.** An empty expression ("Type an expression. A constraint without one states no
  rule."), and an element that is not in this model. Nothing else.
- **On a diagram.** Each diagram chooses for each constraint between braces, `{expression}` by the
  first of its elements the diagram shows, for short text, and a linked constraint note, for long
  text or several elements. Choose it under **On this diagram** in the editor; a new constraint
  starts in braces when its expression is one short line about one element. The braces move with
  their card. The note's dashed links follow the constrained elements on their own. Delete takes
  braces or a note off the diagram and keeps the constraint; double-click either to open its
  editor.
- **Deleting a constrained element** drops it from the constraint in the same undo step, and the
  toast says what the constraint constrains now, such as *Payment deleted · paidInFull now
  constrains Order only*. A constraint left with no element is deleted with it, and the toast
  names it. Deleting the context deletes its constraints.
- The editor's bin deletes the constraint; its arrows move it up and down the list.

### Nested classifiers

A class, an interface or an association class can *own* classifiers of its own: a *Priority*
enumeration that only makes sense for an *Order*, say. Such a classifier is *nested* in its owner
rather than held by a package.

- **Make one.** Right-click a class in **Models** (or select it and press ⇧F10) and choose **New
  inside Order ▸**, then **Class**, **Interface**, **Enumeration** or **Data type**. The new
  classifier appears under *Order* with its name ready to type. **Package** is listed but disabled:
  a package never goes inside a class. The class's **Properties** offer the same kinds under
  **Nested classifiers**, whose **+** names the new one in **Models** too. An enumeration, a data
  type or a primitive type owns no classifiers, so their **New inside** is disabled with the reason.
- **In Models** a nested classifier is listed under its owner, before the owner's attributes and
  operations, with a muted *nested* after its name. **Show in diagram**, **Rename** and **Delete
  from model** are in the same menu.
- **Owner.** Every classifier's **Properties** have **Owner** where **Package** used to be. Pick a
  package or a classifier to move it there, as one step you can undo. It keeps its identity, its
  members, its relationships and every type that uses it. The menu lists each package and
  classifier with its kind; those that cannot take it stay listed, disabled, with the reason at
  the right: *itself*, *owns none* (an enumeration, a data type or a primitive type), or *class
  Priority already holds that name* where the owner already has a classifier of that name, said by
  its kind. A classifier inside it is never
  offered, since owning it would make a cycle. A primitive type and an association class stay in a
  package.
- **Names.** A nested classifier's name differs, whatever the case, from the other classifiers its
  owner nests. Its owner's attributes and operations may share it, so *Order* can nest *Priority*
  and have the attribute *priority : Priority*.
- **On a diagram**, drag the nested classifier's row from **Models** onto the canvas. Where its
  owner is on the diagram too, a line ending in ⊕ at the owner shows the ownership; it is worked out
  from the model, comes and goes with either card, and cannot be selected. Where the owner is not on
  the diagram, the card reads *Order::Priority*, so its context is never lost. A card that merely
  sits over another card means nothing: only **Models** and **Owner** change ownership. The
  classifier tool on the canvas still places classifiers in the diagram's package.
- **Deleting the owner** from the model deletes what it nests, with their relationships and cards,
  as one step you can undo. The dialog names them first, such as *1 nested classifier goes with it:
  Order::Priority.* Removing the owner's card from a diagram only removes the card.

<!-- picture: workspace-nested-classifiers light | Priority nested in Order: Models lists it under Order with nested after its name; on the diagram Priority's card sits over Order's, joined by a line ending in ⊕ at Order; Properties shows Owner Order and what changing it keeps -->

For agents, `uml.nested.add` makes a nested classifier, `uml.classifier.set-owner` changes an
owner, `uml.nested.list` lists what a classifier nests and `uml.classifier.owners` lists where a
classifier may go, with the reasons. The existing `uml.class.move` and its siblings still move a
classifier, a nested one included, into a package.

### Templates

A *template* is a class, an interface or a data type written in terms of *parameters* it leaves
open: a *Repository* of some item type `T` found by some key type `Key`. A classifier becomes a
template when it gets its first parameter and stops being one when its last goes.

- **Add a parameter.** Select the class and press **+** in **Template parameters** in
  **Properties**. The editor takes its **Name**, its **Kind**, what it **Constrained to** and its
  **Default**, and a line under the fields says what that asks of an argument, such as *Arguments
  for Key must be data types. Bindings that leave it out use Integer.* **Apply** (or Enter in the
  name) adds it as one step you can undo; **Cancel** or Escape leaves the model as it was. The
  pencil on a row, or a double click, opens a parameter again; in its editor the bin removes it and
  the arrows move it up and down the list.
- **Kinds.** A parameter stands for a **Class**, an **Interface**, a **Data type**, an
  **Enumeration** or a **Primitive type**. **Value** and **Operation** parameters are listed in the
  menu but disabled, saying *not yet*: they are not supported in this version, and no other kind is
  used in their place.
- **Constrained to** narrows the arguments to one classifier of the kind and those that specialise
  it; **Any** leaves them open. **Default** is the argument a binding that leaves the parameter out
  uses: a classifier of the kind that fits the constraint, so a *Data type* parameter takes
  *Integer* or your own *Money*, never the class *Order*. The menus offer only what fits.
- **Using a parameter.** Inside the template, the type menus of its attributes, operations and
  parameters offer its template parameters first, each marked *template parameter*: `- items : T
  [*]`, `+ find(key : Key) : T`. No other classifier can use them, and they never appear in
  **Models**, the search or a diagram on their own.
- **On a diagram** the card shows the signature in a dashed box over its top-right corner, one
  parameter a line, such as `T : Class` and `Key : DataType = Integer`; a constraint follows `>`.
  Every diagram that shows the template shows the same box. **Models** writes *template* after its
  name, and **Properties** says *Class · template, 2 parameters*.
- **Identity.** Renaming a parameter, moving it or changing its default keeps it the same
  parameter: every member typed by it follows, and so does every binding. Changing its kind keeps
  the parameter too, and the members typed by it follow.
- **What is refused.** A name another parameter of the template has, whatever the case: *Repository
  already has a template parameter named T. Pick another name.* A default that does not fit is
  refused under **Default**. The reason shows in the editor, the editor stays open, and nothing
  reaches the model or the undo history.
- **Bindings.** Another class can *bind* a template, giving each parameter an argument. A parameter
  a binding uses cannot be removed: the bin says, for example, *OrderRepository binds Key ->
  Integer. Change or remove that binding first, then remove Key.*, under the rows, and **Show the
  binding** selects the element that binds it. A template's last parameter cannot be removed while
  any binding names the template, even one with no arguments yet, since the template would stop
  being one under it. Its kind or constraint cannot change so that an
  argument no longer fits either. Drawing bindings comes in a later version; deleting a template
  deletes the bindings to it.

<!-- picture: workspace-template light | Repository with its signature box T : Class and Key : DataType = Integer over its card's top-right corner, items : T [*], find(key : Key) : T and save(item : T) on the card; Properties shows Template parameters · 2 with the editor open on Key: Kind Data type, Constrained to Any, Default Integer -->

<!-- picture: workspace-template-kinds light | The same, the Kind menu open: Class, Interface, Data type ticked, Enumeration and Primitive type, then Value and Operation disabled, saying not yet -->

<!-- picture: workspace-template-in-use light | OrderRepository binds Repository: Key's removal is refused under the rows, naming the binding, with Show the binding under it -->

For agents, `uml.template-parameter.add`, `.set` and `.move` add, change and move a parameter,
`uml.template-parameter.list` lists a classifier's parameters with the bindings that use them, and
`element.remove` removes one.

### Cards

A card grows to show every member you add. A card you made shorter by hand says how many rows it
cannot show, such as `+ 2 more literals`. An enumeration holds at most 256 literals; past that, a
new one is refused and the reason says so.

### Removing and deleting

As everywhere in Archforge (see [The workspace](03-workspace.md#removing-from-a-diagram-versus-deleting-from-the-model)),
**Remove from this diagram**, or Delete, takes a card or a line off the diagram and keeps it in
the model; **Delete from the model…** (⇧⌫) asks first, then deletes it everywhere.

- Deleting a class or an interface takes its interface realizations with it, and the classifiers
  it nests with theirs.
- Deleting a generalization takes it out of its generalization sets, and a set left with no member
  goes with it.
- Deleting an association class takes its ends, attributes and operations with it. Removing a
  class at either end takes the association class along.
- An association class's row in **Models** can be dragged onto a diagram to place its card; where
  the diagram shows both its classes, its line comes too. A diagram may also show the line
  without the card: the line then carries the name at its middle, and Delete on it takes it only
  from the diagram.

### Skills on class elements

Classes, interfaces, enumerations, data types, your own primitive types and association classes
carry skills (see [Skills](06-skills.md)). Literals and the built-in types carry none. A package's
skill can aim at every element of one kind in it, such as every data type; a skill for every data
type does not reach primitive types, and a skill for every class or every association does not
reach association classes.

Next: [Skills](06-skills.md).
