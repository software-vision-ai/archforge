//! The Versions explorer (AR-255, from page 09 of the design and sections 2, 10 and 11 of
//! `docs/design/versioning.md`): Branches, Working changes and History, the versions read off the
//! writer a page at a time. A read keeps what was shown until its answer comes, and one for
//! another project or session is dropped by the next. A row opens the version's details.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use archforge_mof::rows::Rows;
use archforge_notations::differences::{Area, ChangeKind, Difference};
use egui::{Color32, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::chat_words::{Clock, date_and_time, seconds_of};
use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::{fmt, tr, tr_ctx, trn};
use crate::settings::{Look, button, control, focus_ring};
use crate::ux::{self, Palette, icons, types};
use crate::{BranchEntry, Shell, VersionEntry, VersionRead, VersionsPage, widgets};

/// The versions read at a time.
const PAGE: usize = 50;
/// A section's header, and a version's row and the distance between rows.
const HEADER: f32 = 40.0;
const ROW: f32 = 48.0;
const PITCH: f32 = 50.0;
/// The divider where the versions shared with `main` start, and the room it takes in the list.
const SHARED: f32 = 16.0;
const SHARED_PITCH: f32 = 18.0;
/// A branch's row and the distance between rows.
const BRANCH: f32 = 36.0;
const BRANCH_PITCH: f32 = 42.0;
/// What a list leaves under it: below Branches, Working changes' header and words and History's
/// header and two rows; below Working changes and its words, History's header and two rows. The
/// least a branch list keeps, so a row stays in reach.
const UNDER_BRANCHES: f32 = 3.0 * HEADER + 2.0 * PITCH;
const UNDER_CHANGES: f32 = 8.0 + HEADER + 2.0 * PITCH;
const LEAST_BRANCHES: f32 = BRANCH + 4.0;
/// A change's row, its pitch and a group's heading; the list that gets a filter; its share of the column.
const CHANGE: f32 = 44.0;
const CHANGE_PITCH: f32 = 46.0;
const GROUP: f32 = 24.0;
const FILTER_FROM: usize = 12;
const MOST_OF_COLUMN: f32 = 0.5;
/// What a comparison is of: the last version (none before the first) and the session's revision.
pub(crate) type Compared = (Option<String>, u64);
/// The bars a first read shows in place of the rows.
const SKELETON_ROWS: usize = 6;

/// What the panel keeps between frames.
#[derive(Default)]
pub struct State {
    /// The project and session the panel is about.
    pub(crate) about: Option<(PathBuf, u64)>,
    /// Whether a read has started for them.
    asked: bool,
    pub(crate) reading: Option<Receiver<Result<VersionsPage, String>>>,
    pub(crate) page: Option<VersionsPage>,
    /// The branches, the read of them on its way, why the last failed, and whether one is asked
    /// for since the versions were last read.
    pub(crate) branches: Option<Vec<BranchEntry>>,
    /// More branches exist than were listed.
    pub(crate) branches_cut: bool,
    branches_reading: Option<Receiver<Result<(Vec<BranchEntry>, bool), String>>>,
    branches_failed: Option<String>,
    pub(crate) branches_asked: bool,
    /// Whether Branches was opened or folded by hand; until then it is open with two or more.
    branches_open: Option<bool>,
    /// Why the last read failed; what was shown stays.
    failed: Option<String>,
    /// A save or a restore may have made a version since the read.
    pub(crate) stale: bool,
    /// A branch switch started from the question, until it arrives or says why not.
    pub(crate) switching: Option<crate::switch_ask::Switching>,
    /// A branch asked for in New branch…, until the host answers.
    pub(crate) making: Option<crate::branch_ask::Making>,
    /// A read begun for that has yet to succeed, so the first row may not be the last version.
    last_unsure: bool,
    /// The working changes counted, which the rail's badge shows; AR-269 keeps it.
    pub(crate) working: usize,
    /// The versions that cannot be shown, by hash, with why; none for one of a newer Archforge.
    pub(crate) unavailable: BTreeMap<String, Option<String>>,
    /// The version whose read failed in a way that may pass, while its banner stands: asking for
    /// a version ends it, as do going back from one, a restore, a branch switch and a project change.
    pub(crate) load_failed: Option<LoadFailed>,
    /// The last version's rows, by its hash; none before the first version.
    base: Option<(Option<String>, Arc<Rows>)>,
    reading_base: Option<(String, Receiver<VersionRead>)>,
    pub(crate) comparing: Option<(Compared, Receiver<Result<Vec<Difference>, String>>)>,
    /// What the changes shown are of, and why they could not be told.
    pub(crate) compared: Option<Compared>,
    pub(crate) changes: Vec<Difference>,
    pub(crate) compare_failed: Option<String>,
    pub(crate) filter: String,
    changes_scroll: TreeScroll,
    /// The next page, on its way, and why the last one could not be read.
    pub(crate) more_reading: Option<Receiver<Result<VersionsPage, String>>>,
    pub(crate) more_failed: Option<String>,
    /// The version chosen, by its hash: a list that grows or a filter that hides it keeps it.
    pub(crate) selected: Option<String>,
    /// Only versions with a description are listed.
    described_only: bool,
    /// The Save version dialog, while it is open.
    pub(crate) dialog: Option<crate::save_version::Dialog>,
    /// The restore asked for here, until it ends or, failed, until its question comes back.
    pub(crate) restoring: Option<crate::restore_ask::Restoring>,
    /// The version whose details the centre shows.
    pub(crate) details: Option<crate::version_details::Details>,
    /// The one comparison of two versions the details have under way.
    pub(crate) details_comparing: Option<crate::version_details::Running<Arc<Vec<Difference>>>>,
    /// The one read of the version before the link has under way.
    pub(crate) details_reading: Option<crate::version_details::Running<VersionsPage>>,
    /// The changed element the centre shows, and the one read of its two sides under way.
    pub(crate) element: Option<crate::change_view::OpenedElement>,
    pub(crate) element_reading: Option<crate::change_view::ElementReading>,
    pub(crate) focus_save: bool,
    working_folded: bool,
    history_folded: bool,
    scroll: TreeScroll,
    branches_scroll: TreeScroll,
}

/// A read of a version that failed in a way that may pass: what Retry asks for again.
#[derive(Clone)]
pub(crate) struct LoadFailed {
    pub(crate) commit: String,
    /// What the chip then says of it, when it was asked for with a date.
    pub(crate) label: Option<String>,
    pub(crate) why: String,
}

/// Keeps the panel to the project and session shown, takes the answer that has come, and asks for
/// the versions when they are due. A change of either lets go of all that belonged to the old one,
/// the read on its way among it, so an answer for it cannot land, whether or not a host is there.
pub(crate) fn refresh(shell: &mut Shell, ctx: &egui::Context) {
    let about = shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()));
    let v = &mut shell.versions;
    if about != v.about {
        // A version shown moves the session but not what can be shown: a reason holds of a hash.
        if about.as_ref().map(|a| &a.0) != v.about.as_ref().map(|a| &a.0) {
            v.unavailable.clear();
            (v.load_failed, v.branches, v.branches_open, v.branches_cut) = (None, None, None, false);
        }
        // A menu's row is by its place in the list, which the next read may change.
        (v.branches_reading, v.branches_failed, v.branches_asked) = (None, None, false);
        if matches!(shell.panels.menu, Some(crate::panels::Menu::Branch(_))) {
            shell.panels.menu = None;
        }
        (v.about, v.page, v.failed, v.reading, v.asked) = (about, None, None, None, false);
        v.scroll = TreeScroll::default();
        // Both were about what the panel has just left.
        (v.working, v.changes, v.compare_failed) = (0, Vec::new(), None);
        (v.base, v.reading_base, v.comparing, v.compared) = (None, None, None, None);
        (v.filter, v.changes_scroll) = (String::new(), TreeScroll::default());
        (v.more_reading, v.more_failed, v.selected, v.described_only, v.last_unsure) = (None, None, None, false, false);
        (v.element, v.element_reading) = (None, None);
    }
    let answer = match v.reading.as_ref().map(Receiver::try_recv) {
        Some(Ok(answer)) => Some(answer),
        Some(Err(TryRecvError::Disconnected)) => Some(Err("the read stopped without an answer".to_string())),
        _ => None,
    };
    match answer {
        // Begun before a save that has ended since, it may not list that save's version: `stale`
        // says so until the read after it.
        Some(Ok(page)) => (v.reading, v.page, v.last_unsure) = (None, Some(page), false),
        Some(Err(why)) => {
            if v.last_unsure {
                // Counted against a version that may no longer be the last: not "no changes" either.
                (v.working, v.changes, v.compared, v.comparing) = (0, Vec::new(), None, None);
                v.compare_failed = Some(why.clone());
            }
            (v.reading, v.failed) = (None, Some(why));
        }
        None => {}
    }
    take_more(v);
    let waiting = v.branches_reading.is_some();
    // A row of an open menu may now hold another branch.
    if take_branches(v) && matches!(shell.panels.menu, Some(crate::panels::Menu::Branch(_))) {
        shell.panels.menu = None;
    }
    // The chip and its list, drawn before, may now say another branch, date or limit.
    if waiting && v.branches_reading.is_none() {
        ctx.request_repaint();
    }
    if (!v.asked || v.stale) && v.reading.is_none() {
        read(shell);
    }
    // Once the versions are read, so two reads don't take two of the host's places at once, and
    // once a read on its way has answered: the one asked for meanwhile follows it.
    let v = &mut shell.versions;
    let reads = (&v.reading, &v.branches_reading);
    if let (false, (None, None), Some(host), Some((path, _))) = (v.branches_asked, reads, &shell.services.projects, &v.about) {
        (v.branches_asked, v.branches_reading) = (true, Some(host.branches(path)));
    }
    keep_changes(shell);
    let v = &shell.versions;
    if v.reading.is_some() || v.more_reading.is_some() || v.reading_base.is_some() || v.comparing.is_some() || v.branches_reading.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Takes the answers about the changes that have come, then compares the session's newest rows
