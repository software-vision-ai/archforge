//! ER's screens tested without the shell (ADR 0005, "Tests follow the plugin"): what ER's own code
//! decides, driven through ER's interfaces over the shell's interfaces' test context
//! (`archforge_shell_api::testing`). A test opens a project's session in a context given ER alone,
//! runs commands through the narrow context as the shell's panels do, and reads ER's refusals as
//! the panels say them, in the chosen language.
//!
//! What the shell does around ER's screens (its canvas, Models, menus, the Properties panel) is
//! tested with the whole window in the app.

use archforge_mof::Uuid;
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::i18n::set_language;
use archforge_shell_api::notation_ui::NotationUi;
use archforge_shell_api::testing::TestContext;

use crate::er_ui::ErUi;

/// ER alone, as a test context is given it.
static UIS: &[&dyn NotationUi] = &[&ErUi];

/// Ids for a test's model: version 7 from a counter, no clock.
fn test_ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// A test context given ER, in English, with a new project's session open, as the shell opens
/// one.
fn context() -> TestContext {
    set_language("en");
    let mut tc = TestContext::new(UIS).unwrap();
    tc.session = Some(archforge_notations::NOTATIONS.new_project_session("Commerce platform", test_ids()).unwrap());
    tc
}

/// The problems ER finds in `store`, as the shell lists them.
fn problems(store: &archforge_mof::Store) -> Vec<archforge_shell_api::notation_ui::Problem> {
    let mut out = Vec::new();
    ErUi.problems(UIS, store, &mut out);
    out
}

/// Runs the command `cmd` through the narrow context, as the shell's panels do: a refusal is said
/// as the panel says it.
fn run(tc: &mut TestContext, cmd: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    tc.cx(|cx: &mut Ctx| archforge_shell_api::panels::run_cx(cx, cmd, params))
}

/// An ER command's id, key columns and constraint name refused are said in the chosen language,
/// never in the engine's English: no id, as the id of what the parameter takes, or of no element,
/// an element of another kind by its name, a subtype's among them, a key's columns too many, no ids or one given twice,
/// by the column's name (AR-544), and a constraint name the schema has; a kind ER does not name is left to the other
/// rows (AR-438).
#[test]
fn an_er_id_key_column_or_constraint_name_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_shell_api::i18n::{fmt, tr, tr_ctx};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let table = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
    let column = id("er.attribute.add", json!({ "entity": table, "name": "nummer", "type": "BIGINT" }));
    id("er.unique.add", json!({ "entity": table, "columns": [column], "name": "uq_nummer" }));
    let refused = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let no_id = tr("No id is given. Give the id of {what}.");
    assert_eq!(no_id, "Es ist keine ID angegeben. Hier wird {what} per ID erwartet.", "the language is German");
    let no_table = fmt(&no_id, &[("what", &tr_ctx("refusal", "a table"))]);
    assert_eq!(refused(tc, "er.entity.rename", json!({ "entity": "", "name": "X" })), no_table);
    let nope = fmt(&tr("There is no element with the id {id}."), &[("id", "nope")]);
    assert_eq!(refused(tc, "er.entity.rename", json!({ "entity": "nope", "name": "X" })), nope);
    let not_a = tr("{name} is not {what}.");
    let not_table = fmt(&not_a, &[("name", "nummer"), ("what", &tr_ctx("refusal", "a table"))]);
    assert_eq!(refused(tc, "er.entity.rename", json!({ "entity": column, "name": "X" })), not_table);
    let not_key = fmt(&not_a, &[("name", "Kunde"), ("what", &tr_ctx("refusal", "a key"))]);
    assert_eq!(refused(tc, "er.foreign-key.add", json!({ "entity": table, "key": table })), not_key);
    // A subtype link, the interface's word for the link from a subtype to its supertype.
    let no_subtype = fmt(&no_id, &[("what", &tr_ctx("refusal", "a subtype"))]);
    assert_eq!(refused(tc, "er.generalization.remove", json!({ "generalization": "" })), no_subtype);
    let not_subtype = fmt(&not_a, &[("name", "Kunde"), ("what", &tr_ctx("refusal", "a subtype"))]);
    assert_eq!(refused(tc, "er.generalization.remove", json!({ "generalization": table })), not_subtype);
    let columns = |given: serde_json::Value| json!({ "entity": table, "columns": given, "name": "uq_x" });
    let many = vec![column.as_str(); archforge_er::rules::MAX_KEY_COLUMNS + 1];
    let most = fmt(&tr("A key holds at most {max} columns."), &[("max", "32")]);
    assert_eq!(refused(tc, "er.unique.add", columns(json!(many))), most);
    assert_eq!(refused(tc, "er.unique.add", columns(json!([7]))), tr("Give the key's columns as a list of ids."));
    let twice = "nummer kommt im Schlüssel zweimal vor. Geben Sie jede Spalte nur einmal an.";
    assert_eq!(refused(tc, "er.unique.add", columns(json!([column, column]))), twice);
    let taken = fmt(&tr("There is {what} named {name} already."), &[("what", &tr_ctx("refusal", "a constraint")), ("name", "uq_nummer")]);
    let clash = json!({ "entity": table, "columns": [column], "name": "UQ_NUMMER" });
    assert_eq!(refused(tc, "er.unique.add", clash).to_lowercase(), taken.to_lowercase());
    // A kind ER does not name, though the catalogues have its word, is not ER's to say.
    assert_eq!(crate::er_said::said(UIS, &Reason::NotA { kind: "attribute", name: "x".into() }), None);
    set_language("en");
}

