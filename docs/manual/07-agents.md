# Working with an agent

An AI agent can work in the project you have open while you watch. It reads the model and the
skills that apply, and it changes the model with the same commands you use, so every change it
makes shows in the window at once and ⌘Z (Ctrl Z) takes it back.

## What does what

- **Archforge** holds the model, draws it, checks every change, keeps the history and saves. It
  never generates code or text itself.
- **The agent** does the writing: SQL DDL, documentation, a change to the model you asked for. It
  runs either in Archforge's Chat panel (see [Chat](#chat)) or outside Archforge, in the coding
  assistant or tool you already use, talking to Archforge through `archforge-cli`, an
  [MCP](https://modelcontextprotocol.io) server.

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
| `--token-file <path>` | Reads the token from that file, an absolute path, instead of the settings folder's `control-<port>.token`. Archforge gives each Chat conversation a file of its own; you do not need it for your own agent. |
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
  argument, would then no longer specialize the parameter's constraint. A parameter without a
  default is refused on a template a binding names, and so is clearing a default a binding
  relies on.
- **Template bindings**: `uml_template_binding_add` makes `bound` (a class, an interface or a data
  type) bind `template` (one of the same kind with template parameters), with `arguments`, an
  object mapping each parameter's name or id to a classifier's id or a built-in type's name; a
  parameter without a default needs one, and one left out uses its default.
  `uml_template_binding_set` changes the bound element, the template (its arguments given anew) or
  some arguments (`""` takes one back to its default); `uml_template_binding_list` gives a
  classifier's bindings (`binds`) and those to it (`bound_by`), each with its `label`, `«bind» <T ->
  Order, Key -> Integer>`, and every parameter's argument, `default` true where the default is
  used. An argument of another kind, one that does not fit the parameter's constraint, a template
  parameter's own classifier, a template of another kind, a second binding of the same template and
  a cycle of bindings are refused. Show it on a class diagram with `diagram_connect` and its text
  with `diagram_label` (`kind` name). `uml_relationship_reconnect` moves its source to another
  bound element, or its target to another template whose parameters all have defaults.
  `element_remove` removes it with its substitutions; a classifier a binding uses as an argument is
  refused, naming the binding, until that binding changes or goes; removing the bound element or
  the template takes the binding along.
- **N-ary associations**: `uml_association_add_nary` joins three to 32 classes or interfaces in one
  association (`classes`, in order; a class given twice has two ends), each end named after its
  class and starting at `*`. On a class diagram it is a diamond: `diagram_place` with the
  association's id, then `diagram_connect` from the diamond's node to each class's node with the
  end's id (or `diagram_connect_all`), all in one `batch` for one undo step. `uml_association_set_end`
  sets an end by its place among the ends; its role must differ from every other end's, and an
  n-ary end takes no aggregation. `uml_association_add_end`, `uml_association_reconnect_end` and
  `uml_association_remove_end` add, move and remove an end; a leg is drawn or kept only on the
  diagrams that show both the diamond and the end's class, and what the ends subset or redefine
  that a move or removal makes invalid is dropped and listed.
  Removing one of three ends is refused unless `binary` is true, which makes it an ordinary
  association between the two classes left. `uml_relationship_list` gives it with the kind
  `n-ary association` and all its ends.
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
- **Profiles and stereotypes**: `uml_profile_add` makes a profile in a package, the root package
  when none is named; `uml_stereotype_add` makes a stereotype in it, with the metaclasses it
  extends by UML's names (Class, Interface, Enumeration, DataType, Property, Operation,
  Association, Generalization, Package; any other is refused, saying it is not drawn yet);
  `uml_stereotype_set` renames it, moves it to another profile or replaces what it extends;
  `uml_attribute_add` with the stereotype's id adds a property, typed by a primitive type, an
  enumeration or a data type; `uml_profile_apply` and `uml_profile_unapply` apply a profile to a
  package and take it away; `uml_profile_diagram` gives the profile's diagram, made the first time
  and filled with every stereotype, every metaclass they extend and the extension lines, what it
  showed already kept, as the window's Open profile diagram does. Unapplying,
  deleting, dropping an extension, moving a stereotype or moving what a use sits in is refused
  while elements use the stereotypes, the uses named; deleting a used element with
  `element_remove` removes its applications with it. `uml_profile_list`, `uml_stereotype_get`,
  `uml_package_profiles`, `uml_profile_uses` and `uml_metaclass_list` read them; a profile or a
  stereotype is removed with `element_remove`.
- **Stereotypes applied**: `uml_stereotype_apply` applies a stereotype to an element (a classifier,
  an attribute or an association end, an n-ary association's ends included, which the window's
  Properties do not offer yet (their ids come from `uml_association_add_nary` or
  `uml_relationship_list`), an operation, an association, a generalization or a package)
  whose metaclass it extends, its profile applied to the element's package or a package
  around it, each stereotype once; `uml_stereotype_unapply` takes it off with its values, the
  stereotype kept; `uml_stereotype_set_value` sets one property's `values` as text, read by the
  property's type as a default is (a literal's name for an enumeration) and checked against its
  multiplicity, `[]` clearing them so the default shows; `uml_stereotype_show_values` turns a class
  diagram's values compartments on or off; `uml_stereotype_list_applied` reads an element's
  stereotypes with every property's values, `set` telling a value set from a default, and
  `uml_stereotype_applicable` every stereotype with whether it applies and why not. Removing a
  literal a value names, retyping a property whose values are set or giving it a multiplicity a set
  value breaks is refused, naming the value.

Ask it in your own words, for example: *"Add a table Invoice to the Shop schema with an id
primary key and a foreign key to Order."* The table appears on screen as the agent adds it.

## Setting up AI agents for Chat

Chat talks to an agent program installed on your computer: Claude
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
- **Codex** signs in with your ChatGPT account or uses an API key from your OpenAI account. With
  **Sign in with ChatGPT**, **Sign in…** opens the sign-in page in your browser, and on a computer
  without a browser **Sign in with a device code** shows a code to enter on any device with a
  browser. Under the code the card shows the sign-in page's address the codex program sent; select
  it, or copy it with the icon beside it, and open it on that device. Archforge shows the address,
  and **Open sign-in page** opens it in this computer's browser, only when it is on
  `https://auth.openai.com/`; for any other the card says *Not an address Archforge opens*. The
  codex program keeps that sign-in itself: in the system's secure store, or, where there is none,
  in its own files in Archforge's private settings folder. A sign-in not finished
  within 15 minutes, or one the codex program cancels, says so under it; sign in again. **Sign
  out** ends it.
  With **API key**, save the key as for Claude.
- **Let Chat use the model** is off until you turn it on. While it is on, each Chat conversation
  gets agent access of its own while it runs: Chat reads the project's model through it, and a
  conversation changes the model only when **Allow changes** is on in it. Archforge enforces
  that itself: through its agent access, a conversation without Allow changes is refused every
  change, undo and save it asks for. The access opens on your computer alone when a conversation needs it
  and closes when the last conversation ends, at once when you turn the switch off, and when you
  quit.

The status bar says which, at its right beside the project's location: *Chat is using the model*
with a blue dot while a conversation holds agent access, *Agent access off* with a purple dot
otherwise. Click it, or reach it with Tab and press Enter, to open **Settings ▸ AI Agents**.

Opening the page sends nothing over the network. Archforge looked for the programs when it
started; your keys are read from the secure store the first time you open the page, never when
Archforge starts, so the system asks for access only once you use AI agents. Who Codex is signed
in as is read then too, by starting the codex program on your computer; it is never started just
because Archforge did. It is read again each time you open the page, when you choose **Check
connection**, and when a Codex conversation says *Sign-in expired*, so a sign-in that expires while
Archforge runs shows as expired, with its email and **Sign in again…**, without a restart.

- **Check connection** is free and runs on your computer: it looks again for each program and its
  version, reads which API keys are saved and who Codex is signed in as, and sends nothing. The
  badge under it sums up each agent.
- **Send test message** sends one short message to the default agent's provider, which uses a
  little of your plan or API credit. It runs without access to your model, and leaves nothing
  behind on your computer.

When a program is missing, its card shows how to install it, as its maker documents it:

```sh
npm install -g @anthropic-ai/claude-code
npm install -g @openai/codex
```

Then choose **Look again**; **Installation guide** opens the maker's own page. A program newer than
the version Archforge was tested with works, and its badge says *Not yet tested with this version*.

On Linux without a running Secret Service (such as GNOME Keyring or KWallet), API keys cannot be
saved until you start one and restart Archforge, and Archforge keeps them nowhere else. Sign in
with ChatGPT works without a secure store: the codex program then keeps that sign-in in its own
files, in Archforge's private settings folder, readable by you alone. On Windows, AI Agents are not available yet; everything else
works as usual.

## Chat

The Chat panel is the speech bubble at the top of the right rail. It holds the open project's
conversations with Claude or Codex.

<!-- picture: workspace-chat light | The Chat panel ready: the intro, the context chips, the composer with Claude's badge, Allow changes off -->

Until an agent is set up, the panel says *No agent set up yet*; **Configure agent** opens
**Settings ▸ AI Agents** (see [Setting up AI agents for Chat](#setting-up-ai-agents-for-chat)).
Chat answers once the default agent is ready and **Let Chat use the model** is on.

- **Ask** in the box at the foot of the panel. Enter sends; Shift Enter starts a new line. Your
  question is kept in the project first, then sent. A question that holds one of your saved API
  keys is neither sent nor kept: the panel says so and leaves it in the box for you to edit. So
  does a project that cannot be written.
- **The chips** above the box are sent with your question: the project, the open diagram and what
  is selected, by name. Remove one with its ×; it stays removed for this conversation. The line
  under them is a reminder: they, and everything the agent reads, go to the agent's provider.
- **What the agent does** shows as it happens, under its name: *Read diagram Commerce · 8
  classes*, *Changed Customer · description*. Each row is what Archforge itself answered the
  agent, so a refused change says *refused*. The reply is written in after them.
- **Stop** (the square in place of Send) ends the reply; what was written so far stays, marked as
  stopped. If the agent's program does not stop within ten seconds, Archforge ends it. While a reply runs you can type the next question, but Enter, the badge, the switch,
  New conversation and the history wait: *Stop the reply first*.
- **Allow changes to the model** is off for every conversation, and every time one is opened again:
  the agent can then only read. Turn it on and the agent may change the model; everything one
  question changes is **one step** of the project's undo history, named *Chat 1: Turn 2*, as long
  as nothing else changes the model while the reply runs. The green card under the changes offers
  **Undo this turn's changes**, which takes back exactly those changes and keeps your own. If you
  or another conversation change the model while the reply runs, the question's changes become
  separate steps around yours. Then, or once the model was changed since, or the project was
  closed in between, the card says the changes can no longer be undone on their own; ⌘Z (Ctrl Z)
  still works step by step.
- **The badge** in the box names the agent and its model, *Claude · Sonnet*, then the exact version
  once the agent names it, *Claude · Sonnet 4.5*. Choose another model there; before the first
  question you can choose the other agent too. A conversation keeps its agent and model: changing
  the default in Settings never switches a conversation already started.

When something goes wrong, a card in the conversation says what and what to do:

<!-- picture: workspace-chat-network-lost light | Connection lost: the question back in the box, ready to send again -->

- *Rate limited*: the provider asks Chat to wait. When it says until when, the card names the time
  and **Send again** waits for it; otherwise **Send again** is there at once.
- *Connection lost*: when it went before the agent answered anything and the box is empty, your
  question goes back into the box, ready to send again, edited or not; otherwise it stays where it
  was. **Send again** sends it again.
- *Model not available*: choose another model in the badge, then **Send again**, or check the key
  in Settings.
- A missing program, a refused key, no key or an expired sign-in: **Open Settings**.
- *No answer*: anything else; **Send again**.

**Send again** asks the same question again in the same place: the conversation keeps it once.
A question whose turn already changed the model is not sent again; ask again in your own words.

**Conversations** (the clock at the top of the panel) lists the project's conversations, newest
first, with when, the agent and how many messages. Click one to open it again, read-only until you
allow changes. **New conversation** (+) starts another. Conversations are kept in the project: they
travel with it, never show in another project, and are not unsaved changes. Leaving the project's
tab stops a reply that runs; closing the project ends its conversation.

### Deleting a conversation

Point at a conversation in the list and click its bin, or right-click it (or press the menu key,
or Shift F10) and choose **Delete conversation**. With the keyboard on a row, ⌫ (Delete on Windows
and Linux) does the same. Archforge asks first:

<!-- picture: workspace-chat-delete-confirm light | Delete this conversation? Its title, messages, agent and age, and what deleting it does -->

- **It is removed from this project's conversation list.** Nothing else changes: not the model,
  not its undo history, not whether the project has unsaved changes.
- **It stays in the project's saved history.** Archforge keeps every version of a project, so
  anyone with a copy of the project can still find the conversation there. Deleting it does not
  erase it.
- If its reply is still running, the reply stops first; what was written so far is kept with the
  conversation.

Cancel (where the keyboard starts) or Esc keeps it. Delete takes it out of the list; if it was the
open conversation, the next one in the list opens (or the one above, or a new one).

A note at the foot of the window says it was deleted and is still in the project's saved history,
with **Undo**. Undo brings the conversation back, whole and in its place. The note stays about
eight seconds, longer while you point at it or the keyboard is on Undo. ⌘Z (Ctrl Z) never deletes
or brings back a conversation: it stays the model's undo.

<!-- picture: workspace-chat-deleted light | The conversation deleted: the list without it, and the note with Undo -->

When the note has gone (or the project closes, or Archforge quits), the agent program's own working
files for that conversation are removed from your computer. If a copy of the project open beside
it asks that conversation something in those few seconds, the answer is *No answer*; its files are
then kept, and **Send again** a moment later goes through. When the project's conversations
cannot be changed (for example because one of its tables is damaged), the bin and the menu item
are greyed out and say why.

Links in a reply open only after you confirm them, and only web addresses; pictures show their
description, and HTML shows as text.

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
