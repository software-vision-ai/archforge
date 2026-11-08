//! Settings → AI Agents: the default agent, a card for each agent program (Claude with an API
//! key; Codex signed in with ChatGPT or with an API key), and Check connection and Send test
//! message with the status badge under them.
//!
//! The page shows what this run already knows: the programs were looked for when Archforge
//! started, through the host; the keys are read from the secure store the first time the page is
//! drawn (a local read, sending nothing), and kept. Otherwise drawing it runs nothing; only Look
//! again, Check connection (local), a key's Save or Remove, Send test message and the sign-in
//! buttons ask the host for anything. A key is typed into a hidden field that keeps no undo
//! history, handed to the host once and its draft overwritten; afterwards only its last four
//! characters are shown. The toolkit's own short-lived copies of the field's text, made while it
//! is drawn, are freed but not overwritten.

use egui::{Rect, Response, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use zeroize::Zeroize;

use crate::ai_agents::{
    self as agents, AccountError, CodexAccount, CodexSignIn, Key, KeyRefusal, KeyText, LoginWay, Outcome, Program, Provider, Readiness, Store, System,
};
use crate::i18n::{fmt, tr, trn};
use crate::settings::{Look, Tone, badge, button, control, focus_ring, icon_button, wrapped};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The page's longest sentences, looked up through `tr` by name; the catalogue test reads them
/// through [`texts`].
const INTRO: &str = "Chat talks to an agent program installed on this computer. Archforge finds the program and keeps your keys in the system's secure store. Opening this page sends nothing.";
const NOT_ON_WINDOWS: &str =
    "AI Agents work on macOS and Linux for now. On Windows, Chat and these settings stay off; everything else in Archforge works as usual.";
const NO_STORE: &str = "Archforge keeps API keys only in the system's secure store, and this session has none running (a Secret Service such as GNOME Keyring or KWallet). Start one and restart Archforge to save keys; Archforge never keeps them anywhere else.";
const MISSING: &str =
    "Archforge looked for the {program} program on this computer and didn't find it. Chat runs {agent} through that program, so install it first.";
const OWN_FILES: &str =
    "The codex program keeps its ChatGPT sign-in in its own files, so this works without the secure store. An API key for Codex would need it.";

/// The sentences the page looks up by name, for the catalogue test.
#[cfg(test)]
pub(crate) fn texts() -> [&'static str; 5] {
    [INTRO, NOT_ON_WINDOWS, NO_STORE, MISSING, OWN_FILES]
}

/// A select's list open on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    DefaultAgent,
    Model(Provider),
}

/// The page's own state while Settings is open: the list open, and each agent's key as it is being
/// typed (overwritten when it is handed over, cancelled or dropped).
pub struct Page {
    pub menu: Option<Menu>,
    /// Reserved at the most a key may take, so typing never moves the text and leaves no copy
    /// behind; overwritten whole, spare room included.
    drafts: [String; 2],
    replacing: [bool; 2],
    refusal: [Option<KeyRefusal>; 2],
    focus_key: Option<Provider>,
    /// Codex's account was asked to be read again for this opening of the page (AR-222).
    account_asked: bool,
}

impl Default for Page {
    /// The page as it opens: no list open, the key fields empty with their room reserved, and
    /// Codex's account not yet asked for.
    fn default() -> Self {
        Page { menu: None, drafts: [draft_room(), draft_room()], replacing: [false; 2], refusal: [None; 2], focus_key: None, account_asked: false }
    }
}

/// The page shown again from another section of the dialog: Codex's account is read again, as
/// when Settings opens (AR-222).
pub(crate) fn shown_again(page: &mut Page) {
    page.account_asked = false;
}

/// An empty draft with room for the longest key the field takes, in any characters.
fn draft_room() -> String {
    String::with_capacity(agents::MAX_KEY_BYTES.saturating_mul(4))
}

/// Whether anything is typed in either key field (tests).
#[cfg(test)]
pub(crate) fn drafted(page: &Page) -> bool {
    page.drafts.iter().any(|d| !d.is_empty())
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page").field("menu", &self.menu).field("replacing", &self.replacing).finish_non_exhaustive()
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        for d in &mut self.drafts {
            d.zeroize();
        }
    }
}

fn slot(p: Provider) -> usize {
    match p {
        Provider::Claude => 0,
        Provider::Codex => 1,
    }
}

fn key_field_id(p: Provider) -> String {
    format!("settings.agents.{}.key", p.key())
}

/// Drops what was typed into the key fields: the drafts overwritten, and the fields' own memory of
/// the text (their undo history) cleared.
pub(crate) fn forget_drafts(ctx: &egui::Context, page: &mut Page) {
    for p in Provider::ALL {
        forget_draft(ctx, page, p);
    }
}

fn forget_draft(ctx: &egui::Context, page: &mut Page, p: Provider) {
    page.drafts[slot(p)].zeroize();
    page.drafts[slot(p)] = draft_room();
    egui::text_edit::TextEditState::default().store(ctx, widgets::egui_id(&key_field_id(p)));
}

/// Where keys are kept, as the sentences name it.
fn store_name(store: &Store) -> String {
    match store {
        Store::MacKeychain => tr("the macOS Keychain"),
        Store::SecretService => tr("the Secret Service"),
        _ => tr("the system's secure store"),
    }
}

/// How long ago `at` was, for the status badge.
fn ago(at: u64) -> String {
    ago_secs(crate::home::now().saturating_sub(at))
}

/// `secs` seconds ago in words: just now, then whole minutes, then whole hours.
fn ago_secs(secs: u64) -> String {
    if secs < 60 {
        tr("just now")
    } else if secs < 3600 {
        trn("{n} minute ago", "{n} minutes ago", secs / 60)
    } else {
        trn("{n} hour ago", "{n} hours ago", secs / 3600)
    }
}

/// What the device-code box says: where to enter the code, and in how many minutes it runs out.
fn device_code_hint(minutes: u32) -> String {
    let expiry = trn("It runs out in {n} minute.", "It runs out in {n} minutes.", u64::from(minutes));
    format!("{} {expiry}", tr("On any device with a browser, open the sign-in page and enter this code."))
}

/// An account error in a sentence's words.
fn account_error(e: &AccountError) -> String {
    match e {
        AccountError::NotAvailableYet => tr("Not available yet"),
        AccountError::KeyRefused { detail } => fmt(&tr("The provider refused the key ({detail})."), &[("detail", detail)]),
        AccountError::NoCredential => tr("There is no key or sign-in to use."),
        AccountError::SignInTimedOut => tr("The sign-in was not finished in time. Sign in again."),
        AccountError::SignInCancelled => tr("The codex program cancelled the sign-in. Sign in again."),
        AccountError::Failed { detail } => detail.clone(),
    }
}