/// A key's columns refused are said in the chosen language, never in the engine's English: none,
/// for a primary key and a unique constraint, a column of another table, a key column a foreign key
/// needs and no primary key to remove; a column that may be null in a primary key, which the rules
/// type, too (AR-492).
#[test]
fn an_er_keys_columns_refused_are_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let kunde = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
    let order = id("er.entity.add", json!({ "schema": schema, "name": "Bestellung" }));
    let nummer = id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    let column = id("er.attribute.add", json!({ "entity": order, "name": "kunde_nummer", "type": "BIGINT", "nullable": "no" }));
    id("er.foreign-key.add", json!({ "entity": order, "references": kunde, "columns": [column], "name": "fk_kunde" }));
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let none = "Ein Schlüssel braucht mindestens eine Spalte.";
    assert_eq!(refused("er.primary-key.set", json!({ "entity": order, "columns": [], "name": "" })), none);
    assert_eq!(refused("er.unique.add", json!({ "entity": order, "columns": [] })), none);
    let other = "nummer ist keine Spalte von Bestellung.";
    assert_eq!(refused("er.unique.add", json!({ "entity": order, "columns": [nummer] })), other);
    let flags = json!({ "attribute": nummer, "primary_key": false, "nullable": "auto", "unique": false });
    let in_use = "Der Fremdschlüssel fk_kunde verweist auf diesen Schlüssel von Kunde. Ändern oder entfernen Sie ihn zuerst.";
    assert_eq!(refused("er.attribute.set-flags", flags), in_use);
    assert_eq!(refused("er.primary-key.remove", json!({ "entity": order })), "Bestellung hat keinen Primärschlüssel.");
    let nullable = crate::er_said::said(UIS, &Reason::NullableKeyColumn { name: "notiz".into() });
    assert_eq!(nullable.as_deref(), Some("notiz darf leer sein, was eine Primärschlüsselspalte nie darf."));
    // At least two columns is no rule of a key's.
    assert_eq!(crate::er_said::said(UIS, &Reason::TooFew { kind: "key column", min: 2 }), None);
    set_language("en");
}

/// A key's column given twice is said by the column's name in the chosen language: refused when a
/// primary key or a unique constraint lists it twice, and as the key rules' problem the Problems
/// panel says, which no document holds, as a key's columns are a list without repeats (AR-544).
#[test]
fn an_er_key_column_given_twice_is_said_by_its_name_in_the_chosen_language() {
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let kunde = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
    let nummer = id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "nullable": "no" }));
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let twice = "nummer kommt im Schlüssel zweimal vor. Geben Sie jede Spalte nur einmal an.";
    let pk = json!({ "entity": kunde, "columns": [nummer, nummer], "name": "pk_kunde" });
    assert_eq!(refused("er.primary-key.set", pk), twice);
    assert_eq!(refused("er.unique.add", json!({ "entity": kunde, "columns": [nummer, nummer] })), twice);
    let st = tc.session.as_ref().unwrap().doc();
    let obj = |id: &str| st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
    let (table, column) = (obj(&kunde), obj(&nummer));
    let problem = archforge_er::rules::key_columns_problem(st, table, &[column, column], true).unwrap();
    assert_eq!(archforge_shell_api::panels::said(UIS, &problem.reason()).as_deref(), Some(twice));
    set_language("en");
}

