//! ER's window tests of Properties: a table's columns and the column editor, its keys, checks,
//! indexes, types, sequences, views and subtypes, a relationship's ends and foreign key, the
//! Problems ER's rules find, ER's editors brought into view, and the delete dialog's and a
//! version's ER words.

use super::*;

/// Two editors opened before one draw, the first key's above the body and a new check's below it:
/// each reveal counts the scroll the one before it moved, so the check's card, drawn last, ends in
/// view.
#[test]
fn two_editors_opened_before_one_draw_leave_the_last_in_view() {
    in_both_appearances(|theme| {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let schema = id("er.schema.add", serde_json::json!({ "name": "Shop" }));
        let table = id("er.entity.add", serde_json::json!({ "schema": schema, "name": "Orders" }));
        for i in 0..30 {
            let column = id("er.attribute.add", serde_json::json!({ "entity": table, "name": format!("c{i}"), "type": "TEXT" }));
            id("er.unique.add", serde_json::json!({ "entity": table, "columns": [column], "name": format!("u{i}") }));
        }
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [table] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let entity = archforge_mof::Uuid::parse(&table).unwrap();
        let first = {
            let st = h.state().session.as_ref().unwrap().doc();
            er_keys::rows(st, st.by_uuid(entity).unwrap()).remove(0)
        };
        let at = reached_in_properties(&mut h, &format!("workspace.properties.key.row.{}", first.id));
        let above = h.state().panels.body.unwrap().top() - at.y;
        wheel_properties(&mut h, above - 200.0);
        assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.properties.check")), "{theme:?}: Checks under the fold");
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| er_keys::open(cx, entity, Some(&first)));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| er_checks::open(cx, entity, None));
        h.run();
        assert!(in_view(&h, "workspace.properties.check.apply").is_some(), "{theme:?}: the check's Apply in view");
    });
}

/// The delete dialog of a column a key lists, which a foreign key points at, says in the chosen
/// language that the key and the foreign key go with it, in ER's own words, under its question and
/// above what it holds. In light and dark.
#[test]
fn the_delete_dialog_says_the_keys_that_go_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let schema = id("er.schema.add", json!({ "name": "Shop" }));
        let kunde = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
        let bestellung = id("er.entity.add", json!({ "schema": schema, "name": "Bestellung" }));
        let nummer = id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
        let column = id("er.attribute.add", json!({ "entity": bestellung, "name": "kunde_nummer", "type": "BIGINT", "nullable": "no" }));
        id("er.foreign-key.add", json!({ "entity": bestellung, "references": kunde, "columns": [column], "name": "fk_kunde" }));
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", json!({ "ids": [nummer] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: the dialog asks"));
        assert_eq!(asked.consequences, ["1 Schlüssel geht mit.", "1 Fremdschlüssel geht mit."], "{theme:?}");
        for line in ["1 Schlüssel geht mit.", "1 Fremdschlüssel geht mit.", "Was es enthält, geht mit."] {
            assert!(painted_has(&h, line), "{theme:?}: {line} in {:?}", painted(&h));
        }
        h.state_mut().explorer.delete = None;
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The Problems panel names a loaded document's enum type without a name, which no command makes, by
/// its id when a column's default is none of its literals, in the chosen language, never as an empty
/// name. In light and dark.
#[test]
fn an_er_columns_default_none_of_an_unnamed_enum_types_literals_names_the_type_by_its_id() {
    use archforge_er::{ATTRIBUTE, ENTITY, ENUM_TYPE, SCHEMA};
    use archforge_mof::Value;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid::v7(1_761_670_800_000, [0, 0, 0, 0, 0, 0, 0, 0, 0, u8::try_from(n).unwrap_or(u8::MAX)])
        });
        let mut st = archforge_er::store(ids).unwrap();
        let schema = st.create(SCHEMA).unwrap();
        let status = st.create(ENUM_TYPE).unwrap();
        st.push(status, archforge_er::enum_type::LITERAL, Value::str("offen")).unwrap();
        st.push(schema, archforge_er::schema::ENUM_TYPE, Value::Ref(status)).unwrap();
        let bestellung = st.create(ENTITY).unwrap();
        st.set(bestellung, archforge_er::named_element::NAME, Value::str("bestellung")).unwrap();
        st.push(schema, archforge_er::schema::ENTITY, Value::Ref(bestellung)).unwrap();
        let zustand = st.create(ATTRIBUTE).unwrap();
        st.set(zustand, archforge_er::named_element::NAME, Value::str("zustand")).unwrap();
        st.set(zustand, archforge_er::attribute::ENUM_TYPE, Value::Ref(status)).unwrap();
        st.set(zustand, archforge_er::attribute::DEFAULT, Value::str("'zu'")).unwrap();
        st.push(bestellung, archforge_er::entity::ATTRIBUTE, Value::Ref(zustand)).unwrap();
        let id = st.uuid(status).unwrap();
        let texts: Vec<String> = problems_all(SCREENS.notation_uis(), &st).into_iter().map(|p| p.text).collect();
        let zustand: Vec<&String> = texts.iter().filter(|t| t.starts_with("zustand")).collect();
        assert_eq!(zustand, [&format!("zustand: 'zu' ist kein Literal von {id}.")], "{texts:?} {theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The Problems panel says a loaded document's typed rule problems in the chosen language: a key
/// without columns or with one that may be null in a primary key, a column's default none of its enum type's literals, a check without an
/// expression and a foreign key without columns; a foreign key without its key keeps its own row.
/// In light and dark (AR-532).
#[test]
fn an_er_documents_typed_problems_are_said_in_the_chosen_language() {
    use archforge_er::{ATTRIBUTE, CHECK_CONSTRAINT, ENTITY, ENUM_TYPE, FOREIGN_KEY, PRIMARY_KEY, SCHEMA, UNIQUE_CONSTRAINT};
    use archforge_mof::Value;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        // A document no command makes, each table in the schema, so the foreign key's rules apply.
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid::v7(1_761_670_800_000, [0, 0, 0, 0, 0, 0, 0, 0, 0, u8::try_from(n).unwrap_or(u8::MAX)])
        });
        let mut st = archforge_er::store(ids).unwrap();
        let schema = st.create(SCHEMA).unwrap();
        let table = |st: &mut archforge_mof::Store, name: &str, nullable: bool| {
            let e = st.create(ENTITY).unwrap();
            st.set(e, archforge_er::named_element::NAME, Value::str(name)).unwrap();
            st.push(schema, archforge_er::schema::ENTITY, Value::Ref(e)).unwrap();
            let id = st.create(ATTRIBUTE).unwrap();
            st.set(id, archforge_er::named_element::NAME, Value::str("id")).unwrap();
            st.set(id, archforge_er::attribute::TYPE, Value::str("INT")).unwrap();
            st.set(id, archforge_er::attribute::NULLABLE, Value::Bool(nullable)).unwrap();
            st.push(e, archforge_er::entity::ATTRIBUTE, Value::Ref(id)).unwrap();
            let pk = st.create(PRIMARY_KEY).unwrap();
            st.set(pk, archforge_er::key::NAME, Value::str(&format!("pk_{name}"))).unwrap();
            st.push(pk, archforge_er::key::COLUMN, Value::Ref(id)).unwrap();
            st.set(e, archforge_er::entity::PRIMARY_KEY, Value::Ref(pk)).unwrap();
            e
        };
        let kunde = table(&mut st, "kunde", false);
        let bestellung = table(&mut st, "bestellung", true);
        let leer = st.create(UNIQUE_CONSTRAINT).unwrap();
        st.set(leer, archforge_er::key::NAME, Value::str("uq_leer")).unwrap();
        st.push(bestellung, archforge_er::entity::UNIQUE_CONSTRAINT, Value::Ref(leer)).unwrap();
        let status = st.create(ENUM_TYPE).unwrap();
        st.set(status, archforge_er::named_element::NAME, Value::str("status")).unwrap();
        st.push(status, archforge_er::enum_type::LITERAL, Value::str("offen")).unwrap();
        st.push(schema, archforge_er::schema::ENUM_TYPE, Value::Ref(status)).unwrap();
        let zustand = st.create(ATTRIBUTE).unwrap();
        st.set(zustand, archforge_er::named_element::NAME, Value::str("zustand")).unwrap();
        st.set(zustand, archforge_er::attribute::ENUM_TYPE, Value::Ref(status)).unwrap();
        st.set(zustand, archforge_er::attribute::DEFAULT, Value::str("'zu'")).unwrap();
        st.push(bestellung, archforge_er::entity::ATTRIBUTE, Value::Ref(zustand)).unwrap();
        let check = st.create(CHECK_CONSTRAINT).unwrap();
        st.set(check, archforge_er::check_constraint::NAME, Value::str("ck_leer")).unwrap();
        st.push(bestellung, archforge_er::entity::CHECK_CONSTRAINT, Value::Ref(check)).unwrap();
        let pk_kunde = archforge_er::rules::primary_key_of(&st, kunde).unwrap();
        for (name, key) in [("fk_kunde", Some(pk_kunde)), ("fk_offen", None)] {
            let fk = st.create(FOREIGN_KEY).unwrap();
            st.set(fk, archforge_er::foreign_key::NAME, Value::str(name)).unwrap();
            if let Some(key) = key {
                st.set(fk, archforge_er::foreign_key::REFERENCES, Value::Ref(key)).unwrap();
            }
            st.push(bestellung, archforge_er::entity::FOREIGN_KEY, Value::Ref(fk)).unwrap();
        }
        let mut texts: Vec<String> = problems_all(SCREENS.notation_uis(), &st).into_iter().map(|p| p.text).collect();
        texts.sort();
        assert_eq!(
            texts,
            [
                "ck_leer: Geben Sie einen Ausdruck ein. Eine Einschränkung ohne Ausdruck legt keine Regel fest.",
                "fk_kunde: Ein Fremdschlüssel braucht mindestens eine Spalte.",
                "fk_offen: hat keine Tabelle oder keinen Schlüssel",
                "id ist eine Schlüsselspalte und darf leer sein",
                "pk_bestellung: id darf leer sein, was eine Primärschlüsselspalte nie darf.",
                "uq_leer: Ein Schlüssel braucht mindestens eine Spalte.",
                "zustand: 'zu' ist kein Literal von status.",
            ],
            "{theme:?}"
        );
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// Properties lists a table's columns, and the column editor changes them through the engine.
#[test]
fn properties_lists_a_tables_columns_and_the_editor_changes_them_through_the_engine() {
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    let rows: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).cloned().collect();
    assert_eq!(rows.len(), 2, "two column rows: {found:?}");
    assert!(found.iter().any(|i| i == "workspace.properties.kind"));
    // Open the second column's editor and make it unique and not null.
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_some(), "the editor opened");
    let at = rect_of(&h, "workspace.properties.column.unique").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.not-null").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    let email_obj = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
    assert!(archforge_er::rules::is_unique(st, email_obj));
    assert!(!archforge_er::rules::is_nullable(st, email_obj));
    assert!(s.can_undo());
    // The + adds a column through the editor.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.target.is_none() && c.name == "column_1"));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3);
    // The kind select makes a mapping table.
    let at = rect_of(&h, "workspace.properties.kind").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::KIND));
    let at = rect_of(&h, "workspace.properties.kind.1").center();
    click(&mut h, at);
    // A mapping table needs its foreign keys: the engine refuses, and the panel says so.
    assert!(h.state().panels.menu.is_none());
    assert!(h.state().panels.notice.is_some(), "the refusal is shown");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(customer_obj, archforge_er::entity::KIND).unwrap(), archforge_mof::Value::Enum(archforge_er::EntityKind::Regular.index()));
}

