//! Offscreen rendering of the window, behind the `snapshot` feature: a fresh shell in
//! the headless harness, drawn once settled and rendered to RGBA.

use crate::theme::ThemeKind;
use crate::{Folder, Services, Shell};

/// What a render shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    /// Home with a pinned folder and four recent projects, shown as a gallery.
    HomeGallery,
    /// The same with its search open, nothing typed yet.
    HomeSearch,
    /// Home with the Settings dialog open, a name entered.
    HomeSettings,
    /// The New project dialog over Home, a name typed and the usual folder.
    NewProject,
    /// The same after a creation failed for want of Dolt.
    NewProjectFailed,
    /// The same with a folder chosen with Change… in place of the usual one.
    NewProjectFolder,
    /// The workspace of a project just created.
    Workspace,
    /// The same after `+` added the first schema, its name open for typing.
    WorkspaceSchema,
    /// A table with columns selected, its Properties shown.
    WorkspaceTable,
    /// The same table with the Skills tab open on it, a skill on the schema inherited.
    WorkspaceSkills,
    /// The table's naming skill open in its tab, its Markdown in Write.
    WorkspaceSkillTab,
    /// The same skill in Preview.
    WorkspaceSkillPreview,
    /// The same Preview with a link clicked, its confirmation open.
    WorkspaceSkillLink,
    /// A skill just made with the Skills panel's +: its tab, the template, the name selected.
    WorkspaceSkillNew,
    /// The same table with the key editor open on its unique constraint.
    WorkspaceKeyEditor,
    /// The same table with the column editor open on its email column, More open.
    WorkspaceColumnEditor,
    /// The same table with the index editor open on its index, More open.
    WorkspaceIndexEditor,
    /// An empty diagram with Party and its subtypes Person and Company drawn, Party selected
    /// (slice S6).
    WorkspaceSubtypes,
    /// The table's schema selected, with a domain and an enum type in its Types section and in
    /// the Models tree.
    WorkspaceSchemaTypes,
    /// The table's schema selected, with a sequence owned by its key and a view in its Sequences
    /// and Views sections and in the Models tree (slice S7).
    WorkspaceSchemaObjects,
    /// The diagram of two tables with a table of the CRM schema placed beside them, dimmed and
    /// dashed, its relationship to Order drawn (slice S7).
    WorkspaceCrossSchema,
    /// The diagram of two tables with Invoice and Product placed so that their lines from Customer
    /// would cross Order: right-angled lines go around it.
    WorkspaceRouting,
    /// A class diagram of a package with three classes, the package open in the tree
    ///.
    WorkspaceClassDiagram,
    /// The sample class model to read: members, every line kind and end.
    WorkspaceClassDiagramRead,
    /// The sample class model with the Chat panel open: Coming soon.
    WorkspaceChat,
    /// A class diagram of three cards: a new class with its first operation at the default size, one
    /// grown to show its members, and two made short by hand saying the rows they hide.
    WorkspaceClassSizing,
    /// The sample class model with Order selected: its Properties, Abstract and its members
    ///.
    WorkspaceClassMembers,
    /// The same with Customer's operation rename open in its editor, its parameter on a row.
    WorkspaceMemberEditor,
    /// The sample class model with Catalog selected and a new attribute typed on its card.
    WorkspaceInlineMember,
    /// The sample class model with the association "paid by" selected: its line's end handles and
    /// its Properties with both ends.
    WorkspaceAssociation,
    /// The same with its Payment end's multiplicity presets open, the current one ticked.
    WorkspaceMultiplicityPresets,
    /// The same with `5..2` typed into the Payment end's multiplicity and refused under it.
    WorkspaceMultiplicityRefused,
    /// The sample class model with the Relationship tool active and its kinds' menu open.
    WorkspaceRelateKinds,
    /// The sample class model edited, unsaved, and its close asking Save, Don't save or Cancel
    ///.
    WorkspaceCloseAsk,
    /// The routing sample with the top bar's search open on "order".
    WorkspaceSearch,
    /// The Diagrams explorer with the diagram "dbo" open, its one table Company placed and its key
    /// column selected: the owner's screen of 2026-10-31.
    WorkspaceColumnSelected,
    /// The diagram of two tables with a mapping table between Order and Tag selected: its
    /// Properties offer to collapse it, Order's key column shows its link mark.
    WorkspaceMappingTable,
    /// A first diagram of a schema, just created and open, empty.
    WorkspaceDiagramEmpty,
    /// A diagram of two tables and their relationship, the first table selected.
    WorkspaceDiagram,
    /// The same with a removal's toast and its Undo, clear of the minimap.
    WorkspaceToast,
    /// The filled diagram with its schema selected, export skills on the project and the schema,
    /// and File → Generate… open over it.
    WorkspaceGenerate,
    /// The same with the Appearance panel open on the first table, its fill and border its own.
    WorkspaceAppearance,
    /// The same with the relationship selected and its own width.
    WorkspaceAppearanceLine,
    /// The first table's Appearance with the picker open on its fill.
    WorkspaceAppearancePicker,
    /// An empty diagram with its first entity just placed, its name open for typing.
    WorkspaceEntityNew,
    /// The diagram of two tables with the Relationship tool active and the cardinality popover open.
    WorkspaceCardinality,
    /// The same diagram with the relationship selected, its Properties shown.
    WorkspaceRelationship,
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
    let mut shell = Shell::new(Services { ids: Some(ids), ..Services::default() });
    shell.state.theme = theme;
    if matches!(screen, Screen::HomeGallery | Screen::HomeSearch) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        let now = crate::home::now();
        for (name, ago) in [("Billing", 9 * 86_400), ("Warehouse", 2 * 86_400), ("Customer care", 3 * 3_600), ("Commerce platform", 600)] {
            shell.state.remember(name, std::path::Path::new(&format!("/Users/robin/Documents/Archforge/{name}.archforge")), now - ago);
        }
        shell.state.recent_view = crate::state::RecentView::Gallery;
        shell.state.pin(std::path::Path::new("/Users/robin/Projects/Architecture"));
        shell.home_search.open = screen == Screen::HomeSearch;
    }
    if screen == Screen::HomeSettings {
        shell.state.user_name = "Ada Lovelace".into();
        crate::settings::open(&mut shell);
    }
    if !matches!(
        screen,
        Screen::Home
            | Screen::HomeGallery
            | Screen::HomeSearch
            | Screen::HomeSettings
            | Screen::NewProject
            | Screen::NewProjectFailed
            | Screen::NewProjectFolder
    ) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        shell.open_project(crate::Opened {
            name: "Commerce platform".into(),
            path: "/Users/robin/Documents/Archforge/Commerce platform.archforge".into(),
            ..Default::default()
        });
    }
    if matches!(screen, Screen::NewProject | Screen::NewProjectFailed | Screen::NewProjectFolder) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        crate::new_project::open(&mut shell);
        if let Some(d) = shell.new_project.as_mut() {
            d.name = "Commerce platform".into();
            if screen == Screen::NewProjectFailed {
                d.show_failure(crate::CreateError::DoltMissing {
                    detail: "no dolt found (ARCHFORGE_DOLT is not set and no dolt is on the PATH)".into(),
                });
            }
            if screen == Screen::NewProjectFolder {
                d.folder = Some("/Volumes/Work/Clients/Acme".into());
            }
        }
        // Change… as a host with a folder picker shows it.
        if screen == Screen::NewProjectFolder {
            shell.services.pick_folder = Some(std::sync::Arc::new(|| None));
        }
    }
    if screen == Screen::WorkspaceSchema {
        // The session first, as the first frame would make it, then the schema the + adds.
        shell.adopt_project();
        crate::explorer::add(&mut shell, "schema");
    }
    if matches!(
        screen,
        Screen::WorkspaceTable
            | Screen::WorkspaceSkills
            | Screen::WorkspaceSkillTab
            | Screen::WorkspaceSkillPreview
            | Screen::WorkspaceSkillLink
            | Screen::WorkspaceSkillNew
            | Screen::WorkspaceKeyEditor
            | Screen::WorkspaceColumnEditor
            | Screen::WorkspaceIndexEditor
            | Screen::WorkspaceSchemaTypes
            | Screen::WorkspaceSchemaObjects
    ) {
        shell.adopt_project();
        let skill_tab = matches!(screen, Screen::WorkspaceSkillTab | Screen::WorkspaceSkillPreview | Screen::WorkspaceSkillLink);
        sample_table(&mut shell, screen == Screen::WorkspaceSkills || skill_tab || screen == Screen::WorkspaceSkillNew)?;
        if skill_tab {
            sample_skill_tab(&mut shell, screen != Screen::WorkspaceSkillTab)?;
        }
        if screen == Screen::WorkspaceSkillNew {
            let customer = shell.session.as_ref().and_then(|s| s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok()));
            crate::skill_tab::create(&mut shell, customer);
        }
        if screen == Screen::WorkspaceSkillLink
            && let Some(doc) = shell.canvas.skills.first_mut()
        {
            doc.link = Some("https://docs.example.com.sign-in.accounts.verify.example.org/naming/columns?section=keys".into());
        }
        if screen == Screen::WorkspaceSchemaTypes {
            sample_types(&mut shell)?;
        }
        if screen == Screen::WorkspaceSchemaObjects {
            sample_objects(&mut shell)?;
        }
        if screen == Screen::WorkspaceColumnEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let email = store
                .alive()
                .find(|o| archforge_core::name_of(store, *o) == "email")
                .and_then(|a| Some((store.uuid(a).ok()?, archforge_er::rules::entity_of(store, a)?)));
            if let Some((column, entity)) = email
                && let Ok(entity) = store.uuid(entity)
            {
                crate::panels::open_column(&mut shell, entity, Some(column));
                if let Some(c) = shell.panels.column.as_mut() {
                    c.more = true;
                }
            }
        }
        if screen == Screen::WorkspaceIndexEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let customer = store.alive().find(|o| archforge_core::name_of(store, *o) == "Customer");
            if let Some(e) = customer
                && let Ok(entity) = store.uuid(e)
                && let Some(index) = crate::indexes::rows(store, e).into_iter().next()
            {
                crate::indexes::open(&mut shell, entity, Some(&index));
                if let Some(i) = shell.panels.index.as_mut() {
                    i.more = true;
                }
            }
        }
        if screen == Screen::WorkspaceKeyEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let customer = store.alive().find(|o| archforge_core::name_of(store, *o) == "Customer");
            if let Some(e) = customer
                && let Ok(entity) = store.uuid(e)
                && let Some(unique) = crate::keys::rows(store, e).into_iter().find(|k| !k.primary)
            {
                crate::keys::open(&mut shell, entity, Some(&unique));
            }
        }
    }
    if screen == Screen::WorkspaceColumnSelected {
        shell.adopt_project();
        sample_column_selected(&mut shell)?;
    }
    if screen == Screen::WorkspaceClassDiagram {
        shell.adopt_project();
        sample_class_diagram(&mut shell)?;
    }
    if screen == Screen::WorkspaceClassSizing {
        shell.adopt_project();
        sample_class_sizing(&mut shell)?;
    }
    if screen == Screen::WorkspaceChat {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        shell.state.show_panel(crate::state::Panel::Agent);
    }
    if screen == Screen::WorkspaceClassDiagramRead {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
    }
    if screen == Screen::WorkspaceCloseAsk {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        if let Some(s) = shell.session.as_mut() {
            s.mark_unsaved();
        }
        shell.close_project();
    }
    let association = matches!(screen, Screen::WorkspaceAssociation | Screen::WorkspaceMultiplicityPresets | Screen::WorkspaceMultiplicityRefused);
    if association || screen == Screen::WorkspaceRelateKinds {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        if association {
            let id = {
                let store = shell.session.as_ref().ok_or("no project is open")?.doc();
                let a = store
                    .alive()
                    .find(|o| store.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(store, *o) == "paid by")
                    .ok_or("the sample has no such association")?;
                store.uuid(a).map_err(|e| e.to_string())?
            };
            crate::panels::run(&mut shell, "view.select", serde_json::json!({ "ids": [id.to_string()] }))?;
            shell.state.show_panel(crate::state::Panel::Properties);
            if screen == Screen::WorkspaceMultiplicityPresets {
                shell.panels.menu = Some(crate::panels::Menu::EndMultiplicity(1));
            }
            if screen == Screen::WorkspaceMultiplicityRefused {
                crate::uml_relate::refuse_typed(&mut shell, id, 1, "5..2")?;
            }
        } else {
            shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
            shell.canvas.uml_relation = crate::uml_relate::Relate::Composition;
            shell.canvas.popover = true;
        }
    }
    if screen == Screen::WorkspaceInlineMember {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        let class = {
            let store = shell.session.as_ref().ok_or("no project is open")?.doc();
            let c = store
                .alive()
                .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == "Catalog")
                .ok_or("the sample has no such class")?;
            store.uuid(c).map_err(|e| e.to_string())?
        };
        if !crate::uml_inline::open(&mut shell, class, crate::uml_panel::MemberKind::Attribute, None) {
            return Err("the class's card cannot hold a row".into());
        }
        if let Some(inline) = shell.canvas.inline.as_mut() {
            inline.edit.name = "code".into();
            inline.ty = "Integer".into();
        }
        // Catalog at a working zoom near the canvas's top left.
        let at = shell.session.as_ref().and_then(|s| {
            let store = s.doc();
            let n = archforge_diagram::views_of(store, store.by_uuid(class)?).2.into_iter().next()?;
            Some(archforge_diagram::grid::bounds_of(store, n))
        });
        if let Some(b) = at {
            shell.canvas.fit_pending = false;
            shell.canvas.zoom = 1.25;
            shell.canvas.pan = egui::vec2(200.0 - b.x as f32 * 1.25, 160.0 - b.y as f32 * 1.25);
        }
    }
    if matches!(screen, Screen::WorkspaceClassMembers | Screen::WorkspaceMemberEditor) {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_class_members(&mut shell, screen == Screen::WorkspaceMemberEditor)?;
    }
    if matches!(
        screen,
        Screen::WorkspaceDiagramEmpty
            | Screen::WorkspaceDiagram
            | Screen::WorkspaceToast
            | Screen::WorkspaceGenerate
            | Screen::WorkspaceAppearance
            | Screen::WorkspaceAppearanceLine
            | Screen::WorkspaceAppearancePicker
            | Screen::WorkspaceEntityNew
            | Screen::WorkspaceCardinality
            | Screen::WorkspaceRelationship
            | Screen::WorkspaceMappingTable
            | Screen::WorkspaceSubtypes
            | Screen::WorkspaceCrossSchema
            | Screen::WorkspaceRouting
            | Screen::WorkspaceSearch
    ) {
        shell.adopt_project();
        sample_diagram(
            &mut shell,
            matches!(
                screen,
                Screen::WorkspaceDiagram
                    | Screen::WorkspaceToast
                    | Screen::WorkspaceGenerate
                    | Screen::WorkspaceAppearance
                    | Screen::WorkspaceAppearanceLine
                    | Screen::WorkspaceAppearancePicker
                    | Screen::WorkspaceCardinality
                    | Screen::WorkspaceRelationship
                    | Screen::WorkspaceMappingTable
                    | Screen::WorkspaceCrossSchema
                    | Screen::WorkspaceRouting
                    | Screen::WorkspaceSearch
            ),
        )?;
        if matches!(screen, Screen::WorkspaceRouting | Screen::WorkspaceSearch) {
            sample_routing(&mut shell)?;
        }
        if screen == Screen::WorkspaceSearch {
            crate::home_search::open_with(&mut shell, "order");
        }
        if screen == Screen::WorkspaceGenerate {
            sample_generate(&mut shell)?;
        }
        if screen == Screen::WorkspaceToast {
            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            let text = crate::i18n::fmt(&crate::i18n::tr("{what} removed"), &[("what", "region")]);
            shell.canvas.toast = Some(crate::connect::Toast { text, until: u64::MAX, undo_label });
        }
        if screen == Screen::WorkspaceCrossSchema {
            sample_cross_schema(&mut shell)?;
        }
        if screen == Screen::WorkspaceMappingTable {
            sample_mapping_table(&mut shell)?;
        }
        if screen == Screen::WorkspaceSubtypes {
            sample_subtypes(&mut shell)?;
        }
        if matches!(screen, Screen::WorkspaceAppearance | Screen::WorkspaceAppearanceLine | Screen::WorkspaceAppearancePicker) {
            sample_appearance(&mut shell, screen)?;
        }
        if screen == Screen::WorkspaceRelationship
            && let Some(session) = shell.session.as_mut()
        {
            let store = session.doc();
            let rel = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).and_then(|o| store.uuid(o).ok());
            if let Some(rel) = rel {
                session.execute("view.select", &serde_json::json!({ "ids": [rel.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
            }
        }
        if screen == Screen::WorkspaceEntityNew {
            sample_new_entity(&mut shell)?;
        }
        if screen == Screen::WorkspaceCardinality {
            shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
            shell.canvas.popover = true;
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
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop" }))?;
    let customer = run(session, "er.entity.add", json!({ "schema": schema, "name": "Customer" }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR", "length": 255, "unique": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR", "length": 120 }))?;
    let created = run(session, "er.attribute.add", json!({ "entity": customer, "name": "created_at", "type": "TIMESTAMP" }))?;
    run(session, "er.check.add", json!({ "entity": customer, "expression": "email <> ''", "name": "ck_customer_email" }))?;
    run(
        session,
        "er.index.add",
        json!({
            "entity": customer,
            "columns": [{ "expression": "lower(email)" }, { "attribute": created, "direction": "descending" }],
            "name": "ix_customer_email_created",
            "where": "email <> ''",
            "method": "btree",
        }),
    )?;
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
        shell.state.show_panel(crate::state::Panel::Skills);
    }
    Ok(())
}

/// Writes the table's naming skill out in Markdown and opens it in its tab, in Preview when
/// `preview`.
fn sample_skill_tab(shell: &mut Shell, preview: bool) -> Result<(), String> {
    use serde_json::json;
    const TEXT: &str = "# Customer naming\n\nName every column of **Customer** in `snake_case`, and keep each name under 30 characters.\n\n## Keys\n\n- The primary key is `customer_id`.\n- A foreign key to another table is named `<table>_id`.\n\n## Words\n\n> Prefer plain words over abbreviations: `address`, not `addr`.\n\n1. Singular table names: `Customer`, not `Customers`.\n2. No prefixes such as `tbl_` or `t_`.\n";
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let skill = archforge_core::skills::skills(store)
        .into_iter()
        .find(|s| archforge_core::skills::name_of(store, *s) == "Customer naming")
        .and_then(|s| store.uuid(s).ok())
        .ok_or_else(|| "the sample has no Customer naming skill".to_string())?;
    session.execute("skill.set-body", &json!({ "skill": skill.to_string(), "body": TEXT })).map_err(|e| format!("skill.set-body: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    crate::canvas::open(shell, skill);
    if let Some(doc) = shell.canvas.skills.iter_mut().find(|d| d.skill == skill) {
        doc.preview = preview;
    }
    Ok(())
}

/// Adds a domain and an enum type to the sample table's schema, puts email on the domain and a
/// status column on the enum type, and selects the schema.
fn sample_types(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(customer), Some(email)) = (by_name("Shop"), by_name("Customer"), by_name("email")) else {
        return Err("the sample table".into());
    };
    let domain = run(
        session,
        "er.domain.add",
        json!({ "schema": schema, "name": "email_address", "base_type": "VARCHAR", "length": 255, "not_null": true, "checks": ["position('@' in value) > 1"] }),
    )?;
    let status = run(session, "er.enum.add", json!({ "schema": schema, "name": "customer_status", "literals": ["active", "dormant", "closed"] }))?;
    run(session, "er.attribute.set-type", json!({ "attribute": email, "domain": domain }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "status", "enum": status, "default": "'active'" }))?;
    run(session, "view.select", json!({ "ids": [schema] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// Adds Tag to the filled diagram, a many-to-many line from Order to it, and resolves the line into
/// the mapping table Order_Tag, which stays selected.
fn sample_mapping_table(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(order), Some(diagram)) = (
        by_name("Shop schema"),
        by_name("Order"),
        store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).and_then(|d| store.uuid(d).ok()).map(|u| u.to_string()),
    ) else {
        return Err("the filled diagram".into());
    };
    let order_node = archforge_diagram::views_of(store, store.by_uuid(archforge_mof::Uuid::parse(&order).ok_or("id")?).ok_or("order")?)
        .2
        .first()
        .and_then(|n| store.uuid(*n).ok())
        .map(|u| u.to_string())
        .ok_or("Order's node")?;
    let tag = run(session, "er.entity.add", json!({ "schema": schema, "name": "Tag" }))?;
    run(session, "er.attribute.add", json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true }))?;
    run(session, "er.attribute.add", json!({ "entity": tag, "name": "label", "type": "VARCHAR", "length": 40 }))?;
    let line = run(session, "er.relationship.add", json!({ "from": order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }))?;
    let t = run(session, "diagram.place", json!({ "diagram": diagram, "element": tag, "x": 920, "y": 360, "width": 240, "height": 120 }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": line, "source": order_node, "target": t }))?;
    run(session, "er.relationship.resolve", json!({ "relationship": line }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// A schema "dbo" with its diagram open in the Diagrams explorer, Company on it, its key column
/// selected.
fn sample_column_selected(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "dbo" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": "er", "element": schema, "name": "dbo" }))?;
    let company = run(session, "er.entity.add", json!({ "schema": schema, "name": "Company" }))?;
    let id = run(session, "er.attribute.add", json!({ "entity": company, "name": "id", "type": "UUID", "primary_key": true }))?;
    run(
        session,
        "diagram.place",
        json!({ "diagram": diagram, "element": company, "x": 160, "y": 180, "width": 240, "height": crate::editing::content_height(1) }),
    )?;
    let d = archforge_mof::Uuid::parse(&diagram).ok_or_else(|| "the diagram's id does not parse".to_string())?;
    crate::canvas::open(shell, d);
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    run(session, "view.select", json!({ "ids": [id] }))?;
    shell.state.show_explorer(crate::state::Explorer::Diagrams);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::Vec2::ZERO;
    Ok(())
}

/// Gives the first table of the open diagram its own fill and border, and the relationship its own
/// width, then opens Appearance on the table, on the relationship, or on the table with the picker.
fn sample_appearance(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let class = |c| store.alive().find(|o| store.class_of(*o).ok() == Some(c));
    let selected = session.view().selection.first().copied().ok_or_else(|| "nothing is selected".to_string())?;
    let node = store
        .alive()
        .find(|o| {
            store.class_of(*o).ok() == Some(archforge_diagram::NODE)
                && store.get(*o, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(selected)
        })
        .and_then(|o| store.uuid(o).ok())
        .ok_or_else(|| "the selected table has no node".to_string())?;
    let rel = class(archforge_er::RELATIONSHIP).and_then(|o| store.uuid(o).ok()).ok_or_else(|| "the sample has no relationship".to_string())?;
    let edge = class(archforge_diagram::EDGE).and_then(|o| store.uuid(o).ok()).ok_or_else(|| "the sample has no line".to_string())?;
    let run = |session: &mut archforge_notations::Session, params: serde_json::Value| {
        session.execute("diagram.style", &params).map(|_| ()).map_err(|e| format!("diagram.style: {e:?}"))
    };
    run(session, json!({ "ids": [node.to_string()], "fill": "#EBF0FE", "stroke": "#9EAFE7", "line": 1.5 }))?;
    run(session, json!({ "ids": [edge.to_string()], "line": 2 }))?;
    if screen == Screen::WorkspaceAppearanceLine {
        session.execute("view.select", &json!({ "ids": [rel.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.state.show_panel(crate::state::Panel::Appearance);
    if screen == Screen::WorkspaceAppearancePicker {
        let c = egui::Color32::from_rgb(0xC2, 0x5E, 0x0A);
        shell.panels.picker = Some(crate::appearance::Picker::new(crate::appearance::Group::Fill, c));
    }
    Ok(())
}

/// A sequence owned by the sample table's key and a view of its schema; the schema selected.
fn sample_objects(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(key)) = (by_name("Shop"), by_name("customer_id")) else {
        return Err("the sample table".into());
    };
    run(session, "er.sequence.add", json!({ "schema": schema, "name": "customer_id_seq", "start": 1000, "minimum": "1000", "owned_by": key }))?;
    run(
        session,
        "er.view.add",
        json!({ "schema": schema, "name": "recent_customers", "query": "SELECT customer_id, email\nFROM \"Customer\"\nWHERE created_at > now() - interval '30 days'" }),
    )?;
    run(session, "view.select", json!({ "ids": [schema] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// A CRM schema whose Account each Order of the shop belongs to, placed on the shop's diagram.
fn sample_cross_schema(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(order), Some(diagram)) = (by_name("Order"), shell.canvas.active.map(|d| d.to_string())) else {
        return Err("the sample diagram".into());
    };
    let order_node = {
        let store = session.doc();
        let o = store.by_uuid(archforge_mof::Uuid::parse(&order).ok_or("the order's id")?).ok_or("the order")?;
        archforge_diagram::views_of(store, o).2.first().and_then(|n| store.uuid(*n).ok()).map(|u| u.to_string()).ok_or("the order's node")?
    };
    let crm = run(session, "er.schema.add", json!({ "name": "CRM" }))?;
    let account = run(session, "er.entity.add", json!({ "schema": crm, "name": "Account" }))?;
    run(session, "er.attribute.add", json!({ "entity": account, "name": "account_id", "type": "BIGINT", "primary_key": true }))?;
    let owns = run(
        session,
        "er.relationship.add",
        json!({ "from": account, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "owns", "create_foreign_key": true }),
    )?;
    let a = run(
        session,
        "diagram.place",
        json!({ "diagram": diagram, "element": account, "x": 560, "y": 680, "width": 240, "height": crate::editing::content_height(1) }),
    )?;
    let edge = run(session, "diagram.connect", json!({ "diagram": diagram, "element": owns, "source": a, "target": order_node }))?;
    run(session, "diagram.label", json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
    run(session, "view.select", json!({ "ids": [order] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    // The three tables framed on the canvas.
    shell.canvas.pan = egui::vec2(-150.0, -280.0);
    Ok(())
}

/// Selects a class of the sample class model and shows its Properties: Order, or Customer with its
/// operation open in the editor.
fn sample_class_members(shell: &mut Shell, editor: bool) -> Result<(), String> {
    let name = if editor { "Customer" } else { "Order" };
    let (class, op) = {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let c = store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == name)
            .ok_or("the sample has no such class")?;
        let op = archforge_uml::members::operations(store, c).first().and_then(|o| store.uuid(*o).ok());
        (store.uuid(c).map_err(|e| e.to_string())?, op)
    };
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [class.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    if editor && let Some(op) = op {
        crate::uml_panel::open(shell, class, crate::uml_panel::MemberKind::Operation, Some(op));
    }
    Ok(())
}

/// A package, Commerce, with Order, Customer and Payment on its class diagram; Order selected.
fn sample_class_diagram(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Commerce" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Commerce" }))?;
    let mut order = String::new();
    for (name, x, y) in [("Order", 120, 120), ("Customer", 480, 120), ("Payment", 120, 360)] {
        let class = run(session, "uml.class.add", json!({ "package": package, "name": name }))?;
        run(session, "diagram.place", json!({ "diagram": diagram, "element": class, "x": x, "y": y }))?;
        if name == "Order" {
            order = class;
        }
    }
    run(session, "view.select", json!({ "ids": [order] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    crate::canvas::open(shell, diagram);
    Ok(())
}

/// A package, Billing, on its class diagram: ReviewInvoice new with its first operation, Invoice
/// as tall as its members, Order made shorter by hand than its members need, Payment shorter than
/// its header and a row.
fn sample_class_sizing(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Billing" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Billing" }))?;
    let members: [(&str, &[&str], &[&str], f64); 4] = [
        ("ReviewInvoice", &[], &["calculateTotal"], archforge_uml::CLASS_SIZE.1),
        ("Invoice", &["number", "issuedAt", "total"], &["send", "cancel"], crate::uml_ui::members_height(3, 2)),
        ("Order", &["id", "placedAt"], &["submit", "cancel"], 120.0),
        ("Payment", &["amount"], &["authorise"], 60.0),
    ];
    for (i, (name, attributes, operations, height)) in members.into_iter().enumerate() {
        let class = run(session, "uml.class.add", json!({ "package": package, "name": name }))?;
        for a in attributes {
            run(session, "uml.attribute.add", json!({ "class": class, "name": a }))?;
        }
        for o in operations {
            run(session, "uml.operation.add", json!({ "class": class, "name": o }))?;
        }
        let x = 80.0 + 320.0 * i as f64;
        run(
            session,
            "diagram.place",
            json!({ "diagram": diagram, "element": class, "x": x, "y": 120, "width": archforge_uml::CLASS_SIZE.0, "height": height }),
        )?;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    crate::canvas::open(shell, diagram);
    Ok(())
}

/// Invoice above right and Product right of Order, each related to Customer, so the straight or
/// plain right-angled way runs through Order; the diagram's lines go around the tables.
fn sample_routing(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(customer), Some(schema), Some(diagram)) = (by_name("Customer"), by_name("Shop schema"), shell.canvas.active.map(|d| d.to_string()))
    else {
        return Err("the sample diagram".into());
    };
    let customer_node = {
        let store = session.doc();
        let c = store.by_uuid(archforge_mof::Uuid::parse(&customer).ok_or("the customer's id")?).ok_or("the customer")?;
        archforge_diagram::views_of(store, c).2.first().and_then(|n| store.uuid(*n).ok()).map(|u| u.to_string()).ok_or("the customer's node")?
    };
    for (name, rel, x, y) in [("Invoice", "bills", 920, 120), ("Product", "favours", 920, 380)] {
        let table = run(session, "er.entity.add", json!({ "schema": schema, "name": name }))?;
        run(
            session,
            "er.attribute.add",
            json!({ "entity": table, "name": format!("{}_id", name.to_lowercase()), "type": "BIGINT", "primary_key": true }),
        )?;
        let r = run(session, "er.relationship.add", json!({ "from": customer, "to": table, "name": rel, "create_foreign_key": true }))?;
        let n = run(
            session,
            "diagram.place",
            json!({ "diagram": diagram, "element": table, "x": x, "y": y, "width": 240, "height": crate::editing::content_height(2) }),
        )?;
        run(session, "diagram.connect", json!({ "diagram": diagram, "element": r, "source": customer_node, "target": n }))?;
    }
    run(session, "view.select", json!({ "ids": [] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.canvas.zoom = 0.75;
    shell.canvas.pan = egui::vec2(-40.0, 40.0);
    Ok(())
}

/// A schema "Shop schema" with its diagram open: empty, as the journey's first diagram; or with
/// Customer and Order placed, the relationship `places` between them and Customer selected, as
/// the design's ER frame.
/// Export skills on the project and on the sample's schema, the schema selected, and File →
/// Generate… open for it.
fn sample_generate(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let schema = store
        .alive()
        .find(|o| store.class_of(*o).ok() == Some(archforge_er::SCHEMA))
        .and_then(|s| store.uuid(s).ok())
        .ok_or("the sample has no schema")?
        .to_string();
    for params in [
        json!({ "name": "DDL conventions", "body": "Plain CREATE TABLE statements.", "triggers": ["export"] }),
        json!({ "subject": schema, "name": "DDL conventions", "body": "PostgreSQL; a named constraint per key.", "triggers": ["export"], "dialect": "PostgreSQL" }),
        json!({ "subject": schema, "name": "Audit columns", "body": "Every table gets created_at.", "triggers": ["export"] }),
    ] {
        session.execute("skill.add", &params).map_err(|e| format!("skill.add: {e:?}"))?;
    }
    session.execute("view.select", &json!({ "ids": [schema] })).map_err(|e| format!("view.select: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.open_generate();
    Ok(())
}

fn sample_diagram(shell: &mut Shell, filled: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
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
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR", "length": 255, "unique": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR", "length": 120 }))?;
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

/// Party with its subtypes Person and Company on the empty sample diagram, drawn as the design's
/// "Subtypes" card, Party selected.
fn sample_subtypes(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let diagram = shell.canvas.active.and_then(|id| store.by_uuid(id)).ok_or_else(|| "the sample has no open diagram".to_string())?;
    let schema = crate::diagrams::schema_of(store, diagram).ok_or_else(|| "the sample diagram has no schema".to_string())?;
    let (schema, diagram) = (store.uuid(schema).map_err(|e| e.to_string())?.to_string(), store.uuid(diagram).map_err(|e| e.to_string())?.to_string());
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let party = run(session, "er.entity.add", json!({ "schema": schema, "name": "Party" }))?;
    run(session, "er.attribute.add", json!({ "entity": party, "name": "party_id", "type": "BIGINT", "primary_key": true }))?;
    let kind = run(session, "er.attribute.add", json!({ "entity": party, "name": "party_type", "type": "TEXT" }))?;
    let person = run(session, "er.entity.add", json!({ "schema": schema, "name": "Person" }))?;
    let company = run(session, "er.entity.add", json!({ "schema": schema, "name": "Company" }))?;
    let g1 = run(session, "er.generalization.add", json!({ "supertype": party, "subtype": person }))?;
    let g2 = run(session, "er.generalization.add", json!({ "supertype": party, "subtype": company }))?;
    run(session, "er.attribute.add", json!({ "entity": person, "name": "birth_date", "type": "DATE" }))?;
    run(session, "er.attribute.add", json!({ "entity": company, "name": "vat_no", "type": "TEXT" }))?;
    run(
        session,
        "er.entity.set-subtyping",
        json!({ "entity": party, "complete": true, "disjoint": true, "discriminator": kind, "mapping": "table-per-type" }),
    )?;
    let n_party = run(session, "diagram.place", json!({ "diagram": diagram, "element": party, "x": 420, "y": 200, "width": 240, "height": 126 }))?;
    let n_person = run(session, "diagram.place", json!({ "diagram": diagram, "element": person, "x": 200, "y": 460, "width": 240, "height": 126 }))?;
    let n_company =
        run(session, "diagram.place", json!({ "diagram": diagram, "element": company, "x": 640, "y": 460, "width": 240, "height": 126 }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": g1, "source": n_person, "target": n_party }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": g2, "source": n_company, "target": n_party }))?;
    run(session, "view.select", json!({ "ids": [party] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
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
    shell.canvas.tool = crate::canvas::Tool::Place("entity");
    shell.canvas.rename = Some(crate::editing::NodeRename { node, entity, text: "Entity".to_string(), opened: true, refused: None });
    Ok(())
}