/// A foreign key's column given twice is said by the column's name in the foreign key's own words in
/// the chosen language, not a key's: refused when adding a foreign key and when pointing one at
/// another key (AR-544).
#[test]
fn an_er_foreign_key_column_given_twice_is_said_as_the_foreign_keys_in_the_chosen_language() {
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let kunde = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
    let bestellung = id("er.entity.add", json!({ "schema": schema, "name": "Bestellung" }));
    id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    let mail = id("er.attribute.add", json!({ "entity": kunde, "name": "mail", "type": "TEXT", "nullable": "no" }));
    let uq = id("er.unique.add", json!({ "entity": kunde, "columns": [mail], "name": "uq_mail" }));
    let column = id("er.attribute.add", json!({ "entity": bestellung, "name": "kunde_nummer", "type": "BIGINT", "nullable": "no" }));
    let fk = id("er.foreign-key.add", json!({ "entity": bestellung, "references": kunde, "columns": [column], "name": "fk_kunde" }));
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let twice = "kunde_nummer kommt im Fremdschlüssel zweimal vor.";
    let add = json!({ "entity": bestellung, "references": kunde, "columns": [column, column], "name": "fk_zwei" });
    assert_eq!(refused("er.foreign-key.add", add), twice);
    assert_eq!(refused("er.foreign-key.set-key", json!({ "foreign_key": fk, "key": uq, "columns": [column, column] })), twice);
    set_language("en");
}

/// The key a foreign key points at, refused, is said in the chosen language, never in the engine's
/// English: a key of another table, a table without a primary key, the key it points at already
/// and identifying relationships that would run in a cycle; a key without columns, which only a
/// loaded document holds, too (AR-501).
#[test]
fn the_key_a_foreign_key_points_at_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut ok = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap();
    let mut id = |cmd: &str, p: serde_json::Value| ok(cmd, p)["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let [kunde, posten, teil, notiz] =
        ["Kunde", "Posten", "Teil", "Notiz"].map(|name| id("er.entity.add", json!({ "schema": schema, "name": name })));
    id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    let hat = id("er.relationship.add", json!({ "from": kunde, "to": posten, "create_foreign_key": true, "identifying": true }));
    id("er.relationship.add", json!({ "from": posten, "to": teil, "create_foreign_key": true, "identifying": true }));
    let mut entity = |e: &str| ok("er.entity.get", json!({ "entity": e }));
    let key_of = |table: &serde_json::Value| table["primary_key"]["id"].as_str().unwrap().to_string();
    let (kunde_pk, teil_pk) = (key_of(&entity(&kunde)), key_of(&entity(&teil)));
    let posten_json = entity(&posten);
    let (posten_pk, posten_fk) = (key_of(&posten_json), posten_json["foreign_keys"][0]["id"].as_str().unwrap().to_string());
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    for (cmd, p, said) in [
        (
            "er.foreign-key.add",
            json!({ "entity": notiz, "key": kunde_pk, "references": posten }),
            "Der Schlüssel ist keiner der Schlüssel von Posten.",
        ),
        ("er.foreign-key.add", json!({ "entity": kunde, "references": notiz }), "Notiz hat keinen Primärschlüssel."),
        (
            "er.foreign-key.set-key",
            json!({ "foreign_key": posten_fk, "key": kunde_pk }),
            "Der Fremdschlüssel verweist bereits auf diesen Schlüssel. Nennen Sie die Spalten, um sie zu ändern.",
        ),
        (
            "er.foreign-key.add",
            json!({ "entity": kunde, "key": posten_pk, "relationship": hat }),
            "Identifizierende Beziehungen würden einen Zyklus zwischen Kunde und Posten bilden.",
        ),
        (
            "er.foreign-key.set-key",
            json!({ "foreign_key": posten_fk, "key": teil_pk }),
            "Identifizierende Beziehungen würden einen Zyklus zwischen Posten und Teil bilden.",
        ),
    ] {
        assert_eq!(refused(cmd, p), said, "{cmd}");
    }
    let no_columns = crate::er_said::said(UIS, &Reason::NoKeyColumns { table: "Lager".into() });
    assert_eq!(no_columns.as_deref(), Some("Der Schlüssel von Lager hat keine Spalten, auf die verwiesen werden kann."));
    // Another kind pointed at again is not a foreign key's.
    assert_eq!(crate::er_said::said(UIS, &Reason::Again { relationship: "index" }), None);
    set_language("en");
}