/// A column's editor refused by the engine says why in plain words under the card, never with the
/// command's id or the parameter it is about: a column renamed, or a new one added. In light and
/// dark (AR-428).
#[test]
fn a_column_refused_in_properties_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for new in [false, true] {
            let (mut h, _customer) = table_harness();
            h.state_mut().state.theme = theme;
            h.run();
            let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
            let opener = if new { "workspace.properties.columns.add".to_string() } else { format!("{}.edit", rows[1]) };
            let at = rect_of(&h, &opener).center();
            click(&mut h, at);
            er_of_mut(h.state_mut()).column.as_mut().unwrap_or_else(|| panic!("{theme:?}: the editor opened")).name = "customer_id".into();
            h.run();
            let at = rect_of(&h, "workspace.properties.column.apply").center();
            click(&mut h, at);
            let refused = er_of(h.state()).column.as_ref().and_then(|c| c.refused.clone()).unwrap_or_else(|| panic!("{theme:?}: refused"));
            let plain = refused.starts_with("There is a column named") && !refused.contains("er.") && !refused.contains("name:");
            assert!(plain, "{theme:?}, new {new}: {refused}");
        }
    }
}

/// Apply closes a new column's editor once a type is picked from the type list.
#[test]
fn apply_closes_a_new_columns_editor_after_a_type_is_picked_from_the_list() {
    let (mut h, customer) = table_harness();
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.name").center();
    click(&mut h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("id".into()));
    h.run();
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu.2").center();
    click(&mut h, at);
    assert!(h.state().panels.menu.is_none(), "a pick closes the list");
    // The list lies over the sections below the card; its last entry takes the click, not the
    // section row under it.
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let n = er_table::BASE_TYPES.len() - 1;
    let last = rect_of(&h, &format!("workspace.properties.column.type.menu.{n}"));
    let keys_add = rect_of(&h, "workspace.properties.keys.add");
    assert!(last.bottom() > keys_add.top(), "the list reaches over the keys: {last:?} {keys_add:?}");
    click(&mut h, egui::pos2(keys_add.center().x.min(last.right() - 4.0), last.center().y));
    assert_eq!(er_of(h.state()).column.as_ref().map(|c| c.ty.clone()), Some(er_table::BASE_TYPES[n].to_string()));
    assert!(er_of(h.state()).key.is_none(), "the key editor under the list stayed closed");
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu.2").center();
    click(&mut h, at);
    // Primary key brings Not null with it.
    let at = rect_of(&h, "workspace.properties.column.primary-key").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.primary_key && c.not_null));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    h.run();
    assert!(er_of(h.state()).column.is_none(), "one Apply closes the editor: {:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3, "one column added");
    // Not null cannot be unticked under a key: it takes the key with it.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[0])).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.not-null").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| !c.primary_key && !c.not_null));
}

