//! Conversations kept in the project (AR-206), against a real dolt as the project tests are: made,
//! answered, listed and loaded; kept through closing, Save and Don't save; the format raised with
//! the first conversation and only then; deleted; their change links moved by undo, redo and Don't
//! save; their limits; a kept key refused; a deleted element's anchor kept; damaged chat tables
//! refused without blocking the model. Needs dolt 2.6 or later.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::{Uuid, rows};
use archforge_notations::Session;
use archforge_project::{
    AnchorRole, CHAT_TABLES, ChangeKind, ChangeState, ChangedElement, Error, FORMAT, ModelSchema, NewAnchor, NewChat, NewQuestion, NewReply,
    NewToolCall, NewTurn, Project, Provider, Rerun, Role, ToolStatus, TurnChanges, TurnEnd, TurnError, TurnOutcome, TurnSpan, limits, timestamp,
};
use archforge_secrets::{MemoryStore, Secret, SecretName, SecretStore};
use mysql::prelude::Queryable;
use serde_json::json;

struct Env {
    base: PathBuf,
    run_root: PathBuf,
    dolt: Dolt,
}

impl Env {
    fn new(name: &str) -> Env {
        let tag = std::process::id();
        let base = std::env::temp_dir().join(format!("af-pc-{name}-{tag}"));
        let run_root = std::env::temp_dir().join(format!("afc-{name}-{tag}"));
        for d in [&base, &run_root] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        let home = Home::prepare(&base.join("home")).unwrap();
        let dolt = Dolt::find(&home).unwrap_or_else(|e| panic!("these tests need dolt 2.6 or later on the PATH or in ARCHFORGE_DOLT: {e}"));
        Env { base, run_root, dolt }
    }

    fn opts(&self) -> ServerOptions {
        ServerOptions {
            run_root: Some(self.run_root.clone()),
            ready_budget: Duration::from_secs(20),
            ..ServerOptions::new(self.base.join("records"))
        }
    }

    /// A new project, its model prepared and saved, as the app leaves one.
    fn create(&self, name: &str, schema: &ModelSchema) -> Project {
        let p = Project::create(&self.dolt, &self.opts(), &self.base.join("Projects"), name, "Archforge test", &timestamp(1_761_670_800)).unwrap();
        p.prepare_model(schema).unwrap();
        p.commit("Saved").unwrap();
        p
    }

    fn open(&self, path: &Path, schema: &ModelSchema) -> Project {
        let p = Project::open(&self.dolt, &self.opts(), path).unwrap();
        p.prepare_model(schema).unwrap();
        p
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root).stderr(Stdio::null()).status();
        let _ = std::fs::remove_dir_all(&self.base);
        let _ = std::fs::remove_dir_all(&self.run_root);
    }
}

/// Ids in creation order, as the app makes them.
fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// The `n`th id of a test's own, apart from the model's.
fn id(n: u64) -> Uuid {
    let mut r = [0xc0u8; 10];
    r[2..].copy_from_slice(&n.to_be_bytes());
    Uuid::v7(1_762_400_000_000, r)
}

/// A time `n` seconds after a fixed start.
fn at(n: u64) -> String {
    timestamp(1_762_437_731 + n)
}

fn schema() -> ModelSchema {
    let universe = archforge_notations::store(ids(0)).unwrap().universe().clone();
    let (tables, junctions) = archforge_notations::storage();
    ModelSchema::new(&universe, tables, junctions).unwrap()
}

fn session() -> Session {
    archforge_notations::new_project_session("Commerce platform", ids(0)).unwrap()
}

fn run(s: &mut Session, cmd: &str, p: serde_json::Value, origin: Option<&str>) -> Uuid {
    let out = s.execute_from(cmd, &p, origin).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(serde_json::Value::as_str).and_then(Uuid::parse).unwrap_or(Uuid([0; 16]))
}

fn rows_of(s: &Session, schema: &ModelSchema) -> rows::Rows {
    rows::snapshot(s.doc(), schema.tables(), &schema.junctions()).unwrap()
}

fn sql(p: &Project) -> mysql::Conn {
    let mut c = p.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    c
}

fn count(p: &Project, query: &str) -> i64 {
    sql(p).query_first(query).unwrap().unwrap()
}

/// The chat tables with changes not committed.
fn uncommitted_chats(p: &Project) -> Vec<String> {
    let names: Vec<String> = sql(p).query("SELECT table_name FROM dolt_status").unwrap();
    names.into_iter().filter(|n| CHAT_TABLES.contains(&n.as_str())).collect()
}

fn format_at(p: &Project, revision: &str) -> String {
    sql(p).query_first(format!("SELECT `value` FROM settings AS OF '{revision}' WHERE `key` = 'format'")).unwrap().unwrap()
}

fn new_chat(n: u64, origin: &str, when: u64) -> NewChat {
    NewChat {
        id: id(n),
        origin: origin.into(),
        title: "Rename Customer\nto Client".into(),
        provider: Provider::Claude,
        model: "sonnet".into(),
        created_by: "Archforge test".into(),
        now: at(when),
    }
}

fn turn(n: u64, chat: Uuid, when: u64) -> NewTurn {
    NewTurn { id: id(n), chat, model: "claude-sonnet-4-5".into(), allow_changes: true, now: at(when) }
}

fn question(n: u64, body: &str) -> NewQuestion {
    NewQuestion { id: id(n), body: body.into(), anchors: Vec::new() }
}

/// A complete reply of the test's own, `body` at `when`.
fn reply(n: u64, turn: Uuid, body: &str, when: u64) -> NewReply {
    NewReply { id: id(n), turn, body: body.into(), complete: true, shortened: false, now: at(when) }
}

fn ended(turn: Uuid, when: u64, changes: Option<TurnChanges>) -> TurnEnd {
    TurnEnd { turn, outcome: TurnOutcome::Completed, error: None, now: at(when), changes }
}

fn anchor(element: Uuid, role: AnchorRole, name: &str) -> NewAnchor {
    NewAnchor { element, role, metaclass: "er_entity".into(), name: Some(name.into()) }
}

fn no_keys() -> MemoryStore {
    MemoryStore::new()
}

/// A conversation with one finished turn: a question, nothing more.
fn one_turn(p: &Project, chat: u64, origin: &str, when: u64) -> Uuid {
    p.create_chat(&new_chat(chat, origin, when), &no_keys()).unwrap();
    p.start_turn(&turn(chat + 1, id(chat), when), &question(chat + 2, "Hello"), &no_keys()).unwrap();
    p.end_turn(&ended(id(chat + 1), when, None)).unwrap();
    id(chat)
}