/// A foreign key's columns, refused, are said in the chosen language, never in the engine's
/// English: not as many as the key's, types that differ, a column without a type on either side,
/// and a column's new type that would break a foreign key; none, too many and one twice, which
/// only a loaded document holds, too (AR-502).
#[test]
fn a_foreign_keys_columns_refused_are_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let [kunde, auftrag, notiz] = ["Kunde", "Auftrag", "Notiz"].map(|name| id("er.entity.add", json!({ "schema": schema, "name": name })));
    id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    id("er.attribute.add", json!({ "entity": notiz, "name": "code", "primary_key": true }));
    let mut column = |name: &str, ty: &str| id("er.attribute.add", json!({ "entity": auftrag, "name": name, "type": ty }));
    let (kaeufer, haendler, titel, frei) = (column("kaeufer", "BIGINT"), column("haendler", "BIGINT"), column("titel", "TEXT"), column("frei", ""));
    let fk = id("er.foreign-key.add", json!({ "entity": auftrag, "references": kunde, "columns": [kaeufer], "name": "fk_kaeufer" }));
    let notiz_pk = run(tc, "er.entity.get", json!({ "entity": notiz })).unwrap()["primary_key"]["id"].as_str().unwrap().to_string();
    let add = |columns: serde_json::Value| json!({ "entity": auftrag, "references": kunde, "columns": columns });
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    for (cmd, p, said) in [
        ("er.foreign-key.add", add(json!([kaeufer, haendler])), "Der Fremdschlüssel braucht so viele Spalten wie der Schlüssel von Kunde, also 1."),
        ("er.foreign-key.add", add(json!([titel])), "titel hat den Typ TEXT, aber nummer hat den Typ BIGINT."),
        ("er.foreign-key.add", add(json!([frei])), "frei hat keinen Typ, aber nummer hat den Typ BIGINT."),
        (
            "er.foreign-key.set-key",
            json!({ "foreign_key": fk, "key": notiz_pk, "columns": [haendler] }),
            "haendler hat den Typ BIGINT, aber code hat keinen Typ.",
        ),
        (
            "er.attribute.set-type",
            json!({ "attribute": kaeufer, "type": "TEXT" }),
            "Der Fremdschlüssel fk_kaeufer wäre danach ungültig: kaeufer hat den Typ TEXT, aber nummer hat den Typ BIGINT.",
        ),
    ] {
        assert_eq!(refused(cmd, p), said, "{cmd}");
    }
    let kind = "foreign key column";
    for (reason, said) in [
        (Reason::TooFew { kind, min: 1 }, "Ein Fremdschlüssel braucht mindestens eine Spalte."),
        (Reason::TooMany { kind, max: 32 }, "Ein Fremdschlüssel umfasst höchstens 32 Spalten."),
        (Reason::NamedTwice { kind, name: "c0".into() }, "c0 kommt im Fremdschlüssel zweimal vor."),
    ] {
        assert_eq!(crate::er_said::said(UIS, &reason).as_deref(), Some(said));
    }
    // Two columns without a type never differ: that is left to the engine's English.
    let untyped = Reason::TypesDiffer { column: "a".into(), target: "b".into(), types: Box::new([None, None]) };
    assert_eq!(crate::er_said::said(UIS, &untyped), None);
    set_language("en");
}

