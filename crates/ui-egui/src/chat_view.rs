//! The Chat panel as page 07 of the design draws it (frames B1 to B7): the empty state that sends
//! the person to Settings, the conversation with its question bubbles, tool rows, replies and
//! cards, the composer with its context chips, provider badge and Send or Stop, the read-only
//! switch, and the history of the project's conversations.
//!
//! Drawing changes nothing itself: it gathers what the person asked for ([`Act`]) and
//! [`crate::chat_panel`] does it once the panel is drawn.

use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::Shell;
use crate::ai_agents::Provider;
use crate::chat::FailureKind;
use crate::chat_panel::{Chat, Chip, Item, Laid, Panel, Turn, TurnState, View};
use crate::chat_store::{ChangeState, ToolStatus};
use crate::i18n::{fmt, tr, trn};
use crate::settings::{Look, button, control};
use crate::ux::{self, Palette, icons, types};

/// The panel's inner width's margins, the gap between a turn's entries, and between turns.
const PAD: f32 = 16.0;
const ENTRY_GAP: f32 = 6.0;
const TURN_GAP: f32 = 12.0;
/// The composer's lines: two at least, six at most.
const MIN_LINES: usize = 2;
const MAX_LINES: usize = 6;
/// Why a control waits while a reply runs.
const WAIT: &str = "Stop the reply first";

/// What the person asked for while the panel was drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Act {
    Send,
    Stop,
    Retry,
    Configure,
    Settings,
    Undo(archforge_mof::Uuid),
    AllowChanges(bool),
    Choose(Provider, String),
    RemoveChip(Chip),
    History,
    New,
    Open(archforge_mof::Uuid),
    /// The bin, ⌫ or Delete on a history row, or its menu's item: the confirmation opens.
    Delete(archforge_mof::Uuid, usize),
    /// A history row's menu opens at the place given.
    Menu(archforge_mof::Uuid, usize, egui::Pos2),
    Link(String),
    /// Enter while a reply runs: it waits.
    Waiting,
}

/// Every text this module passes to `tr` through a variable, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    [WAIT, "Conversations", "New conversation", "Send", "Stop", "Remove", "Agent and model", "Allow changes to the model"].into_iter()
}

/// The panel header's own buttons, left of its collapse button: the history and, once an agent is
/// set up, New conversation.
pub(crate) fn header(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let busy = shell.chat_panel.chat.as_ref().is_some_and(Chat::busy);
    let paint = ui.painter().clone();
    let history = Rect::from_min_size(pos2(r.right() - 94.0, r.top() + 8.0), Vec2::splat(28.0));
    let on = shell.chat_panel.view == View::History;
    let mut acts = Vec::new();
    if quiet_button(shell, ui, p, &paint, "workspace.chat.history", history, &icons::CONVERSATIONS, "Conversations", on, busy) {
        acts.push(Act::History);
    }
    let provider = shell.chat_panel.chat.as_ref().map_or(shell.state.agents.default_agent, |c| c.provider);
    let shown = crate::chat_panel::set_up(shell, provider) == Some(true) || shell.chat_panel.chat.as_ref().is_some_and(|c| !c.turns.is_empty());
    // The last row deleted: the keyboard goes to New conversation, or the history when it is hidden.
    // Asked until it holds: a dialog closing this frame still keeps the keyboard from what is
    // behind it.
    if shell.chat_panel.focus == Some(crate::chat_delete::FocusTo::New) {
        let to = crate::widgets::egui_id(if shown { "workspace.chat.new" } else { "workspace.chat.history" });
        if ui.memory(|m| m.has_focus(to)) || shell.chat_panel.focus_tries == 0 {
            shell.chat_panel.focus = None;
        } else {
            shell.chat_panel.focus_tries -= 1;
            ui.memory_mut(|m| m.request_focus(to));
        }
    }
    if shown {
        let new = Rect::from_min_size(pos2(r.right() - 64.0, r.top() + 8.0), Vec2::splat(28.0));
        if quiet_button(shell, ui, p, &paint, "workspace.chat.new", new, &icons::PLUS, "New conversation", false, busy) {
            acts.push(Act::New);
        }
    }
    run(shell, ui.ctx(), acts);
}

/// A quiet icon button of the header, filled while `on`; one that waits for a reply says so.
#[allow(clippy::too_many_arguments)]
fn quiet_button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    paint: &egui::Painter,
    id: &str,
    r: Rect,
    icon: &ux::Icon,
    words: &str,
    on: bool,
    busy: bool,
) -> bool {
    let resp = control(shell, ui, id, "button", r, !busy).on_hover_text(if busy { tr(WAIT) } else { tr(words) });
    let _ = resp.clone().accessible_name(tr(words));
    if on {
        paint.rect_filled(r, 6.0, p.accent_tint);
    } else if resp.hovered() && !busy {
        paint.rect_filled(r, 6.0, p.page);
    }
    let ink = if busy { p.text_muted } else { p.text_secondary };
    ux::icon_in(paint, r, icon, ink);
    crate::settings::focus_ring(paint, p, r, 6.0, &resp);
    !busy && resp.clicked()
}

/// Draws the panel's body under its header, from `y`.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) {
    crate::chat_panel::showing(shell);
    let body = Rect::from_min_max(pos2(r.left(), r.top() + y - 14.0), r.max);
    let mut panel = std::mem::take(&mut shell.chat_panel);
    let mut acts = Vec::new();
    match panel.view {
        View::History => history(shell, ui, p, body, &mut panel, &mut acts),
        View::Conversation => {
            let provider = panel.chat.as_ref().map_or(shell.state.agents.default_agent, |c| c.provider);
            let started = panel.chat.as_ref().is_some_and(|c| !c.turns.is_empty() || c.step.busy());
            match crate::chat_panel::set_up(shell, provider) {
                // The keys are being read: nothing to say yet.
                None if !started => {}
                Some(false) if !started => empty(shell, ui, p, body, &mut acts),
                _ => conversation(shell, ui, p, body, &mut panel, &mut acts),
            }
        }
    }
    if let Some(url) = panel.link.clone()
        && let Some(open) = crate::skill_tab::link_card(shell, ui, p, body, "workspace.chat.link.layer", "workspace.chat.link", &url)
    {
        if open {
            ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
        }
        panel.link = None;
    }
    shell.chat_panel = panel;
    if let Some((id, pick)) = crate::chat_delete::row_menu(shell, ui, p) {
        let row = shell.chat_panel.row_menu.as_ref().map_or(0, |m| m.row);
        let row = rows_index(shell, id).unwrap_or(row);
        acts.push(match pick {
            crate::chat_delete::MenuPick::Open => Act::Open(id),
            crate::chat_delete::MenuPick::Delete => Act::Delete(id, row),
        });
    }
    run(shell, ui.ctx(), acts);
}

/// The place of the conversation `id` in the history as last read.
fn rows_index(shell: &Shell, id: archforge_mof::Uuid) -> Option<usize> {
    match &shell.chat_panel.history {
        Some(Ok(rows)) => rows.iter().position(|l| l.id == id),
        _ => None,
    }
}