/// with the last version's when either has moved, off the interface thread. One comparison runs at
/// a time, so revisions that move while it runs fold into the next, which reads the newest rows;
/// edits an agent makes through the control channel move the revision like any other. A version
/// in view is not current work, and nothing is compared while the last version is in doubt.
fn keep_changes(shell: &mut Shell) {
    take_change_answers(shell);
    let (Some(key), Some(session)) = (working_key(shell), shell.session.as_ref()) else { return };
    if shell.versions.compared.as_ref() == Some(&key) || shell.versions.comparing.is_some() {
        return;
    }
    let (tables, junctions) = archforge_notations::storage();
    let v = &mut shell.versions;
    let Some(before) = base_held(v, &key.0) else {
        if let Some(hash) = key.0.as_ref().filter(|hash| v.reading_base.as_ref().is_none_or(|(h, _)| h != *hash))
            && let Some((host, (path, _))) = shell.services.projects.as_ref().zip(v.about.as_ref())
        {
            v.reading_base = Some((hash.clone(), host.read_version(path, hash.clone())));
        }
        return;
    };
    let after = match archforge_mof::rows::snapshot(session.doc(), &tables, &junctions) {
        Ok(rows) => rows,
        Err(e) => return settle_changes(shell, key, Err(e.to_string())),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let compare = move || {
        let found = archforge_notations::differences::classified(&before, &after, crate::editing::fit_tables);
        let _ = tx.send(found.map_err(|e| e.to_string()));
    };
    match std::thread::Builder::new().name("archforge-working-changes".into()).spawn(compare) {
        Ok(_) => v.comparing = Some((key, rx)),
        Err(e) => settle_changes(shell, key, Err(e.to_string())),
    }
}

/// What a comparison of current work is of now; none while a version is in view, the last version
/// is in doubt or an edit is under way.
pub(crate) fn working_key(shell: &Shell) -> Option<Compared> {
    let v = &shell.versions;
    let comparable = shell.viewing.is_none() && v.page.is_some() && last_known(v);
    let session = shell.session.as_ref().filter(|s| comparable && !s.interacting())?;
    Some((last_version(v), session.revision()))
}

/// The rows of the last version `last`, when they are held. Before the first version there is no
/// baseline, so they are empty rows and everything is added.
pub(crate) fn base_held(v: &State, last: &Option<String>) -> Option<Arc<Rows>> {
    match (&v.base, last) {
        (Some((hash, rows)), _) if hash == last => Some(rows.clone()),
        (_, Some(_)) => None,
        (_, None) => {
            let (tables, junctions) = archforge_notations::storage();
            Some(Arc::new(Rows { tables: vec![Vec::new(); tables.len()], junctions: vec![Vec::new(); junctions.len()] }))
        }
    }
}

/// Takes the last version's rows and the comparison's answer when they have come.
fn take_change_answers(shell: &mut Shell) {
    let v = &mut shell.versions;
    if let Some((hash, rx)) = &v.reading_base {
        let answer = match rx.try_recv() {
            Err(TryRecvError::Empty) => None,
            Ok(answer) => Some(answer),
            Err(TryRecvError::Disconnected) => Some(Err("the read stopped without an answer".to_string().into())),
        };
        let hash = hash.clone();
        match answer {
            None => {}
            Some(Ok(rows)) => (v.reading_base, v.base) = (None, Some((Some(hash), rows))),
            Some(Err(why)) => {
                let revision = shell.session.as_ref().map_or(0, |s| s.revision());
                shell.versions.reading_base = None;
                // Only of the version the changes are counted against.
                if last_known(&shell.versions) && last_version(&shell.versions).as_ref() == Some(&hash) {
                    settle_changes(shell, (Some(hash), revision), Err(why.to_string()));
                }
            }
        }
    }
    let Some((key, rx)) = &shell.versions.comparing else { return };
    let answer = match rx.try_recv() {
        Err(TryRecvError::Empty) => return,
        Ok(answer) => answer,
        Err(TryRecvError::Disconnected) => Err("the comparison stopped without an answer".to_string()),
    };
    let key = key.clone();
    shell.versions.comparing = None;
    let moved = shell.session.as_ref().map(|s| s.revision()) != Some(key.1);
    // Against another last version, or one a save may have replaced, it says nothing of these
    // rows. Rows that have only moved on still show what they were, or edits in a steady stream
    // would hide every answer until they stop; a failure for them says nothing of the newest
    // rows, which the next look compares.
    if last_known(&shell.versions) && key.0 == last_version(&shell.versions) && (!moved || answer.is_ok()) {
        settle_changes(shell, key, answer);
    }
}

/// What a comparison found, or why it could not tell; the project's root is no change of its own.
pub(crate) fn settle_changes(shell: &mut Shell, key: Compared, answer: Result<Vec<Difference>, String>) {
    let v = &mut shell.versions;
    v.compared = Some(key);
    let (found, failed) = match answer {
        Ok(found) => (found.into_iter().filter(|d| d.class != archforge_core::MODEL).collect::<Vec<_>>(), None),
        Err(why) => (Vec::new(), Some(why)),
    };
    (v.working, v.changes, v.compare_failed) = (found.len(), found, failed);
}

/// Adds the next page to the list when it has come, or keeps why it could not be read.
fn take_more(v: &mut State) {
    let answer = match v.more_reading.as_ref().map(Receiver::try_recv) {
        Some(Ok(answer)) => answer,
        Some(Err(TryRecvError::Disconnected)) => Err("the read stopped without an answer".to_string()),
        _ => return,
    };
    v.more_reading = None;
    match (answer, v.page.as_mut()) {
        (Ok(next), Some(page)) => {
            // An empty page ends the list whatever it says, so reading on always stops.
            page.more = next.more && !next.versions.is_empty();
            page.versions.extend(next.versions);
        }
        (Ok(_), None) => {}
        (Err(why), _) => v.more_failed = Some(why),
    }
}

/// Takes the branches when they have come; a failure keeps those shown and says why. Gives whether
/// the list that came holds other names, or them in another order, than the one it replaced.
fn take_branches(v: &mut State) -> bool {
    let answer = match v.branches_reading.as_ref().map(Receiver::try_recv) {
        Some(Ok(answer)) => answer,
        Some(Err(TryRecvError::Disconnected)) => Err("the read stopped without an answer".to_string()),
        _ => return false,
    };
    v.branches_reading = None;
    let Ok((listed, cut)) = answer.map_err(|why| v.branches_failed = Some(why)) else { return false };
    let moved = v.branches.as_ref().is_none_or(|old| old.iter().map(|b| &b.name).ne(listed.iter().map(|b| &b.name)));
    (v.branches, v.branches_cut, v.branches_failed) = (Some(listed), cut, None);
    moved
}

/// Asks for the page after the versions listed; its button is off while a read is on its way.
pub(crate) fn read_more(shell: &mut Shell) {
    let v = &shell.versions;
    let after = v.page.as_ref().and_then(|page| page.versions.last()).map(|last| last.hash.clone());
    let (Some(host), Some((path, _)), Some(after)) = (shell.services.projects.clone(), v.about.clone(), after) else { return };
    let v = &mut shell.versions;
    (v.more_failed, v.more_reading) = (None, Some(host.versions(&path, Some(after), PAGE)));
}

/// Starts a read of the newest versions when a host can answer it: only then is it asked for. The
/// page on its way belonged to the list this replaces, so it goes. A read for a save's version
/// leaves the last version in doubt until one succeeds.
fn read(shell: &mut Shell) {
    let (Some(host), Some((path, _))) = (shell.services.projects.clone(), shell.versions.about.clone()) else { return };
    let v = &mut shell.versions;
    v.last_unsure |= v.stale;
    (v.failed, v.stale, v.asked, v.more_reading, v.more_failed, v.branches_asked) = (None, false, true, None, None, false);
    v.reading = Some(host.versions(&path, None, PAGE));
}

/// When the last version was saved, as the list says it; none before the first.
pub(crate) fn last_saved(v: &State, clock: Clock) -> Option<String> {
    v.page.as_ref()?.versions.first().map(|version| when(&version.date, clock))
}

/// The last version listed, by its hash; none before the first version or before the first read.
pub(crate) fn last_version(v: &State) -> Option<String> {
    v.page.as_ref().and_then(|page| page.versions.first()).map(|version| version.hash.clone())
}

/// Whether the first row listed is known to be the last version: no save or restore has ended
/// since the last read that succeeded began.
pub(crate) fn last_known(v: &State) -> bool {
    !v.stale && !v.last_unsure
}

/// Whether a comparison of what is shown now has finished, and none is on its way: an answer for
/// rows that have moved on may be on the list, but it does not say the list is current.
pub(crate) fn settled(shell: &Shell) -> bool {
    let v = &shell.versions;
    let now = shell.session.as_ref().map(|s| (last_version(v), s.revision()));
    last_known(v) && v.compared.is_some() && v.compared == now && v.comparing.is_none() && v.reading_base.is_none()
}

/// What Working changes says, when what is known allows it. "No changes" is what Save would find:
/// no difference in the model and diagrams, and none in what only a save writes (`settings` and
/// `model_tables`, which leave the session marked unsaved with nothing to list).
fn working_words(shell: &Shell) -> Option<String> {
    let page = shell.versions.page.as_ref()?;
    let untouched = shell.session.as_ref().is_some_and(|s| !s.is_dirty()) && !crate::panels::description_pending(&shell.panels);
    if page.versions.is_empty() {
        Some(tr("No versions yet. Save version records your first one."))
    } else if !settled(shell) {
        shell.viewing.is_none().then(|| tr("Loading"))
    } else if untouched && shell.versions.working == 0 {
        Some(tr("No changes since the last version."))
    } else {
        None
    }
}

/// Draws a section's header at `y`, from its id, label, whether it is open (none when it cannot
/// fold, which draws no chevron) and count, and gives whether it was toggled.
fn header(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, section: (&str, &str, Option<bool>, Option<String>)) -> bool {
    let (id, label, open, count) = section;
    let row = Rect::from_min_max(pos2(r.left() + 12.0, y + 6.0), pos2(r.right() - 12.0, y + HEADER - 6.0));
    let resp = control(shell, ui, &format!("workspace.versions.{id}"), "button", row, open.is_some());
    let _ = resp.clone().accessible_name(label);
    let paint = ui.painter();
    focus_ring(paint, p, row, 6.0, &resp);
    if let Some(open) = open {
        let chevron = Rect::from_center_size(pos2(r.left() + 24.0, row.center().y), Vec2::splat(12.0));
        ux::icon_in(paint, chevron, if open { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
    }
    let beside = if id == "working" { 150.0 } else { 80.0 };
    ux::text(paint, pos2(r.left() + 38.0, row.center().y - 9.5), label, types::UI, p.text, Some((r.width() - beside).max(0.0)));
    if let Some(n) = count {
        let g = ux::galley(paint, &n, types::CAPTION, p.text_muted, None);
        paint.galley(pos2(r.right() - 16.0 - g.size().x, row.center().y - g.size().y / 2.0), g, p.text_muted);
    }
    resp.clicked() && open.is_some()
}

/// A line across the column at `y`.
fn rule(ui: &egui::Ui, p: &Palette, r: Rect, y: f32) {
    ui.painter().rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width() - 1.0, 1.0)), 0.0, p.border);
}

