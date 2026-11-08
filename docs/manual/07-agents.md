# Working with an agent

An AI agent can work in the project you have open while you watch. It reads the model and the
skills that apply, and it changes the model with the same commands you use, so every change it
makes shows in the window at once and ⌘Z (Ctrl Z) takes it back.

There are two ways to work with one:

- **Chat**, inside Archforge: ask Claude or Codex in the Chat panel. Set it up once in
  [Setting up AI agents for Chat](#setting-up-ai-agents-for-chat), then see [Chat](#chat).
- **Your own agent**, outside Archforge: the coding assistant or tool you already use, connected
  through Archforge's command-line tool. See [Connecting an agent](#connecting-an-agent).

Both work on macOS and Linux. On Windows they are not available yet; everything else in Archforge
works as usual.

## What does what

- **Archforge** holds the model, draws it, checks every change, keeps the history and saves. It
  never generates code or text itself.
- **The agent** does the writing: an answer about the model, SQL DDL, documentation, a change to the
  model you asked for. It runs either in Archforge's Chat panel or outside Archforge, talking to
  Archforge through `archforge-cli`, an [MCP](https://modelcontextprotocol.io) server.
- **The agent's provider** runs the AI model behind it: Anthropic for Claude, OpenAI for Codex.
  What the agent reads is sent there; see
  [What is sent to the provider](#what-is-sent-to-the-provider).

## Setting up AI agents for Chat

Chat talks to an agent program installed on your computer: Claude through the program `claude`,
or Codex through the program `codex`. Archforge never bundles them. You install one, give it a key
or a sign-in, and let Chat use the model. You need to do this once.

You set them up in **Settings ▸ AI Agents**: click the avatar at the right of the bar, or press
⌘, (Ctrl ,), and choose **AI Agents** on the left.

<!-- picture: settings-ai-agents-ready light | Settings, AI Agents: Claude with a saved API key and its model, Codex signed in with ChatGPT, and Let Chat use the model on -->

The page has a card for each agent, then **Let Chat use the model**, **Check connection** and
**Send test message**. **Default agent**, at the top, is the agent a new conversation starts with.

Opening the page sends nothing over the network. Archforge looked for the programs when it
started, asking each only for its version. Your keys are read from the secure store the first
time you open the page or the Chat panel, never when Archforge starts, so the system asks for
access only once you use AI agents. Who Codex is signed in as is read by starting the codex
program on your computer: the first time the page shows or a Codex conversation starts, and then
again each time you open the page, choose **Check connection**, or a Codex conversation says
*Sign-in expired*.

### What you need

- **macOS or Linux.** On Windows the page says *Not available on this system yet*, and Chat stays
  off.
- **The agent's program**, of a version Archforge works with (below).
- **An account with the agent's provider**, to pay for what Chat asks (see
  [How you pay](#how-you-pay)).
- **On Linux, a running Secret Service** (such as GNOME Keyring or KWallet) to save an API key.
  Without one, only Codex's sign-in with ChatGPT works (see
  [Where your keys are kept](#where-your-keys-are-kept)).
- **From the source**, build the command-line tool too, `cargo build -p archforge-cli`: Chat runs
  it to reach the model. The packages carry it already.

### Installing the programs

Install the program of the agent you want, as its maker documents it. The commands need Node.js and
npm; each card's **Installation guide** opens the maker's own page, with other ways to install.

| Agent | Program | Install | Versions Archforge works with |
|---|---|---|---|
| Claude | `claude` | `npm install -g @anthropic-ai/claude-code` | 2.1.286 or later 2.x; tested with 2.1.317 |
| Codex | `codex` | `npm install -g @openai/codex` | 0.166.0 or later 0.x; tested with 0.166.0 |

1. In a terminal, run the install command.
2. In **Settings ▸ AI Agents**, look at the agent's card. If it says *Not found*, choose **Look
   again**.

The card's badge says what Archforge found:

- *Found · claude 2.1.317*: ready to use.
- *Not yet tested with this version · claude 2.1.320*: a newer version of the same major version,
  as a fresh install usually is. It works; Archforge was not tested with it yet.
- *Not found*: the card shows the install command, **Copy the command**, **Look again** and
  **Installation guide**.
- *Too old*: update the program with the command the card shows, then choose **Look again**.
- *Can't be used*: the card says why. A version of the next major version (Claude 3, Codex 1) is
  not used until Archforge is tested with it.

<!-- picture: settings-ai-agents-missing light | Settings, AI Agents: Claude's program not found, with its install steps, and Codex on a saved API key -->

Archforge looks on your `PATH`, then in the folders installers use: npm's global folder,
`~/.local/bin`, `~/.claude/local`, Volta's, Bun's and the Node version manager's folders,
`/opt/homebrew/bin` on macOS, Homebrew's folder on Linux, and `/usr/local/bin`. An application
started from the macOS Finder does not see the `PATH` your terminal has, so these folders are how
it finds a program you installed from a terminal. A program found only in one of these folders is
used only when no other account on the computer can change it.

To use a program somewhere else, start Archforge from a terminal with `ARCHFORGE_CLAUDE` or
`ARCHFORGE_CODEX` set to the program's full path. Archforge then looks there alone.

### How you pay

Every question Chat sends is paid for by your account with the agent's provider. Archforge itself is
free and takes no part in the payment.

| Agent | How Chat signs in | What it uses |
|---|---|---|
| Claude | **API key** only | Your Anthropic API account, billed for what you use. A Claude subscription cannot be used in Chat. |
| Codex | **Sign in with ChatGPT** | Your ChatGPT plan: the Codex use it includes, within its limits. |
| Codex | **API key** | Your OpenAI API account, billed for what you use. |

When a plan's or an account's limit is reached, Chat says *Rate limited* (see
[When something goes wrong](#when-something-goes-wrong)).

### Adding a key or signing in

For **Claude**:

1. Create an API key in your Anthropic account.
2. On the Claude card, paste it under **API key**.
3. Choose **Save key**.

The field then shows only the key's last four characters, with **Replace** and **Remove**. Choose a
**Model** below it: **The program's default**, **Sonnet**, **Opus** or **Haiku**. The model can be
chosen once a key is saved.

For **Codex** with your ChatGPT account:

1. On the Codex card, choose **Sign in with ChatGPT** under **Sign-in**.
2. Choose **Sign in…**. Your browser opens the sign-in page.
3. Sign in there. The card then says *Signed in as* your email and plan.

On a computer without a browser, choose **No browser on this computer? Sign in with a device
code.** instead of step 2. The card shows a code, which runs out in 15 minutes, and under it the
sign-in page's address:

1. On any device with a browser, open that address. Type it, or select it on the card or copy it
   with the icon beside it (**Copy the address**) and send it to the device.
2. Enter the code there. The icon beside the code copies it (**Copy the code**).
3. Sign in. The card then says *Signed in as* your email and plan.

**Open sign-in page** opens the same address in this computer's browser, if it has one.
**Cancel** stops the sign-in.

The codex program names the page each time. Archforge shows the address, and opens it, only when
it is on `https://auth.openai.com/`. For any other address the card says *Not an address
Archforge opens* in its place, in red, with nothing to copy, and **Open sign-in page** is greyed
out; choose **Cancel**.

<!-- picture: settings-ai-agents-device-code light | Settings, AI Agents: Codex's device code and the sign-in page's address, each with its copy icon, Open sign-in page and Cancel -->

A sign-in not finished within 15 minutes, or one the codex program cancels, says so under it; sign
in again. **Sign out** ends the sign-in.

When a sign-in expires, Chat says *Sign-in expired*, and the Codex card then shows it too, with
the email (*The ChatGPT sign-in for … has expired.*) and **Sign in again…**, without a restart.
Until the codex program or a Codex conversation reports the expiry, the card may still say
*Signed in as*.

For **Codex** with an API key:

1. Create an API key in your OpenAI account.
2. On the Codex card, choose **API key** under **Sign-in**.
3. Paste the key and choose **Save key**.

Codex's **Model** stays **The program's default**: the codex program chooses.

### Where your keys are kept

Archforge keeps API keys in the system's secure store and nowhere else: never in the project, never
in the preferences, never in a file of its own. A key reaches the agent's program only while Chat
runs it or sends the test message: Claude's through the program's environment, Codex's through its
input, never on a command line. A ChatGPT sign-in is kept by the codex program itself, in its own
files in Archforge's private settings folder (`agents/codex-home`, which other users of the
computer cannot open), on macOS and Linux alike; it never goes in the system's Keychain or Secret
Service. Unlike a key in the Keychain, it is an ordinary file: programs you run can read it, and
backups of your home folder include it. **Sign out** asks the codex program to remove it. Archforge
reads only who is signed in, never the sign-in's tokens.

| System | API keys (Claude, Codex) | Codex's sign-in with ChatGPT |
|---|---|---|
| macOS | The macOS Keychain, under *Archforge* | The codex program's own files, in `agents/codex-home` |
| Linux with a Secret Service | The Secret Service | The codex program's own files, in `agents/codex-home` |
| Linux without a Secret Service | Cannot be saved | The codex program's own files, in `agents/codex-home` |
| Windows | Not available yet | Not available yet |

On Linux without a running Secret Service, the page says *No secure store on this system*, and
**Save key** says *Can't be saved: there is no secure store to keep it in.* Start one and restart
Archforge to save keys. Once Codex is signed in with ChatGPT, its card says that the codex program
keeps the sign-in in its own files, so that sign-in works without the secure store.

Archforge's private settings folder is `~/Library/Application Support/Archforge` on macOS and
`~/.config/archforge` on Linux (`$XDG_CONFIG_HOME/archforge` where that is set). The agents' working
files are in its `agents` folder.

### Let Chat use the model

**Let Chat use the model** is off until you turn it on. While it is off, a new conversation says
*No agent set up yet*.

Each Chat conversation gets agent access of its own while it runs. Chat reads the project's model
through it. A conversation changes the model only when **Allow changes to the
model** is on in it (see [Chat](#chat)).

Archforge enforces that itself: through its agent access, a conversation without Allow changes is
refused every change, undo and save it asks for. The access opens on your computer alone when a
conversation needs it. It closes when the last conversation ends (turning the switch off ends them
all, and each one's access is revoked at once), and when you quit.

Turning it off also ends every conversation that runs: a reply under way stops, and what it wrote
so far stays, marked as stopped. Until you turn it on again:

- A new conversation says *No agent set up yet*, with **Configure agent**.
- A conversation already started, or one reopened from the history, keeps its box. A question
  sent there, or **Send again**, starts its agent again and is sent.

The status bar says which, at its right beside the project's location:

- *Chat is using the model*, with a blue dot, while a conversation holds agent access;
- *Agent access off*, with a purple dot, otherwise.

Click it, or reach it with Tab and press Enter, to open **Settings ▸ AI Agents**.

### Checking it works

- **Check connection** is free and runs on your computer. It looks again for each program, asking
  it only for its version, and reads which API keys are saved and who Codex is signed in as;
  Archforge sends nothing. The badge on each card sums up its agent.
- **Send test message** sends one message to the default agent's provider: *Reply with the single
  word OK.* It uses a little of your plan or API credit. It runs without access to your model, in a
  folder of its own that is removed afterwards. The page then says *Claude answered the test
  message* (or Codex), or why it failed. A key the provider refuses is marked on its card: replace
  it, or remove it.

<!-- picture: settings-ai-agents-refused light | Settings, AI Agents: Claude's key refused by the test message, and Codex's sign-in expired -->

The test message goes to the default agent only. To test the other one, make it the default for a
moment.

## Chat

The Chat panel is the speech bubble at the top of the right rail. It holds the open project's
conversations with Claude or Codex.

Until the default agent is ready and **Let Chat use the model** is on, a new conversation says
*No agent set up yet*; **Configure agent** opens **Settings ▸ AI Agents** (see
[Setting up AI agents for Chat](#setting-up-ai-agents-for-chat)). A key the test message found
refused counts as not ready.

<!-- picture: workspace-chat light | The Chat panel ready: the intro, the context chips, the composer with Claude's badge, Allow changes off -->

### Asking a question

**A project made with Archforge 0.12 or earlier** is raised to a newer project format by the first
question asked in it, as it then holds conversations. Archforge 0.12 and earlier cannot open it
after that: they say the project is in a format they do not read. If someone still opens the
project with an older Archforge, copy its `.archforge` folder first, or wait with Chat until they
have moved on. Opening the project alone changes nothing.

1. Open a project, and the diagram your question is about.
2. Select the elements it is about, if any.
3. Type your question in the box at the foot of the panel, *Ask about this model…*.
4. Press Enter, or choose Send (the arrow).

Shift Enter starts a new line. A question is at most 64 KiB.

Your question is kept in the project first, then sent. A question that holds one of your saved API
keys is neither sent nor kept: the panel says so and leaves it in the box for you to edit. So does a
project that cannot be written.

- **The chips** above the box are sent with your question: the project, the open diagram and what
  is selected, by name. Remove one with its ×; it stays removed for this conversation, also when
  you open it again from **Conversations**. A removed chip cannot be put back in that conversation:
  start a new conversation to have every chip again. Which chips **Send again** sends is in
  [What Send again and Enter send](#what-send-again-and-enter-send). The line under
  them is a reminder: they, and everything the agent reads, go to the agent's provider.
- **What the agent does** shows as it happens, under its name: a row such as *Read diagram
  Commerce* with *8 classes* beside it, or *Changed Customer* with *description*. Each row is what
  Archforge itself answered the agent, so a refused change says *refused*. The reply is written in
  after them.
- **Stop** (the square in place of Send) ends the reply; what was written so far stays, marked as
  stopped. If the agent's program does not stop within ten seconds, Archforge ends it. While a reply
  runs you can type the next question, but Enter, the badge, the switch, New conversation and the
  history wait: *Stop the reply first*.
- **The badge** in the box names the agent and its model, *Claude · Sonnet*, then the exact version
  once the agent names it, *Claude · Sonnet 4.5*. Choose another of Claude's models there (Codex has
  only the program's default); before the first question you can choose the other agent too. A
  conversation keeps its agent and model: changing the default in Settings never switches a
  conversation already started.

Links in a reply open only after you confirm them, and only web addresses; pictures show their
description, and HTML shows as text.

### Read-only, or Allow changes

**Allow changes to the model**, under the box, is off for every new conversation, and every time
one is opened again from **Conversations**, the one already shown included. While it is off, the
agent can only read: Archforge refuses every change, undo and save it asks for.

Turn it on, and the agent may change the model:

<!-- picture: workspace-chat-changes light | A turn with changes: five changes as one step, with Undo this turn's changes -->

- Everything one question changes is **one step** of the project's undo history, named *Chat 1:
  Turn 2*, as long as nothing else changes the model while the reply runs.
- The green card under the changes offers **Undo this turn’s changes**. It takes back exactly those
  changes and keeps your own.
- If you or another conversation change the model while the reply runs, the question's changes
  become separate steps around yours. Then, or once the model was changed since, or the project was
  closed in between, the card says the changes can no longer be undone on their own. ⌘Z (Ctrl Z)
  still works step by step.

Turn it off again whenever you only want answers.

### What is sent to the provider

Chat runs the agent's program on your computer, and the program sends to its provider (Anthropic
for Claude, OpenAI for Codex):

- **your question**, as you typed it;
- **the chips you kept**, as one line after the question, *[Context the person chose: …]*: the
  name, kind and id of the project, the open diagram and each selected element, up to 50 in all;
  past that the line only counts the rest (*and 120 more selected*). To have the agent told every
  element by name, select fewer at a time;
- **the conversation so far**: this conversation's earlier questions and replies, as the agent's
  program kept them in Archforge's private folder on this computer (see below). Claude keeps the
  whole conversation in its transcript, but may sum up earlier parts when its context fills, so a
  long conversation's later questions may not carry every earlier word. Codex keeps only its text
  messages, not what the agent read or changed, and at most 4,096 messages and 4 MiB of text, the
  oldest going first; whenever its program starts again (the conversation reopened, **Allow
  changes** or the model switched), Codex gets only that copy, so repeat details it may have lost;
- **what the agent reads in the model**, exactly as Archforge answers it: the elements it looks at
  with their names, descriptions, attributes and other details, the diagrams, the skills, and what
  is open and selected, with the project's name and its folder's location on your computer;
- **the results of its changes**, while Allow changes is on;
- **the list of Archforge's tools** it may use (only the reading ones while Allow changes is off),
  and the program's own instructions.

The rows under the agent's name sum up what the agent read and changed, each by a label and a
detail, such as *Read diagram Commerce* with *8 classes*. They are a summary, not the data that
went: looking up Archforge's commands makes no row, and past 2,000 calls in one turn the rest are
counted in a single line.

Beyond that, Chat sends nothing of yours:

- **No files from your computer.** The programs run with their own tools switched off (no shell,
  no file reading, no web search), in a folder of their own. They reach only the open
  project's model, through Archforge.
- **No other project's model**, and **no other conversation**: each conversation has its own
  folder.
- **No saved API key in a question**: a question that holds one is neither sent nor kept.
- **The programs' own traffic** is not yours, and Archforge switches off what it can of it:
  Claude's updates, telemetry, error reports and feature-flag fetches, and Codex's analytics, web
  search and plugins.

The conversation so far comes from the program's own copy in Archforge's private settings folder,
not from the project. Open the project on another computer, or after that copy is gone, and the
panel still shows the earlier messages, but the agent does not get them: Codex answers without
them, and Claude cannot pick the conversation up, so the question ends with *No answer*. Start a
new conversation there, and repeat what the agent needs to know.

Opening Settings, **Check connection** and the history send nothing. **Send test message** sends
only its one sentence.

### When something goes wrong

A card in the conversation says what went wrong and what to do.

<!-- picture: workspace-chat-network-lost light | Connection lost: the question back in the box, ready to send again -->

| Card | What happened | What to do |
|---|---|---|
| *Rate limited* | The provider asks Chat to wait: a limit of your plan or account is reached. | Wait a while, then **Send again**. |
| *Connection lost* | The network went away. | Check your connection, then **Send again**. If it went before the agent answered anything and the box is empty, your question is back in the box, ready to send, edited or not. |
| *Model not available* | The model is not offered to your key or plan, or is busy. | Claude: choose another model in the badge, then **Send again**. Codex offers only the program's default, so there is nothing to choose: **Open Settings** to check the key or sign-in, or ask again later. |
| *Program not found* | The agent's program is not on this computer. | **Open Settings**, install it, choose **Look again**, then ask again. |
| *Key refused* | The provider refused the saved key: it is wrong, or was revoked. | **Open Settings**, **Replace** the key, then ask again. |
| *No key saved* | There is no key or sign-in for the agent. | **Open Settings**, add one, then ask again. |
| *Sign-in expired* | Codex's ChatGPT sign-in has expired. | **Open Settings**: the Codex card says the sign-in expired. Choose **Sign in again…**, then ask again. |
| *Not started yet* | The conversation's working files are being removed: a copy of the project deleted it, or it was just brought back with Undo. | **Send again** in a moment. Reopened from **Conversations** later, the same turn shows *No answer*, as the reason is not kept. |
| *No answer* | Anything else: the agent stopped before answering. | **Send again**. |

**Send again** asks the same question again in the same place: the conversation keeps it once. A
question whose turn already changed the model is not sent again; ask again in your own words.

#### What Send again and Enter send

A failed question either stays in the conversation or, after *Connection lost* with the box
empty, comes back into the box. What the card's **Send again** and Enter then send:

| Where the failed question is | **Send again** on its card sends | Enter sends |
|---|---|---|
| In the conversation, the box empty | The question as last sent, with the chips it last went with | Nothing |
| In the conversation, other text in the box | The question as last sent, with the chips it last went with | The box's text, as a new question, with the chips shown now |
| Back in the box, as it came back or edited | The box's text, with the chips shown now | The box's text, with the chips shown now |
| Back in the box, then the box emptied | The question as last sent, with the chips it last went with | Nothing |

- **Send again** and Enter on a question back in the box run the failed turn again in its place:
  the conversation keeps the question once. Enter on other text starts a new turn.
- *As last sent*: each time a question is sent again, what was sent replaces the earlier question
  and chips.
- *The chips shown now* are the chips above the box, a newly selected element included, without
  those you removed in this conversation.
- **Opened again from Conversations** (after another conversation was shown), a question that was
  back in the box counts as in the conversation: the first two rows. The box keeps its text when
  you switch conversations, so an edited question still in it goes with Enter as a new question,
  with the chips shown now.
- The card offers **Send again** only on the conversation's last turn, and only as the
  [table above](#when-something-goes-wrong) says: not on a card that sends you to Settings, and on
  *Model not available* only after another model was chosen. Once you ask another question, the
  earlier card no longer offers it: ask that question again as a new turn.

To send only the chips shown now, ask a new question. The earlier question, with its chips' line,
stays part of the conversation so far, so it may still reach the provider.

While the provider does not answer (no network, or the provider is busy), the agent's program
keeps trying again for a while: Claude for about three minutes without a network. The reply says
so in an amber line, *No answer from Claude's provider yet. Trying again, attempt 3 of 10…*, with
the attempt and of how many as far as the program says them, and **Stop** ends the wait at once.
When the program gives up, the card for what went wrong shows: *Connection lost* when the network
stayed away, *Rate limited* when the provider kept asking Chat to wait.

A turn that was running when Archforge closed says *Interrupted: Archforge closed while this turn
ran.*

### Conversations

**Conversations** (the clock at the top of the panel) lists the project's conversations, newest
first, with when, the agent and how many messages. Click one to open it again, read-only until you
allow changes; this holds for the conversation already shown too. **New conversation** (+) starts
another.

<!-- picture: workspace-chat-history light | The project's conversations, newest first, saved with the project -->

Conversations are kept in the project: they travel with it and its copies, and never show in a
different project.
A conversation is kept as you go, without a save; the changes the agent made to the model are not:
save them as your own (⌘S), or **Don't save** drops them and their card says *Not kept: the changes
were not saved.* Anyone you give a copy of the project can read them. Leaving the project's tab
stops a reply that runs; closing the project ends its conversation.

### Deleting a conversation

Point at a conversation in the list and click its bin, or right-click it (or press the menu key,
or Shift F10) and choose **Delete conversation**. With the keyboard on a row, ⌫ or Delete does the
same. Archforge asks first:

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
files for that conversation are removed from your computer, with these exceptions, each said in
Archforge's log:

- a copy of the project open beside it still shows that conversation;
- that copy asks the conversation something in those few seconds: its card says *Not started
  yet*, *This conversation is being removed. Try again in a moment.*, and **Send again** a moment
  later goes through;
- the program did not end in time, the project did not answer, Archforge was closing, or the
  removal failed.

Then the files stay in the `agents` folder of Archforge's private settings folder, as a
`conversation-…` folder, until you remove it; nothing removes it later. While a copy of the project
open beside it still shows the conversation, leave the folder: that copy's conversation uses it,
and without it Claude cannot pick the conversation up and Codex loses its saved copy. Remove it
only once no open project shows the conversation.

When the project's conversations cannot be changed (for example because one of its tables is
damaged), the bin and the menu item are greyed out and say why.

## Connecting an agent

To work with an agent of your own, outside Archforge, connect it to the open project. You need
none of this for Chat. Agent access works on macOS and Linux for now.

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

**File ▸ Generate…** says whether agent access is on. The port listens only on your own computer,
and a token made for each start, kept in a file only you can read in Archforge's settings folder,
guards it. Without `--control`, nothing listens.

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
  resources `archforge://project` (the open project, its revision, the selection),
  `archforge://skills`, `archforge://element/{id}` (a schema with its tables, a table, a diagram, or
  a class, an interface, an enumeration with its literals, a data type, a primitive type or an
  association class, each with its attributes, operations and description) and
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
  end's id (or `diagram_connect_all`), all in one `batch` for one undo step.
  `uml_association_set_end` sets an end by its place among the ends; its role must differ from every
  other end's, and an n-ary end takes no aggregation. `uml_association_add_end`,
  `uml_association_reconnect_end` and `uml_association_remove_end` add, move and remove an end; a
  leg is drawn or kept only on the diagrams that show both the diamond and the end's class, and what
  the ends subset or redefine that a move or removal makes invalid is dropped and listed. Removing
  one of three ends is refused unless `binary` is true, which makes it an ordinary association
  between the two classes left. `uml_relationship_list` gives it with the kind `n-ary association`
  and all its ends.
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
