# Chat acceptance pass for 0.13.0 (AR-207)

The script for the acceptance pass of Chat: real round trips with Claude and Codex, failure
injection, and a search for keys in every place Archforge writes. It needs the owner's own keys and
accounts, and spends a little API credit and plan use. Nothing in it has been run yet; the manual's
[Setting up AI agents for Chat](../manual/07-agents.md#setting-up-ai-agents-for-chat) and
[Chat](../manual/07-agents.md#chat) say what the steps should look like.

Run it on macOS, and on Linux where a step says so. Windows has no Chat yet: check only W1.

## How to record

1. Copy this file to the ignored `plan/` folder under a name no earlier record has; the copy
   refuses to replace one:

   ```sh
   mkdir -p plan/acceptance && REC="plan/acceptance/chat-0.13.0-$(date +%Y%m%d-%H%M%S).md"
   if [ -e "$REC" ]; then echo "STOP: $REC exists; run this again"; else cp docs/acceptance/chat-0.13.0.md "$REC" && echo "record: $REC"; fi
   ```

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
WORK="$(mktemp -d "$HOME/archforge-acceptance.XXXXXX")" && chmod 700 "$WORK"   # removed at the end
[ -n "$WORK" ] && [ -d "$WORK" ] && echo "work folder ready" || echo "STOP: no work folder made"
PROJECT="$HOME/Documents/Archforge/Chat acceptance.archforge"                # made in step P1
case "$(uname -s)" in
  Darwin) SETTINGS="$HOME/Library/Application Support/Archforge" ;;
  *)      SETTINGS="${XDG_CONFIG_HOME:-$HOME/.config}/archforge" ;;
esac
echo "$SETTINGS"
wait_quit() {   # after you quit Archforge: waits up to 30 s for its process to end
  for i in $(seq 1 30); do pgrep -x archforge > /dev/null || { echo "Archforge has quit"; return 0; }; sleep 1; done
  echo "STILL RUNNING after 30 s: wait, then run wait_quit again"; return 1
}
ours() {   # Archforge's agent programs and their bridges: their command lines name its settings folder
  ps -A -ww -o pid=,command= | grep -F -- "$SETTINGS/" | grep -v -F -- 'grep -F' | cut -c1-160
}
```

Quitting is not instant: Archforge first ends the agent programs and the project's database
server. Wherever a step quits Archforge, run `wait_quit` and go on only after *Archforge has
quit*.

`ours` lists only the programs Archforge started for Chat, never your own `claude` or `codex`
sessions: a `claude` (or `node …claude…`) line whose `--mcp-config` lies in the settings folder, a
`codex app-server` line, and an `archforge-cli mcp` line for each bridge. A `dolt` line, if any, is
the project's database server. Their command lines hold no key.

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
   class diagram with three classes or more. Leave at least two of them without a description:
   R1 step 7 asks the agent to describe them, and step 8 undoes it.
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

Before step 1, make sure `codex-home` holds no ChatGPT sign-in from before the pass, or step 4
proves nothing. Quit Archforge, run `wait_quit`, then:

```sh
H="$SETTINGS/agents/codex-home"; KEPT=""
if [ ! -e "$H/auth.json" ]; then echo "no earlier auth.json"
elif KEPT="$(mktemp -d "$HOME/archforge-earlier-sign-in.XXXXXX")" && chmod 700 "$KEPT" \
     && mv "$H/auth.json" "$KEPT/auth.json"; then echo "KEPT an earlier sign-in in $KEPT"
