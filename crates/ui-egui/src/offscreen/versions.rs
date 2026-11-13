//! The Versions screens of the user manual (AR-347): the chapter's example, Customer saved as
//! *Added Customer* and its attribute email as *Added email*, then changed, over a host that answers
//! from those two versions kept in memory. History, Working changes, a version's details and the
//! two sides are then read and compared by the panel's own steps, as the app does.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use archforge_mof::rows::Rows;
use serde_json::json;

use super::Screen;
use crate::chat_words::Clock;
use crate::{BranchEntry, Shell, VersionEntry, VersionRead, VersionReadError, VersionsPage};

/// The screens built here.
pub(super) const SCREENS: [Screen; 8] = [
    Screen::WorkspaceVersions,
    Screen::WorkspaceVersionList,
    Screen::WorkspaceVersionDetails,
    Screen::WorkspaceVersionViewing,
    Screen::WorkspaceChangeElement,
    Screen::WorkspaceChangeDiagram,
    Screen::WorkspaceRestoreAsk,
    Screen::WorkspaceBranches,
];

/// The versions kept, newest first, each with its model, and the project's branches, as a project
/// keeps them: `main` at least.
struct Kept(Vec<(VersionEntry, Arc<Rows>)>, Vec<BranchEntry>);

impl crate::ProjectHost for Kept {
    /// Makes no project: the screens open theirs.
    fn create(&self, _folder: &Path, _name: &str) -> Receiver<Result<crate::Opened, crate::CreateError>> {
        channel().1
    }

    /// Every version on the first page, and nothing after it.
    fn versions(&self, _project: &Path, after: Option<String>, _limit: usize) -> Receiver<Result<VersionsPage, String>> {
        let (tx, rx) = channel();
        let versions = if after.is_none() { self.0.iter().map(|(v, _)| v.clone()).collect() } else { Vec::new() };
        let _ = tx.send(Ok(VersionsPage { versions, more: false }));
        rx
    }

    /// The model kept for `commit`; any other is no version of this branch.
    fn read_version(&self, _project: &Path, commit: String) -> Receiver<VersionRead> {
        let (tx, rx) = channel();
        let _ = tx.send(self.0.iter().find(|(v, _)| v.hash == commit).map(|(_, rows)| rows.clone()).ok_or(VersionReadError::Unavailable));
        rx
    }

    /// The branch marked current, as stored.
    fn current_branch(&self, _project: &Path) -> Option<String> {
        self.1.iter().find(|b| b.current).map(|b| b.name.clone())
    }

    /// The branches kept, all of them.
    fn branches(&self, _project: &Path) -> Receiver<Result<(Vec<BranchEntry>, bool), String>> {
        let (tx, rx) = channel();
        let _ = tx.send(Ok((self.1.clone(), false)));
        rx
    }
}

/// The model of the session open as rows, as a version keeps it.
fn rows(shell: &Shell) -> Result<Arc<Rows>, String> {
    let session = shell.session.as_ref().ok_or("the sample needs a session")?;
    let (tables, junctions) = archforge_notations::storage();
    session.doc().freeze().rows(&tables, &junctions).map(Arc::new).map_err(|e| e.to_string())
}

/// Runs `command` on the session open and gives the id it answers, if any.
fn run(shell: &mut Shell, command: &str, params: serde_json::Value) -> Result<String, String> {
    let session = shell.session.as_mut().ok_or("the sample needs a session")?;
    let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
    Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
}

/// The class diagram Commerce with Customer, saved as *Added Customer*, then with email : String,
/// saved as *Added email*; then unsaved, email renamed contact, Customer's box moved, or, for the
/// two sides, only the one of them they show. On the branch Contact details the rename is saved
/// there, the versions before it shared with `main`. Versions is the explorer, the clock Chat's.
pub(super) fn sample(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    shell.adopt_project();
    let root = run(shell, "uml.model.root", json!({}))?;
    let diagram = run(shell, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": root, "name": "Commerce" }))?;
    let customer = run(shell, "uml.class.add", json!({ "package": root, "name": "Customer" }))?;
    let node = run(shell, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 160, "y": 140 }))?;
    let first = rows(shell)?;
    let email = run(shell, "uml.attribute.add", json!({ "class": customer, "name": "email", "type": "String" }))?;
    let second = rows(shell)?;
    shell.session.as_mut().ok_or("the sample needs a session")?.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    if screen != Screen::WorkspaceChangeDiagram {
        run(shell, "uml.attribute.set", json!({ "attribute": email, "name": "contact" }))?;
    }
    let branched = screen == Screen::WorkspaceBranches;
    let third = rows(shell)?;
    if screen != Screen::WorkspaceChangeElement {
        run(shell, "diagram.move", json!({ "nodes": [node], "dx": 260, "dy": 60 }))?;
    }
    let version = |hash: &str, description: &str, date: &str, previous: Option<&str>| VersionEntry {
        hash: hash.into(),
        description: Some(description.into()),
        author: Some("Robin".into()),
        date: date.into(),
        previous: previous.map(str::to_string),
        shared: branched && hash != "c3",
        ..VersionEntry::default()
    };
    let mut kept = vec![
        (version("c2", "Added email", "2026-11-07T13:02:00Z", Some("c1")), second),
        (version("c1", "Added Customer", "2026-11-07T12:40:00Z", None), first),
    ];
    let branch = |name: &str, current, latest: &str| BranchEntry {
        name: name.into(),
        shown: name.into(),
        current,
        unsaved: current,
        latest: Some(latest.into()),
        latest_version: None,
    };
    let mut branches = vec![branch("main", !branched, "2026-11-07T13:02:00Z")];
    if branched {
        kept.insert(0, (version("c3", "Renamed email to contact", "2026-11-07T13:20:00Z", Some("c2")), third));
        branches.push(branch("Contact details", true, "2026-11-07T13:20:00Z"));
    }
    shell.services.projects = Some(Arc::new(Kept(kept, branches)));
    let now = crate::chat_fixture::NOW;
    shell.services.clock = Some(Arc::new(move || Clock { now, offset: Some(crate::chat_fixture::OFFSET) }));
    shell.state.show_explorer(crate::state::Explorer::SourceControl);
    let diagram = archforge_mof::Uuid::parse(&diagram).ok_or("the sample's diagram")?;
    crate::canvas::open(shell, diagram);
    Ok(())
}