/// Does what was asked, the panel back in the shell.
fn run(shell: &mut Shell, ctx: &egui::Context, acts: Vec<Act>) {
    for act in acts {
        match act {
            Act::Send => crate::chat_panel::send(shell),
            Act::Stop => crate::chat_panel::stop(shell),
            Act::Retry => crate::chat_panel::retry(shell, None),
            Act::Configure | Act::Settings => crate::settings::open_at(shell, crate::settings::Section::Agents),
            Act::Undo(turn) => crate::chat_panel::undo_turn(shell, turn),
            Act::AllowChanges(on) => crate::chat_panel::set_allow_changes(shell, on),
            Act::Choose(provider, model) => {
                shell.chat_panel.menu = false;
                crate::chat_panel::choose(shell, provider, &model);
            }
            Act::RemoveChip(chip) => {
                if let Some(c) = shell.chat_panel.chat.as_mut() {
                    c.removed.insert(chip);
                }
            }
            Act::History => crate::chat_panel::toggle_history(shell),
            Act::New => crate::chat_panel::new_conversation(shell),
            Act::Open(id) => {
                shell.chat_panel.loading_stays = false;
                crate::chat_panel::open(shell, id);
            }
            Act::Delete(id, row) => crate::chat_delete::ask(shell, id, row),
            Act::Menu(id, row, at) => {
                // The menu takes the keys while it is open: the row gives the keyboard up, and has
                // it back when the menu closes with Escape.
                ctx.memory_mut(|m| {
                    if let Some(f) = m.focused() {
                        m.surrender_focus(f);
                    }
                });
                let keyboard = crate::settings::keyboard_last(ctx);
                shell.chat_panel.row_menu = Some(crate::chat_delete::RowMenu { chat: id, row, at, reached: keyboard.then_some(0) });
            }
            Act::Link(url) => shell.chat_panel.link = Some(url),
            Act::Waiting => shell.chat_panel.notice = Some(tr(WAIT)),
        }
        ctx.request_repaint();
    }
}

/// B1: no agent set up, Configure agent opens Settings on AI Agents.
fn empty(shell: &mut Shell, ui: &egui::Ui, p: &Palette, body: Rect, acts: &mut Vec<Act>) {
    let paint = ui.painter().clone();
    let width = (body.width() - 2.0 * PAD).max(0.0);
    let title = ux::galley(&paint, &tr("No agent set up yet"), types::UI_STRONG, p.text, Some(width));
    let text =
        tr("Chat answers with Claude or Codex, run by the program installed on this computer. Choose one in Settings and let Chat use the model.");
    let lines = centred(&paint, &text, types::SMALL, p.text_secondary, width);
    let height = 22.0 + 10.0 + title.size().y + 10.0 + lines.size().y + 10.0 + 28.0;
    let top = (body.top() + (body.height() * 0.3).min((body.height() - height).max(16.0))).round();
    ux::icon_in(&paint, Rect::from_min_size(pos2(body.center().x - 11.0, top), Vec2::splat(22.0)), &icons::CHAT, p.text_secondary);
    let mut y = top + 32.0;
    paint.galley(pos2(body.center().x - title.size().x / 2.0, y), title.clone(), p.text);
    y += title.size().y + 10.0;
    // Centred lines are placed by their middle.
    paint.galley(pos2(body.center().x, y), lines.clone(), p.text_secondary);
    y += lines.size().y + 10.0;
    let label = tr("Configure agent");
    let w = ux::galley(&paint, &label, types::UI, p.text, None).size().x + 20.0;
    let (_, resp) = button(shell, ui, p, "workspace.chat.configure", pos2((body.center().x - w / 2.0).round(), y), &label, Look::Primary, true, None);
    if resp.clicked() {
        acts.push(Act::Configure);
    }
}

/// `text` laid out centred in `width`.
fn centred(paint: &egui::Painter, text: &str, ty: ux::Type, color: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::text::TextFormat { font_id: ty.font(), color, line_height: Some(ty.line), ..Default::default() },
    );
    job.wrap.max_width = width;
    job.halign = egui::Align::Center;
    paint.layout_job(job)
}

/// The conversation and the composer under it (B2 to B6).
fn conversation(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, body: Rect, panel: &mut Panel, acts: &mut Vec<Act>) {
    let Some(chat) = panel.chat.as_mut() else { return };
    let busy = chat.busy();
    let left = body.left() + PAD;
    let width = (body.width() - 2.0 * PAD).max(0.0);
    let bottom = body.bottom() - 12.0;
    // From the foot up: the switch, the composer, the notice, the chips, a refusal.
    let switch = Rect::from_min_size(pos2(left, bottom - 28.0), vec2(width, 28.0));
    allow_switch(shell, ui, p, switch, chat, busy, acts);
    let lines = composer_lines(ui, &panel.draft, width - 20.0);
    let box_h = 10.0 + lines as f32 * types::UI.line + 6.0 + 28.0 + 8.0;
    let composer_rect = Rect::from_min_size(pos2(left, switch.top() - 8.0 - box_h), vec2(width, box_h));
    composer(shell, ui, p, composer_rect, panel, acts);
    let Some(chat) = panel.chat.as_mut() else { return };
    let paint = ui.painter().clone();
    let notice_g = ux::galley_rows(
        &paint,
        &tr("These and everything the agent reads are sent to the provider."),
        types::CAPTION,
        p.text_muted,
        Some(width - 20.0),
        2,
    );
    let notice_y = composer_rect.top() - 8.0 - notice_g.size().y.max(15.0);
    ux::icon_in(&paint, Rect::from_min_size(pos2(left, notice_y), Vec2::splat(14.0)), &icons::READ, p.text_secondary);
    paint.galley(pos2(left + 20.0, notice_y), notice_g, p.text_muted);
    let chips_bottom = notice_y - 8.0;
    let mut top_of_foot = chip_rows(shell, ui, p, left, width, chips_bottom, chat, acts);
    let refusal = panel.notice.clone().or_else(|| chat.save_error.clone());
    if let Some(why) = refusal {
        let g = ux::galley_rows(&paint, &why, types::SMALL, p.danger, Some(width), 4);
        top_of_foot -= g.size().y + 8.0;
        paint.galley(pos2(left, top_of_foot), g, p.danger);
    }
    // The timeline above, scrolled.
    let area = Rect::from_min_max(pos2(body.left(), body.top() + 16.0), pos2(body.right(), (top_of_foot - 16.0).max(body.top() + 16.0)));
    timeline(shell, ui, p, area, panel, acts);
}

/// How many lines the composer shows for `draft` wrapped at `width`: two to six.
fn composer_lines(ui: &egui::Ui, draft: &str, width: f32) -> usize {
    if draft.is_empty() {
        return MIN_LINES;
    }
    let mut job = egui::text::LayoutJob::single_section(
        draft.to_string(),
        egui::text::TextFormat { font_id: types::UI.font(), line_height: Some(types::UI.line), ..Default::default() },
    );
    job.wrap.max_width = width.max(1.0);
    let rows = ui.painter().layout_job(job).rows.len();
    rows.clamp(MIN_LINES, MAX_LINES)
}

