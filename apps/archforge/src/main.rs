//! Archforge: the desktop application, a thin binary over the shell.
//!
//! Start-up order: arguments (an unknown option exits 2 before any I/O), `--version` (prints
//! without logging), the logger, the panic hook, the settings directory, preferences, then the
//! window, retried with the fallback graphics choice when the first one cannot start.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(test)]
mod acceptance;
mod agents;
mod app_dirs;
mod app_icon;
mod build_info;
mod chats;
mod fonts;
mod graphics;
mod logging;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_window;
mod prefs;
mod projects;
mod services;
#[cfg(test)]
mod winres;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use archforge_ui_egui::Shell;

const USAGE: &str = "usage: archforge [--version | --help | --control <port>]
  --control <port>  turn agent access on for this start: archforge-cli mcp --bridge <port> drives this window";

#[derive(Debug, Default)]
struct Args {
    version: bool,
    help: bool,
    /// The loopback port of the control channel, when agent access is on for this start.
    control: Option<u16>,
}

fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--version" | "-V" => a.version = true,
            "--help" | "-h" => a.help = true,
            "--control" => {
                let port = it.next().ok_or("--control needs a port")?;
                a.control = Some(port.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(|| format!("{port:?} is not a port (1 to 65535)"))?);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(a)
}

/// The frame host: forwards per-frame logic, drawing and exit to the shell.
struct DesktopApp {
    shell: Shell,
    marker: PathBuf,
    settings: PathBuf,
    /// The projects the shell creates: their servers stop on exit.
    projects: std::sync::Arc<projects::Projects>,
    /// AI Agents and Chat: its conversations, programs and agent access end on exit.
    agents: std::sync::Arc<agents::Agents>,
    /// The control channel while agent access is on; dropping it removes its token file.
    _control: Option<archforge_control::Server>,
    /// The preferences as last saved, and when the next save may run, so a change is saved a
    /// moment after it is made (Home's recent list survives a crash), not on every frame of a drag.
    saved: prefs::Prefs,
    next_save: std::time::Instant,
    #[cfg(target_os = "macos")]
    menu: Option<macos_menu::NativeMenu>,
}

/// The least time between two saves of changed preferences, and after a save that failed.
const PREFS_SETTLE: std::time::Duration = std::time::Duration::from_secs(1);
const PREFS_RETRY: std::time::Duration = std::time::Duration::from_secs(30);