/// One agent's readiness in the badge's words.
fn readiness_text(p: Provider, r: Readiness) -> String {
    let agent = [("agent", p.name())];
    fmt(
        &match r {
            Readiness::Ready => tr("{agent} ready"),
            Readiness::NotFound => tr("{agent} not found"),
            Readiness::TooOld => tr("{agent} too old"),
            Readiness::Unusable => tr("{agent} can't be used"),
            Readiness::SignInUnreadable => tr("{agent} sign-in could not be read"),
            Readiness::NoKey => tr("{agent} has no key"),
            Readiness::KeyRefused => tr("{agent} key refused"),
            Readiness::NotSignedIn => tr("{agent} not signed in"),
            Readiness::SignInExpired => tr("{agent} sign-in expired"),
            Readiness::SignInNotAvailable => tr("{agent} sign-in not available yet"),
            Readiness::NotLooked => tr("{agent} not looked for yet"),
        },
        &agent,
    )
}

/// The width a compact button takes for `label`.
fn button_width(paint: &egui::Painter, p: &Palette, label: &str) -> f32 {
    (ux::galley(paint, label, types::UI, p.text, None).size().x + 20.0).ceil()
}

/// Draws the page from `top`, `width` wide at `left`; gives how far it reaches below `top`. The
/// first frame of each opening reads Codex's account again.
pub(crate) fn page(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, page: &mut Page, left: f32, top: f32, width: f32) -> f32 {
    let paint = ui.painter().clone();
    ux::text(&paint, pos2(left, top + 26.0), &tr("AI Agents"), types::LEAD, p.text, Some(width));
    let system = shell.services.agents.as_ref().map(|h| h.system());
    if system != Some(System::Supported) {
        return unavailable(ui, p, system, left, top, width);
    }
    // The keys are read from the secure store the first time the page is drawn, and kept. Codex's
    // account is read each time the page opens (AR-222), so a sign-in that expired since shows so;
    // reading it starts the codex program, so never before the page shows.
    agents::need_keys(shell);
    if !page.account_asked {
        page.account_asked = true;
        agents::reread_account(shell);
    }
    let mut y = top + 52.0;
    y += wrapped(&paint, pos2(left, y), width, &tr(INTRO), types::UI, p.text_secondary, 3) + 20.0;
    if let Store::Missing { why } = &shell.agents.look.store {
        y = no_store(ui, p, &why.clone(), left, y, width) + 20.0;
    }
    // The default agent.
    ux::text_mid(&paint, left, y + 14.0, &tr("Default agent"), types::UI, p.text, Some(140.0));
    let default_select = Rect::from_min_size(pos2(left + 152.0, y), vec2(220.0_f32.min(width - 152.0).max(0.0), 28.0));
    let current = shell.state.agents.default_agent;
    select(shell, ui, p, page, "settings.agents.default", default_select, current.name(), true, None, Menu::DefaultAgent, &tr("Default agent"));
    y += 48.0;
    let mut models = [Rect::NOTHING; 2];
    for agent in Provider::ALL {
        let (bottom, model) = card(shell, ui, p, page, agent, left, y, width);
        models[slot(agent)] = model;
        y = bottom + 20.0;
    }
    paint.hline(left..=left + width, y, Stroke::new(1.0, p.border));
    y += 24.0;
    // Check connection and Send test message, their notes beside them, the badge under them.
    let check_label = if shell.agents.checking() { tr("Checking…") } else { tr("Check connection") };
    let test_label = if shell.agents.testing() { tr("Sending…") } else { tr("Send test message") };
    let note_x = left + button_width(&paint, p, &check_label).max(button_width(&paint, p, &test_label)) + 12.0;
    let note_w = (left + width - note_x).max(0.0);
    let (_, resp) = button(shell, ui, p, "settings.agents.check", pos2(left, y), &check_label, Look::Secondary, !shell.agents.checking(), None);
    if resp.clicked() {
        agents::check_connection(shell);
    }
    let note = wrapped(
        &paint,
        pos2(note_x, y),
        note_w,
        &tr("Free, runs on this computer: finds the program, its version and your key or sign-in. Sends nothing."),
        types::SMALL,
        p.text_secondary,
        3,
    );
    y += note.max(28.0) + 12.0;
    let default = shell.state.agents.default_agent;
    // A refused key may be tested again, as the provider may have been wrong for a moment.
    let ready = matches!(agents::readiness(shell, default), Readiness::Ready | Readiness::KeyRefused);
    let accounts = shell.services.agents.is_some();
    let live = ready && accounts && !shell.agents.testing();
    let hover = if !accounts {
        Some(tr("Not available yet"))
    } else if !ready {
        Some(fmt(&tr("Set up {agent} first: it is the default agent."), &[("agent", default.name())]))
    } else {
        None
    };
    let (_, resp) = button(shell, ui, p, "settings.agents.test", pos2(left, y), &test_label, Look::Secondary, live, hover.as_deref());
    if resp.clicked() {
        agents::send_test(shell);
    }
    let note = wrapped(
        &paint,
        pos2(note_x, y),
        note_w,
        &tr("Sends one short message to the default agent's provider. It uses a little of your plan or API credit."),
        types::SMALL,
        p.text_secondary,
        3,
    );
    y += note.max(28.0) + 10.0;
    if let Some((text, tone)) = outcome(shell) {
        let r = badge(&paint, p, y, Some(left), None, &text, tone, width);
        widgets::record_visible(ui, &mut shell.registry, "settings.agents.status", "status", r);
        y = r.bottom() + 4.0;
    }
    y += 24.0;
    // The open list, over the rest.
    match page.menu {
        Some(Menu::DefaultAgent) => {
            let items: Vec<String> = Provider::ALL.iter().map(|a| a.name().to_string()).collect();
            let at = Provider::ALL.iter().position(|a| *a == current).unwrap_or(0);
            if let Some(i) = menu(shell, ui, p, page, default_select, "settings.agents.default", &items, at)
                && let Some(chosen) = Provider::ALL.get(i)
            {
                shell.state.agents.default_agent = *chosen;
            }
        }
        Some(Menu::Model(agent)) => {
            let anchor = models[slot(agent)];
            let ids: Vec<&str> = agent.models().iter().map(|(m, _)| *m).collect();
            let items: Vec<String> = ids.iter().map(|m| agents::model_name(m, agent)).collect();
            let at = ids.iter().position(|m| *m == shell.state.agents.model(agent)).unwrap_or(0);
            if anchor != Rect::NOTHING
                && let Some(i) = menu(shell, ui, p, page, anchor, &format!("settings.agents.{}.model", agent.key()), &items, at)
                && let Some(id) = ids.get(i)
            {
                shell.state.agents.set_model(agent, id);
            }
        }
        None => {}
    }
    y - top
}

