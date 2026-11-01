//! What ER brings to the shell's explorers, search, problems and panels: the
//! Models tree's schemas with their skills, types, sequences, views and tables, and a table's
//! skills and columns; `+` adds a schema, a table or a column; the search's schemas, tables,
//! columns, relationships and types; the problems the rules find, in the user's words; and
//! Properties for every ER element, drawn by ER's sections.

use std::collections::BTreeSet;

use archforge_er::rules as r;
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::Rect;
use serde_json::json;

use crate::Shell;
use crate::explorer::{Row, Tree, fresh_name};
use crate::i18n::{fmt, tr, trn};
use crate::notation_ui::{Added, Entry, NotationUi, Offer, Words};
use crate::panels::{FIELD, Subject};
use crate::problems::Problem;
use crate::ux::{Palette, icons};

/// ER in the shell.
pub(crate) struct ErUi;

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

fn children(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|c| store.is_alive(*c)).collect()
}

fn class(store: &Store, o: Obj) -> Option<archforge_mof::ClassId> {
    store.class_of(o).ok()
}

/// The schemas of the model, in the order the model lists them.
fn schemas(store: &Store) -> Vec<Obj> {
    let Some(model) = archforge_core::skills::model_of(store) else { return Vec::new() };
    children(store, model, archforge_core::model::MEMBER).into_iter().filter(|o| class(store, *o) == Some(archforge_er::SCHEMA)).collect()
}

/// What goes with `obj` when it is removed: the relationships that reach it, and the keys and
/// foreign keys the ER cascade takes.
pub(crate) fn consequences(store: &Store, obj: Obj) -> (usize, usize, usize) {
    let reaching = |e: Obj| {
        store.referrers(e).into_iter().filter(|(_, p)| *p == archforge_er::relationship::FROM || *p == archforge_er::relationship::TO).map(|(r, _)| r)
    };
    let relationships = match class(store, obj) {
        Some(c) if c == archforge_er::ENTITY => reaching(obj).collect::<BTreeSet<_>>().len(),
        Some(c) if c == archforge_er::SCHEMA => {
            let own: BTreeSet<Obj> = children(store, obj, archforge_er::schema::RELATIONSHIP).into_iter().collect();
            let reaching: BTreeSet<Obj> = children(store, obj, archforge_er::schema::ENTITY).into_iter().flat_map(reaching).collect();
            own.union(&reaching).count()
        }
        _ => 0,
    };
    let (keys, fks) = archforge_er::commands::dependents(store, obj).unwrap_or_default();
    (relationships, keys.len(), fks.len())
}

impl NotationUi for ErUi {
    fn metamodel(&self) -> u16 {
        archforge_er::INDEX
    }

    fn is_top(&self, store: &Store, member: Obj) -> bool {
        class(store, member) == Some(archforge_er::SCHEMA)
    }

