# Versions

You are about to rework part of a model and want a way back to it as it is now. Or you have reached
a point worth naming, such as what the team agreed on. A *version* is that point: the whole project
as it stood at one moment, with a line that says what it is, kept in the project's history.

This chapter explains what a version is and how to save one. [The workspace](03-workspace.md) says
how to look at the versions you have saved.

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
   **Working changes** lists them, model elements first and then diagrams. Click a model element or a
   diagram to see how it differs from your last version.
2. Choose **Save version…**, at the right of the **Working changes** heading. The **Save version**
   dialog opens and says how many model elements and diagrams changed since your last version.
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

Press ⌘S (Ctrl S), or choose **File ▸ Save**, to make a version straight away, with no review and no
description. **Save** in the question Archforge asks when you close a project does the same.
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

⌘S doesn't wait for a first change: on a new project you haven't touched, it saves the empty project
as your first version, with *No description*.

## When there is nothing to save

With no changes since your last version, **Working changes** says *No changes since the last
version.* and **Save version…** is off. ⌘S makes no second version, and the status bar stays on
*Saved*.

Now and then the status bar says *Unsaved changes* while **Working changes** lists nothing, for
example after a newer Archforge has brought an older project up to date. There is nothing to review,
so **Save version…** stays off, and ⌘S saves it.

Point at **Save version…** while it is off to see why:

| It says | Why | What to do |
|---|---|---|
| *No changes since the last version.* | There is nothing to review | Nothing, or make a change first |
| *Loading* | Archforge is still working out what changed | Wait a moment |
| *Saving…* | A save is running | Wait for it to finish |
| *The changes couldn't be compared.* | Archforge couldn't work out what changed | ⌘S still saves, without a review |
| *Back to current work to save a version* | You are viewing an earlier version | Choose **Back to current work** |

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

Back to the [contents](README.md).