/// A column's row in Properties selects the column; only its edit button opens the editor, and
/// selecting elsewhere closes it.
#[test]
fn a_column_row_selects_its_column_only_its_edit_button_opens_the_editor_and_selecting_elsewhere_closes_it() {
    let (mut h, customer) = table_harness();
    let select = |h: &mut Harness<'static, Shell>, id: &str| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
    };
    let email = {
        let st = h.state().session.as_ref().unwrap().doc();
        let email = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
        st.uuid(email).unwrap().to_string()
    };
    select(&mut h, &email);
    h.run();
    assert!(er_of(h.state()).column.is_none(), "selecting a column only marks its row: {:?}", er_of(h.state()).column);
    // A click on a row selects its column, as the tree and the canvas do, and opens its table in
    // the tree so the row there shows too; it does not open the editor.
    let customer_id = archforge_mof::Uuid::parse(&customer).unwrap();
    select(&mut h, &customer);
    h.state_mut().explorer.expanded.remove(&customer_id);
    h.run();
    let at = rect_of(&h, &format!("workspace.properties.column.{email}")).center();
    click(&mut h, at);
    h.run();
    assert_eq!(selected_name(&h), "email", "the row's click selects its column");
    assert!(h.state().explorer.expanded.contains(&customer_id), "its table opens in the tree");
    assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{email}")), "{:?}", ids(&h));
    assert!(er_of(h.state()).column.is_none(), "a click on the row does not open it");
    let at = rect_of(&h, &format!("workspace.properties.column.{email}.edit")).center();
    click(&mut h, at);
    h.run();
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email.clone())), "the edit button opens it");
    // The edit button of a row while the table is selected selects the column too.
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    select(&mut h, &customer);
    h.run();
    let at = rect_of(&h, &format!("workspace.properties.column.{email}.edit")).center();
    click(&mut h, at);
    h.run();
    assert_eq!(selected_name(&h), "email");
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email.clone())), "and opens it");
    // Selecting the table, then the column again: the editor closed and stays closed.
    select(&mut h, &customer);
    h.run();
    assert!(er_of(h.state()).column.is_none(), "selecting elsewhere closes it");
    select(&mut h, &email);
    h.run();
    assert!(er_of(h.state()).column.is_none(), "coming back does not reopen it");
    // The + opens an editor for a new column; Cancel closes it and it stays closed.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    h.run();
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.target.is_none()), "the + opens an editor for a new column");
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    h.run();
    h.run();
    assert!(er_of(h.state()).column.is_none(), "Cancel stays closed: {:?}", er_of(h.state()).column);
}

/// A relationship's Properties set its ends, roles, Identifying and the foreign key's actions.
#[test]
fn relationship_properties_set_the_ends_roles_identifying_and_the_foreign_key_actions() {
    let (mut h, rel) = relationship_harness();
    let found = ids(&h);
    for id in [
        "workspace.properties.identifying",
        "workspace.properties.end.from.0",
        "workspace.properties.end.to.3",
        "workspace.properties.role.from",
        "workspace.properties.foreign-key",
        "workspace.properties.points-to",
        "workspace.properties.pair.0",
        "workspace.properties.on-delete",
        "workspace.properties.more",
    ] {
        assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
    }
    // The to end becomes one-or-more.
    let at = rect_of(&h, "workspace.properties.end.to.3").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::cardinality_of(st, rel, archforge_er::relationship::TO_END), Some(archforge_er::Cardinality::OneOrMore));
    // Identifying: the foreign key's columns join the child's primary key.
    let at = rect_of(&h, "workspace.properties.identifying").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(matches!(st.get(rel, archforge_er::relationship::IDENTIFYING), Ok(archforge_mof::Value::Bool(true))), "{:?}", h.state().panels.notice);
    // A role typed and committed with Enter.
    let at = rect_of(&h, "workspace.properties.role.from").center();
    click(&mut h, at);
    h.event(egui::Event::Text("buyer".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(rel, archforge_er::relationship::FROM_ROLE).unwrap().as_str(), Some("buyer"));
    // On delete: Cascade.
    let at = rect_of(&h, "workspace.properties.on-delete").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.on-delete.2").center();
    click(&mut h, at);
    let fk = fk_of(&h, rel).expect("the foreign key");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(
        st.get(fk, archforge_er::foreign_key::ON_DELETE).unwrap(),
        archforge_mof::Value::Enum(archforge_er::ReferentialAction::Cascade.index())
    );
    // More opens Deferrable; checked at commit.
    let at = rect_of(&h, "workspace.properties.more").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.deferrable").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.deferrable.2").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(
        st.get(fk, archforge_er::foreign_key::DEFERRABLE).unwrap(),
        archforge_mof::Value::Enum(archforge_er::Deferrability::InitiallyDeferred.index())
    );
}

/// The Foreign key switch removes the key with its columns, and adds them back, each as one step.
#[test]
fn the_foreign_key_switch_removes_and_adds_the_key_with_its_columns_as_one_step() {
    let (mut h, rel) = relationship_harness();
    let columns_of_order = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let order = st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap();
        st.list(order, archforge_er::entity::ATTRIBUTE).unwrap().len()
    };
    let before = columns_of_order(&h);
    let fk = fk_of(&h, rel).expect("the foreign key");
    let key_columns = h.state().session.as_ref().unwrap().doc().list(fk, archforge_er::foreign_key::COLUMN).unwrap().len();
    assert!(key_columns >= 1);
    // Off: the key and its columns go together.
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(fk_of(&h, rel).is_none(), "{:?}", h.state().panels.notice);
    assert_eq!(columns_of_order(&h), before - key_columns);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Remove foreign key"));
    // On again: one Order column per Customer key column, and the key.
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(fk_of(&h, rel).is_some(), "{:?}", h.state().panels.notice);
    assert_eq!(columns_of_order(&h), before);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add foreign key"));
    // A pair's child column can be an existing column of the same type.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "buyer_ref", "type": "BIGINT" })).unwrap();
        // Adding selected the column; the panel shows the relationship again.
        let rel_id = s.doc().uuid(rel).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [rel_id] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.pair.0").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::PAIR.at(0)));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.pair.0.")).collect();
    assert!(items.len() >= 2, "{items:?}");
    // The last item is buyer_ref (same type as customer_id).
    let at = rect_of(&h, items.last().unwrap()).center();
    click(&mut h, at);
    let fk = fk_of(&h, rel).unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let cols: Vec<String> = st
        .list(fk, archforge_er::foreign_key::COLUMN)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .map(|c| archforge_core::name_of(st, c))
        .collect();
    assert!(cols.contains(&"buyer_ref".to_string()), "{cols:?} notice {:?}", h.state().panels.notice);
}