/// A foreign key's actions and relationship, refused, are said in the chosen language, never in
/// the engine's English: set null on a column that may never be null, from adding a foreign key and
/// from setting its actions, a relationship that does not join the two tables, a column outside
/// the primary key under an identifying relationship, and a column made not null under a set-null
/// foreign key; a key that is none and one without columns, which only a loaded document holds,
/// too (AR-503).
#[test]
fn a_foreign_keys_actions_and_relationship_refused_are_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let [kunde, auftrag, artikel] = ["Kunde", "Auftrag", "Artikel"].map(|name| id("er.entity.add", json!({ "schema": schema, "name": name })));
    id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    id("er.attribute.add", json!({ "entity": artikel, "name": "code", "type": "TEXT", "primary_key": true }));
    let mut column =
        |name: &str, nullable: &str| id("er.attribute.add", json!({ "entity": auftrag, "name": name, "type": "BIGINT", "nullable": nullable }));
    let (kaeufer, vielleicht) = (column("kaeufer", "no"), column("vielleicht", "yes"));
    let fk = id("er.foreign-key.add", json!({ "entity": auftrag, "references": kunde, "columns": [kaeufer], "name": "fk_kaeufer" }));
    let null = json!({ "entity": auftrag, "references": kunde, "columns": [vielleicht], "name": "fk_vielleicht", "on_delete": "set-null" });
    id("er.foreign-key.add", null);
    let bestellt = json!({ "from": kunde, "to": auftrag, "to_end": "zero-or-more", "name": "bestellt" });
    let bestellt = id("er.relationship.add", bestellt);
    let traegt = json!({ "from": artikel, "to": auftrag, "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    let traegt = id("er.relationship.add", traegt);
    let titel = id("er.attribute.add", json!({ "entity": auftrag, "name": "titel", "type": "TEXT", "nullable": "no" }));
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    for (cmd, p, said) in [
        (
            "er.foreign-key.add",
            json!({ "entity": auftrag, "references": kunde, "columns": [kaeufer], "on_delete": "set-null" }),
            "„Auf Null setzen“ braucht Spalten, die leer sein dürfen, aber kaeufer darf nie leer sein.",
        ),
        (
            "er.foreign-key.set-actions",
            json!({ "foreign_key": fk, "on_delete": "no-action", "on_update": "set-null" }),
            "„Auf Null setzen“ braucht Spalten, die leer sein dürfen, aber kaeufer darf nie leer sein.",
        ),
        (
            "er.foreign-key.add",
            json!({ "entity": auftrag, "references": artikel, "columns": [titel], "relationship": bestellt }),
            "Die Beziehung verbindet Auftrag und Artikel nicht.",
        ),
        (
            "er.foreign-key.add",
            json!({ "entity": auftrag, "references": artikel, "columns": [titel], "relationship": traegt }),
            "Die Fremdschlüsselspalten einer identifizierenden Beziehung gehören zum Primärschlüssel, titel aber nicht.",
        ),
        (
            "er.attribute.set-flags",
            json!({ "attribute": vielleicht, "primary_key": false, "nullable": "no", "unique": false }),
            "Der Fremdschlüssel fk_vielleicht wäre danach ungültig: „Auf Null setzen“ braucht Spalten, die leer sein dürfen, aber vielleicht darf nie leer sein.",
        ),
    ] {
        assert_eq!(refused(cmd, p), said, "{cmd}");
    }
    for (reason, said) in [
        (Reason::NotA { kind: "key", name: "nummer".into() }, "nummer passt hier nicht: erwartet wird ein Schlüssel."),
        (Reason::NoKeyColumns { table: "Kunde".into() }, "Der Schlüssel von Kunde hat keine Spalten, auf die verwiesen werden kann."),
    ] {
        assert_eq!(crate::er_said::said(UIS, &reason).as_deref(), Some(said));
    }
    set_language("en");
}

/// A subtype refused is said in the chosen language, never in the engine's English: a table its own
/// subtype, tables of two schemas, a subtype with a supertype already, a circle, identifying keys in
/// a cycle, a discriminator of another table and a part of a subtype's key changed on its own; a
/// chain too deep too (AR-506).
#[test]
fn a_subtype_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut ok = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap();
    let mut id = |cmd: &str, p: serde_json::Value| ok(cmd, p)["id"].as_str().unwrap().to_string();
    let (schema, archiv) = (id("er.schema.add", json!({ "name": "Shop" })), id("er.schema.add", json!({ "name": "Archiv" })));
    let [partei, person, gut, a, b] =
        ["Partei", "Person", "Gut", "A", "B"].map(|name| id("er.entity.add", json!({ "schema": schema, "name": name })));
    let alt = id("er.entity.add", json!({ "schema": archiv, "name": "Alt" }));
    for (e, name) in [(&partei, "nummer"), (&a, "a_nr"), (&b, "b_nr")] {
        id("er.attribute.add", json!({ "entity": e, "name": name, "type": "BIGINT", "primary_key": true }));
    }
    let fremd = id("er.attribute.add", json!({ "entity": gut, "name": "art", "type": "TEXT" }));
    id("er.generalization.add", json!({ "supertype": partei, "subtype": person }));
    id("er.relationship.add", json!({ "from": b, "to": a, "create_foreign_key": true, "identifying": true }));
    let geerbt = ok("er.entity.get", json!({ "entity": person }))["attributes"][0]["id"].as_str().unwrap().to_string();
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let add = |sup: &str, sub: &str| json!({ "supertype": sup, "subtype": sub });
    let piece =
        "Dies ist Teil des Schlüssels eines Untertyps. Entfernen Sie stattdessen den Untertyp oder ändern Sie die Untertyp-Abbildung des Obertyps.";
    for (cmd, p, said) in [
        ("er.generalization.add", add(&partei, &partei), "Eine Tabelle kann kein Untertyp von sich selbst sein."),
        (
            "er.generalization.add",
            add(&partei, &alt),
            "Die beiden Tabellen liegen in verschiedenen Schemas. Ein Untertyp liegt im Schema seines Obertyps.",
        ),
        ("er.generalization.add", add(&gut, &person), "Person hat bereits einen Obertyp. Eine Tabelle hat einen Obertyp."),
        ("er.generalization.add", add(&person, &partei), "Der Obertyp ist bereits ein Untertyp dieser Tabelle, das würde einen Kreis schließen."),
        ("er.generalization.add", add(&a, &b), "Identifizierende Beziehungen würden einen Zyklus zwischen B und A bilden."),
        ("er.entity.set-subtyping", json!({ "entity": partei, "discriminator": fremd }), "art ist keine Spalte von Partei."),
        ("er.attribute.remove", json!({ "attribute": geerbt }), piece),
        ("er.primary-key.remove", json!({ "entity": person }), piece),
    ] {
        assert_eq!(refused(cmd, p), said, "{cmd}");
    }
    let deep = crate::er_said::said(UIS, &Reason::TooDeep { kind: "subtype", max: 32 });
    assert_eq!(deep.as_deref(), Some("Untertypen würden hier tiefer als 32 Ebenen reichen."));
    // A cycle of another relationship is not a subtype's.
    assert_eq!(crate::er_said::said(UIS, &Reason::Cycle { relationship: "foreign key" }), None);
    set_language("en");
}

