//! Chat's conversations kept in the project (AR-205 over AR-206): the shell's [`ChatStore`] on the
//! host. Every write runs on the project's writer thread ([`crate::projects`]), in order with the
//! model's rows, Save and Don't save, so no chat write races a model write or a reset.
//!
//! A question, and the title a new conversation takes from it, is checked against the API keys in
//! the system's secure store before anything is written: on one of the agents host's threads, never
//! on the writer, since reading a key may wait for the system to ask the person. A question holding
//! a key is refused and nothing is written; only then does the write reach the writer, which writes
//! it as checked. A store that cannot be read refuses the question too: it could not be checked.
//!
//! The shell's times are seconds since 1970; the project stores them as RFC 3339 text in UTC.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use archforge_mof::Uuid;
use archforge_project as project;
use archforge_secrets::{Deleted, Secret, SecretError, SecretName, SecretStore};
use archforge_ui_egui::ai_agents::Provider;
use archforge_ui_egui::chat::FailureKind;
use archforge_ui_egui::chat_store::{
    Anchor, AnchorRole, Answer, ChangeKind, ChangeState, ChatStore, Listed, Outcome, Refusal, Row, Stored, StoredBody, StoredChange, StoredMessage,
    StoredTurn, ToolFacts, ToolStatus, Write,
};

/// The project's conversations, through the projects host's writer and the agents host's keys.
pub struct Chats {
    projects: Arc<crate::projects::Projects>,
    agents: Arc<crate::agents::Agents>,
    /// The commit before each conversation's last deletion, by project, as the writer made it: an
    /// Undo pressed before its deletion answered restores from it ([`Write::Restore`] without one).
    deleted: Before,
}

/// The commits before deletions, read and written on the writer alone.
type Before = Arc<std::sync::Mutex<std::collections::HashMap<(PathBuf, Uuid), String>>>;

impl Chats {
    /// The store of conversations over `projects`' writer, checking questions with `agents`' keys.
    pub fn new(projects: Arc<crate::projects::Projects>, agents: Arc<crate::agents::Agents>) -> Chats {
        Chats { projects, agents, deleted: Before::default() }
    }
}

/// A store that holds no key: what the writer checks a question against once the agents host has
/// checked it against the real one, so the writer never waits on the system's store.
struct Checked;

impl SecretStore for Checked {
    /// No key: the question was checked against the system's store before it came here.
    fn get(&self, name: &SecretName) -> Result<Secret, SecretError> {
        Err(SecretError::NotFound { name: name.as_str().to_string() })
    }
    /// Never keeps one.
    fn set(&self, _name: &SecretName, _secret: &Secret) -> Result<(), SecretError> {
        Err(SecretError::NoSecureStore { reason: "the chat writer keeps no keys".into() })
    }
    /// Has none to remove.
    fn delete(&self, _name: &SecretName) -> Result<Deleted, SecretError> {
        Ok(Deleted::WasAbsent)
    }
}

/// The time the project stores for `secs`.
fn stamp(secs: u64) -> String {
    project::timestamp(secs)
}

/// Seconds since 1970 of a time as the project stores it, `2026-11-06T14:02:11Z`; none for any
/// other text.
pub fn seconds(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    if b.len() != 20 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') || b.get(10) != Some(&b'T') || b.get(19) != Some(&b'Z') {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> { text.get(from..to).filter(|s| s.bytes().all(|c| c.is_ascii_digit()))?.parse().ok() };
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&m) || !(1..=month_days).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Howard Hinnant's days from civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3_600 + mm * 60 + ss).ok()
}

/// The project's code for the shell's provider.
fn provider_to(p: Provider) -> project::Provider {
    match p {
        Provider::Claude => project::Provider::Claude,
        Provider::Codex => project::Provider::Codex,
    }
}

/// The shell's provider for the project's code.
fn provider_from(p: project::Provider) -> Provider {
    match p {
        project::Provider::Claude => Provider::Claude,
        project::Provider::Codex => Provider::Codex,
    }
}

/// The provider whose key a refusal names, from its secret's name.
fn provider_of_key(name: &str) -> Option<Provider> {
    if name == SecretName::ANTHROPIC_API_KEY.as_str() {
        Some(Provider::Claude)
    } else if name == SecretName::OPENAI_API_KEY.as_str() {
        Some(Provider::Codex)
    } else {
        None
    }
}