/// The badge under Check connection and Send test message: what the last of them found.
fn outcome(shell: &Shell) -> Option<(String, Tone)> {
    if shell.agents.checking() {
        return Some((tr("Checking…"), Tone::Quiet));
    }
    if shell.agents.testing() {
        return Some((tr("Sending a test message…"), Tone::Quiet));
    }
    Some(match shell.agents.outcome.as_ref()? {
        Outcome::Checked { at, agents } => {
            let mut parts = vec![fmt(&tr("Checked {when}"), &[("when", &ago(*at))])];
            parts.extend(agents.iter().map(|(a, r)| readiness_text(*a, *r)));
            let tone = match agents.iter().map(|(_, r)| r.tone()).max().unwrap_or(0) {
                0 => Tone::Good,
                1 => Tone::Warn,
                _ => Tone::Bad,
            };
            (parts.join(" · "), tone)
        }
        Outcome::TestAnswered { at, provider } => {
            (fmt(&tr("{agent} answered the test message {when}"), &[("agent", provider.name()), ("when", &ago(*at))]), Tone::Good)
        }
        Outcome::TestFailed { provider, error, .. } => {
            let tone = if matches!(error, AccountError::NotAvailableYet) { Tone::Warn } else { Tone::Bad };
            (fmt(&tr("The test message to {agent} failed: {why}"), &[("agent", provider.name()), ("why", &account_error(error))]), tone)
        }
    })
}

/// The page on a system without agents, or a host without the service: why, in a box.
fn unavailable(ui: &egui::Ui, p: &Palette, system: Option<System>, left: f32, top: f32, width: f32) -> f32 {
    let paint = ui.painter().clone();
    let h = wrapped(
        &paint,
        pos2(left, top + 52.0),
        width,
        &tr("Chat talks to an agent program installed on this computer."),
        types::UI,
        p.text_secondary,
        2,
    );
    let (title, body) = if system == Some(System::NotYet) {
        (tr("Not available on this system yet"), tr(NOT_ON_WINDOWS))
    } else {
        (tr("Not available yet"), tr("This build of Archforge has no agent programs to talk to."))
    };
    let y = top + 52.0 + h + 20.0;
    let g = ux::galley_rows(&paint, &body, types::UI, p.text_secondary, Some((width - 58.0).max(0.0)), 4);
    let r = Rect::from_min_size(pos2(left, y), vec2(width, 16.0 + 22.0 + g.size().y + 16.0));
    paint.rect_filled(r, 10.0, p.page);
    ux::icon_in(&paint, Rect::from_min_size(pos2(r.left() + 16.0, r.top() + 18.0), Vec2::splat(14.0)), &icons::TERMINAL, p.text_secondary);
    ux::text(&paint, pos2(r.left() + 42.0, r.top() + 14.0), &title, types::BODY, p.text, Some(width - 58.0));
    paint.galley(pos2(r.left() + 42.0, r.top() + 38.0), g, p.text_secondary);
    r.bottom() + 28.0 - top
}

/// The notice for a system without a secure store (A7): keys can't be saved, nothing falls back,
/// and why in the system's words. The store's answer holds until Archforge restarts, so the notice
/// says to restart rather than offering to look again.
fn no_store(ui: &egui::Ui, p: &Palette, why: &str, left: f32, y: f32, width: f32) -> f32 {
    let paint = ui.painter().clone();
    let room = Some((width - 20.0).max(0.0));
    let g = ux::galley_rows(&paint, &tr(NO_STORE), types::SMALL, p.text_secondary, room, 4);
    let w = ux::galley_rows(&paint, why, types::CAPTION, p.text_muted, room, 3);
    let r = Rect::from_min_size(pos2(left, y), vec2(width, 34.0 + g.size().y + 6.0 + w.size().y + 12.0));
    paint.rect_filled(r, 8.0, p.amber_tint);
    ux::icon_in(&paint, Rect::from_min_size(pos2(r.left() + 10.0, r.top() + 14.0), Vec2::splat(14.0)), &icons::WARNING, p.amber);
    ux::text(&paint, pos2(r.left() + 30.0, r.top() + 12.0), &tr("No secure store on this system"), types::UI, p.text, Some(width - 40.0));
    let body_y = r.top() + 34.0;
    let gh = g.size().y;
    paint.galley(pos2(r.left() + 10.0, body_y), g, p.text_secondary);
    paint.galley(pos2(r.left() + 10.0, body_y + gh + 6.0), w, p.text_muted);
    r.bottom()
}

/// The card's badge: what was found of the program, or Codex's expired sign-in.
fn card_badge(shell: &Shell, agent: Provider) -> (String, Tone) {
    let program = agent.program();
    match shell.agents.look.program(agent) {
        Program::NotLooked if shell.agents.looking() => (tr("Looking…"), Tone::Quiet),
        Program::NotLooked => (tr("Not looked for yet"), Tone::Quiet),
        Program::Found { version, tested } => {
            let what = format!("{program} {version}");
            if agent == Provider::Codex
                && shell.state.agents.codex_sign_in == CodexSignIn::ChatGpt
                && matches!(shell.agents.codex, Some(Ok(CodexAccount::Expired { .. })))
            {
                (tr("Sign-in expired"), Tone::Warn)
            } else if *tested {
                (fmt(&tr("Found · {what}"), &[("what", &what)]), Tone::Good)
            } else {
                (fmt(&tr("Not yet tested with this version · {what}"), &[("what", &what)]), Tone::Warn)
            }
        }
        Program::TooOld { version, .. } => (fmt(&tr("Too old · {what}"), &[("what", &format!("{program} {version}"))]), Tone::Warn),
        Program::Unusable { .. } => (tr("Can't be used"), Tone::Warn),
        Program::Missing => (tr("Not found"), Tone::Warn),
    }
}