/// The read-only switch, Allow changes to the model: a lock while off, a pencil in the accent
/// while on. It waits while a reply runs.
fn allow_switch(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chat: &Chat, busy: bool, acts: &mut Vec<Act>) {
    let paint = ui.painter().clone();
    let on = chat.allow_changes;
    let ink = if on { p.accent } else { p.text_secondary };
    ux::icon_in(
        &paint,
        Rect::from_min_size(pos2(r.left(), r.center().y - 7.0), Vec2::splat(14.0)),
        if on { &icons::EDIT } else { &icons::LOCK },
        ink,
    );
    let label = tr("Allow changes to the model");
    ux::text_mid(&paint, r.left() + 22.0, r.center().y, &label, types::SMALL, if on { p.accent } else { p.text }, Some((r.width() - 60.0).max(0.0)));
    let knob = Rect::from_min_size(pos2(r.right() - 28.0, r.center().y - 8.0), vec2(28.0, 16.0));
    if busy {
        let paint = ui.painter();
        paint.rect_filled(knob, 8.0, if on { p.accent_fill.gamma_multiply(0.45) } else { p.control_off });
        paint.circle_filled(pos2(if on { knob.right() - 8.0 } else { knob.left() + 8.0 }, knob.center().y), 6.0, p.on_accent);
        let _ = control(shell, ui, "workspace.chat.allow-changes", "toggle", knob, false).on_hover_text(tr(WAIT));
        return;
    }
    let resp = crate::settings_agents::toggle(shell, ui, p, "workspace.chat.allow-changes", knob, on, &label);
    if resp.clicked() {
        acts.push(Act::AllowChanges(!on));
    }
}

/// The context chips, flowing from the left, their foot at `bottom`; gives their top. A removed
/// chip stays removed for the conversation.
fn chip_rows(shell: &mut Shell, ui: &egui::Ui, p: &Palette, left: f32, width: f32, bottom: f32, chat: &Chat, acts: &mut Vec<Act>) -> f32 {
    let paint = ui.painter().clone();
    let chips: Vec<crate::chat_panel::ChipView> = crate::chat_panel::chips(shell).into_iter().filter(|c| !chat.removed.contains(&c.chip)).collect();
    if chips.is_empty() {
        return bottom + 8.0;
    }
    // Laid out first, then placed bottom-up.
    let mut rows: Vec<Vec<(f32, usize)>> = vec![Vec::new()];
    let mut x = 0.0;
    for (i, c) in chips.iter().enumerate() {
        let label_w = ux::galley(&paint, &c.label, types::SMALL, p.text, Some((width - 46.0).max(0.0))).size().x;
        let w = (6.0 + 15.0 + 4.0 + label_w + 4.0 + 14.0 + 4.0).min(width);
        if x > 0.0 && x + w > width {
            rows.push(Vec::new());
            x = 0.0;
        }
        if let Some(row) = rows.last_mut() {
            row.push((w, i));
        }
        x += w + 4.0;
    }
    let top = bottom - rows.len() as f32 * 28.0 + 4.0;
    for (n, row) in rows.iter().enumerate() {
        let mut x = left;
        let y = top + n as f32 * 28.0;
        for (w, i) in row {
            let Some(c) = chips.get(*i) else { continue };
            let r = Rect::from_min_size(pos2(x, y), vec2(*w, 24.0));
            paint.rect_filled(r, 6.0, p.field);
            ux::icon_in(&paint, Rect::from_min_size(pos2(r.left() + 6.0, r.center().y - 7.5), Vec2::splat(15.0)), c.icon, p.text_secondary);
            ux::text_mid(&paint, r.left() + 25.0, r.center().y, &c.label, types::SMALL, p.text, Some((w - 46.0).max(0.0)));
            let key = match c.chip {
                Chip::Project => "project",
                Chip::Diagram => "diagram",
                Chip::Selection => "selection",
            };
            let x_box = Rect::from_min_size(pos2(r.right() - 18.0, r.center().y - 7.0), Vec2::splat(14.0));
            let resp = control(shell, ui, &format!("workspace.chat.chip.{key}.remove"), "button", x_box, true).on_hover_text(tr("Remove"));
            let _ = resp.clone().accessible_name(format!("{}: {}", tr("Remove"), c.label));
            ux::icon_in(&paint, x_box, &icons::CLOSE, if resp.hovered() { p.text } else { p.text_secondary });
            if resp.clicked() {
                acts.push(Act::RemoveChip(c.chip));
            }
            x += w + 4.0;
        }
    }
    top
}