/// The Versions explorer in the column `r`.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    ux::text(ui.painter(), r.min + vec2(20.0, 21.0), &tr("Versions").to_uppercase(), types::EYEBROW, p.text, Some((r.width() - 40.0).max(0.0)));
    let mut y = r.top() + 56.0;
    rule(ui, p, r, y);
    y = branches(shell, ui, p, r, y);
    rule(ui, p, r, y);
    y = working(shell, ui, p, r, y);
    rule(ui, p, r, y);
    history(shell, ui, p, r, y);
}

/// Branches from `y`: its header, folded while there is only `main`, each branch's row, scrolling
/// when they are many, and that branches can't be merged. Gives where the next section starts.
fn branches(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let mut listed = shell.versions.branches.clone().unwrap_or_default();
    // The mark moves with a switch at once; the rest of a list read before it stays until the next.
    let host = shell.services.projects.as_ref().zip(shell.versions.about.as_ref());
    if let Some(now) = host.and_then(|(host, (path, _))| host.current_branch(path)) {
        listed.iter_mut().for_each(|b| b.current = b.name.eq_ignore_ascii_case(&now));
    }
    // A failed read opens it, so that it says so.
    let open = shell.versions.branches_open.unwrap_or(listed.len() > 1 || shell.versions.branches_failed.is_some());
    let count = (!listed.is_empty()).then(|| listed.len().to_string());
    if header(shell, ui, p, r, y, ("branches", &tr("Branches"), Some(open), count)) {
        shell.versions.branches_open = Some(!open);
    }
    if crate::branch_ask::offered(shell) {
        let plus = Rect::from_center_size(pos2(r.right() - 48.0, y + HEADER / 2.0), Vec2::splat(24.0));
        let resp = control(shell, ui, "workspace.versions.branches.new", "button", plus, true).on_hover_text(tr("New branch…"));
        focus_ring(ui.painter(), p, plus, 6.0, &resp);
        ux::icon_in(ui.painter(), Rect::from_center_size(plus.center(), Vec2::splat(14.0)), &icons::PLUS, p.text_secondary);
        if resp.accessible_name(tr("New branch…")).clicked() {
            crate::branch_ask::open(shell);
        }
    }
    let y = y + HEADER;
    if !open {
        return y;
    }
    // A failed read's banner and the lines under the rows scroll with them, so a short column keeps
    // the list's first row in reach and History's header and a row under it.
    let failed = shell.versions.branches_failed.clone();
    let above = if failed.is_some() { 52.0 } else { 0.0 };
    let merged = tr("Branches can't be merged yet.");
    let n = listed.len().to_string();
    let cut = fmt(&tr("Only the first {n} branches are listed."), &[("n", &n)]);
    let words = if shell.versions.branches_cut { format!("{cut}\n{merged}") } else { merged };
    let g = ux::galley_rows(ui.painter(), &words, types::CAPTION, p.text_muted, Some((r.width() - 40.0).max(1.0)), 4);
    let (tail, rows) = (g.size().y + 12.0, listed.len() as f32 * BRANCH_PITCH);
    let least = LEAST_BRANCHES.min(above + rows);
    let height = (above + rows + tail + 8.0).min(r.height() * 0.3).min((r.bottom() - y - UNDER_BRANCHES).max(least));
    let tree = Rect::from_min_size(pos2(r.left(), y), vec2(r.width() - 1.0, height));
    // The row the keys moved to comes whole into view, under the banner too.
    let scroll = &mut shell.versions.branches_scroll;
    if let Some(i) = key_branches(ui, &listed) {
        let top = above + i as f32 * BRANCH_PITCH;
        scroll.offset = scroll.offset.min(top).max(top + BRANCH - height);
    }
    let offset = scroll_tree(ui, p, tree, y, listed.len(), BRANCH_PITCH, above + tail, true, None, scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    if let Some(why) = failed {
        let ids = ["workspace.versions.branches.failed", "workspace.versions.branches.retry"];
        banner(shell, &clip, p, (r, y - offset), ids, (&tr("The branches couldn't be read."), &why), |s| s.versions.branches_asked = false);
    }
    for (i, b) in listed.iter().enumerate() {
        let area = Rect::from_min_size(pos2(r.left() + 8.0, y + above + i as f32 * BRANCH_PITCH - offset), vec2(r.width() - 16.0, BRANCH));
        if area.intersects(tree) {
            branch_row(shell, &clip, p, (area, i), b);
        }
    }
    let note = Rect::from_min_size(pos2(r.left() + 20.0, y + above + rows - offset + 4.0), g.size());
    let _ = control(shell, &clip, "workspace.versions.branches.note", "status", note, false).accessible_name(words.as_str());
    clip.painter().galley(note.min, g, p.text_muted);
    tree.bottom() + 8.0
}

/// ↑ and ↓ on a focused branch row, the current one included, move the focus to the next or
/// previous row, and ↓ on a failed read's Retry to the first; gives the row moved to.
fn key_branches(ui: &egui::Ui, listed: &[BranchEntry]) -> Option<usize> {
    let row = |i: usize| widgets::egui_id(&format!("workspace.versions.branch.row.{}", listed[i].name));
    let retry = widgets::egui_id("workspace.versions.branches.retry");
    let on_retry = ui.memory(|m| m.has_focus(retry));
    let at = (0..listed.len()).find(|&i| ui.memory(|m| m.has_focus(row(i))));
    let pressed = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    let next = match at {
        _ if on_retry && pressed(egui::Key::ArrowDown) => (!listed.is_empty()).then_some(0),
        Some(at) if pressed(egui::Key::ArrowDown) => (at + 1 < listed.len()).then_some(at + 1),
        Some(at) if pressed(egui::Key::ArrowUp) => at.checked_sub(1),
        _ => None,
    };
    if let Some(i) = next {
        ui.memory_mut(|m| m.request_focus(row(i)));
    }
    // The keys are the list's, not egui's to move focus by place; only what has focus takes this.
    let arrows = egui::EventFilter { vertical_arrows: true, ..Default::default() };
    let held = next.or(at).map(row).or(on_retry.then_some(retry));
    if let Some(id) = held {
        ui.memory_mut(|m| m.set_focus_lock_filter(id, arrows));
    }
    next
}

/// One branch in `area`, the `i`th: its name, Current on the branch in use, a dot when it holds
/// unsaved changes to the model, and when its latest version was saved. A click or Enter on
/// another branch switches to it, as its menu's Switch does.
fn branch_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, (area, i): (Rect, usize), b: &BranchEntry) {
    // The branch in use shows its session's state as it changes; another's waits for a switch.
    let unsaved = if b.current { shell.shown_unsaved() } else { b.unsaved };
    let date = b.latest.as_deref().map(|d| when(d, crate::chat_panel::clock(shell)));
    let current = tr_ctx("branch", "Current");
    let said = [Some(b.shown.clone()), b.current.then(|| current.clone()), unsaved.then(|| tr("Unsaved changes")), date.clone()];
    // The name last, as a name may hold a dot.
    let id = |part: &str| format!("workspace.versions.branch.{part}.{}", b.name);
    // The current row takes focus too, as the keys' way into the list, but switches to nothing.
    let resp = if b.current {
        widgets::record_visible(ui, &mut shell.registry, &id("row"), "row", area);
        ui.interact(area, widgets::egui_id(&id("row")), egui::Sense::focusable_noninteractive())
    } else {
        control(shell, ui, &id("row"), "row", area, true)
    };
    let _ = resp.clone().accessible_name(said.into_iter().flatten().collect::<Vec<_>>().join(", "));
    let menu = crate::panels::Menu::Branch(i);
    let paint = ui.painter();
    if b.current {
        paint.rect_filled(area, 8.0, p.accent_tint);
    }
    focus_ring(paint, p, area, 8.0, &resp);
    let mid = area.center().y;
    ux::icon_in(paint, Rect::from_center_size(pos2(area.left() + 16.0, mid), Vec2::splat(14.0)), &icons::BRANCH, p.text_secondary);
    // From the right: the menu, the date and the dot, then the name with what room is left.
    let mut right = area.right() - if b.current { 10.0 } else { 36.0 };
    if let Some(date) = &date {
        let g = ux::galley(paint, date, types::CAPTION, p.text_muted, None);
        right -= g.size().x;
        paint.galley(pos2(right, mid - g.size().y / 2.0), g, p.text_muted);
        right -= 10.0;
    }
    if unsaved {
        paint.circle_filled(pos2(right - 3.0, mid), 3.0, p.amber);
        right -= 14.0;
    }
    let ink = if b.current { p.accent } else { p.text };
    let name = ux::text(paint, pos2(area.left() + 32.0, mid - 9.5), &b.shown, types::UI, ink, Some((right - area.left() - 32.0).max(0.0)));
    if b.current {
        let room = (right - name.right() - 8.0).max(0.0);
        ux::text(paint, pos2(name.right() + 8.0, mid - 8.0), &current, types::CAPTION, p.text_muted, Some(room));
    } else if resp.clicked() {
        switch_to(shell, b.name.clone());
    }
    if b.current {
        return;
    }
    let more = Rect::from_center_size(pos2(area.right() - 18.0, mid), Vec2::splat(28.0));
    let open = shell.panels.menu == Some(menu);
    let more_resp = control(shell, ui, &id("more"), "button", more, true).on_hover_text(tr("More"));
    if more_resp.hovered() || open {
        ui.painter().rect_filled(more, 6.0, p.page);
    }
    ux::icon_mid(ui.painter(), more.center(), &icons::MORE, if more_resp.hovered() || open { p.text_secondary } else { p.text_muted });
    if more_resp.clicked() {
        shell.panels.menu = if open { None } else { Some(menu) };
    }
    // Rename and Delete join Switch with AR-292.
    let anchor = Rect::from_min_max(pos2(more.right() - 180.0, more.top()), more.max);
    if shell.panels.menu == Some(menu) && crate::panels::menu(shell, ui, p, anchor, &id("menu"), &[tr("Switch")]) == Some(0) {
        switch_to(shell, b.name.clone());
    }
}

