# A first ER model

This chapter builds a small database model: a schema *Shop* with two tables, *Customer* and
*Order*, where a customer places many orders. Start from a new, empty project (see
[Getting started](02-getting-started.md)).

## 1. Make a schema and a diagram

1. In **Models**, choose **+**. A schema appears, named *Schema 1* and ready to type. Type *Shop* and
   press Enter.
2. Choose the **+** at the start of the centre's tab strip. The **New diagram** dialog first asks
   what the diagram shows: keep the **ER diagram** card and choose **Continue**. Then it asks for a
   name and a schema: choose the schema *Shop*, name the diagram *Shop schema* and choose
   **Create**. (Without a schema, the dialog offers **New schema**.)

The empty diagram opens in its tab.

<!-- picture: workspace-diagram-empty light | A new, empty ER diagram with its tools at the bottom -->

## 2. Add the tables

1. Choose the **Entity** tool in the toolbar at the bottom, then click on the canvas. A table
   appears with its name ready to type: type *Customer* and press Enter.
2. Do the same for a table *Order*, to the right of *Customer*.

After placing one table, the tool goes back to **Select**.

## 3. Add columns and keys

1. Select *Customer*. In **Properties**, the **Columns** section has a **+**: choose it to add a
   column, or use the **Add column** button beside the selected table on the canvas.
2. The column's editor opens. Name it *customer_id*, choose the type *BIGINT* and tick
   **Primary key** (which also ticks **Not null**). Choose the check at the editor's foot (Apply),
   or press Enter.
3. Add *email* (type *VARCHAR*, length *255*, and tick **Unique**) and *name* (*VARCHAR*, length
   *120*) the same way.
4. Select *Order* and add *order_id* (*BIGINT*, primary key) and *placed_at* (*TIMESTAMP*).

<!-- picture: workspace-column-editor light | A column's editor in Properties: name, type, primary key and the other flags -->

A primary-key column shows **PK** at the left of its row on the table.

## 4. Draw the relationship

1. Choose the **Relationship** tool. Its chevron opens the cardinality popover: choose
   one-to-many, drawn as one bar at the parent end and a crow's foot at the child end.
2. Press on *Customer* and drag onto *Order*, then let go.

<!-- picture: workspace-cardinality light | The Relationship tool's popover: the ends the next line gets -->

The line joins the two tables, and Archforge adds a foreign key to *Order*: a column that refers
to *Customer*'s primary key, marked **FK**. Select the line to name it (for example *places*) and
to change its ends in **Properties**. A solid line is identifying, a dashed one non-identifying.

## The result

You now have a schema *Shop* with two tables, their primary keys, and a one-to-many relationship
whose foreign key Archforge made for you. Models lists the schema with its tables and columns, and
**Properties** of *Customer* shows its columns and its keys.

<!-- picture: workspace-diagram light | The finished model: Customer places Orders -->

Press ⌘S (Ctrl S) to save a version. From here you can add checks, indexes, domains, enum types,
subtypes, sequences and views from a table's or a schema's **Properties**.

Next: [A first UML class model](05-uml-class-model.md).