/// The composer: the text, the provider badge and Send, or Stop while a reply runs.
fn composer(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, panel: &mut Panel, acts: &mut Vec<Act>) {
    let Some(chat) = panel.chat.as_ref() else { return };
    let busy = chat.busy();
    let running = chat.turns.last().is_some_and(|t| t.state == TurnState::Running);
    let paint = ui.painter().clone();
    paint.rect_filled(r, 10.0, p.surface);
    let id = "workspace.chat.composer";
    let eid = crate::widgets::egui_id(id);
    let focused = ui.ctx().memory(|m| m.has_focus(eid));
    paint.rect_stroke(r, 10.0, Stroke::new(1.0, if focused { p.focus } else { p.border }), StrokeKind::Inside);
    // Enter sends, Shift+Enter starts a new line; while a reply runs Enter waits.
    // (A shortcut without Shift matches with it held too, so Shift is looked at first.)
    if focused && ui.input_mut(|i| !i.modifiers.shift && !i.modifiers.alt && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
        acts.push(if busy { Act::Waiting } else { Act::Send });
    }
    let text_rect = Rect::from_min_max(pos2(r.left() + 10.0, r.top() + 10.0), pos2(r.right() - 8.0, r.bottom() - 8.0 - 28.0 - 6.0));
    crate::widgets::record_visible(ui, &mut shell.registry, id, "field", text_rect);
    let agent = chat.provider.name();
    let hint =
        if running { fmt(&tr("{agent} is replying… you can type the next message"), &[("agent", agent)]) } else { tr("Ask about this model…") };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    child.set_clip_rect(text_rect.intersect(ui.clip_rect()));
    // The question's lines at the design's 19-point pitch.
    let ink = p.text;
    let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = egui::text::LayoutJob::single_section(
            buf.as_str().to_string(),
            egui::text::TextFormat { font_id: types::UI.font(), color: ink, line_height: Some(types::UI.line), ..Default::default() },
        );
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    egui::ScrollArea::vertical().id_salt(id).max_height(text_rect.height()).auto_shrink([false, true]).show(&mut child, |ui| {
        let edit = egui::TextEdit::multiline(&mut panel.draft)
            .layouter(&mut layouter)
            .id(eid)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(types::UI.font())
            .text_color(p.text)
            .hint_text(egui::RichText::new(hint).color(p.text_muted).font(types::UI.font()))
            .desired_rows(MIN_LINES)
            // Shift+Enter starts a new line; Enter alone sends, taken above.
            .return_key(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter))
            .desired_width(text_rect.width())
            .char_limit(crate::chat_panel::QUESTION_BYTES + 1);
        let out = edit.show(ui);
        let _ = (*out.response).clone().accessible_name(tr("Ask about this model…"));
        if out.response.changed() {
            panel.notice = None;
        }
    });
    let Some(chat) = panel.chat.as_ref() else { return };
    // The badge: the agent and its model; its menu picks another for the next question.
    let row = Rect::from_min_size(pos2(r.left() + 10.0, r.bottom() - 8.0 - 28.0), vec2(r.width() - 18.0, 28.0));
    let label = chat.model_label();
    let label_g = ux::galley(&paint, &label, types::SMALL, p.text_secondary, Some((row.width() - 80.0).max(0.0)));
    let badge = Rect::from_min_size(pos2(row.left(), row.center().y - 12.0), vec2(4.0 + 16.0 + 5.0 + label_g.size().x + 5.0 + 16.0 + 4.0, 24.0));
    let resp = control(shell, ui, "workspace.chat.badge", "select", badge, !busy).on_hover_text(if busy { tr(WAIT) } else { tr("Agent and model") });
    let _ = resp.clone().accessible_name(format!("{}: {label}", tr("Agent and model")));
    if resp.hovered() && !busy {
        paint.rect_filled(badge, 6.0, p.page);
    }
    ux::icon_in(&paint, Rect::from_min_size(pos2(badge.left() + 4.0, badge.center().y - 8.0), Vec2::splat(16.0)), &icons::AGENT, p.text);
    paint.galley(pos2(badge.left() + 25.0, badge.center().y - label_g.size().y / 2.0), label_g, p.text_secondary);
    ux::icon_mid(&paint, pos2(badge.right() - 12.0, badge.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    crate::settings::focus_ring(&paint, p, badge, 6.0, &resp);
    if resp.clicked() && !busy {
        panel.menu = !panel.menu;
    }
    if panel.menu && !busy {
        badge_menu(shell, ui, p, badge, panel, acts);
    }
    let Some(chat) = panel.chat.as_ref() else { return };
    // Send, or Stop while a reply runs.
    let b = Rect::from_min_size(pos2(row.right() - 28.0, row.top()), Vec2::splat(28.0));
    if running {
        let stopping = chat.turns.last().is_some_and(|t| t.stopping);
        let resp = control(shell, ui, "workspace.chat.stop", "button", b, !stopping).on_hover_text(tr("Stop"));
        let _ = resp.clone().accessible_name(tr("Stop"));
        paint.rect_filled(b, 6.0, if resp.hovered() { p.page } else { p.surface });
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_in(&paint, b, &icons::STOP, p.text_secondary);
        crate::settings::focus_ring(&paint, p, b, 6.0, &resp);
        if resp.clicked() && !stopping {
            acts.push(Act::Stop);
        }
    } else {
        // Drawn ready, as the design has it; with nothing typed it sends nothing.
        let ready = !busy && !panel.draft.trim().is_empty();
        let resp = control(shell, ui, "workspace.chat.send", "button", b, !busy).on_hover_text(if busy { tr(WAIT) } else { tr("Send") });
        let _ = resp.clone().accessible_name(tr("Send"));
        let fill = if busy { p.accent_fill.gamma_multiply(0.45) } else { p.accent_fill };
        paint.rect_filled(b, 6.0, if resp.hovered() && ready { fill.gamma_multiply(0.9) } else { fill });
        ux::icon_in(&paint, b, &icons::SEND, p.on_accent);
        crate::settings::focus_ring(&paint, p, b, 6.0, &resp);
        if resp.clicked() && ready {
            acts.push(Act::Send);
        }
    }
}

/// The badge's menu, above it: the conversation's agent's models, and the other agent's too
/// before the first question. The model in use is ticked.
fn badge_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, badge: Rect, panel: &mut Panel, acts: &mut Vec<Act>) {
    let Some(chat) = panel.chat.as_ref() else { return };
    let agents: Vec<Provider> = if chat.turns.is_empty() { Provider::ALL.to_vec() } else { vec![chat.provider] };
    let mut items: Vec<(Provider, String, String)> = Vec::new();
    for a in agents {
        for (id, _) in a.models() {
            let name = crate::ai_agents::model_name(id, a);
            items.push((a, id.to_string(), fmt(&tr("{agent} · {model}"), &[("agent", a.name()), ("model", &name)])));
        }
    }
    let height = 8.0 + items.len() as f32 * 30.0;
    let width = 240.0_f32.min(ui.ctx().content_rect().width());
    let r = Rect::from_min_size(pos2(badge.left(), badge.top() - 4.0 - height), vec2(width, height));
    let ctx = ui.ctx().clone();
    let current = (chat.provider, chat.model.clone());
    let picked = egui::Area::new(crate::widgets::egui_id("workspace.chat.badge.layer"))
        .order(egui::Order::Tooltip)
        .fixed_pos(r.min)
        .constrain(false)
        .fade_in(false)
        .show(&ctx, |ui| {
            ui.set_clip_rect(ctx.content_rect());
            ui.allocate_rect(r, egui::Sense::hover());
            let paint = ui.painter().clone();
            ux::shadow(&paint, r, 8.0, p.shadow);
            paint.rect_filled(r, 8.0, p.surface);
            paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            let mut picked = None;
            for (i, (a, id, words)) in items.iter().enumerate() {
                let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * 30.0), vec2((r.width() - 8.0).max(0.0), 30.0));
                let resp = control(shell, ui, &format!("workspace.chat.badge.{i}"), "menu-item", row, true);
                let _ = resp.clone().accessible_name(words);
                if resp.hovered() || resp.has_focus() {
                    paint.rect_filled(row, 5.0, p.page);
                }
                ux::text_mid(&paint, row.left() + 10.0, row.center().y, words, types::UI, p.text, Some((row.width() - 40.0).max(0.0)));
                if (*a, id.clone()) == current {
                    ux::icon_mid(&paint, pos2(row.right() - 14.0, row.center().y), &icons::TICK, p.accent);
                }
                if resp.clicked() {
                    picked = Some((*a, id.clone()));
                }
            }
            picked
        })
        .inner;
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || badge.contains(pos)));
    match picked {
        Some((a, m)) => acts.push(Act::Choose(a, m)),
        None if outside => panel.menu = false,
        None => {}
    }
}