/// An agent's card: its name and badge, then its key or sign-in and its model, or how to install
/// its program. Gives its bottom and its model select.
fn card(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, page: &mut Page, agent: Provider, left: f32, top: f32, width: f32) -> (f32, Rect) {
    let paint = ui.painter().clone();
    let x = left + 16.0;
    let inner = (width - 32.0).max(0.0);
    ux::icon_in(&paint, Rect::from_center_size(pos2(x + 8.0, top + 26.0), Vec2::splat(16.0)), &icons::AGENT, p.text);
    ux::text_mid(&paint, x + 24.0, top + 26.0, agent.name(), types::BODY, p.text, Some(inner / 2.0));
    let (text, tone) = card_badge(shell, agent);
    let r = badge(&paint, p, top + 16.0, None, Some(left + width - 16.0), &text, tone, inner / 2.0);
    widgets::record_visible(ui, &mut shell.registry, &format!("settings.agents.{}.status", agent.key()), "status", r);
    let mut y = top + 52.0;
    let mut model = Rect::NOTHING;
    let program = shell.agents.look.program(agent).clone();
    if program.found() {
        let ready_for_model = match agent {
            Provider::Claude => {
                ux::text(&paint, pos2(x, y), &tr("API key"), types::SMALL, p.text_secondary, Some(inner));
                y = key_field(shell, ui, p, page, agent, x, y + 20.0, inner);
                shell.agents.look.key(agent).saved()
            }
            Provider::Codex => {
                ux::text(&paint, pos2(x, y), &tr("Sign-in"), types::SMALL, p.text_secondary, Some(inner));
                y = codex_sign_in(shell, ui, p, page, x, y + 20.0, inner);
                match shell.state.agents.codex_sign_in {
                    CodexSignIn::ApiKey => shell.agents.look.key(agent).saved(),
                    CodexSignIn::ChatGpt => matches!(shell.agents.codex, Some(Ok(CodexAccount::SignedIn { .. }))),
                }
            }
        };
        y += 12.0;
        ux::text(&paint, pos2(x, y), &tr("Model"), types::SMALL, p.text_secondary, Some(inner));
        model = Rect::from_min_size(pos2(x, y + 20.0), vec2(300.0_f32.min(inner), 28.0));
        let value = agents::model_name(shell.state.agents.model(agent), agent);
        let hover = match (agent, shell.state.agents.codex_sign_in) {
            (Provider::Codex, CodexSignIn::ChatGpt) => tr("Sign in first"),
            _ => tr("Save a key first"),
        };
        select(
            shell,
            ui,
            p,
            page,
            &format!("settings.agents.{}.model", agent.key()),
            model,
            &value,
            ready_for_model,
            Some(&hover),
            Menu::Model(agent),
            &fmt(&tr("{agent} model"), &[("agent", agent.name())]),
        );
        y = model.bottom();
    } else {
        y = install(shell, ui, p, agent, &program, x, y - 8.0, inner);
    }
    let card = Rect::from_min_max(pos2(left, top), pos2(left + width, y + 16.0));
    paint.rect_stroke(card, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    (card.bottom(), model)
}

/// How to get the program when it is missing, too old or cannot be used (A3): why, the command,
/// Look again and the maker's installation guide. Gives where it ends.
fn install(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, agent: Provider, program: &Program, x: f32, top: f32, inner: f32) -> f32 {
    let paint = ui.painter().clone();
    let names = [("program", agent.program()), ("agent", agent.name())];
    let why = match program {
        Program::TooOld { version, floor } => fmt(
            &tr("Archforge found {program} {version}, older than {floor}, the oldest version it works with. Update it, then choose Look again."),
            &[("program", agent.program()), ("version", version), ("floor", floor)],
        ),
        Program::Unusable { why } => {
            fmt(&tr("Archforge found the {program} program but can't use it: {why}"), &[("program", agent.program()), ("why", why)])
        }
        Program::NotLooked => fmt(&tr("Archforge has not looked for the {program} program yet."), &names),
        _ => fmt(&tr(MISSING), &names),
    };
    let mut y = top + wrapped(&paint, pos2(x, top), inner, &why, types::UI, p.text, 4) + 6.0;
    if !matches!(program, Program::Unusable { .. } | Program::NotLooked) {
        let step = if matches!(program, Program::TooOld { .. }) { tr("1. In a terminal, update it:") } else { tr("1. In a terminal, install it:") };
        ux::text(&paint, pos2(x, y), &step, types::SMALL, p.text_secondary, Some(inner));
        let code = Rect::from_min_size(pos2(x, y + 22.0), vec2(inner, 32.0));
        paint.rect_filled(code, 6.0, p.field);
        ux::code_mid(&paint, code.left() + 10.0, code.center().y, agent.install_command(), 13.0, p.text, Some((inner - 48.0).max(0.0)));
        let copy = Rect::from_center_size(pos2(code.right() - 18.0, code.center().y), Vec2::splat(24.0));
        if icon_button(shell, ui, p, &format!("settings.agents.{}.copy-install", agent.key()), copy, &icons::COPY, &tr("Copy the command")).clicked()
        {
            ui.ctx().copy_text(agent.install_command().to_string());
        }
        y = code.bottom() + 8.0;
        y += wrapped(
            &paint,
            pos2(x, y),
            inner,
            &tr("2. Come back and choose Look again. Archforge checks your PATH and the usual install folders, and sends nothing."),
            types::SMALL,
            p.text_secondary,
            2,
        ) + 8.0;
    }
    let looking = shell.agents.looking();
    let (r, resp) =
        button(shell, ui, p, &format!("settings.agents.{}.look-again", agent.key()), pos2(x, y), &tr("Look again"), Look::Primary, !looking, None);
    if resp.clicked() {
        agents::look(shell);
    }
    let (_, resp) = button(
        shell,
        ui,
        p,
        &format!("settings.agents.{}.guide", agent.key()),
        pos2(r.right() + 8.0, y),
        &tr("Installation guide"),
        Look::Secondary,
        true,
        Some(&tr("Opens in your browser")),
    );
    if resp.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(agent.install_guide()));
    }
    y + 28.0
}