/// A relationship whose parent has no key shows the card for it, and Add id column makes the key.
#[test]
fn a_parent_without_a_key_gets_the_card_and_add_id_column_makes_the_key() {
    let (mut h, _customer, _order) = two_tables_harness();
    // Payment without any column, joined from Order as the parent-less side.
    let (payment_rel, payment) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
        let payment = s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Payment" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let rel = s.execute("er.relationship.add", &serde_json::json!({ "from": payment, "to": order, "name": "settles" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [rel] })).unwrap();
        (rel, payment)
    };
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.foreign-key"));
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.add-id"), "the card offers Add id column: {:?}", ids(&h));
    let at = rect_of(&h, "workspace.properties.add-id").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    let payment_obj = st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap();
    assert!(archforge_er::rules::primary_key_of(st, payment_obj).is_some(), "Payment has its id key: {:?}", h.state().panels.notice);
    let rel_obj = st.by_uuid(archforge_mof::Uuid::parse(&payment_rel).unwrap()).unwrap();
    assert!(st.referrers(rel_obj).into_iter().any(|(_, p)| p == archforge_er::foreign_key::RELATIONSHIP), "the foreign key exists");
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add id column"));
}

/// The key editor adds a unique constraint and removes the primary key.
#[test]
fn the_key_editor_adds_a_unique_constraint_and_removes_the_primary_key() {
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    assert!(found.iter().any(|i| i == "workspace.properties.keys.add"), "{found:?}");
    let key_rows: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.properties.key.row.") && !i.ends_with(".edit")).cloned().collect();
    assert_eq!(key_rows.len(), 1, "the primary key: {key_rows:?}");
    // + opens the editor for a new unique constraint; a column is added from the menu; Apply.
    let at = rect_of(&h, "workspace.properties.keys.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).key.as_ref().is_some_and(|k| k.target.is_none() && !k.primary));
    let at = rect_of(&h, "workspace.properties.key.column.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::KEY_COLUMN));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.key.column.add.")).collect();
    assert!(!items.is_empty());
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).key.as_ref().unwrap().columns.len(), 1);
    let at = rect_of(&h, "workspace.properties.key.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).key.is_none(), "{:?}", er_of(h.state()).key);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_er::rules::unique_constraints(st, customer_obj).len(), 1, "the unique constraint was added");
    assert_eq!(selected_name(&h), "Customer", "the table stays selected");
    // The primary key's row opens its editor; the bin removes the key, and the table gains a problem.
    let pk_row = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.key.row.") && !i.ends_with(".edit")).unwrap();
    let at = rect_of(&h, &format!("{pk_row}.edit")).center();
    click(&mut h, at);
    assert!(er_of(h.state()).key.as_ref().is_some_and(|k| k.primary));
    let at = rect_of(&h, "workspace.properties.key.remove").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(archforge_er::rules::primary_key_of(st, customer_obj).is_none(), "{:?}", h.state().panels.notice);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.problem.0"), "Problems lists the missing key: {:?}", ids(&h));
    let problems = archforge_shell_api::problems::of(SCREENS.notation_uis(), st, &problems_by_element(SCREENS.notation_uis(), st), customer_obj);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].text, "Customer has no primary key");
}

/// The column editor sets a column's size, default and generation, and the heading takes the table's
/// description.
#[test]
fn the_column_editor_sets_the_size_default_and_generation_and_the_heading_takes_a_description() {
    let (mut h, customer) = table_harness();
    // email becomes VARCHAR with a length of 255 and a default; the row shows VARCHAR(255).
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    er_of_mut(h.state_mut()).column.as_mut().unwrap().ty = "VARCHAR".into();
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.length"), "a varchar takes a length: {:?}", ids(&h));
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.column.scale"));
    {
        let c = er_of_mut(h.state_mut()).column.as_mut().unwrap();
        c.length = "255".into();
        c.default = "''".into();
    }
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    let email = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
    assert_eq!(archforge_er::rules::type_display(st, email), "VARCHAR(255)");
    assert_eq!(archforge_er::rules::text_of(st, email, archforge_er::attribute::DEFAULT), "''");
    // A numeric takes precision and scale; the scale above the precision is refused and the editor stays open.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    {
        let c = er_of_mut(h.state_mut()).column.as_mut().unwrap();
        c.ty = "NUMERIC".into();
        c.length.clear();
        c.precision = "4".into();
        c.scale = "6".into();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.scale"));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.refused.is_some()), "the refusal stays in the editor");
    er_of_mut(h.state_mut()).column.as_mut().unwrap().scale = "2".into();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::type_display(st, email), "NUMERIC(4, 2)");
    // Generation: computed with an expression through the select and the expression field.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    er_of_mut(h.state_mut()).column.as_mut().unwrap().default.clear();
    let at = rect_of(&h, "workspace.properties.column.generation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.generation.3").center();
    click(&mut h, at);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.expression"), "a computed column takes an expression");
    er_of_mut(h.state_mut()).column.as_mut().unwrap().expression = "lower(name)".into();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::generation_of(st, email), archforge_er::Generation::ComputedStored);
    assert_eq!(archforge_er::rules::text_of(st, email, archforge_er::attribute::EXPRESSION), "lower(name)");
    // The heading's description field describes the table.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.description"));
    let at = rect_of(&h, "workspace.properties.description").center();
    click(&mut h, at);
    h.event(egui::Event::Text("People who buy.".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_er::rules::text_of(st, customer_obj, archforge_er::element::DESCRIPTION), "People who buy.");
}