/// The conversation's timeline in `area`, scrolled; it follows the end while the person has not
/// scrolled up.
fn timeline(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, area: Rect, panel: &mut Panel, acts: &mut Vec<Act>) {
    let dark = shell.state.theme.is_dark();
    let left = area.left() + PAD;
    let width = (area.width() - 2.0 * PAD).max(0.0);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(area));
    clip.set_clip_rect(area.intersect(ui.clip_rect()));
    let paint = clip.painter().clone();
    let origin = panel.chat.as_ref().and_then(|c| c.origin.clone());
    let project = shell.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let now = crate::chat_panel::clock(shell).now;
    // Laid out in content coordinates from the top, then moved by the scroll.
    let top = area.top() - panel.scroll;
    let mut y = top;
    let Some(chat) = panel.chat.as_mut() else { return };
    if chat.turns.is_empty() {
        let title = fmt(&tr("Ask about {project}"), &[("project", &project)]);
        y += ux::text(&paint, pos2(left, y), &title, types::UI_STRONG, p.text, Some(width)).height() + 12.0;
        let intro = tr("Chat reads the model to answer and shows each read below. It changes nothing until you allow changes for this conversation.");
        y += crate::settings::wrapped(&paint, pos2(left, y), width, &intro, types::SMALL, p.text_secondary, 8);
    }
    let speaker = chat.provider.name();
    let model_now = chat.named.clone().unwrap_or_else(|| chat.model.clone());
    let provider = chat.provider;
    let last = chat.turns.len().saturating_sub(1);
    let mut link_n = 0usize;
    for (n, turn) in chat.turns.iter_mut().enumerate() {
        if n > 0 {
            y += TURN_GAP;
        }
        // The question, unless it went back to the composer.
        if !turn.returned {
            let g = ux::galley_rows(&paint, &turn.question, types::UI, p.text, Some(width - 20.0), 400);
            let bubble = Rect::from_min_size(pos2(left, y), vec2(width, g.size().y + 16.0));
            paint.rect_filled(bubble, 10.0, p.accent_tint);
            paint.galley(pos2(left + 10.0, y + 8.0), g, p.text);
            y += bubble.height() + TURN_GAP;
        }
        y += ux::text(&paint, pos2(left, y), speaker, types::CAPTION_STRONG, p.text_muted, Some(width)).height();
        let streaming_at = turn.streaming.as_ref().map(|(at, _)| *at);
        let count = turn.items.len();
        // The turn's changes follow its last tool row, before the reply that sums them up.
        let card_at = turn.items.iter().rposition(|i| matches!(i, Item::Tool(_)));
        let refusal = crate::chat_panel::undo_refusal(shell, origin.as_deref(), turn);
        let mut card_drawn = false;
        for i in 0..=count {
            if !card_drawn
                && card_at.is_some_and(|c| c + 1 == i)
                && let Some(change) = turn.change.clone()
            {
                y += ENTRY_GAP;
                y += changes_card(shell, &clip, p, pos2(left, y), width, turn, &change, refusal.clone(), acts);
                card_drawn = true;
            }
            if streaming_at == Some(i) {
                y += ENTRY_GAP;
                let text = turn.streaming.as_ref().map(|(_, t)| format!("{t}▍")).unwrap_or_default();
                y += reply(shell, &clip, p, pos2(left, y), width, &text, &mut turn.laid, dark, &mut link_n, acts);
            }
            let Some(item) = turn.items.get_mut(i) else { continue };
            y += ENTRY_GAP;
            match item {
                Item::Tool(facts) => y += tool_row(&paint, p, pos2(left, y), width, facts),
                Item::Text { text, shortened, laid, .. } => {
                    y += reply(shell, &clip, p, pos2(left, y), width, text, laid, dark, &mut link_n, acts);
                    if *shortened {
                        y += 4.0
                            + ux::text(&paint, pos2(left, y + 4.0), &tr("The reply was shortened to fit."), types::SMALL, p.text_muted, Some(width))
                                .height();
                    }
                }
            }
        }
        if turn.rows_dropped > 0 {
            y += ENTRY_GAP;
            let more = trn("{n} more call is not shown", "{n} more calls are not shown", turn.rows_dropped);
            y += ux::text(&paint, pos2(left + 20.0, y), &more, types::SMALL, p.text_muted, Some(width - 20.0)).height();
        }
        // The turn's changes when no tool row came before them, then how it ended.
        if !card_drawn && let Some(change) = turn.change.clone() {
            y += ENTRY_GAP;
            y += changes_card(shell, &clip, p, pos2(left, y), width, turn, &change, refusal, acts);
        }
        match &turn.state {
            TurnState::Stopped => {
                y += ENTRY_GAP;
                let words = if turn.change.is_some() { tr("Stopped.") } else { tr("Stopped. Nothing was changed.") };
                y += ended_line(&paint, p, pos2(left, y), width, &words);
            }
            TurnState::Interrupted => {
                y += ENTRY_GAP;
                y += ended_line(&paint, p, pos2(left, y), width, &tr("Interrupted: Archforge closed while this turn ran."));
            }
            TurnState::Failed(f) => {
                y += ENTRY_GAP;
                let retry_now = n == last && crate::chat_panel::retryable(turn, now);
                y += failure_card(shell, &clip, p, pos2(left, y), width, turn, f, provider, &model_now, retry_now, n == last, now, acts);
            }
            // The agent's program retries the provider (AR-224): said, with Stop still there; once
            // Stop is asked, it no longer says it tries again.
            TurnState::Running if turn.retrying.is_some() && !turn.stopping => {
                y += ENTRY_GAP;
                y += retry_line(&paint, p, pos2(left, y), width, speaker, turn.retrying);
            }
            TurnState::Running | TurnState::Completed => {}
        }
    }
    let content = y - top + 8.0;
    let room = (content - area.height()).max(0.0);
    if panel.follow {
        panel.scroll = room;
        panel.follow = false;
    }
    if clip.rect_contains_pointer(area) {
        let dy = clip.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            panel.scroll -= dy;
        }
    }
    panel.scroll = panel.scroll.clamp(0.0, room);
    panel.at_end = panel.scroll >= room - 1.0;
    if room > 0.0 {
        let track = area.shrink2(vec2(0.0, 6.0));
        let h = (track.height() * area.height() / content).max(24.0).min(track.height());
        let at = panel.scroll / room.max(1.0);
        let thumb = Rect::from_min_size(pos2(area.right() - 6.0, track.top() + at * (track.height() - h)), vec2(4.0, h));
        paint.rect_filled(thumb, 2.0, p.scroll_thumb);
    }
}

/// A reply rendered at `at`, `width` wide, its layout kept in `laid` until the width or the
/// appearance changes; its links are controls that ask before they open. Gives its height.
#[allow(clippy::too_many_arguments)]
fn reply(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    text: &str,
    laid: &mut Option<Laid>,
    dark: bool,
    link_n: &mut usize,
    acts: &mut Vec<Act>,
) -> f32 {
    let fresh = laid.as_ref().is_none_or(|l| (l.width - width).abs() > 0.5 || l.dark != dark);
    // A streaming reply's layout is dropped with each piece of text that comes in.
    if fresh {
        let r = crate::chat_markdown::render(text, p, width);
        let job = std::sync::Arc::new(r.job);
        let height = ui.painter().layout_job((*job).clone()).size().y;
        *laid = Some(Laid { width, dark, job, height, links: r.links });
    }
    let Some(l) = laid.as_ref() else { return 0.0 };
    // Laid out and drawn only while it shows; its height is known either way.
    let shows = ui.clip_rect().intersects(Rect::from_min_size(at, vec2(width, l.height)));
    if !shows {
        return l.height;
    }
    let galley = ui.painter().layout_job((*l.job).clone());
    let paint = ui.painter();
    paint.galley(at, galley.clone(), p.text);
    for (range, url) in &l.links {
        let char_at = |byte: usize| galley.job.text.get(..byte).map_or(0, |s| s.chars().count());
        let a = galley.pos_from_cursor(egui::text::CCursor::new(char_at(range.start)));
        let b = galley.pos_from_cursor(egui::text::CCursor::new(char_at(range.end)));
        // The link's first row, as far as it goes on it.
        let right = if (a.min.y - b.min.y).abs() < 1.0 { b.min.x } else { galley.rect.right() };
        let hit = Rect::from_min_max(at + a.min.to_vec2(), at + vec2(right.max(a.min.x + 4.0), a.max.y));
        let id = format!("workspace.chat.link.{}", *link_n);
        *link_n += 1;
        let resp = control(shell, ui, &id, "link", hit, true).on_hover_text(url);
        let _ = resp.clone().accessible_name(url);
        if resp.clicked() {
            acts.push(Act::Link(url.clone()));
        }
    }
    l.height
}

