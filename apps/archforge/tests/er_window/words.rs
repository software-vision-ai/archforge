//! ER's refusals in the chosen language that take a row of the shell's catalogue, which ER's
//! screens crate's test context does not register: said through the whole window.

use super::*;

/// An index column's direction or nulls order that is none of the words, or no word, is said in
/// the chosen language under the choice's label, with the words taken, one of only spaces as no
/// choice given, though not one whose word comes after 40 spaces; a word of another place is not ER's to say. In a window of the design's height, in light and dark (AR-491).
#[test]
fn an_er_index_column_word_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let schema = id("er.schema.add", json!({ "name": "Shop" }));
        let table = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
        let column = id("er.attribute.add", json!({ "entity": table, "name": "nummer", "type": "BIGINT" }));
        let mut refused = |field: &str, value: serde_json::Value| {
            let p = json!({ "entity": table, "columns": [{ "attribute": column, field: value }] });
            archforge_ui_egui::snapshot::run(h.state_mut(), "er.index.add", p).err().unwrap_or_default()
        };
        let up = "Richtung: up ist keine der Möglichkeiten. Möglich sind ascending, descending.";
        assert_eq!(refused("direction", json!("up")), up, "{theme:?}");
        let array = "Nullwerte: [1] ist keine der Möglichkeiten. Möglich sind default, first, last.";
        assert_eq!(refused("nulls", json!([1])), array, "{theme:?}");
        let none = "Richtung: Es ist keine Möglichkeit angegeben. Möglich sind ascending, descending.";
        assert_eq!(refused("direction", json!(" ")), none, "{theme:?}");
        // A word after more spaces than the typed word keeps is no blank.
        assert_eq!(refused("direction", json!(format!("{}up", " ".repeat(40)))), up, "{theme:?}");
        let diagram = archforge_engine::NotOneOf {
            what: archforge_engine::WordPlace::DiagramKind,
            word: "x".into(),
            known: vec![archforge_er::DIAGRAM_KIND.into()],
        };
        assert_eq!(er_said::said(SCREENS.notation_uis(), &archforge_engine::Reason::NotOneOf(Box::new(diagram))), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A key a step would break is said in the chosen language with why in the same words, and keys built
/// on each other too deep too; a reason inside it this can't say leaves the whole to the engine's
/// English. In light and dark (AR-493).
#[test]
fn an_er_key_a_step_would_break_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let breaks =
            |kind, why| archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::WouldBreak { kind, name: "pk_kunde".into(), why: Box::new(why) });
        let nullable = || Reason::NullableKeyColumn { name: "notiz".into() };
        let said = "Der Schlüssel pk_kunde wäre danach ungültig: notiz darf leer sein, was eine Primärschlüsselspalte nie darf.";
        assert_eq!(breaks("primary key", nullable()).as_deref(), Some(said), "{theme:?}");
        let other = "Der Schlüssel pk_kunde wäre danach ungültig: nummer ist keine Spalte von Bestellung.";
        let not_column = Reason::NotColumnOf { column: "nummer".into(), table: "Bestellung".into() };
        assert_eq!(breaks("unique constraint", not_column).as_deref(), Some(other), "{theme:?}");
        assert_eq!(breaks("check", nullable()), None, "{theme:?}");
        assert_eq!(breaks("primary key", Reason::TooFew { kind: "key column", min: 2 }), None, "{theme:?}");
        let deep = archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::TooDeep { kind: "key", max: 32 });
        assert_eq!(
            deep.as_deref(),
            Some("Aufeinander aufbauende Schlüssel reichen hier tiefer als 32 Ebenen oder bilden eine Schleife."),
            "{theme:?}"
        );
        assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::TooDeep { kind: "untyped kind", max: 32 }), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The selection's removal refused is said in the chosen language: an element removed where it is