/// A computed or an identity column made a plain one with a default in one Apply of the column
/// editor lands: the generation is given up before the default is set, and the computed column's
/// expression, hidden once None is chosen, goes with it, for a saved plain column taken to
/// Computed and back in its editor and for a column being added too. A default
/// on a column that stays computed is still refused by its rule, and a new expression alone lands.
/// In light and dark.
#[test]
fn the_column_editor_makes_a_computed_or_identity_column_plain_with_a_default() {
    use archforge_er::{Generation, attribute, rules};
    let computed = ("er.attribute.set-generation", serde_json::json!({ "generation": "computed-stored", "expression": "lower(name)" }));
    let integer = ("er.attribute.set-type", serde_json::json!({ "type": "BIGINT" }));
    let not_null = ("er.attribute.set-flags", serde_json::json!({ "primary_key": false, "nullable": "no", "unique": false }));
    let identity = ("er.attribute.set-generation", serde_json::json!({ "generation": "identity-always" }));
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        // Computed, stored to None with a default.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, std::slice::from_ref(&computed), 0, |c| c.default = "'none'".into());
        assert_eq!(refused, None, "computed to plain with a default lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::text_of(st, email, attribute::DEFAULT), "'none'");
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "", "the hidden expression went with the generation");
        // Computed, stored to None and nothing else.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, std::slice::from_ref(&computed), 0, |_| {});
        assert_eq!(refused, None, "computed to plain lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "");
        // Identity, always to None with a default.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, &[integer.clone(), not_null.clone(), identity.clone()], 0, |c| c.default = "0".into());
        assert_eq!(refused, None, "identity to plain with a default lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::text_of(st, email, attribute::DEFAULT), "0");
        // A default on a column that stays computed is still refused, and nothing changes.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, std::slice::from_ref(&computed), 3, |c| c.default = "'x'".into());
        let said = archforge_shell_api::panels::said(
            SCREENS.notation_uis(),
            &archforge_engine::Reason::Generation(archforge_engine::GenerationRule::ComputedTakesNoDefault),
        );
        assert_eq!(refused, said, "a computed column takes no default");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::ComputedStored);
        assert_eq!(rules::text_of(st, email, attribute::DEFAULT), "");
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "lower(name)");
        // A column that stays computed takes a new expression alone.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, std::slice::from_ref(&computed), 3, |c| c.expression = "upper(name)".into());
        assert_eq!(refused, None, "a new expression lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::ComputedStored);
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "upper(name)");
        // A saved plain column made computed, given an expression and made plain again in its
        // editor stays plain without an expression: the generation never changed in the model.
        let (mut h, _) = table_harness_in(theme);
        let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
        let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
        click(&mut h, at);
        for generation in [3, 0] {
            let at = rect_of(&h, "workspace.properties.column.generation").center();
            click(&mut h, at);
            let at = rect_of(&h, &format!("workspace.properties.column.generation.{generation}")).center();
            click(&mut h, at);
            if generation == 3 {
                er_of_mut(h.state_mut()).column.as_mut().unwrap().expression = "lower(name)".into();
            }
        }
        let at = rect_of(&h, "workspace.properties.column.apply").center();
        click(&mut h, at);
        assert!(er_of(h.state()).column.is_none(), "the plain column's Apply lands: {:?}", er_of(h.state()).column);
        let st = h.state().session.as_ref().unwrap().doc();
        let email = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "");
        // A column being added that was made computed, given an expression and made plain again
        // is added plain, without the expression its hidden field still holds.
        let (mut h, customer) = table_harness_in(theme);
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| er_table::open_column(cx, archforge_mof::Uuid::parse(&customer).unwrap(), None));
        h.run();
        er_of_mut(h.state_mut()).column.as_mut().unwrap().name = "nickname".into();
        er_of_mut(h.state_mut()).column.as_mut().unwrap().ty = "TEXT".into();
        for generation in [3, 0] {
            let at = rect_of(&h, "workspace.properties.column.generation").center();
            click(&mut h, at);
            let at = rect_of(&h, &format!("workspace.properties.column.generation.{generation}")).center();
            click(&mut h, at);
            if generation == 3 {
                er_of_mut(h.state_mut()).column.as_mut().unwrap().expression = "lower(name)".into();
            }
        }
        let at = rect_of(&h, "workspace.properties.column.apply").center();
        click(&mut h, at);
        assert!(er_of(h.state()).column.is_none(), "the new column is added: {:?}", er_of(h.state()).column);
        let st = h.state().session.as_ref().unwrap().doc();
        let nickname = st.alive().find(|o| archforge_core::name_of(st, *o) == "nickname").unwrap();
        assert_eq!(rules::generation_of(st, nickname), Generation::NoneKind);
        assert_eq!(rules::text_of(st, nickname, attribute::EXPRESSION), "");
    }
}

/// A column's type, nullability and generation changed together in one Apply of the column editor
/// land whichever generation it leaves and takes: the old generation is given up first and the new
/// one taken last, so no step meets a rule only the other generation has. An identity left
/// nullable is still refused, a refusal after the old generation was given up rolls the whole
/// Apply back, and the Apply that gives one generation up for another is one undo step. In light
/// and dark.
#[test]
fn the_column_editor_changes_type_nullability_and_generation_in_one_apply() {
    use archforge_er::{Generation, attribute, rules};
    let integer = ("er.attribute.set-type", serde_json::json!({ "type": "BIGINT" }));
    let not_null = ("er.attribute.set-flags", serde_json::json!({ "primary_key": false, "nullable": "no", "unique": false }));
    let identity = ("er.attribute.set-generation", serde_json::json!({ "generation": "identity-always" }));
    let computed = ("er.attribute.set-generation", serde_json::json!({ "generation": "computed-stored", "expression": "lower(name)" }));
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        // None (TEXT, nullable) to Identity, always with BIGINT and Not null.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, &[], 1, |c| {
            c.ty = "BIGINT".into();
            c.not_null = true;
        });
        assert_eq!(refused, None, "plain to identity with its type and Not null lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::IdentityAlways);
        assert!(!rules::is_nullable(st, email));
        // Identity, always (BIGINT, not null) to Computed, stored with TEXT, nullable.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, &[integer.clone(), not_null.clone(), identity.clone()], 3, |c| {
            c.ty = "TEXT".into();
            c.not_null = false;
            c.expression = "lower(name)".into();
        });
        assert_eq!(refused, None, "identity to computed with another type and nullable lands");
        let computed_state = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            (
                rules::generation_of(st, email),
                rules::type_of(st, email),
                rules::is_nullable(st, email),
                rules::text_of(st, email, attribute::EXPRESSION),
            )
        };
        let after = (Generation::ComputedStored, "TEXT".to_string(), true, "lower(name)".to_string());
        assert_eq!(computed_state(&h), after);
        // The Apply is one step: one Undo brings the identity column back whole, one Redo the
        // computed one.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(computed_state(&h), (Generation::IdentityAlways, "BIGINT".to_string(), false, String::new()), "one undo");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert_eq!(computed_state(&h), after, "one redo");
        // Identity, always (BIGINT, not null) to None with TEXT, nullable.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, &[integer.clone(), not_null.clone(), identity.clone()], 0, |c| {
            c.ty = "TEXT".into();
            c.not_null = false;
        });
        assert_eq!(refused, None, "identity to plain with another type and nullable lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::type_of(st, email), "TEXT");
        assert!(rules::is_nullable(st, email));
        // Computed, stored (TEXT) to Identity, by default with BIGINT and Not null.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, std::slice::from_ref(&computed), 2, |c| {
            c.ty = "BIGINT".into();
            c.not_null = true;
        });
        assert_eq!(refused, None, "computed to identity with its type and Not null lands");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::IdentityByDefault);
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "");
        // Identity left nullable is still refused, and the column keeps what it had.
        let (mut h, _) = table_harness_in(theme);
        let (email, refused) = apply_email_edit(&mut h, &[], 1, |c| c.ty = "BIGINT".into());
        let said = archforge_shell_api::panels::said(
            SCREENS.notation_uis(),
            &archforge_engine::Reason::Generation(archforge_engine::GenerationRule::IdentityNotNullable),
        );
        assert_eq!(refused, said, "an identity column cannot be nullable");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::NoneKind);
        assert_eq!(rules::type_of(st, email), "TEXT");
        // A refusal after the old generation was given up rolls the whole Apply back: a computed
        // column made Identity with BIGINT but left nullable keeps its type, generation and
        // expression, and neither a step nor a change is added.
        let (mut h, _) = table_harness_in(theme);
        give_email(&mut h, std::slice::from_ref(&computed));
        let session = h.state().session.as_ref().unwrap();
        let before = (session.undo_label().map(str::to_string), session.revision());
        let (email, refused) = apply_email_edit(&mut h, &[], 1, |c| c.ty = "BIGINT".into());
        let said = archforge_shell_api::panels::said(
            SCREENS.notation_uis(),
            &archforge_engine::Reason::Generation(archforge_engine::GenerationRule::IdentityNotNullable),
        );
        assert_eq!(refused, said, "an identity column cannot be nullable");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(rules::generation_of(st, email), Generation::ComputedStored);
        assert_eq!(rules::type_of(st, email), "TEXT");
        assert_eq!(rules::text_of(st, email, attribute::EXPRESSION), "lower(name)");
        let session = h.state().session.as_ref().unwrap();
        assert_eq!((session.undo_label().map(str::to_string), session.revision()), before, "nothing landed");
    }
}