/// A relationship refused is said in the chosen language, never in the engine's English:
/// identifying or a key to point at without the foreign key, a foreign key for many to many, a
/// table with itself without a role or identifying itself, a key of another table, a parent without
/// a primary key, roles against the name rule or without a letter, and a subtype's key changed
/// (AR-504).
#[test]
fn a_relationship_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut ok = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap();
    let mut id = |cmd: &str, p: serde_json::Value| ok(cmd, p)["id"].as_str().unwrap().to_string();
    let schema = id("er.schema.add", json!({ "name": "Shop" }));
    let [kunde, auftrag, notiz, vip] =
        ["Kunde", "Auftrag", "Notiz", "Vip"].map(|name| id("er.entity.add", json!({ "schema": schema, "name": name })));
    id("er.attribute.add", json!({ "entity": kunde, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    id("er.attribute.add", json!({ "entity": notiz, "name": "nummer", "type": "BIGINT", "primary_key": true }));
    id("er.generalization.add", json!({ "supertype": kunde, "subtype": vip }));
    let places = id("er.relationship.add", json!({ "from": kunde, "to": auftrag }));
    let vip_json = ok("er.entity.get", json!({ "entity": vip }));
    let vip_fk = vip_json["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    let notiz_pk = ok("er.entity.get", json!({ "entity": notiz }))["primary_key"]["id"].as_str().unwrap().to_string();
    let mut refused = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let add = "er.relationship.add";
    let keyed = |from: &str, to: &str, extra: serde_json::Value| {
        let mut p = json!({ "from": from, "to": to, "create_foreign_key": true });
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    for (cmd, p, said) in [
        (
            add,
            json!({ "from": kunde, "to": auftrag, "identifying": true }),
            "Eine identifizierende Beziehung braucht ihren Fremdschlüssel; erst den Schalter einschalten",
        ),
        (
            add,
            json!({ "from": kunde, "to": auftrag, "points_to": notiz_pk }),
            "Nur eine Beziehung mit ihrem Fremdschlüssel verweist auf einen Schlüssel.",
        ),
        (
            add,
            keyed(&kunde, &auftrag, json!({ "from_end": "one-or-more" })),
            "Eine Viele-zu-viele-Beziehung hat keinen Fremdschlüssel. Fügen Sie stattdessen eine Zuordnungstabelle hinzu.",
        ),
        (
            add,
            keyed(&kunde, &kunde, json!({})),
            "Eine Beziehung einer Tabelle mit sich selbst braucht eine Rolle an dem Ende, auf das ihr Schlüssel verweist, etwa parent.",
        ),
        (
            add,
            keyed(&kunde, &kunde, json!({ "from_role": "ganzes", "identifying": true })),
            "Identifizierende Beziehungen würden einen Zyklus zwischen Kunde und Kunde bilden.",
        ),
        (add, keyed(&kunde, &auftrag, json!({ "points_to": notiz_pk })), "Der Schlüssel ist keiner der Schlüssel von Kunde."),
        (add, keyed(&auftrag, &kunde, json!({})), "Auftrag hat keinen Primärschlüssel."),
        (add, keyed(&kunde, &auftrag, json!({ "to_role": "käufer " })), "Am Anfang und am Ende darf kein Leerzeichen stehen."),
        (
            add,
            keyed(&kunde, &auftrag, json!({ "from_role": "--" })),
            "Eine Rolle braucht einen Buchstaben oder eine Ziffer, da sie Schlüsselspalten benennt.",
        ),
        (
            "er.relationship.set-identifying",
            json!({ "relationship": places, "identifying": true }),
            "Eine identifizierende Beziehung braucht ihren Fremdschlüssel; erst den Schalter einschalten",
        ),
        (
            "er.foreign-key.remove",
            json!({ "foreign_key": vip_fk }),
            "Dies ist Teil des Schlüssels eines Untertyps. Entfernen Sie stattdessen den Untertyp oder ändern Sie die Untertyp-Abbildung des Obertyps.",
        ),
    ] {
        assert_eq!(refused(cmd, p), said, "{cmd}");
    }
    set_language("en");
}

/// A domain or an enum type that columns take, and an enum type's literals refused, are said in the
/// chosen language: the type with its first columns, singular and plural, a list the engine cut
/// ending in an ellipsis; none, too many, no list
/// of names, a literal that is no name with why in the same words (an empty one on its own), and
/// one twice, also as a problem of a loaded enum type (AR-498).
#[test]
fn an_er_type_in_use_and_its_literals_refused_are_said_in_the_chosen_language() {
    use archforge_engine::{Reason, TypeInUse};
    // A context given ER registers ER's catalogues; nothing runs on its session.
    let _context = context();
    set_language("de");
    let in_use = |name: &str, columns: &[&str], count| {
        Reason::TypeInUse(Box::new(TypeInUse { name: name.into(), columns: columns.iter().map(|c| c.to_string()).collect(), count }))
    };
    let named = |name: &str, why| Reason::LiteralName { name: name.into(), why: Box::new(why) };
    for (reason, said) in [
        (in_use("status", &["Bestellung.status"], 1), "1 Spalte verwendet status: Bestellung.status. Ändern Sie zuerst ihren Typ."),
        (
            in_use("geld", &["Bestellung.summe", "Kunde.saldo"], 7),
            "7 Spalten verwenden geld: Bestellung.summe, Kunde.saldo, …. Ändern Sie zuerst ihren Typ.",
        ),
        (Reason::NamesExpected { kind: "enum literal" }, "Geben Sie die Literale als Liste von Namen an."),
        (Reason::TooFew { kind: "enum literal", min: 1 }, "Ein Aufzählungstyp braucht mindestens ein Literal."),
        (Reason::TooMany { kind: "enum literal", max: 256 }, "Ein Aufzählungstyp hat höchstens 256 Literale."),
        (named("", Reason::NameEmpty), "Ein Literal darf nicht leer sein."),
        (named(" offen", Reason::EdgeSpace), "Das Literal „ offen“ ist kein gültiger Name. Am Anfang und am Ende darf kein Leerzeichen stehen."),
        (
            Reason::NamedTwice { kind: "enum literal", name: "OFFEN".into() },
            "Das Literal „OFFEN“ ist doppelt angegeben. Geben Sie jedes Literal nur einmal an.",
        ),
    ] {
        assert_eq!(archforge_shell_api::panels::said(UIS, &reason).as_deref(), Some(said), "{reason:?}");
    }
    // A loaded enum type's literal twice, which no command makes, is a problem in the same words.
    let mut n = 0u64;
    let ids = Box::new(move || {
        n += 1;
        archforge_mof::Uuid::v7(1_761_670_800_000, [0, 0, 0, 0, 0, 0, 0, 0, 0, u8::try_from(n).unwrap_or(u8::MAX)])
    });
    let mut st = archforge_er::store(ids).unwrap();
    let e = st.create(archforge_er::ENUM_TYPE).unwrap();
    st.set(e, archforge_er::named_element::NAME, archforge_mof::Value::str("status")).unwrap();
    for literal in ["offen", "OFFEN"] {
        st.push(e, archforge_er::enum_type::LITERAL, archforge_mof::Value::str(literal)).unwrap();
    }
    let texts: Vec<String> = problems(&st).into_iter().map(|p| p.text).collect();
    assert_eq!(texts, ["status: Das Literal „OFFEN“ ist doppelt angegeben. Geben Sie jedes Literal nur einmal an."]);
    set_language("en");
}

/// An index's columns refused are said in the chosen language: none, too many, one twice, an item
/// of another shape, and an index column with both a column and an expression or with neither, also
/// as a problem of a loaded index without columns (AR-500).
#[test]
fn an_er_indexs_columns_refused_are_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    // A context given ER registers ER's catalogues; nothing runs on its session.
    let _context = context();
    set_language("de");
    for (reason, said) in [
        (Reason::TooFew { kind: "index column", min: 1 }, "Ein Index braucht mindestens eine Spalte."),
        (Reason::TooMany { kind: "index column", max: 32 }, "Ein Index umfasst höchstens 32 Spalten."),
        (Reason::NamedTwice { kind: "index column", name: "total".into() }, "total steht zweimal im Index. Geben Sie jede Spalte nur einmal an."),
        (
            Reason::ItemsExpected { kind: "index column" },
            "Geben Sie jede Indexspalte als ID einer Spalte an oder als Objekt mit einem Attribut oder einem Ausdruck.",
        ),
        (
            Reason::AttributeAndExpression { column: "total".into() },
            "total ist zusammen mit einem Ausdruck angegeben. Eine Indexspalte sortiert nach einer Spalte oder nach einem Ausdruck.",
        ),
        (Reason::NoAttributeNorExpression, "Eine Indexspalte braucht eine Spalte oder einen Ausdruck, nach dem sie sortiert."),
    ] {
        assert_eq!(archforge_shell_api::panels::said(UIS, &reason).as_deref(), Some(said), "{reason:?}");
    }
    // A loaded index without columns, which no command makes, is a problem in the same words.
    let mut n = 0u64;
    let ids = Box::new(move || {
        n += 1;
        archforge_mof::Uuid::v7(1_761_670_800_000, [0, 0, 0, 0, 0, 0, 0, 0, 0, u8::try_from(n).unwrap_or(u8::MAX)])
    });
    let mut st = archforge_er::store(ids).unwrap();
    let e = st.create(archforge_er::ENTITY).unwrap();
    st.set(e, archforge_er::named_element::NAME, archforge_mof::Value::str("Bestellung")).unwrap();
    let ix = st.create(archforge_er::TABLE_INDEX).unwrap();
    st.set(ix, archforge_er::table_index::NAME, archforge_mof::Value::str("ix_leer")).unwrap();
    st.push(e, archforge_er::entity::TABLE_INDEX, archforge_mof::Value::Ref(ix)).unwrap();
    let texts: Vec<String> = problems(&st).into_iter().map(|p| p.text).collect();
    let index: Vec<&String> = texts.iter().filter(|t| t.starts_with("ix_leer")).collect();
    assert_eq!(index, ["ix_leer: Ein Index braucht mindestens eine Spalte."], "{texts:?}");
    set_language("en");
}