#[test]
fn a_conversation_is_made_answered_listed_newest_first_and_loaded_whole() {
    let env = Env::new("roundtrip");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let customer = id(900);
    let chat = id(1);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    let mut q = question(3, "Rename Customer to Client");
    q.anchors = vec![
        NewAnchor { element: id(901), role: AnchorRole::Project, metaclass: "core_model".into(), name: Some("Commerce platform".into()) },
        anchor(customer, AnchorRole::Selection, "Customer"),
        anchor(customer, AnchorRole::Selection, "Customer"),
    ];
    assert_eq!(p.start_turn(&turn(2, chat, 1), &q, &no_keys()).unwrap(), 1);
    // One turn runs at a time in a conversation.
    assert!(matches!(p.start_turn(&turn(20, chat, 1), &question(21, "And?"), &no_keys()), Err(Error::ChatValue { .. })));
    let call = NewToolCall {
        id: id(4),
        turn: id(2),
        tool: "er_entity_rename".into(),
        command: Some("er.entity.rename".into()),
        detail: Some("name".into()),
        status: ToolStatus::Done,
        changed_model: true,
        element_count: Some(1),
        arguments: Some(r#"{"entity":"…","name":"Client"}"#.into()),
        result: Some(r#"{"id":"…"}"#.into()),
        error: None,
        anchors: vec![anchor(customer, AnchorRole::Target, "Client")],
        now: at(2),
    };
    assert_eq!(p.append_tool_call(&call).unwrap(), 2);
    assert_eq!(p.append_reply(&reply(5, id(2), "Renamed **Customer** to Client.", 3)).unwrap(), 3);
    let changes = TurnChanges {
        changes: 1,
        elements: 1,
        changed: vec![ChangedElement { element: customer, kind: ChangeKind::Changed, metaclass: "er_entity".into(), name: Some("Client".into()) }],
    };
    p.end_turn(&ended(id(2), 4, Some(changes))).unwrap();
    // An ended turn takes nothing more.
    assert!(matches!(p.append_reply(&reply(6, id(2), "late", 5)), Err(Error::ChatValue { .. })));
    // A second conversation, newer; then the first is active again and comes first.
    p.create_chat(&new_chat(10, "Chat 2", 10), &no_keys()).unwrap();
    assert!(matches!(p.create_chat(&new_chat(11, "Chat 2", 11), &no_keys()), Err(Error::ChatValue { .. })), "an origin is unique");
    let order: Vec<Uuid> = p.list_chats().unwrap().iter().map(|l| l.id).collect();
    assert_eq!(order, [id(10), chat]);
    p.rename_chat(chat, "Renaming", &at(20), &no_keys()).unwrap();
    let listed = p.list_chats().unwrap();
    assert_eq!(listed.iter().map(|l| l.id).collect::<Vec<_>>(), [chat, id(10)]);
    let first = listed[0].entry.as_ref().unwrap();
    assert_eq!((first.chat.provider, first.messages, first.chat.title.as_str()), (Provider::Claude, 2, "Renaming"), "questions and replies");
    let second = listed[1].entry.as_ref().unwrap();
    assert_eq!((second.messages, second.chat.title.as_str()), (0, "Rename Customer to Client"), "the automatic title on one line");

    let c = p.load_chat(chat).unwrap();
    assert_eq!((c.chat.origin.as_str(), c.chat.model.as_str(), c.chat.created_by.as_str()), ("Chat 1", "sonnet", "Archforge test"));
    assert_eq!(c.turns.len(), 1);
    let t = &c.turns[0];
    assert_eq!((t.turn.number, t.turn.outcome, t.turn.allow_changes, t.turn.model.as_str()), (1, TurnOutcome::Completed, true, "claude-sonnet-4-5"));
    let change = t.change.as_ref().unwrap();
    assert_eq!((change.changes, change.elements, change.state, change.changed.len()), (1, 1, ChangeState::Applied, 1));
    let roles: Vec<Role> = c.messages.iter().map(|m| m.message.role).collect();
    assert_eq!(roles, [Role::User, Role::Tool, Role::Assistant]);
    assert_eq!(c.messages[0].message.body.as_deref(), Some("Rename Customer to Client"));
    assert_eq!(c.messages[0].anchors.len(), 2, "each element and role once");
    let tool = c.messages[1].tool.as_ref().unwrap();
    assert_eq!(
        (tool.tool.as_str(), tool.command.as_deref(), tool.status, tool.result_bytes),
        ("er_entity_rename", Some("er.entity.rename"), ToolStatus::Done, Some(12)),
        "the result's bytes"
    );
    assert_eq!(c.messages[1].anchors[0].role, AnchorRole::Target);
    assert_eq!(c.messages[2].message.position, 3);
    // Chats never make the project unsaved; they are in chat commits of their own.
    assert!(!p.has_unsaved().unwrap());
    let authors: Vec<String> = sql(&p).query("SELECT committer FROM dolt_log WHERE message LIKE 'Chat 1:%'").unwrap();
    assert!(authors.len() >= 2 && authors.iter().all(|a| a == "Archforge chat"), "{authors:?}");
    let messages: Vec<String> = sql(&p).query("SELECT message FROM dolt_log WHERE committer = 'Archforge chat' ORDER BY date").unwrap();
    assert!(messages.contains(&"Chat 1: turn 1".to_string()), "named by origin and turn, never by the user's words: {messages:?}");
    p.close().unwrap();
}

#[test]
fn a_saved_project_shows_its_conversations_again_and_a_turn_left_running_is_interrupted() {
    let env = Env::new("reopen");
    let schema = schema();
    let p = env.create("Library", &schema);
    let chat = one_turn(&p, 1, "Chat 1", 0);
    let before = p.load_chat(chat).unwrap();
    // A second turn is running when Archforge goes away: its question was written, not committed.
    p.start_turn(&turn(10, chat, 5), &question(11, "Add a Loan entity"), &no_keys()).unwrap();
    assert!(!uncommitted_chats(&p).is_empty());
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let again = env.open(&path, &schema);
    assert!(!again.has_unsaved().unwrap(), "uncommitted chat rows never make a project unsaved");
    again.open_chats(&at(60)).unwrap();
    assert!(uncommitted_chats(&again).is_empty(), "opening commits them");
    let after = again.load_chat(chat).unwrap();
    assert_eq!(after.turns[0], before.turns[0]);
    assert_eq!(after.messages[0], before.messages[0]);
    let left = &after.turns[1].turn;
    assert_eq!((left.outcome, left.ended_at.as_deref()), (TurnOutcome::Interrupted, Some(at(60).as_str())));
    assert_eq!(after.messages[1].message.body.as_deref(), Some("Add a Loan entity"), "a question sent is never lost");
    assert_eq!(again.list_chats().unwrap().len(), 1);
    again.close().unwrap();
}

#[test]
fn dont_save_keeps_the_conversations_and_puts_the_model_back() {
    let env = Env::new("discard");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let mut s = session();
    run(&mut s, "er.schema.add", json!({ "name": "shop" }), None);
    let saved = rows_of(&s, &schema);
    p.write_model(&schema, &saved).unwrap();
    p.commit("Saved").unwrap();
    let chat = one_turn(&p, 1, "Chat 1", 0);
    // The agent's model edit, written and not saved, and a turn still running with a reply.
    run(&mut s, "er.schema.add", json!({ "name": "crm" }), Some("Chat 1"));
    p.write_model(&schema, &rows_of(&s, &schema)).unwrap();
    p.start_turn(&turn(10, chat, 5), &question(11, "Add a schema"), &no_keys()).unwrap();
    p.append_reply(&reply(12, id(10), "Added crm.", 6)).unwrap();
    assert!(p.has_unsaved().unwrap());
    p.discard(&at(7)).unwrap();
    assert_eq!(p.read_model(&schema).unwrap(), saved, "the model is back at its save");
    assert!(!p.has_unsaved().unwrap());
    let c = p.load_chat(chat).unwrap();
    assert_eq!(c.turns.len(), 2, "every turn stays");
    assert_eq!(c.messages.last().unwrap().message.body.as_deref(), Some("Added crm."), "even what was written since the last chat commit");
    assert!(uncommitted_chats(&p).is_empty());
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let again = env.open(&path, &schema);
    assert_eq!(again.load_chat(chat).unwrap().messages.len(), 3);
    again.close().unwrap();
}

#[test]
fn a_save_commits_the_model_alone_and_the_project_opens_again_whole() {
    let env = Env::new("save");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let chat = one_turn(&p, 1, "Chat 1", 0);
    // A turn half-way: its rows are written, and a Save meanwhile commits only the model.
    p.start_turn(&turn(10, chat, 5), &question(11, "Add a schema"), &no_keys()).unwrap();
    let mut s = session();
    run(&mut s, "er.schema.add", json!({ "name": "shop" }), Some("Chat 1"));
    let model = rows_of(&s, &schema);
    p.write_model(&schema, &model).unwrap();
    assert!(p.commit("Saved").unwrap());
    assert!(!p.has_unsaved().unwrap());
    assert!(!uncommitted_chats(&p).is_empty(), "the running turn waits for its own chat commit");
    let saved_tables: Vec<String> = sql(&p).query("SELECT table_name FROM dolt_diff WHERE commit_hash = HASHOF('HEAD')").unwrap();
    assert!(!saved_tables.is_empty() && saved_tables.iter().all(|t| !CHAT_TABLES.contains(&t.as_str())), "{saved_tables:?}");
    p.end_turn(&ended(id(10), 6, None)).unwrap();
    assert!(uncommitted_chats(&p).is_empty());
    let path = p.path().to_path_buf();
    p.close().unwrap();
    // The fingerprint check passes, the model reads back, the conversation is whole.
    let again = env.open(&path, &schema);
    assert_eq!(again.read_model(&schema).unwrap(), model);
    assert!(!again.has_unsaved().unwrap());
    assert_eq!(again.load_chat(chat).unwrap().turns.len(), 2);
    assert_eq!(again.settings().format, FORMAT);
    again.close().unwrap();
}

/// Makes the project say format 2, committed, as an earlier Archforge left it.
fn as_format_2(p: &Project) {
    let mut c = sql(p);
    c.query_drop("UPDATE settings SET `value` = '2' WHERE `key` = 'format'").unwrap();
    c.query_drop("CALL DOLT_COMMIT('-am', 'format 2')").unwrap();
}

#[test]
fn a_format_2_project_becomes_format_3_with_its_first_conversation_and_never_with_a_pending_upgrade() {
    let env = Env::new("raise");
    let schema = schema();
    let p = env.create("Earlier", &schema);
    assert_eq!(format_at(&p, "HEAD"), FORMAT.to_string(), "a new project is format 3");
    as_format_2(&p);
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let p = env.open(&path, &schema);
    assert_eq!(p.settings().format, 2);
    // Its first conversation raises it, in the chat commit.
    one_turn(&p, 1, "Chat 1", 0);
    assert_eq!(format_at(&p, "HEAD"), "3");
    assert!(!p.has_unsaved().unwrap());
    p.close().unwrap();

    // With another change of the settings waiting (a model upgrade on opening), the format waits
    // for the next Save: the settings never go into a chat commit beside someone else's change.
    let q = env.create("Upgraded", &schema);
    as_format_2(&q);
    sql(&q).query_drop("INSERT INTO settings (`key`, `value`) VALUES ('pending', 'upgrade')").unwrap();
    one_turn(&q, 1, "Chat 1", 0);
    assert_eq!(format_at(&q, "HEAD"), "2");
    assert_eq!(format_at(&q, "WORKING"), "3");
    assert!(uncommitted_chats(&q).is_empty(), "the conversation is committed all the same");
    // Don't save puts the settings back, and the format is raised again at once.
    q.discard(&at(9)).unwrap();
    assert_eq!(format_at(&q, "HEAD"), "3");
    assert_eq!(count(&q, "SELECT COUNT(*) FROM settings WHERE `key` = 'pending'"), 0, "the other change is gone");
    assert!(!q.has_unsaved().unwrap());
    let path = q.path().to_path_buf();
    q.close().unwrap();
    assert_eq!(env.open(&path, &schema).settings().format, 3);
}

#[test]
fn a_format_2_project_without_conversations_stays_format_2() {
    let env = Env::new("stay");
    let schema = schema();
    let p = env.create("Earlier", &schema);
    as_format_2(&p);
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let p = env.open(&path, &schema);
    p.open_chats(&at(0)).unwrap();
    let mut s = session();
    run(&mut s, "er.schema.add", json!({ "name": "shop" }), None);
    p.write_model(&schema, &rows_of(&s, &schema)).unwrap();
    assert!(p.commit("Saved").unwrap());
    assert!(!p.commit_chats().unwrap(), "nothing of the conversations to commit");
    assert!(p.list_chats().unwrap().is_empty());
    p.discard(&at(1)).unwrap();
    p.close().unwrap();
    let again = env.open(&path, &schema);
    assert_eq!(again.settings().format, 2, "so an Archforge that reads up to format 2 still opens it");
    again.close().unwrap();
}

#[test]
fn a_project_of_a_newer_format_is_refused_as_before() {
    let env = Env::new("newer");
    let schema = schema();
    let p = env.create("Later", &schema);
    let mut c = sql(&p);
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'format'", ((FORMAT + 1).to_string(),)).unwrap();
    c.query_drop("CALL DOLT_COMMIT('-am', 'format 4')").unwrap();
    drop(c);
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let refused = Project::open(&env.dolt, &env.opts(), &path);
    assert!(matches!(refused, Err(Error::NewerFormat { found, supported: FORMAT }) if found == FORMAT + 1));
}

#[test]
fn deleting_a_conversation_removes_its_rows_and_leaves_the_others() {
    let env = Env::new("delete");
    let schema = schema();
    let p = env.create("Library", &schema);
    let gone = id(1);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    let mut q = question(3, "Rename Customer");
    q.anchors = vec![anchor(id(900), AnchorRole::Selection, "Customer")];
    p.start_turn(&turn(2, gone, 1), &q, &no_keys()).unwrap();
    let call = NewToolCall {
        id: id(4),
        turn: id(2),
        tool: "er_entity_get".into(),
        command: None,
        detail: None,
        status: ToolStatus::Done,
        changed_model: false,
        element_count: Some(1),
        arguments: None,
        result: None,
        error: None,
        anchors: vec![anchor(id(900), AnchorRole::Read, "Customer")],
        now: at(2),
    };
    p.append_tool_call(&call).unwrap();
    let changed = vec![ChangedElement { element: id(900), kind: ChangeKind::Changed, metaclass: "er_entity".into(), name: None }];
    p.end_turn(&ended(id(2), 3, Some(TurnChanges { changes: 1, elements: 1, changed }))).unwrap();
    let kept = one_turn(&p, 10, "Chat 2", 5);
    p.delete_chat(gone).unwrap();
    for table in CHAT_TABLES {
        let rows = count(&p, &format!("SELECT COUNT(*) FROM `{table}`"));
        let left = match table {
            "chat" | "chat_turn" | "chat_message" => 1,
            _ => 0,
        };
        assert_eq!(rows, left, "{table} holds only the other conversation's rows");
    }
    assert_eq!(p.list_chats().unwrap().iter().map(|l| l.id).collect::<Vec<_>>(), [kept]);
    assert!(matches!(p.load_chat(gone), Err(Error::ChatMissing { .. })));
    assert!(matches!(p.delete_chat(gone), Err(Error::ChatMissing { .. })));
    assert!(uncommitted_chats(&p).is_empty(), "the deletion is committed");
    // It stays in the project's earlier commits.
    assert_eq!(count(&p, "SELECT COUNT(*) FROM `chat` AS OF 'HEAD~1'"), 2);
    p.close().unwrap();
}

/// A conversation's rows in each chat table, as the project holds them now or `at` a commit, in a
/// fixed order: every column of every row.
fn rows_of_chat(p: &Project, chat: Uuid, at: Option<&str>) -> Vec<(&'static str, Vec<Vec<mysql::Value>>)> {
    let of = |table: &str| at.map_or_else(|| format!("`{table}`"), |h| format!("`{table}` AS OF '{h}'"));
    CHAT_TABLES
        .iter()
        .map(|table| {
            let rows = match *table {
                "chat" => "`id` = ?".to_string(),
                "chat_turn" | "chat_message" => "`chat` = ?".to_string(),
                "chat_tool_call" | "chat_anchor" => format!("`message` IN (SELECT `id` FROM {} WHERE `chat` = ?)", of("chat_message")),
                _ => format!("`turn` IN (SELECT `id` FROM {} WHERE `chat` = ?)", of("chat_turn")),
            };
            let found: Vec<mysql::Row> = sql(p).exec(format!("SELECT * FROM {} WHERE {rows} ORDER BY 1, 2", of(table)), (chat.0.to_vec(),)).unwrap();
            (*table, found.into_iter().map(mysql::Row::unwrap).collect())
        })
        .collect()
}

/// A conversation with a row in every chat table: a question with a chip, a tool row with its
/// anchor, a reply, and a turn's change with its element.
fn whole_chat(p: &Project, n: u64, origin: &str, when: u64) -> Uuid {
    let chat = id(n);
    p.create_chat(&new_chat(n, origin, when), &no_keys()).unwrap();
    let mut q = question(n + 2, "Rename Customer");
    q.anchors = vec![anchor(id(900), AnchorRole::Selection, "Customer")];
    p.start_turn(&turn(n + 1, chat, when + 1), &q, &no_keys()).unwrap();
    let call = NewToolCall {
        id: id(n + 3),
        turn: id(n + 1),
        tool: "engine.execute".into(),
        command: Some("er.entity.rename".into()),
        detail: Some("name".into()),
        status: ToolStatus::Done,
        changed_model: true,
        element_count: Some(1),
        arguments: Some(r#"{"name":"Client"}"#.into()),
        result: Some(r#"{"id":"x"}"#.into()),
        error: None,
        anchors: vec![anchor(id(900), AnchorRole::Target, "Client")],
        now: at(when + 2),
    };
    p.append_tool_call(&call).unwrap();
    p.append_reply(&reply(n + 4, id(n + 1), "Renamed.", when + 3)).unwrap();
    let changed = vec![ChangedElement { element: id(900), kind: ChangeKind::Changed, metaclass: "er_entity".into(), name: Some("Client".into()) }];
    p.end_turn(&ended(id(n + 1), when + 4, Some(TurnChanges { changes: 1, elements: 1, changed }))).unwrap();
    chat
}

/// The commits on the project's branch, newest first: hash, message and committer.
fn commits(p: &Project) -> Vec<(String, String, String)> {
    sql(p).query("SELECT commit_hash, message, committer FROM dolt_log ORDER BY commit_order DESC").unwrap()
}

#[test]
fn a_deletion_is_one_chat_only_commit_the_rows_stay_in_history_and_undo_brings_every_row_back() {
    let env = Env::new("delete-restore");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let gone = whole_chat(&p, 1, "Chat 1", 0);
    let kept = whole_chat(&p, 20, "Chat 2", 10);
    // The person's own model change, written and not saved.
    let mut s = session();
    run(&mut s, "er.schema.add", json!({ "name": "crm" }), None);
    let model = rows_of(&s, &schema);
    p.write_model(&schema, &model).unwrap();
    assert!(p.has_unsaved().unwrap());
    let whole = rows_of_chat(&p, gone, None);
    assert!(whole.iter().all(|(_, rows)| !rows.is_empty()), "a row in every chat table: {whole:?}");
    let before = commits(&p);

    let deleted = p.delete_chat(gone).unwrap();
    let after = commits(&p);
    assert_eq!(after.len(), before.len() + 1, "exactly one commit");
    assert_eq!((after[0].1.as_str(), after[0].2.as_str()), ("Chat 1: deleted", "Archforge chat"));
    assert_eq!((deleted.origin.as_str(), deleted.before.as_str()), ("Chat 1", before[0].0.as_str()), "the commit just before");
    let committed: Vec<String> = sql(&p).query("SELECT table_name FROM dolt_diff WHERE commit_hash = HASHOF('HEAD')").unwrap();
    assert!(!committed.is_empty() && committed.iter().all(|t| CHAT_TABLES.contains(&t.as_str())), "chat tables only: {committed:?}");
    assert!(p.has_unsaved().unwrap(), "the model's change stays unsaved");
    assert_eq!(p.read_model(&schema).unwrap(), model, "and untouched");
    assert!(uncommitted_chats(&p).is_empty());
    assert!(rows_of_chat(&p, gone, None).iter().all(|(_, rows)| rows.is_empty()), "every row is gone");
    assert_eq!(rows_of_chat(&p, gone, Some(&deleted.before)), whole, "and every row is in the saved history");
    assert_eq!(p.list_chats().unwrap().iter().map(|l| l.id).collect::<Vec<_>>(), [kept]);

    assert_eq!(p.restore_chat(gone, &deleted.before).unwrap(), "Chat 1");
    let restored = commits(&p);
    assert_eq!(restored.len(), after.len() + 1, "one more commit");
    assert_eq!((restored[0].1.as_str(), restored[0].2.as_str()), ("Chat 1: restored", "Archforge chat"));
    let committed: Vec<String> = sql(&p).query("SELECT table_name FROM dolt_diff WHERE commit_hash = HASHOF('HEAD')").unwrap();
    assert!(committed.iter().all(|t| CHAT_TABLES.contains(&t.as_str())), "chat tables only: {committed:?}");
    assert_eq!(rows_of_chat(&p, gone, None), whole, "every row of every chat table, as it was");
    assert!(uncommitted_chats(&p).is_empty());
    assert!(p.has_unsaved().unwrap());
    assert_eq!(p.list_chats().unwrap().iter().map(|l| l.id).collect::<Vec<_>>(), [kept, gone], "in its place by its last activity");
    assert_eq!(p.load_chat(gone).unwrap().messages.len(), 3);
    p.close().unwrap();
}

#[test]
fn what_waits_is_kept_first_so_the_commit_before_a_deletion_holds_the_conversation_whole() {
    let env = Env::new("delete-pending");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let gone = one_turn(&p, 1, "Chat 1", 0);
    // A turn half-way: its question is written and waits for the turn's own chat commit.
    p.start_turn(&turn(10, gone, 5), &question(11, "And then?"), &no_keys()).unwrap();
    assert!(!uncommitted_chats(&p).is_empty());
    let before = commits(&p).len();
    let deleted = p.delete_chat(gone).unwrap();
    let after = commits(&p);
    assert_eq!(after.len(), before + 2);
    assert_eq!((after[1].1.as_str(), after[0].1.as_str()), ("Conversations kept", "Chat 1: deleted"));
    assert_eq!(deleted.before, after[1].0);
    let held = rows_of_chat(&p, gone, Some(&deleted.before));
    assert_eq!(held.iter().find(|(t, _)| *t == "chat_message").map(|(_, r)| r.len()), Some(2), "both questions");
    p.restore_chat(gone, &deleted.before).unwrap();
    assert_eq!(p.load_chat(gone).unwrap().turns.len(), 2);
    p.close().unwrap();
}

#[test]
fn bringing_a_conversation_back_is_refused_when_listed_forged_its_label_taken_or_the_commit_lacks_it() {
    let env = Env::new("restore-refused");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let first = commits(&p)[0].0.clone();
    let gone = whole_chat(&p, 1, "Chat 1", 0);
    let deleted = p.delete_chat(gone).unwrap();
    let nothing_written = |p: &Project, n: usize| {
        assert_eq!(commits(p).len(), n, "no commit");
        assert!(uncommitted_chats(p).is_empty(), "no row");
    };
    let n = commits(&p).len();
    let refused = |r: Result<String, Error>| matches!(r, Err(Error::ChatNotRestored { .. }));
    // Not a commit at all, the shape of one but not this project's, and one before it was made.
    for forged in ["HEAD", "x' OR '1'='1", "", "0123456789abcdefghijklmnopqrstuv"] {
        assert!(refused(p.restore_chat(gone, forged)), "{forged:?}");
        nothing_written(&p, n);
    }
    assert!(refused(p.restore_chat(gone, &first)), "the first commit does not hold it");
    nothing_written(&p, n);
    // Its label taken since by another conversation.
    p.create_chat(&new_chat(30, "Chat 1", 30), &no_keys()).unwrap();
    let n = commits(&p).len();
    let taken = p.restore_chat(gone, &deleted.before);
    assert!(matches!(&taken, Err(Error::ChatNotRestored { why, .. }) if why.contains("Chat 1")), "{taken:?}");
    nothing_written(&p, n);
    assert!(rows_of_chat(&p, gone, None).iter().all(|(_, rows)| rows.is_empty()));
    // Back once it is free; then a second time is refused, it is listed already.
    p.delete_chat(id(30)).unwrap();
    p.restore_chat(gone, &deleted.before).unwrap();
    let n = commits(&p).len();
    assert!(refused(p.restore_chat(gone, &deleted.before)), "listed already");
    nothing_written(&p, n);
    // A conversation not there is not deleted, and nothing is committed for it.
    assert!(matches!(p.delete_chat(id(99)), Err(Error::ChatMissing { .. })));
    nothing_written(&p, n);
    p.close().unwrap();
}

#[test]
fn change_links_follow_undo_redo_and_dont_save() {
    let env = Env::new("links");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let mut s = session();
    let chat = id(1);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    // A turn whose edits carry the conversation's origin (AR-198).
    p.start_turn(&turn(2, chat, 1), &question(3, "Add a shop schema with an entity"), &no_keys()).unwrap();
    let before = s.revision();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }), Some("Chat 1"));
    let order = run(&mut s, "er.entity.add", json!({ "schema": shop.to_string(), "name": "Order" }), Some("Chat 1"));
    let first = TurnSpan { before, after: s.revision() };
    assert_eq!(s.undo_origin(), Some("Chat 1"));
    let changed = vec![
        ChangedElement { element: shop, kind: ChangeKind::Added, metaclass: "er_schema".into(), name: Some("shop".into()) },
        ChangedElement { element: order, kind: ChangeKind::Added, metaclass: "er_entity".into(), name: Some("Order".into()) },
    ];
    p.end_turn(&ended(id(2), 2, Some(TurnChanges { changes: 2, elements: 2, changed }))).unwrap();
    p.write_model(&schema, &rows_of(&s, &schema)).unwrap();
    p.commit("Saved").unwrap();
    let state =
        |p: &Project, turn: Uuid| p.load_chat(chat).unwrap().turns.into_iter().find(|t| t.turn.id == turn).and_then(|t| t.change).map(|c| c.state);
    // ⌘Z back past the turn's first step: undone. Part way back is not.
    s.undo().unwrap();
    assert_eq!(first.moved(ChangeState::Applied, s.revision()), None);
    s.undo().unwrap();
    let moved = first.moved(ChangeState::Applied, s.revision()).unwrap();
    p.set_change_states(&[(id(2), moved)], &at(3)).unwrap();
    assert_eq!(state(&p, id(2)), Some(ChangeState::Undone));
    // Redo brings it back.
    s.redo().unwrap();
    s.redo().unwrap();
    p.set_change_states(&[(id(2), first.moved(ChangeState::Undone, s.revision()).unwrap())], &at(4)).unwrap();
    assert_eq!(state(&p, id(2)), Some(ChangeState::Applied));
    // Undone again, unsaved; and a second turn after the save.
    p.set_change_states(&[(id(2), ChangeState::Undone)], &at(5)).unwrap();
    p.start_turn(&turn(10, chat, 6), &question(11, "Add Customer"), &no_keys()).unwrap();
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop.to_string(), "name": "Customer" }), Some("Chat 1"));
    let changed = vec![ChangedElement { element: customer, kind: ChangeKind::Added, metaclass: "er_entity".into(), name: Some("Customer".into()) }];
    p.end_turn(&ended(id(10), 7, Some(TurnChanges { changes: 1, elements: 1, changed }))).unwrap();
    assert!(matches!(p.set_change_states(&[(id(99), ChangeState::Undone)], &at(8)), Err(Error::ChatMissing { .. })), "no change link");
    // Don't save: each goes back to its state at the save; the later turn's changes are discarded.
    p.discard(&at(9)).unwrap();
    assert_eq!(state(&p, id(2)), Some(ChangeState::Applied));
    assert_eq!(state(&p, id(10)), Some(ChangeState::Discarded));
    let change = p.load_chat(chat).unwrap().turns[1].change.clone().unwrap();
    assert_eq!((change.changed.len(), change.updated_at.as_str()), (1, at(9).as_str()), "what the turn did stays; only the state moves");
    assert!(uncommitted_chats(&p).is_empty());
    p.close().unwrap();
}

