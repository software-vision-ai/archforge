//! The Versions explorer (AR-255, from page 09 of the design and sections 2, 10 and 11 of
//! `docs/design/versioning.md`): Branches, Working changes and History, the versions read off the
//! writer a page at a time. A read keeps what was shown until its answer comes, and one for
//! another project or session is dropped by the next. Nothing here opens a version yet.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Rect, Vec2, pos2, vec2};

use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::tr;
use crate::settings::{Look, button, control, focus_ring};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, VersionEntry, VersionsPage};

/// The versions read at a time.
const PAGE: usize = 50;
/// A section's header, and a version's row and the distance between rows.
const HEADER: f32 = 40.0;
const ROW: f32 = 48.0;
const PITCH: f32 = 50.0;
/// The bars a first read shows in place of the rows.
const SKELETON_ROWS: usize = 6;

/// What the panel keeps between frames.
#[derive(Default)]
pub struct State {
    /// The project and session the versions were asked for.
    asked: Option<(PathBuf, u64)>,
    reading: Option<Receiver<Result<VersionsPage, String>>>,
    page: Option<VersionsPage>,
    /// Why the last read failed; what was shown stays.
    failed: Option<String>,
    /// A save made a version since the read.
    pub(crate) stale: bool,
    working_folded: bool,
    history_folded: bool,
    scroll: TreeScroll,
}

/// Asks for the versions when the project or the session changed or a save made one, and takes
/// the answer that has come.
fn refresh(shell: &mut Shell, ctx: &egui::Context) {
    let asked = shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()));
    if asked != shell.versions.asked || shell.versions.stale {
        // Another project's versions are not this one's, even for a moment.
        if shell.versions.asked.as_ref().map(|a| &a.0) != asked.as_ref().map(|a| &a.0) {
            shell.versions.page = None;
            shell.versions.scroll = TreeScroll::default();
        }
        shell.versions.asked = asked;
        shell.versions.stale = false;
        read(shell);
    }
    let answer = match shell.versions.reading.as_ref().map(Receiver::try_recv) {
        None => return,
        Some(Err(TryRecvError::Empty)) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        Some(Ok(answer)) => answer,
        Some(Err(TryRecvError::Disconnected)) => Err("the read stopped without an answer".to_string()),
    };
    shell.versions.reading = None;
    match answer {
        Ok(page) => shell.versions.page = Some(page),
        Err(why) => shell.versions.failed = Some(why),
    }
}

/// Starts a read of the newest versions, in place of any on its way.
fn read(shell: &mut Shell) {
    shell.versions.failed = None;
    let path = shell.versions.asked.as_ref().map(|a| a.0.clone());
    shell.versions.reading = match (&shell.services.projects, path) {
        (Some(host), Some(path)) => Some(host.versions(&path, None, PAGE)),
        _ => None,
    };
}

/// What Working changes says, when what is known allows it. Until AR-269 counts the changes,
/// "No changes" is said only of a project with no unsaved work.
fn working_words(shell: &Shell) -> Option<String> {
    let page = shell.versions.page.as_ref()?;
    let untouched = shell.session.as_ref().is_some_and(|s| !s.is_dirty()) && !crate::panels::description_pending(&shell.panels);
    if page.versions.is_empty() {
        Some(tr("No versions yet. Save version records your first one."))
    } else if untouched {
        Some(tr("No changes since the last version."))
    } else {
        None
    }
}

/// Draws a section's header at `y`, from its id, label, whether it is open, whether it can fold and
/// its count, and gives whether it was toggled.
fn header(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, section: (&str, &str, bool, bool, Option<usize>)) -> bool {
    let (id, label, open, live, count) = section;
    let row = Rect::from_min_max(pos2(r.left(), y), pos2(r.right(), y + HEADER));
    let hit = Rect::from_min_max(pos2(r.left() + 12.0, y + 6.0), pos2(r.right() - 12.0, y + HEADER - 6.0));
    let resp = control(shell, ui, &format!("workspace.versions.{id}"), "button", hit, live);
    let _ = resp.clone().accessible_name(label);
    let paint = ui.painter();
    focus_ring(paint, p, hit, 6.0, &resp);
    let chevron = Rect::from_center_size(pos2(r.left() + 24.0, row.center().y), Vec2::splat(12.0));
    ux::icon_in(paint, chevron, if open { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
    ux::text(paint, pos2(r.left() + 38.0, row.center().y - 9.5), label, types::UI, p.text, Some((r.width() - 80.0).max(0.0)));
    if let Some(n) = count {
        let g = ux::galley(paint, &n.to_string(), types::CAPTION, p.text_muted, None);
        paint.galley(pos2(r.right() - 16.0 - g.size().x, row.center().y - g.size().y / 2.0), g, p.text_muted);
    }
    resp.clicked() && live
}

/// A line across the column at `y`.
fn rule(ui: &egui::Ui, p: &Palette, r: Rect, y: f32) {
    ui.painter().rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width() - 1.0, 1.0)), 0.0, p.border);
}

