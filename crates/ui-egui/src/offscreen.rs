//! Offscreen rendering of the window, behind the `snapshot` feature: a fresh shell in
//! the headless harness, drawn once settled and rendered to RGBA.

use crate::theme::ThemeKind;
use crate::{Folder, Services, Shell};

/// What a render shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    /// The New project dialog over Home, a name typed and the usual folder.
    NewProject,
    /// The same after a creation failed for want of Dolt.
    NewProjectFailed,
    /// The workspace of a project just created.
    Workspace,
    /// The same after `+` added the first schema, its name open for typing.
    WorkspaceSchema,
    /// A table with columns selected, its Properties shown.
    WorkspaceTable,
    /// The same table with the Skills tab open on it, a skill on the schema inherited.
    WorkspaceSkills,
    /// A first diagram of a schema, just created and open, empty.
    WorkspaceDiagramEmpty,
    /// A diagram of two tables and their relationship, the first table selected.
    WorkspaceDiagram,
    /// An empty diagram with its first entity just placed, its name open for typing.
    WorkspaceEntityNew,
}

/// Renders Home at `width` by `height` logical pixels, `scale` pixels a point, in `theme`:
/// `(width, height, rgba)` in pixels.
pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String> {
    render_screen(width, height, scale, theme, Screen::Home)
}

/// [`render`] of a given screen.
pub fn render_screen(width: f32, height: f32, scale: f32, theme: ThemeKind, screen: Screen) -> Result<(u32, u32, Vec<u8>), String> {
    crate::i18n::set_language("en");
    let n = std::sync::atomic::AtomicU64::new(0);
    let ids: std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync> = std::sync::Arc::new(move || {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&i.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    });
    let mut shell = Shell::new(Services { user_name: Some("Jordan Dale".into()), ids: Some(ids), ..Services::default() });
    shell.state.theme = theme;
    if !matches!(screen, Screen::Home | Screen::NewProject | Screen::NewProjectFailed) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        shell.open_project(crate::Opened {
            name: "Commerce platform".into(),
            path: "/Users/robin/Documents/Archforge/Commerce platform.archforge".into(),
        });
    }
    if matches!(screen, Screen::NewProject | Screen::NewProjectFailed) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        crate::new_project::open(&mut shell);
        if let Some(d) = shell.new_project.as_mut() {
            d.name = "Commerce platform".into();
            if screen == Screen::NewProjectFailed {
                d.show_failure(crate::CreateError::DoltMissing {
                    detail: "no dolt found (ARCHFORGE_DOLT is not set and no dolt is on the PATH)".into(),
                });
            }
        }
    }
    if screen == Screen::WorkspaceSchema {
        // The session first, as the first frame would make it, then the schema the + adds.
        shell.adopt_project();
        crate::explorer::add(&mut shell, crate::explorer::Kind::Schema);
    }
    if matches!(screen, Screen::WorkspaceTable | Screen::WorkspaceSkills) {
        shell.adopt_project();
        sample_table(&mut shell, screen == Screen::WorkspaceSkills)?;
    }
    if matches!(screen, Screen::WorkspaceDiagramEmpty | Screen::WorkspaceDiagram | Screen::WorkspaceEntityNew) {
        shell.adopt_project();
        sample_diagram(&mut shell, screen == Screen::WorkspaceDiagram)?;
        if screen == Screen::WorkspaceEntityNew {
            sample_new_entity(&mut shell)?;
        }
    }
    let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(width, height)).with_pixels_per_point(scale).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            // The harness frames its ui with an 8 px margin; the shell draws on the whole window,
            // as it does in the application.
            let window = ui.ctx().content_rect();
            let mut full = ui.new_child(egui::UiBuilder::new().max_rect(window));
            full.set_clip_rect(window);
            shell.ui(&mut full);
        },
        shell,
    );
    harness.run();
    let image = harness.render()?;
    let (w, h) = (image.width(), image.height());
    Ok((w, h, image.into_raw()))
}

