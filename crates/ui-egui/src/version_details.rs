//! A version's details in the centre (frame V2.2 of page 09 of the design and sections 4 and 10 of
//! `docs/design/versioning.md`): what it is, when and by whom, and the way to view it. Changed items
//! and the version before come with AR-354; Compare and Restore join the actions when they can be
//! carried out from here.

use std::path::PathBuf;

use egui::{Rect, pos2, vec2};

use crate::i18n::tr;
use crate::settings::{Look, button, control, focus_ring};
use crate::state::Explorer;
use crate::ux::{self, Palette, icons, types};
use crate::versions::lines;
use crate::{Shell, VersionEntry};

const MARGIN: f32 = 32.0;
const BAND: f32 = 72.0;

/// The details shown: of one version, for the project and session it was chosen in.
pub struct Details {
    about: (PathBuf, u64),
    entry: VersionEntry,
}

/// The project and session shown.
fn about(shell: &Shell) -> Option<(PathBuf, u64)> {
    shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()))
}

/// Selects `entry` and opens its details in place of any shown.
pub(crate) fn open(shell: &mut Shell, entry: VersionEntry) {
    shell.versions.selected = Some(entry.hash.clone());
    shell.versions.details = about(shell).map(|about| Details { about, entry });
}

/// Once a frame: details of another project or session close.
pub(crate) fn keep(shell: &mut Shell) {
    let about = about(shell);
    if shell.versions.details.as_ref().is_some_and(|d| Some(&d.about) != about.as_ref()) {
        shell.versions.details = None;
    }
}

/// Draws the details in the centre `r` while Versions is the explorer; false leaves `r` to the canvas.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let Some(entry) = shell.versions.details.as_ref().map(|d| d.entry.clone()) else { return false };
    if shell.state.explorer != Explorer::SourceControl {
        return false;
    }
    ui.painter().rect_filled(r, 0.0, p.page);
    breadcrumb(shell, ui, p, r);
    let clock = crate::chat_panel::clock(shell);
    let (title, meta) = lines(&entry, clock);
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    // Panels still report on the canvas, which this covers.
    if let Some(notice) = shell.canvas.notice.clone() {
        ux::text(ui.painter(), pos2(left, r.top() + BAND + 6.0), &notice, types::CAPTION, p.amber, Some(width));
    }
    let mut y = r.top() + BAND + 24.0;
    let g = ux::galley_rows(ui.painter(), &title, types::PAGE_TITLE, p.text, Some(width), 3);
    let height = g.size().y;
    ui.painter().galley(pos2(left, y), g, p.text);
    y += height + 10.0;
    ux::text(ui.painter(), pos2(left, y), &meta, types::SMALL, p.text_secondary, Some(width));
    y += 32.0;
    let off = if shell.session.as_ref().is_some_and(|s| s.interacting()) {
        Some(tr("Finish or cancel your edit first."))
    } else {
        (!crate::version_chip::available(shell)).then(|| tr("Wait for the change under way to finish."))
    };
    let id = "workspace.version-details.view";
    if button(shell, ui, p, id, pos2(left, y), &tr("View this version"), Look::Primary, off.is_none(), off.as_deref()).1.clicked() {
        // Off above whatever would refuse it, so no refusal is left to say on the canvas this hides.
        shell.versions.details = None;
        if shell.viewing() != Some(entry.hash.as_str()) {
            crate::version_chip::view(shell, entry.hash.clone(), crate::version_chip::when(&entry.date, clock));
        }
        return true;
    }
    y += 45.0;
    ui.painter().rect_filled(Rect::from_min_size(pos2(left, y), vec2(width, 1.0)), 0.0, p.border);
    if entry.previous.is_none() {
        let card = Rect::from_min_size(pos2(left, y + 22.0), vec2(width, 52.0));
        ui.painter().rect_filled(card, 8.0, p.surface);
        let words = tr("First version: there is nothing earlier to compare it with.");
        ux::text_mid(ui.painter(), left + 12.0, card.center().y, &words, types::UI, p.text, Some((width - 24.0).max(0.0)));
    }
    true
}

/// The way back at the top: the project's name, which closes the details, and where they are.
fn breadcrumb(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let name = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let (mid, paint) = (r.top() + 34.0, ui.painter().clone());
    let g = ux::galley(&paint, &name, types::UI, p.text_secondary, Some((r.width() / 2.0).max(0.0)));
    let at = Rect::from_min_size(pos2(r.left() + MARGIN, mid - g.size().y / 2.0), g.size());
    let resp = control(shell, ui, "workspace.version-details.close", "button", at, true).on_hover_text(tr("Close"));
    let _ = resp.clone().accessible_name(format!("{name}, {}", tr("Close")));
    focus_ring(&paint, p, at, 3.0, &resp);
    paint.galley(at.min, g, p.text_secondary);
    ux::icon_mid(&paint, pos2(at.right() + 18.0, mid), &icons::CHEVRON_RIGHT, p.text_secondary);
    ux::text_mid(&paint, at.right() + 37.0, mid, &tr("History"), types::UI, p.text, Some((r.width() / 3.0).max(0.0)));
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + BAND - 1.0), vec2(r.width(), 1.0)), 0.0, p.border);
    if resp.clicked() {
        shell.versions.details = None;
    }
}
