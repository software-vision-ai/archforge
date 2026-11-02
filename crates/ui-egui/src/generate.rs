//! Generate (AR-64): File → Generate… hands the selection to an agent connected to Archforge,
//! which writes SQL DDL or documentation from the model and its skills. Nothing is generated in
//! the window. The dialog shows what it is from (the selected schema, table, class or diagram,
//! else the open diagram), the output and its dialect, the export skills in effect there with
//! where each comes from, and the request to give the agent: what to write and the resources it
//! reads through the MCP server, at the model's revision. Its status line says whether agent
//! access is on, that the request was copied, and when the model changed after copying.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::ux::{self, icons, types};
use crate::{Shell, widgets};

/// What the agent is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Ddl,
    Documentation,
}

/// The dialog while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generate {
    pub output: Output,
    pub dialect: String,
    /// The model's revision when the request was last copied.
    pub copied: Option<u64>,
}

/// The longest dialect typed.
const MAX_DIALECT: usize = archforge_core::skills::MAX_DIALECT;
const WIDTH: f32 = 640.0;

/// What the request is about: the element, its kind's word and its name.
struct Scope {
    id: Uuid,
    obj: Obj,
    kind: String,
    name: String,
}

/// The kind word of an element the resources read, when it is one.
fn kind_of(store: &Store, o: Obj) -> Option<String> {
    let class = store.class_of(o).ok()?;
    Some(if class == archforge_er::SCHEMA {
        tr("Schema")
    } else if class == archforge_er::ENTITY {
        tr("Table")
    } else if class == archforge_uml::CLASS {
        tr("Class")
    } else if class == archforge_diagram::DIAGRAM {
        tr("Diagram")
    } else {
        return None;
    })
}

/// The selected element the resources read, else the open diagram.
fn scope(shell: &Shell, store: &Store) -> Option<Scope> {
    let selection = shell.session.as_ref().map(|s| s.view().selection.clone()).unwrap_or_default();
    let picked = match selection.as_slice() {
        [one] if kind_of(store, *one).is_some() => Some(*one),
        _ => None,
    };
    let obj = picked.or_else(|| store.by_uuid(shell.canvas.active?))?;
    let kind = kind_of(store, obj)?;
    Some(Scope { id: store.uuid(obj).ok()?, obj, kind, name: archforge_core::name_of(store, obj) })
}

/// The export skills in effect on `o`, each with where it comes from and whether a deeper one
/// overrides it.
fn export_skills(shell: &Shell, store: &Store, o: Obj) -> Vec<(String, String, bool, String)> {
    let Some(session) = shell.session.as_ref() else { return Vec::new() };
    archforge_core::skills::effective_skills(session.view(), store, o, Some(archforge_core::SkillTrigger::Export))
        .into_iter()
        .map(|e| {
            let origin = match e.origin {
                None => tr("from the project"),
                Some(at) if at == o => tr("on this element"),
                Some(at) => fmt(&tr("from {name}"), &[("name", &archforge_core::name_of(store, at))]),
            };
            let dialect = store.get(e.skill, archforge_core::skill::DIALECT).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
            (archforge_core::name_of(store, e.skill), origin, e.overridden_by.is_some(), dialect)
        })
        .collect()
}

/// Opens the dialog, its dialect the one the export skills in effect name, else PostgreSQL.
pub fn open(shell: &mut Shell) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let dialect = scope(shell, store)
        .map(|s| export_skills(shell, store, s.obj))
        .and_then(|skills| skills.into_iter().rev().find(|(_, _, overridden, d)| !overridden && !d.is_empty()).map(|(_, _, _, d)| d))
        .unwrap_or_else(|| "PostgreSQL".to_string());
    shell.generate = Some(Generate { output: Output::Ddl, dialect, copied: None });
}

/// The request for the agent: what to write, from what, at which revision, and where to read it,
/// a line at a time.
fn request(project: &str, scope: &Scope, g: &Generate, revision: u64) -> String {
    let what = fmt(&tr("{kind} \"{name}\""), &[("kind", &scope.kind), ("name", &scope.name)]);
    let revision = revision.to_string();
    let args = [("what", what.as_str()), ("project", project), ("revision", revision.as_str()), ("dialect", g.dialect.trim())];
    let (first, last) = match g.output {
        Output::Ddl => (
            tr("Generate SQL DDL ({dialect}) for the {what} of the Archforge project \"{project}\", model revision {revision}."),
            tr("Follow the skills in effect, the more specific over the outer. Give the result for review; do not run it on a database."),
        ),
        Output::Documentation => (
            tr("Write Markdown documentation for the {what} of the Archforge project \"{project}\", model revision {revision}."),
            tr("Follow the skills in effect, the more specific over the outer. Give the result for review."),
        ),
    };
    // The resources are addresses the server reads, the same in every language.
    [
        fmt(&first, &args),
        tr("Read it through Archforge's MCP server:"),
        format!("  archforge://element/{}", scope.id),
        format!("  archforge://skills/effective/{}/export", scope.id),
        last,
    ]
    .join("\n")
}