/// Asks to go to the branch stored as `name`; a refusal says why over the canvas. Gives whether it went ahead.
pub(crate) fn switch_to(shell: &mut Shell, name: String) -> bool {
    if let Err(why) = shell.ask_switch_branch(name) {
        let words = crate::version_chip::refused_words(shell, why);
        shell.canvas.say_refused_step(words);
        return false;
    }
    true
}

/// Working changes from `y`: its header and what can be said so far. Gives where the next
/// section starts.
fn working(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let open = !shell.versions.working_folded;
    if header(shell, ui, p, r, y, ("working", &tr("Working changes"), Some(open), None)) {
        shell.versions.working_folded = open;
    }
    let label = tr("Save version…");
    let width = (ux::galley(ui.painter(), &label, types::UI, p.text, None).size().x + 20.0).ceil();
    let off = crate::save_version::off_because(shell);
    let at = pos2(r.right() - 16.0 - width, y + 6.0);
    let look = if off.is_none() { Look::Primary } else { Look::Secondary };
    let (_, resp) = button(shell, ui, p, "workspace.versions.save", at, &label, look, off.is_none(), off.as_deref());
    if resp.clicked() {
        crate::save_version::open(shell);
    }
    // The read after a save turns the button off for a moment, and an off button takes no keyboard.
    if shell.versions.focus_save && !crate::save_version::waiting(shell) {
        shell.versions.focus_save = false;
        resp.request_focus();
    }
    let next = y + HEADER;
    if !open {
        return next;
    }
    if !shell.versions.changes.is_empty() || shell.versions.compare_failed.is_some() {
        return changes(shell, ui, p, r, next);
    }
    let Some(words) = working_words(shell) else { return next };
    let g = ux::galley_rows(ui.painter(), &words, types::SMALL, p.text_secondary, Some((r.width() - 40.0).max(1.0)), 3);
    let h = g.size().y;
    // Only where History's header and a row still fit under them.
    if r.bottom() - next - h - 21.0 < UNDER_CHANGES {
        return next;
    }
    ui.painter().galley(pos2(r.left() + 20.0, next), g, p.text_secondary);
    next + h + 21.0
}