/// The project's neutral category of a failure; no key saved is kept as a refused key.
fn error_to(kind: FailureKind) -> project::TurnError {
    match kind {
        FailureKind::MissingRuntime => project::TurnError::MissingRuntime,
        FailureKind::NoCredential | FailureKind::InvalidCredential => project::TurnError::InvalidCredential,
        FailureKind::SignInExpired => project::TurnError::SignInExpired,
        FailureKind::RateLimited => project::TurnError::RateLimited,
        FailureKind::Network => project::TurnError::Network,
        FailureKind::ModelUnavailable => project::TurnError::ModelUnavailable,
        FailureKind::Other => project::TurnError::Other,
    }
}

/// The shell's kind of a kept failure.
fn error_from(e: project::TurnError) -> FailureKind {
    match e {
        project::TurnError::MissingRuntime => FailureKind::MissingRuntime,
        project::TurnError::InvalidCredential => FailureKind::InvalidCredential,
        project::TurnError::SignInExpired => FailureKind::SignInExpired,
        project::TurnError::RateLimited => FailureKind::RateLimited,
        project::TurnError::Network => FailureKind::Network,
        project::TurnError::ModelUnavailable => FailureKind::ModelUnavailable,
        project::TurnError::Other => FailureKind::Other,
    }
}

/// The project's outcome of a turn.
fn outcome_to(o: Outcome) -> project::TurnOutcome {
    match o {
        Outcome::Running => project::TurnOutcome::Running,
        Outcome::Completed => project::TurnOutcome::Completed,
        Outcome::Stopped => project::TurnOutcome::Stopped,
        Outcome::Failed => project::TurnOutcome::Failed,
        Outcome::Interrupted => project::TurnOutcome::Interrupted,
    }
}

/// The shell's outcome of a kept turn.
fn outcome_from(o: project::TurnOutcome) -> Outcome {
    match o {
        project::TurnOutcome::Running => Outcome::Running,
        project::TurnOutcome::Completed => Outcome::Completed,
        project::TurnOutcome::Stopped => Outcome::Stopped,
        project::TurnOutcome::Failed => Outcome::Failed,
        project::TurnOutcome::Interrupted => Outcome::Interrupted,
    }
}

/// The project's state of a turn's changes.
fn state_to(s: ChangeState) -> project::ChangeState {
    match s {
        ChangeState::Applied => project::ChangeState::Applied,
        ChangeState::Undone => project::ChangeState::Undone,
        ChangeState::Discarded => project::ChangeState::Discarded,
    }
}

/// The shell's state of a kept turn's changes.
fn state_from(s: project::ChangeState) -> ChangeState {
    match s {
        project::ChangeState::Applied => ChangeState::Applied,
        project::ChangeState::Undone => ChangeState::Undone,
        project::ChangeState::Discarded => ChangeState::Discarded,
    }
}

/// The project's role of an anchor.
fn role_to(r: AnchorRole) -> project::AnchorRole {
    match r {
        AnchorRole::Project => project::AnchorRole::Project,
        AnchorRole::Diagram => project::AnchorRole::Diagram,
        AnchorRole::Selection => project::AnchorRole::Selection,
        AnchorRole::Read => project::AnchorRole::Read,
        AnchorRole::Target => project::AnchorRole::Target,
    }
}

/// The shell's role of a kept anchor.
fn role_from(r: project::AnchorRole) -> AnchorRole {
    match r {
        project::AnchorRole::Project => AnchorRole::Project,
        project::AnchorRole::Diagram => AnchorRole::Diagram,
        project::AnchorRole::Selection => AnchorRole::Selection,
        project::AnchorRole::Read => AnchorRole::Read,
        project::AnchorRole::Target => AnchorRole::Target,
    }
}

/// The shell's anchors as the project takes them.
fn anchors_to(given: &[Anchor]) -> Vec<project::NewAnchor> {
    given
        .iter()
        .map(|a| project::NewAnchor { element: a.element, role: role_to(a.role), metaclass: a.metaclass.clone(), name: a.name.clone() })
        .collect()
}

/// The project's status of a tool call.
fn status_to(s: ToolStatus) -> project::ToolStatus {
    match s {
        ToolStatus::Done => project::ToolStatus::Done,
        ToolStatus::Refused => project::ToolStatus::Refused,
        ToolStatus::Failed => project::ToolStatus::Failed,
    }
}