/// An agent's API key: the hidden field and Save key, or the saved key masked to its last four
/// with Replace and Remove; the line under it says where it is kept or what went wrong. Gives
/// where it ends.
fn key_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, page: &mut Page, agent: Provider, x: f32, y: f32, inner: f32) -> f32 {
    let paint = ui.painter().clone();
    let i = slot(agent);
    let store = shell.agents.look.store.clone();
    let key = shell.agents.look.key(agent).clone();
    let saving = shell.agents.saving(agent);
    let refused = shell.agents.refused[i].clone();
    let field = Rect::from_min_size(pos2(x, y), vec2(300.0_f32.min(inner - 160.0).max(120.0), 28.0));
    let id = key_field_id(agent);
    let store_words = store_name(&store);
    let (help, bad) = if store.missing() {
        if !page.drafts[i].is_empty() {
            forget_draft(ui.ctx(), page, agent);
        }
        paint.rect_filled(field, 6.0, p.field.gamma_multiply(0.6));
        ux::text_mid(
            &paint,
            field.left() + 8.0,
            field.center().y,
            &tr("Paste an API key"),
            types::UI,
            p.text_muted.gamma_multiply(0.7),
            Some(field.width() - 16.0),
        );
        widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
        let none = tr("Can't be saved: there is no secure store to keep it in.");
        let _ = button(
            shell,
            ui,
            p,
            &format!("settings.agents.{}.save", agent.key()),
            pos2(field.right() + 8.0, y),
            &tr("Save key"),
            Look::Secondary,
            false,
            Some(&none),
        );
        (none, false)
    } else if let (Key::Saved { last4 }, false) = (&key, page.replacing[i]) {
        paint.rect_filled(field, 6.0, p.field);
        let masked = format!("{} {last4}", "•".repeat(20));
        ux::text_mid(&paint, field.left() + 8.0, field.center().y, &masked, types::UI, p.text, Some(field.width() - 16.0));
        if refused.is_some() {
            paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
        }
        let shown = widgets::egui_id(&format!("{id}.masked"));
        let r = ui.interact(field, shown, Sense::hover());
        let _ = r.accessible_name(fmt(&tr("API key saved, ending in {last4}"), &[("last4", last4)]));
        widgets::record_visible(ui, &mut shell.registry, &format!("{id}.masked"), "text", field);
        let (r, resp) = button(
            shell,
            ui,
            p,
            &format!("settings.agents.{}.replace", agent.key()),
            pos2(field.right() + 8.0, y),
            &tr("Replace"),
            Look::Secondary,
            !saving,
            None,
        );
        if resp.clicked() {
            page.replacing[i] = true;
            page.refusal[i] = None;
            page.focus_key = Some(agent);
        }
        let (_, resp) = button(
            shell,
            ui,
            p,
            &format!("settings.agents.{}.remove", agent.key()),
            pos2(r.right() + 8.0, y),
            &tr("Remove"),
            Look::Danger,
            !saving,
            None,
        );
        if resp.clicked() {
            agents::remove_key(shell, agent);
        }
        match refused {
            Some(detail) => (
                fmt(
                    &tr("The provider refused this key when the test message was sent ({detail}). Replace it with a valid key, or remove it."),
                    &[("detail", &detail)],
                ),
                true,
            ),
            None => (fmt(&tr("Kept in {store}. Archforge never shows it again; Replace pastes a new one."), &[("store", &store_words)]), false),
        }
    } else {
        paint.rect_filled(field, 6.0, p.field);
        let font = types::UI.font();
        let row = ui.fonts_mut(|f| f.row_height(&font));
        let text_rect = Rect::from_min_size(pos2(field.left() + 8.0, field.center().y - row / 2.0), vec2((field.width() - 16.0).max(0.0), row));
        let hint = egui::RichText::new(tr("Paste an API key")).font(font.clone()).color(p.text_muted);
        let edit = egui::TextEdit::singleline(&mut page.drafts[i])
            .id(widgets::egui_id(&id))
            .font(font)
            .text_color(p.text)
            .hint_text(hint)
            .password(true)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .char_limit(agents::MAX_KEY_BYTES)
            .desired_width(text_rect.width());
        let resp = ui.put(text_rect, edit);
        // A hidden field keeps no undo history: no older copy of the key stays in its memory.
        if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), widgets::egui_id(&id)) {
            state.clear_undoer();
            state.store(ui.ctx(), widgets::egui_id(&id));
        }
        let _ = resp.clone().accessible_name(fmt(&tr("{agent} API key"), &[("agent", agent.name())]));
        widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
        if page.focus_key == Some(agent) {
            resp.request_focus();
            page.focus_key = None;
        }
        if resp.has_focus() {
            paint.rect_stroke(field, 6.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
        }
        let typed = !page.drafts[i].trim().is_empty();
        let enter = resp.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter));
        let label = if saving { tr("Saving…") } else { tr("Save key") };
        let (r, save) = button(
            shell,
            ui,
            p,
            &format!("settings.agents.{}.save", agent.key()),
            pos2(field.right() + 8.0, y),
            &label,
            Look::Primary,
            typed && !saving,
            None,
        );
        if (save.clicked() || (enter && typed)) && !saving {
            let draft = std::mem::take(&mut page.drafts[i]);
            forget_draft(ui.ctx(), page, agent);
            match KeyText::new(draft) {
                Ok(k) => {
                    page.refusal[i] = None;
                    page.replacing[i] = false;
                    agents::save_key(shell, agent, k);
                }
                Err(why) => page.refusal[i] = Some(why),
            }
        }
        if page.replacing[i] {
            let (_, resp) = button(
                shell,
                ui,
                p,
                &format!("settings.agents.{}.cancel", agent.key()),
                pos2(r.right() + 8.0, y),
                &tr("Cancel"),
                Look::Secondary,
                true,
                None,
            );
            if resp.clicked() {
                page.replacing[i] = false;
                page.refusal[i] = None;
                forget_draft(ui.ctx(), page, agent);
            }
        }
        match page.refusal[i] {
            Some(KeyRefusal::Empty) => (tr("Paste a key first."), true),
            Some(KeyRefusal::NotOneWord) => (tr("A key is one word, without spaces."), true),
            Some(KeyRefusal::TooLong) => (tr("That is longer than any API key."), true),
            None => (
                match agent {
                    Provider::Claude => {
                        fmt(&tr("Saved in {store}, never in the project. Create a key in your Anthropic account."), &[("store", &store_words)])
                    }
                    Provider::Codex => {
                        fmt(&tr("Saved in {store}, never in the project. Create a key in your OpenAI account."), &[("store", &store_words)])
                    }
                },
                false,
            ),
        }
    };
    let (help, bad) = match (&shell.agents.key_error[i], &key) {
        (Some(why), _) => (fmt(&tr("The secure store refused: {why}"), &[("why", why)]), true),
        (None, Key::Unreadable { why }) => (fmt(&tr("The secure store could not be read: {why}"), &[("why", why)]), true),
        _ => (help, bad),
    };
    let h = wrapped(&paint, pos2(x, field.bottom() + 6.0), inner, &help, types::CAPTION, if bad { p.danger } else { p.text_muted }, 3);
    field.bottom() + 6.0 + h
}