/// The changes from `y`, model first and then diagrams, with a filter when they are many, or why
/// they could not be told. Gives where the next section starts.
fn changes(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    if let Some(why) = shell.versions.compare_failed.clone() {
        let words = tr("The changes couldn't be compared.");
        let area = Rect::from_min_size(pos2(r.left() + 20.0, y), vec2((r.width() - 40.0).max(0.0), 21.0));
        let _ = control(shell, ui, "workspace.versions.changes.failed", "status", area, false).on_hover_text(why).accessible_name(&words);
        ux::text(ui.painter(), area.min, &words, types::SMALL, p.danger, Some(area.width()));
        return y + 29.0;
    }
    // Nothing, not even the filter, where it, a heading and a change wouldn't fit over History.
    let filter = if shell.versions.changes.len() > FILTER_FROM { 38.0 } else { 0.0 };
    if r.bottom() - y - UNDER_CHANGES < filter + GROUP + CHANGE_PITCH {
        return y;
    }
    let mut top = y;
    if shell.versions.changes.len() > FILTER_FROM {
        filter_field(shell, ui, p, Rect::from_min_size(pos2(r.left() + 16.0, y), vec2((r.width() - 32.0).max(0.0), 30.0)));
        top += 38.0;
    }
    // Only while the field is there to clear it.
    let needle = if shell.versions.changes.len() > FILTER_FROM { shell.versions.filter.to_lowercase() } else { String::new() };
    let holds = |d: &&Difference| needle.is_empty() || d.name.as_deref().unwrap_or("").to_lowercase().contains(&needle);
    let group = |area: Area| shell.versions.changes.iter().filter(|d| d.area == area).filter(holds).cloned().collect::<Vec<_>>();
    let groups = [(tr("Model elements"), group(Area::Model)), (tr("Diagrams"), group(Area::Diagram))];
    let content: f32 = groups.iter().filter(|(_, g)| !g.is_empty()).map(|(_, g)| GROUP + g.len() as f32 * CHANGE_PITCH).sum();
    let height = (content + 8.0).min(r.height() * MOST_OF_COLUMN).min((r.bottom() - top - UNDER_CHANGES).max(0.0));
    let tree = Rect::from_min_size(pos2(r.left(), top), vec2(r.width() - 1.0, height));
    let offset = scroll_tree(ui, p, tree, top, content as usize, 1.0, 0.0, true, None, &mut shell.versions.changes_scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    let mut at = top - offset;
    for (label, group) in groups.iter().filter(|(_, g)| !g.is_empty()) {
        ux::text(clip.painter(), pos2(r.left() + 20.0, at), &label.to_uppercase(), types::EYEBROW, p.text_secondary, None);
        let g = ux::galley(clip.painter(), &group.len().to_string(), types::CAPTION, p.text_muted, None);
        clip.painter().galley(pos2(r.right() - 16.0 - g.size().x, at), g, p.text_muted);
        at += GROUP;
        for d in group {
            let area = Rect::from_min_max(pos2(r.left() + 8.0, at), pos2(r.right() - 8.0, at + CHANGE));
            // By class too: an element that changes table has two rows.
            change_row(shell, &clip, p, area, &format!("workspace.versions.change.{}.{}", d.id, d.class.0), d, true);
            at += CHANGE_PITCH;
        }
    }
    tree.bottom() + 8.0
}

/// The field that narrows a long list by name.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 17.0, field.center().y), &icons::SEARCH, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.versions.filter", "field", field);
    let inner = crate::panels::field_inner(ui, field.with_min_x(field.left() + 22.0));
    let words = tr("Filter changes…");
    let hint = egui::RichText::new(words.clone()).color(p.text_secondary).font(types::SMALL.font());
    let edit = egui::TextEdit::singleline(&mut shell.versions.filter)
        .id(widgets::egui_id("workspace.versions.filter"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(hint)
        .char_limit(256)
        .desired_width(inner.width());
    let _ = ui.put(inner, edit).accessible_name(words);
}

/// What a change is called, and its ink and tint.
pub(crate) fn kind_look(kind: ChangeKind, p: &Palette) -> (String, Color32, Color32) {
    match kind {
        ChangeKind::Added => (tr("Added"), p.green, p.green_tint),
        ChangeKind::Changed => (tr("Changed"), p.accent, p.accent_tint),
        ChangeKind::Removed => (tr("Removed"), p.danger, p.danger_tint),
    }
}

/// One change in `area`, registered as `id`: its name, where it is, and a letter for what happened
/// to it; `live` makes it open the element in the centre and marks the open one as selected.
/// Nothing out of sight is drawn or registered.
pub(crate) fn change_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, area: Rect, id: &str, d: &Difference, live: bool) {
    if !area.intersects(ui.clip_rect()) {
        return;
    }
    let name = d.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| tr("Unnamed"));
    let (what, ink, tint) = kind_look(d.kind, p);
    let path = d.path.join(" / ");
    let selected = live && shell.versions.element.as_ref().is_some_and(|o| o.row.id == d.id);
    let words = format!("{name}, {path}, {what}");
    let resp = control(shell, ui, id, "row", area, live);
    let _ = resp.clone().accessible_name(if selected { fmt(&tr("{name}, selected"), &[("name", &words)]) } else { words });
    if resp.clicked() && live {
        crate::change_view::open(shell, d.clone());
    }
    let paint = ui.painter();
    if selected {
        paint.rect_filled(area, 8.0, p.accent_tint);
    }
    focus_ring(paint, p, area, 8.0, &resp);
    let room = (area.width() - 76.0).max(0.0);
    let icon = if d.area == Area::Diagram { &icons::DIAGRAM } else { &icons::CUBE };
    ux::icon_in(paint, Rect::from_center_size(pos2(area.left() + 20.0, area.center().y), Vec2::splat(16.0)), icon, p.text_secondary);
    let mid = area.center().y;
    ux::text(paint, pos2(area.left() + 38.0, mid - 17.0), &name, types::UI, if selected { p.accent } else { p.text }, Some(room));
    ux::text(paint, pos2(area.left() + 38.0, mid + 1.0), &path, types::CAPTION, p.text_muted, Some(room));
    let letter = what.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    let mark = Rect::from_center_size(pos2(area.right() - 20.0, area.center().y), Vec2::splat(20.0));
    paint.rect_filled(mark, 5.0, if selected { p.surface } else { tint });
    let g = ux::galley(paint, &letter, types::CAPTION_STRONG, ink, None);
    paint.galley(mark.center() - g.size() / 2.0, g, ink);
}