/// A kept tool row's status as the panel words it: the runtime's own refusal is a refusal too, and
/// a call left running when the turn ended failed.
fn status_from(s: project::ToolStatus) -> ToolStatus {
    match s {
        project::ToolStatus::Done => ToolStatus::Done,
        project::ToolStatus::Refused | project::ToolStatus::Denied => ToolStatus::Refused,
        project::ToolStatus::Failed | project::ToolStatus::Running => ToolStatus::Failed,
    }
}

/// The project's kind of a change to an element.
fn kind_to(k: ChangeKind) -> project::ChangeKind {
    match k {
        ChangeKind::Added => project::ChangeKind::Added,
        ChangeKind::Changed => project::ChangeKind::Changed,
        ChangeKind::Removed => project::ChangeKind::Removed,
    }
}

/// A refusal from the project, in the shell's terms.
fn refused(e: project::Error) -> Refusal {
    match e {
        project::Error::HeldKey { name } => Refusal::HeldKey { provider: provider_of_key(&name) },
        project::Error::ChatListed { .. } => Refusal::Listed,
        // The panel names the conversation itself: only the reason.
        project::Error::ChatNotDeleted { why, .. } | project::Error::ChatNotRestored { why, .. } => Refusal::Failed(why),
        e => Refusal::Failed(e.to_string()),
    }
}

/// How many labels a new conversation tries past the highest it read.
const ORIGIN_TRIES: u32 = 16;

/// The next free `Chat N` of the project: one past the highest kept and the highest of `skip`, the
/// labels of conversations deleted while their Undo is still offered.
fn next_origin(listed: &[project::Listed], skip: &[String]) -> String {
    archforge_ui_egui::chat_store::next_origin(listed.iter().filter_map(|l| l.entry.as_ref().ok()).map(|s| s.chat.origin.as_str()), skip)
}