impl eframe::App for DesktopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.menu {
            menu.frame(&mut self.shell);
        }
        self.shell.logic(ctx);
        if self.shell.frames == graphics::HEALTHY_AFTER_FRAMES {
            graphics::healthy(&self.marker);
        }
        // Changed preferences are saved once the change settles and no pointer is held (a drag
        // saves when it ends); the state is compared in place, copied only to save.
        if self.shell.state != self.saved.ui {
            let held = ctx.input(|i| i.pointer.any_down());
            let wait = self.next_save.saturating_duration_since(std::time::Instant::now());
            if held || !wait.is_zero() {
                ctx.request_repaint_after(wait.max(PREFS_SETTLE));
            } else {
                let now = prefs::capture(&self.shell);
                match prefs::save(&self.settings, &now) {
                    Ok(()) => {
                        self.saved = now;
                        self.next_save = std::time::Instant::now() + PREFS_SETTLE;
                    }
                    Err(e) => {
                        log::error!("the preferences could not be saved: {e}; trying again in {} seconds", PREFS_RETRY.as_secs());
                        self.next_save = std::time::Instant::now() + PREFS_RETRY;
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui);
    }

    fn on_exit(&mut self) {
        self.shell.before_exit();
        if let Err(e) = prefs::save(&self.settings, &prefs::capture(&self.shell)) {
            log::error!("the preferences could not be saved: {e}");
        }
        // Chat's programs and agent access first, then the projects' servers.
        self.agents.shutdown();
        self.projects.shutdown();
        // The person's own access closes with the window: its token is revoked and its file removed.
        self._control = None;
    }
}

/// Installs the process-wide panic hook: message and location go to the logger, and nothing on
/// this path can panic or abort, even with a closed standard-error stream.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown location".to_string());
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic without a message".to_string());
        log::error!("panic at {location}: {message}");
    }));
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln_stderr(&format!("archforge: {e}\n{USAGE}"));
            return 2;
        }
    };
    if args.version {
        println!("{}", build_info::version_line());
        return 0;
    }
    if args.help {
        println!("{USAGE}");
        return 0;
    }
    logging::init();
    install_panic_hook();
    let dirs = app_dirs::AppDirs::discover();
    logging::set_dir(&dirs.logs());
    log::info!("{}", build_info::version_line());

    // One queue of control requests for the window, from the person's own access and Chat's.
    let (window, requests) = agents::Window::new();
    // The person's agent access: the port is opened before the window, so a port in use exits 3 at
    // once.
    let server = match args.control.map(|port| agents::start_control(port, &dirs.settings, &window)) {
        None => None,
        Some(Ok(started)) => Some(started),
        Some(Err(e)) => {
            log::error!("agent access could not be turned on: {e}");
            let _ = writeln_stderr(&format!("archforge: agent access could not be turned on: {e}"));
            return 3;
        }
    };
    let server = Rc::new(RefCell::new(server));
    let requests = Rc::new(RefCell::new(Some(requests)));
    let (stored, warning) = prefs::load(&dirs.settings);
    if let Some(w) = warning {
        log::warn!("{w}");
    }
    let start = Rc::new(RefCell::new(Some(stored)));
    let marker = dirs.gpu_marker();
    let mut last_error = String::new();
    for choice in graphics::plan(&marker) {
        graphics::begin(&marker, choice);
        let mut options = native_options();
        graphics::configure(&mut options, choice);
        let (cell, marker, settings) = (start.clone(), marker.clone(), dirs.settings.clone());
        let (server, requests, window) = (server.clone(), requests.clone(), window.clone());
        let result = eframe::run_native(
            "Archforge",
            options,
            Box::new(move |cc| {
                let stored = cell.borrow_mut().take().ok_or("the application was already started")?;
                let projects = std::sync::Arc::new(projects::Projects::new(&settings, build_info::version_line()));
                let agents = std::sync::Arc::new(agents::Agents::new(&settings, window.clone()));
                let mut shell = Shell::new(services::native(projects.clone(), agents.clone()));
                prefs::apply(&stored, &mut shell);
                if let Some(rx) = requests.borrow_mut().take() {
                    shell.attach_control(rx);
                }
                window.attach(cc.egui_ctx.clone());
                #[cfg(target_os = "macos")]
                let menu = native_menu(&mut shell);
                let saved = prefs::capture(&shell);
                Ok(Box::new(DesktopApp {
                    shell,
                    marker,
                    settings,
                    projects,
                    agents,
                    _control: server.borrow_mut().take(),
                    saved,
                    next_save: std::time::Instant::now() + PREFS_SETTLE,
                    #[cfg(target_os = "macos")]
                    menu,
                }))
            }),
        );
        match result {
            Ok(()) => return 0,
            Err(e) => {
                last_error = e.to_string();
                log::error!("the window could not be opened with the {choice:?} graphics choice: {e}");
                if start.borrow().is_none() {
                    // The application had started; this is not a start-up failure to retry.
                    break;
                }
            }
        }
    }
    log::error!("no graphics choice could open the window: {last_error}");
    1
}

fn native_options() -> eframe::NativeOptions {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Archforge")
        .with_app_id("archforge")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0])
        .with_icon(app_icon::icon());
    #[cfg(target_os = "macos")]
    let viewport = macos_window::style(viewport);
    // The installed fonts come from fonts.rs, read and checked by the app, never mapped; eframe's
    // own system-font scanner stays off whatever its features.
    eframe::NativeOptions { viewport, system_font_fallback: false, ..Default::default() }
}

#[cfg(target_os = "macos")]
fn native_menu(shell: &mut Shell) -> Option<macos_menu::NativeMenu> {
    shell.top_inset = macos_window::TITLE_BAR_INSET;
    match macos_menu::build() {
        Ok(menu) => Some(menu),
        Err(e) => {
            log::warn!("the native menu bar could not be built: {e}");
            None
        }
    }
}