/// Codex's sign-in: the two ways, then the ChatGPT sign-in's row (or the device code while a
/// sign-in runs) or the API key. Gives where it ends.
fn codex_sign_in(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, page: &mut Page, x: f32, y: f32, inner: f32) -> f32 {
    let paint = ui.painter().clone();
    let store_missing = shell.agents.look.store.missing();
    let way = shell.state.agents.codex_sign_in;
    let no_store = tr("Can't be saved: there is no secure store to keep it in.");
    let picked = segmented(
        shell,
        ui,
        p,
        ["settings.agents.codex.way.chatgpt", "settings.agents.codex.way.api-key"],
        pos2(x, y),
        [tr("Sign in with ChatGPT"), tr("API key")],
        if way == CodexSignIn::ChatGpt { 0 } else { 1 },
        [true, !store_missing],
        [None, store_missing.then_some(no_store.as_str())],
    );
    match picked {
        Some(0) => {
            shell.state.agents.codex_sign_in = CodexSignIn::ChatGpt;
            // A key typed for the other way does not wait behind it.
            forget_draft(ui.ctx(), page, Provider::Codex);
            page.replacing[slot(Provider::Codex)] = false;
        }
        Some(_) => shell.state.agents.codex_sign_in = CodexSignIn::ApiKey,
        None => {}
    }
    let y = y + 40.0;
    if shell.state.agents.codex_sign_in == CodexSignIn::ApiKey {
        return codex_key(shell, ui, p, page, x, y, inner);
    }
    let right = x + inner;
    // A sign-in under way: its device code, or the wait for the browser.
    if let Some(login) = &shell.agents.login {
        if let Some((code, url, minutes)) = login.code.clone() {
            return device_code(shell, ui, p, x, y, inner, &code, &url, minutes);
        }
        person_row(&paint, p, x, y, &tr("Waiting for you to finish in the browser…"), p.text_secondary, inner - 100.0);
        let w = button_width(&paint, p, &tr("Cancel"));
        let (_, resp) = button(shell, ui, p, "settings.agents.codex.cancel", pos2(right - w, y), &tr("Cancel"), Look::Secondary, true, None);
        if resp.clicked() {
            agents::cancel_sign_in(shell);
        }
        let h = wrapped(&paint, pos2(x, y + 36.0), inner, &tr("The codex program keeps this sign-in itself."), types::CAPTION, p.text_muted, 2);
        return y + 36.0 + h;
    }
    let account = shell.agents.codex.clone();
    let (line, action): (String, Option<(String, Look, bool, Option<String>)>) = match &account {
        None if shell.agents.looking() || shell.agents.reading_account() => (tr("Reading the sign-in…"), None),
        None => (tr("Not checked yet"), None),
        Some(Ok(CodexAccount::NotSignedIn)) => (tr("Not signed in"), Some((tr("Sign in…"), Look::Primary, true, None))),
        Some(Ok(CodexAccount::SignedIn { email, plan })) => (
            match plan {
                Some(plan) => fmt(&tr("Signed in as {email} · {plan}"), &[("email", email), ("plan", plan)]),
                None => fmt(&tr("Signed in as {email}"), &[("email", email)]),
            },
            Some((tr("Sign out"), Look::Secondary, !shell.agents.signing_out(), None)),
        ),
        // Codex may not say whose sign-in expired.
        Some(Ok(CodexAccount::Expired { email })) if email.is_empty() => {
            (tr("Sign-in expired"), Some((tr("Sign in again…"), Look::Primary, true, None)))
        }
        Some(Ok(CodexAccount::Expired { email })) => {
            (fmt(&tr("The ChatGPT sign-in for {email} has expired."), &[("email", email)]), Some((tr("Sign in again…"), Look::Primary, true, None)))
        }
        Some(Err(AccountError::NotAvailableYet)) => (
            tr("Signing in with ChatGPT is not available in this version yet."),
            Some((tr("Sign in…"), Look::Primary, false, Some(tr("Not available yet")))),
        ),
        Some(Err(e)) => (fmt(&tr("The sign-in could not be read: {why}"), &[("why", &account_error(e))]), None),
    };
    let mut room = inner;
    if let Some((label, look, live, hover)) = action {
        let w = button_width(&paint, p, &label);
        room = inner - w - 12.0;
        let (_, resp) = button(shell, ui, p, "settings.agents.codex.sign-in", pos2(right - w, y), &label, look, live, hover.as_deref());
        if resp.clicked() {
            match &account {
                Some(Ok(CodexAccount::SignedIn { .. })) => agents::sign_out(shell),
                _ => agents::sign_in(shell, LoginWay::Browser),
            }
        }
    }
    person_row(&paint, p, x, y, &line, p.text, room - 24.0);
    let mut y = y + 36.0;
    if let Some(e) = &shell.agents.login_error {
        y += wrapped(&paint, pos2(x, y), inner, &account_error(e), types::CAPTION, p.danger, 2) + 4.0;
    }
    let help = match &account {
        // Said only where the host keeps it so, which is where no secure store opened.
        Some(Ok(CodexAccount::SignedIn { .. })) if shell.agents.look.sign_in_files => tr(OWN_FILES),
        Some(Ok(CodexAccount::SignedIn { .. })) => tr("The codex program keeps this sign-in itself."),
        Some(Ok(_)) => tr("Sign in opens your browser; the codex program keeps the sign-in itself."),
        _ => tr("The codex program keeps the sign-in itself."),
    };
    y += wrapped(&paint, pos2(x, y), inner, &help, types::CAPTION, p.text_muted, 2);
    if matches!(&account, Some(Ok(CodexAccount::NotSignedIn | CodexAccount::Expired { .. }))) {
        // No browser here: the device code, as a link.
        let label = tr("No browser on this computer? Sign in with a device code.");
        let g = ux::galley(&paint, &label, types::CAPTION, p.accent, Some(inner));
        let r = Rect::from_min_size(pos2(x, y + 2.0), g.size());
        let resp = control(shell, ui, "settings.agents.codex.device-code", "link", r, true);
        let _ = resp.clone().accessible_name(&label);
        paint.galley(r.min, g, p.accent);
        if resp.hovered() {
            paint.hline(r.x_range(), r.bottom(), Stroke::new(1.0, p.accent));
        }
        focus_ring(&paint, p, r, 3.0, &resp);
        if resp.clicked() {
            agents::sign_in(shell, LoginWay::DeviceCode);
        }
        y = r.bottom();
    }
    y
}

/// A row that starts with the person icon, its text centred on the row's line.
fn person_row(paint: &egui::Painter, p: &Palette, x: f32, y: f32, text: &str, ink: egui::Color32, room: f32) {
    ux::icon_in(paint, Rect::from_center_size(pos2(x + 8.0, y + 14.0), Vec2::splat(16.0)), &icons::PERSON, p.text_secondary);
    ux::text_mid(paint, x + 24.0, y + 14.0, text, types::UI, ink, Some(room.max(0.0)));
}

