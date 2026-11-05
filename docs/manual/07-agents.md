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

1. Start Archforge with a control port:

   ```sh
   cargo run -p archforge -- --control 47000
   ```

2. Build the command-line tool:

   ```sh
   cargo build -p archforge-cli
   ```

3. Let your agent run its MCP server, bridged to that port: `target/debug/archforge-cli mcp --bridge
   47000`. The repository's `.mcp.json` registers exactly this for agents that read it.

**File ▸ Generate…** says whether agent access is on. The port listens only on your own computer, and a
token made for each start, kept in a file only you can read in Archforge's settings folder, guards
it. Without `--control`, nothing listens.

For an agent short of context, `archforge-cli mcp --bridge 47000 --compact` lists a few general
tools and `search_tools` in place of one tool per command. To try agent access by hand,
`archforge-cli app <method> [<json>] --port 47000` sends one request to the running window, and
`archforge-cli commands` lists the commands there are.

## What the agent can do

- **Change the model**: one tool per command, such as `er_entity_add`, `er_relationship_add`,
  `uml_class_add`, `uml_association_add` or `diagram_place`, plus `batch` to make several changes
  as one undo step, `undo`, `redo`, `inspect_document` to see what is open and selected, and
  `save_project`, which saves as ⌘S does.
- **Read the model and the skills**: `skill_list` and `skill_effective` as tools, and as MCP
  resources `archforge://project` (the open project, its revision, the selection), `archforge://skills`,
  `archforge://element/{id}` (a schema with its tables, a table, a class, an interface, an enumeration with its literals, a data type or a diagram) and
  `archforge://skills/effective/{id}` (the skills that apply to an element, optionally for one
  trigger: modelling, drawing, naming or export).

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
