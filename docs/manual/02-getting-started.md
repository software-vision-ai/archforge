# Getting started

## What you need

Archforge has no ready-made download yet; you build it from its source. You need:

- Stable [Rust](https://www.rust-lang.org/tools/install) 1.95 or later.
- [Dolt](https://github.com/dolthub/dolt) 2.6 or a later 2.x, on your `PATH` (or its full path in
  the `ARCHFORGE_DOLT` environment variable). Archforge keeps every project in a Dolt database.
- On Linux, the development packages for xkbcommon, Wayland, X11, Xrandr, Xi and Mesa GL.
- Optionally, a checkout of [craft-fonts](https://github.com/storytold/craft-fonts) next to the
  Archforge checkout, so the interface is drawn in Inter.

## Starting Archforge

From the Archforge checkout:

```sh
cargo run -p archforge
```

Archforge opens on **Home**. From Home you start a new project, open one, and come back to the
projects you used recently.

When Archforge finds no Dolt it can use, Home says so at the top: **Install Dolt** opens Dolt's
installation page in your browser, and once it is installed, **Look again** finds it.

<!-- picture: home light | Home: start a new project or open one, with your pinned folders and recent projects below -->

## Creating a project

1. On Home, choose **Create project** on the **New project** card.
2. Type a name for the project, such as *Commerce platform*. A name Archforge cannot use (one
   already taken in that folder, or one with a character a file name cannot hold) is explained
   under the field.
3. The **Location** field shows where the project goes: the Archforge folder in your Documents,
   unless you choose another with **Change…**.
4. Choose **Create project**.

<!-- picture: new-project light | The New project dialog: a name, the location, and what every project holds -->

The project opens in the workspace, in a tab of its own in the bar at the top. A project is a
folder named after it, ending in `.archforge` (for example `Commerce platform.archforge`), in the
location you chose.

## Saving

You do not lose work between saves: every change is written to the project a moment after you
make it. Saving makes a version of the project: press ⌘S (Ctrl S), or on macOS choose
**File ▸ Save**. Until you save, the status bar at the bottom says *Unsaved changes · ⌘S saves*, and
a dot shows before the project's name in the window title; afterwards it says *Saved*.

## Closing and reopening

- Close a project with the **×** on its tab. If it has unsaved changes, Archforge asks whether to
  save them: **Save**, **Cancel**, or **Don't save**, which puts the project back to its last save.
- **Home** (the house in the bar) shows Home again without closing your projects. Up to eight
  projects can be open at once, each in its tab.
- On Home, **Recent projects** lists the last twelve projects you opened. Choose one to open it;
  it comes back with the diagrams you had open and the view you left them in.
- To open a project that is not in the list, choose **Browse files** on the **Open a project** card,
  or **+ Open project** in the strip at the very top, and pick the project's `.archforge` folder.
- **Pin a folder** on Home keeps a folder of projects one click away.

## Finding a project on disk

The location under a project's name in **Recent projects**, and at the right of the status bar
while it is open, is the folder that holds it. Copy or back up the whole `.archforge` folder; it
holds the model, its diagrams, its skills and its versions.

Next: [The workspace](03-workspace.md).