/// edited, and a part of a subtype's key without its subtype. In light and dark (AR-508).
#[test]
fn an_er_removal_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        for (reason, said) in [
            (Reason::RemovedWhereEdited, "Dieses Element wird dort entfernt, wo es bearbeitet wird."),
            (Reason::SubtypeKeyPart, "Dies ist Teil des Schlüssels eines Untertyps und wird nur mit diesem Untertyp entfernt."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said), "{reason:?} {theme:?}");
        }
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A relationship that does not resolve into a mapping table, a table that does not collapse into
/// a relationship and one that cannot be associative are said in the chosen language by the rule
/// they break, a table with no foreign key within its primary key apart from one with one, and a
/// mapping table a step would break with why in the same words. In light and dark (AR-505).
#[test]
fn an_er_mapping_table_refused_is_said_in_the_chosen_language() {
    use archforge_engine::{MappingRule as M, Reason};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let keys = |keys| M::KeysInPrimaryKey { table: "Posten".into(), keys };
        for (rule, said) in [
            (M::NotManyToMany, "Nur aus einer Viele-zu-viele-Beziehung wird eine Zuordnungstabelle."),
            (M::LacksEnd, "Der Beziehung fehlt eine ihrer beiden Tabellen."),
            (M::TwoForeignKeys, "Eine Zuordnungstabelle hat genau zwei Fremdschlüssel."),
            (M::NoRelationship { foreign_key: "fk_kunde".into() }, "Der Fremdschlüssel fk_kunde steht für keine Beziehung."),
            (M::NoKey { foreign_key: "fk_ware".into() }, "Der Fremdschlüssel fk_ware verweist auf keinen Schlüssel."),
            (M::OwnRelationship, "Die Tabelle hat eine eigene Beziehung und lässt sich daher nicht in eine Beziehung zusammenfassen."),
            (M::OwnColumns, "Die Tabelle hat eigene Spalten und lässt sich daher nicht in eine Beziehung zusammenfassen."),
            (
                M::ColumnsOutsideKey,
                "Nicht alle Spalten der Tabelle gehören zu ihrem Primärschlüssel, daher lässt sie sich nicht in eine Beziehung zusammenfassen.",
            ),
            (M::UniqueConstraint, "Die Tabelle hat eine Eindeutigkeitsbedingung und lässt sich daher nicht in eine Beziehung zusammenfassen."),
            (M::Check, "Die Tabelle hat eine Prüfbedingung und lässt sich daher nicht in eine Beziehung zusammenfassen."),
            (M::Index, "Die Tabelle hat einen Index und lässt sich daher nicht in eine Beziehung zusammenfassen."),
            (keys(0), "Posten hat keinen Fremdschlüssel innerhalb des Primärschlüssels. Eine Zuordnungstabelle braucht zwei."),
            (keys(1), "Posten hat nur einen Fremdschlüssel innerhalb des Primärschlüssels. Eine Zuordnungstabelle braucht zwei."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::Mapping(rule.clone())).as_deref(), Some(said), "{rule:?} {theme:?}");
        }
        let breaks = Reason::WouldBreak { kind: "table", name: "Verknüpfung".into(), why: Box::new(Reason::Mapping(keys(1))) };
        let said = "Die Tabelle Verknüpfung wäre danach ungültig: Posten hat nur einen Fremdschlüssel innerhalb des Primärschlüssels. \
                    Eine Zuordnungstabelle braucht zwei.";
        assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &breaks).as_deref(), Some(said), "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A column's generation refused is said in the chosen language by the rule it breaks, a column on
