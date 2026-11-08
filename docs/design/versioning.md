# AR-144: model versioning in 0.14.0 — workflow and decisions

**Decided by the owner on 2026-11-08:** Restore is in, forward-only; branches are in; merging and
remote collaboration are out; the project format stays at 3; starting new work from a version means
creating a branch there, and viewing a version stays read-only; the menus are File, Edit, View, Model,
Versions and Help, plus the macOS application menu and an in-window menu on Windows and Linux; a
version records the name from Settings when one is set; every other recommendation as written.

**Nothing in this document is built yet.** It is the workflow the stories under AR-143 build. Line
references are to `main` at `48e9679`. Dolt behaviour marked *measured* was run on a throwaway database
on a real `dolt sql-server`, version 2.7.2 (Archforge accepts 2.6.0 and later,
`crates/dolt/src/binary.rs:10`; 2.6 itself was not tried). Jira: AR-143 to AR-154 and AR-20. Design:
`ArchforgeUx.figcraft`, page 01, frame 813, as a reference for the review layout; section 11 lists
the journeys still to draw.

Short file names: `lib.rs` is `crates/ui-egui/src/lib.rs` unless a path says otherwise, `model.rs` and
`chat.rs` are in `crates/project/src`, `session.rs` in `crates/engine/src`, `control.rs` in
`crates/ui-egui/src`, `projects.rs` in `apps/archforge/src`.

## In one page

- **One history.** A version is a commit Save makes. Nothing new is stored; the project format stays 3.
- **Save and Save version… are one operation.** ⌘S makes a version at once with an automatic
  description, as today. Save version… reviews the changes and takes a description you type.
- **Autosave never makes a version; undo never moves between versions.** Both keep today's meaning.
- **Viewing an earlier version is a read-only preview.** Nothing is written, and Dolt's checkout is not
  used at all: it refuses a commit (measured). Back to current work returns to the same selection,
  tabs and undo history.
- **Restore is forward-only.** It makes a new version equal to an old one and first keeps any unsaved
  work as a version. It rewinds nothing.
- **Branches are separate lines of work.** Each keeps its own versions, its own unsaved work and its own
  conversations. Switching never loses unsaved work; it stays on the branch it was made on. Undo starts
  fresh on the branch you arrive at.
- **There is no merging.** The interface says so plainly. Compare shows how two branches differ.
- **A conversation belongs to the branch it was started on** and shows only there. A new branch starts
  with none.
- **The control is not a slider**: a version chip in the project bar with older and newer steps and a
  keyboard list, plus the existing left-rail entry, renamed **Versions**.

| Word in the interface | What it does | How it reaches Dolt |
|---|---|---|
| *(nothing: autosave)* | Each edit reaches the project a moment later | Working set of the current branch |
| **Save** (⌘S) | A version now, automatic description | `DOLT_COMMIT` |
| **Save version…** (⇧⌘S) | The same, after a review and a description | The same `DOLT_COMMIT` |
| **Undo / Redo** | Steps through this session's edits on this branch | Nothing directly |
| **History** | The versions of the current branch | Reads |
| **View this version** | The whole workspace shows that version, read-only | Reads |
| **Back to current work** | Leaves viewing | Nothing |
| **Compare…** | Before and After between two states | Reads |
| **Restore this version…** | A new version with that content | Writes and commits, forward |
| **New branch…** | A separate line of work, from current work or a version | `DOLT_BRANCH` and writes |
| **Switch branch** | Shows another line of work | Reads; moves the connection |

---

## 1. What exists today

### Autosave

- Each frame `Shell::write_changes` snapshots the session into rows and hands them to the host when
  `session.revision()` moved and no drag runs (`crates/ui-egui/src/lib.rs:1028-1056`, called at `:1422`).
- One writer thread keeps one set of rows per project and writes them after 300 ms of quiet
  (`apps/archforge/src/projects.rs:35`, `:556-635`). A failed write keeps the rows and is tried again
  with the next change or Save (`:5-6`, `:664-672`).
- `Project::write_model` replaces every row of every model table in one transaction, with no commit
  (`crates/project/src/model.rs:247-312`). The same rows written again leave `dolt_status` empty
  (`crates/project/tests/model.rs`; measured).
- So the Dolt working set is the current work.

### Save

- Entry points: ⌘S (`lib.rs:1411`), File ▸ Save on macOS (`apps/archforge/src/macos_menu.rs:40`), the
  close question (`crates/ui-egui/src/close_ask.rs:97`) and the agents' `save_project` tool through
  `app.save` (`crates/mcp/src/lib.rs:135-138`, `crates/ui-egui/src/control.rs:249`).
- The writer writes what waits and commits on one thread (`projects.rs:584-597`). `Project::commit`
  stages
  the changed tables by name, leaving out the chat tables (`model.rs:408-422`). With nothing changed it
  makes no commit and returns `false`.