/// The device code while Codex's sign-in waits for it to be entered (A6): the code with Copy, the
/// sign-in page's address under it as selectable text with Copy (or, for an address off the
/// sign-in host, a line saying Archforge does not open it), then Open sign-in page and Cancel.
/// Gives where the card's caption ends.
fn device_code(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, x: f32, y: f32, inner: f32, code: &str, url: &str, minutes: u32) -> f32 {
    let paint = ui.painter().clone();
    let r = Rect::from_min_size(pos2(x, y), vec2(inner, 168.0));
    paint.rect_filled(r, 8.0, p.page);
    let ix = r.left() + 12.0;
    ux::text(&paint, pos2(ix, r.top() + 12.0), &tr("Sign in with a device code"), types::UI, p.text, Some(inner - 24.0));
    wrapped(&paint, pos2(ix, r.top() + 36.0), inner - 24.0, &device_code_hint(minutes), types::SMALL, p.text_secondary, 2);
    let shown = ux::code_mid(&paint, ix, r.top() + 72.0, code, 20.0, p.text, Some(inner - 64.0));
    widgets::record_visible(ui, &mut shell.registry, "settings.agents.codex.code", "text", shown);
    let copy = Rect::from_center_size(pos2(shown.right() + 18.0, shown.center().y), Vec2::splat(24.0));
    if icon_button(shell, ui, p, "settings.agents.codex.copy-code", copy, &icons::COPY, &tr("Copy the code")).clicked() {
        ui.ctx().copy_text(code.to_string());
    }
    let approved = agents::approved_sign_in_page(url).map(str::to_string);
    sign_in_address(shell, ui, p, ix, r.top() + 102.0, inner - 64.0, approved.as_deref());
    let row = r.top() + 128.0;
    let (b, resp) = button(
        shell,
        ui,
        p,
        "settings.agents.codex.open-page",
        pos2(ix, row),
        &tr("Open sign-in page"),
        Look::Primary,
        approved.is_some(),
        Some(&if approved.is_some() { tr("Opens in your browser") } else { tr("Not an address Archforge opens") }),
    );
    if resp.clicked()
        && let Some(url) = approved
    {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
    let w = button_width(&paint, p, &tr("Cancel"));
    ux::text_mid(
        &paint,
        b.right() + 10.0,
        b.center().y,
        &tr("Waiting for you to finish in the browser…"),
        types::SMALL,
        p.text_muted,
        Some((r.right() - 24.0 - w - b.right() - 10.0).max(0.0)),
    );
    let (_, resp) = button(shell, ui, p, "settings.agents.codex.cancel", pos2(r.right() - 12.0 - w, row), &tr("Cancel"), Look::Secondary, true, None);
    if resp.clicked() {
        agents::cancel_sign_in(shell);
    }
    let h = wrapped(
        &paint,
        pos2(x, r.bottom() + 8.0),
        inner,
        &tr("The code comes from the codex program, which keeps the sign-in itself. Archforge only shows it."),
        types::CAPTION,
        p.text_muted,
        2,
    );
    r.bottom() + 8.0 + h
}

/// The row under the device code centred on `mid`: the sign-in page's address, at most `room` wide,
/// as read-only text a person can select, with an icon that copies it, so the code can be entered
/// on another device. Only an address on the sign-in host (`approved`) is shown; for any other the
/// row says Archforge does not open it, and nothing copies it.
fn sign_in_address(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, x: f32, mid: f32, room: f32, approved: Option<&str>) {
    let id = "settings.agents.codex.address";
    let Some(url) = approved else {
        let shown = ux::text_mid(ui.painter(), x, mid, &tr("Not an address Archforge opens"), types::SMALL, p.danger, Some(room.max(0.0)));
        widgets::record_visible(ui, &mut shell.registry, id, "text", shown);
        return;
    };
    let font = types::UI.font();
    let row = ui.fonts_mut(|f| f.row_height(&font));
    let width = ux::galley(ui.painter(), url, types::UI, p.text, None).size().x.min(room.max(0.0));
    let at = Rect::from_min_size(pos2(x, mid - row / 2.0), vec2(width, row));
    // Read-only: a `&str` buffer takes no typing, yet its text can be selected and copied.
    let mut text = url;
    let edit = egui::TextEdit::singleline(&mut text)
        .id(widgets::egui_id(id))
        .font(font)
        .text_color(p.text)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(width);
    // A field too narrow for the whole address cuts it without a mark: the pointer shows it whole.
    let resp = ui.put(at, edit).on_hover_text(url);
    let _ = resp.clone().accessible_name(tr("Sign-in page address"));
    widgets::record_visible(ui, &mut shell.registry, id, "text", at);
    let copy = Rect::from_center_size(pos2(at.right() + 18.0, mid), Vec2::splat(24.0));
    if icon_button(shell, ui, p, "settings.agents.codex.copy-address", copy, &icons::COPY, &tr("Copy the address")).clicked() {
        ui.ctx().copy_text(url.to_string());
    }
}

/// Codex on an API key: the saved key's row (A3), or the field to paste one.
fn codex_key(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, page: &mut Page, x: f32, y: f32, inner: f32) -> f32 {
    let agent = Provider::Codex;
    let i = slot(agent);
    let key = shell.agents.look.key(agent).clone();
    let Key::Saved { last4 } = key else { return key_field(shell, ui, p, page, agent, x, y, inner) };
    if page.replacing[i] || shell.agents.refused[i].is_some() {
        return key_field(shell, ui, p, page, agent, x, y, inner);
    }
    let paint = ui.painter().clone();
    let saving = shell.agents.saving(agent);
    let right = x + inner;
    let remove_w = button_width(&paint, p, &tr("Remove"));
    let replace_w = button_width(&paint, p, &tr("Replace"));
    let (_, resp) = button(shell, ui, p, "settings.agents.codex.remove", pos2(right - remove_w, y), &tr("Remove"), Look::Danger, !saving, None);
    if resp.clicked() {
        agents::remove_key(shell, agent);
    }
    let (_, resp) = button(
        shell,
        ui,
        p,
        "settings.agents.codex.replace",
        pos2(right - remove_w - 8.0 - replace_w, y),
        &tr("Replace"),
        Look::Secondary,
        !saving,
        None,
    );
    if resp.clicked() {
        page.replacing[i] = true;
        page.focus_key = Some(agent);
    }
    ux::icon_in(&paint, Rect::from_center_size(pos2(x + 7.0, y + 14.0), Vec2::splat(14.0)), &icons::KEY, p.text_secondary);
    let line = fmt(&tr("API key saved · ends in {last4}"), &[("last4", &last4)]);
    let shown = ux::text_mid(&paint, x + 22.0, y + 14.0, &line, types::UI, p.text, Some((inner - remove_w - replace_w - 40.0).max(0.0)));
    widgets::record_visible(ui, &mut shell.registry, "settings.agents.codex.key.masked", "text", shown);
    let help = match &shell.agents.key_error[i] {
        Some(why) => (fmt(&tr("The secure store refused: {why}"), &[("why", why)]), p.danger),
        None => (
            fmt(
                &tr("Kept in {store}, never in the project, and handed to codex only when Chat runs it."),
                &[("store", &store_name(&shell.agents.look.store))],
            ),
            p.text_muted,
        ),
    };
    y + 36.0 + wrapped(&paint, pos2(x, y + 36.0), inner, &help.0, types::CAPTION, help.1, 2)
}

/// A filled select showing `value`; a click opens or closes its list. One that is not `live` says
/// `hover` and opens nothing.
fn select(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    page: &mut Page,
    id: &str,
    r: Rect,
    value: &str,
    live: bool,
    hover: Option<&str>,
    which: Menu,
    name: &str,
) {
    let paint = ui.painter().clone();
    let mut resp = control(shell, ui, id, "select", r, live);
    if !live && let Some(h) = hover {
        resp = resp.on_hover_text(h);
    }
    let _ = resp.clone().accessible_name(name);
    paint.rect_filled(r, 6.0, if live { p.field } else { p.field.gamma_multiply(0.7) });
    let ink = if live { p.text } else { p.text_muted };
    ux::text_mid(&paint, r.left() + 8.0, r.center().y, value, types::UI, ink, Some((r.width() - 36.0).max(0.0)));
    ux::icon_mid(&paint, pos2(r.right() - 16.0, r.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    focus_ring(&paint, p, r, 6.0, &resp);
    if live && resp.clicked() {
        page.menu = if page.menu == Some(which) { None } else { Some(which) };
    }
}

/// A select's list under `anchor`, on a layer over the dialog, the current item ticked; gives the
/// index picked, closing on a pick or a click elsewhere.
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, page: &mut Page, anchor: Rect, id: &str, items: &[String], current: usize) -> Option<usize> {
    let height = 8.0 + items.len() as f32 * 30.0;
    let r = ux::drop_rect(ui, anchor, anchor.left(), vec2(anchor.width(), height));
    let ctx = ui.ctx().clone();
    let picked = egui::Area::new(widgets::egui_id(&format!("{id}.layer")))
        .order(egui::Order::Tooltip)
        .fixed_pos(r.min)
        .constrain(false)
        .fade_in(false)
        .show(&ctx, |ui| {
            ui.set_clip_rect(ctx.content_rect());
            ui.allocate_rect(r, Sense::hover());
            let paint = ui.painter().clone();
            ux::shadow(&paint, r, 8.0, p.shadow);
            paint.rect_filled(r, 8.0, p.surface);
            paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            let mut picked = None;
            for (i, item) in items.iter().enumerate() {
                let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * 30.0), vec2((r.width() - 8.0).max(0.0), 30.0));
                let resp: Response = control(shell, ui, &format!("{id}.{i}"), "menu-item", row, true);
                let _ = resp.clone().accessible_name(item);
                if resp.hovered() || resp.has_focus() {
                    paint.rect_filled(row, 5.0, p.page);
                }
                ux::text_mid(&paint, row.left() + 10.0, row.center().y, item, types::UI, p.text, Some((row.width() - 40.0).max(0.0)));
                if i == current {
                    ux::icon_mid(&paint, pos2(row.right() - 14.0, row.center().y), &icons::TICK, p.accent);
                }
                if resp.clicked() {
                    picked = Some(i);
                }
            }
            picked
        })
        .inner;
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside {
        page.menu = None;
    }
    picked
}