fn writeln_stderr(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    writeln!(std::io::stderr(), "{text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn the_window_never_installs_the_system_font_scanner() {
        assert!(!native_options().system_font_fallback, "installed fonts come from fonts.rs, never mapped");
    }

    /// The agent's way into a shell with an open project: each tool call answered at once by the
    /// shell's control methods, as the running window answers them.
    struct Direct(std::cell::RefCell<Shell>);

    impl archforge_mcp::Backend for Direct {
        fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, archforge_control::CallError> {
            archforge_ui_egui::control::answer(&mut self.0.borrow_mut(), method, &params).map_err(archforge_control::CallError::Refused)
        }
    }

    #[test]
    fn an_agent_builds_a_small_er_diagram_through_the_tools_and_undo_takes_it_apart() {
        use serde_json::{Value, json};
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        let session = archforge_notations::new_project_session("Shop", ids).unwrap();
        let infos: Vec<Value> = session.registry().infos(&session.facts()).into_iter().map(|i| serde_json::to_value(i).unwrap()).collect();
        let catalog = archforge_mcp::Catalog::new(&infos, false);
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        shell.session = Some(session);
        let agent = Direct(std::cell::RefCell::new(shell));
        let revision = || agent.0.borrow().session.as_ref().unwrap().revision();
        // One tool call; gives the command's result, or the refusal as an error.
        let tool = |name: &str, args: Value| -> Result<Value, String> {
            let reply = archforge_mcp::handle(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string().as_bytes(),
                &agent,
                &catalog,
            )
            .unwrap();
            let text = reply["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
            if reply["result"]["isError"] == true { Err(text) } else { Ok(serde_json::from_str::<Value>(&text).unwrap()["result"].clone()) }
        };
        let id = |v: Result<Value, String>| v.unwrap()["id"].as_str().unwrap().to_string();
        let schema = id(tool("er_schema_add", json!({ "name": "shop" })));
        let customer = id(tool("er_entity_add", json!({ "schema": schema, "name": "Customer" })));
        tool("er_attribute_add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let order = id(tool("er_entity_add", json!({ "schema": schema, "name": "Order" })));
        tool("er_attribute_add", json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let places = id(tool("er_relationship_add", json!({ "from": customer, "to": order, "name": "places", "create_foreign_key": true })));
        let diagram = id(tool("diagram_add", json!({ "kind": "er", "element": schema, "name": "Shop" })));
        let c = id(tool("diagram_place", json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 166 })));
        let o = id(tool("diagram_place", json!({ "diagram": diagram, "element": order, "x": 500, "y": 100, "width": 240, "height": 166 })));
        tool("diagram_connect", json!({ "diagram": diagram, "element": places, "source": c, "target": o })).unwrap();
        let changes = 10;
        // What the window draws: two tables on the diagram, the foreign key column in Order.
        let listed = tool("er_entity_list", json!({ "schema": schema })).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 2, "{listed}");
        let shown = tool("diagram_get", json!({ "diagram": diagram })).unwrap().to_string();
        assert!(shown.contains(&customer) && shown.contains(&order) && shown.contains(&places), "{shown}");
        // Refused calls change nothing.
        let before = revision();
        assert!(tool("er_entity_add", json!({ "schema": "not-an-id", "name": "X" })).is_err());
        assert!(tool("er_entity_add", json!({ "schema": schema, "name": "X", "colour": "red" })).unwrap_err().contains("colour"));
        let taken = tool("er_attribute_add", json!({ "entity": customer, "name": "customer_id" })).unwrap_err();
        assert!(taken.contains("customer_id") && taken.to_lowercase().contains("already"), "a name already taken: {taken}");
        assert_eq!(revision(), before, "refused calls leave the model as it was");
        // Undo takes it apart, one step per change, down to the empty project.
        for step in 0..changes {
            tool("undo", json!({})).unwrap_or_else(|e| panic!("undo {step}: {e}"));
        }
        assert!(tool("undo", json!({})).is_err(), "nothing more to undo");
        let left = tool("er_schema_list", json!({})).unwrap();
        assert_eq!(left, json!([]), "the project is empty again");
    }

    #[test]
    fn save_needs_an_open_project() {
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        let refused = archforge_ui_egui::control::answer(&mut shell, "app.save", &serde_json::json!({})).unwrap_err();
        assert!(refused.contains("no project is open"), "{refused}");
        assert!(archforge_ui_egui::control::answer(&mut shell, "app.save", &serde_json::json!({ "path": "x" })).is_err(), "it takes no keys");
    }

    #[test]
    fn an_agent_reads_a_schema_and_its_export_skills_as_resources_to_generate_ddl() {
        use serde_json::{Value, json};
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        let session = archforge_notations::new_project_session("Shop", ids).unwrap();
        let infos: Vec<Value> = session.registry().infos(&session.facts()).into_iter().map(|i| serde_json::to_value(i).unwrap()).collect();
        let catalog = archforge_mcp::Catalog::new(&infos, false);
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        shell.session = Some(session);
        let agent = Direct(std::cell::RefCell::new(shell));
        let rpc = |method: &str, params: Value| {
            archforge_mcp::handle(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string().as_bytes(), &agent, &catalog)
                .unwrap()
        };
        let tool = |name: &str, args: Value| -> Value {
            let reply = rpc("tools/call", json!({ "name": name, "arguments": args }));
            assert_eq!(reply["result"]["isError"], false, "{name}: {reply}");
            serde_json::from_str::<Value>(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap()["result"].clone()
        };
        let read = |uri: &str| -> Value {
            let reply = rpc("resources/read", json!({ "uri": uri }));
            serde_json::from_str(reply["result"]["contents"][0]["text"].as_str().unwrap_or_else(|| panic!("{uri}: {reply}"))).unwrap()
        };
        let id = |v: Value| v["id"].as_str().unwrap().to_string();
        // A small shop: customers place orders, keys and the foreign key the relationship makes.
        let schema = id(tool("er_schema_add", json!({ "name": "shop" })));
        let customer = id(tool("er_entity_add", json!({ "schema": schema, "name": "customer" })));
        tool("er_attribute_add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }));
        tool("er_attribute_add", json!({ "entity": customer, "name": "email", "type": "TEXT" }));
        let order = id(tool("er_entity_add", json!({ "schema": schema, "name": "purchase_order" })));
        tool("er_attribute_add", json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
        tool("er_relationship_add", json!({ "from": customer, "to": order, "name": "places", "create_foreign_key": true }));
        // An export skill for the whole project, one of its name on the schema that overrides it with
        // a dialect, and one on the order table alone.
        let everywhere =
            id(tool("skill_add", json!({ "name": "DDL conventions", "body": "Plain CREATE TABLE statements.", "triggers": ["export"] })));
        let on_schema = id(tool(
            "skill_add",
            json!({ "subject": schema, "name": "DDL conventions", "body": "PostgreSQL; a named constraint per key.", "triggers": ["export"], "dialect": "PostgreSQL" }),
        ));
        let on_order =
            id(tool("skill_add", json!({ "subject": order, "name": "Order history", "body": "Add a created_at column.", "triggers": ["export"] })));
        let revision = agent.0.borrow().session.as_ref().unwrap().revision();
        // Found by listing, then read: the schema with both its tables, its foreign key column too.
        let listed = rpc("resources/list", json!({}));
        assert!(listed["result"]["resources"].as_array().unwrap().iter().any(|r| r["uri"] == "archforge://skills"));
        let model = read(&format!("archforge://element/{schema}"));
        assert_eq!((model["kind"].as_str(), model["revision"].as_u64()), (Some("schema"), Some(revision)));
        let tables = model["schema"]["entities"].to_string();
        for column in ["customer_id", "email", "order_id", "customer_customer_id"] {
            assert!(tables.contains(column), "{column} in {tables}");
        }
        assert_eq!(model["schema"]["entities"].as_array().unwrap().len(), 2);
        assert_eq!(model["schema"]["relationships"][0]["name"], "places");
        // The export skills in effect on the schema agree with the tool's: the project's overridden
        // by the schema's, which carries the dialect.
        let effective = read(&format!("archforge://skills/effective/{schema}/export"));
        assert_eq!(effective["skills"], tool("skill_effective", json!({ "element": schema, "trigger": "export" }))["skills"]);
        let skills = effective["skills"].as_array().unwrap();
        let by_id = |id: &str| skills.iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("{id} in {skills:?}")).clone();
        assert_eq!(by_id(&everywhere)["overridden_by"], on_schema.as_str());
        assert_eq!(by_id(&on_schema)["dialect"], "PostgreSQL");
        assert!(!skills.iter().any(|s| s["id"] == on_order.as_str()), "the order table's skill is its own");
        // The order table, a second scope, adds its own.
        let on_table = read(&format!("archforge://skills/effective/{order}/export"));
        assert!(on_table["skills"].as_array().unwrap().iter().any(|s| s["id"] == on_order.as_str()), "{on_table}");
        // Reading changed nothing: neither the model nor its history.
        let s = &agent.0.borrow();
        let session = s.session.as_ref().unwrap();
        assert_eq!(session.revision(), revision);
        assert_eq!(session.undo_label(), Some("Add skill"));
    }

    #[test]
    fn the_mcp_tools_and_the_shells_methods_never_drift_apart() {
        let shell: Vec<&str> = archforge_ui_egui::control::METHODS.iter().map(|(m, _)| *m).collect();
        let (tools, call_app) = archforge_mcp::tool_methods();
        for (m, keys) in &tools {
            let taken = archforge_ui_egui::control::METHODS.iter().find(|(name, _)| name == m).map(|(_, k)| *k);
            let taken = taken.unwrap_or_else(|| panic!("the tool's method {m} is not one the shell answers"));
            let mut a: Vec<&str> = keys.iter().map(String::as_str).collect();
            let mut b: Vec<&str> = taken.to_vec();
            a.sort();
            b.sort();
            assert_eq!(a, b, "the tool for {m} and the shell differ in its keys");
        }
        for m in &shell {
            assert!(call_app.contains(m), "call_app does not name the method {m}");
        }
        // A fixed tool says it only reads exactly when the shell's method it calls only reads;
        // call_app reaches every method, so it never says so.
        use archforge_ui_egui::control::READS;
        assert!(READS.iter().all(|m| shell.contains(m)), "every read is a method");
        for (name, method, read_only) in archforge_mcp::fixed_tools() {
            assert_eq!(read_only, method.is_some_and(|m| READS.contains(&m)), "{name} calls {method:?}");
        }
    }

    /// A shell with a fresh project, for the control channel tests.
    fn shell_with_project() -> Shell {
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        shell.session = Some(archforge_notations::new_project_session("Shop", ids).unwrap());
        shell
    }

    /// The methods the shell calls reads change neither the model, its history nor the view.
    #[test]
    fn the_shells_reads_change_nothing() {
        use serde_json::json;
        let mut shell = shell_with_project();
        archforge_ui_egui::control::answer(&mut shell, "engine.execute", &json!({ "command": "er.schema.add", "params": { "name": "shop" } }))
            .unwrap();
        let state = |shell: &Shell| {
            let s = shell.session.as_ref().unwrap();
            (s.revision(), s.undo_label().map(str::to_string), s.journal().count(), s.view().selection.clone())
        };
        let before = state(&shell);
        for m in archforge_ui_egui::control::READS {
            let params = match m {
                "engine.describe" => json!({ "command": "er.schema.add" }),
                "app.saved" => json!({ "ticket": 1 }),
                _ => json!({}),
            };
            let _ = archforge_ui_egui::control::answer(&mut shell, m, &params);
            assert_eq!(state(&shell), before, "{m} is listed as a read");
        }
    }

    /// The origin `archforge-cli mcp --origin` sets travels the real control channel beside the
    /// parameters, labels the undo step each change makes, comes back in the result and in
    /// `document.inspect`, and cannot be set through the parameters.
    #[cfg(unix)]
    #[test]
    fn an_origin_labels_the_undo_steps_its_requests_make_through_the_control_channel() {
        use serde_json::{Value, json};
        let settings = std::env::temp_dir().join(format!("archforge-origin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&settings);
        std::fs::create_dir_all(&settings).unwrap();
        let (window, requests) = agents::Window::new();
        let server = agents::start_control(0, &settings, &window).unwrap();
        let port = server.port();
        let folder = settings.clone();
        let agent = std::thread::spawn(move || {
            let chat = archforge_control::Client::new(&folder, port).unwrap().with_origin(Some("Chat 1".into())).unwrap();
            let user = archforge_control::Client::new(&folder, port).unwrap();
            let schema = chat.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "shop" } })).unwrap();
            let id = schema["result"]["id"].as_str().unwrap().to_string();
            let batch = chat
                .call(
                    "engine.batch",
                    json!({ "label": "Add tables", "steps": [
                        { "command": "er.entity.add", "params": { "schema": id, "name": "Customer" } },
                        { "command": "er.entity.add", "params": { "schema": id, "name": "Order" } },
                    ] }),
                )
                .unwrap();
            let inspected = chat.call("document.inspect", json!({})).unwrap();
            let spoof = chat.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "x" }, "origin": "Spoof" }));
            let own = user.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "mine" } })).unwrap();
            let undone = chat.call("engine.undo", json!({})).unwrap();
            let after = user.call("document.inspect", json!({})).unwrap();
            (schema, batch, inspected, spoof, own, undone, after)
        });
        // The window's side: each request answered by the shell, as a frame does.
        let mut shell = shell_with_project();
        while !agent.is_finished() {
            if let Ok(req) = requests.recv_timeout(std::time::Duration::from_millis(10)) {
                use std::sync::atomic::Ordering;
                let ws = archforge_ui_egui::control::WAITING;
                if req.state.compare_exchange(ws, archforge_ui_egui::control::TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                    assert!(!req.read_only, "the person's own access is never read-only");
                    let answer = archforge_ui_egui::control::answer_request(&mut shell, &req);
                    let _ = req.reply.send(answer);
                }
            }
        }
        let (schema, batch, inspected, spoof, own, undone, after) = agent.join().unwrap();
        assert_eq!((schema["undo"].as_str(), schema["origin"].as_str()), (Some("Chat 1: Add schema"), Some("Chat 1")), "{schema}");
        assert_eq!((batch["undo"].as_str(), batch["origin"].as_str()), (Some("Chat 1: Add tables"), Some("Chat 1")), "{batch}");
        assert_eq!((inspected["undo"].as_str(), inspected["undo_origin"].as_str()), (Some("Chat 1: Add tables"), Some("Chat 1")));
        let spoof = spoof.unwrap_err().to_string();
        assert!(spoof.contains("\"origin\""), "an origin among the parameters is refused: {spoof}");
        assert_eq!((own["undo"].as_str(), own.get("origin")), (Some("Add schema"), None), "the user's own change has no origin");
        assert_eq!(undone["undone"], "Add schema");
        assert_eq!((after["redo_origin"].clone(), after["undo_origin"].as_str()), (Value::Null, Some("Chat 1")));
        let session = shell.session.as_ref().unwrap();
        assert_eq!(session.undo_label(), Some("Chat 1: Add tables"));
        drop(server);
        let _ = std::fs::remove_dir_all(&settings);
    }

    /// The window refuses a revoked token's request in the control channel's own words.
    #[test]
    fn the_window_and_the_channel_say_revoked_alike() {
        assert_eq!(archforge_ui_egui::control::REVOKED, archforge_control::REVOKED);
    }

    #[test]
    fn control_takes_a_port() {
        assert_eq!(args(&["--control", "47000"]).unwrap().control, Some(47000));
        for bad in [&["--control"][..], &["--control", "0"], &["--control", "port"], &["--control", "70000"]] {
            assert!(args(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        assert!(args(&["--bogus"]).unwrap_err().contains("--bogus"));
        assert!(args(&["model.archforge"]).is_err());
    }

    #[test]
    fn version_and_help_parse() {
        assert!(args(&["--version"]).unwrap().version);
        assert!(args(&["-h"]).unwrap().help);
        let none = args(&[]).unwrap();
        assert!(!none.version && !none.help);
    }

    #[test]
    fn every_preference_is_applied_and_a_damaged_file_is_set_aside() {
        let stored = prefs::Prefs {
            ui: archforge_ui_egui::UiState {
                theme: archforge_ui_egui::theme::ThemeKind::Dark,
                language: "de".to_string(),
                home_banner_closed: true,
                recent: vec![archforge_ui_egui::state::RecentProject {
                    name: "Commerce platform".into(),
                    path: std::env::temp_dir().join("Commerce platform.archforge"),
                    opened: 1_761_670_800,
                    view: None,
                }],
                ..archforge_ui_egui::UiState::default()
            },
        };
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        prefs::apply(&stored, &mut shell);
        assert_eq!(prefs::capture(&shell), stored);
        // A recent entry the window does not take (here a relative folder) is dropped on applying.
        let mut planted = stored.clone();
        planted.ui.recent.push(archforge_ui_egui::state::RecentProject {
            name: "Elsewhere".into(),
            path: "../elsewhere".into(),
            opened: 1,
            view: None,
        });
        prefs::apply(&planted, &mut shell);
        assert_eq!(prefs::capture(&shell), stored);
        assert_eq!(archforge_ui_egui::i18n::language(), "de");
        // A language no catalogue is registered under is said and replaced with English.
        let said = prefs::unknown_language("tlh").unwrap();
        assert!(said.contains("tlh") && said.contains("en"), "{said}");
        let mut klingon = stored.clone();
        klingon.ui.language = "tlh".to_string();
        prefs::apply(&klingon, &mut shell);
        assert_eq!((shell.state.language.as_str(), archforge_ui_egui::i18n::language()), ("en", "en"));
        let dir = std::env::temp_dir().join(format!("archforge-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        prefs::save(&dir, &stored).unwrap();
        assert_eq!(prefs::load(&dir), (stored, None));
        std::fs::write(dir.join(prefs::FILE), b"{ damaged").unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("set aside"));
        assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("damaged")));
        std::fs::write(dir.join(prefs::FILE), vec![b' '; usize::try_from(prefs::MAX_BYTES).unwrap() + 1]).unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("bytes long"));
        // A file that is not text is set aside too, never overwritten unseen.
        std::fs::write(dir.join(prefs::FILE), [0xff, 0xfe, 0x00]).unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("set aside"));
        assert!(!dir.join(prefs::FILE).exists(), "the unreadable file was moved aside");
        std::fs::remove_dir_all(&dir).unwrap();
        archforge_ui_egui::i18n::set_language("en");
    }

    #[test]
    fn the_graphics_plan_skips_a_choice_that_crashed_and_prefers_the_monitors_gpu() {
        let dir = std::env::temp_dir().join(format!("archforge-gpu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("gpu-starting");
        assert_eq!(graphics::plan(&marker), vec![graphics::Choice::Primary, graphics::Choice::Fallback]);
        graphics::begin(&marker, graphics::Choice::Primary);
        assert_eq!(graphics::plan(&marker), vec![graphics::Choice::Fallback]);
        graphics::healthy(&marker);
        assert!(!marker.exists());
        std::fs::remove_dir_all(&dir).unwrap();
        let adapters = [(0x8086, 1, true), (0x10de, 2, true), (0x1002, 3, false)];
        assert_eq!(graphics::choose_adapter(&adapters, &[(0x10de, 2)]), Some(1));
        assert_eq!(graphics::choose_adapter(&adapters, &[(0x1002, 3)]), Some(0), "an adapter that cannot present is never chosen");
        assert_eq!(graphics::choose_adapter(&adapters, &[]), Some(0));
        assert_eq!(graphics::choose_adapter(&[], &[]), None);
    }

    #[test]
    fn the_version_line_says_dev_build_without_metadata() {
        if build_info::COMMIT.is_none() {
            assert!(build_info::version_line().ends_with("(dev build)"));
        }
    }
}