- The message is always `Saved <RFC 3339 UTC time>` (`projects.rs:679`); the author is always
  `Archforge <archforge@localhost>` (`crates/project/src/lib.rs:52`). The hash is not kept.

### What a commit holds

One line of commits, four kinds:

| Kind | Committer | Message | Tables |
|---|---|---|---|
| Dolt's root | The server's own identity | `Initialize data repository` (measured) | none |
| Creation | `Archforge` | `Create the project` | `settings` (`crates/project/src/lib.rs:369-376`) |
| **Save** | `Archforge` | `Saved <time>` | Every changed table but the chat tables: model and list tables (diagrams and skills included), `model_tables`, `settings` |
| Chat | `Archforge chat`, `chat@localhost` | `Chat N: turn n`, `Conversations kept…` | The seven chat tables, and `settings` when the format raise is its only change (`crates/project/src/chat.rs:915-932`) |

`prepare_model` runs after the creation commit (`projects.rs:298`), so no commit holds a model before the
first Save.

### Undo

In-memory deltas per open project (`crates/engine/src/session.rs:1-9`, `:162-163`, `:714-745`). Save
does not
clear them. Undo has no link to commits.

### History, diff, branches

Nothing in the product uses them: `dolt_log` and `dolt_diff` appear only in tests, `AS OF` only for
bringing
back a deleted conversation (`chat.rs:1535-1570`). What Dolt gives us, measured:

| Mechanism | Result |
|---|---|
| ``USE `project/<hash>` `` | That commit, read-only: the server refuses writes. `information_schema` filtered on `table_schema = database()` shows that commit's table shapes |
| ``USE `project/<branch>` `` | That branch's working set, writable, separate from every other branch's |
| `CALL DOLT_CHECKOUT('<hash>')` | Refused: "dolt does not support a detached head state" |
| Unsaved work on a branch | Kept per branch, and survives a server restart |
| `DOLT_RESET('--hard')` | Affects only the branch of that connection |
| `dolt_log` on a branch | That branch's own line. 1,500 commits filtered and paged: about 48 ms including client start-up |
| `dolt_branches` | Names, heads and a `dirty` column, which counts any uncommitted table, a chat table included |
| `dolt_diff` on a list table | Inserting at the front reports every later position as modified |
| `dolt_diff` after a column was added | Every row reports modified, changed or not |
| `DOLT_BRANCH('-m')` and `'-D'` on `main` | Refused: it is the default branch |
| `DOLT_BRANCH('-d', name)` | Refused unless merged, so a branch that cannot be merged needs `-D` |
| Trigger on a branch | Not visible from `main`'s `information_schema` |
| A new branch at `HEAD` before the first Save | Holds only what the creation commit holds: no `model_tables`, no fingerprint |

What `read_model` (`model.rs:318-394`) must change to read history:

1. `USE project` and `table_schema = 'project'` are fixed (`:178-182`, `:671`, `:750`).
2. Shape checks are exact (`:784-791`), but a commit from before an upgrade has older shapes, since
   `prepare_model` upgrades only the working set (`:201-230`, `EARLIER_MODELS` at `:44-49`).
3. It requires `model_tables` (`:644-646`), which the creation commit lacks.

### How Chat and agent writes reach the document

- Every agent write goes through the control channel into `shell.session`, whichever session that is
  at the
  time (`control.rs:301-302`, `:324`). Commands are the only way in.
- A Chat turn's requests run only while the turn runs in the project shown (`control.rs:169`,
  `crates/ui-egui/src/chat_panel.rs:1701`). Leaving a tab stops the turn (`lib.rs:847-848`).
- A token that is not a Chat turn passes that gate with no origin (`chat_panel.rs:1704`).
- Every conversation write names the project by path and runs on the writer
  (`apps/archforge/src/chats.rs:491-505`,
  `:539`) against whatever connection `model_connection` gives.

---

## 2. The control

| Need | Slider | Stepper chip and list (chosen) | Timeline strip |
|---|---|---|---|
| Discrete versions | Implies a continuum | Exact by construction | Good |
| Long histories | About 40 usable stops in 200 px | Paged list; step or jump | Needs its own scrolling and row |
| Cost per stop | Each stop is a full model read and a session build, so scrubbing is the worst case | One read per deliberate step | One read per step |
| Keyboard | Arrows only; no jump to a known version | ↑/↓, type to filter, ⌘[ ⌘] | Arrows only |
| Narrow windows (minimum 640, `apps/archforge/src/main.rs:256`) | No room | One icon | A whole row |
| Says what you are looking at | Needs a label anyway | The chip is the label | Needs a label |

**The chip** sits in the project bar, left of search (`crates/ui-egui/src/workspace.rs:158-169`,
`crates/ui-egui/src/home.rs:305-308`).