/// A tool row: its icon, its words and its detail. Gives its height.
fn tool_row(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, facts: &crate::chat_store::ToolFacts) -> f32 {
    let (label, detail) = crate::chat_words::tool_words(facts);
    let v = crate::chat_words::verb(facts.command.as_deref(), facts.changed_model, facts.status);
    let (icon, ink) = match (facts.status, v) {
        (ToolStatus::Refused | ToolStatus::Failed, _) => (&icons::WARNING, p.amber),
        (_, crate::chat_words::Verb::Read) => (&icons::READ, p.text_secondary),
        _ => (&icons::EDIT, p.text_secondary),
    };
    let r = Rect::from_min_size(at, vec2(width, 22.0));
    ux::icon_in(paint, Rect::from_min_size(pos2(r.left(), r.center().y - 7.0), Vec2::splat(14.0)), icon, ink);
    let label_room = if detail.is_some() { width * 0.62 } else { width - 20.0 };
    let shown = ux::text_mid(paint, r.left() + 20.0, r.center().y, &label, types::SMALL, p.text_secondary, Some(label_room.max(0.0)));
    if let Some(d) = detail {
        ux::text_mid(paint, shown.right() + 6.0, r.center().y, &d, types::SMALL, p.text_muted, Some((r.right() - shown.right() - 6.0).max(0.0)));
    }
    22.0
}

/// How a stopped or interrupted turn ended: the stop mark and a line. Gives its height.
fn ended_line(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, words: &str) -> f32 {
    ux::icon_in(paint, Rect::from_min_size(pos2(at.x, at.y + 1.0), Vec2::splat(14.0)), &icons::STOP, p.text_muted);
    ux::text(paint, pos2(at.x + 20.0, at.y), words, types::SMALL, p.text_muted, Some((width - 20.0).max(0.0))).height().max(16.0)
}

/// The line of a running turn whose agent's program retries the provider: the attempt, and of how
/// many, as far as the program said them (an attempt of 0 is none said). Gives its height.
fn retry_line(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, agent: &str, retrying: Option<(u32, Option<u32>)>) -> f32 {
    let words = match retrying {
        Some((n, Some(max))) if n > 0 && max >= n => fmt(
            &tr("No answer from {agent}'s provider yet. Trying again, attempt {n} of {max}…"),
            &[("agent", agent), ("n", &n.to_string()), ("max", &max.to_string())],
        ),
        Some((n, _)) if n > 0 => {
            fmt(&tr("No answer from {agent}'s provider yet. Trying again, attempt {n}…"), &[("agent", agent), ("n", &n.to_string())])
        }
        _ => fmt(&tr("No answer from {agent}'s provider yet. Trying again…"), &[("agent", agent)]),
    };
    ux::icon_in(paint, Rect::from_min_size(pos2(at.x, at.y + 1.0), Vec2::splat(14.0)), &icons::LOOK_AGAIN, p.amber);
    let shown = ux::galley_rows(paint, &words, types::SMALL, p.text_secondary, Some((width - 20.0).max(0.0)), 3);
    let h = shown.size().y.max(16.0);
    paint.galley(pos2(at.x + 20.0, at.y), shown, p.text_secondary);
    h
}

/// The card of a turn's changes to the model (B4): how many, made as one step of the undo
/// history unless another change came during the turn, and Undo this turn's changes while a turn
/// of one step is on top; else why not. Undone, it says one step only for a turn of one.
#[allow(clippy::too_many_arguments)]
fn changes_card(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    turn: &Turn,
    change: &crate::chat_panel::Change,
    refusal: Option<String>,
    acts: &mut Vec<Act>,
) -> f32 {
    let paint = ui.painter().clone();
    let n = u64::from(change.changes);
    let (title, body) = match change.state {
        ChangeState::Applied => (
            trn("{n} change to the model", "{n} changes to the model", n),
            refusal.clone().filter(|r| !r.is_empty()).unwrap_or_else(|| tr("Made in this turn, as one step of the project's undo history.")),
        ),
        // One step only when the turn was one: a turn of several, taken back step by step, was not.
        ChangeState::Undone => (
            trn("{n} change undone", "{n} changes undone", n),
            if change.span.is_some_and(|s| s.steps == 1) {
                tr("Taken back in one step of the project's undo history.")
            } else {
                tr("Taken back in the project's undo history.")
            },
        ),
        ChangeState::Discarded => (trn("{n} change to the model", "{n} changes to the model", n), tr("Not kept: the changes were not saved.")),
    };
    let inner = (width - 20.0).max(0.0);
    let body_g = ux::galley_rows(&paint, &body, types::SMALL, p.text_secondary, Some(inner), 6);
    let undo = change.state == ChangeState::Applied && refusal.is_none();
    let h = 10.0 + 16.0 + 6.0 + body_g.size().y + if undo { 6.0 + 28.0 } else { 0.0 } + 10.0;
    let card = Rect::from_min_size(at, vec2(width, h));
    paint.rect_filled(card, 8.0, p.green_tint);
    ux::icon_in(&paint, Rect::from_min_size(card.min + vec2(10.0, 11.0), Vec2::splat(14.0)), &icons::APPLIED, p.green);
    ux::text(&paint, card.min + vec2(30.0, 10.0), &title, types::SMALL_STRONG, p.text, Some((inner - 20.0).max(0.0)));
    let body_h = body_g.size().y;
    paint.galley(card.min + vec2(10.0, 32.0), body_g, p.text_secondary);
    if undo {
        let (_, resp) = button(
            shell,
            ui,
            p,
            &format!("workspace.chat.turn.{}.undo", turn.number),
            card.min + vec2(10.0, 32.0 + body_h + 6.0),
            &tr("Undo this turn’s changes"),
            Look::Secondary,
            true,
            None,
        );
        if resp.clicked() {
            acts.push(Act::Undo(turn.id));
        }
    }
    h
}

