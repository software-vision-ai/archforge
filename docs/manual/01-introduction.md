# What Archforge is

Archforge is a modelling application for macOS, Windows and Linux. You describe a software
system as a model, draw it on diagrams, and an AI agent can read and change the same model while
you watch. Every change, yours or the agent's, shows in the window at once and can be undone.

## What it does today

- **ER diagrams** for relational databases: schemas with tables, columns, primary and foreign keys,
  relationships with their cardinality, and checks, indexes, domains, enum types, subtypes,
  sequences and views.
- **UML class diagrams**: packages with classes, their attributes and operations, and
  associations, aggregations, compositions, generalizations and dependencies, with roles,
  multiplicities and navigability.
- **Skills**: guidance written on the model, on the whole project or on one element, which tells
  people and agents how to work there (naming rules, how to write SQL, and so on).
- **Agent access**: an agent connected through Archforge's command-line tool works on the open
  project with the same commands you use, and reads the model and its skills.
- **Undo and saving**: every change is one step you can undo; edits are written to the project as
  you go, and Save makes a version.
- Light and dark appearances, and an interface in English, German, Spanish, French, Portuguese,
  Japanese and simplified Chinese.

## What is planned

These are not in Archforge yet. Where the window shows a place for them, it says so.

- C4 architecture diagrams.
- A chat with an agent inside the window: the Chat panel says *Coming soon*. Today an agent works
  from outside, through agent access (see [Working with an agent](07-agents.md)).
- Source control for projects, beyond the versions Save makes.
- A menu bar with every command; on macOS, the application and File menus exist already.
- Ready-made packages to install; today Archforge is built from its source.

## Words used in this manual

- **Project**: the folder Archforge keeps your work in. A project holds one model and its
  diagrams.
- **Model**: everything the project describes, such as the schemas, tables, packages and classes,
  whether or not a diagram shows them.
- **Element**: one thing in the model: a table, a column, a class, a relationship.
- **Diagram**: a drawing of part of the model. The same element can appear on several diagrams.
- **Box** and **line**: how a diagram draws an element. A table or a class is a box; a relationship
  is a line.
- **Agent**: an AI assistant, connected from outside Archforge, that works on the open project.

Next: [Getting started](02-getting-started.md).
