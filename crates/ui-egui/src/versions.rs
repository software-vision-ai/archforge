//! The Versions explorer (AR-255, from page 09 of the design and sections 2, 10 and 11 of
//! `docs/design/versioning.md`): Branches, Working changes and History, the versions read off the
//! writer a page at a time. A read keeps what was shown until its answer comes, and one for
//! another project or session is dropped by the next. Nothing here opens a version yet.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Rect, Vec2, pos2, vec2};

use crate::chat_words::{Clock, date_and_time, seconds_of};
use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::{fmt, tr, trn};
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
    /// The project and session the panel is about.
    about: Option<(PathBuf, u64)>,
    /// Whether a read has started for them.
    asked: bool,
    reading: Option<Receiver<Result<VersionsPage, String>>>,
    page: Option<VersionsPage>,
    /// Why the last read failed; what was shown stays.
    failed: Option<String>,
    /// A save made a version since the read.
    pub(crate) stale: bool,
    /// The working changes counted, which the rail's badge shows; AR-269 keeps it.
    pub(crate) working: usize,
    /// Why a version cannot be shown, by its hash; AR-266 fills it in.
    pub(crate) unavailable: BTreeMap<String, String>,
    /// The next page, on its way, and why the last one could not be read.
    more_reading: Option<Receiver<Result<VersionsPage, String>>>,
    more_failed: Option<String>,
    /// The version chosen, by its hash: a list that grows or a filter that hides it keeps it.
    pub(crate) selected: Option<String>,
    /// Only versions with a description are listed.
    described_only: bool,
    working_folded: bool,
    history_folded: bool,
    scroll: TreeScroll,
}

/// Keeps the panel to the project and session shown, takes the answer that has come, and asks for
/// the versions when they are due. A change of either lets go of all that belonged to the old one,
/// the read on its way among it, so an answer for it cannot land, whether or not a host is there.
fn refresh(shell: &mut Shell, ctx: &egui::Context) {
    let about = shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()));
    let v = &mut shell.versions;
    if about != v.about {
        (v.about, v.page, v.failed, v.reading, v.asked) = (about, None, None, None, false);
        v.scroll = TreeScroll::default();
        // Both were about what the panel has just left.
        (v.working, v.unavailable) = (0, BTreeMap::new());
        (v.more_reading, v.more_failed, v.selected, v.described_only) = (None, None, None, false);
    }
    let answer = match v.reading.as_ref().map(Receiver::try_recv) {
        Some(Ok(answer)) => Some(answer),
        Some(Err(TryRecvError::Disconnected)) => Some(Err("the read stopped without an answer".to_string())),
        _ => None,
    };
    match answer {
        Some(Ok(page)) => (v.reading, v.page) = (None, Some(page)),
        Some(Err(why)) => (v.reading, v.failed) = (None, Some(why)),
        None => {}
    }
    take_more(v);
    if (!v.asked || v.stale) && v.reading.is_none() {
        read(shell);
    }
    if shell.versions.reading.is_some() || shell.versions.more_reading.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
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
            page.versions.extend(next.versions);
            page.more = next.more;
        }
        (Ok(_), None) => {}
        (Err(why), _) => v.more_failed = Some(why),
    }
}

/// Asks for the page after the versions listed; its button is off while a read is on its way.
fn read_more(shell: &mut Shell) {
    let v = &shell.versions;
    let after = v.page.as_ref().and_then(|page| page.versions.last()).map(|last| last.hash.clone());
    let (Some(host), Some((path, _)), Some(after)) = (shell.services.projects.clone(), v.about.clone(), after) else { return };
    let v = &mut shell.versions;
    (v.more_failed, v.more_reading) = (None, Some(host.versions(&path, Some(after), PAGE)));
}

/// Starts a read of the newest versions when a host can answer it: only then is it asked for. The
/// page on its way belonged to the list this replaces, so it goes.
fn read(shell: &mut Shell) {
    let (Some(host), Some((path, _))) = (shell.services.projects.clone(), shell.versions.about.clone()) else { return };
    let v = &mut shell.versions;
    (v.failed, v.stale, v.asked, v.more_reading, v.more_failed) = (None, false, true, None, None);
    v.reading = Some(host.versions(&path, None, PAGE));
}

