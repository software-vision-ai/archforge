# Release checklist: what the gate cannot drive

`cargo xtask ci` runs every journey of the application end to end, but inside a test harness: it
never touches the operating system. Before a release, follow this list once in the running app on
macOS, in the light appearance and again in the dark one (the moon or sun at the top right switches
them). Each item takes a few minutes, says what to do and what you should see, and names what it
covers that the gate cannot.

[Releasing](releasing.md) says how a release is cut; this list is its fourth step.

Start from a clean build of the release commit:

```sh
cargo run -p archforge
```

## 1. The window and its icon

**Do:** start Archforge.

**Expect:** the window opens on Home; the Dock shows the Archforge icon (the beaver on a blue tile)
for the running app.

**Covers:** the real window, its title strip and the icon the operating system shows.

## 2. A new project opens without moving the mouse

**Do:** on Home choose **Create project**, type a name, choose **Create project**, then take your
hand off the mouse and keyboard.

**Expect:** within a second the dialog closes, the project's tab appears and its workspace is ready
(the Models explorer, the empty centre, Properties), with the pointer still.

**Covers:** the real window waking up on its own when work finishes, with nothing moving the
pointer.

## 3. Choosing a project's location

**Do:** in the New project dialog choose **Change…**, then **Cancel** in the picker; choose
**Change…** again and pick another folder; create the project.

**Expect:** the picker is titled **Choose project location**. After the cancel, the name and the
folder are as they were. After the pick, the field shows the chosen folder by its last two names,
and Finder shows the project there as a folder named `<name>.archforge`.

**Covers:** the system folder picker and its title, and a project made in a folder outside the
default one.

## 4. Pinning a folder and opening a project

**Do:** on Home choose **Pin a folder** and pick a folder; then choose **+ Open project** at the top
(or press ⌘O), pick a `.archforge` folder and open it; then try ⌘O with a folder that is not a
project.

**Expect:** the first picker is titled **Pin a folder** and the folder appears under Pinned folders.
The project opens in a tab of its own. The folder that is not a project is refused: Home says in
red **The project could not be opened:** and why, and nothing else changes.

**Covers:** the system pickers for pinning and opening, and the refusal of a folder that is not a
project.

## 5. The menu bar

**Do:** with a project open, look through the **Archforge** and **File** menus; choose
**File ▸ Generate…**, take your hand off the mouse and keyboard, and watch; close the dialog; choose
**Archforge ▸ About Archforge…** and keep still again; with About open, look at **File** again. Close
About, type into a name field and, with the field still active, press ⌘Z and then ⇧⌘Z, and choose
**Edit ▸ Undo**. Click the canvas so the field lets go, make a change, press ⌘Z and then ⇧⌘Z. With the
field active again, press ⌘C, ⌘V and ⌘A. Open a class diagram and look through **Model**, choose
**Model ▸ Interface** and click the canvas; open an ER diagram and look through **Model** again; go to Home
and look once more.

**Expect:** the application menu offers **Settings…** (⌘,) and **Quit Archforge** (⌘Q). File offers
**New project…**, **Open…** (⌘O), **Close project**, **Save** (⌘S) and **Generate…** (⇧⌘G), each enabled
while it can act: Save, Close project and Generate… need an open project, and Save is disabled while a
save runs. Each dialog opens within a second of choosing it, with the pointer still. While any dialog
is up, every File item is disabled. Edit offers **Undo** (⌘Z) and **Redo** (⇧⌘Z). While a field has the
keyboard, both are enabled and act on the field's own text, never the model; on the canvas they take the
change back and bring it again, and are disabled when there is no step to take. ⌘C, ⌘V and ⌘A act on the
field, as they always have. Model on a class diagram lists Class to Package, the kinds of the Relationship
tool, then **Note** and **Link note**, and Interface places an interface where you click. On an ER diagram
it lists **Entity**, **Relationship**, **Note** and **Link note**. On Home it lists Note and Link note, disabled.

**Covers:** the native macOS menu bar, which the harness has no part of, and the real window waking
when an item in it is chosen.

## 6. About Archforge and its support links

**Do:** in About Archforge choose **Ko-fi**; then look at **GitHub Sponsors**; with a table selected
in Models behind the dialog, press the arrow keys and Delete; then press Escape.

**Expect:** Ko-fi opens https://ko-fi.com/dejanwho in your browser and Archforge stays as it was.
GitHub Sponsors says **Not set up yet** until its profile is published, and opens nothing. The keys
change nothing behind the dialog; Escape closes it, and the table is still selected.

**Covers:** opening links in the system browser, and the real keyboard behind a dialog.

## 7. Quitting with unsaved changes

**Do:** make a change in a project without saving, then quit with ⌘Q; choose **Cancel**; quit again
and choose **Save**.

**Expect:** Archforge asks **Save the changes to `<name>`?** with Save, Cancel and Don't save. Cancel
keeps Archforge open with the change. Save saves, then Archforge quits; opening the project again
shows the change.

**Covers:** the operating system's quit request and closing the real window.

## 8. Agent access

**Do:** quit, then start Archforge with a control port, open a project, and ask it what is open:

```sh
cargo run -p archforge -- --control 47000
cargo run -p archforge-cli -- app document.inspect --port 47000
```

**Expect:** the command prints the open project's name and what is selected, and **File ▸
Generate…** says **Agent access is on**. Started without `--control`, Generate… says **Agent access
is off** and the command finds no Archforge.

**Covers:** the loopback port and the token file the operating system keeps for agent access.

## Keeping this list current

A change that adds or changes behaviour the gate cannot drive (a native menu item, a system dialog,
a link opened outside the app, a new agent-access step) updates this list in the same change. The
list names only what Archforge does today.
