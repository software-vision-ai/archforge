# A first UML class model

This chapter builds a small class model: a class *Customer* that places many *Order*s. It can live
in the same project as the ER model of the previous chapter, or in a new one.

Every UML model has one **root package**, named after the project, that holds everything without
another home: the classes and packages you add without choosing a package, and the first class
diagram. You don't need to set it up. Archforge makes it with your first class diagram (or your
first package), and you can rename it like any package.

## 1. Make a class diagram

1. Choose the **+** at the start of the centre's tab strip.
2. In the **New diagram** dialog choose **Class diagram**. Its second field, **Package**, shows your
   project's name: the root package, made when you choose **Create**. Name the diagram *Commerce*
   and choose **Create**.

The root package is the top row of **Models**, with what it holds below it. Once it exists, the
**Package** field of a later class diagram offers it and every other package, and **New package**
makes a new package inside the root, named like the diagram.

## 2. Add classes

1. Choose the **Class** tool in the toolbar at the bottom, then click on the canvas. A class
   appears with its name ready to type: type *Customer* and press Enter.
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
   directed association, aggregation, composition, generalization and dependency.)
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

## The result

The root package holds two classes. *Customer* has an attribute and an operation, and a navigable
association *places* leads to many *Order*s. In Models, open the root package (your project's name),
then a class, to see its attributes and operations.

<!-- picture: workspace-class-diagram light | A class diagram of a small commerce model -->

Drag a class's row onto another package's row in Models to move it there; a class of another
package on a diagram is drawn dimmed, its package named above its name. Removing a package hands
what it holds up to the package around it, in the end the root package, which itself stays.
A project made before the root package existed gets one when it opens: its top-level packages move
into it, as one step you can undo.

Next: [Skills](06-skills.md).