/// History from `y`: its header, the banner of a failed read, and the bars, the words or the rows.
fn history(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) {
    let count = shell.versions.page.as_ref().filter(|page| !page.versions.is_empty()).map(|page| {
        let more = if page.more { "+" } else { "" };
        format!("{}{more}", page.versions.len())
    });
    let open = !shell.versions.history_folded;
    if header(shell, ui, p, r, y, ("history", &tr("History"), Some(open), count)) {
        shell.versions.history_folded = open;
    }
    if !open {
        return;
    }
    let mut top = y + HEADER;
    if let Some(why) = shell.versions.failed.clone() {
        let ids = ["workspace.versions.failed", "workspace.versions.retry"];
        top = banner(shell, ui, p, (r, top), ids, (&tr("The versions couldn't be read."), &why), read);
    }
    if let Some(failed) = shell.versions.load_failed.clone() {
        let (ids, why) = (["workspace.versions.load.failed", "workspace.versions.load.retry"], failed.why.clone());
        top = banner(shell, ui, p, (r, top), ids, (&tr("This version couldn't be read."), &why), |s| retry_load(s, failed));
    }
    match shell.versions.page.as_ref().map(|page| page.versions.clone()) {
        None if shell.versions.reading.is_some() => skeleton(shell, ui, p, r, top),
        None => {}
        Some(rows) if rows.is_empty() => {
            ux::text(
                ui.painter(),
                pos2(r.left() + 20.0, top),
                &tr("No versions yet."),
                types::SMALL,
                p.text_muted,
                Some((r.width() - 40.0).max(0.0)),
            );
        }
        Some(rows) => list(shell, ui, p, r, top, &rows),
    }
}