    /// A schema with its skills, types, sequences and views, and its tables, each with its skills
    /// and columns.
    fn tree(&self, store: &Store, schema: Obj, t: &Tree<'_>) -> Vec<Row> {
        let mut out = Vec::new();
        let entities = children(store, schema, archforge_er::schema::ENTITY);
        let mut rows_under = t.skill_rows(schema, 1);
        for d in r::domains_of(store, schema) {
            let label = name(store, d);
            if t.matches(&label) {
                let base = Some(r::text_of(store, d, archforge_er::domain::BASE_TYPE)).filter(|t| !t.is_empty());
                rows_under.extend(t.row(d, 1, "domain", &icons::DOMAIN, label).map(|row| Row { detail: base, ..row }));
            }
        }
        for en in r::enums_of(store, schema) {
            let label = name(store, en);
            if t.matches(&label) {
                let n = r::literals_of(store, en).len();
                let detail = Some(trn("{n} literal", "{n} literals", n as u64));
                rows_under.extend(t.row(en, 1, "enum", &icons::ENUM, label).map(|row| Row { detail, ..row }));
            }
        }
        for q in r::sequences_of(store, schema) {
            let label = name(store, q);
            if t.matches(&label) {
                rows_under.extend(t.row(q, 1, "sequence", &icons::SEQUENCE, label));
            }
        }
        for v in r::views_of(store, schema) {
            let label = name(store, v);
            if t.matches(&label) {
                rows_under.extend(t.row(v, 1, "view", &icons::VIEW, label));
            }
        }
        for e in &entities {
            let e_label = name(store, *e);
            let attributes = children(store, *e, archforge_er::entity::ATTRIBUTE);
            let mut under_entity = t.skill_rows(*e, 2);
            for a in &attributes {
                let label = name(store, *a);
                let ty = Some(r::type_display(store, *a)).filter(|t| !t.is_empty());
                if t.matches(&label) {
                    under_entity.extend(t.row(*a, 2, "column", &icons::COLUMN, label).map(|row| Row { detail: ty, muted: true, ..row }));
                }
            }
            let expandable = if t.filtering() { !under_entity.is_empty() } else { !t.skills(*e).is_empty() || !attributes.is_empty() };
            if t.matches(&e_label) || !under_entity.is_empty() {
                let expanded = expandable && t.open(*e);
                rows_under.extend(t.row(*e, 1, "table", &icons::TABLE, e_label).map(|row| Row { expandable, expanded, draggable: true, ..row }));
                if expanded {
                    rows_under.extend(under_entity);
                }
            }
        }
        let s_label = name(store, schema);
        let has_types = !r::domains_of(store, schema).is_empty()
            || !r::enums_of(store, schema).is_empty()
            || !r::sequences_of(store, schema).is_empty()
            || !r::views_of(store, schema).is_empty();
        let expandable = if t.filtering() { !rows_under.is_empty() } else { !t.skills(schema).is_empty() || !entities.is_empty() || has_types };
        if t.matches(&s_label) || !rows_under.is_empty() {
            let expanded = expandable && t.open(schema);
            out.extend(t.row(schema, 0, "schema", &icons::SCHEMA, s_label).map(|row| Row { expandable, expanded, ..row }));
            if expanded {
                out.extend(rows_under);
            }
        }
        out
    }

    fn offers(&self, store: &Store, selection: Option<Obj>) -> Vec<Offer> {
        let offer = |key, word: String| Offer { key, word };
        match selection.and_then(|o| class(store, o)) {
            Some(c) if c == archforge_er::SCHEMA => vec![offer("table", tr("Table")), offer("schema", tr("Schema"))],
            Some(c) if c == archforge_er::ENTITY || c == archforge_er::ATTRIBUTE => vec![offer("column", tr("Column")), offer("table", tr("Table"))],
            _ => vec![offer("schema", tr("Schema"))],
        }
    }