/// Whether nothing the screens read, compare or hold the project for is still on its way.
fn idle(shell: &Shell) -> bool {
    let v = &shell.versions;
    // Current work alone is compared: a version in view never is.
    (shell.viewing().is_some() || crate::versions::settled(shell))
        && v.reading.is_none()
        && v.element_reading.is_none()
        && v.details_comparing.is_none()
        && v.details_reading.is_none()
        && shell.version_list.steps.reading.is_none()
        && !shell.loads.waiting()
        && !shell.is_held()
}

/// Draws frames until nothing is on its way twice running, giving the comparing threads their turn
/// between frames: the shell crate keeps off the clock, so the wait is a bound on frames, not on
/// time. A screen that never settles within it fails rather than drawing half read.
fn settle(h: &mut egui_kittest::Harness<'static, Shell>) -> Result<(), String> {
    let mut calm = 0;
    for _ in 0..2_000 {
        h.run();
        calm = if idle(h.state()) { calm + 1 } else { 0 };
        if calm == 2 {
            return Ok(());
        }
        std::thread::yield_now();
    }
    Err("the versions were still being read".into())
}

/// The version listed as `hash`.
fn listed(shell: &Shell, hash: &str) -> Result<VersionEntry, String> {
    let page = shell.versions.page.as_ref().ok_or("the versions were not read")?;
    page.versions.iter().find(|v| v.hash == hash).cloned().ok_or_else(|| format!("no version {hash}"))
}

/// Takes `screen` from the drawn sample to what it shows, through the steps a person takes, and
/// waits for what they read.
pub(super) fn reach(h: &mut egui_kittest::Harness<'static, Shell>, screen: Screen) -> Result<(), String> {
    settle(h)?;
    let shell = h.state_mut();
    let clock = crate::chat_panel::clock(shell);
    match screen {
        Screen::WorkspaceVersionList => crate::version_chip::open(shell),
        Screen::WorkspaceVersionDetails => crate::version_details::open(shell, listed(shell, "c2")?),
        Screen::WorkspaceVersionViewing => {
            let label = crate::version_chip::when(&listed(shell, "c1")?.date, clock);
            if !crate::version_chip::view(shell, "c1".into(), label) {
                return Err("the version could not be shown".into());
            }
        }
        Screen::WorkspaceChangeElement | Screen::WorkspaceChangeDiagram => {
            let area = if screen == Screen::WorkspaceChangeElement {
                archforge_notations::differences::Area::Model
            } else {
                archforge_notations::differences::Area::Diagram
            };
            let row = shell.versions.changes.iter().find(|d| d.area == area).cloned().ok_or("no such change")?;
            crate::change_view::open(shell, row);
        }
        Screen::WorkspaceRestoreAsk => {
            let entry = listed(shell, "c1")?;
            let saved = crate::version_chip::when(&entry.date, clock);
            crate::restore_ask::ask(shell, ("c1".into(), saved, crate::versions::dated(&entry, clock)));
        }
        _ => {}
    }
    settle(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use archforge_notations::differences::Area;

    /// Every Versions screen of the manual builds its versions and reaches what it shows in both
    /// appearances: what is compared, the list, the details, the version in view, the two sides,
    /// the question or the branches, so a step that no longer gets there fails here rather than
    /// when the manual is rendered.
    #[test]
    fn every_versions_screen_reaches_what_it_shows_in_both_appearances() {
        for screen in SCREENS {
            for theme in [crate::theme::ThemeKind::Light, crate::theme::ThemeKind::Dark] {
                let h = super::super::drawn(1440.0, 900.0, 1.0, theme, screen).unwrap_or_else(|e| panic!("{screen:?} in {theme:?}: {e}"));
                let s = h.state();
                let v = &s.versions;
                let side = |area| v.element.as_ref().is_some_and(|o| o.row.area == area);
                let shows = match screen {
                    Screen::WorkspaceVersions => v.changes.len() == 2 && v.page.as_ref().is_some_and(|p| p.versions.len() == 2),
                    Screen::WorkspaceVersionList => s.widgets().iter().any(|w| w.id == "workspace.header.version.row.2"),
                    Screen::WorkspaceVersionDetails => v.details.is_some() && crate::branch_ask::offered_at(s, "c2"),
                    Screen::WorkspaceVersionViewing => s.viewing() == Some("c1"),
                    Screen::WorkspaceChangeElement => side(Area::Model) && v.changes.len() == 1,
                    Screen::WorkspaceChangeDiagram => side(Area::Diagram) && v.changes.len() == 1,
                    Screen::WorkspaceRestoreAsk => s.ask.is_some(),
                    _ => v.branches.as_ref().is_some_and(|b| b.len() == 2) && v.changes.len() == 1,
                };
                assert!(shows, "{screen:?} in {theme:?} shows what it is for");
                assert_eq!(s.canvas.notice, None, "{screen:?} in {theme:?} draws without a refusal");
            }
        }
    }
}
