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

This chapter has four parts. You need only the first to make a useful model:

- **A walkthrough in twelve steps**, after [the tools you will use](#the-tools-you-will-use): from
  [a first class diagram](#1-make-a-class-diagram) to [a rule](#12-state-a-rule-constraints), then
  [the result](#the-result) and [which element you need](#which-element-do-i-need).
- **[Organise the model in packages](#organise-the-model-in-packages)**, for a model that grows.
- **[Going further](#going-further)**: short sections for more precise models, such as finding an
  item by a key, templates and your own stereotypes. Each stands on its own; read the ones you
  need.
- **[Reference](#reference)**: the rules and the other ways in, to look up.

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
the underline goes (see [Static members](#a-member-of-the-class-as-a-whole-static-members)). An operation's **Abstract** slants its row, so a static abstract operation is
both underlined and slanted. **No redefinition (leaf)** writes `{leaf}` after the member's row.

## 4. Connect Customer and Order: an association

A customer places orders. An *association* says that instances of two classes are linked like
this. Its ends say how many instances take part (the *multiplicity*) and which way you can follow
the link (*navigability*).

1. Open the chevron beside the **Relationship** tool. The menu lists the kinds of line, the
   current one highlighted.
2. Choose **Association**.

<!-- picture: workspace-guide-relate-kinds light | The Relationship tool's menu open over Customer and Order: Association highlighted, then the other kinds of line, Interface realization and Association class below a line, and N-ary association and Template binding below another -->

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
4. Choose the check at the editor's foot.
5. Choose the pencil on *status*.
6. Open **Default value** and choose *Placed*. The menu lists *OrderStatus*'s literals.
7. Choose the check.

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
2. In **Properties**, choose **+** beside **Constraints · 0**. The editor opens with *Order* among
   the **Constrained elements**, and **Name** says *Constraint 1*.
3. Replace *Constraint 1* in **Name** with *positiveNumber*.
4. Type `number > 0` in **Expression**.
5. In **Language**, pick **OCL**, or type any other language; left empty, it is plain text.
6. Choose the check at the editor's foot. A short rule shows in braces under *Order*'s card:
   `{number > 0}`.
7. Choose **+** again, and replace the name with *paidInFull*.
8. In **Language**, pick **OCL**.
9. In **Expression**, type `status = OrderStatus::Paid`, press Enter for a new line (Enter does not
   apply here), and type `implies total.amount > 0`.
10. Under **Constrained elements**, choose the **+** after *Order* and pick *OrderStatus*.
11. Under **On this diagram**, check that **Linked note** is chosen: Archforge chooses it for a
    long rule about several elements. Then choose the check.

<!-- picture: workspace-guide-constraints light | Order selected: {number > 0} in braces under its card, and the constraint note paidInFull, its eyebrow CONSTRAINT · NOT EVALUATED, linked by dashed lines to Order and OrderStatus; Properties shows Constraints · 2 with the editor open on paidInFull, its language OCL, its expression on two lines, Order and OrderStatus as chips and Linked note chosen -->

The note says *CONSTRAINT · NOT EVALUATED*, and dashed lines join it to *Order* and
*OrderStatus*. (The picture shows the editor opened again with the pencil on *paidInFull*.) It looks different from a plain note on purpose: a plain note is your remark on the
diagram, a constraint is part of the model.

What Archforge checks, and what it does not:

- It refuses a constraint without an expression: *Type an expression. A constraint without one
  states no rule.*
- A constraint names only elements of this model. When you delete one of them, the constraint
  drops it in the same step, and the message says what it constrains now. A constraint left naming
  nothing is deleted with it.
- It never reads the expression for meaning. It does not tell you when the model, or the data your
  software holds, breaks the rule, even though the rule shows on the diagram. Checking the rule is
  up to you, your code or your tests.

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

## Going further

Steps 1 to 12 cover what most class models need. Each section below adds one more idea, for when
your model needs it. Read only the ones you need: each starts from the model as step 12 left it,
unless it says it builds on the section before. Each uses the same pattern: the problem first, then
the steps, then what you see and how to change it later. The [Reference](#reference) has the
rules and the other ways in.

| You want to say | Read |
|---|---|
| A value or an action belongs to the class as a whole, not to each instance | [Static members](#a-member-of-the-class-as-a-whole-static-members) |
| A related item is found by a key, such as a customer's order by its number | [Qualified associations](#find-an-item-by-a-key-qualified-associations) |
| A link only makes sense between three things at once | [N-ary associations](#link-three-things-at-once-n-ary-associations) |
| The more specific kinds of a class cover it, and whether they overlap | [Generalization sets](#divide-a-class-into-kinds-generalization-sets) |
| Nothing may be a more specific kind of a class | [Final](#allow-no-more-specific-kinds-final) |
| A more specific class may not replace a member it inherits | [Leaf](#allow-no-replaced-members-leaf) |
| A type belongs inside the one class that uses it | [Nested classifiers](#keep-a-type-inside-its-class-nested-classifiers) |
| One class works for many types, with blanks to fill in | [Defining a template](#one-class-for-many-types-defining-a-template) |
| A class is such a class with its blanks filled in | [Binding a template](#fill-in-a-template-binding-it) |
| Words and values of your own, such as a database table's name | [Defining a profile](#words-of-your-own-defining-a-profile-and-its-stereotypes) and [applying stereotypes](#label-elements-with-your-words-applying-stereotypes) |
| A rule, such as "an order's number is above zero" | [Step 12](#12-state-a-rule-constraints) |

The pictures in this part come from the latest Archforge release, so they need an internet
connection and show the newest version of each screen.

### A member of the class as a whole: static members

Most facts and actions belong to each instance: every order has its own number. Some belong to the
class as a whole. Handing out the next free order number is one counter for all orders, not one
per order. UML calls such a member *static*, and draws it underlined.

Start from the Commerce diagram of steps 1 to 12.

1. Select *Order*.
2. Press ⇧⌘↩ (Ctrl Shift Enter). A draft row opens in the operations compartment.
3. Type *nextNumber*, press Tab, type *Integer*, and press Enter.
4. Press Escape to close the empty draft.
5. In **Properties**, under **Operations**, choose the pencil on *nextNumber*. Its editor opens.
6. Tick **Static**.
7. Choose the check at the editor's foot.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-static-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-static-light.png" alt="Order selected; its operation nextNumber() : Integer is underlined on its card, and the operation editor in Properties has Static ticked." width="100%">
</picture>

*An underlined row is a static member: it belongs to Order itself, not to each order.*

The row `+ nextNumber() : Integer` is now underlined, on every card that shows *Order* and in
**Properties**. (The picture shows its editor opened again with the pencil.) An attribute works
the same way: tick **Static** in its editor for a value that all instances share, such as the last
number handed out.

To undo it, open the editor again and clear **Static**: the underline goes. An operation that is
both static and **Abstract** is underlined and slanted.

### Find an item by a key: qualified associations

A customer places many orders, and you often look one up by its number: "this customer's order
7". A customer and a number together pick out at most one order. The value that does the picking
is a *qualifier*, and an association with one is a *qualified association*.

Start from the Commerce diagram, with the association *places* of
[step 4](#4-connect-customer-and-order-an-association).

1. Click the line *places* to select it. **Properties** shows a section for each end.
2. Below the fields of **End at Customer**, find **Qualifiers · 0** and choose its **+**. An editor
   opens.
3. In **Name**, type *number*.
4. Open **Type** and choose *Integer*.
5. Leave **Multiplicity** empty: one number picks.
6. Choose the check at the editor's foot.
7. Under **End at Order**, the field that said **Multiplicity** now says **Per qualifier value**.
   Open it and choose `0..1`.

The qualifier goes on the end of the class that does the looking up: a customer looks up its
orders, so it is on **End at Customer**.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-qualifier-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-qualifier-light.png" alt="The association places selected: a small box number : Integer against Customer's card, 0..1 at Order's end; Properties shows End at Customer with Qualifiers · 1 and the qualifier editor open on number." width="100%">
</picture>

*The box against Customer holds the qualifier. Read the line as: one customer and one number pick
0..1 orders.*

A small box reading `number : Integer` sits against *Customer*'s card, where the line starts. The
`0..1` at *Order* now counts the orders for one number, not all of a customer's orders. The
qualifier's editor says it in words: choose the pencil on *number* to see *One Customer and one
number pick 0..1 orders.*, as the picture does. (While you typed it, before step 7, it said
*pick \* orders*.) If the box pushes the line's labels together, drag *Customer*'s card a little
to the left; the picture shows it moved.

A qualifier is not an attribute: *Customer*'s card and its **Attributes** do not list it.

- **Change it:** choose the pencil on its row under **Qualifiers**, or double-click the row.
- **Remove it:** in its editor, choose the red bin.
- **Several qualifiers:** add more with **+**; the arrows in the editor change their order.

More in the reference: [Qualified associations](#qualified-associations).

### Link three things at once: n-ary associations

The shop recommends products to customers, and each recommendation comes from a catalog, such as
the summer catalog. Such a link only makes sense with all three: which customer, which product,
which catalog. Lines between pairs of classes cannot say which catalog recommended which product
to whom. An association that joins three or more classes at once is an *n-ary association*. It is
drawn as a diamond with a leg to each class.

Start with a new class diagram, so the legs have room.

1. Make a class diagram named *Recommendations*, as in [step 1](#1-make-a-class-diagram).
2. Drag *Customer*'s row from **Models** onto the canvas, towards the top left.
3. Drag *Product*'s row onto the canvas, towards the top right.
4. Choose the classifier tool (its tooltip says **Class**) and click below the two cards.
5. Type *Catalog* and press Enter.
6. Open the chevron beside the **Relationship** tool and choose **N-ary association**, below the
   line after **Association class**. Its tooltip says to click each class in turn.
7. Click *Customer*.
8. Click *Product*. A diamond appears between them, and the bar at the top of the canvas counts
   the ends.
9. Click *Catalog*.
10. Press Enter. The association is drawn and selected.
11. In **Properties**, type *recommends* in **Name** and press Enter.
12. Under **Ends · 3**, choose the pencil on the *catalog* row. The end's editor opens.
13. Open **Multiplicity** and choose `1`.
14. Choose the check at the editor's foot, or press Enter.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-nary-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-nary-light.png" alt="The diagram Recommendations: a diamond named recommends with legs to Customer, Product and Catalog, 1 at Catalog; Properties shows Ends · 3 with the editor open on the catalog end." width="100%">
</picture>

*One association, three legs. Each end's multiplicity counts the instances at that end for one
choice at every other end: for one customer and one product, exactly one catalog.*

Each end is named after its class (*customer*, *product*, *catalog*) and starts at `*`, any
number. A click on the diamond or on any leg selects the whole association. (The picture shows the
*catalog* end's editor opened again with its pencil.)

- **Add an end:** choose **+** beside **Ends**, or right-click the diamond and choose **Add end**,
  then click a class.
- **Move an end:** drag the end of a leg onto another class.
- **Remove an end:** choose the red bin in the end's editor. With three ends left, Archforge asks
  first: **Keep 3 ends** changes nothing, **Make it binary** turns it into an ordinary association
  between the two classes left.
- **Take it off this diagram:** press Delete (⌫). **Delete it from the model:** ⇧⌫ (Shift Delete).

If you press Escape, choose another tool or click empty canvas while drawing, nothing is added. An
n-ary association joins classes and interfaces only. More in the reference:
[N-ary associations](#n-ary-associations).

### Divide a class into kinds: generalization sets

A payment is made by card, by bank transfer or from a wallet, and every payment is exactly one of
these. A generalization says one class is a more specific kind of another: a card payment is a
kind of payment. It does not say whether the kinds cover every payment, or whether one payment can
be two kinds at once. A *generalization set* groups the generalizations to one class and says
both.

Start with a new class diagram for payments.

1. Make a class diagram named *Payments*, as in [step 1](#1-make-a-class-diagram).
2. With the classifier tool on **Class**, click near the top and add the class *Payment*.
3. With *Payment* selected, press ⌘↩ (Ctrl Enter), type *amount*, press Tab, type *Money*, and
   press Enter. Press Escape.
4. Add three classes in a row below *Payment*: for each, choose the classifier tool, click, type
   its name and press Enter. Name them *CardPayment*, *BankTransfer* and *WalletPayment*.
5. Open the chevron beside the **Relationship** tool and choose **Generalization**.
6. Press on *CardPayment*, drag onto *Payment*, and let go. A line with a hollow triangle at
   *Payment* says that a card payment is a kind of payment.
7. Choose the **Relationship** tool again (it still draws generalizations) and drag from
   *BankTransfer* onto *Payment*.
8. Do the same from *WalletPayment* onto *Payment*.
9. Click *CardPayment*'s line to select it.
10. Hold Shift and click the other two lines. All three are selected.
11. Right-click one of them and choose **Create generalization set…** (⌥⌘G, or Ctrl Alt G).
    **Properties** shows the new set, named *GeneralizationSet 1*, its **Name** field active.
12. Select the whole name with ⌘A (Ctrl A), type *method*, and press Enter.
13. Under **Completeness**, choose **Complete**.
14. Under **Disjointness**, choose **Disjoint**.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-generalization-set-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-generalization-set-light.png" alt="The diagram Payments: CardPayment, BankTransfer and WalletPayment below Payment, their generalizations crossed by a dashed line labelled method {complete, disjoint}; Properties shows the set with Complete and Disjoint chosen and its three members." width="100%">
</picture>

*The dashed line across the three generalizations is the set. Its label says every payment is
exactly one of these kinds.*

The two choices mean:

- **Complete**: every payment is one of these kinds; there is no plain payment. **Incomplete**:
  there may be payments of other kinds too.
- **Disjoint**: no payment is two kinds at once. **Overlapping**: one payment may be several kinds.

A new set starts **Incomplete** and **Overlapping**, and its label says `{incomplete,
overlapping}` until you choose otherwise. Click the label, or the set's row in **Models**, to select the set again.

- **Add or take out members:** under **Members** in the set's **Properties**, **+** offers the
  other generalizations to *Payment*, and the cross takes one out. Or right-click selected
  generalizations and choose **Add to set ▸ method**.
- **Move the label:** drag it. Where it sits is this diagram's own choice.
- **Delete the set:** Delete with it selected. Its generalizations stay; only the grouping goes.

More in the reference: [Generalization sets](#generalization-sets).

### Allow no more specific kinds: final

Card payments are handled one way, by one payment provider, and nobody should make a special kind
of card payment that might behave differently. Marking *CardPayment* as *final* says that nothing
may be a more specific kind of it.

This section builds on the *Payments* diagram of the section before.

1. Select *CardPayment*.
2. In **Properties**, tick **No subclasses (final)**.

Its card now shows `{final}` after its name, on every diagram. *CardPayment* is still a kind of
*Payment*: final only stops new kinds below it. If you now draw a generalization from another class
to *CardPayment*, nothing is drawn, and the top of the canvas says *CardPayment is final: nothing
may specialise it. Clear "No subclasses" in its Properties first.*

You can only tick the box while nothing specializes the class yet. Otherwise the box stays clear,
and **Properties** names the classes that specialize it, with a button such as **Select the
generalization** to find their lines. Clear the box to allow more specific kinds again.

More in the reference: [Forbidding subclasses: final](#forbidding-subclasses-final).

### Allow no replaced members: leaf

A more specific class may *redefine* a member it inherits: *CardPayment* could replace *Payment*'s
*amount* with an amount of its own. Sometimes the general class must keep control, so that every
payment's amount means the same. A *leaf* member may be inherited, but not replaced. This is a
different promise from final: other classes may still be kinds of *Payment*.

This section builds on the *Payments* diagram too; the picture also shows `{final}` from the
section before.

1. Select *Payment*.
2. In **Properties**, under **Attributes**, choose the pencil on *amount*. Its editor opens.
3. Tick **No redefinition (leaf)**.
4. Choose the check at the editor's foot.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-final-leaf-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-final-leaf-light.png" alt="The diagram Payments: CardPayment with {final} after its name, Payment's row amount : Money {leaf}; Payment selected with the attribute editor open on amount, No redefinition (leaf) ticked." width="100%">
</picture>

*`{final}` after a class's name: no more specific kinds. `{leaf}` after a member: no replacing it.*

*Payment*'s row now reads `+ amount : Money {leaf}` (the picture shows its editor opened again). If a more specific class gets an attribute
whose **Redefines** names *amount*, its editor lists *amount* last with `{leaf}` beside it, and
choosing it is refused: *amount is leaf: nothing may redefine it. Clear "No redefinition" on it
first.*

An operation has the same box in its editor. A class, an interface, an enumeration, a data type
and a primitive type have it too, right after **No subclasses (final)**; the card then shows
`{leaf}` after the name, or `{final, leaf}` with both ticked. You can only tick the box while
nothing redefines the element yet.

More in the reference: [Forbidding redefinition: leaf](#forbidding-redefinition-leaf).

### Keep a type inside its class: nested classifiers

*OrderStatus* only makes sense for orders. Keeping it inside *Order* says so, and keeps it apart
from a status of another class. A classifier owned by a class, rather than by a package, is
*nested* in it.

Start from the Commerce diagram, with *OrderStatus* of [step 5](#5-give-an-order-a-status-an-enumeration).

1. Select *OrderStatus*.
2. In **Properties**, open **Owner**. It shows the root package, your project's name.
3. Choose *Order*.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-nested-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-nested-light.png" alt="OrderStatus selected below Order, joined to it by a line ending in a circled plus at Order; Models lists OrderStatus under Order with nested after its name; Properties shows Owner Order." width="100%">
</picture>

*The line ending in ⊕ at Order says Order owns OrderStatus.*

**Models** now lists *OrderStatus* under *Order*, with a muted *nested* after its name. On the
diagram, a line ending in ⊕ at *Order* shows the ownership. Archforge draws it while both cards
are on the diagram; you cannot select it. On a diagram without *Order*, the card reads
*Order::OrderStatus*, so you can still tell where it belongs. Everything else stays as it was:
*Order*'s *status* is still of the type *OrderStatus*, and its literals are unchanged.

- **Make a new one inside a class:** right-click the class in **Models** (or select its row and
  press ⇧F10), choose **New inside Order ▸**, and pick a kind. Or choose **+** beside **Nested
  classifiers** in the class's **Properties**. Drag the new row onto a diagram to show it.
- **Move it out again:** pick a package as its **Owner**.
- **Who can own one:** a class, an interface or an association class. An enumeration, a data type
  or a primitive type owns none, and a primitive type or an association class always stays in a
  package. The **Owner** menu lists what cannot take it disabled, with the reason.

Placing one card over another on a diagram changes nothing in the model: only **Owner** and
**Models** change ownership. More in the reference: [Nested classifiers](#nested-classifiers).

### One class for many types: defining a template

Your software stores orders and finds them by their number, and it stores customers and finds
them by their email. Both stores work the same way, only the types differ. A *template* is one
class written with blanks for those types: a *Repository* of some item type *T*, found by some key
type *Key*. The blanks are its *template parameters*.

Defining the template, with its blanks, is one task. Using it for orders, with the blanks filled
in, is another: [binding it](#fill-in-a-template-binding-it), in the next section.

Start with a new class diagram.

1. Make a class diagram named *Storage*, as in [step 1](#1-make-a-class-diagram).
2. With the classifier tool on **Class**, click on the canvas and add the class *Repository*.
3. With *Repository* selected, choose **+** beside **Template parameters · 0** in **Properties**.
   The parameter's editor opens.
4. In **Name**, type *T*. Leave **Kind** on **Class** and **Constrained to** on **Any**.
5. Choose the check at the editor's foot. *T* stands for any class.
6. Choose **+** again and type *Key* in **Name**.
7. Open **Kind** and choose **Data type**.
8. Open **Default** and choose *Integer*. The line under the fields says what this asks of a
   binding: *Arguments for Key must be data types. Bindings that leave it out use Integer.*
9. Choose the check.
10. With *Repository* selected, press ⌘↩ (Ctrl Enter), type *items*, press Tab, type *T*, and
    press Enter. Press Escape.
11. In **Properties**, under **Attributes**, choose the pencil on *items*.
12. Open **Multiplicity**, choose `*` (a repository holds many items), and choose the check.
13. Press ⇧⌘↩ (Ctrl Shift Enter), type *find*, press Tab, type *T*, and press Enter. Press Escape.
14. In **Properties**, under **Operations**, choose the pencil on *find*.
15. Choose **Add parameter**, click the new row's **Name** field, and type *key*.
16. Open the parameter's type menu. The template's own parameters come first, after **None**,
    each marked *template parameter*. Choose *Key*.
17. Choose the check.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-template-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-template-light.png" alt="Repository selected on the diagram Storage, a dashed box T : Class and Key : DataType = Integer over its top-right corner, items : T [*] and find(key : Key) : T on its card; Properties shows Template parameters with the editor open on Key." width="100%">
</picture>

*The dashed box over the corner is the template's signature: its blanks, their kinds and Key's
default.*

The card shows the template's signature in a dashed box over its top-right corner: `T : Class`
and `Key : DataType = Integer`. (The picture shows *Key*'s editor opened again with its pencil.) **Models** writes *template* after *Repository*, and **Properties**
says *Class · template, 2 parameters*. Read it as: a repository holds any number of items of some
class *T*, and finds one by a key of some data type *Key*, a whole number unless a binding says
otherwise.

*T* and *Key* are types only inside *Repository*: no other class's type menu offers them, and they
are not listed in **Models**.

- **Change a parameter:** choose the pencil on its row under **Template parameters**. Renaming it
  keeps every member that uses it.
- **Remove a parameter:** choose the red bin in its editor. A parameter that a binding fills in
  cannot be removed until that binding changes.
- **Kinds:** a parameter stands for a class, an interface, a data type, an enumeration or a
  primitive type. **Value** and **Operation** are listed but disabled, marked *not yet*.

More in the reference: [Templates](#templates).

### Fill in a template: binding it

*OrderRepository* is a *Repository* of orders, found by a whole number. Rather than writing its
members again, say that it *binds* the template: it is *Repository* with *T* filled in as *Order*
and *Key* as *Integer*. Each filled-in blank is a *substitution*.

This section builds on the *Storage* diagram of the section before.

1. With the classifier tool on **Class**, click below *Repository* and add the class
   *OrderRepository*.
2. Open the chevron beside the **Relationship** tool and choose **Template binding**, the last row.
   Its tooltip says it runs from the bound class to its template.
3. Press on *OrderRepository*, drag onto *Repository*, and let go. Nothing is in the model yet:
   **Properties** shows *New template binding*, and the line stays dashed in the accent colour.
4. Under **Substitutions**, open the selector beside *T* and choose *Order*. It lists only
   classes, since *T* stands for a class.
5. Leave *Key* as it is: it shows *Integer*, tagged *default*.
6. Choose **Apply**.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-template-binding-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-template-binding-light.png" alt="OrderRepository below Repository, joined by a dashed line with an open arrow at Repository labelled «bind» with T to Order and Key to Integer; Properties shows the binding and its two substitutions, Key tagged default." width="100%">
</picture>

*«bind» and the substitutions: OrderRepository is Repository with T as Order and Key as Integer.*

A dashed line with an open arrowhead at *Repository* reads `«bind» <T -> Order, Key -> Integer>`
(the arrows may show as →).
**Models** writes *binds Repository* after *OrderRepository*. Its card stays empty: the members
come from the template, and Archforge does not copy them.

If you choose **Apply** before picking *T*, nothing is added: the draft stays open and says *T
needs an argument: pick a class. Key can keep its default.* **Cancel**, or Escape, removes the
draft and leaves the model as it was.

- **Change an argument:** select the line and pick another one under **Substitutions**, or
  **Default (Integer)**, say, to go back to a parameter's default.
- **Remove the binding:** select the line and press ⇧⌫ (Shift Delete).
- **What is refused:** a binding between different kinds (a class binds a class template, an
  interface an interface template, a data type a data type template), a second binding of the same
  template, and deleting a class that a binding uses as an argument, such as *Order*.

More in the reference: [Template bindings](#template-bindings).

### Words of your own: defining a profile and its stereotypes

The model says that *Order* is a class. Your team also wants to say that it is stored in a
database table, named *orders*. UML has no word for that, so you add one. A *stereotype* is a word
of your own, such as *Entity*, with values of its own, such as the table's name. Stereotypes are
kept in a *profile*, a package for your own additions, such as *Persistence*.

Defining the words is one task, done once. Putting them on elements is another:
[applying stereotypes](#label-elements-with-your-words-applying-stereotypes), in the next section.

Start from any diagram; this happens in **Models**.

1. In **Models**, right-click the top row, your project's root package. (Or select it and press
   ⇧F10.)
2. Choose **New ▸**, then **Profile**. A profile appears with its name ready to type.
3. Type *Persistence* and press Enter.
4. Right-click *Persistence* and choose **New ▸**, then **Stereotype**.
5. Type *Entity* and press Enter. **Models** says *extends nothing yet* after it.
6. Select *Entity*. In **Properties**, choose **+** beside **Extends · 0**. The menu lists the
   kinds of element the stereotype can be for; UML calls them *metaclasses*.
7. Choose **Class**. **Models** now says *extends Class*.
8. Choose **+** beside **Properties · 0**. An editor opens, as for an attribute.
9. Type *table* in **Name**, choose *String* in **Type**, and choose the check.
10. Select *Persistence* and choose **Open profile diagram** in its **Properties**.
11. Click *Entity* on the diagram to see its **Properties** again.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-profile-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-profile-light.png" alt="The profile diagram of Persistence: a «stereotype» card Entity with table : String under a «metaclass» card Class, joined by a line with a filled triangle at Class; Models lists Persistence with Entity under it, extends Class." width="100%">
</picture>

*A stereotype under the metaclass it extends: Entity can be put on classes, and carries a table.*

The profile's diagram opens. It shows *Entity* as a «stereotype» card under a «metaclass» card
*Class*, joined by a line with a filled triangle at *Class*. Archforge makes this diagram the first
time and adds what it lacks each time you open it. The metaclasses are UML's own: they are not in
**Models**, and you cannot rename or delete them.

- **Extend more kinds:** **+** under **Extends** again. The kinds Archforge draws can be chosen:
  Class, Interface, Enumeration, DataType, Property (an attribute or an association's end),
  Operation, Association, Generalization and Package. Others are listed but disabled.
- **Types of a stereotype's properties:** a primitive type, an enumeration or a data type,
  including the built-in types.
- **Rename** a profile or a stereotype in **Models**; every element that uses it keeps it.

More in the reference: [Profiles and stereotypes](#profiles-and-stereotypes).

### Label elements with your words: applying stereotypes

With *Persistence* defined, you can now say that *Order* is an *Entity* stored in the table
*orders*. First a package allows the profile's words, then each element gets the ones it needs.

This section builds on the profile of the section before.

1. In **Models**, select the top row, your project's root package.
2. In **Properties**, choose **+** beside **Applied profiles · 0**.
3. Choose *Persistence*. **Models** now says *applies Persistence* after the top row. Everything
   in the package, and in the packages inside it, may now use the profile's stereotypes.
4. Go back to the *Commerce* diagram (its tab, or its row in **Diagrams**) and select *Order*.
5. In **Properties**, choose **+** beside **Stereotypes · 0**. A list headed *APPLY TO ORDER*
   opens.
6. Choose *Entity*. (You can also type to filter the list, and press Enter.)
7. Under *«Entity»*, type *orders* in the **table** field and press Enter.
8. Tick **Show stereotype values on this diagram**.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-stereotype-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-guide-stereotype-light.png" alt="Order selected on the Commerce diagram, its card reading «Entity» above its name and a compartment «Entity» table = orders at its foot; Properties shows Stereotypes · 1 with «Entity» and table set to orders, and Show stereotype values on this diagram ticked." width="100%">
</picture>

*«Entity» takes the place of CLASS above the name. The last compartment lists the values set on
this diagram.*

*Order*'s card now says *«Entity»* above its name, where it said *CLASS*, and a compartment at its
foot reads `table = "orders"`. **Models** shows *«Entity»* after *Order*. Each diagram chooses for
itself whether it shows the values; a property left to its default is not listed.

- **Other elements:** an interface, an enumeration, a data type, a package, an association, a
  generalization and an association's end each have a **Stereotypes** section in **Properties**.
  For an attribute or an operation, choose its row in the class's **Properties**; the section is
  named after it. The list offers first the stereotypes that fit the element; others are listed
  disabled, with why, such as *Persistence not applied*.
- **An n-ary association's ends** have no **Stereotypes** section yet. Only an agent can apply
  stereotypes to them (see [Working with an agent](07-agents.md#what-the-agent-can-do)).
- **Remove a stereotype:** choose the bin on its row. Its values go with it; the stereotype stays
  in its profile.
- **Nothing in use is lost:** while *Order* is an *Entity*, unapplying *Persistence* or deleting
  *Entity* is refused, and Archforge names what uses it.

More in the reference: [Applying stereotypes](#applying-stereotypes).

## Reference

The walkthrough and Going further show the usual way to do each thing. This part collects the
other ways and the rules, to look up when you need them. What an agent can do with each feature is
in [Working with an agent](07-agents.md#what-the-agent-can-do).

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
| N-ary association | Three or more classes and interfaces, a class at several ends when their roles differ. Never an enumeration, a data type or a primitive type. |

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

### N-ary associations

Most associations join two classes. An *n-ary* association joins three or more at once, when a
link only makes sense with all of them: a customer is recommended a product from a catalog, so the
association *recommends* joins *Customer*, *Product* and *Catalog*. It is drawn as a diamond with
a leg to each class, and it is one association, however many legs it has.

**Draw one.** Open the **Relationship** tool's chevron and pick **N-ary association**, below
**Association class**. Then click each class in turn: every click on a class adds one end, the
diamond appears with the second end, and the bar at the top of the canvas counts the ends. Press
**Enter** to finish once there are three ends or more, or double-click the last class: the first
click of the double-click adds its end, and the second finishes the association instead of adding
that class again. The association, its diamond and every leg are added as one step you can undo.
**Backspace** takes the last end off; before the first end, it removes the selection as usual.
**Esc**, choosing another tool, or a click on empty canvas cancels and leaves nothing in the model.
To give a class two ends, click it twice while fewer than three ends are drawn, or once more later:
each end then has a role of its own.

Each new end is named after its class, such as *customer*, and starts at `*`; a second end at the
same class is *customer2*. Change them afterwards in **Properties**.

**Select and edit it.** A click on the diamond or on any leg selects the association. Its
**Properties** show its **Name** and its package, then **Ends**, one row per end, such as
`customer : Customer [*]`. The pencil on a row, or a double click, opens the end's editor:
**Role**, **Type** (the class at the end), **Multiplicity**, **Navigable**, **Ordered** and
**Unique**. **Apply** (or Enter) writes them as one step; **Cancel** or Escape leaves the model as
it was. Two ends may have the same type, but never the same role: a role another end has is
refused in the editor, and nothing is written. An n-ary association's ends are never aggregations,
so the editor offers no **Aggregation**.

- **Add an end.** Press **+** beside **Ends**, or choose **Add end** on the diamond's menu (right
  click), then click a class. The new leg is drawn only on the diagrams that show both the
  diamond and that class; on the others it appears once you place the class there.
- **Move an end.** Drag the end of a leg onto another class, or pick another **Type** in the end's
  editor. On each diagram that shows the diamond, the leg moves to the new class's card; where that
  class is not shown, the leg goes until you place it. The ends may drop **Subsets** or
  **Redefines** entries that the move makes invalid; the canvas names them, and **Undo** brings
  them back with the end.
- **Remove an end.** The bin in the end's editor removes it with its legs. With four ends or more
  it goes at once, and a message offers **Undo**. A constraint that applies to that end alone goes
  with it, and the ends left drop any **Subsets** or **Redefines** entry that the removal makes
  invalid. **Undo** brings all of it back. When only three ends are left, removing one would leave
  two, and an n-ary association needs three or more. **Properties** asks first: **Keep 3 ends**
  (where the focus starts; Esc does the same) changes nothing, and **Make it binary** turns it into
  an ordinary association between the two classes left, keeping its name and both ends, as one
  step you can undo. Wherever a diagram showed the diamond and both classes, a line takes its place.
- **Remove from this diagram** (Delete, or the diamond's menu) takes the diamond and its legs off
  this diagram only. **Delete from the model…** (Shift+Delete) asks first, then deletes the
  association and its ends from every diagram.

Taking a class's card off a diagram takes its legs with it; placing the card again brings them
back.

<!-- picture: workspace-nary-drawing light | N-ary association chosen and drawing: Customer and Product clicked, the diamond between them with their legs and a dashed leg to the pointer over Catalog; the bar at the top reads N-ary association · 2 ends; Properties shows New n-ary association with Ends so far · 2 -->

<!-- picture: workspace-nary light | The n-ary association recommends selected, its diamond joined by legs to Customer, Product and Catalog; Properties shows Ends · 3 with the editor open on the catalog end: Role, Type, Multiplicity, Navigable, Ordered and Unique -->

<!-- picture: workspace-nary-binary light | Properties of recommends after its catalog end's bin: Two ends would remain, with Keep 3 ends focused and Make it binary -->

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
  parameter: every member typed by it follows, and so does every binding (see **Bindings** below).
  Changing its kind keeps
  the parameter too, and the members typed by it follow.
- **What is refused.** A name another parameter of the template has, whatever the case: *Repository
  already has a template parameter named T. Pick another name.* A default that does not fit is
  refused under **Default**. The reason shows in the editor, the editor stays open, and nothing
  reaches the model or the undo history. Deleting a generalization or a class so that a
  parameter's default no longer specialises its constraint is refused too, naming the default;
  where another path of generalizations keeps it fitting, the deletion goes ahead.
- **Parameters a binding uses.** A parameter a binding (see **Template bindings** below) gives an
  argument cannot be removed: the bin says, for example, *OrderRepository binds Key -> Integer.
  Change or remove that binding first, then remove Key.*, under the rows, and **Show the binding**
  selects the binding itself, its line on the canvas and its **Properties**. A template's last
  parameter cannot be removed while any binding names the template, since the template would stop
  being one under it. Its kind or constraint cannot change so that an argument no longer fits
  either. A binding that leaves a parameter out uses its default, so a parameter without a default
  cannot be added to a template that is bound, and a default cannot be cleared while a binding
  relies on it. Deleting a template deletes the bindings to it.

<!-- picture: workspace-template light | Repository with its signature box T : Class and Key : DataType = Integer over its card's top-right corner, items : T [*], find(key : Key) : T and save(item : T) on the card; Properties shows Template parameters · 2 with the editor open on Key: Kind Data type, Constrained to Any, Default Integer -->

<!-- picture: workspace-template-kinds light | The same, the Kind menu open: Class, Interface, Data type ticked, Enumeration and Primitive type, then Value and Operation disabled, saying not yet -->

<!-- picture: workspace-template-in-use light | OrderRepository binds Repository: Key's removal is refused under the rows, naming the binding, with Show the binding under it -->

### Template bindings

A *template binding* says that a class is a template with its parameters filled in:
*OrderRepository* binds *Repository* with `T -> Order` and `Key -> Integer`. It is a relationship of
its own, kept in the model with each *substitution*, a parameter and its argument; it is neither a
dependency nor a realization.

- **Draw one.** Pick **Template binding** in the **Relationship** tool's menu, under **N-ary
  association**, and drag from the class that binds to the template. A class binds a class template, an
  interface an interface template, a data type a data type template. Releasing the line on the
  template writes nothing yet: **Properties** opens *New template binding*, *not in the model
  until you apply*, with the **Bound element**, the **Template**, and **Substitutions**, one row
  per parameter. Each selector lists only the arguments that fit the parameter: its kind, and its
  constraint or what specialises it. A parameter with a default shows it, tagged *default*. While
  the draft is open the line stays dashed in the accent colour.
- **Apply** adds the binding, its substitutions and its line as one step you can undo, and selects
  it. A parameter without a default needs an argument: **Apply** then says, for example, *T needs
  an argument: pick a class. Key can keep its default.*, its selector ringed, and keeps the draft.
  **Cancel** or Escape removes the draft and leaves the model and its history as they were;
  selecting something else does the same.
- **On a diagram** the line is dashed with an open arrowhead at the template, and reads `«bind» <T
  -> Order, Key -> Integer>`, each parameter with its argument, a parameter left on its default
  with the default. Every diagram that shows the binding shows the same line and text. **Models**
  writes *binds Repository* after the class that binds.
- **Change one.** Select the binding: **Properties** shows its **Binding**, the bound element and the
  template, and its **Substitutions**. Picking another argument, or **Default** to go back to the
  parameter's default, is one step you can undo. Another bound element keeps the arguments.
  Another template opens the draft over the binding, its own arguments to pick before **Apply**.
  On the canvas the binding's start drags onto another class; its end drags onto another template
  only when that template needs no argument, else the canvas says to pick it in **Properties**.
- **What is refused.** A class that is no class, interface or data type of the model, a template
  of another kind, the template itself, a second binding of the same template, and a template that
  binds the class already, directly or through other bindings: the canvas says why and draws
  nothing. Renaming a parameter or an argument changes the line's text and nothing else: the
  binding follows them. Deleting a class that a binding uses as an argument is refused, naming the
  binding: change or remove that binding first. Deleting the class that binds deletes its
  bindings, and deleting a template deletes the bindings to it.

<!-- picture: workspace-template-binding light | OrderRepository binds Repository: the dashed line labelled «bind» <T -> Order, Key -> Integer> selected, Properties showing the Binding with Bound element and Template, and Substitutions · 2 -->

<!-- picture: workspace-template-binding-new light | A binding being drawn from OrderRepository to Repository: New template binding in Properties, Key on its default Integer, and Apply refused: T needs an argument: pick a class. Key can keep its default. -->

### Profiles and stereotypes

A *profile* extends UML for your own project: it holds *stereotypes*, such as *Entity*, each saying
which kinds of element it is for and which values it carries. Profiles, stereotypes and what they
extend are UML model elements of their own, saved with the model, not labels or tags.

- **Make a profile.** Right-click a package in **Models**, the root package included (or select it
  and press ⇧F10), and choose **New ▸ Profile**. **New** also offers **Package** and the
  classifier kinds. The profile sits in the package beside its packages, with a muted *profile*
  after its name, and its name is ready to type. A profile never goes inside a class.
- **Make a stereotype.** Right-click the profile and choose **New ▸ Stereotype**, or use **+** under
  **Stereotypes** in the profile's **Properties**. In **Models** each stereotype is listed under its
  profile, with what it extends after its name, such as *extends Class*.
- **What a stereotype extends.** In the stereotype's **Properties**, **+** under **Extends** lists
  the metaclasses Archforge draws: Class, Interface, Enumeration, DataType, Property, Operation,
  Association, Generalization and Package. Other UML metaclasses, such as Actor or UseCase, are
  listed but disabled, *not drawn by Archforge yet*. The pencil on an extension changes it to
  another metaclass or removes it. **Profile** moves the stereotype to another profile.
- **Its properties.** Under **Properties**, **+** opens the same editor as a class's attributes.
  A stereotype's property is typed by a primitive type, an enumeration or a data type; the type
  menu offers those, and the built-in types. Names starting *base_* are kept for the stereotype's
  extensions.
- **The profile's diagram.** **Open profile diagram** in the profile's **Properties** opens its
  diagram, made the first time, showing each stereotype as a «stereotype» card under the
  «metaclass» cards of what it extends, joined by a line with a filled triangle at the metaclass.
  Each time, it adds what the diagram lacks below what it shows, as one step you can undo; cards
  you placed stay where they are.
  Drag a stereotype's row from **Models** onto it to show it there with its metaclasses. The
  metaclasses themselves are UML's own: they never appear in **Models**, the search or the type
  menus, and nothing renames, moves or deletes them.
- **Apply a profile to a package.** In a package's **Properties**, **+** under **Applied profiles**
  lists every profile of the model; a profile applied already is ticked. Applying is one step you
  can undo and adds no stereotype by itself: the elements inside the package, and inside its
  packages, can then use the profile's stereotypes. The package's row in **Models** says *applies
  Persistence* after its name. The pencil on an applied profile offers **Unapply**.
- **Nothing in use is dropped.** Once elements use stereotypes, unapplying a profile they need, deleting a stereotype or its profile,
  dropping a metaclass the elements need, moving a stereotype to another profile, deleting a
  stereotype's property whose values are set, and moving or deleting the package or element they
  sit in out of the profile's reach are refused, the uses counted: *Persistence is in use in
  Commerce: «Entity» on Order and Customer, «Id» on 2 elements, «Reference» on places. Remove
  those first.* **Show the 5 uses** selects them. Renaming a profile or a stereotype keeps every
  use. Deleting a package refuses the same way when its elements would lose the profile that
  reaches them, and when it would take along a profile that is in use.
- **Deleting a used element** itself, such as *Order*, asks first, from **Models** too, and the
  dialog names the stereotype applications that go with it and with what it takes along: *3
  stereotype applications go with it: «Entity» on Order, «Id» on id,
  «Reference» on places.* **Delete** removes them in the same step: an application means nothing
  without its element.
- **Deleting** an unused stereotype takes its extensions along; deleting an unused profile takes
  its stereotypes and its applications along, each as one step you can undo.

<!-- picture: workspace-profiles light | The profile Persistence beside Commerce in Models, its stereotypes Entity, Id and Reference listed under it with what each extends; its diagram shows the three «stereotype» cards under the «metaclass» cards Class, Property and Association, joined by lines with filled triangles; Properties shows Entity's name, profile, Extends Class and its properties, the editor open on cached -->

<!-- picture: workspace-profile-refused light | Commerce selected, applying Persistence; unapplying it refused under Applied profiles, naming the uses, with Show the 5 uses -->

#### Applying stereotypes

Once a package applies a profile, its elements, and those of its packages, can carry the profile's
stereotypes. A stereotype and its values are model data, saved with the model.

- **Apply a stereotype.** Select the element: a class, an interface, an enumeration, a data type,
  a primitive type or an association class; an attribute or an operation (select its row in its
  class's **Properties**, and the section is *Stereotypes of id*); an association, a
  generalization or a package; the end of an association between two classes, in that end's part
  of the association's **Properties**. An n-ary association's ends have no **Stereotypes** section
  yet: only an agent can apply stereotypes to them (see
  [Working with an agent](07-agents.md#what-the-agent-can-do)). **+** under **Stereotypes** in its **Properties** lists every
  stereotype of the model. Those that extend the element's kind and whose profile is applied to its
  package, or a package around it, come first; one applied already is ticked. After a line, the
  others are listed but disabled, with why: *properties only* for a stereotype that extends
  Property, *Logging not applied* for one whose profile no package around the element applies.
  Type to filter the list, **↑** and **↓** move, **Enter** applies, **Esc** closes. Applying is one
  step you can undo. **Stereotypes** shows only once the model has a stereotype.
- **Set its values.** Under the stereotype's row each of its properties has a control fitting its
  type: a Boolean or an enumeration a select, any other a field. A property you have not set shows
  its default, muted with a *default* tag; a value you set has no tag, even when it says the same
  as the default. Type a value and press **Enter**, or pick one from the select; its first row,
  *Default (false)*, clears it so the default shows again, as an empty field does. A property that
  holds several values takes them separated by commas. A value its type does not read stays in
  the field with the reason under it, and nothing is written: *many is no value of rows. A whole
  number, such as 42 or -7.* So are too many or too few values for the property's multiplicity,
  and the same value twice where each is held once.
- **Remove a stereotype** with the bin on its row. Its values go with it; the stereotype's
  definition stays in its profile.
- **Removing an attribute or an operation** in its editor asks first when stereotypes are applied
  to it, naming them, as deleting any element that carries stereotypes does.
- **On the diagram** a stereotype takes the place of the kind's word on the card: *«Entity»* for
  *CLASS*. A kind written as a keyword keeps it first: *«interface, Entity»*. An attribute's or an
  operation's row starts with its stereotypes, *«Id» + id : Integer*, in **Properties** too, and a
  line's stereotypes sit above its name: *«Reference»*. An n-ary association's sit above its
  diamond's name, and an end's at the middle of its leg. **Models** shows them, muted, after the
  element's name.
- **Show stereotype values on this diagram**, under the section while a class diagram is open,
  lists the values set on each card in a compartment per stereotype, *«Entity»* then *table =
  "orders"*; a property left to its default is not listed. Each diagram keeps its own choice.
  Turned on, cards that would hide a value grow, in the same step.
- **Nothing is lost unseen.** While a value is set, the stereotype's property is not deleted, given
  another type, or given a multiplicity the value breaks, and a literal a value names is not
  deleted: each is refused, saying which value to change first.

<!-- picture: workspace-stereotypes light | Order selected on Commerce's diagram: its card reads «Entity» over its name, «Id» + id : Integer, and an «Entity» compartment with table = "orders"; «Reference» above places; Properties shows Stereotypes · 1 with «Entity» Persistence, table set to orders and cached false with the default tag, and Show stereotype values on this diagram ticked -->

<!-- picture: workspace-stereotype-picker light | The Stereotypes picker open on Order: APPLY TO ORDER, Entity ticked, Auditable to apply, then Id (properties only), Reference (associations only) and Tracked (Logging not applied) disabled -->

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