/// The Versions explorer in the column `r`.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    refresh(shell, ui.ctx());
    let eyebrow = tr("Versions").to_uppercase();
    ux::text(ui.painter(), r.min + vec2(20.0, 21.0), &eyebrow, types::EYEBROW, p.text, Some((r.width() - 40.0).max(0.0)));
    let mut y = r.top() + 56.0;
    rule(ui, p, r, y);
    // Folded and not to be opened while the project has only `main`; AR-292 adds its list and +.
    header(shell, ui, p, r, y, ("branches", &tr("Branches"), false, false, None));
    y += HEADER;
    rule(ui, p, r, y);
    y = working(shell, ui, p, r, y);
    rule(ui, p, r, y);
    history(shell, ui, p, r, y);
}

/// Working changes from `y`: its header and what can be said so far. Gives where the next
/// section starts.
fn working(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let open = !shell.versions.working_folded;
    if header(shell, ui, p, r, y, ("working", &tr("Working changes"), open, true, None)) {
        shell.versions.working_folded = open;
    }
    let next = y + HEADER;
    let Some(words) = working_words(shell).filter(|_| open) else { return next };
    let g = ux::galley_rows(ui.painter(), &words, types::SMALL, p.text_secondary, Some((r.width() - 40.0).max(1.0)), 3);
    let h = g.size().y;
    ui.painter().galley(pos2(r.left() + 20.0, next), g, p.text_secondary);
    next + h + 21.0
}

/// History from `y`: its header, the banner of a failed read, and the bars, the words or the rows.
fn history(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) {
    let open = !shell.versions.history_folded;
    let count = shell.versions.page.as_ref().map(|page| page.versions.len()).filter(|n| *n > 0);
    if header(shell, ui, p, r, y, ("history", &tr("History"), open, true, count)) {
        shell.versions.history_folded = open;
    }
    if !open {
        return;
    }
    let mut top = y + HEADER;
    if let Some(why) = shell.versions.failed.clone() {
        top = banner(shell, ui, p, r, top, &why);
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

/// The banner of a read that failed, with Retry; what was shown stays under it. Gives where the
/// rows start.
fn banner(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, why: &str) -> f32 {
    let area = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, y + 44.0));
    let paint = ui.painter();
    paint.rect_filled(area, 8.0, p.danger_tint);
    ux::icon_in(paint, Rect::from_center_size(pos2(area.left() + 18.0, area.center().y), Vec2::splat(14.0)), &icons::WARNING, p.danger);
    let label = tr("Retry");
    let width = (ux::galley(paint, &label, types::UI, p.text, None).size().x + 20.0).ceil();
    let words = tr("The versions couldn't be read.");
    ux::text(paint, pos2(area.left() + 34.0, area.center().y - 9.5), &words, types::UI, p.danger, Some((area.width() - 52.0 - width).max(0.0)));
    control(shell, ui, "workspace.versions.failed", "status", area, false).on_hover_text(why);
    let at = pos2(area.right() - 8.0 - width, area.top() + 8.0);
    if button(shell, ui, p, "workspace.versions.retry", at, &label, Look::Secondary, true, None).1.clicked() {
        read(shell);
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

/// The versions, newest first, scrolling under the headers. A row is its description, or its date
/// when it has none, until AR-338 gives it its two lines.
fn list(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, top: f32, rows: &[VersionEntry]) {
    let tree = Rect::from_min_max(pos2(r.left(), top), pos2(r.right() - 1.0, r.bottom()));
    let offset = scroll_tree(ui, p, tree, top, rows.len(), PITCH, 8.0, true, None, &mut shell.versions.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, v) in rows.iter().enumerate() {
        let y = top + i as f32 * PITCH - offset;
        if y + ROW < tree.top() || y > tree.bottom() {
            continue;
        }
        let area = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, y + ROW));
        let title = v.description.as_deref().filter(|d| !d.trim().is_empty()).unwrap_or(&v.date);
        let _ = control(shell, &clip, &format!("workspace.versions.row.{}", v.hash), "row", area, false).accessible_name(title);
        ux::text(clip.painter(), pos2(area.left() + 10.0, y + 14.0), title, types::UI, p.text, Some((area.width() - 20.0).max(0.0)));
    }
}