/// The check and index editors write through the engine, and a refusal changes nothing.
#[test]
fn the_check_and_index_editors_write_through_the_engine_and_a_refusal_changes_nothing() {
    use archforge_er::{NullsOrder, SortDirection};
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    assert!(
        found.iter().any(|i| i == "workspace.properties.checks.add") && found.iter().any(|i| i == "workspace.properties.indexes.add"),
        "{found:?}"
    );
    // + opens the check editor; Apply without an expression is refused and nothing lands; with
    // one, the check is added and the table stays selected.
    let at = rect_of(&h, "workspace.properties.checks.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).check.as_ref().is_some_and(|c| c.target.is_none()));
    let at = rect_of(&h, "workspace.properties.check.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).check.as_ref().is_some_and(|c| c.refused.is_some()), "{:?}", er_of(h.state()).check);
    let customer_obj = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap()
    };
    assert!(archforge_er::rules::checks_of(h.state().session.as_ref().unwrap().doc(), customer_obj).is_empty());
    er_of_mut(h.state_mut()).check.as_mut().unwrap().expression = "email <> ''".into();
    let at = rect_of(&h, "workspace.properties.check.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).check.is_none(), "{:?}", er_of(h.state()).check);
    assert_eq!(archforge_er::rules::checks_of(h.state().session.as_ref().unwrap().doc(), customer_obj).len(), 1);
    assert_eq!(ids(&h).iter().filter(|i| i.starts_with("workspace.properties.check.row.") && !i.ends_with(".edit")).count(), 1);
    assert_eq!(selected_name(&h), "Customer");
    // + opens the index editor; two columns come from the menu, the second turns descending, the
    // first takes Nulls last from its right-click menu; Apply writes the index in one command.
    let at = rect_of(&h, "workspace.properties.indexes.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).index.as_ref().is_some_and(|i| i.target.is_none()));
    let at = rect_of(&h, "workspace.properties.index.column.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::INDEX_COLUMN));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.index.column.add.")).collect();
    assert_eq!(items.len(), 3, "two columns and an expression: {items:?}");
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.index.column.add").center();
    click(&mut h, at);
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.index.column.add.")).collect();
    assert_eq!(items.len(), 2, "the column left and an expression: {items:?}");
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).index.as_ref().unwrap().columns.len(), 2);
    let at = rect_of(&h, "workspace.properties.index.column.1.direction").center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).index.as_ref().unwrap().columns[1].direction, SortDirection::Descending);
    let at = rect_of(&h, "workspace.properties.index.column.0").center();
    right_click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::INDEX_NULLS.at(0)));
    let at = rect_of(&h, "workspace.properties.index.column.0.nulls.2").center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).index.as_ref().unwrap().columns[0].nulls, NullsOrder::Last);
    let at = rect_of(&h, "workspace.properties.index.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).index.is_none(), "{:?}", er_of(h.state()).index);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        let indexes = archforge_er::rules::indexes_of(st, customer_obj);
        assert_eq!(indexes.len(), 1);
        let specs = archforge_er::rules::index_specs(st, indexes[0]);
        assert_eq!((specs.len(), specs[0].nulls, specs[1].direction), (2, NullsOrder::Last, SortDirection::Descending));
        assert_eq!(archforge_er::rules::display_name(st, indexes[0]), "ix_customer_customer_id_email");
    }
    assert_eq!(selected_name(&h), "Customer");
    // The index row's edit button opens its editor; the bin removes the index.
    let row = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.index.row.") && !i.ends_with(".edit")).unwrap();
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert!(er_of(h.state()).index.as_ref().is_some_and(|i| i.target.is_some()));
    let at = rect_of(&h, "workspace.properties.index.remove").center();
    click(&mut h, at);
    assert!(er_of(h.state()).index.is_none());
    assert!(archforge_er::rules::indexes_of(h.state().session.as_ref().unwrap().doc(), customer_obj).is_empty());
}

/// Key columns follow the column they point at until unlinked, and a pure mapping table collapses
/// into a relationship.
#[test]
fn key_columns_follow_their_target_until_unlinked_and_a_pure_mapping_table_collapses() {
    let (mut h, customer) = table_harness();
    let (order, rel, mapping) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Order" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let rel = id(s
            .execute(
                "er.relationship.add",
                &serde_json::json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
            )
            .unwrap()
            .value);
        let tag = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Tag" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true })).unwrap();
        let line = id(s
            .execute("er.relationship.add", &serde_json::json!({ "from": order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }))
            .unwrap()
            .value);
        let mapping = id(s.execute("er.relationship.resolve", &serde_json::json!({ "relationship": line })).unwrap().value);
        s.execute("view.select", &serde_json::json!({ "ids": [order] })).unwrap();
        (order, rel, mapping)
    };
    h.run();
    // Order's key column follows Customer.customer_id: the row carries the link mark, the editor
    // names the target and Unlink frees it.
    let marks: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.") && i.ends_with(".link")).collect();
    assert_eq!(marks.len(), 1, "{marks:?}");
    let row = marks[0].trim_end_matches(".link").to_string();
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).column.as_ref().and_then(|c| c.linked.as_deref()), Some("Customer.customer_id"));
    let at = rect_of(&h, "workspace.properties.column.unlink").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.linked.is_none() && c.unlink), "{:?}", er_of(h.state()).column);
    // Cancel keeps the link; Unlink then Apply is one step that drops it.
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    assert_eq!(ids(&h).iter().filter(|i| i.ends_with(".link")).count(), 1, "Cancel kept the link");
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.unlink").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    assert!(!ids(&h).iter().any(|i| i.ends_with(".link")), "the mark is gone");
    // The relationship's parent end set to zero-or-one makes the key column nullable.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [rel] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.end.from.0").center();
    click(&mut h, at);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        let order_obj = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        let column = st
            .list(order_obj, archforge_er::entity::ATTRIBUTE)
            .unwrap()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .find(|a| archforge_core::name_of(st, *a) == "customer_customer_id")
            .unwrap();
        assert!(archforge_er::rules::is_nullable(st, column), "zero-or-one made the key column nullable");
    }
    // The mapping table's Properties offer to collapse it; the click brings the line back.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [mapping] })).unwrap();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.collapse"), "{:?}", ids(&h));
    let at = rect_of(&h, "workspace.properties.collapse").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(archforge_mof::Uuid::parse(&mapping).unwrap()).is_none_or(|m| !st.is_alive(m)), "{:?}", h.state().panels.notice);
    let relationships = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).count();
    assert_eq!(relationships, 2, "Customer to Order and the many-to-many line");
}

