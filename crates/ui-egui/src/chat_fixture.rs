//! The Chat panel's states as the tests and the offscreen screens build them: the sample class
//! model open, the fake agents host, conversations kept in memory, and a fixed clock, so a state is
//! reached through the panel's own steps (a question asked and kept, the agent's events taken)
//! rather than by setting its fields.

use std::sync::Arc;

use crate::Shell;
use crate::ai_agents::fake::Fake;
use crate::ai_agents::{Look, Provider};
use crate::chat::ChatEvent;
use crate::chat_store::fake::Memory;
use crate::chat_words::Clock;

/// 2026-11-07 13:26 UTC: 14:26 where the screens are set, an hour east of UTC.
pub const NOW: u64 = 1_794_057_960;
/// The screens' local offset from UTC.
pub const OFFSET: i32 = 3_600;
/// The model the program names once it answers.
pub const NAMED: &str = "claude-sonnet-4-5-20250929";
/// A Claude key, as the fake store keeps it.
pub const KEY: &str = "sk-ant-api03-fixture-key-0000-4F2a";

/// What a Chat state is built over: the fake agents host and the conversations kept in memory.
pub struct Rig {
    pub agents: Arc<Fake>,
    pub store: Arc<Memory>,
    /// The fixed clock's time, which a test may move on.
    pub now: Arc<std::sync::atomic::AtomicU64>,
}

/// Sets `shell` up for Chat: the fake host with both programs found (Claude's key saved when
/// `ready`), conversations in memory, the fixed clock, Claude's Sonnet as the default, and the
/// Chat panel shown.
pub fn rig(shell: &mut Shell, ready: bool) -> Rig {
    let fake = Fake::default();
    let key = if ready { crate::ai_agents::Key::Saved { last4: "4F2a".into() } } else { crate::ai_agents::Key::Absent };
    if let Ok(mut look) = fake.look.lock() {
        *look = Look {
            store: crate::ai_agents::Store::MacKeychain,
            claude: crate::ai_agents::fake::found(Provider::Claude),
            codex: crate::ai_agents::fake::found(Provider::Codex),
            claude_key: key,
            ..Look::default()
        };
    }
    let agents = Arc::new(fake);
    let store = Arc::new(Memory { keys: vec![(Provider::Claude, KEY.into())], ..Memory::default() });
    shell.services.agents = Some(agents.clone());
    shell.services.chats = Some(store.clone());
    let now = Arc::new(std::sync::atomic::AtomicU64::new(NOW));
    let clock = now.clone();
    shell.services.clock = Some(Arc::new(move || Clock { now: clock.load(std::sync::atomic::Ordering::Relaxed), offset: Some(OFFSET) }));
    shell.state.agents.claude_model = "sonnet".into();
    shell.state.show_panel(crate::state::Panel::Agent);
    Rig { agents, store, now }
}

/// Runs what a frame runs for Chat, without a window: the host's answers and events, the panel's
/// steps; enough times for an answer to lead to the next step.
pub fn settle(shell: &mut Shell) {
    let ctx = egui::Context::default();
    for _ in 0..6 {
        crate::ai_agents::start(shell);
        crate::ai_agents::poll(shell, &ctx);
        crate::chat_panel::showing(shell);
        crate::chat::frame(shell, &ctx);
        crate::chat_panel::frame(shell);
    }
}

/// Asks `text` as the person does: typed into the composer and sent, then settled.
pub fn ask(shell: &mut Shell, text: &str) {
    shell.chat_panel.draft = text.to_string();
    crate::chat_panel::send(shell);
    settle(shell);
}

/// The conversation shown's id as the host knows it.
pub fn key(shell: &Shell) -> String {
    shell.chat_panel.chat.as_ref().map(|c| c.key.clone()).unwrap_or_default()
}

/// The host says `event` of the conversation shown, and the panel takes it in.
pub fn emit(rig: &Rig, shell: &mut Shell, event: ChatEvent) {
    rig.agents.emit(&key(shell), event);
    settle(shell);
}

/// Sends a control request on behalf of the conversation shown, as its agent's bridge does with
/// the token its switch gave it (read-only unless it allows changes), and answers it as the window
/// does.
pub fn call(shell: &mut Shell, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let origin = shell.chat_panel.chat.as_ref().and_then(|c| c.origin.clone());
    let conversation = shell.chat_panel.chat.as_ref().map(|c| c.key.clone());
    let read_only = shell.chat_panel.chat.as_ref().is_none_or(|c| !c.allow_changes);
    let (reply, _) = std::sync::mpsc::channel();
    let generation = shell.session_generation();
    let req = crate::control::ControlRequest {
        method: method.to_string(),
        params,
        origin,
        conversation,
        reply,
        state: Arc::new(std::sync::atomic::AtomicU8::new(0)),
        read_only,
        live: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        generation,
    };
    let answer = crate::control::answer_request(shell, &req);
    settle(shell);
    answer
}

/// The id of the sample's element named `name` of class `class`.
pub fn element(shell: &Shell, class: archforge_mof::ClassId, name: &str) -> Option<archforge_mof::Uuid> {
    let st = shell.session.as_ref()?.doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name)?;
    st.uuid(o).ok()
}

/// The history of page 07's frames B7 to B7c: five conversations kept, newest first, the first
/// opened and the history shown. Gives their ids in the list's order.
pub fn history(shell: &mut Shell, rig: &Rig) -> Result<Vec<archforge_mof::Uuid>, String> {
    use crate::chat_store::{Row, fake::Kept};
    let titles = [
        ("Descriptions for classes without one", 1_794_056_520, Provider::Claude, 6),
        ("How Payment relates to Order", 1_794_048_000, Provider::Claude, 4),
        ("Review the checkout package", 1_793_980_800, Provider::Codex, 12),
        ("Naming check for the shop schema", 1_793_696_400, Provider::Codex, 8),
        ("First look at the model", 1_793_523_600, Provider::Claude, 2),
    ];
    let mut ids = Vec::new();
    for (n, (title, at, provider, messages)) in titles.into_iter().enumerate() {
        let id = shell.services.ids.as_ref().map(|ids| ids()).ok_or("no ids")?;
        ids.push(id);
        let origin = format!("Chat {}", 5 - n);
        let row = Row { id, origin, title: title.into(), provider, model: "sonnet".into(), updated_at: at, messages, removed: Default::default() };
        rig.store.hold(Kept { row, turns: Vec::new(), messages: Vec::new(), calls: Vec::new(), ends: Vec::new() });
    }
    if let Some(id) = ids.first() {
        crate::chat_panel::open(shell, *id);
        settle(shell);
    }
    crate::chat_panel::toggle_history(shell);
    settle(shell);
    Ok(ids)
}

/// The sample class model open with Order selected and the Chat panel over it, ready or not.
pub fn sample(shell: &mut Shell, ready: bool) -> Result<Rig, String> {
    shell.adopt_project();
    crate::uml_ui::open_sample(shell)?;
    if let Some(order) = element(shell, archforge_uml::CLASS, "Order") {
        crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [order.to_string()] }))?;
    }
    let rig = rig(shell, ready);
    settle(shell);
    Ok(rig)
}