    /// A schema, a table in the selection's schema, or a column in the selection's table; a table
    /// lands on an open diagram of its schema too, and a column's table grows with it, in the
    /// same step.
    fn add(&self, shell: &mut Shell, key: &str, sel: Option<Obj>) -> Option<Result<Added, String>> {
        let store = shell.session.as_ref()?.doc();
        let no_name = || Some(Err(tr("No free name is left")));
        let mut parents: Vec<Uuid> = Vec::new();
        let out = match key {
            "schema" => {
                let Some(name) = fresh_name("Schema", schemas(store).into_iter().map(|s| name(store, s)), false) else { return no_name() };
                crate::explorer::run(shell, "er.schema.add", json!({ "name": name }))
            }
            "table" => {
                let schema = sel.and_then(|o| r::schema_of(store, o))?;
                let taken = children(store, schema, archforge_er::schema::ENTITY).into_iter().map(|e| name(store, e));
                let Some(name) = fresh_name("Table", taken, false) else { return no_name() };
                let schema_id = store.uuid(schema).ok()?;
                parents.push(schema_id);
                crate::editing::add_entity(shell, schema_id, &name).map(|id| json!({ "id": id.to_string() }))
            }
            "column" => {
                let entity = sel.and_then(|o| match class(store, o) {
                    Some(c) if c == archforge_er::ENTITY => Some(o),
                    Some(c) if c == archforge_er::ATTRIBUTE => r::entity_of(store, o),
                    _ => None,
                })?;
                let taken = children(store, entity, archforge_er::entity::ATTRIBUTE).into_iter().map(|a| name(store, a));
                let Some(name) = fresh_name("column", taken, true) else { return no_name() };
                let entity_id = store.uuid(entity).ok()?;
                parents.extend(r::schema_of(store, entity).and_then(|s| store.uuid(s).ok()));
                parents.push(entity_id);
                let params = json!({ "entity": entity_id.to_string(), "name": name, "type": "TEXT" });
                crate::editing::run_growing(shell, &tr("Add column"), "er.attribute.add", params, entity_id)
            }
            _ => return None,
        };
        match out {
            Ok(out) => out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse).map(|id| Ok(Added { id, parents })),
            Err(e) => Some(Err(e)),
        }
    }

    fn rename(&self, store: &Store, obj: Obj) -> Option<(&'static str, &'static str)> {
        match class(store, obj)? {
            c if c == archforge_er::SCHEMA => Some(("er.schema.rename", "schema")),
            c if c == archforge_er::ENTITY => Some(("er.entity.rename", "entity")),
            c if c == archforge_er::ATTRIBUTE => Some(("er.attribute.rename", "attribute")),
            _ => None,
        }
    }

    fn consequences(&self, store: &Store, obj: Obj) -> (usize, usize, usize) {
        consequences(store, obj)
    }

    /// A schema's tables, types, sequences and views, a table's columns (slices S5 and S7).
    fn has_children(&self, store: &Store, obj: Obj) -> bool {
        [
            archforge_er::schema::ENTITY,
            archforge_er::schema::DOMAIN,
            archforge_er::schema::ENUM_TYPE,
            archforge_er::schema::SEQUENCE,
            archforge_er::schema::VIEW,
            archforge_er::entity::ATTRIBUTE,
        ]
        .into_iter()
        .any(|prop| !children(store, obj, prop).is_empty())
    }

    /// The schema and its tables.
    fn foldable(&self, store: &Store, schema: Obj) -> Vec<Obj> {
        std::iter::once(schema).chain(children(store, schema, archforge_er::schema::ENTITY)).collect()
    }

    /// The table and the schema of the element.
    fn parents(&self, store: &Store, obj: Obj) -> Vec<Obj> {
        [r::entity_of(store, obj).filter(|e| *e != obj), r::schema_of(store, obj).filter(|s| *s != obj)].into_iter().flatten().collect()
    }

    fn entry(&self, store: &Store, o: Obj) -> Option<Entry> {
        let place = |kind: String, at: String| if at.is_empty() { kind } else { format!("{kind} · {at}") };
        let schema_name = |o: Obj| r::schema_of(store, o).map(|s| name(store, s)).unwrap_or_default();
        let entry = |icon, order, label, detail| Some(Entry { icon, order, label, detail });
        match class(store, o)? {
            c if c == archforge_er::SCHEMA => entry(&icons::SCHEMA, 1, name(store, o), tr("Schema")),
            c if c == archforge_er::ENTITY => entry(&icons::TABLE, 2, name(store, o), place(tr("Table"), schema_name(o))),
            c if c == archforge_er::ATTRIBUTE => {
                let table = r::entity_of(store, o).map(|e| name(store, e)).unwrap_or_default();
                entry(&icons::COLUMN, 3, name(store, o), place(tr("Column"), table))
            }
            c if c == archforge_er::RELATIONSHIP => {
                let end = |p| store.get(o, p).ok().and_then(|v| v.as_ref()).map(|e| name(store, e)).unwrap_or_default();
                let ends = format!("{} → {}", end(archforge_er::relationship::FROM), end(archforge_er::relationship::TO));
                let own = name(store, o);
                let label = if own.is_empty() { ends.clone() } else { own };
                entry(&icons::LINK, 4, label, place(tr("Relationship"), ends))
            }
            c if c == archforge_er::DOMAIN => entry(&icons::DOMAIN, 5, name(store, o), place(tr("Domain"), schema_name(o))),
            c if c == archforge_er::ENUM_TYPE => entry(&icons::ENUM, 5, name(store, o), place(tr("Enum type"), schema_name(o))),
            c if c == archforge_er::SEQUENCE => entry(&icons::SEQUENCE, 5, name(store, o), place(tr("Sequence"), schema_name(o))),
            c if c == archforge_er::VIEW => entry(&icons::VIEW, 5, name(store, o), place(tr("View"), schema_name(o))),
            _ => None,
        }
    }

    /// A column's table, a relationship's line's first table.
    fn shown(&self, store: &Store, o: Obj) -> Option<Obj> {
        match class(store, o) {
            Some(c) if c == archforge_er::ATTRIBUTE => r::entity_of(store, o),
            Some(c) if c == archforge_er::RELATIONSHIP => store.get(o, archforge_er::relationship::FROM).ok().and_then(|v| v.as_ref()),
            _ => Some(o),
        }
    }

    /// A table without a primary key, a many-to-many relationship still waiting for its mapping
    /// table, a foreign key the rules refuse, a key column that may be null, a key whose columns
    /// the rules refuse, and the rest the rules find; a column's or a key's on its table.
    fn problems(&self, store: &Store, out: &mut Vec<Problem>) {
        for o in store.alive() {
            let Ok(class) = store.class_of(o) else { continue };
            if class == archforge_er::ENTITY {
                if r::primary_key_of(store, o).is_none() {
                    out.push(Problem { element: o, text: fmt(&tr("{name} has no primary key"), &[("name", &name(store, o))]) });
                }
                for a in children(store, o, archforge_er::entity::ATTRIBUTE) {
                    if r::in_primary_key(store, a) && r::is_nullable(store, a) {
                        out.push(Problem { element: o, text: fmt(&tr("{column} is a key column and may be null"), &[("column", &name(store, a))]) });
                    }
                }
                for k in r::primary_key_of(store, o).into_iter().chain(r::unique_constraints(store, o)) {
                    let primary = store.class_of(k).ok() == Some(archforge_er::PRIMARY_KEY);
                    if let Some(p) = r::key_columns_problem(store, o, &r::key_columns(store, k), primary) {
                        out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::label(store, k)), ("problem", &p)]) });
                    }
                }
                if !r::subtype_links(store, o).is_empty()
                    && r::mapping_of(store, o) == archforge_er::SubtypeMapping::TablePerType
                    && r::primary_key_of(store, o).is_none()
                {
                    out.push(Problem { element: o, text: fmt(&tr("{name} has subtypes but no primary key to share"), &[("name", &name(store, o))]) });
                }
                for a in children(store, o, archforge_er::entity::ATTRIBUTE) {
                    if let Some(p) = r::enum_default_problem(store, a) {
                        out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &name(store, a)), ("problem", &p)]) });
                    }
                }
                for c in r::checks_of(store, o) {
                    if let Some(p) = r::check_problem(&r::text_of(store, c, archforge_er::check_constraint::EXPRESSION)) {
                        out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::display_name(store, c)), ("problem", &p)]) });
                    }
                }
                for ix in r::indexes_of(store, o) {
                    if let Some(p) = r::index_columns_problem(store, o, &r::index_specs(store, ix)) {
                        out.push(Problem {
                            element: o,
                            text: fmt(&tr("{key}: {problem}"), &[("key", &r::display_name(store, ix)), ("problem", &p)]),
                        });
                    }
                }
                for fk in children(store, o, archforge_er::entity::FOREIGN_KEY) {
                    let parts = r::key_parts(store, fk);
                    let text = match (parts.entity, parts.key) {
                        (Some(e), Some(k)) => {
                            r::key_problem(store, e, k, &parts.columns, [parts.on_delete, parts.on_update], parts.relationship).map(|(_, p)| p)
                        }
                        _ => Some(tr("lacks its table or its key")),
                    };
                    if let Some(p) = text {
                        out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::label(store, fk)), ("problem", &p)]) });
                    }
                }
            }
            if class == archforge_er::DOMAIN
                && let Some(p) = r::domain_problem(store, o)
            {
                out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &name(store, o)), ("problem", &p)]) });
            }
            if class == archforge_er::ENUM_TYPE
                && let Some(p) = r::literals_problem(&r::literals_of(store, o))
            {
                out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &name(store, o)), ("problem", &p)]) });
            }
            if class == archforge_er::RELATIONSHIP {
                let end = |p| store.get(o, p).ok().and_then(|v| v.as_ref()).map(|e| name(store, e)).unwrap_or_default();
                let ends = [("from", end(archforge_er::relationship::FROM)), ("to", end(archforge_er::relationship::TO))];
                let ends: Vec<(&str, &str)> = ends.iter().map(|(k, v)| (*k, v.as_str())).collect();
                if r::is_many_to_many(store, o) {
                    out.push(Problem { element: o, text: fmt(&tr("{from} to {to} is many to many"), &ends) });
                } else if r::child_end_one_or_more(store, o) {
                    // Said in the model's words: which table must have at least one of which, why the
                    // database cannot hold it, and what to choose instead.
                    let one_or_more = |p| r::cardinality_of(store, o, p) == Some(archforge_er::Cardinality::OneOrMore);
                    let (one, many) = if one_or_more(archforge_er::relationship::TO_END) {
                        (end(archforge_er::relationship::FROM), end(archforge_er::relationship::TO))
                    } else {
                        (end(archforge_er::relationship::TO), end(archforge_er::relationship::FROM))
                    };
                    #[rustfmt::skip]
                    let said = tr("Every {one} has at least one {many} only on the diagram: no database key can require it. Choose zero or more if a {one} may have none.");
                    let text = fmt(&said, &[("one", one.as_str()), ("many", many.as_str())]);
                    out.push(Problem { element: o, text });
                }
            }
        }
    }

    fn problem_holder(&self, store: &Store, o: Obj) -> Obj {
        if class(store, o) == Some(archforge_er::ATTRIBUTE) { r::entity_of(store, o).unwrap_or(o) } else { o }
    }

    fn words(&self, store: &Store, o: Obj) -> Option<Words> {
        let words = |kind: String, word: String, this: String, on_this: String| Words { kind, word, this, on_this, name: name(store, o) };
        Some(match class(store, o)? {
            c if c == archforge_er::SCHEMA => words(tr("Schema · model"), tr("Schema"), tr("This schema"), tr("On this schema · {n}")),
            c if c == archforge_er::ENTITY => words(tr("Table · model, in every diagram"), tr("Table"), tr("This table"), tr("On this table · {n}")),
            c if c == archforge_er::ATTRIBUTE => words(tr("Column · model"), tr("Column"), tr("This column"), tr("On this column · {n}")),
            c if c == archforge_er::DOMAIN => words(tr("Domain · model"), tr("Domain"), tr("This domain"), tr("On this domain · {n}")),
            c if c == archforge_er::ENUM_TYPE => words(tr("Enum type · model"), tr("Enum type"), tr("This enum type"), tr("On this enum type · {n}")),
            c if c == archforge_er::SEQUENCE => words(tr("Sequence · model"), tr("Sequence"), tr("This sequence"), tr("On this sequence · {n}")),
            c if c == archforge_er::VIEW => words(tr("View · model"), tr("View"), tr("This view"), tr("On this view · {n}")),
            c if c == archforge_er::GENERALIZATION => {
                let (sup, sub, _) = r::generalization_parts(store, o);
                let n = |e: Option<Obj>| e.map(|e| name(store, e)).unwrap_or_default();
                let kind = fmt(&tr("Subtype · {sub} is a {super}"), &[("sub", &n(sub)), ("super", &n(sup))]);
                Words { name: n(sub), ..words(kind, tr("Subtype"), tr("This subtype"), tr("On this subtype · {n}")) }
            }
            c if c == archforge_er::RELATIONSHIP => {
                let end = |p| store.get(o, p).ok().and_then(|v| v.as_ref()).map(|e| name(store, e)).unwrap_or_default();
                let kind = fmt(
                    &tr("Relationship · {from} to {to}"),
                    &[("from", &end(archforge_er::relationship::FROM)), ("to", &end(archforge_er::relationship::TO))],
                );
                words(kind, tr("Relationship"), tr("This relationship"), tr("On this relationship · {n}"))
            }
            _ => return None,
        })
    }

    /// A column's panel is its table's, so its first section says Table.
    fn heading(&self, store: &Store, o: Obj) -> Option<String> {
        (class(store, o) == Some(archforge_er::ATTRIBUTE)).then(|| tr("Table"))
    }

    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
        let width = r.width() - 2.0 * crate::panels::PAD;
        match subject.class {
            Some(c) if c == archforge_er::SCHEMA => {
                crate::panels::name_field(shell, ui, p, egui::pos2(r.left() + crate::panels::PAD, y), width, subject, "er.schema.rename", "schema");
                if let (Some(o), Some(id)) = (subject.obj, subject.id) {
                    let y = crate::types::types_section(shell, ui, p, r, y + 17.0 + FIELD + 16.0, o, id);
                    let y = crate::objects::section_of(shell, ui, p, r, y, o, id, crate::objects::ObjectKind::Sequence);
                    let y = crate::objects::section_of(shell, ui, p, r, y, o, id, crate::objects::ObjectKind::View);
                    crate::keys::problems_section(shell, ui, p, r, y + 4.0, o);
                }
            }
            Some(c) if c == archforge_er::GENERALIZATION => {
                // A generalization: its supertype's Subtypes section.
                let sup = shell.session.as_ref().and_then(|s| {
                    let store = s.doc();
                    let sup = crate::subtypes::supertype_of_link(store, subject.obj?)?;
                    Some((sup, store.uuid(sup).ok()?))
                });
                if let (Some((sup, sup_id)), Some(id)) = (sup, subject.id) {
                    let y = crate::subtypes::subtypes_section(shell, ui, p, r, y, sup, sup_id, id);
                    crate::keys::problems_section(shell, ui, p, r, y + 4.0, sup);
                }
            }
            Some(c) if c == archforge_er::DOMAIN || c == archforge_er::ENUM_TYPE => {
                // A type: its schema's Types section, this type's row marked.
                let schema = shell.session.as_ref().and_then(|s| {
                    let store = s.doc();
                    let schema = r::schema_of(store, subject.obj?)?;
                    Some((schema, store.uuid(schema).ok()?, crate::types::kind_of(store, subject.obj?)?))
                });
                if let Some((schema, schema_id, _)) = schema {
                    let y = crate::types::types_section(shell, ui, p, r, y, schema, schema_id);
                    crate::keys::problems_section(shell, ui, p, r, y + 4.0, schema);
                }
            }
            Some(c) if c == archforge_er::SEQUENCE || c == archforge_er::VIEW => {
                // A sequence or view: its schema's section, its row marked.
                let schema = shell.session.as_ref().and_then(|s| {
                    let store = s.doc();
                    let schema = r::schema_of(store, subject.obj?)?;
                    Some((schema, store.uuid(schema).ok()?, crate::objects::kind_of(store, subject.obj?)?))
                });
                if let Some((schema, schema_id, kind)) = schema {
                    let y = crate::objects::section_of(shell, ui, p, r, y, schema, schema_id, kind);
                    crate::keys::problems_section(shell, ui, p, r, y + 4.0, schema);
                }
            }
            Some(c) if c == archforge_er::ENTITY => crate::panels::entity_body(shell, ui, p, r, y, subject),
            Some(c) if c == archforge_er::RELATIONSHIP => crate::relation::body(shell, ui, p, r, y, subject),
            Some(c) if c == archforge_er::ATTRIBUTE => {
                // A column: its table's panel, the column's row marked.
                let table = shell.session.as_ref().and_then(|s| {
                    let store = s.doc();
                    let e = r::entity_of(store, subject.obj?)?;
                    let words = self.words(store, e)?;
                    Some(Subject::of(e, store.uuid(e).ok()?, archforge_er::ENTITY, words))
                });
                if let Some(table) = table {
                    crate::panels::entity_body(shell, ui, p, r, y, &table);
                }
            }
            _ => {}
        }
    }
}