/// Draws the dialog, when it is open, over the window.
pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    let Some(mut g) = shell.generate.clone() else { return };
    let Some(session) = shell.session.as_ref() else {
        shell.generate = None;
        return;
    };
    let store = session.doc();
    let revision = session.revision();
    let project = shell.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let scope = scope(shell, store);
    let skills = scope.as_ref().map(|s| export_skills(shell, store, s.obj)).unwrap_or_default();
    let text = scope.as_ref().map(|s| request(&project, s, &g, revision));
    let p = *ux::palette(ui.visuals().dark_mode);
    let window = ui.ctx().content_rect();
    let mut close = false;
    let mut copy = false;
    egui::Area::new(widgets::egui_id("generate")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("generate.veil"), Sense::click_and_drag());
        let width = WIDTH.min(window.width() - 32.0).max(280.0);
        let inner = width - 64.0;
        let request = text.as_ref().map(|t| ux::galley_rows(&paint, t, types::SMALL, p.text_secondary, Some(inner - 24.0), 12));
        // As tall as what it holds: the header, From, Output, the skills, the request, the status
        // line (two lines at most) and the footer.
        let skills_h = if request.is_some() { 26.0 + 22.0 * skills.len().max(1) as f32 + 8.0 } else { 0.0 };
        let request_h = request.as_ref().map_or(0.0, |r| 26.0 + r.size().y + 24.0 + 16.0);
        let height = (108.0 + 64.0 + 82.0 + skills_h + request_h + 40.0 + 16.0 + 76.0).min(window.height() - 32.0);
        let dialog = Rect::from_center_size(window.center(), vec2(width, height));
        ux::shadow(&paint, dialog, 16.0, p.shadow);
        paint.rect_filled(dialog, 16.0, p.surface);
        paint.rect_stroke(dialog, 16.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        let left = dialog.left() + 32.0;
        let tile = Rect::from_min_size(pos2(left, dialog.top() + 32.0), vec2(40.0, 40.0));
        paint.rect_filled(tile, 10.0, p.accent_tint);
        ux::icon_in_tile(&paint, tile, &icons::EXPORT, p.accent);
        ux::text(&paint, pos2(left + 56.0, dialog.top() + 30.0), &tr("Generate"), types::SECTION, p.text, Some(inner - 96.0));
        ux::text(
            &paint,
            pos2(left + 56.0, dialog.top() + 61.0),
            &tr("An agent connected to Archforge writes it from the model and its skills."),
            types::UI,
            p.text_secondary,
            Some(inner - 56.0),
        );
        let x_button = Rect::from_min_size(pos2(dialog.right() - 60.0, dialog.top() + 28.0), vec2(32.0, 32.0));
        ux::icon_in(&paint, x_button.shrink(8.0), &icons::CLOSE, p.text_secondary);
        if control(shell, ui, "generate.close-x", "button", x_button, true).on_hover_text(tr("Close")).clicked() {
            close = true;
        }
        // From: what the request is about.
        let mut y = dialog.top() + 108.0;
        ux::text(&paint, pos2(left, y), &tr("From"), types::UI_STRONG, p.text, None);
        let from = scope.as_ref().map_or_else(|| tr("Nothing selected"), |s| fmt(&tr("{kind} · {name}"), &[("kind", &s.kind), ("name", &s.name)]));
        ux::text(&paint, pos2(left, y + 24.0), &from, types::UI, if scope.is_some() { p.text } else { p.text_muted }, Some(inner));
        // Output, and the dialect of SQL DDL.
        y += 64.0;
        ux::text(&paint, pos2(left, y), &tr("Output"), types::UI_STRONG, p.text, None);
        let mut bx = left;
        for (key, label, output) in [("ddl", tr("SQL DDL"), Output::Ddl), ("documentation", tr("Documentation"), Output::Documentation)] {
            let w = ux::galley(&paint, &label, types::SMALL, p.text, None).size().x + 32.0;
            let b = Rect::from_min_size(pos2(bx, y + 26.0), vec2(w, 36.0));
            let on = g.output == output;
            paint.rect_filled(b, 8.0, if on { p.accent_tint } else { p.surface });
            paint.rect_stroke(b, 8.0, Stroke::new(1.0, if on { p.accent } else { p.border }), StrokeKind::Inside);
            ux::text(&paint, b.min + vec2(16.0, 9.0), &label, types::SMALL, if on { p.accent } else { p.text }, None);
            if control(shell, ui, &format!("generate.output.{key}"), "toggle", b, true).clicked() {
                g.output = output;
            }
            bx = b.right() + 8.0;
        }
        if g.output == Output::Ddl {
            let dx = (left + inner / 2.0).max(bx + 16.0);
            ux::text(&paint, pos2(dx, y), &tr("Dialect"), types::UI_STRONG, p.text, None);
            let field = Rect::from_min_size(pos2(dx, y + 26.0), vec2((left + inner - dx).max(0.0), 36.0));
            paint.rect_filled(field, 6.0, p.field);
            widgets::record_visible(ui, &mut shell.registry, "generate.dialect", "field", field);
            let edit = egui::TextEdit::singleline(&mut g.dialect)
                .id(widgets::egui_id("generate.dialect"))
                .frame(egui::Frame::NONE)
                .font(types::SMALL.font())
                .text_color(p.text)
                .char_limit(MAX_DIALECT)
                .desired_width(field.width() - 24.0);
            let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(vec2(12.0, 9.0)))));
            let _ = (*out.response).clone().accessible_name(tr("Dialect"));
        }
        y += 82.0;
        // The export skills in effect, and the request.
        if let Some(request) = request {
            ux::text(&paint, pos2(left, y), &tr("Skills in effect for export"), types::UI_STRONG, p.text, None);
            y += 26.0;
            if skills.is_empty() {
                ux::text(&paint, pos2(left, y), &tr("No skill applies to it for export."), types::SMALL, p.text_muted, Some(inner));
                y += 22.0;
            }
            for (name, origin, overridden, dialect) in &skills {
                let mut line = format!("{name} · {origin}");
                if !dialect.is_empty() {
                    line.push_str(&format!(" · {dialect}"));
                }
                if *overridden {
                    line.push_str(&format!(" · {}", tr("overridden")));
                }
                ux::text(&paint, pos2(left, y), &line, types::SMALL, if *overridden { p.text_muted } else { p.text }, Some(inner));
                y += 22.0;
            }
            y += 8.0;
            ux::text(&paint, pos2(left, y), &tr("Request for your agent"), types::UI_STRONG, p.text, None);
            let boxed = Rect::from_min_size(pos2(left, y + 26.0), vec2(inner, request.size().y + 24.0));
            paint.rect_filled(boxed, 8.0, p.field);
            widgets::record_visible(ui, &mut shell.registry, "generate.request", "text", boxed);
            paint.galley(boxed.min + vec2(12.0, 12.0), request, p.text_secondary);
            y = boxed.bottom() + 16.0;
        }
        // The status line: what the request needs, or what became of it.
        let (status, ink) = if scope.is_none() {
            (tr("Select a schema, a table, a class or a diagram first."), p.text_secondary)
        } else if g.copied.is_some_and(|c| c != revision) {
            (tr("The model changed after the request was copied; copy it again so the agent reads this revision."), p.amber)
        } else if g.copied.is_some() {
            (tr("Copied. Paste the request to your agent; it reads the model through MCP."), p.text_secondary)
        } else if shell.control.is_some() {
            (tr("Agent access is on: a connected agent reads this context through MCP. Nothing is generated here."), p.text_secondary)
        } else {
            (tr("Agent access is off: start Archforge with --control <port> so an agent can read this context."), p.text_secondary)
        };
        let g_status = ux::galley_rows(&paint, &status, types::SMALL, ink, Some(inner), 3);
        paint.galley(pos2(left, y), g_status, ink);
        // Close, and Copy request while there is one.
        let foot = dialog.bottom() - 76.0;
        paint.rect_filled(Rect::from_min_size(pos2(dialog.left(), foot), vec2(width, 1.0)), 0.0, p.border);
        let copy_b = Rect::from_min_size(pos2(dialog.right() - 32.0 - 144.0, foot + 20.0), vec2(144.0, 40.0));
        let close_b = Rect::from_min_size(pos2(copy_b.left() - 12.0 - 104.0, foot + 20.0), vec2(104.0, 40.0));
        let live = text.is_some();
        paint.rect_filled(copy_b, 8.0, if live { p.accent_fill } else { p.field });
        ux::text(
            &paint,
            copy_b.min + vec2(16.0, 11.0),
            &tr("Copy request"),
            types::UI_STRONG,
            if live { p.on_accent } else { p.text_muted },
            Some(112.0),
        );
        if control(shell, ui, "generate.copy", "button", copy_b, live).clicked() && live {
            copy = true;
        }
        paint.rect_filled(close_b, 8.0, p.surface);
        paint.rect_stroke(close_b, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::text(&paint, close_b.min + vec2(24.0, 11.0), &tr("Close"), types::UI_STRONG, p.text, Some(72.0));
        if control(shell, ui, "generate.close", "button", close_b, true).clicked() {
            close = true;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            close = true;
        }
    });
    if copy && let Some(text) = text {
        ui.ctx().copy_text(text);
        g.copied = Some(revision);
    }
    shell.generate = if close { None } else { Some(g) };
}