else echo "KEEP FAILED: stop here and fix the error above (KEPT='$KEPT')"; fi
```

**Expect:** *no earlier auth.json*, or *KEPT* with its folder: record which, and the folder. A
kept file is an earlier ChatGPT sign-in, in a private folder of its own outside `$WORK`, so
nothing in the pass removes it; K5 step 2 says how to put it back. Start Archforge again.

1. On the Codex card choose **API key** under **Sign-in**, paste the OpenAI key and choose
   **Save key**. **Expect:** *API key saved · ends in* its last four characters.
2. Set **Default agent** to Codex. Choose **Send test message**. **Expect:** *Codex answered the
   test message just now*.
3. In the Chat panel choose **New conversation** (+). Repeat R1 steps 6 to 11 with Codex; the badge
   reads *Codex*.
4. Quit Archforge, then check that Codex wrote no key file, in a folder that is there. Codex is
   not signed in with ChatGPT yet (R3 comes next), so any `auth.json` here came from the API key:

   ```sh
   wait_quit
   H="$SETTINGS/agents/codex-home"
   if [ ! -d "$H" ]; then echo "NO FOLDER $H: check SETTINGS"
   elif [ -e "$H/auth.json" ]; then echo "auth.json EXISTS"; else echo "no auth.json in $H"; fi
   ```

   **Expect:** *no auth.json in* the folder. *NO FOLDER* proves nothing: fix `SETTINGS` first.

### R3. Codex with ChatGPT, in the browser

1. Start Archforge. On the Codex card choose **Sign in with ChatGPT**, then **Sign in…**.
   **Expect:** the browser opens the sign-in page; the card says *Waiting for you to finish in the
   browser…*.
2. Sign in in the browser. **Expect:** the card says *Signed in as* your email and plan.
3. Choose **Send test message**. **Expect:** *Codex answered the test message just now*.
4. Ask one question in a new conversation. **Expect:** a reply, with *Read …* rows.
5. Check where the sign-in is kept:

   ```sh
   H="$SETTINGS/agents/codex-home"
   if [ ! -d "$H" ]; then echo "NO FOLDER $H: check SETTINGS"
   elif [ -e "$H/auth.json" ]; then echo "auth.json EXISTS"; ls -l "$H/auth.json"; else echo "no auth.json in $H"; fi
   ls -ld "$H"
   ```

   **Expect:** *auth.json EXISTS*, on macOS and Linux alike: the codex program keeps the ChatGPT
   sign-in in its own files in `codex-home`, never in the Keychain or the Secret Service. `ls -ld`
   shows `drwx------` (on macOS perhaps with a trailing `@` or `+`) and you as the owner: only you
   can open the folder. *NO FOLDER* proves nothing: fix `SETTINGS` first. Record the line
   `ls -l` prints for `auth.json` too; its mode is the codex program's own choice.
6. Choose **Sign out**. **Expect:** *Not signed in*, with **Sign in…**. Run step 5's block again
   and record what it says: what sign-out removes from `codex-home` is the codex program's own
   doing. *no auth.json in* the folder makes R4's *auth.json EXISTS* proof that R4 wrote it.

### R4. Codex with ChatGPT, with a device code

1. On the Codex card choose **No browser on this computer? Sign in with a device code.**
   **Expect:** a code with its copy icon, *It runs out in 15 minutes.*, and under the code the
   sign-in page's address, starting with `https://auth.openai.com/`, with its own copy icon
   (**Copy the address** when you point at it); then **Open sign-in page** and **Cancel**.
2. Choose the address's copy icon and paste the address somewhere you can read it, or read it off
   the card. **Expect:** the whole address, the same as the card shows; pointing at a cut-off
   address shows it whole. Record its path (it holds no secret) in the notes.
3. On the second device, open that address (type it, or send it there) and enter the code.
   **Expect:** *Signed in as* your email and plan on the card.
4. Choose **Send test message**, then ask one question. **Expect:** as in R3 steps 3 and 4, and R3
   step 5's check says *auth.json EXISTS*: the sign-in is kept in `codex-home` as in R3. If R3
   step 6 still found `auth.json` after signing out, compare the `ls -l` line's time with R3's.
5. Start another device-code sign-in after signing out, and choose **Cancel**. **Expect:** the
   card goes back to *Not signed in*; no error.

An address off `https://auth.openai.com/` cannot be made to come from the real codex program, so
the pass does not cover it; the app's test
`the_device_code_card_shows_the_sign_in_address_to_select_and_copy` does (*Not an address
Archforge opens*, nothing to copy).

Sign in again (R3 or R4) before the failures.

### R5. Check connection works offline

This shows Check connection needs no network; it does not by itself prove that nothing is sent.

1. Turn the network off (Wi-Fi off, cable out).
2. Open **Settings ▸ AI Agents** and choose **Check connection**. **Expect:** the badges sum up
   both agents as before, with no error about the network.
3. Turn the network back on.

## Failure injection

### F1. A revoked key

1. Make a second Anthropic key. Save it on the Claude card (**Replace**). Set **Default agent**
   to Claude (R2 left Codex there): the test message goes to the default agent only. Choose
   **Send test message**. **Expect:** *Claude answered the test message*.
2. Revoke that key in the Anthropic account.
3. Choose **Send test message**. **Expect:** *The test message to Claude failed: The provider
   refused the key (…)*, and the key field says *The provider refused this key when the test
   message was sent (…). Replace it with a valid key, or remove it.*
4. Ask a question in the Claude conversation of R1 (open it from **Conversations**). **Expect:**
   the card *Key refused* with **Open Settings**. A new conversation would show *No agent set up
   yet* instead, as the refused key counts as not ready.
