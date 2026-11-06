# Working with an agent

An AI agent can work in the project you have open while you watch. It reads the model and the
skills that apply, and it changes the model with the same commands you use, so every change it
makes shows in the window at once and ⌘Z (Ctrl Z) takes it back.

## What does what

- **Archforge** holds the model, draws it, checks every change, keeps the history and saves. It
  never generates code or text itself and has no built-in chat yet.
- **The agent** runs outside Archforge, in the coding assistant or tool you already use. It talks
  to Archforge through `archforge-cli`, an [MCP](https://modelcontextprotocol.io) server, and it
  does the writing: SQL DDL, documentation, a change to the model you asked for.

## Connecting an agent

Agent access works on macOS and Linux for now.

1. Start Archforge with a control port: on macOS
   `/Applications/Archforge.app/Contents/MacOS/archforge --control 47000`, on Linux
   `archforge --control 47000`, or from the source,

   ```sh
   cargo run -p archforge -- --control 47000
   ```

2. Find the command-line tool. The macOS and Linux packages carry it beside the application:
   `/Applications/Archforge.app/Contents/MacOS/archforge-cli` on macOS, `/usr/bin/archforge-cli`
   from the deb or rpm package, and `bin/archforge-cli` beside `bin/archforge` in the tar.gz. The
   AppImage carries it inside the image, at `usr/bin/archforge-cli` of what
   `--appimage-extract` unpacks. From the source, build it:

   ```sh
   cargo build -p archforge-cli
   ```

3. Let your agent run its MCP server, bridged to that port: `archforge-cli mcp --bridge 47000`,
   with the path of step 2 (`target/debug/archforge-cli` from the source). The repository's
   `.mcp.json` registers the source build for agents that read it.

**File ▸ Generate…** says whether agent access is on. The port listens only on your own computer, and a
token made for each start, kept in a file only you can read in Archforge's settings folder, guards
it. Without `--control`, nothing listens.

For an agent short of context, `archforge-cli mcp --bridge 47000 --compact` lists a few general
tools and `search_tools` in place of one tool per command. To try agent access by hand,
`archforge-cli app <method> [<json>] --port 47000` sends one request to the running window, and
`archforge-cli commands` lists the commands there are.

## Read-only sessions and labelled changes

Two more flags of `archforge-cli mcp` shape what an agent may do. You set them where the MCP server
is started (your agent's MCP settings, or the program that starts the agent); nothing the agent
sends can change them.

| Flag | What it does |
|---|---|
| `--bridge <port>` | Connects to Archforge started with `--control <port>`. Needed. |
| `--compact` | Lists the general tools and `search_tools` in place of one tool per command. |
| `--read-only` | Offers only the tools that read: listing commands, `inspect_document`, the reads such as `er_entity_list` or `uml_class_get` (listed even with `--compact`), and the resources. Every other tool is left out of the list, and a call to one is refused with an error that names it and says the session is read-only. Nothing reaches the model. |
| `--origin <label>` | Labels every change the agent makes. Its undo step is named *label: change*, for example `Chat 1: Add entity`, or `Chat 1: Add tables` for a batch, and each result gives the label back as `origin`. The window does not show undo step names yet; the agent's results and `inspect_document` do, and `inspect_document` names the origin of the steps undo and redo would take (`undo_origin`, `redo_origin`). |

The flags combine. For example, an agent you only want answers from:

```sh
archforge-cli mcp --bridge 47000 --read-only
```

and one whose changes you want to tell apart from yours in the history, for the program that
started it or a later look at `inspect_document`:

```sh
archforge-cli mcp --bridge 47000 --origin "Chat 1"
```

A label is 1 to 64 characters of printable text: letters, digits, spaces and punctuation, in any
language. It may not start or end with a space or hold line breaks, tabs, control characters or
characters that hide or reorder text. A label that breaks these rules is refused and the server
does not start (exit code 2). Your own changes in the window carry no label, nor do those of an
agent started without `--origin`, and a change by one origin never merges into another's undo
step.

Neither flag fences the agent off from your computer: an agent that may run commands could still
start `archforge-cli` itself. Keep its other tools to what you trust it with.

## What the agent can do

- **Change the model**: one tool per command, such as `er_entity_add`, `er_relationship_add`,
  `uml_class_add`, `uml_association_add` or `diagram_place`, plus `batch` to make several changes
  as one undo step, `undo`, `redo`, `inspect_document` to see what is open and selected, and
  `save_project`, which saves as ⌘S does.
- **Read the model and the skills**: `skill_list` and `skill_effective` as tools, and as MCP
  resources `archforge://project` (the open project, its revision, the selection), `archforge://skills`,
  `archforge://element/{id}` (a schema with its tables, a table, a diagram, or a class, an
  interface, an enumeration with its literals, a data type, a primitive type or an association
  class, each with its attributes, operations and description) and
  `archforge://skills/effective/{id}` (the skills that apply to an element, optionally for one
  trigger: modelling, drawing, naming or export). Other elements are read through the tools:
  packages with `uml_package_list` and a class's relationships with `uml_relationship_list`, or,
  with `--compact`, `execute_command` given the command ids `uml.package.list` and
  `uml.relationship.list`.

Ask it in your own words, for example: *"Add a table Invoice to the Shop schema with an id
primary key and a foreign key to Order."* The table appears on screen as the agent adds it.

## The Generate handoff

To have an agent generate something from the model, such as SQL DDL for a schema:

1. Select a schema, table, class or diagram (or none, for the open diagram).
2. Open **Generate…**: ⇧⌘G (Ctrl Shift G), **File ▸ Generate…** on macOS, or **Generate…** in a
   table's or class's right-click menu.
3. Choose the output, **SQL DDL** with its dialect or **Documentation**. The dialog lists the export
   skills that apply, where each comes from, and which one overrides another.
4. Choose **Copy request** and paste it to your agent. The request names the MCP resources the
   agent reads and the model's revision.

<!-- picture: workspace-generate light | The Generate handoff: what it is from, the output, the export skills and the request for the agent -->

The dialog itself changes nothing and generates nothing: the agent does the generating, in its own
window. If the model changes after you copy the request, the dialog says so; copy it again so the
agent reads the current revision.

## A small example to try

1. Build the ER model of [A first ER model](04-er-model.md).
2. Add a skill on the project named *DDL conventions*: *"Write PostgreSQL. Name constraints
   pk_<table> and fk_<table>_<column>."*
3. Select the schema *Shop*, open **Generate…**, keep **SQL DDL**, and copy the request to your
   connected agent.

Expected result: the agent reads the schema and the skill through agent access and answers with
`CREATE TABLE` statements for *Customer* and *Order*, their primary keys, and a foreign key from
*Order* to *Customer*, named as the skill says. Archforge's model is unchanged.

Next: [Help and support](08-help-and-support.md).