| State | Chip shows |
|---|---|
| One branch, current work | Clock, `Current work`, a dot when unsaved |
| Two or more branches, current work | Branch icon and name, `Current work`, a dot when unsaved |
| Viewing | Tinted, branch name when there are two or more, `‹ 8 Nov 14:02 ›`, and `Back to current work` |

Clicking the chip opens a popover: Current work, then that branch's versions newest first, paged, then
**New branch…**. With two or more branches a branch row comes first (the current branch, opening the
branch list); a one-branch project never sees the word.

The left rail's entry keeps its existing branch glyph. Its unsaved state is a count badge: the number of
working changes, shown when above zero.

Narrowing order: the branch name truncates, then the label goes, then search hides as today. At 640
px the chip
is one 32 px icon, with the full label in its tooltip and accessible name.

Keys: ⇧⌘H opens the list; ↑/↓ moves, Enter views, Esc closes it; while viewing ⌘[ is older, ⌘]
newer, ⇧⌘H returns;
⇧⌘B opens the branch list. Esc does not return, so a stray Esc cannot drop your place. That ⇧⌘H both
opens the list and, while viewing, returns is intended. AR-154's shortcut audit confirms it and checks
all of these for clashes.

Stepping: from Current work, ‹ goes to the latest version when there are unsaved changes, otherwise
to the one
before it, which is the first that differs. In the list that latest row reads "Same as current work".

---

## 3. Viewing an earlier version

| Option | Verdict |
|---|---|
| Dolt checkout | Refused for a commit |
| Hard reset to the old commit | Destroys unsaved work and moves HEAD behind chat commits. Rejected |
| Copy old rows into the working set | Every step would rewrite current work. Rejected |
| **Read-only preview over a revision connection** | Touches nothing; the server itself refuses writes. **Chosen** |

Viewing is read-only in the engine: a flag at the check that refuses a poisoned session
(`session.rs:602-604`,
`:716`) refuses undoable commands, undo and redo, and lets view commands through. Editing controls
are disabled too,
so nobody meets an error on a click.

| Action | How | Effect |
|---|---|---|
| Enter | Chip, list, **View this version**, or ‹ from Current work | Current state is set aside, the version loads and shows |
| Apply | None: moving between versions is only looking | |
| Cancel | A load can be overtaken or dropped | The last valid state stays |
| Return | **Back to current work** | The set-aside state returns as it was: selection, tabs, pan, zoom, undo history |
| Restore | Separate, explicit, confirmed (section 7) | Never reached by navigating |
| Start a branch | **Start a branch here…** in details and in the banner (section 8) | Viewing ends on the new branch |

In viewing a banner over the centre says "Viewing the version of 8 Nov 14:02 · Read only", with
Compare with current work and Back to current work, and **Restore this version…** and **Start a branch
here…** as icon buttons with tooltips. The status bar says the same.

`shell.session` has about 490 uses, so the preview takes its place and the real session is set
aside; `Parked`
(`lib.rs:288-304`) lists what to keep. Then each path that means "the real project" needs a guard
and a test:

| Path | Why it matters |
|---|---|
| `write_changes` (`lib.rs:1028`) | The one route from a document to the database. Unguarded, a preview overwrites current work |
| `save` (`:1061`), ⌘S | Would make a version of the wrong state, so Save is refused while viewing |
| `tab_unsaved` (`:584`) | Would report the preview as clean, so closing would skip the Save question |
| `keep_view` (`:1005`) | Would keep the preview's tabs as the ones to resume |
| `retain_valid` calls (`:1424-1429`) | Would drop your selection against the old document unless set aside |
| `control::engine` and `save` (`control.rs:249`, `:301`) | Agents would write into the preview |
| `uml.model.root` on adopting (`lib.rs:965-970`) | Adopting edits on open, so a preview does not adopt |

Each load carries a counter and only the newest applies, the pattern of `Saving` (`lib.rs:234-235`).
The last few
previews are cached by hash. A load does not run on the writer or hold `inner.open`, which the
writer holds while it
writes (`projects.rs:647-657`).

---

## 4. Autosave, Save, undo: one history

| Question | Decision |
|---|---|
| Does autosave make versions? | No: working set only |
| Is Save a version? | Yes, unchanged |
| Is check-in a second thing? | No. Save version… is Save with a review and a description; both are one call to `Project::commit` |
| Must Save ask for a description? | No: ⌘S stays one key |
| Does "check in" appear in the interface? | No |
| Does undo interact with versions? | No. Undo after a save makes the project unsaved again |
| Is undo kept across viewing? | Yes: the real session is only set aside |
| Is undo kept across a branch change? | No: section 8 |
| Are chat commits versions? | No: left out of History, still in the log |
| Do existing `Saved <time>` commits count? | Yes, listed with their date and "No description" as the meta line. The exact `Saved <RFC 3339>` shape is matched, so "Saved the day" stays a description |