5. Choose **Open Settings**, **Replace** the key with the valid one, ask again. **Expect:** a reply.
6. Repeat 1 to 5 for Codex with an OpenAI key (Codex on **API key**), with **Default agent** set
   to Codex and R2's Codex conversation. **Expect:** the same, for Codex.

### F2. An expired sign-in

1. F1 left the Codex card on **API key**. Switch it to **Sign in with ChatGPT**. **Expect:**
   still *Signed in as* your email, as before F1 (the API key's runs use the ephemeral store and
   share `codex-home` with the sign-in); if it says *Not signed in*, record that, then sign in as
   in R3. Set **Default agent** to Codex, start a new conversation
   (**New conversation**, +) and ask one question. **Expect:** a reply: this conversation runs on
   the ChatGPT sign-in.
2. End the sign-in from the ChatGPT account: use its security settings to sign out of all
   devices. Then ask again in the conversation of step 1. **Expect:** the card *Sign-in expired*
   with **Open Settings**. Signing out of all devices may not end a Codex sign-in: if the
   question gets a reply, the sign-in is still valid and expiry was not established.
   Record F2 as *skipped: expiry not established*, with the way you tried, and stop F2 here.
3. Choose **Open Settings**. **Expect:** without a restart, the Codex card says *The ChatGPT
   sign-in for* your email *has expired.*, with **Sign in again…**; the conversation's card made
   Archforge read the sign-in again.
4. Choose **Sign in again…**, sign in, then ask again. **Expect:** a reply.

No other way to end a Codex 0.166.0 sign-in from outside is known here. If the account offers
none, leave Archforge signed in for a few days and try step 2 then. F2 passes only with the card
*Sign-in expired*. Whenever expiry cannot be shown, record F2 as *skipped: expiry not
established*: when the sign-in still answers, and also when the question fails with another card
(*Key refused*, *No answer* or any other), as those have other causes. Note that card, its text and
the time apart from the result, so the log can be read for it.

### F3. Network off

1. Turn the network off. Ask a question in R1's Claude conversation (from **Conversations**).
   **Expect:** while Claude retries (about three minutes), an amber line *No answer from Claude's
   provider yet. Trying again, attempt n of 10…* (or without *of 10*, or without the attempt, as
   far as the program says them), with **Stop** still there. Then *Connection lost*: *The network
   went away before Claude answered. Your message is back in the box below, ready to send again.*
   The question is in the box.
2. Turn the network on. Choose **Send again**. **Expect:** the question is sent once; the
   conversation shows it once.
3. Turn the network off, ask another question, and choose **Stop** while the retry line shows.
   **Expect:** the line goes and the reply ends at once, marked as stopped. Turn the network on.
4. Ask a long question. While the reply streams, turn the network off. **Expect:** *Connection
   lost*, *The network went away while Claude was answering.*, the reply so far kept. Turn the
   network on and choose **Send again**. **Expect:** a reply; the question shows once.
5. Repeat 1 to 4 in R2's Codex conversation. **Expect:** the same cards, naming Codex.
6. With the network off, choose **Send test message**. **Expect:** *The test message to … failed:*
   with a reason; nothing else changes. Turn the network back on before going on.

### F4. A program removed