/// What Working changes says, when what is known allows it. "No changes" is never said beside a
/// count, and until AR-269 fills that in, only of a project with no unsaved work.
fn working_words(shell: &Shell) -> Option<String> {
    let page = shell.versions.page.as_ref()?;
    let untouched = shell.session.as_ref().is_some_and(|s| !s.is_dirty()) && !crate::panels::description_pending(&shell.panels);
    if page.versions.is_empty() {
        Some(tr("No versions yet. Save version records your first one."))
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
    ux::text(paint, pos2(r.left() + 38.0, row.center().y - 9.5), label, types::UI, p.text, Some((r.width() - 80.0).max(0.0)));
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
    refresh(shell, ui.ctx());
    ux::text(ui.painter(), r.min + vec2(20.0, 21.0), &tr("Versions").to_uppercase(), types::EYEBROW, p.text, Some((r.width() - 40.0).max(0.0)));
    let mut y = r.top() + 56.0;
    rule(ui, p, r, y);
    // A header only, while the project has only `main`; AR-292 adds its list and +.
    header(shell, ui, p, r, y, ("branches", &tr("Branches"), None, None));
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
    if header(shell, ui, p, r, y, ("working", &tr("Working changes"), Some(open), None)) {
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

/// The versions, newest first, scrolling under the headers: a long list has the filter over it
/// and Show more, or why the next page could not be read, under it.
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
    let tree = Rect::from_min_max(pos2(r.left(), top), pos2(r.right() - 1.0, r.bottom()));
    let offset = scroll_tree(ui, p, tree, top, shown.len(), PITCH, footer, true, None, &mut shell.versions.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    let clock = crate::chat_panel::clock(shell);
    for (i, v) in shown.iter().enumerate() {
        let y = top + i as f32 * PITCH - offset;
        if y + ROW >= tree.top() && y <= tree.bottom() {
            row(shell, &clip, p, (r, y), v, clock);
        }
    }
    if shown.is_empty() {
        let words = tr("None of these versions has a description.");
        ux::text(clip.painter(), pos2(r.left() + 20.0, top + 6.0), &words, types::SMALL, p.text_muted, Some((r.width() - 40.0).max(0.0)));
    }
    let y = top + shown.len().max(1) as f32 * PITCH - offset;
    footer_row(shell, &clip, p, r, y, more);
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
fn when(date: &str, clock: Clock) -> String {
    seconds_of(date).map_or_else(|| tr("Date not known"), |secs| date_and_time(secs, clock))
}

/// What a version was described as, when it was.
fn described(v: &VersionEntry) -> Option<&str> {
    v.description.as_deref().filter(|d| !d.trim().is_empty())
}

/// A version's two lines: its description and when and by whom, or without a description its date
/// and who asked when an agent did, else "No description".
fn lines(v: &VersionEntry, clock: Clock) -> (String, String) {
    let date = when(&v.date, clock);
    let who = if v.by_agent { Some(tr("Saved by an agent")) } else { v.author.clone().filter(|a| !a.is_empty()) };
    match described(v) {
        Some(d) => (d.to_string(), who.map_or(date.clone(), |w| fmt(&tr("{when} · {who}"), &[("when", &date), ("who", &w)]))),
        None if v.by_agent => (date, tr("Saved by an agent")),
        None => (date, tr("No description")),
    }
}

/// One version at `y`: its two lines, or for one that cannot be shown that it cannot, with its
/// reason behind "Why?", a warning at the right, and the reason in what a screen reader says.
fn row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, (r, y): (Rect, f32), v: &VersionEntry, clock: Clock) {
    let area = Rect::from_min_max(pos2(r.left() + 8.0, y), pos2(r.right() - 8.0, y + ROW));
    let why = shell.versions.unavailable.get(&v.hash).cloned();
    let (title, meta) = match &why {
        Some(_) => {
            (tr("This version can't be shown"), fmt(&tr("{when} · {who}"), &[("when", &when(&v.date, clock)), ("who", "")]).trim_end().to_string())
        }
        None => lines(v, clock),
    };
    let room = (area.width() - 20.0 - if why.is_some() { 26.0 } else { 0.0 }).max(0.0);
    let name = why.as_ref().map_or_else(|| format!("{title}, {meta}"), |why| format!("{title}, {meta} {why}"));
    let id = format!("workspace.versions.row.{}", v.hash);
    let selected = shell.versions.selected.as_deref() == Some(v.hash.as_str());
    let name = if selected { fmt(&tr("{name}, selected"), &[("name", &name)]) } else { name };
    let resp = control(shell, ui, &id, "row", area, true);
    let _ = resp.clone().accessible_name(name);
    if resp.clicked() {
        shell.versions.selected = Some(v.hash.clone());
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
