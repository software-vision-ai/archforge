# AR-206: keeping conversations in the project — data model

**Signed off by the owner on 2026-11-06**, with four decisions on the open questions of section 10:
(1) each finished turn commits only the chat tables, in a Dolt commit of its own; chats never make
the project unsaved, and Don't save resets only the model, never the chat tables; (2) project format
2 → 3: new projects are made in format 3, and an older project becomes format 3 only when it first
holds a conversation, by the rule of section 5; (3) a deleted conversation stays in the project's
earlier Dolt commits; (4) every other recommendation as written.

Written for the owner before anything was built (AR-173 decision 6, AR-42), read from `main` at
`bd11b67`: `crates/project` (`lib.rs`, `model.rs`), `crates/engine`
(`session.rs`), `crates/mof` (`schema.rs`, `id.rs`, `store.rs`), `apps/archforge/src/projects.rs`,
`docs/manual/07-agents.md`, and the runtime note `docs/agent-runtimes/README.md` on
`ar-196-runtime-spike`. Jira: AR-206, AR-42, AR-173 and the owner's comment of 2026-11-05.

## In one page

- **Seven tables**, all starting with `chat`, owned by the project like `settings` and
  `model_tables`, outside the model's generated tables and fingerprint:
  `chat` (a conversation), `chat_turn` (one question and everything it caused),
  `chat_message` (the timeline: user text, agent text, tool activity rows),
  `chat_tool_call` (details of a tool activity row), `chat_anchor` (message ↔ element),
  `chat_change` (turn ↔ the model changes it made) and `chat_change_element` (which elements).
- **Ids** are UUID v7 in `BINARY(16)` like every model element; **times** are RFC 3339 UTC text
  from `archforge_project::timestamp`, like `settings.created_at`; order comes from integer
  positions, never from times.
- **Anchors** point at element UUIDs and keep a name and metaclass snapshot. A deleted element's
  anchor stays, shown as deleted; undo brings the element back under the same UUID and the anchor
  is live again.
- **Change links** are per turn: the conversation's origin label (AR-198) marks the turn's undo
  steps in this session; the project keeps a durable record of what the turn changed and its state
  (applied, undone, discarded by Don't save).
- **Format 2 → 3.** Archforge 0.13 creates format-3 projects and raises an older project to 3 when
  it first holds a conversation. Archforge 0.12 already refuses format 3 with its existing
  "newer format" error; nothing needs changing there.
- **Don't save keeps chats** (recommended mechanism): chat rows are committed in their own small
  Dolt commits, one per turn, staging only the chat tables. Don't save keeps its proven
  "reset to the last commit", and the chats are already in that commit. This differs from the
  wording "committed with Save" in decision 6, so it is open question 1.
- **A chat message never makes the project unsaved.** Only the agent's model changes do, as any
  edit does.
- **Not stored:** keys, tokens, the control port and its token, runtime transcripts, raw provider
  events, provider error texts, reasoning or thinking blocks, runtime session ids.

---

## 1. Where the chat tables sit

The project database today holds three kinds of table (`crates/project/src/model.rs`):

- `settings`: key–value, the project's own (`format`, `name`, `created_by`, `created_at`,
  `model_schema`);
- `model_tables`: the generated tables rows have gone to;
- the generated model tables (`core_*`, `uml_*`, `er_*`, diagram tables) and their list tables,
  described by the metamodels' generated code and fingerprinted (`EARLIER_MODELS`). Each is made
  when rows first go there; one missing table reads as empty.

The chat tables are a **fourth kind: the project's own, hand-written, not generated, not in the
model fingerprint**. Reasons:

- They are not model elements. They are not undone with the model, not part of a notation, and not
  written by the "replace every model row" writer (`write_model` deletes and re-inserts every model
  table on each write; chat rows are only appended or updated in place).
- Keeping them out of the fingerprint means a chat-schema change never makes the model look like
  "another model", and a model change never touches chat.

What this needs in the project crate (for the build, not now):

- `ModelSchema::new` refuses a model table whose name is `chat` or starts with `chat_`, as it
  already refuses `settings` and `model_tables`.
- `knows()` and `shapes()` treat the seven chat tables as Archforge's own and check each against its
  exact shape (columns, types, nullability, as for model tables). Without this, today's
  `upgrade()` would refuse any project holding a chat table as "a table this Archforge does not
  know".
- `has_unsaved()` leaves the chat tables out (section 7).

Conventions used below:

| What | Convention | Why |
|---|---|---|
| Row ids | UUID v7, `BINARY(16)` | The same as element ids (`archforge_mof::Uuid`): sorts by creation, never collides across Dolt branches. No auto-increment, which collides on merge. |
| References | Named after their target, no `_id` suffix (`chat`, `turn`, `message`, `element`) | As the generated tables do (`package`, `source`, `subject`). |
| Times | `VARCHAR(20)`, RFC 3339 UTC to the second (`2026-11-06T14:02:11Z`) | As `settings.created_at`. The history list shows "Today 14:02". |
| Order | `INT` positions | Times can tie or go backwards with the clock. |
| Choices | Short lower-case codes in `VARCHAR(16)` (`claude`, `assistant`, `stopped`), checked on read against a closed list | Readable in a Dolt diff; no dependence on literal order. Model tables store enumerations as `INT` because they are generated; chat is not. |
| Flags | `TINYINT NOT NULL` | As the generated tables. |
| Foreign keys | None declared | As the model tables. Anchors must be able to dangle; the integrity is kept by Archforge writing in one transaction and checked on reading. |

## 2. The tables

### 2.1 `chat`: one conversation

```sql
CREATE TABLE chat (
  id          BINARY(16)    NOT NULL PRIMARY KEY, -- UUID v7; also names the private runtime folder
  origin      VARCHAR(64)   NOT NULL,             -- the undo-step label of its changes: 'Chat 7'
  title       VARCHAR(200)  NOT NULL,             -- shown in the history; first words of the first question, renamable
  provider    VARCHAR(16)   NOT NULL,             -- 'claude' | 'codex'; fixed for the conversation's life
  model       VARCHAR(128)  NOT NULL,             -- the model asked for on the next turn (alias or full name)
  created_by  VARCHAR(1024) NOT NULL,             -- who started it; the same source as settings.created_by
  created_at  VARCHAR(20)   NOT NULL,
  updated_at  VARCHAR(20)   NOT NULL,             -- last activity: the history lists newest first by it
  INDEX chat_recent (updated_at)
);
```

- **`origin`** is the label Archforge passes to the bridge as `--origin` for this conversation
  (AR-198: 1–64 printable characters). It is fixed at creation and unique within the project, so
  the control channel can tell which conversation an undo step belongs to; renaming the title does
  not change it. Uniqueness is kept by Archforge, not by a `UNIQUE` index, so a future merge of two
  branches never fails on it (a duplicate after a merge is renumbered).
- **`provider`** never changes. Continuing a conversation whose provider is not set up says so and
  offers Settings or a new conversation; it is never sent to the other provider (AR-173).
- **`model`** is what the next turn asks for. Each turn records the model that actually answered
  (`chat_turn.model`), so the history stays true if the model is changed (open question 7).
- **Message count** in the history ("6 messages") is counted, not stored:
  `SELECT chat, COUNT(*) FROM chat_message WHERE role IN ('user','assistant') GROUP BY chat`.
- **"Allow changes to the model"** is not a column here: it is recorded on each turn
  (open question 8).

### 2.2 `chat_turn`: one question and what it caused

```sql
CREATE TABLE chat_turn (
  id             BINARY(16)   NOT NULL PRIMARY KEY,
  chat           BINARY(16)   NOT NULL,
  number         INT          NOT NULL, -- 1, 2, 3 … within the conversation
  model          VARCHAR(128) NOT NULL, -- the model that answered, as the runtime named it (e.g. a resolved full name)
  allow_changes  TINYINT      NOT NULL, -- whether the model could be changed during this turn
  outcome        VARCHAR(16)  NOT NULL, -- 'running' | 'completed' | 'stopped' | 'failed' | 'interrupted'
  error          VARCHAR(32)  NULL,     -- when failed: 'missing_runtime' | 'invalid_credential' | 'sign_in_expired'
                                        --   | 'rate_limited' | 'network' | 'model_unavailable' | 'other'
  started_at     VARCHAR(20)  NOT NULL,
  ended_at       VARCHAR(20)  NULL,     -- null while running
  UNIQUE INDEX chat_turn_order (chat, number)
);
```

- The turn is the unit of undo ("Undo this turn's changes") and of the Stopped and error states in
  the design, so it gets its own row rather than living on a message.
- `interrupted` is written when Archforge finds a turn still `running` on opening the project
  (Archforge quit or crashed mid-turn).
- `error` is the neutral category from the runtime note's error mapping, never the provider's text.
  The panel words it from the catalogues. A rate limit's reset time is shown live and not stored.

### 2.3 `chat_message`: the timeline