/// A failed turn's card: rate limited and the time it can send again, the connection lost and the
/// question back in the composer, a question not sent as Let Chat use the model went off, no answer
/// (amber, Send again), or what Settings must fix (danger, Open Settings). Gives its height.
#[allow(clippy::too_many_arguments)]
fn failure_card(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    turn: &Turn,
    f: &crate::chat::ChatFailure,
    provider: Provider,
    model_now: &str,
    retry_now: bool,
    last: bool,
    now: u64,
    acts: &mut Vec<Act>,
) -> f32 {
    let paint = ui.painter().clone();
    let agent = provider.name();
    let offset = crate::chat_panel::clock(shell).offset;
    let nothing = if turn.change.is_none() { format!(" {}", tr("Nothing was changed.")) } else { String::new() };
    let mut settings = false;
    let mut retry_label = tr("Send again");
    let (danger, title, body) = match f.kind {
        FailureKind::RateLimited => {
            let body = match f.reset.filter(|at| *at > now) {
                Some(reset) => {
                    let time = crate::chat_words::clock_time(reset, offset);
                    let minutes = reset.saturating_sub(now).div_ceil(60).max(1);
                    retry_label = fmt(&tr("Send again at {time}"), &[("time", &time)]);
                    let wait = trn("in about {n} minute", "in about {n} minutes", minutes);
                    fmt(
                        &tr("{agent}'s provider asks Chat to wait. You can send again at {time}, {wait}."),
                        &[("agent", agent), ("time", &time), ("wait", &wait)],
                    )
                }
                None => fmt(&tr("{agent}'s provider asks Chat to wait."), &[("agent", agent)]),
            };
            (false, tr("Rate limited"), format!("{body}{nothing}"))
        }
        FailureKind::Network => {
            let body = if turn.returned {
                fmt(
                    &tr("The network went away before {agent} answered. Your message is back in the box below, ready to send again."),
                    &[("agent", agent)],
                )
            } else if turn.answered() {
                fmt(&tr("The network went away while {agent} was answering."), &[("agent", agent)])
            } else {
                fmt(&tr("The network went away before {agent} answered."), &[("agent", agent)])
            };
            (false, tr("Connection lost"), body)
        }
        FailureKind::ModelUnavailable => {
            settings = true;
            let model = turn.model.clone();
            let name = crate::chat_words::named_model(&model);
            let name = if name == model { crate::ai_agents::model_name(&model, provider) } else { name };
            (
                true,
                tr("Model not available"),
                fmt(
                    &tr("{model} isn't offered to this key. Choose another model in the badge below and send again, or check the key in Settings."),
                    &[("model", &name)],
                ),
            )
        }
        FailureKind::MissingRuntime => {
            settings = true;
            (
                true,
                tr("Program not found"),
                fmt(&tr("{agent} runs through a program that is not on this computer. Settings shows how to install it."), &[("agent", agent)]),
            )
        }
        FailureKind::InvalidCredential => {
            settings = true;
            (
                true,
                tr("Key refused"),
                fmt(&tr("{agent}'s provider refused the saved key. Replace it in Settings, then send again."), &[("agent", agent)]),
            )
        }
        FailureKind::NoCredential => {
            settings = true;
            (true, tr("No key saved"), fmt(&tr("{agent} needs an API key or a sign-in. Add one in Settings, then send again."), &[("agent", agent)]))
        }
        FailureKind::SignInExpired => {
            settings = true;
            (true, tr("Sign-in expired"), fmt(&tr("{agent}'s sign-in has expired. Sign in again in Settings, then send again."), &[("agent", agent)]))
        }
        // Let Chat use the model went off while its question was being kept (AR-221).
        FailureKind::Other if turn.unsent => {
            let body = if turn.returned {
                tr("Let Chat use the model was turned off, so this question was not sent. It is back in the box, ready once the switch is on.")
            } else {
                tr("Let Chat use the model was turned off, so this question was not sent. Send it again once the switch is on.")
            };
            (false, tr("Not sent"), body)
        }
        FailureKind::Other => (false, tr("No answer"), format!("{}{nothing}", fmt(&tr("{agent} stopped before answering."), &[("agent", agent)]))),
    };
    let body = if turn.change.is_some() && last {
        format!("{body} {}", tr("This turn changed the model, so it is not sent again; ask again to go on."))
    } else {
        body
    };
    // Send again: on the last turn when it can go; for a model not available, once another model
    // is chosen in the badge.
    let model_changed = turn.model != model_now;
    let offer_retry = last && turn.change.is_none() && (!settings || (f.kind == FailureKind::ModelUnavailable && model_changed));
    let waits = f.kind == FailureKind::RateLimited && !retry_now;
    let inner = (width - 20.0).max(0.0);
    let body_g = ux::galley_rows(&paint, &body, types::SMALL, p.text_secondary, Some(inner), 8);
    let actions = offer_retry || settings;
    let h = 10.0 + 16.0 + 6.0 + body_g.size().y + if actions { 6.0 + 28.0 } else { 0.0 } + 10.0;
    let card = Rect::from_min_size(at, vec2(width, h));
    paint.rect_filled(card, 8.0, if danger { p.danger_tint } else { p.amber_tint });
    ux::icon_in(
        &paint,
        Rect::from_min_size(card.min + vec2(10.0, 11.0), Vec2::splat(14.0)),
        &icons::WARNING,
        if danger { p.danger } else { p.amber },
    );
    ux::text(&paint, card.min + vec2(30.0, 10.0), &title, types::SMALL_STRONG, p.text, Some((inner - 20.0).max(0.0)));
    let body_h = body_g.size().y;
    paint.galley(card.min + vec2(10.0, 32.0), body_g, p.text_secondary);
    let mut x = card.left() + 10.0;
    let y = card.top() + 32.0 + body_h + 6.0;
    if settings {
        let (r, resp) = button(
            shell,
            ui,
            p,
            &format!("workspace.chat.turn.{}.settings", turn.number),
            pos2(x, y),
            &tr("Open Settings"),
            Look::Secondary,
            true,
            None,
        );
        if resp.clicked() {
            acts.push(Act::Settings);
        }
        x = r.right() + 6.0;
    }
    if offer_retry {
        let (_, resp) =
            button(shell, ui, p, &format!("workspace.chat.turn.{}.retry", turn.number), pos2(x, y), &retry_label, Look::Secondary, !waits, None);
        if resp.clicked() && !waits {
            acts.push(Act::Retry);
        }
    }
    h
}

