# What Archforge is

Archforge is a software modelling tool for macOS, Windows and Linux. You describe a software
system as a model and draw it on diagrams. On macOS and Linux, an AI agent can also read and change
the same model while you watch (agent access is not available on Windows yet). Every change, yours
or the agent's, shows in the window at once and can be undone.

A model covers more than one aspect of a system. Class modelling is where Archforge's
development started, and a system's databases are one more aspect, kept in the same model. Those
are the two kinds of diagram Archforge has today; what is planned is listed below.

## What it does today

- **UML class diagrams** for a system's structure: packages with classes, interfaces,
  enumerations, data types and primitive types; attributes and operations, with default values
  and other property modifiers; and associations, association classes, aggregations,
  compositions, generalizations, dependencies and interface realizations, with roles,
  multiplicities and navigability.
- **ER diagrams** for a system's relational databases: schemas with tables, columns, primary and
  foreign keys, relationships with their cardinality, and checks, indexes, domains, enum types,
  subtypes, sequences and views.
- **Skills**: guidance written on the model, on the whole project or on one element, which tells
  people and agents how to work there (naming rules, how to write SQL, and so on).
- **Agent access**: an agent connected through Archforge's command-line tool works on the open
  project with the same commands you use, and reads the model and its skills.
- **Undo and saving**: every change is one step you can undo; edits are written to the project as
  you go, and Save makes a version.
- **Chat**: ask Claude or Codex about the model in the Chat panel; with your permission it changes
  the model, and one step undoes what a question changed, as long as nothing else changed the
  model while it answered. It runs the agent's program installed on
  your computer, with your own API key or ChatGPT sign-in (see
  [Setting up AI agents for Chat](07-agents.md#setting-up-ai-agents-for-chat)).
- Light and dark appearances, and an interface in English, German, Spanish, French, Portuguese,
  Japanese and simplified Chinese.

## What is planned

These are not in Archforge yet. Where the window shows a place for them, it says so.

- C4 diagrams of a system's architecture.
- Source control for projects, beyond the versions Save makes.
- A menu bar with every command; for now it holds macOS's application, File, Edit, Model and Versions
  menus and, on Windows and Linux, **File**, **Edit**, **Model**, **Versions** and **Help** in the title strip.
- Signed packages: today's packages are not signed, so macOS and Windows ask before they first
  open Archforge (see [Getting started](02-getting-started.md)).

## Words used in this manual

- **Project**: the folder Archforge keeps your work in. A project holds one model and its
  diagrams.
- **Model**: everything the project describes about a software system, such as its packages,
  classes, schemas and tables, whether or not a diagram shows them.
- **Element**: one thing in the model: a table, a column, a class, a relationship.
- **Diagram**: a drawing of part of the model. The same element can appear on several diagrams.
- **Box** and **line**: how a diagram draws an element. A table or a class is a box; a relationship
  is a line.
- **Agent**: an AI assistant that works on the open project: Claude or Codex in the Chat panel,
  or one you connect from outside Archforge.
- **Provider**: the company that runs an agent's AI model: Anthropic for Claude, OpenAI for Codex.

Next: [Getting started](02-getting-started.md).
