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
  packages with `uml_package_list`, a class's relationships with `uml_relationship_list` and an
  element's constraints with `uml_constraint_list`, or, with `--compact`, `execute_command` given
  the command ids `uml.package.list`, `uml.relationship.list` and `uml.constraint.list`.
- **Constraints**: `uml_constraint_add`, `uml_constraint_set`, `uml_constraint_move` and
  `uml_constraint_show`, which shows a constraint on a class diagram as braces or as a linked
  note. The expression is kept exactly as sent and never evaluated; an empty one, or an element
  outside the model, is refused. A constraint is removed with `element_remove`.
- **Qualifiers**: `uml_qualifier_add` qualifies one end of an association or an association class
  (`end` 0 or 1, the end whose class does the picking) with a name, a type and a multiplicity;
  `uml_qualifier_set` changes them, a field left out keeping its value; `uml_qualifier_move` puts
  one at another place among its end's. A name another qualifier of the end has, whatever the case,
  is refused. `uml_relationship_list` gives each end's qualifiers. A qualifier is removed with
  `element_remove`.
- **Templates**: `uml_template_parameter_add` gives a class, an interface or a data type a
  template parameter (`kind` class, interface, data-type, enumeration or primitive-type, an
  optional `constraint` and `default`, each a classifier's id or a built-in type's name), making it
  a template; `uml_template_parameter_set` changes one, a field left out keeping its value;
  `uml_template_parameter_move` puts one at another place; `uml_template_parameter_list` lists them
  with the bindings that use them, and `uml_class_get` gives them too. The kinds `value` and
  `operation` are refused as not supported yet, never turned into another kind. A name another
  parameter of the template has, whatever the case, a constraint of another kind and a default that
  does not fit are refused. A parameter's `element` types the template's own attributes,
  operations and parameters, and no other classifier's. A parameter is removed with
  `element_remove`, refused while a binding uses it; the template's last parameter takes its
  signature along, and is refused while any binding names the template. `element_remove` also
  refuses removing a generalization or a classifier when a parameter's default, or a binding's
  argument, would then no longer specialize the parameter's constraint. No command makes a
  binding in this version yet, so `bound_by` stays empty in a model made here.
- **Generalization sets**: `uml_generalization_set_add` groups generalizations to one general
  class, complete or incomplete and disjoint or overlapping; `uml_generalization_set_set` renames
  it, changes either flag, and adds or takes out members; `uml_generalization_set_place` puts its
  label somewhere on a class diagram; `uml_generalization_set_list` lists the sets of the model, of
  a class or of a generalization. Generalizations to different classes are refused. A set is
  removed with `element_remove`, which keeps its generalizations; a set left with no member goes
  with its last one. A generalization in a set cannot be reconnected until it is taken out.
- **Nested classifiers**: `uml_nested_add` makes a class, an interface, an enumeration or a data
  type inside a class, an interface or an association class; `uml_classifier_set_owner` moves any
  classifier under a package or a classifier, keeping its id, members, relationships and the types
  that use it, and refuses a cycle, an owner that owns no classifiers and a name the owner holds
  already, with the reason; only a class, an interface, an enumeration or a data type goes under a
  classifier, so a primitive type and an association class stay in packages, and a built-in type
  does not move at all; `uml_nested_list` lists what a classifier nests and
  `uml_classifier_owners` where a classifier may go, each owner allowed or with its reason.
  `uml_class_get` says a classifier's owner and what it nests. Removing an owner with
  `element_remove` removes what it nests.

Ask it in your own words, for example: *"Add a table Invoice to the Shop schema with an id
primary key and a foreign key to Order."* The table appears on screen as the agent adds it.

## Setting up AI agents for Chat

Chat, coming in a later version, talks to an agent program installed on your computer: Claude
through the program `claude`, or Codex through the program `codex`. Archforge never bundles them.
You set them up in **Settings ▸ AI Agents**: click the avatar at the right of the bar, or press
⌘, (Ctrl ,), and choose **AI Agents** on the left.

<!-- picture: settings-ai-agents-missing light | Settings, AI Agents: Claude's program not found, with its install steps, and Codex on a saved API key -->

- **Default agent** is the one Chat starts with.
- **Claude** uses an API key from your Anthropic account. Paste it and choose **Save key**.
  Archforge keeps it in the system's secure store (the macOS Keychain, or the Secret Service on
  Linux) and nowhere else: never in the project and never in the preferences. Afterwards it shows
  only the key's last four characters, with **Replace** and **Remove**. Its model can be chosen
  once a key is saved.
- **Codex** uses an API key from your OpenAI account: choose **API key** and save it as for Claude.
  **Sign in with ChatGPT** is not available in this version yet; the page says so. It becomes
  available once Archforge drives the codex program's own sign-in: **Sign in…** will let the codex
  program open your browser, and on a computer without a browser **Sign in with a device code**
  will show a code to enter on any device with a browser. The codex program will keep that
  sign-in itself.
- **Let Chat use the model** is off until you turn it on. While a conversation holds this access,
  Chat reads the project's model and the status bar says so; changes still wait for **Allow
  changes** in each conversation.

Opening the page sends nothing over the network. Archforge looked for the programs when it
started; your keys are read from the secure store the first time you open the page, never when
Archforge starts, so the system asks for access only once you use AI agents.

- **Check connection** is free and runs on your computer: it looks again for each program and its
  version and reads which API keys are saved, and sends nothing. The badge under it sums up each
  agent. It cannot read a Codex ChatGPT sign-in yet; that comes with the sign-in itself.
- **Send test message** is not available in this version yet. Once Archforge talks to the agents'
  providers, it will send one short message to the default agent's provider, which uses a little
  of your plan or API credit.

When a program is missing, its card shows how to install it, as its maker documents it:

```sh
npm install -g @anthropic-ai/claude-code
npm install -g @openai/codex
```

Then choose **Look again**; **Installation guide** opens the maker's own page. A program newer than
the version Archforge was tested with works, and its badge says *Not yet tested with this version*.

On Linux without a running Secret Service (such as GNOME Keyring or KWallet), API keys cannot be
saved until you start one and restart Archforge, and Archforge keeps them nowhere else. Once
Sign in with ChatGPT is available, it will work without a secure store, as the codex program
keeps that sign-in in its own files. On Windows, AI Agents are not available yet; everything else
works as usual.

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