/// Writes rows straight into a chat table, `n` at a time, from `make(i)` giving one row's values.
fn bulk(p: &Project, table: &str, columns: &str, range: std::ops::Range<u64>, make: impl Fn(u64) -> String) {
    let mut c = sql(p);
    let all: Vec<u64> = range.collect();
    for chunk in all.chunks(500) {
        let values: Vec<String> = chunk.iter().map(|i| make(*i)).collect();
        c.query_drop(format!("INSERT INTO `{table}` ({columns}) VALUES {}", values.join(", "))).unwrap();
    }
}

fn hex(u: Uuid) -> String {
    u.0.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn the_limits_refuse_what_a_person_asked_for_and_cut_what_an_agent_produced() {
    let env = Env::new("limits");
    let schema = schema();
    let p = env.create("Limits", &schema);
    let chat = id(1);
    // An automatic title is cut; a rename over the limit is refused.
    p.create_chat(&NewChat { title: "T".repeat(300), ..new_chat(1, "Chat 1", 0) }, &no_keys()).unwrap();
    assert_eq!(p.load_chat(chat).unwrap().chat.title.chars().count(), limits::TITLE_CHARS);
    assert!(matches!(p.rename_chat(chat, &"T".repeat(limits::TITLE_CHARS + 1), &at(1), &no_keys()), Err(Error::TooLarge { .. })));
    assert!(matches!(p.rename_chat(chat, "", &at(1), &no_keys()), Err(Error::ChatValue { .. })));
    // A question one byte over is refused, with nothing written.
    let long = "q".repeat(limits::QUESTION_BYTES + 1);
    assert!(
        matches!(p.start_turn(&turn(2, chat, 1), &question(3, &long), &no_keys()), Err(Error::TooLarge { size, .. }) if size == limits::QUESTION_BYTES + 1)
    );
    assert_eq!(count(&p, "SELECT COUNT(*) FROM chat_turn"), 0);
    let mut q = question(3, &"q".repeat(limits::QUESTION_BYTES));
    q.anchors = (0..600).map(|n| anchor(id(10_000 + n), AnchorRole::Selection, &"N".repeat(300))).collect();
    p.start_turn(&turn(2, chat, 1), &q, &no_keys()).unwrap();
    // An agent's message over its limit is stored cut at a character boundary, and marked.
    let body = "顧".repeat(limits::REPLY_BYTES / 3 + 10);
    p.append_reply(&reply(4, id(2), &body, 2)).unwrap();
    let call = NewToolCall {
        id: id(5),
        turn: id(2),
        tool: "batch".into(),
        command: None,
        detail: Some("d".repeat(100)),
        status: ToolStatus::Failed,
        changed_model: false,
        element_count: Some(600),
        arguments: Some("a".repeat(9 << 10)),
        result: Some("r".repeat(3 << 10)),
        error: Some("e".repeat(2 << 10)),
        anchors: (0..600).map(|n| anchor(id(20_000 + n), AnchorRole::Read, "x")).collect(),
        now: at(3),
    };
    p.append_tool_call(&call).unwrap();
    let changed: Vec<ChangedElement> =
        (0..2_100).map(|n| ChangedElement { element: id(30_000 + n), kind: ChangeKind::Added, metaclass: "er_entity".into(), name: None }).collect();
    p.end_turn(&ended(id(2), 4, Some(TurnChanges { changes: 2_100, elements: 2_200, changed }))).unwrap();
    let c = p.load_chat(chat).unwrap();
    let asked = &c.messages[0];
    assert_eq!((asked.anchors.len(), asked.message.anchors_cut), (limits::ANCHORS_PER_MESSAGE, true));
    assert!(asked.anchors.iter().all(|a| a.name.as_ref().unwrap().chars().count() == limits::NAME_CHARS), "name snapshots are cut");
    let answer = &c.messages[1].message;
    let stored = answer.body.as_ref().unwrap();
    assert!(answer.shortened && stored.len() <= limits::REPLY_BYTES && stored.len() > limits::REPLY_BYTES - 3);
    assert!(body.starts_with(stored.as_str()));
    let tool_row = &c.messages[2];
    let tool = tool_row.tool.as_ref().unwrap();
    assert!(tool.shortened);
    assert_eq!(tool.arguments.as_ref().unwrap().len(), limits::ARGUMENTS_BYTES);
    assert_eq!((tool.result.as_ref().unwrap().len(), tool.result_bytes), (limits::RESULT_BYTES, Some(3 << 10)), "the true size is kept");
    assert_eq!(tool.error.as_ref().unwrap().len(), limits::ERROR_BYTES);
    assert_eq!(tool.detail.as_ref().unwrap().len(), limits::DETAIL_CHARS);
    assert_eq!((tool_row.anchors.len(), tool_row.message.anchors_cut, tool.element_count), (limits::ANCHORS_PER_MESSAGE, true, Some(600)));
    let change = c.turns[0].change.as_ref().unwrap();
    assert_eq!((change.changed.len(), change.elements, change.changes), (limits::CHANGED_ELEMENTS_PER_TURN, 2_200, 2_100), "the counts stay exact");

    // A conversation holds 5,000 messages at most; a project 10,000 conversations.
    p.start_turn(&turn(6, chat, 5), &question(7, "More"), &no_keys()).unwrap();
    let (turn_hex, chat_hex) = (hex(id(6)), hex(chat));
    bulk(
        &p,
        "chat_message",
        "`id`, `chat`, `turn`, `position`, `role`, `body`, `complete`, `shortened`, `anchors_cut`, `created_at`",
        4..5_000,
        |i| {
            format!(
                "(UNHEX('{}'), UNHEX('{chat_hex}'), UNHEX('{turn_hex}'), {}, 'assistant', 'x', 1, 0, 0, '{}')",
                hex(id(100_000 + i)),
                i + 1,
                at(6)
            )
        },
    );
    assert_eq!(count(&p, "SELECT COUNT(*) FROM chat_message"), limits::MESSAGES_PER_CHAT as i64);
    assert!(matches!(p.append_reply(&reply(8, id(6), "one more", 7)), Err(Error::Full { max: limits::MESSAGES_PER_CHAT, .. })));
    bulk(
        &p,
        "chat",
        "`id`, `origin`, `title`, `provider`, `model`, `created_by`, `created_at`, `updated_at`",
        1..limits::CHATS_PER_PROJECT as u64,
        |i| format!("(UNHEX('{}'), 'Bulk {i}', 'Bulk', 'codex', 'gpt', '', '{}', '{}')", hex(id(200_000 + i)), at(8), at(8)),
    );
    assert!(matches!(p.create_chat(&new_chat(9, "Chat 3", 9), &no_keys()), Err(Error::Full { max: limits::CHATS_PER_PROJECT, .. })));
    assert_eq!(p.list_chats().unwrap().len(), limits::CHATS_PER_PROJECT);
    p.close().unwrap();
}

#[test]
fn a_question_holding_a_kept_api_key_is_refused_and_never_stored() {
    const CANARY: &str = "sk-ant-canary-0b6d3e9f27a4c1";
    let env = Env::new("canary");
    let schema = schema();
    let p = env.create("Secrets", &schema);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    let keys = MemoryStore::new();
    keys.set(&SecretName::ANTHROPIC_API_KEY, &Secret::new(CANARY.to_string()).unwrap()).unwrap();
    let pasted = format!("My key {CANARY} does not work, why?");
    // The title is made from the first question: it is checked as the question is.
    let titled = NewChat { title: pasted.clone(), ..new_chat(5, "Chat 2", 0) };
    assert!(matches!(p.create_chat(&titled, &keys), Err(Error::HeldKey { .. })));
    assert!(matches!(p.rename_chat(id(1), &pasted, &at(1), &keys), Err(Error::HeldKey { .. })));
    assert_eq!(count(&p, &format!("SELECT COUNT(*) FROM chat WHERE title LIKE '%{CANARY}%'")), 0);
    let refused = p.start_turn(&turn(2, id(1), 1), &question(3, &pasted), &keys).unwrap_err();
    assert!(matches!(&refused, Error::HeldKey { name } if name == "anthropic-api-key"));
    assert!(!format!("{refused} {refused:?}").contains(CANARY), "the refusal never shows the key");
    assert_eq!(count(&p, "SELECT COUNT(*) FROM chat_turn") + count(&p, "SELECT COUNT(*) FROM chat_message"), 0, "nothing is written");
    // A question without it goes through.
    p.start_turn(&turn(2, id(1), 1), &question(3, "Does my key work?"), &keys).unwrap();
    assert_eq!(count(&p, &format!("SELECT COUNT(*) FROM chat_message WHERE body LIKE '%{CANARY}%'")), 0);
    let history: i64 = count(&p, &format!("SELECT COUNT(*) FROM dolt_log WHERE message LIKE '%{CANARY}%'"));
    let committed: i64 = count(&p, &format!("SELECT COUNT(*) FROM chat AS OF 'HEAD' WHERE title LIKE '%{CANARY}%'"));
    assert_eq!((history, committed), (0, 0), "no commit holds it");
    p.close().unwrap();
}

/// Send again runs a failed turn in place, its question kept once; the refusals write nothing.
#[test]
fn a_failed_turn_runs_again_in_place_with_its_question_stored_once() {
    let env = Env::new("rerun");
    let schema = schema();
    let p = env.create("Rerun", &schema);
    let chat = id(1);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    let mut q = question(3, "Which classes have no description yet?");
    q.anchors = vec![anchor(id(900), AnchorRole::Selection, "Order")];
    p.start_turn(&turn(2, chat, 1), &q, &no_keys()).unwrap();
    let failed = |when| TurnEnd { turn: id(2), outcome: TurnOutcome::Failed, error: Some(TurnError::Network), now: at(when), changes: None };
    p.end_turn(&failed(2)).unwrap();
    let rerun = |question: Option<(&str, Vec<NewAnchor>)>, when| Rerun {
        turn: id(2),
        model: "opus".into(),
        allow_changes: false,
        question: question.map(|(b, a)| (b.to_string(), a)),
        now: at(when),
    };
    // Sent again as it was: the same row runs again, nothing new is stored.
    p.rerun_turn(&rerun(None, 3), &no_keys()).unwrap();
    p.append_reply(&reply(4, id(2), "Five of them.", 4)).unwrap();
    p.end_turn(&failed(5)).unwrap();
    // Edited and sent again: the stored question and its chips are replaced, not added to.
    let edited = vec![anchor(id(901), AnchorRole::Diagram, "Commerce")];
    p.rerun_turn(&rerun(Some(("Which classes lack a description?", edited)), 6), &no_keys()).unwrap();
    p.end_turn(&ended(id(2), 7, None)).unwrap();
    let c = p.load_chat(chat).unwrap();
    assert_eq!(c.turns.len(), 1, "one stored row per turn");
    let t = &c.turns[0].turn;
    assert_eq!((t.number, t.outcome, t.error, t.model.as_str(), t.allow_changes), (1, TurnOutcome::Completed, None, "opus", false));
    assert_eq!((t.started_at.as_str(), t.ended_at.as_deref()), (at(6).as_str(), Some(at(7).as_str())));
    let questions: Vec<_> = c.messages.iter().filter(|m| m.message.role == Role::User).collect();
    assert_eq!(questions.len(), 1, "the question is stored once");
    assert_eq!(questions[0].message.body.as_deref(), Some("Which classes lack a description?"));
    assert_eq!(questions[0].anchors.iter().map(|a| (a.element, a.role)).collect::<Vec<_>>(), [(id(901), AnchorRole::Diagram)]);
    assert_eq!(c.messages.iter().filter(|m| m.message.role == Role::Assistant).count(), 1, "the failed attempt's reply stays as it was");
    assert!(uncommitted_chats(&p).is_empty(), "the turn's end committed it");

    // Refused, with nothing written: a turn that did not fail, one not the last, one that changed
    // the model, and an edited question holding a kept key.
    let refused = |r: &Rerun, keys: &MemoryStore| p.rerun_turn(r, keys).unwrap_err();
    assert!(matches!(refused(&rerun(None, 8), &no_keys()), Error::ChatValue { .. }), "a completed turn");
    p.start_turn(&turn(10, chat, 9), &question(11, "And now?"), &no_keys()).unwrap();
    let changes = TurnChanges {
        changes: 1,
        elements: 1,
        changed: vec![ChangedElement { element: id(900), kind: ChangeKind::Changed, metaclass: "uml_class".into(), name: Some("Order".into()) }],
    };
    p.end_turn(&TurnEnd { turn: id(10), outcome: TurnOutcome::Failed, error: Some(TurnError::Other), now: at(10), changes: Some(changes) }).unwrap();
    let again = Rerun { turn: id(10), ..rerun(None, 11) };
    assert!(matches!(refused(&again, &no_keys()), Error::ChatValue { .. }), "a turn that changed the model");
    let first = TurnEnd { turn: id(20), outcome: TurnOutcome::Failed, error: Some(TurnError::Network), now: at(13), changes: None };
    let other = id(30);
    p.create_chat(&NewChat { id: other, ..new_chat(30, "Chat 2", 12) }, &no_keys()).unwrap();
    p.start_turn(&turn(20, other, 12), &question(21, "First"), &no_keys()).unwrap();
    p.end_turn(&first).unwrap();
    p.start_turn(&turn(22, other, 14), &question(23, "Second"), &no_keys()).unwrap();
    p.end_turn(&ended(id(22), 15, None)).unwrap();
    assert!(matches!(refused(&Rerun { turn: id(20), ..rerun(None, 16) }, &no_keys()), Error::ChatValue { .. }), "a turn not the last");
    p.start_turn(&turn(24, other, 17), &question(25, "Third"), &no_keys()).unwrap();
    p.end_turn(&TurnEnd { turn: id(24), ..first.clone() }).unwrap();
    const CANARY: &str = "sk-ant-api03-canary-rerun-0000-4F2a";
    let keys = MemoryStore::new();
    keys.set(&SecretName::ANTHROPIC_API_KEY, &Secret::new(CANARY.to_string()).unwrap()).unwrap();
    let pasted = Rerun { turn: id(24), ..rerun(Some((&format!("Try {CANARY}"), Vec::new())), 18) };
    assert!(matches!(refused(&pasted, &keys), Error::HeldKey { .. }));
    let stored = p.load_chat(other).unwrap();
    assert_eq!(stored.turns.last().map(|t| t.turn.outcome), Some(TurnOutcome::Failed), "nothing was written");
    assert_eq!(count(&p, &format!("SELECT COUNT(*) FROM chat_message WHERE body LIKE '%{CANARY}%'")), 0);
}

#[test]
fn an_anchor_to_a_deleted_element_is_kept_with_its_snapshot_and_lives_again_after_undo() {
    let env = Env::new("anchor");
    let schema = schema();
    let p = env.create("Commerce platform", &schema);
    let mut s = session();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }), None);
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop.to_string(), "name": "Customer" }), None);
    p.write_model(&schema, &rows_of(&s, &schema)).unwrap();
    p.commit("Saved").unwrap();
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    let mut q = question(3, "Describe Customer");
    q.anchors = vec![anchor(customer, AnchorRole::Selection, "Customer")];
    p.start_turn(&turn(2, id(1), 1), &q, &no_keys()).unwrap();
    p.end_turn(&ended(id(2), 2, None)).unwrap();
    // The element is deleted and the deletion saved: the anchor stays, with its snapshot.
    run(&mut s, "er.entity.remove", json!({ "entity": customer.to_string() }), None);
    assert!(s.doc().by_uuid(customer).is_none());
    p.write_model(&schema, &rows_of(&s, &schema)).unwrap();
    p.commit("Saved").unwrap();
    let a = &p.load_chat(id(1)).unwrap().messages[0].anchors[0];
    assert_eq!((a.element, a.name.as_deref(), a.metaclass.as_str(), a.role), (customer, Some("Customer"), "er_entity", AnchorRole::Selection));
    assert!(uncommitted_chats(&p).is_empty(), "deleting an element writes nothing to the conversations");
    // Undo brings it back under the same UUID, so the anchor is live again.
    s.undo().unwrap();
    assert!(s.doc().by_uuid(customer).is_some());
    p.close().unwrap();
}