/// a domain that would break with why in the same words, and a key or identity column asked to be
/// nullable; a reason inside it this can't say leaves the whole to the engine's English. In light
/// and dark (AR-494).
#[test]
fn an_er_columns_generation_refused_is_said_in_the_chosen_language() {
    use archforge_engine::{GenerationRule as G, Reason};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        for (rule, said) in [
            (G::ExpressionNeedsComputed, "Nur eine berechnete Spalte hat einen Ausdruck."),
            (G::ComputedNeedsExpression, "Eine berechnete Spalte braucht einen Ausdruck."),
            (G::ComputedTakesNoDefault, "Eine berechnete Spalte hat keinen Standardwert."),
            (G::IdentityNeedsInteger, "Eine Identitätsspalte braucht einen Ganzzahltyp."),
            (G::IdentityNotNullable, "Eine Identitätsspalte darf nie leer sein."),
            (G::IdentityTakesNoDefault, "Eine Identitätsspalte hat keinen Standardwert."),
            (G::IdentityTakesNoExpression, "Eine Identitätsspalte hat keinen Ausdruck."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::Generation(rule)).as_deref(), Some(said), "{rule:?} {theme:?}");
        }
        let breaks =
            |why| archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::WouldBreak { kind: "column", name: "nummer".into(), why: Box::new(why) });
        let said = "Die Spalte nummer wäre danach ungültig: Eine Identitätsspalte braucht einen Ganzzahltyp.";
        assert_eq!(breaks(Reason::Generation(G::IdentityNeedsInteger)).as_deref(), Some(said), "{theme:?}");
        assert_eq!(breaks(Reason::TooFew { kind: "key column", min: 2 }), None, "{theme:?}");
        let null = archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::NotNullable);
        assert_eq!(null.as_deref(), Some("Eine Schlüssel- oder Identitätsspalte darf nie leer sein."), "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A column's or a domain's size that does not fit its type is said in the chosen language by the
/// rule it breaks, with the type and the size; a column without a type name of its own as taking no
/// size, whichever part was given. In light and dark (AR-496).
#[test]
fn an_er_size_refused_is_said_in_the_chosen_language() {
    use archforge_engine::{Reason, SizeRule as R};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let none = "Eine Spalte ohne eigenen Typnamen hat keine Größe.";
        for (rule, ty, value, said) in [
            (R::OutOfRange, "VARCHAR", 0, "Die Größe 0 liegt nicht zwischen 1 und 65535."),
            (R::TakesNoLength, "", 4, none),
            (R::TakesNoPrecision, "", 4, none),
            (R::TakesNoScale, "", 2, none),
            (R::TakesNoLength, "INT", 4, "INT hat keine Länge."),
            (R::TakesNoPrecision, "VARCHAR", 3, "VARCHAR hat keine Genauigkeit."),
            (R::TakesNoScale, "TIMESTAMP", 2, "TIMESTAMP hat keine Nachkommastellen."),
            (R::BinaryTooLong, "binary", 256, "Ein BINARY fasst höchstens 255 Bytes, nicht 256."),
            (R::ScaleOverPrecision, "NUMERIC", 5, "Die Nachkommastellen (5) übersteigen die Genauigkeit."),
            (R::ScaleWithoutPrecision, "NUMERIC", 2, "Nachkommastellen brauchen eine Genauigkeit."),
        ] {
            let reason = Reason::SizeMisfit { rule, ty: ty.into(), value };
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said), "{reason:?} {theme:?}");
        }
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A column's own default, collation, place and link refused are said in the chosen language: a
/// default none of its enum type's literals in the words UML says it in, also for a column that
/// would break; a default or collation its domain gives; a place past the table's last, singular
/// and plural; and an unlink of a column that follows none. In light and dark (AR-495).
#[test]
fn an_er_columns_default_collation_place_and_link_refused_are_said_in_the_chosen_language() {
    use archforge_engine::{DefaultKind, Reason};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let unread = Reason::DefaultUnread { value: "'zu'".into(), kind: DefaultKind::Literal("status".into()) };
        let breaks = Reason::WouldBreak { kind: "column", name: "zustand".into(), why: Box::new(unread.clone()) };
        for (reason, said) in [
            (unread, "'zu' ist kein Literal von status."),
            (breaks, "Die Spalte zustand wäre danach ungültig: 'zu' ist kein Literal von status."),
            (Reason::DomainGives { what: "default" }, "Eine Spalte auf einer Domäne übernimmt den Standardwert der Domäne."),
            (Reason::DomainGives { what: "collation" }, "Eine Spalte auf einer Domäne übernimmt die Sortierung der Domäne."),
            (Reason::PastLastColumn { columns: 1 }, "Die Tabelle hat 1 Spalte."),
            (Reason::PastLastColumn { columns: 3 }, "Die Tabelle hat 3 Spalten."),
            (Reason::FollowsNoColumn { name: "kunde_id".into() }, "kunde_id folgt keiner Spalte."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said), "{reason:?} {theme:?}");
        }
        assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::DomainGives { what: "check" }), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// What a column is typed by, refused, is said in the chosen language: a type name that holds its
/// own size, a domain and an enum type together, and a type name or a size beside a domain or an
/// enum type; a part neither knows is left to the engine's English. In light and dark (AR-497).
#[test]
fn what_an_er_column_is_typed_by_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        for (reason, said) in [
            (Reason::SizeInTypeName, "Ein Typname enthält keine Größe; geben Sie stattdessen Länge, Genauigkeit oder Nachkommastellen an."),
            (Reason::DomainAndEnum, "Eine Spalte hat eine Domäne oder einen Aufzählungstyp, nicht beides."),
            (Reason::DomainGives { what: "type" }, "Eine Spalte auf einer Domäne übernimmt den Typ der Domäne."),
            (Reason::DomainGives { what: "size" }, "Eine Spalte auf einer Domäne übernimmt die Größe der Domäne."),
            (Reason::EnumTypeTakesNo { what: "type" }, "Eine Spalte eines Aufzählungstyps hat keinen eigenen Typnamen."),
            (Reason::EnumTypeTakesNo { what: "size" }, "Eine Spalte eines Aufzählungstyps hat keine Größe."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said), "{reason:?} {theme:?}");
        }
        assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::EnumTypeTakesNo { what: "default" }), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A check's expression and a domain's checks refused are said in the chosen language: no