/// A schema's Properties list its domains and enum types, and a column takes an enum type from the
/// column editor's type menu.
#[test]
fn the_schema_lists_its_types_and_a_column_takes_an_enum_type_from_the_type_menu() {
    let (mut h, customer) = table_harness();
    let schema = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        s.execute(
            "er.domain.add",
            &serde_json::json!({ "schema": schema, "name": "email_address", "base_type": "VARCHAR", "length": 255, "checks": ["position('@' in value) > 1"] }),
        )
        .unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
        schema
    };
    h.run();
    // The schema's Properties list the domain; + offers the kinds; the enum editor takes its
    // literals and Apply writes the enum type.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.type.row.") && !i.ends_with(".edit")).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let at = rect_of(&h, "workspace.properties.types.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::TYPE_KIND));
    let at = rect_of(&h, "workspace.properties.types.add.1").center();
    click(&mut h, at);
    assert!(er_of(h.state()).type_edit.as_ref().is_some_and(|t| t.kind == er_types::TypeKind::Enum && t.target.is_none()));
    let at = rect_of(&h, "workspace.properties.enum.literal.add").center();
    click(&mut h, at);
    click(&mut h, at);
    {
        let edit = er_of_mut(h.state_mut()).type_edit.as_mut().unwrap();
        edit.name = "customer_status".into();
        edit.literals = vec!["active".into(), "dormant".into()];
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.type.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).type_edit.is_none(), "{:?}", er_of(h.state()).type_edit);
    let (schema_obj, enum_id) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let schema_obj = st.by_uuid(archforge_mof::Uuid::parse(&schema).unwrap()).unwrap();
        let enums = archforge_er::rules::enums_of(st, schema_obj);
        assert_eq!(enums.len(), 1);
        assert_eq!(archforge_er::rules::literals_of(st, enums[0]), ["active", "dormant"]);
        (schema_obj, st.uuid(enums[0]).unwrap())
    };
    assert_eq!(selected_name(&h), "Shop", "the schema stays selected");
    assert_eq!(ids(&h).iter().filter(|i| i.starts_with("workspace.properties.type.row.") && !i.ends_with(".edit")).count(), 2);
    // The Models tree lists both types under the schema.
    assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{enum_id}")), "{:?}", ids(&h));
    // A column takes the enum type from its editor's type menu; the row shows the type's name.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    h.run();
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::COLUMN_TYPE));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.type.menu.")).collect();
    assert!(items.len() > 10, "the schema's two types and the base types: {items:?}");
    let at = rect_of(&h, &items[1]).center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).column.as_ref().and_then(|c| c.enum_type), Some(enum_id));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    let email = st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().iter().filter_map(archforge_mof::Value::as_ref).nth(1).unwrap();
    assert_eq!(archforge_er::rules::type_display(st, email), "customer_status");
    // A column on a sized domain applies a change of its own (Not null) without touching the size.
    let domain = archforge_er::rules::domains_of(st, schema_obj)[0];
    let domain = st.uuid(domain).unwrap().to_string();
    let email_id = st.uuid(email).unwrap().to_string();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("er.attribute.set-type", &serde_json::json!({ "attribute": email_id, "domain": domain })).unwrap();
    }
    h.run();
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.column.default"), "a domain column shows no default of its own");
    er_of_mut(h.state_mut()).column.as_mut().unwrap().not_null = true;
    h.run();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.is_none(), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!archforge_er::rules::is_nullable(st, email));
    assert_eq!(archforge_er::rules::type_display(st, email), "email_address");
    // A new column typed with a default, then put on the domain from the menu, adds cleanly: the
    // domain's default counts and the typed one goes with the pick.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    er_of_mut(h.state_mut()).column.as_mut().unwrap().default = "'x'".into();
    h.run();
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.type.menu.")).collect();
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.domain.is_some() && c.default.is_empty()));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    // The add selects the new column, whose editor opens on it without a refusal.
    assert!(er_of(h.state()).column.as_ref().is_none_or(|c| c.refused.is_none() && c.target.is_some()), "{:?}", er_of(h.state()).column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3);
}

/// A subtype shows in its supertype's Properties, and the ER toolbar has no generalization tool.
#[test]
fn a_subtype_shows_in_the_supertypes_properties_and_the_toolbar_draws_no_generalizations() {
    let (mut h, customer, order) = two_tables_harness();
    // ER draws subtypes with the tools it has; the toolbar has no generalization tool.
    assert!(!ids(&h).iter().any(|i| i == "workspace.tools.generalization"));
    let generalization = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("er.generalization.add", &serde_json::json!({ "supertype": customer, "subtype": order })).unwrap();
        let id = out.value.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        archforge_mof::Uuid::parse(&id).unwrap()
    };
    h.run();
    let (generalization, customer_obj, order_obj) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let g = st.by_uuid(generalization).unwrap();
        let c = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
        let o = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        assert_eq!(archforge_er::rules::generalization_parts(st, g).0, Some(c));
        assert_eq!(archforge_er::rules::generalization_parts(st, g).1, Some(o));
        assert!(archforge_er::rules::generalization_parts(st, g).2.is_some(), "table per type made the key");
        (g, c, o)
    };
    // The generalization's Properties show the supertype's Subtypes section; Mapping single table
    // removes the shared key in one step.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.subtype.row.") && !i.ends_with(".remove")).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let at = rect_of(&h, "workspace.properties.subtypes.mapping").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.subtypes.mapping.1").center();
    click(&mut h, at);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(archforge_er::rules::mapping_of(st, customer_obj), archforge_er::SubtypeMapping::SingleTable);
        assert!(archforge_er::rules::generalization_parts(st, generalization).2.is_none(), "the key went");
        assert_eq!(st.list(order_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 1, "Order keeps its own order_id only");
    }
    // The row's cross removes the subtype; the supertype stays selected.
    let remove = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.subtype.row.") && i.ends_with(".remove")).unwrap();
    let at = rect_of(&h, &remove).center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.is_alive(generalization));
    assert_eq!(selected_name(&h), "Customer");
}