/// B7: the project's conversations, newest first, the open one marked; a click opens one.
fn history(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, body: Rect, panel: &mut Panel, acts: &mut Vec<Act>) {
    let paint = ui.painter().clone();
    let left = body.left() + 8.0;
    let width = (body.width() - 16.0).max(0.0);
    let mut y = body.top() + 16.0;
    let project = shell.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let eyebrow = fmt(&tr("Saved with {project}"), &[("project", &project)]).to_uppercase();
    ux::text(&paint, pos2(left + 10.0, y), &eyebrow, types::EYEBROW, p.text_muted, Some(width - 20.0));
    y += 24.0;
    let clock = crate::chat_panel::clock(shell);
    let open = panel.chat.as_ref().map(|c| c.id);
    let (mut scroll_by, mut scroll_room, mut return_early) = (0.0_f32, None, false);
    match &panel.history {
        None => {}
        Some(Err(why)) => {
            let words = fmt(&tr("The conversations in this project cannot be read: {why}"), &[("why", why)]);
            y += crate::settings::wrapped(&paint, pos2(left + 10.0, y), width - 20.0, &words, types::SMALL, p.danger, 6) + 8.0;
        }
        Some(Ok(rows)) if rows.is_empty() => {
            y += crate::settings::wrapped(
                &paint,
                pos2(left + 10.0, y),
                width - 20.0,
                &tr("No conversations in this project yet."),
                types::SMALL,
                p.text_secondary,
                3,
            ) + 8.0;
        }
        Some(Ok(rows)) => {
            // The rows scroll under the eyebrow; the footnote follows the last.
            let area = Rect::from_min_max(pos2(body.left(), y), body.max);
            let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(area));
            clip.set_clip_rect(area.intersect(ui.clip_rect()));
            let paint = clip.painter().clone();
            let top = y;
            y -= panel.history_scroll;
            let mut focus_found = false;
            for (i, listed) in rows.iter().enumerate() {
                let r = Rect::from_min_size(pos2(left, y), vec2(width, 48.0));
                y += 50.0;
                let wanted = match panel.focus {
                    Some(crate::chat_delete::FocusTo::Row(at)) => at == i,
                    Some(crate::chat_delete::FocusTo::Chat(id)) => id == listed.id,
                    _ => false,
                };
                if !clip.clip_rect().intersects(r) && !wanted {
                    continue;
                }
                let row_id = format!("workspace.chat.history.{i}");
                let bin_id = format!("{row_id}.delete");
                let bin = Rect::from_min_size(pos2(r.right() - 34.0, r.center().y - 14.0), Vec2::splat(28.0));
                let refused = crate::chat_delete::refusal(panel, &listed.entry);
                let pointed = clip.rect_contains_pointer(r);
                match &listed.entry {
                    Ok(row) => {
                        let resp = control(shell, &clip, &row_id, "row", r, true);
                        let _ = resp.clone().accessible_name(&row.title);
                        // Asked until it holds: a dialog closing this frame still keeps the
                        // keyboard from the rows.
                        if wanted {
                            if resp.has_focus() || panel.focus_tries == 0 {
                                focus_found = true;
                            } else {
                                panel.focus_tries -= 1;
                                resp.request_focus();
                            }
                        }
                        // The keyboard is on the row or its bin while its marks show: the pointer last
                        // used leaves a focus moved for it unmarked, as the row then looks.
                        let keyboard = crate::settings::keyboard_last(clip.ctx());
                        let bin_focused = clip.memory(|m| m.has_focus(crate::widgets::egui_id(&bin_id)));
                        let with_bin = pointed || (keyboard && (resp.has_focus() || bin_focused));
                        let selected = open == Some(row.id);
                        if selected {
                            paint.rect_filled(r, 6.0, p.accent_tint);
                        } else if pointed {
                            paint.rect_filled(r, 6.0, p.page);
                        }
                        // The title shortens to make room for the bin.
                        let text_w = if with_bin { width - 20.0 - 34.0 } else { width - 20.0 };
                        ux::text(&paint, r.min + vec2(10.0, 8.0), &row.title, types::UI, if selected { p.accent } else { p.text }, Some(text_w));
                        let meta = fmt(
                            &tr("{when} · {agent} · {messages}"),
                            &[
                                ("when", &crate::chat_words::when(row.updated_at, clock)),
                                ("agent", row.provider.name()),
                                ("messages", &trn("{n} message", "{n} messages", u64::from(row.messages))),
                            ],
                        );
                        ux::text(&paint, r.min + vec2(10.0, 25.0), &meta, types::CAPTION, p.text_muted, Some(text_w));
                        crate::settings::focus_ring(&paint, p, r, 6.0, &resp);
                        if with_bin && crate::chat_delete::bin(shell, &clip, p, &paint, &bin_id, bin, refused.as_deref()) {
                            acts.push(Act::Delete(row.id, i));
                        }
                        // On the focused row: ⌫ or Delete asks, the menu key or ⇧F10 opens its menu.
                        if resp.has_focus() && panel.ask.is_none() && panel.row_menu.is_none() {
                            let (delete, menu) = clip.input_mut(|k| {
                                let delete = k.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
                                    || k.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
                                let menu =
                                    k.consume_key(egui::Modifiers::NONE, egui::Key::Menu) || k.consume_key(egui::Modifiers::SHIFT, egui::Key::F10);
                                (delete, menu)
                            });
                            if delete && refused.is_none() {
                                acts.push(Act::Delete(row.id, i));
                            }
                            if menu {
                                acts.push(Act::Menu(row.id, i, pos2(r.right() - 8.0, r.bottom() - 6.0)));
                            }
                        }
                        if resp.secondary_clicked() {
                            let at = clip.input(|k| k.pointer.interact_pos()).unwrap_or(r.center());
                            acts.push(Act::Menu(row.id, i, at));
                        }
                        // A click on a disabled bin opens nothing.
                        let on_bin = with_bin && clip.input(|k| k.pointer.interact_pos()).is_some_and(|at| bin.contains(at));
                        if resp.clicked() && !on_bin {
                            acts.push(Act::Open(row.id));
                        }
                    }
                    Err(why) => {
                        // A row that cannot be read takes no keyboard: the request is given up.
                        focus_found |= wanted;
                        let words = fmt(&tr("This conversation cannot be read: {why}"), &[("why", why)]);
                        let text_w = if pointed { width - 20.0 - 34.0 } else { width - 20.0 };
                        crate::settings::wrapped(&paint, r.min + vec2(10.0, 8.0), text_w, &words, types::CAPTION, p.text_muted, 2);
                        if pointed {
                            crate::chat_delete::bin(shell, &clip, p, &paint, &bin_id, bin, refused.as_deref());
                        }
                    }
                }
            }
            // A row the keyboard was to go to that the list no longer holds: it goes nowhere.
            let target = panel.focus.is_some_and(|f| match f {
                crate::chat_delete::FocusTo::Row(at) => at < rows.len(),
                crate::chat_delete::FocusTo::Chat(id) => rows.iter().any(|l| l.id == id),
                crate::chat_delete::FocusTo::New => true,
            });
            if panel.focus.is_some_and(|f| f != crate::chat_delete::FocusTo::New) && (focus_found || (!target && panel.listing.is_none())) {
                panel.focus = None;
            }
            let foot = tr("Conversations are kept in the project, so they travel with it and never show in another project.");
            y += crate::settings::wrapped(&paint, pos2(left + 10.0, y + 10.0), width - 20.0, &foot, types::CAPTION, p.text_muted, 4) + 20.0;
            let content = y + panel.history_scroll - top;
            let room = (content - area.height()).max(0.0);
            if clip.rect_contains_pointer(area) {
                scroll_by = clip.input(|i| i.smooth_scroll_delta.y);
            }
            scroll_room = Some(room);
            return_early = true;
        }
    }
    if let Some(room) = scroll_room {
        panel.history_scroll = (panel.history_scroll - scroll_by).clamp(0.0, room);
    }
    if let Some(why) = panel.list_notice.as_ref().or(panel.load_error.as_ref()) {
        let at = if return_early { body.bottom() - 60.0 } else { y };
        crate::settings::wrapped(&paint, pos2(left + 10.0, at), width - 20.0, why, types::SMALL, p.danger, 4);
    }
    if return_early {
        return;
    }
    let foot = tr("Conversations are kept in the project, so they travel with it and never show in another project.");
    crate::settings::wrapped(&paint, pos2(left + 10.0, y + 10.0), width - 20.0, &foot, types::CAPTION, p.text_muted, 4);
}