Archforge looks for the programs in several folders, so moving one copy away may leave another.
`ARCHFORGE_CLAUDE` (and `ARCHFORGE_CODEX`) make Archforge look in one place alone. Start Archforge
from the shell of [Before you start](#before-you-start) for this step, in the background, so the
same shell can move the program; `APP` is its program:

```sh
APP=/Applications/Archforge.app/Contents/MacOS/archforge     # or target/debug/archforge
CLAUDE_AT="$(command -v claude)"; echo "$CLAUDE_AT"
```

1. Quit Archforge, then start it with a program that is not there (a path inside `$WORK`,
   which holds no such file):

   ```sh
   wait_quit && ARCHFORGE_CLAUDE="$WORK/no-claude-here" "$APP" > "$WORK/app-f4.log" 2>&1 &
   ```

2. Open **Settings ▸ AI Agents**. **Expect:** Claude's badge says *Not found*; the card shows the
   install command, **Look again** and **Installation guide**.
3. With Claude the default agent, open the Chat panel. **Expect:** *No agent set up yet*, with
   **Configure agent**.
4. Quit, and start Archforge with the real program as the only place to look:

   ```sh
   wait_quit && ARCHFORGE_CLAUDE="$CLAUDE_AT" "$APP" > "$WORK/app-f4.log" 2>&1 &
   ```

   Ask in a Claude conversation. **Expect:** a reply. Then, in the same shell, move the program
   away (`sudo mv` where its folder needs it), and ask once more in a new conversation (**New
   conversation**, +):

   ```sh
   if [ -e "$CLAUDE_AT.away" ]; then echo "STOP: $CLAUDE_AT.away exists already"
   elif mv -n "$CLAUDE_AT" "$CLAUDE_AT.away" && [ ! -e "$CLAUDE_AT" ]; then echo "moved away"
   else echo "MOVE FAILED: the program is still there; this step proves nothing"; fi
   ```

   **Expect:** the card *Program not found* with **Open Settings**. Record what shows if not.
5. Put it back and choose **Look again** in Settings:

   ```sh
   if [ -e "$CLAUDE_AT" ]; then echo "STOP: $CLAUDE_AT exists"
   elif mv -n "$CLAUDE_AT.away" "$CLAUDE_AT" && [ -e "$CLAUDE_AT" ]; then echo "put back"
   else echo "MOVE FAILED: put $CLAUDE_AT.away back by hand"; fi
   ```

   **Expect:** *Found* or *Not yet tested with this version*, with its version; a question gets a
   reply.
6. Repeat 1 to 5 for Codex, with `CODEX_AT="$(command -v codex)"`, `ARCHFORGE_CODEX` and
   `$WORK/no-codex-here`. Then quit Archforge, run `wait_quit`, and start it as usual, without
   the override.

### F5. Linux without a secure store (Linux only)

1. Start Archforge in a session with no Secret Service running.
2. Open **Settings ▸ AI Agents**. **Expect:** *No secure store on this system*; **Save key** says
   *Can't be saved: there is no secure store to keep it in.*
3. Sign Codex in with ChatGPT. **Expect:** signed in; the card says the codex program keeps the
   sign-in in its own files, so it works without the secure store; R3 step 5 finds `auth.json`,
   as on every system.

### F6. Let Chat use the model turned off

1. With Claude the default agent, open a started conversation (R1's, from **Conversations**) and
   ask a long question. While the reply runs, run `ours`. **Expect:** a `claude` line and an
   `archforge-cli mcp` line.
2. Open **Settings ▸ AI Agents**, turn **Let Chat use the model** off, and close Settings.
   **Expect:** the reply so far stays, marked as stopped; the status bar says *Agent access off*.
   Check that the programs ended:

   ```sh
   agents() { ours | grep -v -E '(^| |/)dolt( |$)'; }
   for i in $(seq 1 15); do [ -z "$(agents)" ] && break; sleep 1; done
   agents || echo "no agent program runs"
   ```

   **Expect:** *no agent program runs*.
3. Type a question and press Enter. **Expect:** the question stays in the box, and the panel says
   *Let Chat use the model is off, so nothing is sent. Turn it on in Settings ▸ AI Agents.* Run
   `agents || echo "no agent program runs"` again. **Expect:** *no agent program runs*: nothing
   was started, so nothing was sent.
4. Open another conversation from **Conversations** and press Enter on a question there.
   **Expect:** the same line, and still *no agent program runs*.
5. Start a new conversation (**New conversation**, +). **Expect:** *No agent set up yet*, with
   **Configure agent**.
6. Turn **Let Chat use the model** on again and go back to the conversation of step 4.
   **Expect:** the line has gone. Press Enter. **Expect:** the question is sent once and answered;
   `ours` lists the program again.
7. The *Not sent* card shows only when the switch goes off while a question is still being kept,
   before it is sent: a moment too short to reach by hand, as the switch is in Settings. Record
   *not reached*. The app's tests cover it:
   `a_question_being_kept_when_model_use_goes_off_is_not_sent_and_goes_back_to_the_box` (the card,
   the question back in the box, and sent once the switch is on) and
   `a_question_not_sent_while_the_box_holds_other_text_can_be_sent_again` (**Send again**).

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
ASK="Reply with this one word and read nothing:"   # the questions of K2 steps 4 and 5
printf '%s\n%s\n' "$CANARY_CLAUDE" "$CANARY_CODEX" > "$WORK/canaries"
printf '%s\n' "$MARKER" > "$WORK/marker"
printf '%s\n' "$CANARY_CLAUDE" "$CANARY_CODEX" "$MARKER"   # to paste in K2 step 1 and K3
printf '%s %s\n' "$ASK" "$MARKER_CODEX_KEY" "$ASK" "$MARKER_CODEX_GPT" "$ASK" "$MARKER_DELETED"   # the three questions, to paste whole
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
if [ "$(wc -l < "$WORK/keys")" -eq 2 ]; then KEYS_OK=1; echo "both keys read"
else KEYS_OK=0; echo "STOP: $(wc -l < "$WORK/keys") of 2 keys read; fix the store or the keys and run this again"; false; fi
```

1. Make Claude the default agent, and start a new conversation (**New conversation**, +). Turn
   **Allow changes to the model** on and ask: *Add a class named* and paste `$MARKER`'s value.
   **Expect:** the class appears. Save (⌘S).
2. Ask a long question. While the reply runs, first check that the programs the search is about
   are running, then list the processes whose command line or environment holds a key (process id
   and program only):

   ```sh
   ours
   if [ "${KEYS_OK:-0}" != 1 ]; then
     echo "SKIPPED: K2 did not read both real keys; fix that and read them again first"
   elif [ "$(uname -s)" = Darwin ]; then
     ps -A -ww -E -o pid=,comm=,command= | grep -F -f "$WORK/keys" | awk '{print $1, $2}'
   else
     for p in /proc/[0-9]*; do
       if grep -q -a -F -f "$WORK/keys" "$p/environ" "$p/cmdline" 2>/dev/null; then echo "${p#/proc/} $(cat "$p/comm")"; fi
     done
   fi
   ```

   *SKIPPED* is neither a pass nor a fail: this step and step 3 count only after both keys were
   read.

   **Expect:** `ours` lists a `claude` line and an `archforge-cli mcp` line (the bridge), else the
   turn ended too soon: ask again. Then one key line, for that same `claude` process id (its key
   is in its environment, by design); an npm install runs it through Node.js, so its program may
   show as `node`. Never an `archforge-cli` or `archforge` id. A key line for a process `ours` does
   not list is a program of your own holding the same key: note it, it is outside the pass.
3. Set **Default agent** to Codex with its card on **API key**, start a new conversation
   (**New conversation**, +; the Claude conversation of step 2 stays Claude's) and repeat step 2
   there. **Expect:** `ours` lists a `codex app-server` line and an `archforge-cli mcp` line, then
   no key line for either: the key goes over the program's input. Step 2's line for Claude shows
   that the search finds a key where one is.
4. With Codex the default agent and still on **API key**, start a new conversation and paste the
   first question K1 printed, the one ending in `$MARKER_CODEX_KEY`'s value, whole and unchanged.
   Then switch the card to **Sign in with ChatGPT** (sign in as in R3 if it says *Not signed
   in*, or with **Sign in again…** if it says the sign-in *has expired*), start another new
   conversation and paste the second, ending in `$MARKER_CODEX_GPT`'s value. **Expect:** a reply
   each time, repeating the word. A *Sign-in expired* card instead of the second reply means the
   sign-in ended since F2: sign in again from **Open Settings**, then paste the question again in
   another new conversation. Each Codex route has now sent one question with a marker of its
   own, for K4 step 8.
5. Make Claude the default agent again, start a new conversation and paste the third question K1
   printed, ending in `$MARKER_DELETED`'s value. **Expect:** a reply. This is the
   conversation K3 step 5 deletes; its title starts with the question.

### K3. Send the canaries everywhere they can go

1. Save `$CANARY_CLAUDE` as Claude's key and `$CANARY_CODEX` as Codex's (Codex on **API key**).
   Then quit Archforge, run `wait_quit`, and start it again, so that no program started with a
   real key is still running: every program from here on starts with a canary.
2. Choose **Send test message** with each as the default agent. **Expect:** each fails, the
   provider refusing the key.
3. Ask a question with each in an existing conversation of that agent (from **Conversations**).
   **Expect:** *Key refused*.
4. With Claude, ask a question that holds the canary: *Is … a key?*, pasting `$CANARY_CLAUDE`'s
   value. **Expect:** *This question holds your saved Claude API key, so it was neither sent nor
   saved.*, the question left in the box. Clear the box.
5. Before deleting, find the private folder of the conversation of K2 step 5 (the only one that
   holds `$MARKER_DELETED`) and keep its path:

   ```sh
   DELETED_DIR=""
   for d in "$SETTINGS"/agents/conversation-*/; do
     if grep -r -q -a -F "$MARKER_DELETED" "$d"; then DELETED_DIR="${d%/}"; echo "$DELETED_DIR"; fi
   done
   [ -n "$DELETED_DIR" ] && [ -d "$DELETED_DIR" ] && echo "found, and it is there" || echo "NOT FOUND: stop here"
   ```

   **Expect:** one folder path, then *found, and it is there*. Then open **Conversations** and
   delete that conversation, the one titled with `$MARKER_DELETED`'s question, and no other. Let
   its note go (about eight seconds). The folder is removed after that, while the program ends,
   so check the folder itself, waiting up to 30 seconds:

   ```sh
   if [ -n "$DELETED_DIR" ]; then
     for i in $(seq 1 30); do [ -e "$DELETED_DIR" ] || break; sleep 1; done
     if [ -e "$DELETED_DIR" ]; then echo "STILL THERE after 30 s: $DELETED_DIR"; else echo "gone: $DELETED_DIR"; fi
   fi
   ```

   **Expect:** *gone:* and the path. Only *STILL THERE after 30 s* is a fail; the log then says
   why it was kept (`grep -h 'is kept' "$SETTINGS"/logs/archforge.log*`), which goes in the
   notes. The conversations of K2 steps 1 and 4 stay, for K4.
6. Select the diagram, open **Generate…** (⇧⌘G) and choose **Copy request**. Keep what it
   copied (it holds no key, only the request), before Archforge quits and the clipboard may go
   with it:

   ```sh
   if [ "$(uname -s)" = Darwin ]; then pbpaste; else wl-paste 2>/dev/null || xclip -o -selection clipboard; fi > "$WORK/request.txt"
   ```

7. Quit Archforge, so its log, its preferences and its folders are settled, and run `wait_quit`.
   Go on to K4 only after *Archforge has quit*.

### K4. Search every place Archforge writes

Each command prints the files that hold a canary, or *none found*. Run each with `$WORK/canaries`,
then again with `$WORK/keys` (the real keys, from K2), then with `$WORK/marker`.

```sh
search() {
  if [ "$1" = "$WORK/keys" ] && [ "${KEYS_OK:-0}" != 1 ]; then
    echo "SEARCH FAILED: K2 did not read both real keys, so this search would prove nothing"; return 2
  fi
  grep -r -a -l -F -f "$1" -- "${@:2}"
  case $? in 0) echo "FOUND: the files above" ;; 1) echo "none found" ;; *) echo "SEARCH FAILED: see the error above" ;; esac
}
```

*SEARCH FAILED* is neither a pass nor a fail: fix the path and run it again.

A *none found* proves something only where the search can find what is there. So each place
below is probed first with a word it is known to hold; the probe must say *FOUND*, or that
place's *none found* proves nothing:

```sh
probe() { printf '%s\n' "$1" > "$WORK/probe"; search "$WORK/probe" "${@:2}"; }
```

1. **Logs:**

   ```sh
   probe "Let Chat use the model is" "$SETTINGS/logs"   # logged at every start
   search "$WORK/canaries" "$SETTINGS/logs"
   ```

2. **Preferences:**

   ```sh
   probe chat_uses_model "$SETTINGS/prefs.json"         # the Let Chat use the model setting
   search "$WORK/canaries" "$SETTINGS/prefs.json"
   ```

3. **The private runtime homes** (every conversation's `home`, `config`, `work` and `tmp`, Claude's
   transcripts, Codex's copies, and Codex's own `codex-home`):

   ```sh
   probe "$MARKER" "$SETTINGS/agents"                   # Claude's transcript of K2 step 1
   search "$WORK/canaries" "$SETTINGS/agents"
   find "$SETTINGS" -name auth.json -print              # Codex's ChatGPT sign-in, where it is kept
   ```

   **Expect** from `find`: no `auth.json` anywhere but `$SETTINGS/agents/codex-home/auth.json`,
   which is there while Codex keeps a ChatGPT sign-in (R3 or R4). It holds the ChatGPT
   sign-in, not an API key: neither real key from K2 is in it.

4. **Everything else in the settings folder** (token files, the Dolt home):

   ```sh
   probe "$MARKER" "$SETTINGS"
   search "$WORK/canaries" "$SETTINGS"
   ```

5. **The project and its saved history.** Search a copy, with Archforge quit. Dolt keeps its data
   compressed, so read every table and every saved version of it through Dolt:

   ```sh
   # Every try gets a new, empty folder of its own: the copy and every file the dump writes go there.
   TRY="$(mktemp -d "$WORK/project-try.XXXXXX")" || TRY=""
   COPY=""
   if [ -n "$TRY" ] && [ -d "$TRY" ] && cp -R "$PROJECT" "$TRY/copy.archforge" && [ -d "$TRY/copy.archforge/project/.dolt" ]
   then COPY="$TRY/copy.archforge"; echo "copied to $COPY"
   else echo "COPY FAILED: stop here, fix the error above and run this step again"; fi
   dump() {   # every table and every saved version of it; stops at the first query that fails
     ( [ -n "$COPY" ] || { echo "failed: no complete copy" >&2; exit 1; }
       mkdir -p "$WORK/dolt-home" || { echo "failed: no Dolt home" >&2; exit 1; }
       export DOLT_ROOT_PATH="$WORK/dolt-home"   # in this subshell only: leaves your ~/.dolt alone
       cd "$COPY/project" || exit 1
       dolt sql -r csv -q "SHOW TABLES" < /dev/null > "$TRY/tables.csv" || { echo "failed: SHOW TABLES" >&2; exit 1; }
       tail -n +2 "$TRY/tables.csv" > "$TRY/tables"
       [ -s "$TRY/tables" ] || { echo "failed: no tables read" >&2; exit 1; }
       while read -r t; do
         dolt sql -r csv -q "SELECT * FROM \`$t\`" < /dev/null >> "$TRY/project-rows.csv" || { echo "failed: $t" >&2; exit 1; }
         dolt sql -r csv -q "SELECT * FROM \`dolt_history_$t\`" < /dev/null >> "$TRY/project-history.csv" || { echo "failed: dolt_history_$t" >&2; exit 1; }
       done < "$TRY/tables"
       dolt log < /dev/null > "$TRY/project-log.txt" || { echo "failed: dolt log" >&2; exit 1; } )
   }
   if dump; then
     search "$WORK/canaries" "$TRY/project-rows.csv" "$TRY/project-history.csv" "$TRY/project-log.txt" "$COPY"
   else
     echo "DUMP FAILED: the project was not searched; fix the error above and run this step again"
   fi
   ```

   `$TRY/project-rows.csv` holds the tables as they are now, `$TRY/project-history.csv` every
   saved version; a try that failed leaves its folder behind and is never read again. Run the later
   searches of this step's files (with `$WORK/keys` and `$WORK/marker`, the same files in `$TRY`)
   and the counts below only after a dump that did not fail. *DUMP FAILED* is neither a pass nor a
   fail.

   The probe here is `$WORK/marker`, which must be found in `$TRY/project-rows.csv`.

   The deleted conversation, which the manual says stays in the saved history. Count its rows
   directly, in the conversation list now and in the saved versions:

   ```sh
   ( export DOLT_ROOT_PATH="$WORK/dolt-home"; cd "$COPY/project" || { echo "no copy: run the dump first"; exit 1; }
     for t in chat_message dolt_history_chat_message; do
       printf '%s: ' "$t"
       if dolt sql -r csv -q "SELECT COUNT(*) FROM \`$t\` WHERE body LIKE '%$MARKER_DELETED%'" < /dev/null > "$TRY/count"
       then tail -n 1 "$TRY/count"; else echo "QUERY FAILED: proves nothing"; fi
     done )
   ```

   **Expect:** `chat_message: 0` and `dolt_history_chat_message:` a number above 0. Its private
   folder was checked directly in K3 step 5.

   If Dolt says a server holds the database, remove `.dolt/sql-server.lock` from the copy only.
   The last place also searches the raw files, as a quick extra look.

6. **Exports:** the Generate request kept in K3 step 6 (the copy of the project you would send
   someone is step 5's copy):

   ```sh
   probe "Chat acceptance" "$WORK/request.txt"          # the request names the project
   search "$WORK/canaries" "$WORK/request.txt"
   ```

7. **The system's temporary folder** (the programs get their own; nothing should be here):

   ```sh
   PROBE_FILE="$(mktemp "${TMPDIR:-/tmp}/af-probe.XXXXXX")" && printf 'AFTMPPROBE\n' > "$PROBE_FILE"
   probe AFTMPPROBE "${TMPDIR:-/tmp}"; rm -f "$PROBE_FILE"
   search "$WORK/canaries" "${TMPDIR:-/tmp}"
   ```

8. **Codex's own home**, for the question each Codex route sent in K2 step 4, one route at a
   time:

   ```sh
   for m in "$MARKER_CODEX_KEY" "$MARKER_CODEX_GPT"; do
     printf '%s %s\n' "$ASK" "$m" > "$WORK/one-question"
     echo "== the whole question with $m, in the conversations' own folders (the probe)"
     search "$WORK/one-question" "$SETTINGS/agents"
     echo "== the whole question with $m, in codex-home"
     search "$WORK/one-question" "$SETTINGS/agents/codex-home"
     printf '%s\n' "$m" > "$WORK/one-marker"
     grep -r -a -l -F -f "$WORK/one-marker" "$SETTINGS/agents/codex-home" > "$WORK/marker-files-$m.txt"
     echo "$(wc -l < "$WORK/marker-files-$m.txt") codex-home files hold $m anywhere, listed in $WORK/marker-files-$m.txt"
   done
   ```

   The question is searched whole, exactly as pasted, so no match can be cut off by another. The
   first search is the probe: it must find the whole question in that conversation's own copy, in
   its `conversation-…` folder; otherwise the second search proves nothing. For each route:
   - **FOUND** in `codex-home`: Codex keeps the question as text there. Record the files.
   - **none found** in `codex-home`, with the probe found: the question is not in `codex-home` as
     plain text. Codex could still keep it in a form the search cannot read, so record it as
     *not found as text*.
   - The `marker-files-…` list names every `codex-home` file that holds the marker in any form
     (the reply, a log line). Record them as what they are, with their files; they do not count
     as the question.

**Expect** for `$WORK/canaries` and `$WORK/keys`: *none found* everywhere, each place's probe
*FOUND*. The secure store itself is the only place that holds an API key. Codex's ChatGPT
sign-in is kept apart from them, in `codex-home/auth.json` (step 3).

**Expect** for `$WORK/marker` (`$MARKER`, the class of K2 step 1): found in `$TRY/project-rows.csv`
and `$TRY/project-history.csv` (the class and the question); this proves the search reads the
project. Elsewhere, record where it shows: Claude's own transcript in that conversation's
`conversation-…` folder is expected.

### K5. Put things back

Run this only after every K4 search, step 8 included: it removes `$WORK`, with the markers and the
key files the searches read.

1. Start Archforge. Replace both keys with the real ones, or **Remove** them.
2. If R2 kept an earlier sign-in (it printed *KEPT* and a folder): the pass signed Codex in
   again, and F2's *sign out of all devices* may have ended the earlier sign-in too. Choose, and
   record your choice: keep the pass's sign-in, or put the earlier one back. Quit Archforge, run
   `wait_quit`, then run one of the two commands. In a new shell, first set `SETTINGS` as in
   *Before you start* and `KEPT` to the folder R2 printed:

   ```sh
   # either: keep the pass's sign-in
   [ -n "$KEPT" ] && rm -rf "$KEPT" && echo "kept the pass's sign-in" || echo "NOT DONE: KEPT is empty, or see the error above"
   # or: put the earlier sign-in back
   [ -n "$KEPT" ] && mv "$KEPT/auth.json" "$SETTINGS/agents/codex-home/auth.json" && rmdir "$KEPT" \
     && echo "put the earlier sign-in back" || echo "NOT DONE: KEPT is empty, or see the error above"
   ```

   Start Archforge and open **Settings ▸ AI Agents**. K3 and step 1 left the Codex card on
   **API key**, which shows the key, not the account: choose **Sign in with ChatGPT** under
   **Sign-in**. **Expect:** *Signed in as* your email. That reads only what the codex program
   keeps; it does not ask OpenAI whether the sign-in still holds, so test it: set **Default
   agent** to Codex and choose **Send test message**. **Expect:** *Codex answered the test message
   just now*. If it fails, find out why: open *Chat acceptance*, start a new conversation with
   Codex and ask one question. The card *Sign-in expired* means F2 ended that sign-in too: choose
   **Open Settings**, then **Sign in again…**, sign in, and send the test message again. Any other
   card, or a reply, means the test message failed for another cause: record its text. Then set
   **Sign-in** and **Default agent** back to how you use them.
3. Remove the pass's files, which hold the real keys and the project's rows:

   ```sh
   rm -rf "$WORK"
   unset WORK CANARY_CLAUDE CANARY_CODEX MARKER MARKER_CODEX_KEY MARKER_CODEX_GPT MARKER_DELETED \
     DELETED_DIR ASK PROBE_FILE CLAUDE_AT CODEX_AT APP KEYS_OK TRY COPY KEPT H
   ```

## Results

| Step | What | Result | Notes |
|---|---|---|---|
| P1 | A project to talk about | | |
| R1 | Claude with an API key | | |
| R2 | Codex with an API key | | |
| R3 | Codex with ChatGPT, in the browser | | |
| R4 | Codex with ChatGPT, with a device code | | |
| R5 | Check connection works offline | | |
| F1 | A revoked key (Claude, Codex) | | |
| F2 | An expired sign-in | | |
| F3 | Network off (Claude, Codex) | | |
| F4 | A program removed (claude, codex) | | |
| F5 | Linux without a secure store | | |
| F6 | Let Chat use the model turned off | | |
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
- R3 step 6: what signing out removes from `codex-home` (whether `auth.json` is gone).
- K4 step 8: whether Codex's own databases in `codex-home` hold question text, for each route
  (API key, ChatGPT) on its own, from the marker each sent in K2 step 4.
