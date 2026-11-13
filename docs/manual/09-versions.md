# Versions

You are about to rework part of a model and want a way back to it as it is now. Or you have reached
a point worth naming, such as what the team agreed on. A *version* is that point: the whole project
as it stood at one moment, with a line that says what it is, kept in the project's history.

This chapter explains what a version is, how to save one, how to
[look back at one](#look-back-at-a-version) and come back to your work, how to
[see what changed](#see-what-changed) between two of them, how to
[restore one](#restore-a-version), and how to try something out on a
[branch](#work-on-branches) of its own.

## Current work and versions

Your **current work** is the project as it stands now: what you see and change in the window. A
**version** is a copy of the project that you saved. Going on with your current work does not change
a version you saved earlier.

| A version holds | A version does not hold |
|---|---|
| The model: its elements, their members and relationships | Your Chat conversations, which the project keeps on their own |
| The diagrams, with their layout and style | The steps you can undo |
| The skills | Open tabs, the selection, pan and zoom |
| | Your settings, such as theme and language |

One small example runs through this chapter. In the *Commerce platform* project from
[Getting started](02-getting-started.md) you add a class *Customer* and save, then give *Customer*
an attribute *email*:

| What you do | Your current work | Your versions | The status bar says |
|---|---|---|---|
| Add the class *Customer* | *Customer* | None yet | *Unsaved changes* |
| Save a version called *Added Customer* | *Customer* | *Added Customer* | *Saved* |
| Add the attribute *email* | *Customer* with *email* | *Added Customer*, which has no *email* | *Unsaved changes* |
| Press ⌘Z (Ctrl Z) | *Customer* | *Added Customer* | *Saved* |
| Press ⇧⌘Z (Ctrl Shift Z or Ctrl Y) | *Customer* with *email* | *Added Customer* | *Unsaved changes* |
| Save a version called *Added email* | *Customer* with *email* | *Added email*, *Added Customer* | *Saved* |

## Autosave, a saved version and undo

Three things look after your work, and each does a different job.

| | Autosave | A saved version | Undo and redo |
|---|---|---|---|
| What it is for | Not losing what you just did | A point you can name and come back to | Taking a step back, or forward again |
| When it happens | By itself, a moment after each change | When you save | When you press the keys, or choose **Edit ▸ Undo** or **Edit ▸ Redo** |
| Adds a row to **History** | No | Yes | No |
| After you close the project | Still there, unless you choose **Don't save** | Still there | The steps are gone |

**Autosave** is why *Customer* is kept without your saving it. Archforge writes each change to the
project a moment after you make it. If Archforge or your computer stops, the changes it had written
are there when you open the project again, and the status bar says *Unsaved changes*. A change made
just before the stop, and not yet written, is lost. If a write fails, the status bar says *Changes
could not be written to the project*, and Archforge tries again with your next change or save. If
you close the project then and choose **Save**, Archforge says why it could not write your changes
and asks whether to keep the project open or close it without writing them.

What autosave writes is not a version. When you close a project with unsaved changes, Archforge asks
what to do with them, and **Don't save** throws away what you changed since your last version. A
project with no version yet goes back to how it was created. Your conversations stay.

**Undo and redo** move your current work one step at a time. Saving doesn't end them. In the example,
you could press ⌘Z after saving *Added email* and take *email* back. The version *Added email* stays
as it was, still with *email*, and the status bar goes back to *Unsaved changes*, because your
current work now differs from your last version. Undo never removes a version and never adds one.

Undo remembers up to 500 steps, and only while the project stays open. A
[restore](#restore-a-version) and a [change of branch](#switch-branch) start it afresh.

## Save a version

Do this when you want to look over your changes and say what they are.

1. In the left rail, choose **Versions**. A number on its icon says how many things have changed.
   **Working changes** lists them in groups: model elements, then diagrams, then skills, which sit
   under **Skills** of their own. Click a change, or press Enter on it, to see how it differs from
   your last version; ↑ and ↓ move between them, across the groups.
2. Choose **Save version…**, at the right of the **Working changes** heading. The **Save version**
   dialog opens and says how many model elements, diagrams and skills changed since your last
   version, such as *3 model elements, 1 diagram and 1 skill changed since 8 Nov 14:02.*
3. In **Description**, type what this version is, such as *Added email*. A description is one line
   of at most 200 characters, and it stays in the history, where anyone given a copy of the project
   can read it. Don't put anything secret in it.
4. Choose **Save version**, or press Enter. The button stays off until you have typed something
   other than spaces.

Archforge saves exactly the changes the dialog summed up. Then the dialog closes, and:

- your version is at the top of **History**, with its description, the date and time and, if you
  have entered your name in Settings, your name;
- **Working changes** says *No changes since the last version.*, and the number on the icon goes;
- the status bar says *Saved*.

Your current work stays open, and you carry on from where you were.

**To cancel**, choose **Cancel** or press Escape. No version is made, your changes stay in your
current work, and what you typed in **Description** is not kept. Once your save is running it can't
be cancelled: **Cancel** is off and Escape does nothing until it has finished.

**If the model changes while the dialog is open**, for example because Chat is working on it, the
dialog says *The model changed while this was open. The summary is up to date again.* The summary
then counts the new changes, so read it again before you save.

## Save at once

Press ⌘S (Ctrl S), or choose **File ▸ Save**, to make a version of your changes straight away, with no
review and no description. **Save** in the question Archforge asks when you close a project does the same.
**History** lists such a version by its date and time, with *No description*.

Use it for a quick safety point, and **Save version…** for a point you will want to find again. A
version an agent saved says *Saved by an agent*.

## Your first version

A new project has no versions. **Working changes** and **History** both say so, and **Save version…**
stays off until you have added something. The status bar says *Saved* all the same, because you have
changed nothing yet. Once you have added something, **Working changes** lists it all as added, since
there is nothing earlier to compare it with, and the dialog says what changed without a *since*.

In the example, adding *Customer* turns **Save version…** on, and *Added Customer* is your first
version.

⌘S waits for a first change too: on a new project you haven't touched, it makes no version, and
**History** still says there are none.

## When there is nothing to save

With no changes since your last version, **Working changes** says *No changes since the last
version.* and **Save version…** is off. ⌘S and **File ▸ Save** make no second version, and the status bar stays on *Saved*. Text you are still typing in a skill or a
description counts as a change: ⌘S keeps it and saves it.

Now and then the status bar says *Unsaved changes* while **Working changes** lists nothing, for
example after a newer Archforge has brought an older project up to date. There is nothing to review,
so **Save version…** stays off, and ⌘S saves it.

Point at **Save version…** while it is off to see why:

| It says | Why | What to do |
|---|---|---|
| *No changes since the last version.* | There is nothing to review | Nothing, or make a change first |
| *What you're typing isn't kept yet* | Text you are typing in a skill or a description isn't part of your work yet | In a skill, pause for a moment; in a description, press Enter or click elsewhere |
| *Loading* | Archforge is still working out what changed | Wait a moment |
| *Saving…* | A save is running | Wait for it to finish |
| *The changes couldn't be compared.* | Archforge couldn't work out what changed | ⌘S still saves, without a review |
| *Back to current work to save a version* | You are [viewing an earlier version](#view-an-earlier-version) | Choose **Back to current work** |

## When a save fails

A failed save leaves your current work as it was, so you can go on working. The dialog stays open
with the reason and what you typed, and **Save version** becomes **Retry**. The status bar says
*Not saved*, with the same reason. A save made with ⌘S has no dialog, so the status bar is where you
read it, and you press ⌘S again to retry.

| The message | What happened | What to do |
|---|---|---|
| *No version was made:* and a reason | The save failed, and **History** is as it was | Choose **Retry** |
| *A version was made, even though saving reported an error:* and a reason | Your version is in **History** | Choose **Cancel**. ⌘S then sets the status bar right without a second version |
| *Archforge can't tell whether a version was made:* and a reason, or *The save stopped without an answer…* | The save may or may not have finished | Choose **Retry**: it makes no second version. If **Retry** stays off, choose **Cancel** and look in **History** |
| *Archforge could not tell whether a version was made. Look at History before trying again.* | The answer was lost | Choose **Cancel**, since **History** can't be used behind the dialog, and look there. If your version isn't there, save again |
| *The project changed after you reviewed it, so nothing was saved.* | The model moved on after the review | Read the summary again, then save |
| *This description holds one of your saved API keys, so nothing was saved.* | The history would have kept your key | Change the description, then choose **Retry** |
| *This description could not be checked against your saved API keys, so nothing was saved.* | Archforge couldn't read your keys to compare them | Choose **Retry**. If it keeps failing, choose **Cancel** and press ⌘S, which saves with no description |

## Look back at a version

You can read what each version changed, show the project in the window as it was then, and come
back to your current work. Looking changes neither your current work nor your versions.

### History and a version's details

**History**, under **Working changes** in **Versions**, lists the project's versions, newest first.
A row gives the description and, below it, the date and time and, if you had entered your name, your
name. A version saved with ⌘S gives its date and *No description*. In a long history, **Show more**
at the end lists older versions, and **With a description** above the list leaves out the versions
that have none; **All** shows them again.

On a branch other than `main`, History lists the branch's own versions and, below them, those it
shares with `main`, the versions from before the two parted. A divider, *Shared with main up to
here*, sits above the first shared version once the list has reached it. It says only what the two
have in common, not which branch this one was made from. On `main` there is no divider.

Click a version, or press Enter on it, to open its details in the centre; ↑ and ↓ move between
versions:

- its description, when it was saved and, where that is recorded, by whom;
- **View this version**, which [shows it in the window](#view-an-earlier-version);
- under **Changed items**, what changed since the version before it, model elements first, then
  diagrams, then skills. While no button and not the Explorer has the keyboard (Esc lets it go),
  the arrow keys scroll the list by an item, Page Up and Page Down by a page, and Home and End to
  either end. The first version says *First version: there is nothing earlier to compare it with.*;
- under **Previous version**, the version before it, which opens its details the same way.

Versions are compared one at a time, so when you click through History quickly, the changes of the
version you stop on appear once the comparison under way has ended. The top of the centre reads the
project's name and **History**: click the project's name to close the details.

In the example, History lists *Added email* above *Added Customer*. The details of *Added email*
show what it changed, with *Added Customer* as its previous version. *Added Customer* is the first
version.

### View an earlier version

There are three ways to show a version in place of your current work:

- In its details, choose **View this version**.
- Click the version chip, in the bar at the top, left of search. It says *Current work*, with a dot
  while you have unsaved changes. Its list starts with **Current work**, after the branch you are
  on once the project has a second branch ([Switch branch](#switch-branch)), then the versions,
  newest first, by date and description; the latest reads *Same as current work* when nothing is unsaved.
  Click a version to view it, or type to narrow the list, move with ↑ and ↓ and press Enter; Escape
  closes the list. Scrolling to its end lists older versions. From your current work,
  ⇧⌘H (Ctrl Shift H) opens the list too; while a version shows, it takes you
  [back to current work](#back-to-current-work) instead.
- Press ⌘[ (Ctrl [). From your current work this goes to the latest version when you have unsaved
  changes, and otherwise to the one before it, the newest version that differs from what you see.

The version opens on the diagram you were looking at, if it has that diagram, and otherwise on its
first diagram. Over the centre a banner says *Viewing the version of*, its date, and *· Read only*,
and the status bar says the same. The chip is tinted and shows the date between ‹ and ›, with
**Back to current work** beside it. In a narrow window the chip is one icon, and pointing at it says
which version shows.

To step from one version to the next, press ⌘[ (Ctrl [) for an older one and ⌘] (Ctrl ]) for a newer
one, or click ‹ and › on the chip. A step stays on the diagram you are looking at when the next
version has it. At the oldest version ‹ is greyed out and says *No older version*; at the latest, ›
says *No newer version*, since your current work is not a step away: use
[Back to current work](#back-to-current-work). Click the date on the chip to open its list.

In the example, with nothing unsaved, ⌘[ shows *Added Customer*: the class *Customer* without
*email*. ⌘] then shows *Added email*, the same as your current work, and a second ⌘] goes no
further.

### While a version shows

A version you view is read only. You look at it as it was saved, and Archforge turns off everything
that would change it. A control that is off says why when you point at it, usually *Read only while
viewing a version*.

| You can | Off until you go back |
|---|---|
| Open its diagrams and skills, select, pan and zoom | Every tool but Select and Pan, the **Model** menu, deleting, renaming, dragging and typing into the model |
| Read Properties, Appearance and Skills, marked *Read only* | Undo and redo |
| Use History, a version's details and the steps to other versions | Saving: ⌘S does nothing, **File ▸ Save** is off, and **Save version…** says *Back to current work to save a version* |
| Read your Chat conversations | Asking Chat: its message field says *Back to current work to chat*. Undoing a turn's changes from Chat is off too |

An agent you connected through the command-line tool can still read what shows. Anything it asks
that would change the model or save is refused, with a message telling it to go back to current work
first.

### Back to current work

Choose **Back to current work** in the banner or on the chip, choose **Current work** at the top of
the chip's list, or press ⇧⌘H (Ctrl Shift H). Escape doesn't take you back, so a stray key can't lose
your place.

Your current work comes back as you left it: your unsaved changes, the open tabs, the selection, pan
and zoom, and the steps you can undo. Archforge also takes you back to your current work first when
you show another project's tab or Home, close the project or quit; closing and quitting then ask
about unsaved changes as usual.

**Your unsaved work is kept.** Looking at versions doesn't change it: it waits, set aside, until you
come back. Before a version shows, Archforge makes sure nothing is left half done:

| When | What happens |
|---|---|
| A name or note is open for typing, or you are drawing or dragging something | The version doesn't show, and Archforge says *Finish or cancel your edit first.* Finish or cancel it, then try again |
| You were typing a description in Properties | It is written to the model first. If the model refuses it, the version doesn't show and the reason is said |
| Chat is changing the model | Archforge asks *Chat is still changing the model. Stop it and go on?* **Stop and go on** ends its reply, keeping what it has written so far, and shows the version; **Keep working** leaves everything as it was |
| A restore or another change of the whole project is under way | The version doesn't show, and Archforge says *Wait for the change under way to finish.* |

Stepping from one version to another leaves no current work on screen, so Chat is not asked about.

### When a version can't be shown

| What you see | What happened | What to do |
|---|---|---|
| In History, **This version can't be shown**, with the reason behind **Why?** | Archforge can't read or build this version | Nothing. The steps pass it and say so, such as *8 Nov 14:02 can't be shown. Skipped to 7 Nov 09:30.* |
| In History, **Made with a newer Archforge.** | A later Archforge saved this version | Nothing. The steps pass it the same way |
| In History, **This version couldn't be read.**, with **Retry** | Reading it failed, and may work another time | Choose **Retry** to ask for it again. The message goes when you ask for any version, go back to current work, restore, or change branch or project |
| After a step, the reason in the centre | The step failed, and the version you were on stays | Step again, which reads again |
| *The versions couldn't be read.* or *The next versions couldn't be read.*, with **Retry**, in History or the chip's list | The list itself couldn't be read | Choose **Retry** |

If a version can't be shown when you leave your current work for it, your current work stays as it
was and the status bar gives the reason.

**Compare…** in a version's details opens the [Compare page](#choose-what-to-compare).
**Restore this version…** and **Start a branch here…** there, and the restore and branch buttons
beside **Back to current work** in the banner, are described under
[Restore a version](#restore-a-version) and [Make a branch](#make-a-branch).

## See what changed

Before you save, or when you look back, you will want to know what changed. Archforge shows it in
three places, and all three list it the same way:

| Where | Before | After |
|---|---|---|
| **Working changes**, in **Versions** | Your last version | Your current work |
| **Changed items**, in a version's details | The version before it | That version |
| The **Compare** page | Your choice | Your choice |

*Before* is where you start from and *After* is where you arrive. Each change is named by what
happened on the way from one to the other: **Added**, **Changed** or **Removed**.

### The changes list

Each row is one thing that differs: its name, where it is in the model, such as the package and
class that hold it, and a letter for what happened, **A** for Added, **C** for Changed and **R** for
Removed. Something without a name says *Unnamed*. The rows come in groups, **Model elements**, then
**Diagrams**, then **Skills**, each with how many it holds. A diagram that changed, or a box or line
on one, is listed under **Diagrams**.

Under **Working changes** the list keeps up as you work, and once it holds more than 12 changes, the
field **Filter changes…** above it narrows it to the rows whose name holds what you type. Click a
row there, or press Enter on it, to open it in the centre beside your last version, as the next
sections show. The rows in a version's details and on the Compare page are for reading only.

In the example, after saving *Added Customer* you add *email*. **Working changes** lists one row
under **Model elements**: *email*, with *Customer* at the end of where it is, and **A**. *Customer*
itself isn't listed, because a member that comes is counted once, on its own row.

### Choose what to compare

The **Compare** page sets any two versions of the branch you are on, or one version and your
current work or another branch's latest version, side by side. To open it:

- in a version's details, choose **Compare…**;
- or choose **Versions ▸ Compare…**, which takes the version whose details are open, or else the
  newest version.

The page starts with that version as **Before** and your current work as **After**. The top of the
centre reads the project's name and **Compare**: click the project's name to close the page.

Along the top are the two selects, **Before** and **After**, with a button between them and
*Before → After* at the end. Click a select to see what it can hold: under **This branch**,
**Current work** first, saying *Unsaved changes* or *No unsaved changes*, then the versions, newest
first, by date and description. Scrolling to the end of the list reads older versions; if they
can't be read, it says *The next versions couldn't be read.*, with **Retry**. Under **Another
branch** come the project's other branches that have a version, each with *Latest version* beside
it; when **Branches** lists only the first 200, so does this list, and it says so. ↑ and ↓ move
through the list, passing over its headings, Enter or Space chooses, and Escape or Tab closes it.

- **Current work** is always your own work, even while you are
  [viewing an earlier version](#view-an-earlier-version).
- Two versions are compared as each was saved: what you do afterwards doesn't change them.
- Another branch stands for its latest version, and the select says *Latest on* and its name, so
  you can see how two branches differ while they can't be merged. To compare its earlier versions,
  switch to it first.

**Which way round.** Put the older state on the left, as Before, to read the changes the way they
happened. The button between the selects, **Swap Before and After** when you point at it, turns the
pair round. Swapping changes only how the differences are named, never your work or your versions.

In the example, with *Added Customer* as Before and your current work as After, the page lists
*email* as Added. Swap the two and *email* is Removed, since going from your current work back to
*Added Customer* takes it away.

**What the page says.** Under the selects, the page counts the differences, such as *1 difference*,
says how they divide, such as *1 model element*, and lists them in their groups. With current work
on one side, it keeps up as you work. While no button and not the Explorer has the keyboard (Escape
lets it go), the arrow keys scroll the list by a difference, Page Up and Page Down by a page, and
Home and End to either end.

| The page says | What it means | What to do |
|---|---|---|
| *These two versions are the same.*, *This version is the same as your current work.* or *Both sides are your current work.* | There is nothing to list | *Choose another pair above.* |
| Bars, with **Stop** under them | The comparison is under way. When it runs again after a change, **Stop** is above the earlier list | Wait, or choose **Stop** |
| *The comparison was stopped.*, with **Retry** | You chose **Stop**, and nothing is compared again until you choose **Retry** | Choose **Retry**, or another pair |
| *The changes couldn't be compared.*, with **Retry** | The comparison failed. Point at the message to read why | Choose **Retry** |
| *This version can't be shown*, and the reason | A version can't be read here, such as one made with a newer Archforge, or the branch chosen was deleted or renamed since the list was read | Choose another pair |
| *Too large to compare* | A version, or the comparison, is larger than Archforge can compare. Nothing is listed | Choose another pair |

### An element side by side

Click a model element under **Working changes**. The top of the centre reads the project's name and
**Working changes**, and on the right *Compared with the version of* and its date, or, before your
first version, *There is no version to compare with yet.* Click the project's name to close it.

Under the element's name, with **Added**, **Changed** or **Removed** beside it and where it is
below, a summary counts what differs, such as *1 value changed* or *2 members changed*, and says
the first three values in words. *Each difference is marked on the cards below.* An element that
came or went says *This element was added* or *This element was removed* instead.

Two cards follow, **Before** headed *Last version* or *Version of* and its date, and **After**
headed *Current work*. Each gives the element's kind, name and values, then its attributes or
columns, operations, literals, other members, relationships and owner under headings. A long list
scrolls with the cards.

- Something that differs has a tint, with − on the Before card and + on the After card.
- A side the element isn't on says *Nothing here yet* on Before or *Removed* on After.
- A relationship is listed on the cards of both elements it joins, by its role at the far end or
  its own name, else its kind, and what it joins to, such as `lines → OrderLine [1..*]` or
  `Generalization → Shape`. *Not available on this side* means nothing is set there.
- An element with no name goes by what it joins or says, such as *Customer → Order*. Renaming
  *Customer* isn't marked as a change to it.

Under the cards, **Related change** names the other model elements connected to this one that also
differ: what it or its members refer to, what refers to them, and its owner. Point at one to read,
for example, *Order was changed in this comparison.*, and choose **Go to** and its name to open it
the same way.

In the example, save *Added email*, then rename *email* to *contact*. The summary says
*1 value changed* and *The name changed from email to contact.*, the Before card shows *email*
marked −, and the After card *contact* marked +.

### A diagram side by side

Click a diagram, or a box or line of one, under **Working changes**. Two cards, **Model** and
**Layout**, say what changed ([what each means](#a-model-change-or-a-layout-change)). Under them,
**Before** and **After** draw the diagram as it was on each side, at one zoom, so a box that stayed
is in the same place on both.

- A box that differs has an outline, and a line that differs is drawn over in colour. Each says
  *Added*, *Changed* or *Removed*.
- When the element a marked box or line draws also differs, that word opens the element's change,
  as **Go to** does under an element's cards.
- A side the diagram isn't on says *Nothing here yet* or *Removed*.

The two sides move together. Drag or scroll either to pan both, pinch or scroll with ⌘ (Ctrl) held
to zoom both, or click a side, or Tab to it, and use the arrow keys, **+** and **−**, and **0** to
fit both again. Pointing at a side reminds you of these keys. Clicking the diagram in the list again
fits it too.

### A model change or a layout change

A diagram shows part of the model: the boxes and lines draw elements and their relationships. So a
diagram can change in two ways, and the two cards keep them apart.

- **Model** is about the elements its boxes and lines draw: a class renamed, an attribute added, a
  relationship removed. It counts them, such as *1 element changed*, and names them. The model is
  shared, so such a change reaches every diagram that draws the element.
- **Layout** is about this diagram alone: a box placed, moved, taken off or changed, such as resized,
  and a line drawn, bent, relabelled or taken off. It counts them, such as *1 box moved*, and names
  them. Taking a box off a diagram is a layout change: the element stays in the model.

When only one changed, the other card says so: *The model didn't change* and *Only the layout
changed.*, or *The layout didn't change* and *Only the model changed.* *The diagram's own details
changed* means something such as its name, or the order its boxes are drawn in. An empty diagram
that came or went says *This diagram was added* or *This diagram was removed*.

In the example, drag the box *Customer* to a new place on its class diagram. **Working changes**
lists it under **Diagrams**. The **Layout** card says *1 box moved* and *Customer*, the **Model**
card *The model didn't change* and *Only the layout changed.*, and only the Before and After places
of *Customer* differ.

## Restore a version

Viewing an earlier version only looks. To go on working from it, restore it: its content becomes
your current work again, as a new version on top of the history.

1. Open the version: click it in **History**, or [view it](#view-an-earlier-version).
2. Choose **Restore this version…** in its details, or the restore button beside **Back to current
   work** in the banner while it shows.
3. The question **Restore this version?** names the version and says what will happen. Choose
   **Restore**. **Cancel** or Escape leaves everything as it was. Enter does nothing here.

Archforge first takes you back to current work, if you were viewing, and makes sure nothing is left
half done, as for [viewing a version](#back-to-current-work); while Chat is changing the model, it
asks *Chat is still changing the model. Stop it and go on?* first. Then:

- If you have unsaved changes, they are saved first as a version of their own, called *Before
  restoring* and the date and time, such as *Before restoring 8 Nov 15:42*. The question says so
  beforehand: *You can reach them again from History.*
- The model, its diagrams and its skills are made as they were in that version, and saved as a new
  version called *Restored the version of* and its date.

### What a restore keeps

*Nothing is rewound or removed, and later versions stay in History.* A restore only ever adds.

| A restore changes | A restore keeps |
|---|---|
| Your current work: the model, the diagrams and the skills, as the version had them | Every version, the later ones too, and you can restore any of them in turn |
| | Your Chat conversations, which stay as they are |
| | Your other branches, since a restore works on the branch you are on |

*Undo starts fresh after a Restore.* You can't undo a restore step by step: to go back, restore the
version from before it, such as *Before restoring* or the version that was on top.

**Restore this version…** isn't offered for the latest version while nothing is unsaved, since that
is what you have now, nor for a version that can't be read.

In the example, restore *Added Customer*. History then lists *Restored the version of* and its date
above *Added email* and *Added Customer*, and your current work is *Customer* without *email*. To
have *email* back, restore *Added email*.

### When a restore doesn't finish

The question comes back headed *The restore did not finish*, with the reason, **Retry** where trying
again could help, and **Close**. If another question is open at that moment, it waits until that
one is answered.

| The message | What happened | What to do |
|---|---|---|
| *This version is the same as your current work.* | There was nothing to restore, and no version was made. Unsaved work may have been kept as a version first, and then the message says so | Nothing |
| *The restore could not go ahead because the project changed or was busy, so nothing was restored.* | Something else changed the project first | Choose **Retry** |
| *Your unsaved work could not be kept, so nothing was restored.* | Your unsaved work couldn't be written or saved first | Choose **Retry**, or save your work yourself first |
| *Archforge could not tell whether your unsaved work was kept as a version, so nothing was restored.* | The answer was lost | Choose **Close** and look in **History** before trying again |
| *The version could not be restored:* and a reason | Nothing was restored. The message adds whether your unsaved work was kept as a version and whether it is written with your next change | Choose **Retry**, or **Close** |
| *Archforge could not tell whether an earlier restore finished. Close the project and open it again before restoring.* | An earlier restore's outcome is unknown | Choose **Close**, then close the project and open it again |

When Archforge can't tell whether the restore itself finished, the status bar says *Archforge could
not tell whether the last change finished. Nothing more was written. Close the project and reopen
it.* Do that, then look in **History**.

While a restore that didn't finish waits to tell you why, **Restore this version…** in your other
open projects is off and says *A restore in* the project's name *did not finish.*: go to that
project first.

## Work on branches

A *branch* is a separate line of work in the same project. *A branch keeps its own versions,
unsaved work and conversations.* Use one to try out an idea without touching the work you share,
and switch back whenever you like. Every project starts with one branch, `main`.

*Branches can't be merged yet*: work on one branch doesn't reach another. To see how two branches
differ, [compare](#choose-what-to-compare) with the other branch's latest version.

### The Branches section

**Branches**, at the top of **Versions**, lists the project's branches by name, each with when its
latest version was saved. It stays folded while the project has only `main`. The branch you are on
says *Current*, and a dot marks a branch with unsaved changes to the model; conversations don't
count. ↑ and ↓ move between branches. A branch's **More** menu holds **Switch**, **Rename…** and
**Delete…**. A project with more than 200 branches lists the first 200, and says so. If the list
can't be read, it says *The branches couldn't be read.*, with **Retry**.

Once there is a second branch, the version chip in the bar at the top names the branch you are on
before *Current work*, and the status bar says *On* and its name before whether your work is saved.
In a narrower window the chip shortens the name first, then leaves out *Current work*, then shows
only its icon, with the whole label when you point at it.

[History](#history-and-a-versions-details) on a branch lists its own versions, then those it shares
with `main`.

### Make a branch

**New branch…** makes a branch from your current work. Open it with the **+** in the **Branches**
header, there even while the section is folded, with **Versions ▸ New branch…**, or with **New
branch…** at the foot of the version chip's list (↓ past the last row reaches it). The new branch
starts with everything you have now, unsaved changes included, and the branch you are on keeps them
too.

**Start a branch here…** makes a branch from an earlier version instead. It is in a version's
details, and it is the branch button beside **Back to current work** in the banner while a version
shows. Your unsaved changes stay on the branch you are on. If you were viewing the version, viewing
ends on the new branch. It isn't offered for a version that can't be read.

In either dialog:

1. In **Name**, type the branch's name: at most 60 characters, on one line, not starting or ending
   with a space. A name another branch has, even with different capitals, is refused with *That
   name is already in use.*
2. Choose **Create**, or press Enter. It stays off until the name can be used.

Archforge makes the branch and switches to it. The dialog stays up while it works. **Cancel** or
Escape closes it without making anything.

A new branch starts with no conversations: Chat says *No conversations on this branch yet.*
*Conversations stay on the branch where they started*, and each branch lists only its own
([Conversations](07-agents.md#conversations)). Undo starts fresh on the new branch.

In the example, with *Added email* saved, make a branch called *Contact details* and rename *email*
to *contact* there. Switch back to `main`, and *Customer* still has *email*.

### Switch branch

Click another branch in **Branches**, press Enter on it, or choose **Switch** from its **More**
menu. From the version chip, choose the branch at the top of its list, or press ⇧⌘B (Ctrl Shift B),
or choose **Versions ▸ Switch branch**, to list the branches there; type to narrow them, and click
one or press Enter on it.

With nothing unsaved, Archforge switches at once. With unsaved changes, it asks **Switch to** and
the branch's name, and says *{branch} has unsaved changes. They stay on {branch} and are there when
you come back. You won't be able to undo earlier steps.*, with the name of the branch you are on:

- **Save version and switch** saves them as a version first, as ⌘S does, then switches;
- **Switch** leaves them unsaved on this branch, waiting for you to come back;
- **Cancel**, or Escape, stays where you are. Enter does nothing here.

If you are viewing a version, Archforge takes you back to current work first. It makes sure nothing
is left half done, and asks about Chat, as for [viewing a version](#back-to-current-work).

After a switch:

- you see the other branch's current work, with its own unsaved changes, versions and conversations;
- undo starts fresh, so you can't undo steps made on the branch you left;
- closing the project or quitting asks only about the branch you are on. Other branches keep their
  unsaved work for the next time you switch to them.

The project opens again on the branch you left it on. If that branch is gone, or can't be opened,
it opens on `main`, and the status bar says *The branch* and its name *is no longer in this project*,
or why it *could not be opened*.

### Rename or delete a branch

**Rename…** in a branch's **More** menu opens **Rename branch**, with the name filled in. The name is
checked as for a new branch, and **Rename** stays off while it is unchanged. *The conversations and
versions on this branch are not affected.* You can rename the branch you are on, and you stay on it.
`main` keeps its name.

**Delete…** opens **Delete** and the branch's name. Under *Goes with the branch:* it counts the
versions only on this branch and its conversations, and says whether it holds unsaved changes.
**Delete branch** waits for that count, or for word that it couldn't be read. The branch leaves the
project's list, and its contents may remain in the project folder's storage. *There is no Undo for
deleting a branch.* `main` and the branch you are on can't be deleted.

### When a branch change doesn't work

| The message | What happened | What to do |
|---|---|---|
| *Couldn't make the branch:* and a reason, under the name | No branch was made, and you stay where you were | Change the name, or try again |
| *The project changed before the branch was made, so none was made.* | Something else changed the project first | Choose **Create** again |
| *Archforge could not tell whether the branch was made. Look at Branches before trying again.* | The answer was lost | Choose **Cancel**, then look in **Branches** before trying again |
| *The last new branch is not cleaned up yet. Opening the project again removes it.* | A branch that wasn't finished is still in the project's storage | Choose **Cancel**, then close the project and open it again |
| **Couldn't switch to** and the branch's name, with the reason and *You are still on* your branch | The switch didn't happen. The message says whether your work is unchanged, or saved | Choose **Close**, then try again |
| **Switching is refused**, with *Your work could not be kept, so the switch was not made.* | Your work couldn't be written to the project, so it stayed on screen | Choose **Close**, and switch again once the status bar no longer says your changes could not be written |
| *Couldn't rename the branch:* and a reason | The name stays | Try again |
| *Archforge could not tell whether the branch was renamed to* a name | The answer was lost | Rename it to that name again to find out |
| **Couldn't delete** and the branch's name, with *The branch stays:* and a reason | Nothing was deleted | Try again |
| *Archforge could not tell whether the branch was deleted. Delete it again to find out.* | The answer was lost | Delete it again |

A new branch that was made but couldn't be switched to stays in **Branches**: switch to it later.

Back to the [contents](README.md).
