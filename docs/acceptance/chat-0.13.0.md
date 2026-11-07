# Chat acceptance pass for 0.13.0 (AR-207)

The script for the acceptance pass of Chat: real round trips with Claude and Codex, failure
injection, and a search for keys in every place Archforge writes. It needs the owner's own keys and
accounts, and spends a little API credit and plan use. Nothing in it has been run yet; the manual's
[Setting up AI agents for Chat](../manual/07-agents.md#setting-up-ai-agents-for-chat) and
[Chat](../manual/07-agents.md#chat) say what the steps should look like.

Run it on macOS, and on Linux where a step says so. Windows has no Chat yet: check only W1.

## How to record

1. Copy this file to the ignored `plan/` folder, as `plan/acceptance/chat-0.13.0-<date>.md`.
2. Fill the [Results](#results) table there, one row per step: **pass**, **fail** or **skipped**,
   with a note. For a fail, note what showed instead, and the time, so the log can be found.
3. Paste the filled table into AR-207. Open a Fireline bug for each fail.

Never paste a key, a sign-in code or a file holding one into the record. The commands below print
file names and counts only.

## Before you start

Build or install the build under test. From the source, build both programs; Chat runs the
command-line tool beside the application:

```sh
cargo build -p archforge -p archforge-cli
```

Set up the shell you run the checks from, in bash or zsh. Keep it open for the whole pass. The
pass's own files go in your home folder, outside every place the key search looks:

```sh
WORK="$(mktemp -d "$HOME/archforge-acceptance.XXXXXX")"; chmod 700 "$WORK"   # removed at the end
PROJECT="$HOME/Documents/Archforge/Chat acceptance.archforge"                # made in step P1
case "$(uname -s)" in
  Darwin) SETTINGS="$HOME/Library/Application Support/Archforge" ;;
  *)      SETTINGS="${XDG_CONFIG_HOME:-$HOME/.config}/archforge" ;;
esac
echo "$SETTINGS"
```

Record the versions under test:

```sh
claude --version; codex --version
/Applications/Archforge.app/Contents/MacOS/archforge --version   # from the source:
#                                                                   target/debug/archforge --version
date -u +%FT%TZ                                                   # the pass's start
```

You need:

- an Anthropic API key with a little credit, and the right to make and revoke keys in that account;
- an OpenAI API key with a little credit, and the right to make and revoke keys;
- a ChatGPT account whose plan includes Codex;
- a second device with a browser, for the device code (a phone works);
- `dolt` on your `PATH`, the one Archforge uses, for the project search in K4;
- on Linux, `secret-tool` (libsecret) and `wl-paste` or `xclip`.

### P1. A project to talk about

1. Start Archforge. Make a project named *Chat acceptance* in the default location.
2. Build the class model of the manual's
   [A first UML class model](../manual/05-uml-class-model.md) up to its first few classes, or any
   class diagram with three classes or more.
3. Save it (⌘S).

**Expect:** the status bar says *Saved* and *Agent access off*.

## Round trips

One full round trip per route. Each uses the project of P1.

### R1. Claude with an API key

1. Open **Settings ▸ AI Agents**. **Expect:** Claude's badge says *Found · claude* and its
   version, or *Not yet tested with this version · claude* and its version.
2. Paste the Anthropic key on the Claude card and choose **Save key**. **Expect:** the field shows
   only the key's last four characters, with **Replace** and **Remove**.
3. Set **Default agent** to Claude. Turn **Let Chat use the model** on.
4. Choose **Send test message**. **Expect:** *Claude answered the test message just now* within a
   minute.
5. Close Settings. Open the Chat panel (the speech bubble at the top of the right rail). Select one
   class.
6. Ask: *Which classes are on this diagram, and which have no description?* **Expect:** the status
   bar says *Chat is using the model*; rows such as *Read diagram …* appear under *Claude*; then a
   reply naming the classes; the badge names the exact model, such as *Claude · Sonnet 4.5*.
7. Turn **Allow changes to the model** on. Ask: *Give each class without a description a one-line
   description.* **Expect:** rows *Changed …* with *description* beside them, the card *N changes
   to the model* with **Undo this turn’s changes**, and the descriptions in **Properties**.
8. Choose **Undo this turn’s changes**. **Expect:** *N changes undone*; the descriptions are gone.
9. Ask a long question, such as *Describe every class in detail.* Choose **Stop** while it
   answers. **Expect:** the reply so far stays, marked as stopped.
10. Close the project, open it again, open **Conversations** (the clock). **Expect:** the
    conversation is listed with *Claude* and its message count. Open it. **Expect:** every message
    is there, and **Allow changes to the model** is off.
11. Ask a follow-up: *What did I ask you first?* **Expect:** an answer that knows the earlier
    questions (the program resumed the conversation).

### R2. Codex with an API key

1. On the Codex card choose **API key** under **Sign-in**, paste the OpenAI key and choose
   **Save key**. **Expect:** *API key saved · ends in* its last four characters.
2. Set **Default agent** to Codex. Choose **Send test message**. **Expect:** *Codex answered the
   test message just now*.
3. In the Chat panel choose **New conversation** (+). Repeat R1 steps 6 to 11 with Codex; the badge
   reads *Codex*.
4. Quit Archforge, then check that Codex wrote no key file:

   ```sh
   ls -la "$SETTINGS/agents/codex-home/auth.json" 2>&1
   ```

   **Expect:** *No such file or directory*.

### R3. Codex with ChatGPT, in the browser

1. Start Archforge. On the Codex card choose **Sign in with ChatGPT**, then **Sign in…**.
   **Expect:** the browser opens the sign-in page; the card says *Waiting for you to finish in the
   browser…*.
2. Sign in in the browser. **Expect:** the card says *Signed in as* your email and plan.
3. Choose **Send test message**. **Expect:** *Codex answered the test message just now*.
4. Ask one question in a new conversation. **Expect:** a reply, with *Read …* rows.
5. Check where the sign-in is kept:

   ```sh
   ls -la "$SETTINGS/agents/codex-home/" | grep -c auth.json
   ```

   **Expect:** `0` on macOS and on Linux with a Secret Service (the sign-in is in the secure
   store). On Linux without one, `1`, and the card says the codex program keeps the sign-in in its
   own files.
6. Choose **Sign out**. **Expect:** *Not signed in*, with **Sign in…**.

### R4. Codex with ChatGPT, with a device code

1. On the Codex card choose **No browser on this computer? Sign in with a device code.**
   **Expect:** a code, *It runs out in 15 minutes.*, **Open sign-in page** and **Cancel**.
2. The card does not show the sign-in page's address yet (a known bug, to be fixed separately).
   Choose **Open sign-in page**: it opens the page in this computer's browser. Copy the address
   from the browser's address bar to the second device, open it there and enter the code.
   **Expect:** the address starts with `https://auth.openai.com/`; then *Signed in as* your email
   and plan. Record the address's path (it holds no secret) in the notes.
3. Choose **Send test message**, then ask one question. **Expect:** as in R3 steps 3 and 4.
4. Start another device-code sign-in after signing out, and choose **Cancel**. **Expect:** the
   card goes back to *Not signed in*; no error.

Sign in again (R3 or R4) before the failures.

### R5. Settings send nothing

1. Turn the network off (Wi-Fi off, cable out).
2. Open **Settings ▸ AI Agents** and choose **Check connection**. **Expect:** the badges sum up
   both agents as before, with no error about the network.
3. Turn the network back on.

## Failure injection

### F1. A revoked key

1. Make a second Anthropic key. Save it on the Claude card (**Replace**). Choose **Send test
   message**. **Expect:** answered.
2. Revoke that key in the Anthropic account.
3. Choose **Send test message**. **Expect:** *The test message to Claude failed: The provider
   refused the key (…)*, and the key field says *The provider refused this key when the test
   message was sent (…). Replace it with a valid key, or remove it.*
4. Ask a question in the Claude conversation of R1 (open it from **Conversations**). **Expect:**
   the card *Key refused* with **Open Settings**. A new conversation would show *No agent set up
   yet* instead, as the refused key counts as not ready.
5. Choose **Open Settings**, **Replace** the key with the valid one, ask again. **Expect:** a reply.
6. Repeat 1 to 5 for Codex with an OpenAI key (Codex on **API key**). **Expect:** the same, for
   Codex.

### F2. An expired sign-in

1. With Codex signed in with ChatGPT (R3), end the sign-in from the ChatGPT account: use its
   security settings to sign out of all devices.
2. In Archforge, ask a question in a Codex conversation. **Expect:** the card *Sign-in expired* with
   **Open Settings**.
3. Choose **Open Settings**. **Expect:** the Codex card may still say *Signed in as*: Archforge
   reads the sign-in once a run. Record what it says.
4. Quit and start Archforge, and open **Settings ▸ AI Agents**. **Expect:** *Sign-in expired*,
   with **Sign in again…**.
5. Sign in again, then ask again. **Expect:** a reply.

If the account offers no way to end the sign-in, leave Archforge signed in for a few days and try
step 2 then. Record what the card says in either case: *Key refused* or *No answer* in place of
*Sign-in expired* is a fail to note.

### F3. Network off

1. Turn the network off. Ask a Claude question. **Expect:** the reply waits (Claude retries for
   about three minutes), then *Connection lost*: *The network went away before Claude answered.
   Your message is back in the box below, ready to send again.* The question is in the box.
2. Turn the network on. Choose **Send again**. **Expect:** the question is sent once; the
   conversation shows it once.
3. Turn the network off, ask another question, and choose **Stop** before a card comes.
   **Expect:** the reply ends at once, marked as stopped. Turn the network on.
4. Ask a long question. While the reply streams, turn the network off. **Expect:** *Connection
   lost*, *The network went away while Claude was answering.*, the reply so far kept. Turn the
   network on and choose **Send again**. **Expect:** a reply; the question shows once.
5. Repeat 1 to 4 with Codex. **Expect:** the same cards, naming Codex.
6. With the network off, choose **Send test message**. **Expect:** *The test message to … failed:*
   with a reason; nothing else changes.

### F4. A program removed

Archforge looks for the programs in several folders, so moving one copy away may leave another.
`ARCHFORGE_CLAUDE` (and `ARCHFORGE_CODEX`) make Archforge look in one place alone. Start Archforge
from the terminal for this step; `APP` is its program:

```sh
APP=/Applications/Archforge.app/Contents/MacOS/archforge     # or target/debug/archforge
CLAUDE_AT="$(command -v claude)"; echo "$CLAUDE_AT"
```

1. Quit Archforge, then start it with a program that is not there:

   ```sh
   ARCHFORGE_CLAUDE="$CLAUDE_AT.away" "$APP"
   ```

2. Open **Settings ▸ AI Agents**. **Expect:** Claude's badge says *Not found*; the card shows the
   install command, **Look again** and **Installation guide**.
3. With Claude the default agent, open the Chat panel. **Expect:** *No agent set up yet*, with
   **Configure agent**.
4. Quit, and start Archforge with the real program as the only place to look:

   ```sh
   ARCHFORGE_CLAUDE="$CLAUDE_AT" "$APP"
   ```

   Ask in a Claude conversation. **Expect:** a reply. Then, in another terminal, move the program
   away (`sudo` where its folder needs it), and ask once more in a new conversation (**New
   conversation**, +):

   ```sh
   mv "$CLAUDE_AT" "$CLAUDE_AT.away"
   ```

   **Expect:** the card *Program not found* with **Open Settings**. Record what shows if not.
5. Put it back and choose **Look again** in Settings:

   ```sh
   mv "$CLAUDE_AT.away" "$CLAUDE_AT"
   ```

   **Expect:** *Found* or *Not yet tested with this version*, with its version; a question gets a
   reply.
6. Repeat 1 to 5 for Codex, with `CODEX_AT="$(command -v codex)"` and `ARCHFORGE_CODEX`. Then
   quit Archforge and start it as usual, without the override, so the shell of
   [Before you start](#before-you-start) is free for the key search.

### F5. Linux without a secure store (Linux only)

1. Start Archforge in a session with no Secret Service running.
2. Open **Settings ▸ AI Agents**. **Expect:** *No secure store on this system*; **Save key** says
   *Can't be saved: there is no secure store to keep it in.*
3. Sign Codex in with ChatGPT. **Expect:** signed in; the card says the codex program keeps the
   sign-in in its own files; R3 step 5 finds `auth.json`.

### W1. Windows (Windows only)

Open **Settings ▸ AI Agents**. **Expect:** *Not available on this system yet* and *AI Agents work
on macOS and Linux for now. On Windows, Chat and these settings stay off; everything else in
Archforge works as usual.*

## The key search

A made-up canary key goes through every path a key can take; then every place Archforge writes is
searched for it, and for the real keys. Run it last: it replaces the saved keys, which you put back
at the end.

### K1. Make the canaries and the markers

```sh
CANARY_CLAUDE="sk-ant-api03-AFCANARY$(openssl rand -hex 12)"
CANARY_CODEX="sk-proj-AFCANARY$(openssl rand -hex 12)"
MARKER="AFMARKER$(openssl rand -hex 6)"           # a class name, added through Claude
MARKER_CODEX_KEY="AFQKEY$(openssl rand -hex 6)"   # one question to Codex on an API key
MARKER_CODEX_GPT="AFQGPT$(openssl rand -hex 6)"   # one question to Codex signed in with ChatGPT
MARKER_DELETED="AFQDEL$(openssl rand -hex 6)"     # one question in the conversation K3 deletes
printf '%s\n%s\n' "$CANARY_CLAUDE" "$CANARY_CODEX" > "$WORK/canaries"
printf '%s\n' "$MARKER" > "$WORK/marker"
printf '%s\n' "$MARKER_DELETED" > "$WORK/marker-deleted"
printf '%s\n' "$CANARY_CLAUDE" "$CANARY_CODEX" "$MARKER" "$MARKER_CODEX_KEY" "$MARKER_CODEX_GPT" "$MARKER_DELETED"   # to paste
```

The markers are words that must be found. `$MARKER` is a class name: it proves the project search
finds what was written. Each Codex marker goes in one question of one Codex route and nowhere
else, never in the model, so a match tells which route's question was kept. `$MARKER_DELETED`
goes in the one conversation that K3 deletes; the conversations that hold the other markers are
never deleted, so K4 finds their copies.

### K2. The real keys, and the programs while a turn runs

With the real keys saved (as after R1 and R2), put them in a file without showing them:

```sh
( umask 077
  case "$(uname -s)" in
    Darwin) # the system asks to allow access to each entry
      { security find-generic-password -s Archforge -a anthropic-api-key -w
        security find-generic-password -s Archforge -a openai-api-key -w; } 2>/dev/null ;;
    *) { secret-tool lookup service Archforge username anthropic-api-key; echo
         secret-tool lookup service Archforge username openai-api-key; echo; } ;;
  esac | grep . > "$WORK/keys" )
wc -l < "$WORK/keys"    # how many keys were read (2 expected), never what they are
[ -s "$WORK/keys" ] || echo "NO KEYS READ: the searches for the real keys would prove nothing"
```

1. Make Claude the default agent, and start a new conversation (**New conversation**, +). Turn
   **Allow changes to the model** on and ask: *Add a class named* and paste `$MARKER`'s value.
   **Expect:** the class appears. Save (⌘S).
2. Ask a long question. While the reply runs, list the processes whose command line or
   environment holds a key (process id and program only):

   ```sh
   if [ "$(uname -s)" = Darwin ]; then
     ps -A -ww -E -o pid=,comm=,command= | grep -F -f "$WORK/keys" | awk '{print $1, $2}'
   else
     for p in /proc/[0-9]*; do
       if grep -q -a -F -f "$WORK/keys" "$p/environ" "$p/cmdline" 2>/dev/null; then echo "${p#/proc/} $(cat "$p/comm")"; fi
     done
   fi
   ```

   **Expect:** one line: the Claude program (its key is in its environment, by design). An npm
   install runs it through Node.js, so it may show as `node`; check its id with `ps -p <id> -o
   command= | cut -c1-120` (the command line holds no key). Never `archforge-cli`, `archforge` or
   any other.
3. Repeat step 2 with Codex on **API key**. **Expect:** no line: the key goes over the program's
   input.
4. With Codex the default agent and still on **API key**, ask in a new conversation: *Reply with
   this one word and read nothing:* and paste `$MARKER_CODEX_KEY`'s value. Then switch the card to
   **Sign in with ChatGPT** (sign in as in R3 if it says *Not signed in*), and ask the same in
   another new conversation with `$MARKER_CODEX_GPT`'s value. **Expect:** a reply each time,
   repeating the word. Each Codex route has now sent one question with a marker of its own, for
   K4 step 8.
5. Make Claude the default agent again, start a new conversation and ask: *Reply with this one
   word and read nothing:* and paste `$MARKER_DELETED`'s value. **Expect:** a reply. This is the
   conversation K3 step 5 deletes; its title starts with the question.

### K3. Send the canaries everywhere they can go

1. Save `$CANARY_CLAUDE` as Claude's key and `$CANARY_CODEX` as Codex's (Codex on **API key**).
2. Choose **Send test message** with each as the default agent. **Expect:** each fails, the
   provider refusing the key.
3. Ask a question with each in an existing conversation of that agent (from **Conversations**).
   **Expect:** *Key refused*.
4. With Claude, ask a question that holds the canary: *Is … a key?*, pasting `$CANARY_CLAUDE`'s
   value. **Expect:** *This question holds your saved Claude API key, so it was neither sent nor
   saved.*, the question left in the box. Clear the box.
5. Open **Conversations** and delete the conversation of K2 step 5, the one titled with
   `$MARKER_DELETED`'s question, and no other. Let its note go (about eight seconds). The
   conversations of K2 steps 1 and 4 stay, for K4.
6. Select the diagram, open **Generate…** (⇧⌘G) and choose **Copy request**. Keep what it
   copied (it holds no key, only the request), before Archforge quits and the clipboard may go
   with it:

   ```sh
   if [ "$(uname -s)" = Darwin ]; then pbpaste; else wl-paste 2>/dev/null || xclip -o -selection clipboard; fi > "$WORK/request.txt"
   ```

7. Quit Archforge, so its log, its preferences and its folders are settled.

### K4. Search every place Archforge writes

Each command prints the files that hold a canary, or *none found*. Run each with `$WORK/canaries`,
then again with `$WORK/keys` (the real keys, from K2), then with `$WORK/marker`.

```sh
search() {
  grep -r -a -l -F -f "$1" -- "${@:2}"
  case $? in 0) echo "FOUND: the files above" ;; 1) echo "none found" ;; *) echo "SEARCH FAILED: see the error above" ;; esac
}
```

*SEARCH FAILED* is neither a pass nor a fail: fix the path and run it again.

1. **Logs:**

   ```sh
   search "$WORK/canaries" "$SETTINGS/logs"
   ```

2. **Preferences:**

   ```sh
   search "$WORK/canaries" "$SETTINGS/prefs.json"
   ```

3. **The private runtime homes** (every conversation's `home`, `config`, `work` and `tmp`, Claude's
   transcripts, Codex's copies, and Codex's own `codex-home`):

   ```sh
   search "$WORK/canaries" "$SETTINGS/agents"
   ```

4. **Everything else in the settings folder** (token files, the Dolt home):

   ```sh
   search "$WORK/canaries" "$SETTINGS"
   ```

5. **The project and its saved history.** Search a copy, with Archforge quit. Dolt keeps its data
   compressed, so read every table and every saved version of it through Dolt:

   ```sh
   cp -R "$PROJECT" "$WORK/project-copy.archforge"
   export DOLT_ROOT_PATH="$WORK/dolt-home"; mkdir -p "$DOLT_ROOT_PATH"   # leaves your ~/.dolt alone
   dump() {   # every table and every saved version of it; stops at the first query that fails
     ( cd "$WORK/project-copy.archforge/project" || exit 1
       dolt sql -r csv -q "SHOW TABLES" < /dev/null > "$WORK/tables.csv" || { echo "failed: SHOW TABLES" >&2; exit 1; }
       tail -n +2 "$WORK/tables.csv" > "$WORK/tables"
       [ -s "$WORK/tables" ] || { echo "failed: no tables read" >&2; exit 1; }
       : > "$WORK/project-rows.csv"; : > "$WORK/project-history.csv"
       while read -r t; do
         dolt sql -r csv -q "SELECT * FROM \`$t\`" < /dev/null >> "$WORK/project-rows.csv" || { echo "failed: $t" >&2; exit 1; }
         dolt sql -r csv -q "SELECT * FROM \`dolt_history_$t\`" < /dev/null >> "$WORK/project-history.csv" || { echo "failed: dolt_history_$t" >&2; exit 1; }
       done < "$WORK/tables"
       dolt log < /dev/null > "$WORK/project-log.txt" || { echo "failed: dolt log" >&2; exit 1; } )
   }
   if dump; then
     search "$WORK/canaries" "$WORK/project-rows.csv" "$WORK/project-history.csv" "$WORK/project-log.txt" "$WORK/project-copy.archforge"
   else
     echo "DUMP FAILED: the project was not searched; fix the error above and run this step again"
   fi
   ```

   `project-rows.csv` holds the tables as they are now, `project-history.csv` every saved version.
   Run the later searches of this step's files (with `$WORK/keys`, `$WORK/marker` and, below,
   `$WORK/marker-deleted`) only after a dump that did not fail. *DUMP FAILED* is neither a pass
   nor a fail.

   The deleted conversation, which the manual says stays in the saved history:

   ```sh
   search "$WORK/marker-deleted" "$WORK/project-rows.csv"      # expect: none found
   search "$WORK/marker-deleted" "$WORK/project-history.csv"   # expect: found
   search "$WORK/marker-deleted" "$SETTINGS/agents"            # expect: none found (its folder went)
   ```

   If Dolt says a server holds the database, remove `.dolt/sql-server.lock` from the copy only.
   The last place also searches the raw files, as a quick extra look.

6. **Exports:** the Generate request kept in K3 step 6 (the copy of the project you would send
   someone is step 5's copy):

   ```sh
   search "$WORK/canaries" "$WORK/request.txt"
   ```

7. **The system's temporary folder** (the programs get their own; nothing should be here):

   ```sh
   search "$WORK/canaries" "${TMPDIR:-/tmp}"
   ```

8. **Codex's own home**, for the question each Codex route sent in K2 step 4, one route at a
   time:

   ```sh
   for m in "$MARKER_CODEX_KEY" "$MARKER_CODEX_GPT"; do
     printf '%s\n' "$m" > "$WORK/one-marker"
     echo "== $m in codex-home"
     search "$WORK/one-marker" "$SETTINGS/agents/codex-home"
     grep -r -a -o -E ".{0,80}$m.{0,40}" "$SETTINGS/agents/codex-home" > "$WORK/matches-$m.txt"
     echo "$(wc -l < "$WORK/matches-$m.txt") matches, every one in $WORK/matches-$m.txt"
     echo "== $m in the conversations' own folders"
     search "$WORK/one-marker" "$SETTINGS/agents"
   done
   ```

   For each route, read its `matches-…` file to the end, every line, before recording it: each
   line is one match, its file first, then up to 80 characters before the marker and 40 after. A context line that
   reads *Reply with this one word and read nothing:* before the marker is the question kept by
   Codex: record its file. A match with other text around it (the reply, a log line) is recorded
   as what it is, with its file. No match means that route's question is not in `codex-home`. The
   second search should also list each conversation's own copy, in its `conversation-…` folder.
   The context lines should hold only the question and the reply. If one shows anything like a
   key, record a fail without copying it.

**Expect** for `$WORK/canaries` and `$WORK/keys`: *none found* everywhere. The secure
store itself is the only place that holds a key.

**Expect** for `$WORK/marker` (`$MARKER`, the class of K2 step 1): found in `project-rows.csv`
and `project-history.csv` (the class and the question); this proves the search reads the project. Elsewhere, record where
it shows: Claude's own transcript in that conversation's `conversation-…` folder is expected.

### K5. Put things back

Run this only after every K4 search, step 8 included: it removes `$WORK`, with the markers and the
key files the searches read.

1. Start Archforge. Replace both keys with the real ones, or **Remove** them.
2. Remove the pass's files, which hold the real keys and the project's rows:

   ```sh
   rm -rf "$WORK"; unset WORK CANARY_CLAUDE CANARY_CODEX MARKER MARKER_CODEX_KEY MARKER_CODEX_GPT MARKER_DELETED
   ```

## Results

| Step | What | Result | Notes |
|---|---|---|---|
| P1 | A project to talk about | | |
| R1 | Claude with an API key | | |
| R2 | Codex with an API key | | |
| R3 | Codex with ChatGPT, in the browser | | |
| R4 | Codex with ChatGPT, with a device code | | |
| R5 | Settings send nothing | | |
| F1 | A revoked key (Claude, Codex) | | |
| F2 | An expired sign-in | | |
| F3 | Network off (Claude, Codex) | | |
| F4 | A program removed (claude, codex) | | |
| F5 | Linux without a secure store | | |
| W1 | Windows | | |
| K2 | The real keys: command lines and environments | | |
| K3 | The canaries sent | | |
| K4.1 | Logs | | |
| K4.2 | Preferences | | |
| K4.3 | Private runtime homes | | |
| K4.4 | The settings folder | | |
| K4.5 | The project and its saved history | | |
| K4.6 | Exports | | |
| K4.7 | The temporary folder | | |
| K4.8 | Codex's own home (one marker per route) | | |
| K5 | Put back | | |

Versions: Archforge …, claude …, codex …, system …, date ….

## Also to answer while running it

The agent runtimes' notes list what only a real round trip can show ([To verify with a real round
trip](../agent-runtimes/README.md#to-verify-with-a-real-round-trip-needs-the-owners-ok)).
Note what the pass shows for these:

- R1 step 11: Claude resumes a conversation after **Allow changes** was switched.
- K2: the bridge (`archforge-cli`) never holds Claude's key in its environment.
- R2 step 4: Codex with an API key writes no `auth.json`.
- K4 step 8: whether Codex's own databases in `codex-home` hold question text, for each route
  (API key, ChatGPT) on its own, from the marker each sent in K2 step 4.
