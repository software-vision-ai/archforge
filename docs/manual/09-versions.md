# Versions

You are about to rework part of a model and want a way back to it as it is now. Or you have reached
a point worth naming, such as what the team agreed on. A *version* is that point: the whole project
as it stood at one moment, with a line that says what it is, kept in the project's history.

This chapter explains what a version is, how to save one, and how to
[look back at one](#look-back-at-a-version) and come back to your work.

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
could not be written to the project*, and Archforge tries again with your next change or save.

What autosave writes is not a version. When you close a project with unsaved changes, Archforge asks
what to do with them, and **Don't save** throws away what you changed since your last version. A
project with no version yet goes back to how it was created. Your conversations stay.

**Undo and redo** move your current work one step at a time. Saving doesn't end them. In the example,
you could press ⌘Z after saving *Added email* and take *email* back. The version *Added email* stays
as it was, still with *email*, and the status bar goes back to *Unsaved changes*, because your
current work now differs from your last version. Undo never removes a version and never adds one.

Undo remembers up to 500 steps, and only while the project stays open. A
[restore](03-workspace.md) starts it afresh.

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
  diagrams, then skills. The first version says *First version: there is nothing earlier to compare it with.*;
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
  on once the project has a second branch ([the workspace](03-workspace.md)), then the versions,
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

**Compare…**, **Restore this version…** and **Start a branch here…** in a version's details, and the
restore and branch buttons beside **Back to current work** in the banner, are described in
[The workspace](03-workspace.md).

Back to the [contents](README.md).