/// Asks again for the version whose read failed. Asking ends its banner, so a refusal leaves it
/// standing and says why.
fn retry_load(shell: &mut Shell, failed: LoadFailed) {
    let done = shell.move_to_version(&failed.commit, failed.label);
    let words = done.err().and_then(|why| crate::version_chip::refused_words(shell, why));
    shell.canvas.say_refused_step(words);
}

/// A banner of a read that failed, saying `words` with `why` on hover, and a Retry that runs
/// `retry`; what was shown stays under it. `ids` are its own and its button's. Gives where the rows
/// start.
fn banner(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    (r, y): (Rect, f32),
    ids: [&str; 2],
    (words, why): (&str, &str),
    retry: impl FnOnce(&mut Shell),
) -> f32 {
    let area = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, y + 44.0));
    let paint = ui.painter();
    paint.rect_filled(area, 8.0, p.danger_tint);
    ux::icon_in(paint, Rect::from_center_size(pos2(area.left() + 18.0, area.center().y), Vec2::splat(14.0)), &icons::WARNING, p.danger);
    let label = tr("Retry");
    let width = (ux::galley(paint, &label, types::UI, p.text, None).size().x + 20.0).ceil();
    ux::text(paint, pos2(area.left() + 34.0, area.center().y - 9.5), words, types::UI, p.danger, Some((area.width() - 52.0 - width).max(0.0)));
    let _ = control(shell, ui, ids[0], "status", area, false).on_hover_text(why).accessible_name(words);
    let at = pos2(area.right() - 8.0 - width, area.top() + 8.0);
    if button(shell, ui, p, ids[1], at, &label, Look::Secondary, true, None).1.clicked() {
        retry(shell);
    }
    y + 52.0
}

/// Bars in the place of the rows, while the first read is on its way.
fn skeleton(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32) {
    let width = (r.width() - 36.0).max(0.0);
    let area = Rect::from_min_size(pos2(r.left() + 8.0, y), vec2(r.width() - 16.0, SKELETON_ROWS as f32 * PITCH));
    let _ = control(shell, ui, "workspace.versions.loading", "status", area, false).accessible_name(tr("Loading"));
    for i in 0..SKELETON_ROWS {
        let top = y + i as f32 * PITCH + 10.0;
        ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + 18.0, top), vec2(width * 0.6, 10.0)), 4.0, p.border);
        ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + 18.0, top + 17.0), vec2(width * 0.36, 8.0)), 4.0, p.border);
    }
}

/// The versions, newest first, scrolling under the headers: a long list has the filter over it
/// and Show more, or why the next page could not be read, under it. Over the first version `main`
/// has too, a divider says the line is shared from there; until a page holds one, there is none.
fn list(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, top: f32, rows: &[VersionEntry]) {
    let more = shell.versions.page.as_ref().is_some_and(|page| page.more);
    let mut top = top;
    if more || rows.len() >= PAGE || shell.versions.described_only {
        let (ids, chosen) = (["workspace.versions.all", "workspace.versions.described"], usize::from(shell.versions.described_only));
        let labels = [tr("All"), tr("With a description")];
        if let Some(pick) =
            crate::settings_agents::segmented(shell, ui, p, ids, pos2(r.left() + 16.0, top), labels, chosen, [true, true], [None, None])
        {
            (shell.versions.described_only, shell.versions.scroll) = (pick == 1, TreeScroll::default());
        }
        top += 38.0;
    }
    let described_only = shell.versions.described_only;
    let shown: Vec<&VersionEntry> = rows.iter().filter(|v| !described_only || described(v).is_some()).collect();
    let footer = if more || shell.versions.more_failed.is_some() { 52.0 } else { 8.0 };
    let split = shown.iter().position(|v| v.shared);
    let divider = if split.is_some() { SHARED_PITCH } else { 0.0 };
    let tree = Rect::from_min_max(pos2(r.left(), top), pos2(r.right() - 1.0, r.bottom()));
    let offset = scroll_tree(ui, p, tree, top, shown.len(), PITCH, footer + divider, true, None, &mut shell.versions.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    let clock = crate::chat_panel::clock(shell);
    // The rows from the split on sit under the divider.
    let at = |i: usize| top + i as f32 * PITCH - offset + if split.is_some_and(|s| i >= s) { SHARED_PITCH } else { 0.0 };
    if let Some(s) = split {
        shared_divider(shell, &clip, p, r, at(s) - SHARED_PITCH);
    }
    for (i, v) in shown.iter().enumerate() {
        let y = at(i);
        if y + ROW >= tree.top() && y <= tree.bottom() {
            row(shell, &clip, p, (r, y), v, clock);
        }
    }
    if shown.is_empty() {
        let words = tr("None of these versions has a description.");
        ux::text(clip.painter(), pos2(r.left() + 20.0, top + 6.0), &words, types::SMALL, p.text_muted, Some((r.width() - 40.0).max(0.0)));
    }
    let y = top + shown.len().max(1) as f32 * PITCH + divider - offset;
    footer_row(shell, &clip, p, r, y, more);
}

/// The divider at `y`: its words, then a line to the column's edge. It claims no more than what is
/// shared, as Dolt does not record which branch a branch was made from.
fn shared_divider(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32) {
    if y + SHARED < ui.clip_rect().top() || y > ui.clip_rect().bottom() {
        return;
    }
    let words = tr("Shared with main up to here");
    let at = pos2(r.left() + 20.0, y + 1.5);
    let text = ux::text(ui.painter(), at, &words, types::CAPTION, p.text_muted, Some((r.width() - 48.0).max(0.0)));
    let area = Rect::from_min_max(pos2(at.x, y), pos2(r.right() - 20.0, y + SHARED));
    let _ = control(shell, ui, "workspace.versions.shared", "status", area, false).accessible_name(words.as_str());
    if text.right() + 8.0 < area.right() {
        let line = Rect::from_min_max(pos2(text.right() + 8.0, y + SHARED / 2.0 - 0.5), pos2(area.right(), y + SHARED / 2.0 + 0.5));
        ui.painter().rect_filled(line, 0.0, p.border);
    }
}

/// Under the list: why the next page could not be read, with Retry for that page; or Show more.
fn footer_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, more: bool) {
    let at = pos2(r.left() + 18.0, y + 4.0);
    if let Some(why) = shell.versions.more_failed.clone() {
        let (words, label) = (tr("The next versions couldn't be read."), tr("Retry"));
        let width = (ux::galley(ui.painter(), &label, types::UI, p.text, None).size().x + 20.0).ceil();
        let area = Rect::from_min_size(at, vec2((r.width() - 44.0 - width).max(0.0), 28.0));
        control(shell, ui, "workspace.versions.more.failed", "status", area, false).on_hover_text(why);
        ux::text(ui.painter(), at + vec2(0.0, 5.0), &words, types::SMALL, p.danger, Some(area.width()));
        if button(shell, ui, p, "workspace.versions.more.retry", pos2(r.right() - 18.0 - width, at.y), &label, Look::Secondary, true, None)
            .1
            .clicked()
        {
            read_more(shell);
        }
    } else if more {
        let live = shell.versions.more_reading.is_none() && shell.versions.reading.is_none();
        if button(shell, ui, p, "workspace.versions.more", at, &tr("Show more"), Look::Secondary, live, None).1.clicked() && live {
            read_more(shell);
        }
    }
}

