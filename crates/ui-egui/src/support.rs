//! About Archforge: what Archforge is, and how to support its development through
//! GitHub Sponsors or Ko-fi. It opens from the bar's Help button and, on macOS, from the
//! application menu.
//!
//! A destination opens its provider's own page in the browser, through the window's link
//! opening; payment happens there and Archforge never sees it. Only an approved HTTPS address
//! of the provider is opened: a destination the host has not set up, or one that is not such an
//! address, says Not set up yet and takes no click. A contribution unlocks nothing, and the
//! dialog never opens by itself. Done, the close button and Escape close it; the open project
//! stays as it was.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::ux::{self, icons, types};
use crate::{Shell, widgets};

const WIDTH: f32 = 560.0;
const HEIGHT: f32 = 460.0;
const PAD: f32 = 32.0;
/// The least distance from the window's edges when the window is smaller than the dialog.
const EDGE: f32 = 16.0;
const ROW: f32 = 64.0;
/// Where the body starts below the dialog's top, and how tall it is: Support Archforge to the end
/// of the payment note, with a little room under it.
const BODY_TOP: f32 = 108.0;
const BODY_HEIGHT: f32 = 274.0;

/// Where support goes, as the host sets it up: each an address on the provider's own site, or
/// none while its account is not ready.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Destinations {
    pub github_sponsors: Option<String>,
    pub ko_fi: Option<String>,
}

/// The dialog while it is open.
#[derive(Debug, Default)]
pub struct Dialog;

pub fn open(shell: &mut Shell) {
    if shell.about.is_none() {
        // The bar's search closes: its list would otherwise take Enter and the arrows behind the
        // dialog.
        if shell.home_search.open {
            crate::home_search::close(shell);
        }
        shell.about = Some(Dialog);
    }
}

/// Escape while the dialog is open closes it and goes no further: the shell asks before drawing
/// the window behind, whose editors would otherwise read the same key and drop their drafts.
pub(crate) fn dismiss_key(shell: &mut Shell, ctx: &egui::Context) {
    if shell.about.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        shell.about = None;
    }
}

/// `url` when it is an HTTPS address on `site` naming one account there (letters, digits, `-`
/// and `_`), else none: nothing else is ever opened from this dialog.
pub fn approved<'a>(url: Option<&'a str>, site: &str) -> Option<&'a str> {
    let url = url?;
    let handle = url.strip_prefix(site)?;
    let handle = handle.strip_suffix('/').unwrap_or(handle);
    let ok = !handle.is_empty() && handle.len() <= 64 && handle.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(url)
}

const GITHUB_SPONSORS: &str = "https://github.com/sponsors/";
const KO_FI: &str = "https://ko-fi.com/";

fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect, live: bool) -> egui::Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    let resp = ui.interact(rect, widgets::egui_id(id), if live { Sense::click() } else { Sense::hover() });
    if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    if shell.about.is_none() || !ux::ready(ui.ctx()) {
        return;
    }
    let Some(dialog) = shell.about.take() else { return };
    let p = *ux::palette(shell.state.theme.is_dark());
    let window = ui.max_rect();
    let mut close = false;
    let mut open_url: Option<String> = None;
    let destinations = shell.services.support.clone();
    egui::Area::new(egui::Id::unique("about")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_min_size(window.size());
        // A modal layer: Tab and clicks stay in the dialog, so nothing behind it moves.
        ui.ctx().memory_mut(|m| m.set_modal_layer(ui.layer_id()));
        // The veil takes every click behind the dialog; it and the frame take no keyboard focus,
        // so Tab goes from the close button to the destinations and Done.
        ui.painter().rect_filled(window, 0.0, p.scrim);
        let _ = ui.interact(window, ui.scope_id().with("about.veil"), Sense::CLICK | Sense::DRAG);
        let size = vec2(WIDTH.min(window.width() - 2.0 * EDGE).max(0.0), HEIGHT.min(window.height() - 2.0 * EDGE).max(0.0));
        let frame = Rect::from_center_size(window.center(), size);
        let paint = ui.painter().clone();
        paint.add(egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: p.shadow }.as_shape(frame, 16.0));
        paint.rect_filled(frame, 16.0, p.surface);
        paint.rect_stroke(frame, 16.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        let _ = ui.interact(frame, ui.scope_id().with("about.frame"), Sense::CLICK);
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let inner = (frame.width() - 2.0 * PAD).max(0.0);
        // The header: its icon, the title and its line, and the close button.
        let tile = Rect::from_min_size(at(PAD, PAD), Vec2::splat(40.0));
        paint.rect_filled(tile, 10.0, p.accent_tint);
        ux::icon_in_tile(&paint, tile, &icons::HELP, p.accent);
        let room = (frame.width() - 88.0 - 72.0).max(0.0);
        ux::text(&paint, at(88.0, 30.0), &tr("About Archforge"), types::SECTION, p.text, Some(room));
        // The line sits below the close button, so it may run to the inner edge.
        let line_room = (frame.width() - 88.0 - PAD).max(0.0);
        ux::text(&paint, at(88.0, 61.0), &tr("Free and open source modelling for ER and UML class diagrams."), types::BODY, p.text_secondary, Some(line_room));
        let x = Rect::from_min_size(at(frame.width() - 60.0, 28.0), Vec2::splat(32.0));
        let resp = control(shell, ui, "about.close", "button", x, true).on_hover_text(tr("Close"));
        if resp.hovered() {
            paint.rect_filled(x, 8.0, p.border);
        }
        ux::icon(&paint, x.min + vec2(9.0, 9.0), &icons::CLOSE, 14.0, p.text_secondary);
        close |= resp.clicked();
        // Between the header and the footer, the body: it scrolls when the window is too short to
        // show it all, so the destinations and the note never run under Done.
        let body = Rect::from_min_max(pos2(frame.left(), frame.top() + BODY_TOP), pos2(frame.right(), (frame.bottom() - 84.0).max(frame.top() + BODY_TOP)));
        let mut part = ui.new_child(egui::UiBuilder::new().max_rect(body));
        egui::ScrollArea::vertical().id_salt(widgets::egui_id("about.body")).auto_shrink([false, false]).max_height(body.height()).show(&mut part, |ui| {
            let (area, _) = ui.allocate_exact_size(vec2(body.width(), BODY_HEIGHT), Sense::hover());
            let paint = ui.painter().clone();
            // Offsets as the design gives them, from the dialog's top.
            let bt = |x: f32, y: f32| area.min + vec2(x, y - BODY_TOP);
            // Support: why, then the two destinations.
            ux::text(&paint, bt(PAD, 108.0), &tr("Support Archforge"), types::UI_STRONG, p.text, Some(inner));
            let why = ux::galley_rows(
                &paint,
                &tr("Archforge is free. If it helps your work, you can support its development. A contribution unlocks nothing: every feature stays free for everyone."),
                types::SMALL,
                p.text_secondary,
                Some(inner),
                2,
            );
            paint.galley(bt(PAD, 132.0), why, p.text_secondary);
            let rows = [
                ("github-sponsors", tr("GitHub Sponsors"), tr("One-time or monthly, on GitHub"), approved(destinations.github_sponsors.as_deref(), GITHUB_SPONSORS)),
                ("ko-fi", tr("Ko-fi"), tr("A one-time tip, on Ko-fi"), approved(destinations.ko_fi.as_deref(), KO_FI)),
            ];
            for (i, (key, name, how, url)) in rows.into_iter().enumerate() {
                let row = Rect::from_min_size(bt(PAD, 180.0 + 76.0 * i as f32), vec2(inner, ROW));
                let live = url.is_some();
                let resp = control(shell, ui, &format!("about.support.{key}"), "button", row, live);
                let resp = resp.on_hover_text(if live { tr("Opens in your browser") } else { tr("Not set up yet") });
                let _ = resp.clone().accessible_name(&name);
                paint.rect_filled(row, 10.0, if live && resp.hovered() { p.accent_tint } else { p.page });
                if resp.has_focus() {
                    paint.rect_stroke(row, 10.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
                }
                let state_w = 160.0_f32.min(inner / 2.0);
                let left_room = Some((inner - 32.0 - state_w).max(0.0));
                ux::text(&paint, row.min + vec2(16.0, 12.0), &name, types::UI_STRONG, p.text, left_room);
                ux::text(&paint, row.min + vec2(16.0, 34.0), &how, types::SMALL, p.text_secondary, left_room);
                let (state, ink) = if live { (tr("Opens in your browser"), p.accent) } else { (tr("Not set up yet"), p.text_muted) };
                let g = ux::galley(&paint, &state, types::SMALL, ink, Some(state_w));
                paint.galley(pos2(row.right() - 16.0 - g.size().x, row.center().y - g.size().y / 2.0), g, ink);
                if live && resp.clicked() {
                    open_url = url.map(str::to_string);
                }
            }
            let note = ux::galley_rows(
                &paint,
                &tr("Payment happens on the provider’s page in your browser; Archforge never sees it, and your project stays open here."),
                types::SMALL,
                p.text_muted,
                Some(inner),
                2,
            );
            paint.galley(bt(PAD, 332.0), note, p.text_muted);
        });
        // Done, at the footer's right.
        paint.rect_filled(Rect::from_min_size(pos2(frame.left(), frame.bottom() - 84.0), vec2(frame.width(), 1.0)), 0.0, p.border);
        let done = Rect::from_min_size(pos2(frame.right() - PAD - 104.0, frame.bottom() - 62.0), vec2(104.0, 40.0));
        let resp = control(shell, ui, "about.done", "button", done, true);
        paint.rect_filled(done, 8.0, if resp.hovered() { p.accent_fill.gamma_multiply(0.9) } else { p.accent_fill });
        centred(&paint, done, &tr("Done"), p.on_accent);
        close |= resp.clicked();
    });
    if let Some(url) = open_url {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
    if !close {
        shell.about = Some(dialog);
    }
}

fn centred(paint: &egui::Painter, r: Rect, label: &str, color: Color32) {
    let g = ux::galley(paint, label, types::UI_STRONG, color, Some(r.width() - 8.0));
    paint.galley(r.center() - g.size() / 2.0, g, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_https_address_naming_one_account_on_the_providers_site_is_approved() {
        assert_eq!(approved(Some("https://github.com/sponsors/DejanWho"), GITHUB_SPONSORS), Some("https://github.com/sponsors/DejanWho"));
        assert_eq!(approved(Some("https://ko-fi.com/archforge/"), KO_FI), Some("https://ko-fi.com/archforge/"));
        for refused in [
            "http://ko-fi.com/archforge",
            "https://ko-fi.com/",
            "https://ko-fi.com.evil.example/archforge",
            "https://ko-fi.com/a/b",
            "https://ko-fi.com/archforge?next=https://evil.example",
            "javascript:alert(1)",
        ] {
            assert_eq!(approved(Some(refused), KO_FI), None, "{refused}");
        }
        assert_eq!(approved(None, KO_FI), None);
    }
}