#[test]
fn damaged_conversations_are_refused_with_a_reason_and_never_block_the_model() {
    let env = Env::new("damaged");
    let schema = schema();
    let p = env.create("Damaged", &schema);
    let mut s = session();
    run(&mut s, "er.schema.add", json!({ "name": "shop" }), None);
    let model = rows_of(&s, &schema);
    p.write_model(&schema, &model).unwrap();
    p.commit("Saved").unwrap();
    let chat = one_turn(&p, 1, "Chat 1", 0);
    one_turn(&p, 10, "Chat 2", 1);
    // A wrong code makes its conversation's entry unreadable, the others listed.
    sql(&p).exec_drop("UPDATE chat SET provider = 'gemini' WHERE id = ?", (chat.0.to_vec(),)).unwrap();
    let listed = p.list_chats().unwrap();
    let bad = listed.iter().find(|l| l.id == chat).unwrap();
    assert!(bad.entry.as_ref().is_err_and(|why| why.contains("gemini") && why.contains("claude, codex")), "{:?}", bad.entry);
    assert!(listed.iter().any(|l| l.id == id(10) && l.entry.is_ok()));
    assert!(matches!(p.load_chat(chat), Err(Error::ChatUnreadable { .. })));
    sql(&p).exec_drop("UPDATE chat SET provider = 'claude' WHERE id = ?", (chat.0.to_vec(),)).unwrap();
    // An orphan message: its turn is missing.
    sql(&p).exec_drop("UPDATE chat_message SET turn = ? WHERE chat = ?", (id(77).0.to_vec(), chat.0.to_vec())).unwrap();
    assert!(matches!(p.load_chat(chat), Err(Error::ChatUnreadable { why, .. }) if why.contains("no turn")));
    // A table without its key, from elsewhere: two change links for one turn are refused, not one
    // of them taken.
    let mut c = sql(&p);
    c.query_drop("ALTER TABLE chat_change DROP PRIMARY KEY").unwrap();
    for _ in 0..2 {
        c.exec_drop("INSERT INTO chat_change VALUES (?, 1, 1, 'applied', ?)", (id(11).0.to_vec(), at(2))).unwrap();
    }
    drop(c);
    assert!(matches!(p.load_chat(id(10)), Err(Error::ChatUnreadable { why, .. }) if why.contains("two change links")));
    // A chat table of another shape: the conversations cannot be read or written, the model opens.
    sql(&p).query_drop("ALTER TABLE chat_anchor ADD COLUMN extra INT NULL").unwrap();
    assert!(matches!(p.list_chats(), Err(Error::ChatsUnreadable { .. })));
    assert!(matches!(p.create_chat(&new_chat(20, "Chat 3", 2), &no_keys()), Err(Error::ChatsUnreadable { .. })));
    assert!(matches!(p.open_chats(&at(3)), Err(Error::ChatsUnreadable { .. })));
    let path = p.path().to_path_buf();
    p.close().unwrap();
    let again = env.open(&path, &schema);
    assert_eq!(again.read_model(&schema).unwrap(), model);
    again.close().unwrap();
}

#[test]
fn a_model_table_may_not_take_a_conversations_name() {
    let universe = archforge_notations::store(ids(0)).unwrap().universe().clone();
    let (mut tables, junctions) = archforge_notations::storage();
    tables[0].name = "chat_note";
    assert!(ModelSchema::new(&universe, tables, junctions).is_err_and(|e| e.contains("chat_note")));
}

#[test]
fn a_failed_turn_names_its_error_and_only_a_failed_one() {
    let env = Env::new("failed");
    let schema = schema();
    let p = env.create("Failures", &schema);
    p.create_chat(&new_chat(1, "Chat 1", 0), &no_keys()).unwrap();
    p.start_turn(&turn(2, id(1), 1), &question(3, "Hi"), &no_keys()).unwrap();
    let mut end = ended(id(2), 2, None);
    end.outcome = TurnOutcome::Failed;
    assert!(matches!(p.end_turn(&end), Err(Error::ChatValue { .. })));
    end.error = Some(TurnError::RateLimited);
    p.end_turn(&end).unwrap();
    let t = &p.load_chat(id(1)).unwrap().turns[0].turn;
    assert_eq!((t.outcome, t.error), (TurnOutcome::Failed, Some(TurnError::RateLimited)));
    p.close().unwrap();
}