/// When `date`, as the project stores it, was, in the language shown; never the stored text.
pub(crate) fn when(date: &str, clock: Clock) -> String {
    seconds_of(date).map_or_else(|| tr("Date not known"), |secs| date_and_time(secs, clock))
}

/// What a version was described as, when it was.
pub(crate) fn described(v: &VersionEntry) -> Option<&str> {
    v.description.as_deref().filter(|d| !d.trim().is_empty())
}

/// A version as a select or a link names it: its date, then what it was described as when it was.
pub(crate) fn dated(v: &VersionEntry, clock: Clock) -> String {
    let date = when(&v.date, clock);
    described(v).map_or(date.clone(), |d| fmt(&tr("{when} · {who}"), &[("when", &date), ("who", d)]))
}

/// A version's two lines: its description and when and by whom, or without a description its date
/// and who asked when an agent did, else "No description".
pub(crate) fn lines(v: &VersionEntry, clock: Clock) -> (String, String) {
    let date = when(&v.date, clock);
    let who = if v.by_agent { Some(tr("Saved by an agent")) } else { v.author.clone().filter(|a| !a.is_empty()) };
    match described(v) {
        Some(d) => (d.to_string(), who.map_or(date.clone(), |w| fmt(&tr("{when} · {who}"), &[("when", &date), ("who", &w)]))),
        None if v.by_agent => (date, tr("Saved by an agent")),
        None => (date, tr("No description")),
    }
}

/// One version at `y`: its two lines, or for one that cannot be shown that it cannot, with its
/// reason behind "Why?", a warning at the right, and the reason in what a screen reader says; one
/// of a newer Archforge says that and its date, as no reason would add to it.
fn row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, (r, y): (Rect, f32), v: &VersionEntry, clock: Clock) {
    let area = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, y + ROW));
    let why = shell.versions.unavailable.get(&v.hash).cloned();
    let (title, meta) = match &why {
        Some(Some(_)) => {
            (tr("This version can't be shown"), fmt(&tr("{when} · {who}"), &[("when", &when(&v.date, clock)), ("who", "")]).trim_end().to_string())
        }
        Some(None) => (tr("Made with a newer Archforge."), when(&v.date, clock)),
        None => lines(v, clock),
    };
    let room = (area.width() - 20.0 - if why.is_some() { 26.0 } else { 0.0 }).max(0.0);
    let name = why.as_ref().and_then(Option::as_ref).map_or_else(|| format!("{title}, {meta}"), |why| format!("{title}, {meta} {why}"));
    let id = format!("workspace.versions.row.{}", v.hash);
    let selected = shell.versions.selected.as_deref() == Some(v.hash.as_str());
    let name = if selected { fmt(&tr("{name}, selected"), &[("name", &name)]) } else { name };
    let resp = control(shell, ui, &id, "row", area, true);
    let _ = resp.clone().accessible_name(name);
    if resp.clicked() {
        crate::version_details::open(shell, v.clone());
    }
    let paint = ui.painter();
    if selected {
        paint.rect_filled(area, 8.0, p.accent_tint);
    }
    focus_ring(paint, p, area, 8.0, &resp);
    let ink = if why.is_some() {
        p.text_muted
    } else if selected {
        p.accent
    } else {
        p.text
    };
    ux::text(paint, pos2(area.left() + 10.0, y + 7.0), &title, types::UI, ink, Some(room));
    let g = ux::galley(paint, &meta, types::CAPTION, p.text_muted, Some(room));
    let after = pos2(area.left() + 10.0 + g.size().x, y + 25.0);
    paint.galley(pos2(area.left() + 10.0, y + 25.0), g, p.text_muted);
    let Some(why) = why else { return };
    ux::icon_in(paint, Rect::from_center_size(pos2(area.right() - 18.0, area.center().y), Vec2::splat(14.0)), &icons::WARNING, p.text_secondary);
    let Some(why) = why else { return };
    let word = tr("Why?");
    let g = ux::galley(paint, &word, types::CAPTION, p.text_secondary, None);
    let hit = Rect::from_min_size(after + vec2(4.0, 0.0), g.size());
    let resp = control(shell, ui, &format!("{id}.why"), "button", hit, true).on_hover_text(&why);
    let _ = resp.clone().accessible_name(format!("{word} {why}"));
    if resp.has_focus() {
        resp.show_tooltip_text(&why);
    }
    focus_ring(ui.painter(), p, hit, 3.0, &resp);
    ui.painter().galley(hit.min, g, p.text_secondary);
}

/// What the rail's entry says: its name, and how many changes there are when there are any.
pub(crate) fn rail_words(working: usize) -> String {
    match working {
        0 => tr("Versions"),
        n => trn("Versions · {n} change", "Versions · {n} changes", n as u64),
    }
}

/// Draws the count on the rail's button `button`, when there is one.
pub(crate) fn rail_badge(shell: &mut Shell, ui: &egui::Ui, p: &Palette, button: Rect) {
    let n = shell.versions.working;
    if n == 0 {
        return;
    }
    let text = if n > 99 { "99+".to_string() } else { n.to_string() };
    let paint = ui.painter();
    let g = ux::galley(paint, &text, types::CAPTION_STRONG, p.on_accent, None);
    let badge = Rect::from_center_size(pos2(button.right() - 8.0, button.top() + 6.0), vec2((g.size().x + 8.0).max(16.0), 16.0));
    paint.rect_filled(badge, 8.0, p.accent_fill);
    paint.galley(badge.center() - g.size() / 2.0, g, p.on_accent);
    crate::widgets::record_visible(ui, &mut shell.registry, "workspace.explorers.source-control.badge", "badge", badge);
}