```sql
CREATE TABLE chat_message (
  id           BINARY(16)  NOT NULL PRIMARY KEY,
  chat         BINARY(16)  NOT NULL,
  turn         BINARY(16)  NOT NULL,
  position     INT         NOT NULL, -- order in the conversation, from 1
  role         VARCHAR(16) NOT NULL, -- 'user' | 'assistant' | 'tool'
  body         MEDIUMTEXT  NULL,     -- the text as written (the agent's Markdown); null for a tool row
  complete     TINYINT     NOT NULL, -- 0 when Stop, an error or a crash cut the text off ("the reply so far is kept")
  shortened    TINYINT     NOT NULL, -- 1 when the body was cut to the size limit when stored
  anchors_cut  TINYINT     NOT NULL, -- 1 when the message named more elements than are anchored
  created_at   VARCHAR(20) NOT NULL,
  UNIQUE INDEX chat_message_order (chat, position),
  INDEX chat_message_turn (turn)
);
```

- The design interleaves agent text and tool activity rows ("Read 5 classes", then text), so both
  share one ordered timeline. A tool row's details are in `chat_tool_call`.
- AR-42's "role, text, time" are `role`, `body` and `created_at` (`body` as `core_comment` names
  its text).
- `chat` is repeated from the turn so the history list and the count need no join.

### 2.4 `chat_tool_call`: a tool activity row

```sql
CREATE TABLE chat_tool_call (
  message        BINARY(16)    NOT NULL PRIMARY KEY, -- the chat_message row with role 'tool'
  tool           VARCHAR(64)   NOT NULL, -- the bridge tool without the runtime's prefix: 'uml_class_get', 'batch'
  command        VARCHAR(128)  NULL,     -- the command id when the tool runs one: 'uml.class.set'
  detail         VARCHAR(64)   NULL,     -- the property or aspect it was about, from the arguments: 'description'
  status         VARCHAR(16)   NOT NULL, -- 'running' | 'done' | 'refused' | 'failed' | 'denied'
  changed_model  TINYINT       NOT NULL, -- 1 when the call changed the model
  element_count  INT           NULL,     -- how many elements the result named: "Read 5 classes"
  arguments      MEDIUMTEXT    NULL,     -- the call's arguments as JSON, at most 8 KiB
  result         MEDIUMTEXT    NULL,     -- the start of the result as JSON text, at most 2 KiB
  result_bytes   INT           NULL,     -- the result's full size
  error          VARCHAR(1024) NULL,     -- Archforge's own refusal or failure text, as the bridge gave it
  shortened      TINYINT       NOT NULL  -- 1 when arguments or result were cut
);
```

- The row's words ("Read 5 classes · descriptions", "Changed Customer · description") are built
  when it is drawn, from `tool`, `command`, `detail`, `element_count` and the message's anchors,
  through the catalogues. So stored rows show in the language the user has today, and no
  user-facing string is stored.
- `refused` is Archforge saying no (read-only session, a disabled command, a bad parameter);
  `denied` is the runtime's own permission refusal (`permission_denials`, a declined approval).

### 2.5 `chat_anchor`: message ↔ element

```sql
CREATE TABLE chat_anchor (
  message    BINARY(16)   NOT NULL,
  element    BINARY(16)   NOT NULL, -- the element's UUID, as in the model tables' id column
  role       VARCHAR(16)  NOT NULL, -- 'project' | 'diagram' | 'selection' : the context chips sent with a user message
                                    -- 'read'   : named in a tool's result
                                    -- 'target' : the element a changing tool call created or changed
  metaclass  VARCHAR(64)  NOT NULL, -- the element's generated table when anchored: 'uml_class', 'core_model'
  name       VARCHAR(256) NULL,     -- its name when anchored, for when it is gone
  PRIMARY KEY (message, element, role),
  INDEX chat_anchor_element (element)
);
```