/// expression in a constraint's words, too many checks, one named twice, an item of another shape,
/// and an item's key the list does not take, also one without a name. In light and dark (AR-499).
#[test]
fn an_er_checks_refused_are_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let takes = &["expression", "name"];
        for (reason, said) in [
            (Reason::NoExpression, "Geben Sie einen Ausdruck ein. Eine Einschränkung ohne Ausdruck legt keine Regel fest."),
            (Reason::TooMany { kind: "check", max: 32 }, "Geben Sie höchstens 32 Prüfbedingungen an."),
            (
                Reason::NamedTwice { kind: "check", name: "pos".into() },
                "Zwei Prüfbedingungen heißen pos. Geben Sie jeder Prüfbedingung einen eigenen Namen.",
            ),
            (Reason::ItemsExpected { kind: "check" }, "Geben Sie jede Prüfbedingung als Text an oder als Objekt mit einem Ausdruck und einem Namen."),
            (Reason::UnknownKey { key: "expresion".into(), takes }, "Ein Eintrag kennt keinen Schlüssel expresion. Er kennt expression, name."),
            (Reason::UnknownKey { key: String::new(), takes }, "Ein Eintrag enthält einen Schlüssel ohne Namen. Er kennt expression, name."),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said), "{reason:?} {theme:?}");
        }
        assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::ItemsExpected { kind: "gadget" }), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A sequence's numbers and owner and a view's query refused are said in the chosen language: each
/// rule the numbers break, an owner of another schema, of no table, of no type and of a type that is
/// no whole number, and a query by the text rules' rows. In light and dark (AR-507).
#[test]
fn an_er_sequences_numbers_owner_and_views_query_refused_are_said_in_the_chosen_language() {
    use archforge_engine::{Reason, SequenceRule as Rule};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let between = "keine ganze Zahl zwischen -9223372036854775808 und 9223372036854775807.";
        for (reason, said) in [
            (Rule::IncrementZero, "Die Schrittweite darf nicht 0 sein. Eine Sequenz bewegt sich um mindestens 1.".to_string()),
            (Rule::MinimumUnread { value: "eins".into() }, format!("Das Minimum „eins“ ist {between}")),
            (Rule::MaximumUnread { value: "zwei".into() }, format!("Das Maximum „zwei“ ist {between}")),
            (Rule::MaximumNotAbove { maximum: 5, minimum: 10 }, "Das Maximum 5 ist nicht größer als das Minimum 10.".into()),
            (Rule::StartBelow { start: 1, minimum: 10 }, "Der Startwert 1 liegt unter dem Minimum 10.".into()),
            (Rule::StartAbove { start: 50, maximum: 10 }, "Der Startwert 50 liegt über dem Maximum 10.".into()),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &Reason::Sequence(reason.clone())), Some(said), "{reason:?} {theme:?}");
        }
        let owner = "Eine Sequenz gehört zu einer Spalte";
        for (reason, said) in [
            (
                Reason::ColumnOfOtherSchema { column: "Kontakt.id".into() },
                format!("Kontakt.id ist eine Spalte eines anderen Schemas. {owner} ihres Schemas."),
            ),
            (Reason::InNoTable { column: "lose".into() }, "Die Spalte lose gehört zu keiner Tabelle.".into()),
            (Reason::NotIntegerType { column: "bar".into(), ty: "".into() }, format!("bar hat keinen Typ. {owner} mit ganzzahligem Typ.")),
            (Reason::NotIntegerType { column: "notiz".into(), ty: "TEXT".into() }, format!("notiz hat den Typ TEXT. {owner} mit ganzzahligem Typ.")),
            (Reason::NotA { kind: "column", name: "Kunde".into() }, "Kunde passt hier nicht: erwartet wird eine Spalte.".into()),
            (Reason::TextEmpty, "Der Text darf nicht leer sein.".into()),
            (Reason::ControlCharacter, "Steuerzeichen und unsichtbare Zeichen sind nicht erlaubt.".into()),
        ] {
            assert_eq!(archforge_ui_egui::panels::said(SCREENS.notation_uis(), &reason).as_deref(), Some(said.as_str()), "{reason:?} {theme:?}");
        }
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}