/// Runs one write against the open project, on the writer.
fn apply(p: &project::Project, write: Write, created_by: &str, deleted: &Before) -> Result<Answer, Refusal> {
    match write {
        Write::Create { chat, title, provider, model, skip, now } => {
            // One past the highest label read and those skipped; a conversation whose row cannot
            // be read may hold that one, so the next ones are tried, a few.
            let first = next_origin(&p.list_chats().map_err(refused)?, &skip);
            let n: u32 = first.strip_prefix("Chat ").and_then(|n| n.parse().ok()).unwrap_or(1);
            let mut last = None;
            for k in 0..ORIGIN_TRIES {
                let origin = format!("Chat {}", n.saturating_add(k));
                let new = project::NewChat {
                    id: chat,
                    origin: origin.clone(),
                    title: title.clone(),
                    provider: provider_to(provider),
                    model: model.clone(),
                    created_by: created_by.to_string(),
                    now: stamp(now),
                };
                match p.create_chat(&new, &Checked) {
                    Ok(()) => return Ok(Answer::Created { origin }),
                    Err(project::Error::ChatValue { what: "the conversation's origin", why }) => last = Some(why),
                    Err(e) => return Err(refused(e)),
                }
            }
            Err(Refusal::Failed(last.unwrap_or_default()))
        }
        Write::Ask { chat, turn, question, text, chips, model, allow_changes, now } => {
            let turn = project::NewTurn { id: turn, chat, model, allow_changes, now: stamp(now) };
            let question = project::NewQuestion { id: question, body: text, anchors: anchors_to(&chips) };
            let number = p.start_turn(&turn, &question, &Checked).map_err(refused)?;
            Ok(Answer::Asked { number })
        }
        Write::Rerun { turn, model, allow_changes, question, now } => {
            let rerun = project::Rerun { turn, model, allow_changes, question: question.map(|(q, chips)| (q, anchors_to(&chips))), now: stamp(now) };
            p.rerun_turn(&rerun, &Checked).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::Reply { id, turn, text, complete, shortened, now } => {
            p.append_reply(&project::NewReply { id, turn, body: text, complete, shortened, now: stamp(now) }).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::Tool { id, turn, facts, arguments, result, error, now } => {
            let call = project::NewToolCall {
                id,
                turn,
                tool: facts.tool,
                command: facts.command,
                detail: facts.detail,
                status: status_to(facts.status),
                changed_model: facts.changed_model,
                element_count: facts.element_count,
                arguments,
                result,
                error,
                anchors: anchors_to(&facts.anchors),
                now: stamp(now),
            };
            p.append_tool_call(&call).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::End { turn, outcome, error, changes, now } => {
            let changes = changes.map(|c| project::TurnChanges {
                changes: c.changes,
                elements: c.elements,
                changed: c
                    .changed
                    .into_iter()
                    .map(|e| project::ChangedElement { element: e.element, kind: kind_to(e.kind), metaclass: e.metaclass, name: e.name })
                    .collect(),
            });
            let end = project::TurnEnd { turn, outcome: outcome_to(outcome), error: error.map(error_to), now: stamp(now), changes };
            p.end_turn(&end).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::States { moves, now } => {
            let moves: Vec<(Uuid, project::ChangeState)> = moves.into_iter().map(|(t, s)| (t, state_to(s))).collect();
            p.set_change_states(&moves, &stamp(now)).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::Model { chat, model, now } => {
            p.set_chat_model(chat, &model, &stamp(now)).map_err(refused)?;
            Ok(Answer::Done)
        }
        Write::Delete { chat } => {
            let done = p.delete_chat(chat).inspect_err(|e| log::warn!("the conversation {chat} was not deleted: {e}")).map_err(refused)?;
            log::info!("the conversation {chat} ({}) was deleted after commit {}", done.origin, done.before);
            deleted.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert((p.path().to_path_buf(), chat), done.before.clone());
            Ok(Answer::Deleted { before: done.before })
        }
        Write::Restore { chat, from } => {
            let remembered = deleted.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&(p.path().to_path_buf(), chat)).cloned();
            let Some(from) = from.or(remembered) else {
                // No deletion of it to undo: listed, or its deletion failed.
                let listed = p.list_chats().is_ok_and(|l| l.iter().any(|c| c.id == chat));
                log::warn!("the conversation {chat} was not brought back: no deletion of it was asked here");
                return Err(if listed { Refusal::Listed } else { Refusal::Failed("no deletion of it was asked in this project".into()) });
            };
            let origin =
                p.restore_chat(chat, &from).inspect_err(|e| log::warn!("the conversation {chat} was not brought back: {e}")).map_err(refused)?;
            log::info!("the conversation {chat} ({origin}) was brought back from commit {from}");
            Ok(Answer::Done)
        }
    }
}

/// A kept conversation's row, in the shell's terms.
fn row(chat: project::Chat, messages: u32) -> Result<Row, String> {
    let updated_at = seconds(&chat.updated_at).ok_or_else(|| format!("its last activity {:?} is not a time", chat.updated_at))?;
    Ok(Row { id: chat.id, origin: chat.origin, title: chat.title, provider: provider_from(chat.provider), model: chat.model, updated_at, messages })
}

/// A whole conversation read back, in the shell's terms.
fn stored(c: project::Conversation) -> Result<Stored, String> {
    let questions = u32::try_from(c.messages.iter().filter(|m| matches!(m.message.role, project::Role::User | project::Role::Assistant)).count())
        .unwrap_or(u32::MAX);
    let turns = c
        .turns
        .into_iter()
        .map(|t| StoredTurn {
            id: t.turn.id,
            number: t.turn.number,
            model: t.turn.model,
            allow_changes: t.turn.allow_changes,
            outcome: outcome_from(t.turn.outcome),
            error: t.turn.error.map(error_from),
            change: t.change.map(|c| StoredChange { changes: c.changes, state: state_from(c.state) }),
        })
        .collect();
    let mut messages = Vec::new();
    for m in c.messages {
        let body = match m.message.role {
            project::Role::User => StoredBody::Question {
                text: m.message.body.unwrap_or_default(),
                chips: m
                    .anchors
                    .iter()
                    .map(|a| Anchor { element: a.element, role: role_from(a.role), metaclass: a.metaclass.clone(), name: a.name.clone() })
                    .collect(),
            },
            project::Role::Assistant => {
                StoredBody::Reply { text: m.message.body.unwrap_or_default(), complete: m.message.complete, shortened: m.message.shortened }
            }
            project::Role::Tool => {
                let Some(t) = m.tool else { return Err(format!("the tool row {} has no details", m.message.id)) };
                let anchors = m
                    .anchors
                    .into_iter()
                    .map(|a| Anchor { element: a.element, role: role_from(a.role), metaclass: a.metaclass, name: a.name })
                    .collect();
                StoredBody::Tool(ToolFacts {
                    tool: t.tool,
                    command: t.command,
                    detail: t.detail,
                    status: status_from(t.status),
                    changed_model: t.changed_model,
                    element_count: t.element_count,
                    anchors,
                })
            }
        };
        messages.push(StoredMessage { id: m.message.id, turn: m.message.turn, body });
    }
    Ok(Stored { row: row(c.chat, questions)?, turns, messages })
}

/// The text a write asks to keep that a person typed: a question, or the title made from one.
fn typed(write: &Write) -> Option<&str> {
    match write {
        Write::Create { title, .. } => Some(title),
        Write::Ask { text, .. } => Some(text),
        Write::Rerun { question: Some((text, _)), .. } => Some(text),
        _ => None,
    }
}

impl ChatStore for Chats {
    /// The conversations, read on the writer.
    fn list(&self, project: &Path) -> Receiver<Result<Vec<Listed>, String>> {
        self.projects.on_writer(project, |p| {
            let listed = p.list_chats().map_err(|e| e.to_string())?;
            Ok(listed.into_iter().map(|l| Listed { id: l.id, entry: l.entry.and_then(|s| row(s.chat, s.messages)) }).collect())
        })
    }

    /// One conversation, whole, read on the writer.
    fn load(&self, project: &Path, chat: Uuid) -> Receiver<Result<Stored, String>> {
        self.projects.on_writer(project, move |p| p.load_chat(chat).map_err(|e| e.to_string()).and_then(stored))
    }

    /// A write on the writer; one that keeps what a person typed is checked against the keys
    /// first, on the agents host's thread, and reaches the writer only once it passed.
    fn write(&self, project: &Path, write: Write) -> Receiver<Result<Answer, Refusal>> {
        let Some(text) = typed(&write).map(str::to_string) else {
            return write_now(&self.projects, project, write, &self.deleted);
        };
        let (tx, rx) = channel();
        let projects = self.projects.clone();
        let deleted = self.deleted.clone();
        let path = project.to_path_buf();
        let checked = self.agents.check_keys(text);
        let started = std::thread::Builder::new().name("archforge-chat-keys".into()).spawn(move || {
            let answer = match checked.recv() {
                Ok(Ok(None)) => {
                    write_now(&projects, &path, write, &deleted).recv().unwrap_or_else(|_| Err(Refusal::Failed("the writer stopped".into())))
                }
                Ok(Ok(Some(provider))) => Err(Refusal::HeldKey { provider }),
                Ok(Err(why)) => Err(Refusal::Failed(why)),
                Err(_) => Err(Refusal::Failed("the keys could not be checked".into())),
            };
            let _ = tx.send(answer);
        });
        if let Err(e) = started {
            let (tx, rx) = channel();
            let _ = tx.send(Err(Refusal::Failed(format!("the question could not be checked: {e}"))));
            return rx;
        }
        rx
    }
}

/// Sends `write` to the writer for the project at `project`; a project no longer open refuses it.
fn write_now(projects: &crate::projects::Projects, project: &Path, write: Write, deleted: &Before) -> Receiver<Result<Answer, Refusal>> {
    let deleted = deleted.clone();
    let created_by = projects.created_by().to_string();
    let shown = project.display().to_string();
    projects.on_writer_open(project, move |p| match p {
        Some(p) => apply(p, write, &created_by, &deleted),
        None => Err(Refusal::Failed(format!("{shown} is no longer open"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A time the project stores reads back exactly; any other text is refused.
    #[test]
    fn a_stored_time_reads_back_as_the_seconds_it_was_made_from() {
        for secs in [0, 1_762_437_731, 1_794_060_120, 4_102_444_799] {
            assert_eq!(seconds(&stamp(secs)), Some(secs), "{secs}");
        }
        for bad in [
            "",
            "2026-02-31T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-11-06 14:02:11Z",
            "2026-13-06T14:02:11Z",
            "2026-11-06T24:02:11Z",
            "x026-11-06T14:02:11Z",
            "2026-11-06T14:02:11+01",
        ] {
            assert_eq!(seconds(bad), None, "{bad:?}");
        }
    }

    /// A project without conversations starts at Chat 1.
    #[test]
    fn the_next_origin_is_one_past_the_highest_kept() {
        assert_eq!(next_origin(&[], &[]), "Chat 1");
        assert_eq!(
            next_origin(&[], &["Chat 7".into(), "Chat 3".into()]),
            "Chat 8",
            "a deleted conversation's label is skipped while it can come back"
        );
    }
}