/// A schema's Properties add and edit sequences and views, and a selected one's edit button opens
/// its editor.
#[test]
fn the_schema_adds_and_edits_sequences_and_views_and_a_selected_ones_edit_button_opens_its_editor() {
    let (mut h, _) = table_harness();
    let (schema, key) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let key = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "customer_id").unwrap()).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
        (schema, key)
    };
    h.run();
    // + opens the sequence editor; Owned by offers the whole-number columns; Apply writes it.
    let at = rect_of(&h, "workspace.properties.sequences.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.as_ref().is_some_and(|e| e.kind == er_objects::ObjectKind::Sequence && e.target.is_none()));
    let at = rect_of(&h, "workspace.properties.sequence.owned-by").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(er_state::menus::OWNED_BY));
    let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.sequence.owned-by.")).collect();
    assert_eq!(offered.len(), 2, "None and Customer.customer_id, not the text column: {offered:?}");
    let at = rect_of(&h, "workspace.properties.sequence.owned-by.1").center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).object_edit.as_ref().and_then(|e| e.owned_by), Some(key));
    {
        let edit = er_of_mut(h.state_mut()).object_edit.as_mut().unwrap();
        edit.name = "customer_id_seq".into();
        edit.start = "one".into();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.object.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.as_ref().is_some_and(|e| e.refused.is_some()), "a start that is no number is refused");
    {
        let edit = er_of_mut(h.state_mut()).object_edit.as_mut().unwrap();
        edit.start = "1000".into();
        edit.minimum = "1000".into();
    }
    h.run();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.is_none(), "{:?}", er_of(h.state()).object_edit);
    // The view editor takes the query over several lines.
    let at = rect_of(&h, "workspace.properties.views.add").center();
    click(&mut h, at);
    {
        let edit = er_of_mut(h.state_mut()).object_edit.as_mut().unwrap();
        assert_eq!(edit.kind, er_objects::ObjectKind::View);
        edit.name = "recent_customers".into();
        edit.query = "SELECT customer_id\nFROM \"Customer\"\n".into();
        edit.materialized = true;
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.object.apply").center();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.is_none(), "{:?}", er_of(h.state()).object_edit);
    let (seq, view) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let s = st.by_uuid(archforge_mof::Uuid::parse(&schema).unwrap()).unwrap();
        let (seqs, views) = (archforge_er::rules::sequences_of(st, s), archforge_er::rules::views_of(st, s));
        assert_eq!((seqs.len(), views.len()), (1, 1));
        let n = archforge_er::rules::sequence_numbers(st, seqs[0]);
        assert_eq!((n.start, n.minimum.as_str()), (1000, "1000"));
        assert_eq!(archforge_er::rules::owner_of_sequence(st, seqs[0]).and_then(|a| st.uuid(a).ok()), Some(key));
        assert_eq!(archforge_er::rules::text_of(st, views[0], archforge_er::view::QUERY), "SELECT customer_id\nFROM \"Customer\"");
        (st.uuid(seqs[0]).unwrap(), st.uuid(views[0]).unwrap())
    };
    assert_eq!(selected_name(&h), "Shop", "the schema stays selected");
    // Both show in the Models tree; a click on one selects it under its schema's section, and its
    // row's edit button opens its editor.
    for id in [seq, view] {
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{id}")), "{:?}", ids(&h));
    }
    let at = rect_of(&h, &format!("workspace.explorer.row.{view}")).center();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.is_none(), "selecting it does not open its editor");
    let at = rect_of(&h, &format!("workspace.properties.view.row.{view}.edit")).center();
    click(&mut h, at);
    assert!(er_of(h.state()).object_edit.as_ref().is_some_and(|e| e.target == Some(view) && e.materialized));
    // The bin removes it; the schema is selected again.
    let at = rect_of(&h, "workspace.properties.object.remove").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(view).is_none_or(|v| !st.is_alive(v)));
    assert_eq!(selected_name(&h), "Shop");
}

/// The Properties body scrolls, and an editor opened below the fold comes into view.
#[test]
fn the_properties_body_scrolls_and_an_editor_opened_below_the_fold_comes_into_view() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1440.0, 760.0));
    let customer = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
        let customer = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
        for i in 0..14 {
            s.execute(
                "er.attribute.add",
                &serde_json::json!({ "entity": customer, "name": format!("column_{i}"), "type": "BIGINT", "primary_key": i == 0 }),
            )
            .unwrap();
        }
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        customer
    };
    let _ = customer;
    h.run();
    // The sections past the window's foot are out of reach until the body scrolls.
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.indexes.add"), "the Indexes section starts below the fold");
    assert!(h.state().panels.content > h.state().panels.body.unwrap().height());
    let body = h.state().panels.body.unwrap();
    h.event(egui::Event::PointerMoved(body.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -2000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    // The wheel scrolls smoothly over several frames.
    h.run_steps(30);
    assert!(h.state().panels.scroll > 0.0);
    let at = rect_of(&h, "workspace.properties.indexes.add").center();
    click(&mut h, at);
    h.run();
    // The index editor opened at the foot is brought into view with its Apply.
    let apply = rect_of(&h, "workspace.properties.index.apply");
    let body = h.state().panels.body.unwrap();
    assert!(body.contains_rect(apply), "{apply:?} in {body:?}");
    // Another table starts at the top again.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
    }
    h.run();
    assert_eq!(h.state().panels.scroll, 0.0);
}

/// A column's Properties page is its table's, as ER answers the shell
/// ([`archforge_shell_api::notation_ui::NotationUi::page`]): moving from a table to one of its columns keeps the
/// body where it was scrolled, and moving on to another element starts it at the top.
#[test]
fn a_column_shows_its_tables_page_so_moving_to_it_keeps_the_scroll() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1440.0, 760.0));
        let (schema, column) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
            let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
            let customer = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
            let mut last = String::new();
            for i in 0..14 {
                let added = s.execute(
                    "er.attribute.add",
                    &serde_json::json!({ "entity": customer, "name": format!("column_{i}"), "type": "BIGINT", "primary_key": i == 0 }),
                );
                last = id(added.unwrap().value);
            }
            s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
            (schema, last)
        };
        h.run();
        let body = h.state().panels.body.unwrap();
        h.event(egui::Event::PointerMoved(body.center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -2000.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
        let scrolled = h.state().panels.scroll;
        assert!(scrolled > 0.0, "{theme:?}: the table's body scrolled");
        // Its column shows the same page: the body stays where it was.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [column] })).unwrap();
        h.run();
        assert_eq!(h.state().panels.scroll, scrolled, "{theme:?}: the column's page is its table's");
        // The schema is another page: it starts at the top.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
        h.run();
        assert_eq!(h.state().panels.scroll, 0.0, "{theme:?}: another element starts at the top");
    }
}