/// A schema "Shop" with a table "Customer" of three columns, the table selected; with `skills`,
/// a naming skill on the schema and one on the table, the Skills tab open. The first refused
/// step is the error, so a sample never renders half-built.
fn sample_table(shell: &mut Shell, skills: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_er::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop" }))?;
    let customer = run(session, "er.entity.add", json!({ "schema": schema, "name": "Customer" }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR(255)", "unique": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR(120)" }))?;
    if skills {
        run(
            session,
            "skill.add",
            json!({ "subject": schema, "name": "Table naming", "applies_to": "table", "triggers": ["naming"], "body": "Tables are singular nouns in PascalCase." }),
        )?;
        run(
            session,
            "skill.add",
            json!({ "subject": customer, "name": "Customer naming", "triggers": ["naming"], "body": "Columns end in _id for keys." }),
        )?;
    }
    run(session, "view.select", json!({ "ids": [customer] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    if skills {
        shell.state.panel = crate::state::Panel::Skills;
    }
    Ok(())
}

/// A schema "Shop schema" with its diagram open: empty, as the journey's first diagram; or with
/// Customer and Order placed, the relationship `places` between them and Customer selected, as
/// the design's ER frame.
fn sample_diagram(shell: &mut Shell, filled: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_er::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop schema" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": "er", "element": schema, "name": "Shop schema" }))?;
    let mut select = diagram.clone();
    if filled {
        let customer = run(session, "er.entity.add", json!({ "schema": schema, "name": "Customer" }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "region", "type": "TEXT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR(255)", "unique": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR(120)" }))?;
        let order = run(session, "er.entity.add", json!({ "schema": schema, "name": "Order" }))?;
        run(session, "er.attribute.add", json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": order, "name": "placed_at", "type": "TIMESTAMP" }))?;
        let places = run(
            session,
            "er.relationship.add",
            json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places", "create_foreign_key": true }),
        )?;
        let c = run(session, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 200, "y": 360, "width": 240, "height": 166 }))?;
        let o = run(session, "diagram.place", json!({ "diagram": diagram, "element": order, "x": 560, "y": 360, "width": 240, "height": 166 }))?;
        let edge = run(session, "diagram.connect", json!({ "diagram": diagram, "element": places, "source": c, "target": o }))?;
        run(session, "diagram.label", json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        select = customer;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let id = archforge_mof::Uuid::parse(&diagram).ok_or_else(|| "the diagram's id does not parse".to_string())?;
    crate::canvas::open(shell, id);
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    run(session, "view.select", json!({ "ids": [select] }))?;
    // The canvas fits the diagram on its first frame; the sample wants the design's 100 %.
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::Vec2::ZERO;
    Ok(())
}

/// The first entity of the empty sample diagram, placed by the Entity tool with its name open for
/// typing, as the design's "First entity named in place".
fn sample_new_entity(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let diagram = shell.canvas.active.and_then(|id| store.by_uuid(id)).ok_or_else(|| "the sample has no open diagram".to_string())?;
    let schema = crate::diagrams::schema_of(store, diagram).ok_or_else(|| "the sample diagram has no schema".to_string())?;
    let (schema_id, diagram_id) = (store.uuid(schema).map_err(|e| e.to_string())?, store.uuid(diagram).map_err(|e| e.to_string())?);
    let out = session
        .batch("Add table", |b| {
            let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": "Entity" }))?;
            let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            b.execute(
                "diagram.place",
                &json!({ "diagram": diagram_id.to_string(), "element": entity, "x": 200, "y": 360, "width": crate::editing::ENTITY_WIDTH, "height": crate::editing::ENTITY_EMPTY_HEIGHT }),
            )?;
            Ok(())
        })
        .map_err(|e| format!("Add table: {e:?}"))?;
    let id_at = |i: usize| out.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(archforge_mof::Uuid::parse);
    let (Some(entity), Some(node)) = (id_at(0), id_at(1)) else { return Err("the sample's table has no id".to_string()) };
    session.execute("view.select", &json!({ "ids": [entity.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.canvas.tool = crate::canvas::Tool::Entity;
    shell.canvas.rename = Some(crate::editing::NodeRename { node, entity, text: "Entity".to_string(), opened: true, refused: None });
    Ok(())
}