/// The design's segmented control: two choices, the chosen one raised; gives the one clicked.
fn segmented(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    ids: [&str; 2],
    at: egui::Pos2,
    labels: [String; 2],
    chosen: usize,
    live: [bool; 2],
    hovers: [Option<&str>; 2],
) -> Option<usize> {
    let paint = ui.painter().clone();
    let widths: Vec<f32> = labels.iter().map(|l| ux::galley(&paint, l, types::UI, p.text, None).size().x + 24.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let back = Rect::from_min_size(at, vec2(total, 28.0));
    paint.rect_filled(back, 7.0, p.field);
    let mut x = at.x + 2.0;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(0.0);
        let r = Rect::from_min_size(pos2(x, at.y + 2.0), vec2(w, 24.0));
        x += w + 2.0;
        let on = live.get(i).copied().unwrap_or(false);
        let mut resp = control(shell, ui, ids.get(i).copied().unwrap_or_default(), "option", r, on);
        if let Some(Some(h)) = hovers.get(i) {
            resp = resp.on_hover_text(*h);
        }
        let _ = resp.clone().accessible_name(label);
        if i == chosen {
            paint.rect_filled(r, 5.0, p.surface);
            paint.rect_stroke(r, 5.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let ink = if !on {
            p.text_muted.gamma_multiply(0.7)
        } else if i == chosen {
            p.text
        } else {
            p.text_secondary
        };
        let g = ux::galley(&paint, label, types::UI, ink, None);
        paint.galley(r.center() - g.size() / 2.0, g, ink);
        focus_ring(&paint, p, r, 5.0, &resp);
        if on && resp.clicked() && i != chosen {
            clicked = Some(i);
        }
    }
    clicked
}

/// The design's switch, 28 by 16: the accent track with the knob right when on.
pub(crate) fn toggle(shell: &mut Shell, ui: &egui::Ui, p: &Palette, id: &str, r: Rect, on: bool, name: &str) -> Response {
    let paint = ui.painter().clone();
    let resp = control(shell, ui, id, "toggle", r, true);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, true, on, name));
    paint.rect_filled(r, 8.0, if on { p.accent_fill } else { p.control_off });
    let knob = if on { r.right() - 8.0 } else { r.left() + 8.0 };
    paint.circle_filled(pos2(knob, r.center().y), 6.0, p.on_accent);
    focus_ring(&paint, p, r, 8.0, &resp);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_and_hours_take_their_plural_forms() {
        crate::i18n::set_language("en");
        assert_eq!(ago_secs(30), "just now");
        assert_eq!(ago_secs(60), "1 minute ago");
        assert_eq!(ago_secs(5 * 60), "5 minutes ago");
        assert_eq!(ago_secs(3600), "1 hour ago");
        assert_eq!(ago_secs(3 * 3600), "3 hours ago");
        assert!(device_code_hint(1).ends_with("It runs out in 1 minute."));
        assert!(device_code_hint(15).ends_with("It runs out in 15 minutes."));
        crate::i18n::set_language("de");
        assert_eq!(ago_secs(60), "vor 1 Minute");
        assert_eq!(ago_secs(2 * 60), "vor 2 Minuten");
        assert!(device_code_hint(1).ends_with("Er läuft in 1 Minute ab."));
        assert!(device_code_hint(15).ends_with("Er läuft in 15 Minuten ab."));
        crate::i18n::set_language("fr");
        assert_eq!(ago_secs(3600), "il y a 1 heure");
        assert_eq!(ago_secs(2 * 3600), "il y a 2 heures");
        crate::i18n::set_language("ja");
        assert_eq!(ago_secs(60), "1 分前");
        assert_eq!(ago_secs(5 * 60), "5 分前");
        crate::i18n::set_language("en");
    }
}