- **Context chips.** The design's three removable chips are stored as anchors on the user message
  they went with: `project` (the model's root, its `core_model` row), `diagram` (the open diagram)
  and `selection` (each selected element). A chip the user removed is not stored, so the record
  says what was actually sent.
- **`read`** anchors come from the ids in a tool's result that are live elements. They give the
  "Read 5 classes" row its subjects and let an element show the conversations that looked at it
  (open question 4).
- **`target`** gives "Changed Customer" its subject.
- `chat_anchor_element` is the reverse index AR-42 asks for: "which conversations are about this
  element" for the model tree and the property panel. The panel feature itself comes later.

### 2.6 `chat_change` and `chat_change_element`: turn ↔ model changes

```sql
CREATE TABLE chat_change (
  turn        BINARY(16)  NOT NULL PRIMARY KEY, -- one row per turn that changed the model
  changes     INT         NOT NULL, -- commands that changed the model: "5 changes to the model"
  elements    INT         NOT NULL, -- elements added, changed or removed in all (the rows below may be fewer)
  state       VARCHAR(16) NOT NULL, -- 'applied' | 'undone' | 'discarded'
  updated_at  VARCHAR(20) NOT NULL
);

CREATE TABLE chat_change_element (
  turn       BINARY(16)   NOT NULL,
  element    BINARY(16)   NOT NULL,
  kind       VARCHAR(16)  NOT NULL, -- 'added' | 'changed' | 'removed' (net, over the whole turn)
  metaclass  VARCHAR(64)  NOT NULL,
  name       VARCHAR(256) NULL,     -- after the change; before it for 'removed'
  PRIMARY KEY (turn, element),
  INDEX chat_change_element_element (element)
);
```

AR-42 speaks of "the change sets it caused, referenced through the elements registry". Neither
change sets nor the registry exist yet (AR-31 is in the backlog), so this first cut records the
turn's changes itself. When AR-31 lands, a change-set reference becomes one more column on
`chat_change`, added under the usual rule (a new column at the end, empty for old rows).

Section 4 says how these rows are filled and kept up to date.

## 3. What is not stored

| Never in the project | Where it is instead |
|---|---|
| API keys, Codex sign-in tokens | The system credential store (`archforge-secrets`, AR-197); Codex's own store in its private `CODEX_HOME` |
| The control port, its token, the Dolt password | Memory and Archforge's private settings folder |
| Runtime transcripts (Claude's `--resume` files, Codex thread history) | The private per-conversation folder outside the project, deleted with the conversation |
| Runtime session and thread ids | The private folder. Claude's `--session-id` can simply be the conversation's UUID; a Codex thread id goes in a small file there. They mean nothing on another computer. |
| Raw provider events (`stream_event`, `system/init`, JSON-RPC notifications), usage and cost figures, rate-limit details | Nowhere. The parts the panel needs are taken out as they arrive. |
| Provider error texts | The log only. The project keeps the neutral category. |
| Reasoning or thinking blocks | Nowhere (open question 12) |
| Partial streaming deltas | Memory. Text is stored when a message completes or the turn ends. |
| The instructions Archforge gives the runtime | Nowhere. They come from Archforge's own code and are rebuilt on each start. |

Tool calls are kept **in part**:

- the arguments, up to 8 KiB of JSON (they are model edits, so they are worth seeing later);
- the first 2 KiB of the result and its full size (enough to show what came back; the full reads
  are large and can be read again from the model);
- Archforge's own error text, up to 1 KiB.

Two secret checks belong with storing (open question 9):

1. A user message that contains one of the API keys Archforge holds is refused before it is sent
   or stored. Archforge can compare against its own keys exactly.
2. Environment and command-line values never reach a tool row: the bridge's answers do not carry
   them, and the adapters never copy runtime events into rows wholesale.

## 4. Anchors and change links

### 4.1 Anchors by UUID, and deleted elements

An anchor is the element's UUID plus a snapshot of its metaclass and name taken when the row was
written. Nothing resolves anchors at write time beyond taking the snapshot. At draw time, the UUID
is looked up in the live model.

**When an anchored element is deleted, recommended: keep the anchor as a dangling reference.** It
is shown with its snapshot name and a "deleted" mark, not as a link.

- The conversation is a record of what was said. Dropping anchors would rewrite history: "Read 5
  classes" would turn into "Read 3 classes".
- Undo of a delete brings the element back under the **same UUID** (`Op::Destroy` inverts to
  `Op::Create` with the same `uuid`), so the anchor is live again with no bookkeeping. With
  dropping, an undo would leave the anchor lost.
- A rename after the fact is shown by the live name. The snapshot only matters once the element is
  gone.
- Deleting an element therefore never writes to the chat tables, so the model's own commands and
  undo stay free of chat side effects.

The alternative, dropping anchors on delete, needs a hook in every delete and in undo, and loses
the record. Not recommended.

### 4.2 Linking a turn's edits through the origin (AR-198)

What exists today (`crates/engine/src/session.rs`, `docs/manual/07-agents.md`):

- The bridge is started with `--origin <label>`. Every request it sends carries that origin, and
  `execute_from` and `batch_from` make undo steps that carry it, labelled `<origin>: <label>`.
  A step never merges into another origin's step.
- `undo_origin()` and `redo_origin()` name the origin of the step undo or redo would take.
- Undo steps and the journal live in memory only. Nothing about them survives closing the project.
  Revisions (`Session::revision`) are session-local numbers.

Proposed linking:

1. **One origin per conversation:** `chat.origin`, such as `Chat 7`. The supervisor starts that
   conversation's bridge with `--origin "Chat 7"`. One turn runs at a time per conversation, so
   while a turn runs the window knows which turn every step from that origin belongs to.
2. **In memory, per turn of this session:** the revision before the turn's first change and after
   its last. "Undo this turn's changes" is offered while the turn's steps are on top of the undo
   history (`undo_origin()` is this conversation's origin and the revision is the turn's
   "after"). When the user has changed the model since, the button explains why it is off instead
   of undoing around the user's edits.
3. **One step per turn** (the design: "Made in this turn, as one step of the project's undo
   history") needs a small engine addition. Today each command is its own step unless the agent
   calls `batch`. The addition: while a turn runs, steps of that origin compose into the turn's
   one step, and any other origin's step in between ends the group. This belongs to the subtask
   that does "edits undo as a group" (AR-173 decision 8), not to the schema. Until it exists,
   "Undo this turn's changes" undoes the turn's consecutive steps one after another, and ⌘Z takes
   them one at a time.
4. **Durable, in the project:** at the end of the turn, Archforge writes `chat_change` with
   `changes` (the commands that changed the model) and `chat_change_element` (the net
   added/changed/removed per element). The elements come from the turn's composed delta: the MOF
   ops carry or resolve to UUIDs (`Create`/`Destroy` hold `uuid`; `Set`, `Insert` and the rest hold
   an object whose UUID the store knows). An element added and removed within the same turn is
   left out. The engine's `Document` trait cannot list touched elements today; the notation
   documents can, so this is a small host-side addition.

### 4.3 What survives undo, redo, Save, Don't save and reopening

`chat_change.state` records what Archforge last knew. It is a record, not a promise that the model
still looks that way, because the user may edit the same elements later.

| Event | `state` |
|---|---|
| The turn ends with changes | `applied` |
| The user undoes the turn (button or ⌘Z back past its first step) | `undone` |
| Redo brings it back | `applied` |
| Save | unchanged. Archforge notes in memory each turn's state at this save. |
| Don't save | Each turn of this session goes back to the state it had at the last save; a turn that ran after it becomes `discarded` ("Not kept: the changes were not saved") |
| The history is cleared by its caps, or the project is closed and reopened | unchanged; the card shows the record without the Undo button ("The undo history ends when the project closes") |
| The agent calls the `undo` tool itself | Recorded as a tool row; the turn's net delta already accounts for it. Whether a chat may undo steps that are not its own is for the bridge subtask. |

The change rows (`chat_change_element`) are never removed by undo or Don't save: they say what the
turn did. Only `state` moves.

## 5. Project format 2 → 3

Today: `FORMAT = 2`. `create` writes it; `read_settings` refuses `format > FORMAT` with
`Error::NewerFormat` ("the project is in format 3; this Archforge reads up to format 2"). Formats 1
and 2 open as they are; the format is never rewritten on opening. Model-table changes are handled
separately by the fingerprint (`EARLIER_MODELS`) and leave the format alone. The fingerprint has
not changed since 0.12.0.

Proposed:

- **`FORMAT = 3`**, meaning "may hold conversations, chat tables of shape 1". Archforge 0.13 reads
  formats 1–3 and writes 3 in new projects.
- **The chat tables are made with the first conversation**, all seven together, then the first
  rows: the same "a table comes with its first rows" rule as the model tables. A project without
  conversations holds none of them. A chat table that is missing reads as empty.
- **An older project is raised to 3 when it first holds a conversation, not when it is opened.**
  Opening and saving a format-2 project in 0.13 without chatting leaves it at format 2, so 0.12
  still opens it. With the unchanged fingerprint, that is the only thing that would otherwise
  lock 0.12 out.
- **How the format row reaches the commit.** Dolt stages whole tables, and `settings` also holds
  the model fingerprint, which a model upgrade on opening may have changed without saving. So the
  first conversation's chat commit (section 6) stages `settings` with `format = 3` **only when
  `settings` has no other unsaved change**. That is the normal case today. Otherwise `format = 3`
  waits in the working set for the next Save. Committing a new fingerprint without its new columns
  would leave a project the next opening refuses as damaged, so `settings` is never staged
  together with someone else's change. Whenever Archforge finds chat tables in a project that says
  format 1 or 2, it writes 3 again by the same rule, for example after a Don't save in that window.
- **Refusal for older apps.** Nothing to change: 0.12 and earlier refuse format 3 with the
  existing error. In the short window where a project holds committed chat tables but still says
  format 2, 0.12 opening it loses nothing. Checked against today's code: it reads only the model
  tables, its Save (`DOLT_COMMIT -Am`) commits the chat tables as they are, and its Don't save
  (`DOLT_RESET --hard`, `DOLT_CLEAN`) neither resets nor removes committed tables.
- **Later chat-schema changes** bump the format again (4, …) and follow the model tables' upgrade
  rule: only columns added at the end, empty or with a default for old rows. Anything else is
  refused, never changed.

## 6. "Don't save" keeps chats: how, with Dolt

Today Save is `CALL DOLT_COMMIT('-Am', 'Saved <time>', '--author', …)`, which commits every table.
Don't save (`Project::discard`) is `CALL DOLT_RESET('--hard')` then `CALL DOLT_CLEAN()`, which
resets every table to the last commit and removes the tables made since. Left as it is, it would
throw away every chat message written since the last Save.

### Recommended: chat commits of their own

- Chat rows are written to the working set as they happen (section 7). **At the end of each turn**,
  and when a conversation is created, renamed or deleted, the writer runs
  `CALL DOLT_ADD('chat', 'chat_turn', 'chat_message', 'chat_tool_call', 'chat_anchor', 'chat_change', 'chat_change_element')`
  then `CALL DOLT_COMMIT('-m', 'Chat 7: turn 4', '--author', 'Archforge chat <chat@localhost>')`.
  Without `-a`/`-A` Dolt commits only what is staged, so the user's unsaved model changes stay
  unsaved in the working set, exactly as before.
- **Don't save keeps its code.** The writer first writes and commits whatever chat rows wait, then
  runs the same reset and clean. The chats are in the commit it resets to, so they stay. Then it
  updates `chat_change.state` (section 4.3) and commits that as a chat commit.
- **Save is unchanged.** `-Am` also takes along chat rows of a turn still running, which does no
  harm.
- The commit message names the origin and turn number, never the user's words. The separate
  author lets a future history view tell chat commits from Saves by author rather than by
  message text.

Risks:

- **The Dolt history gets one commit per turn.** It is small and harmless to Dolt. A future history
  view filters by author.
- **If a chat commit fails** (disk full, say) right before Don't save, the reset would lose the
  waiting chat rows. So Don't save is refused with the reason, as a failing reset is refused today.
  It never quietly goes ahead.
- **A turn still running** when the user picks Don't save: closing a project stops its turns
  first (Stop), records them as `stopped`, commits, then resets.
- This **reads decision 6 differently**: chats are committed per turn, not "with Save". The effect
  the owner asked for ("Don't save resets only model tables") holds. Open question 1.

### Alternative: chats committed with Save, Don't save restores only the model tables

The literal reading of decision 6:

- Chat rows wait in the working set until Save's `-Am`.
- Don't save becomes selective: `CALL DOLT_CHECKOUT(<every changed table except the chat
  tables>)`, `CALL DOLT_CLEAN(<every new table except the chat tables>)`, and the `model_schema`
  row of `settings` set back to its committed value (`… AS OF 'HEAD'`) while the `format` row is
  kept.

Risks, which are why it is not recommended:

- The list of tables to put back must be exactly right. A mistake silently keeps a model change or
  wipes a chat, which is the kind of failure the crash-safety rules forbid.
- `settings` must be put back row by row, because it holds model and chat facts side by side.
- Conversations of a project that is never saved live only in the working set. They survive a crash
  (Dolt keeps the working set on disk) but are not in any commit, so a later Dolt push or clone
  would not carry them.
- `has_unsaved` must ignore the chat tables in both designs. Here it matters more, because chat
  rows can sit uncommitted for a long time.

## 7. Writes off the UI thread, and the save model

The application already has one writer thread per Archforge (`apps/archforge/src/projects.rs`,
`enum Job { Write, Save, Flush, Drop }`). The window hands it the model's rows; it writes them,
commits on Save, and lets go on Don't save.

Proposed:

- **Chat writes become jobs on the same writer** (a `Job::Chat(project, writes)`, where `writes`
  holds rows to insert or update and whether a chat commit follows). Sharing the writer keeps the
  order: chat rows, model rows, Save, Don't save and Close happen in the order they were asked,
  with no second connection racing the first.
- **When rows are written:**

  | Moment | Written |
  |---|---|
  | The user sends | `chat_turn` (`running`), the user `chat_message`, its chip anchors (and the `chat` row and tables for a new conversation). Written before the runtime gets the text, so a crash cannot lose a question that was sent. |
  | A tool call ends | its `chat_message` + `chat_tool_call` + anchors |
  | An agent message completes | its `chat_message` |
  | Undo or redo moves a turn's state | the `chat_change.state` update |
  | The turn ends (completed, stopped, failed) | the turn's outcome, any cut-off text (`complete = 0`), `chat_change` + `chat_change_element`; **then the chat commit** |
  | Conversation created, renamed or deleted | the rows; then a chat commit |
  | Before Close, Close without writing, or Don't save | whatever waits; then a chat commit |

  Streaming text stays in memory until its message completes. The writer coalesces what waits, as
  it does for model rows today.
- **Each write is one SQL transaction**, so a turn's end lands whole or not at all. A failed chat
  write is shown in the chat panel ("This conversation could not be saved: …") and retried with the
  next write. It never blocks the model writer and never changes the model.
- **Is a chat message an unsaved change? Recommended: no.**
  - The title-bar state, the close prompt and "All changes saved" are about the model only.
  - `has_unsaved()` filters the chat tables out of `dolt_status`. Otherwise a reopened project
    with an uncommitted chat row would open as "unsaved" (`open_job` calls `mark_unsaved` from it).
  - The agent's **model changes** make the project unsaved like any edit, and the user saves or
    discards them as usual. That matches "Chat is read-only by default".
- **On opening**, a turn left `running` becomes `interrupted`, and any uncommitted chat rows are
  committed in a chat commit.

## 8. Sizes, retention and deletion

Writing (Archforge's own limits; refusing rather than clamping where the user asked for something):

| Item | Limit | Over the limit |
|---|---|---|
| A user message | 64 KiB of text | The Send button is refused with the size; nothing is sent or stored |
| An agent message | 256 KiB stored | Stored cut at a character boundary, `shortened = 1`; the panel says it was shortened |
| Tool arguments / result excerpt / error | 8 KiB / 2 KiB / 1 KiB | Cut, `shortened = 1`; `result_bytes` keeps the true size |
| Title | 200 characters | Refused when renaming; the automatic title is cut |
| Name snapshot | 256 characters | Cut |
| Anchors per message | 500 | The rest are not stored, `anchors_cut = 1`; `element_count` stays exact |
| `chat_change_element` per turn | 2,000 | The rest are not stored; `chat_change.elements` stays exact |
| Messages per conversation | 5,000 | "This conversation is full; start a new one" |
| Conversations per project | 10,000 | A new one is refused with the reason |

Reading (a project from elsewhere is never trusted, as for the model):

- Every chat table is shape-checked. Rows are read with caps on rows and bytes: the history list
  reads only `chat` and the counts, and a conversation's messages are read when it is opened, with
  a 64 MiB budget.
- A wrong code, an orphan row (a message whose conversation is missing) or a broken table is
  **never silently skipped**. A bad row makes its conversation "cannot be read: <reason>" in the
  history; a chat table of the wrong shape makes the panel say "The conversations in this project
  cannot be read: <reason>" and turns chat writing off for that project. The model still opens:
  damaged chats never block the model.

Retention: **none automatic**. Conversations are part of the project's record, and the user deletes
them.

Deleting a conversation:

1. Stop its turn if one runs.
2. Delete its rows from all seven tables in one transaction, then a chat commit
   ("Chat 7: deleted").
3. Delete its private runtime folder.

The undo history is not involved: deleting a conversation changes no model element. The
conversation **stays in the project's earlier Dolt commits**, like any deleted row (open
question 6). Private folders whose project is gone (the project was deleted outside Archforge)
cannot be found from a project. Each private folder can record its project's last path, so a later
Settings page can offer to clear the orphans. That is not part of this subtask.

## 9. Things the build depends on

- **Engine grouping of a turn's steps** (section 4.2, step 3): the "edits undo as a group" subtask.
- **Touched elements from a delta** (section 4.2, step 4): a host-side helper over the notation
  documents.
- **The supervisor** passes `--origin <chat.origin>` and keeps one bridge per conversation
  (AR-200).
- **The project crate**: reserved `chat` names, own-table shape checks, `has_unsaved` without chat,
  the format rule, and chat writes and commits.
- **Tests** (AR-206's Done list): against a real Dolt, a saved conversation reopened; Don't save
  keeping chats while the model goes back; a format-2 project gaining a conversation (with and
  without a pending model upgrade); 0.12's refusal of format 3 (the existing `NewerFormat` test,
  with 3); `has_unsaved` false with only chat rows uncommitted; a crash mid-turn becoming
  `interrupted`; hostile chat tables (wrong shape, orphan rows, oversized cells) refused without
  blocking the model.

## 10. Open questions for the owner (recommendation first)

1. **How chats are committed.** *Recommended:* in their own Dolt commits, one per turn, staging
   only the chat tables, so Don't save keeps its proven whole reset (section 6). *Alternative:*
   committed with Save as decision 6 words it, with a selective Don't save that puts back every
   table except the chat tables.
2. **When a project becomes format 3.** *Recommended:* new projects are created in format 3, and
   an older project is raised to 3 only when it first holds a conversation, so a 0.13 user who
   never chats keeps projects 0.12 can open. *Alternative:* every project opened and saved by 0.13
   becomes format 3.
3. **An anchored element is deleted.** *Recommended:* keep the anchor, shown with its name
   snapshot and a "deleted" mark; undo revives it under the same UUID. *Alternative:* drop the
   anchor.
4. **Which elements a message is "about".** *Recommended:* the context chips that were sent, the
   elements a tool read, and the elements a turn changed, each capped. *Alternative:* chips and
   changes only, a smaller record and a thinner "conversations about this element".
5. **How much of a tool call is kept.** *Recommended:* arguments up to 8 KiB, the first 2 KiB of
   the result with its full size, and Archforge's own error text. *Alternatives:* nothing but the
   summary fields, or everything (large, and mostly re-readable from the model).
6. **A deleted conversation stays in the project's Dolt history.** *Recommended:* accept it, and
   say so plainly in the delete confirmation and the manual ("It stays in the project's saved
   history"). *Alternative:* rewriting history to remove it, which is risky and out of scope for
   0.13.
7. **Changing the model within a conversation.** *Recommended:* allowed within the same provider;
   each turn records the model that answered. *Alternative:* the model is fixed for the
   conversation, like the provider.
8. **Does "Allow changes to the model" survive reopening the project?** *Recommended:* no. Every
   conversation opens read-only again, and each turn records whether changes were allowed, so the
   history stays exact. *Alternative:* store the switch on the conversation and restore it.
9. **A key typed into a message.** *Recommended:* refuse to send a message that contains one of the
   API keys Archforge holds, saying why. *Alternative:* no check; whatever the user types is sent
   and stored.
10. **Runtime session ids.** *Recommended:* kept in the private folder, not the project, with
    Claude's session id equal to the conversation's UUID. On another computer, or when the
    runtime's transcript is missing or behind, Archforge starts a fresh runtime session primed
    from its own copy. *Alternative:* store them in `chat`, where they mean nothing on another
    computer.
11. **Who started a conversation.** *Recommended:* record `created_by`, from the same source as
    the project's own `created_by`. That matters once projects are shared. *Alternative:* not
    recorded.
12. **Reasoning text and cost.** *Recommended:* neither stored in 0.13. Reasoning is the
    provider's working, and cost belongs to the account, not the project. *Alternative:* store a
    turn's token use and cost for a later "what did this cost" view.

## 11. As built (AR-206)

The storage layer and its API, in `crates/project/src/chat.rs`; no interface yet.

- **Save** stages the changed tables by name, every one but the chat tables, after unstaging
  whatever an interrupted step left staged, and commits without `-a`/`-A`. So a turn still running
  is never committed half-way by a Save; its rows wait for the turn's own chat commit.
- **A chat commit** stages exactly the chat tables that changed (and `settings` when it holds only
  the raised format), the same way, by the author `Archforge chat <chat@localhost>`.
- **Don't save** commits what of the chats waits first and checks that no chat table is left
  uncommitted, refusing otherwise; only then the unchanged reset and clean. It runs on the writer
  thread, so no chat write can come between. Afterwards each change link of this session goes back
  to its state at the last save (or `discarded`) and the format is raised again if the reset put it
  back, in one more chat commit.
- **Change links**: `TurnSpan` holds a turn's revisions before and after (the steps carry the
  conversation's origin); after an undo or redo it says whether the turn's changes became undone or
  applied, and `set_change_states` records it. The project remembers each state at the last save for
  Don't save.
- **Kept keys**: the question, and the title made from it or given on renaming, are checked against
  the API keys Archforge keeps (`archforge-secrets`) before anything is written, so a pasted key
  never reaches a row or a commit.
- **Reading** refuses, with the reason, duplicate rows a table without its keys could hold (two
  change links for one turn, an element named twice), as it refuses orphans and broken order.
- **Left for later**: grouping a turn's steps into one undo step (AR-203); the host-side list of the
  elements a turn touched, from its composed delta; the chat panel, its wording and the private
  runtime folders.