**What a version is, in code:** a commit on the current branch whose committer email is not the chat
committer's
(`chat@localhost`), leaving out the two oldest commits of the line, which are Dolt's root and the
creation. The email
decides, not the name, because a person's Settings name could be anything. The **previous version**
is the nearest
earlier version in that list, not the Dolt parent, which is often a chat commit. For "changes since
the last version",
`HEAD` is the right baseline: a chat commit's model tables equal those of the last Save. Before the
first Save there is no version and no model at `HEAD`, so the Before side is empty (section 10).

**Save version…** ties its review to `session.revision()`. If that moves, the dialog refreshes and
says so before Save
is enabled again. A failure compares HEAD before and after, so the message says truthfully whether a
version exists; when HEAD cannot be read it says it cannot tell. A
description is required there (an empty one is what ⌘S is for), is one line of at most 200
characters, and is refused
when it holds a kept API key, as a chat question is (`chat.rs:744`), since it stays in history.

**Author.** `Project::commit` gets an author: the Settings name (`state.user_name`) when one is set,
with the email
`archforge@localhost`. A save an agent asked for uses `agent@localhost` and the agent's origin
label, or `Agent` when it
has none, and reads "Saved by an agent". Dolt takes non-ASCII names but refuses parentheses in one
("Author not formatted
correctly", measured), so the name is cleaned first, and a name that cannot be used falls back to
`Archforge`: a name never
stops a Save. An older version, whose author is `Archforge`, shows no author. Anyone given a copy of
the project can read
the names in it.

---

## 5. What a version contains

| In a version | Outside a version |
|---|---|
| Model elements, relationships, members, ownership, every notation | Conversations: their own commits (section 9) |
| Diagrams, layout, routing, style (`diagram_*` tables) | Undo history |
| Skills | Pan, zoom, open tabs, selection, panel widths (`prefs.json`) |
| Project settings (`format`, `name`, the model's fingerprint), kept with the commit; Restore does not bring them back (section 7) | Theme, language, your name, agent setup and keys |

Pan and zoom are not in the project (`crates/diagram/src/generated/schema.rs:10-50`,
`crates/ui-egui/src/state.rs:121-150`), so they cannot look like a model change.

---

## 6. Leaving current work: viewing and switching

One rule set for both. A version is read from its commit, and another branch's work stays in that
branch's working set, so
what is set aside is whatever exists and nothing is transformed.

| Situation on leaving | Behaviour |
|---|---|
| Unsaved edits | Stay in the working set. Viewing keeps them in the set-aside session too |
| Autosave waiting | Viewing: it finishes on its own. Switching: written first; if that fails the switch is refused |
| A save under way | Waits for its answer: `poll_save` marks the session shown as saved (`lib.rs:255-289`) |
| Autosave failing | Viewing allowed, status keeps saying so. Switching refused, as the work could not be kept |
| Description or skill text being typed | Committed to the document first (`lib.rs:849-850`). A refused description blocks leaving and shows the refusal |
| Inline editor, a drawing under way, an open dialog, a drag | Refused: "Finish or cancel your edit first." |
| Chat requests on their way | Settled first (below): a question answered later would start a turn on what replaced the session |
| Chat turn running | Asks: "Chat is still changing the model. Stop it and go on?" **Stop and go on** / **Keep working** |
| Agent with its own token, while viewing | `engine.*` and `app.save` are refused with an error naming the state and how to return. `document.inspect` reports what is shown. Nothing is queued |
| Agent with its own token, while the project is held (below) | Changing requests are refused until the hold has ended |

During viewing, switching project tab returns to current work first, and closing or quitting returns
first and then asks the
usual question from the real session's state. A version is checked just before loading: the shape of
its hash
(`is_commit`, `chat.rs:769`) and that it is an ancestor (the check at `chat.rs:1511-1519`).

### Holding the project still

Entering viewing, Restore and a branch change (a switch or a new branch) replace the session, and each
follows one order. Stepping between versions and returning have nothing to protect, and do not.

1. Only one runs at a time; the others are unavailable until it has ended.
2. The refusals above are checked, and a running Chat turn is asked about.
3. The hold starts.
4. Chat settles: the turn is ended and written, and requests still on their way are answered (questions
   waiting on a key check, queued writes, deletions and restorations), within a time limit. Past it the
   operation is refused and the hold ends.
5. A save under way answers, and the latest rows are handed over.
6. The replacement is read, checked and built into a session. One that cannot be built ends it here,
   with nothing written.
7. The operation's writes run as one job on the writer. Viewing has none.
8. The replacement takes the session's place, and the hold ends.

| While held | Why |
|---|---|
| The session is read-only; undo and Save are refused | An edit made now would be replaced |
| `write_changes` is stopped once step 5 has handed over | Rows handed over later would reach the replaced state |
| Control requests that change anything are refused, naming the state | An agent's change would be lost the same way |
| The Chat composer is off, and a question answered under another session starts no turn | A turn must not start on what replaced the session |
| Showing another project or Home, opening a project, closing and quitting wait for the answer | The session being replaced must still be there, and the job is still writing |

| When it goes wrong | Result |
|---|---|
| Steps 1 to 6 | Nothing is written; the hold ends and the session is as it was |
| A commit or branch operation gives no clear answer | `HEAD` or the branch list is read, and what it shows is what happened: the step counts as finished or failed by that |
| Those cannot be read either | Archforge says it could not tell. Nothing is written back, and the project stays held until it is closed; reopening reads what is stored |
| Archforge stops partway | Reopening reads what is stored. A Restore before its commit shows the restored rows as unsaved, with earlier work in its `Before restoring` version; a switch reopens where it was; a new branch that did not finish is removed on reopening (section 8), and its source is unchanged; a rename may leave both names, with the work still on the old one |

A control request can wait its turn after it is accepted: `control::drain` takes 32 a frame
(`control.rs:126`, `:149-160`). So each request carries the window's session generation, taken when the
channel accepts it, beside its token's liveness (`control.rs:37-59`). The window moves that counter whenever
it replaces the session: viewing a version, returning, a branch change, Restore, showing another project's
tab. A request from an earlier generation is refused, so it cannot land on a session it was not sent to.

### Comparisons

| Comparison | Before | After |
|---|---|---|
| Working changes against the last version | `HEAD` rows | Live session snapshot |
| A version against the previous version | Read at revision | Read at revision |
| Two chosen versions on this branch | Read at revision | Read at revision |
| A version against current work | Read at revision | Live session snapshot |
| Current work or a version against another branch's latest version | Read at revision | Read at revision or live snapshot |

The live side comes from the session, not the database, so a comparison is pinned to one
`session.revision()` and
refreshes when that moves. Direction is Before → After, shown in the header; Swap reverses it
deliberately. One
comparison engine feeds the lists, element cards and diagram cards, so counts agree.

Differences are found by comparing rows by identity, not from Dolt's diff output:

- Object tables have `id BINARY(16) PRIMARY KEY`, so rename and move stay one element.
- List tables are keyed by owner and position, so values are compared per owner, which removes the
  shift noise.
- `dolt_diff_summary` is a cheap "are these identical?" check between two saved versions only, once the
  chat tables and `settings` are left out of its rows. Anything involving current work compares the live
  session's rows, since an edit may not have reached the database yet.
- `session_from_rows_repaired` fits table boxes on open (`crates/notations/src/lib.rs:169-178`). The
  same repair is applied to the
  Before side, or a false "layout changed" shows after opening.
- `diagram_*` tables and style are presentation; the rest is model. A node's `element` column links
  the two, which gives "model
  changed, layout did not".

---

## 7. Restore

Restore makes a new version whose content equals an old one, on the current branch. It is offered in
version details and in the viewing banner, after a confirmation that names the version. It follows
section 6's order, and viewing ends first.

At that order's step 6 the version is built and compared with current work, with the same repair on
both sides. A version whose model is the same as current work is not offered. If that is only found when
it runs, it says "This version is the same as your current work." and makes no version, an empty commit
included.

Step 7 is one writer job:

1. What waits is written, and any unsaved changes are saved as a version first, `Before restoring <date>`.
2. The version's model rows, in today's table shape, are written to the working set.
3. They are committed as `Restored the version of <date>`. `Project::commit` returning `false` means
   nothing differed, and is reported as above.

| Question | Decision |
|---|---|
| What comes back? | The model tables: elements, relationships, diagrams, skills, and the root element's name |
| What stays as it is? | The `settings` table: `format`, `model_schema`, `name`, `created_by`, `created_at`. None is a setting a person changes. Keeping `format` and `model_schema` means an older version cannot lower the format of a branch that holds conversations |
| Why forward-only? | Rewinding would drop later chat commits, and deleted-conversation Undo needs its commit to stay an ancestor of `HEAD` |
| Undo across a Restore? | No: undo starts fresh after a Restore that finished. Chat cards say the undo history no longer holds their change, wording that exists. After a failure undo is as it was |
| An older version needing opening's repairs | The restored project may read as unsaved at once, as opening it does |
| Reading, building or step 1 fails | Nothing has changed |
| Writing the rows fails | The write is one transaction, so the working set is unchanged. A version from step 1 stays, the message says so, and the project reads as saved |
| The commit fails | Section 6 decides from `HEAD`. If the commit landed, Restore has finished. If not, the session stays, its rows are written back as autosave writes them, tried again with the next change if that fails too, and the message says so |
| Restoring from another branch? | Not offered: History lists only the current branch |

---

## 8. Branches

A branch is a separate line of work in the same project. In the interface: "A branch keeps its own
versions, unsaved work and
conversations." Each Dolt branch has its own working set, so unsaved work needs no new storage.

### Storage and format

- `Project` gains a current branch, and `model_connection` (`model.rs:178-182`) uses ``USE
  `project/<branch>` ``. Every model and
  conversation read and write then follows it. Shape checks use `database()` instead of the fixed name.
- The current branch per project is kept in `prefs.json` beside the project's saved view, outside
  the project. A copy of a project
  opens on `main`.
- **Format stays 3.** No table or column is added. An older Archforge opens `USE project`, which is
  `main` (measured), and does not see
  other branches.
- A branch switch runs the checks an opening runs on the target: format, model fingerprint and
  upgrade, and refusal of triggers,
  procedures and events. These checks today see only `main`, and a trigger on another branch is
  invisible from it (measured), so a
  foreign project must be checked per branch. Reopening on a stored branch runs them too. Arriving on a
  branch whose tables are upgraded reads as unsaved, as opening such a project does today.

### Names

Dolt refuses spaces and non-ASCII in branch names (measured), and the interface has seven languages,
so a name is typed as ordinary text
(one line, at most 60 characters) and stored through a reversible encoding into letters, digits,
`-`, `.` and `_`, decoded for display. A
name not made by Archforge shows as it is when it is in that safe set. One outside it (a quote or
backtick, in a folder from elsewhere) is
listed and cannot be opened, with the reason: such a name goes into a ``USE `…` `` statement. Dolt
refuses a name shaped like a commit
hash itself (measured). `main` keeps its name, and cannot be renamed or deleted, since Dolt refuses both.

### Operations

All run on the writer, in order with every model and conversation write.

| Operation | Where | What happens | On failure |
|---|---|---|---|
| **Create from current work** | Versions panel `+`, chip list, Versions menu | Writes what waits; checks the name is free and writes the creation record (below); `DOLT_BRANCH` at `HEAD`; removes inherited conversations on the new branch; prepares its model tables (`prepare_model`, since `HEAD` may be the creation commit, which has none); copies the current working rows to it; removes the record; switches. The source branch keeps its unsaved work as well | The new branch is deleted, then the record is removed; you stay where you were. If the deletion fails too, the record stays: the branch stays out of the list, and the next opening removes it |
| **Create from a version** | **Start a branch here…** in details and the viewing banner | The creation record; `DOLT_BRANCH` at that hash; removes inherited conversations; removes the record; switches. Viewing ends | The same. Not offered on a version that cannot be read |
| **Switch** | Branch row, chip list | Section 6's rules, then: writes what waits, commits waiting conversations, runs the opening checks on the target and builds its model, moves the connection, reloads. A conversation commit that fails does not stop it: those rows wait in that branch's working set for the next chat commit | Connection and session stay on the current branch |
| **Rename** | Branch row menu | `DOLT_BRANCH('-m')`. Allowed on the current branch; the connection and the stored current branch follow | Name stays |
| **Delete** | Branch row menu, not on `main` or the current branch | Confirmation names the versions only on this branch, its conversations and whether it holds unsaved work (found as the branch row's dot is); then `DOLT_BRANCH('-D')`; conversation folders are let go through `chat_delete::settle`'s host path | Branch stays |

Creating a branch keeps a record in the project folder, beside the `.creating` marker: the branch being
made and the branch it was made from. The record is written atomically and flushed to disk, its folder included, once
the name is checked as free, before `DOLT_BRANCH`. It is removed last, and that removal is flushed too,
folder included, before the hold ends: a record that came back would delete a finished branch. A removal
that fails counts as a failed creation. An older Archforge ignores the file. While a record names a branch,
the branch list leaves it out. While a record remains, another creation is refused until cleanup has succeeded. Opening a project deals with the record before
anything lists or reads branches:

| On opening | Result |
|---|---|
| A record names a branch that exists | That branch is deleted, then the record is removed. Never `main`, the record's source branch, or the branch the project opens on: for those only the record goes, and the status bar says so |
| A record names a branch that does not exist (stopped before `DOLT_BRANCH`) | The record is removed |
| A record cannot be read, or names something outside the safe set of names | Nothing is deleted. It is set aside under a new name, as a damaged preferences file is, and the status bar says so. Archforge cannot tell which branch it meant, so that branch stays listed |
| Archforge stops during this cleanup | The next opening repeats it; a branch already gone is the second row |
| No record | No cleanup, so a branch made outside Archforge is never touched |

A delete cannot be undone in 0.14.0. The confirmation says the branch leaves the project's list and
that its contents may remain in the
project folder's storage, as a deleted conversation does in the project's saved history.

A branch change holds the project still (section 6), because rows handed over during it would reach
the new branch. The target is read and checked over its own connection first; only then does
the current branch change.

Every job handed to the writer names the switch generation it was made under, a counter a switch moves
and a rename does not. The generation is taken when the job is asked for, before any key check. A job
of another generation is refused with an error saying so, and is not applied to the branch now shown.
Reads off the writer (the version list, a preview) carry it too, and an answer from another generation
is dropped.

### Switching

| Situation | What you see |
|---|---|
| No unsaved changes | Switches at once |
| Unsaved changes (the model's, as `has_unsaved` counts them) | "{branch} has unsaved changes. They stay on {branch} and are there when you come back. You won't be able to undo earlier steps." **Save version and switch** (an ordinary Save with the automatic description, inside the hold) / **Switch** / **Cancel** |
| Editor open, turn running, description refused | Section 6 |
| Viewing a version | Back to current work first, then the above |

**Undo starts fresh on every branch change, creation included.** Keeping a session per branch would
double the memory, and steps carry
`Chat N` labels that a new conversation on the other branch could reuse, so that conversation could
undo another's step (`session.rs:286-293`).

Closing or quitting asks only about the current branch. Other branches keep their unsaved work for the
next time they are shown.

### History on a branch

History lists the current branch's line, including versions from before it split off (measured),
with a divider "Shared with main up to here" at the
merge base (`DOLT_MERGE_BASE`). Dolt does not record which branch a branch came from, so the divider
does not claim it. The chat commit that removes inherited conversations is a chat
commit, so it is not a version.

### Where it shows

| Place | What |
|---|---|
| Chip | Section 2: the name appears once a second branch exists, so a one-branch project never sees the word |
| Versions panel | Sections **Branches**, **Working changes**, **History**. Branches is folded while there is only `main` |
| Branch row | Name, current mark, unsaved dot, date of its latest version. The dot counts only the model's changes: `dirty` is a quick first look, and where it is set, that branch's `dolt_status` is read without the chat tables, as `has_unsaved` does. The date is that of its latest version, not of its latest commit, which may be a chat commit; Enter switches; menu has Switch, Rename, Delete |
| Status bar | "On {branch}" beside the save state, once a second branch exists |
| Tab and title dot | The current branch's unsaved state |
| Merging | One line under Branches: "Branches can't be merged yet." Compare shows how two branches differ |

Agents have no branch commands in 0.14.0. `document.inspect` reports the current branch, and
`save_project` saves on it.

---

## 9. Conversations on branches

A conversation belongs to the branch it was started on and shows only there. The history header says
"Conversations stay on the branch where
they started."

The chat tables are ordinary tables on each branch, so a new branch would inherit every
conversation. It does not: creating a branch deletes
the inherited rows on the new branch only, in one chat commit. No table or column is needed.

| Moment | What happens |
|---|---|
| A conversation is started | Its rows and chat commits are on the current branch |
| New branch | Starts with none: "No conversations on this branch yet." The source branch keeps its own |
| Switch | The running turn ends as closing a tab ends it (`lib.rs:758-765`), its reply so far kept and written before the switch; the requests still on their way are settled first (section 6); the program is let go of; the panel reloads from the new branch |
| Numbering | `Chat N` is per branch, so a new branch starts at Chat 1 |
| Rename a branch | Conversations unaffected |
| Delete a branch | Its conversations go with it; the confirmation counts them; their private folders are let go |
| Change links | The session's ledger (`Project::changes`) is reset on a switch, as on opening |
| Viewing a version | Chat stays on the live branch with its composer off: "Back to current work to chat" |
| A link to an element absent from the viewed version | Says so |

Checks against what exists:

| Mechanism | Result |
|---|---|
| Deleted-conversation Undo: `DOLT_MERGE_BASE(from, 'HEAD')` must be `from` (`chat.rs:1511-1519`) | Holds. The commit before a deletion is on that branch's own line. A switch settles the toasts (`chat_delete::settle`), as closing does, so Undo cannot run from another branch |
| A conversation's rows at a branch point | The removed rows remain in the new branch's ancestor commits, the same statement as for a deleted conversation |
| Restore | Touches no chat table, and forward-only keeps every chat commit an ancestor |
| Save and Don't save | Work per connection, so per branch (measured for `DOLT_RESET`); chat commits come first as today |
| Requests still on their way | A question in `Step::Creating`, `Asking`, `Rerunning` or `Keeping` waits on a key check off the writer (`apps/archforge/src/chats.rs:505-530`), and writes, chip writes, deletions and restorations may still be queued. Section 6's step 4 waits for them (`chat_panel::waiting`, `chat_delete::waiting`) |
| Writes arriving after a switch | A chat write names only the project path, so without more one arriving late would reach the new branch. Each carries the switch generation (section 8) and is refused on any other |
| `format` raised on a branch | Per branch, and checked on switching. `main` can stay at an older format |

---

## 10. States

| State | Where | Behaviour |
|---|---|---|
| No version yet | Versions panel | "No versions yet. Save version records your first one." The creation commit is not listed |
| Before the first version | Working changes | No baseline: everything is Added, except the root Model element |
| First version | Details, Compare | "First version: there is nothing earlier to compare it with." Before shows the absent state |
| No changes | Working changes | "No changes since the last version." Save version… disabled, with the reason in its tooltip |
| Unsaved only in `settings` or `model_tables` (an upgrade, a table first used) | Working changes, status bar | Working changes lists nothing; the status bar keeps Unsaved changes, and ⌘S saves them. Save version… stays disabled |
| A branch with no version | Compare | That branch is not offered as a side of a comparison |
| Identical pair | Compare | "These two versions are the same." |
| Restoring a version equal to current work | Details, banner | Not offered; found only when it runs: "This version is the same as your current work." No version is made |
| Loading | Chip, list, centre | Skeleton rows; the previous state stays usable |
| Version unavailable | Row | "This version can't be shown", with the reason behind a "Why?" in the meta text. Stays listed; stepping skips it and says so |
| Newer or foreign model shape | Row | "Made with a newer Archforge." |
| Read failed | Banner | The previous state stays, Retry offered; never half a model |
| Save failed | Status bar, dialog | Says whether a version was made, then Retry |
| Long history | List | Pages of 50, selection held by hash, Show more, filter All / With a description |
| Metadata missing | Details | The row is left out; no invented author |
| Very large model | Compare | The read budgets hold (`model.rs:26`, `:30`). Over them: "Too large to compare", no partial list |
| Only `main` | Branches | Folded; New branch… offered |
| Name empty, too long, or taken | New branch dialog | Under the field, Create disabled |
| A creation record remains | New branch | Refused, saying the last new branch is not cleaned up yet; the next opening removes it |
| Branch can't be opened | Row | Damaged, newer format, or holds scripts: the reason, and you stay on the current branch |
| Name can't be used | Row | Listed, disabled, with the reason |
| Stored branch is gone on reopening | Status bar | Opens `main` and says "The branch {name} is no longer in this project" |
| Stored branch can't be opened on reopening | Status bar | Opens `main` and gives the reason |
| Switch failed | Branch list | The reason; current branch and work unchanged |
| Delete refused | Dialog | The reason; the branch stays |

---

## 11. Names, menus and journeys

Names: rail and panel **Versions** (the automation id stays `source-control`); **Working changes**,
**History**, **Branches**; Save version…, View this
version, Back to current work, Compare…, Restore this version…, New branch…, Start a branch here…,
Before, After, Added, Changed, Removed. Hashes, "commit",
"uncommitted" and "checkout" stay out of the main journey. The design's branch selector gives way to
the Branches section, and its remote row,
graph line and "Changed by" go.

Menus (AR-154, deciding AR-20): **File**, **Edit**, **View**, **Model**, **Versions**, **Help**, the
macOS application menu, and an in-window menu on Windows and
Linux. A group appears only with real actions. **File** holds Save, and **Versions** holds Save version…. On
Windows and Linux Quit is under File and About under Help. **Model** holds the create-element and
create-relationship actions through the same commands as the diagram tools (`workspace.tools.*`,
`crates/ui-egui/src/canvas.rs:2714-2750`): Note, Link note and the open diagram's own kinds, for UML those
of the existing relationship and classifier lists. **Versions** also holds History, Compare…, New branch…,
Switch branch, Back to current work, and older and newer. New project, Close project, the zooms and the
tool picks show no shortcut, as none exists today.

Journeys to draw in `ArchforgeUx.figcraft`, from page 02's components, in light and dark, with real
content:

1. **Review → Save version.** Unsaved state on the chip and rail; Working changes; element
   Before/After; the dialog; changed-after-review; after saving; no-changes and failure variants.
2. **Browse → inspect → return.** Chip list; version details; viewing with banner and read-only
   Properties; refusals (editor open, Chat running); Back to current work; unavailable and
   long-history variants; 640 px.
3. **Compare two versions.** Selector with Swap; list with the pair in the header; element
   comparison with a related link; diagram comparison with model and layout apart; identical,
   first-version and too-large variants.
4. **Restore.** Confirmation; unsaved work kept; the new version; failure.
5. **Branches.** New from current work and from a version; Branches section and chip with two
   branches; switch with unsaved changes; a new branch's empty Chat; rename; delete confirmation;
   `Can't be merged yet`; failure states; 640 px.

Stories and PR-sized subtasks are under AR-143.
