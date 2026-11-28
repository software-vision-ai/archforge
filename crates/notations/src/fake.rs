//! A fake notation for the tests of what holds notations without naming one (ADR 0005): the
//! shell's, whose own tests give it this notation's screens part, and the project store's. Its
//! model is archforge-mof's toy metamodel: packages holding classes and the associations between
//! them, a class's attributes, operations and generalizations, an element's description in an owned
//! comment. Its one diagram kind, [`KIND`], is of a package and shows its classes as cards and
//! their generalizations and associations as lines.
//! Its commands add, rename, list and read them; the shell's removal removes them. Kept as small
//! as the interfaces allow; behind the `testing` feature, never shipped.

use archforge_diagram::figure::{Card, End, EndMark, Line, Row, Text, Tone};
use archforge_diagram::{Notation, ProjectView, Removed, Standing};
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, always, cmd};
use archforge_mof::schema::{Column, Junction, Table};
use archforge_mof::toy::{
    ASSOCIATION, ASSOCIATION_MEMBER_END, ASSOCIATION_OWNED_END, CLASS, CLASS_FEATURE, CLASS_GENERALIZATION, CLASS_IS_ABSTRACT, CLASS_OWNED_ATTRIBUTE,
    CLASS_OWNED_OPERATION, COMMENT, COMMENT_ANNOTATED_ELEMENT, COMMENT_BODY, ELEMENT_OWNED_COMMENT, GENERALIZATION, GENERALIZATION_GENERAL, INDEX,
    LITERAL_INTEGER, LITERAL_INTEGER_VALUE, LITERAL_UNLIMITED_NATURAL, LITERAL_UNLIMITED_VALUE, MULTIPLICITY_IS_ORDERED, MULTIPLICITY_IS_UNIQUE,
    MULTIPLICITY_LOWER_VALUE, MULTIPLICITY_UPPER_VALUE, NAMED_ELEMENT_NAME, OPERATION, OPERATION_OWNED_PARAMETER, PACKAGE, PACKAGE_PACKAGED_ELEMENT,
    PROPERTY, PROPERTY_AGGREGATION, PROPERTY_ASSOCIATION, PROPERTY_CLASS, TYPED_ELEMENT_TYPE, ToySlabs,
};
use archforge_mof::{ClassId, Invalid, NameKind, Obj, PropId, Store, Value};
use serde_json::{Value as Json, json};

use crate::{Notations, Part};

/// The fake notation's one diagram kind: a sketch of a package's classes.
pub const KIND: &str = "test.sketch";

/// The most characters a name of the fake notation's takes.
pub const MAX_NAME: usize = 128;

/// The fake notation alone, composed: the toy metamodel's part, the notation and its commands.
pub static NOTATIONS: Notations = Notations::new(PARTS, VIEWS, commands);

/// What the toy metamodel brings to a project's store.
const PARTS: &[Part] = &[Part {
    name: "sketch",
    metamodel: archforge_mof::toy::metamodel,
    index: INDEX,
    slabs: || Box::new(ToySlabs::default()),
    tables: TABLES,
    junctions: JUNCTIONS,
    name_kind: |_| Some(NameKind::Group("")),
}];

/// The notation the view asks.
static VIEWS: &[&dyn Notation] = &[&Sketch];

/// The id column every table starts with.
const ID: Column = Column { name: "id", prop: None, sql: "BINARY(16) NOT NULL PRIMARY KEY" };

/// One table for each class the notation makes, and for the literals a multiplicity's bounds are.
const TABLES: &[Table] = &[
    Table { name: "sketch_package", class: PACKAGE, columns: &[ID, Column { name: "name", prop: Some(NAMED_ELEMENT_NAME), sql: "MEDIUMTEXT NULL" }] },
    Table {
        name: "sketch_class",
        class: CLASS,
        columns: &[
            ID,
            Column { name: "name", prop: Some(NAMED_ELEMENT_NAME), sql: "MEDIUMTEXT NULL" },
            Column { name: "is_abstract", prop: Some(CLASS_IS_ABSTRACT), sql: "TINYINT NOT NULL" },
        ],
    },
    Table {
        name: "sketch_property",
        class: PROPERTY,
        columns: &[
            ID,
            Column { name: "name", prop: Some(NAMED_ELEMENT_NAME), sql: "MEDIUMTEXT NULL" },
            Column { name: "type", prop: Some(TYPED_ELEMENT_TYPE), sql: "BINARY(16) NULL" },
            Column { name: "lower_value", prop: Some(MULTIPLICITY_LOWER_VALUE), sql: "BINARY(16) NULL" },
            Column { name: "upper_value", prop: Some(MULTIPLICITY_UPPER_VALUE), sql: "BINARY(16) NULL" },
            Column { name: "is_ordered", prop: Some(MULTIPLICITY_IS_ORDERED), sql: "TINYINT NOT NULL" },
            Column { name: "is_unique", prop: Some(MULTIPLICITY_IS_UNIQUE), sql: "TINYINT NOT NULL" },
            Column { name: "aggregation", prop: Some(PROPERTY_AGGREGATION), sql: "INT NOT NULL" },
            Column { name: "association", prop: Some(PROPERTY_ASSOCIATION), sql: "BINARY(16) NULL" },
            Column { name: "class", prop: Some(PROPERTY_CLASS), sql: "BINARY(16) NULL" },
        ],
    },
    Table {
        name: "sketch_operation",
        class: OPERATION,
        columns: &[ID, Column { name: "name", prop: Some(NAMED_ELEMENT_NAME), sql: "MEDIUMTEXT NULL" }],
    },
    Table {
        name: "sketch_generalization",
        class: GENERALIZATION,
        columns: &[ID, Column { name: "general", prop: Some(GENERALIZATION_GENERAL), sql: "BINARY(16) NULL" }],
    },
    Table {
        name: "sketch_association",
        class: ASSOCIATION,
        columns: &[ID, Column { name: "name", prop: Some(NAMED_ELEMENT_NAME), sql: "MEDIUMTEXT NULL" }],
    },
    Table { name: "sketch_comment", class: COMMENT, columns: &[ID, Column { name: "body", prop: Some(COMMENT_BODY), sql: "MEDIUMTEXT NULL" }] },
    Table {
        name: "sketch_literal_integer",
        class: LITERAL_INTEGER,
        columns: &[ID, Column { name: "value", prop: Some(LITERAL_INTEGER_VALUE), sql: "BIGINT NOT NULL" }],
    },
    Table {
        name: "sketch_literal_unlimited_natural",
        class: LITERAL_UNLIMITED_NATURAL,
        columns: &[ID, Column { name: "value", prop: Some(LITERAL_UNLIMITED_VALUE), sql: "BIGINT NOT NULL" }],
    },
];

/// One list table for each list the notation fills.
const JUNCTIONS: &[Junction] = &[
    Junction { name: "sketch_package_packaged_element", class: PACKAGE, prop: PACKAGE_PACKAGED_ELEMENT, ordered: false, reference: true },
    Junction { name: "sketch_package_owned_comment", class: PACKAGE, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_class_owned_attribute", class: CLASS, prop: CLASS_OWNED_ATTRIBUTE, ordered: true, reference: true },
    Junction { name: "sketch_class_owned_operation", class: CLASS, prop: CLASS_OWNED_OPERATION, ordered: true, reference: true },
    Junction { name: "sketch_class_generalization", class: CLASS, prop: CLASS_GENERALIZATION, ordered: false, reference: true },
    Junction { name: "sketch_class_feature", class: CLASS, prop: CLASS_FEATURE, ordered: false, reference: true },
    Junction { name: "sketch_class_owned_comment", class: CLASS, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_property_owned_comment", class: PROPERTY, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_operation_owned_parameter", class: OPERATION, prop: OPERATION_OWNED_PARAMETER, ordered: true, reference: true },
    Junction { name: "sketch_operation_owned_comment", class: OPERATION, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_generalization_owned_comment", class: GENERALIZATION, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_association_member_end", class: ASSOCIATION, prop: ASSOCIATION_MEMBER_END, ordered: true, reference: true },
    Junction { name: "sketch_association_owned_end", class: ASSOCIATION, prop: ASSOCIATION_OWNED_END, ordered: true, reference: true },
    Junction { name: "sketch_association_owned_comment", class: ASSOCIATION, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_comment_owned_comment", class: COMMENT, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction { name: "sketch_comment_annotated_element", class: COMMENT, prop: COMMENT_ANNOTATED_ELEMENT, ordered: false, reference: true },
    Junction { name: "sketch_literal_integer_owned_comment", class: LITERAL_INTEGER, prop: ELEMENT_OWNED_COMMENT, ordered: false, reference: true },
    Junction {
        name: "sketch_literal_unlimited_natural_owned_comment",
        class: LITERAL_UNLIMITED_NATURAL,
        prop: ELEMENT_OWNED_COMMENT,
        ordered: false,
        reference: true,
    },
];

/// The fake notation: packages of classes, each sketched as a card with its attributes and its
/// operations, their
/// generalizations as lines with a triangle at the general end and a text at the specific one, and
/// their associations as plain lines that may show their names.
pub struct Sketch;

impl Notation for Sketch {
    /// "Sketch".
    fn name(&self) -> &'static str {
        "Sketch"
    }
    /// The toy metamodel's.
    fn metamodel(&self) -> u16 {
        INDEX
    }
    /// [`KIND`].
    fn kinds(&self) -> &'static [&'static str] {
        &[KIND]
    }
    /// A sketch is of a package.
    fn needs_element(&self, kind: &str) -> bool {
        kind == KIND
    }
    /// Of a package.
    fn is_of(&self, kind: &str, class: ClassId) -> bool {
        kind == KIND && class == PACKAGE
    }
    /// Its classes as cards.
    fn shows(&self, kind: &str, class: ClassId) -> bool {
        kind == KIND && class == CLASS
    }
    /// Their generalizations and associations as lines.
    fn connects(&self, kind: &str, class: ClassId) -> bool {
        kind == KIND && (class == GENERALIZATION || class == ASSOCIATION)
    }
    /// A generalization's specific class, which holds it, then its general one; an association's
    /// classes, its first end's then its second's.
    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        if store.class_of(element).ok()? == ASSOCIATION {
            let typed = |end: &Value| store.get(end.as_ref()?, TYPED_ELEMENT_TYPE).ok()?.as_ref();
            let ends = store.list(element, ASSOCIATION_OWNED_END).ok()?;
            return Some((typed(ends.first()?)?, typed(ends.get(1)?)?));
        }
        let specific = store.container(element).ok()??.0;
        let general = store.get(element, GENERALIZATION_GENERAL).ok()?.as_ref()?;
        Some((specific, general))
    }
    /// An association's line may show its name and each end's role.
    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        if class == ASSOCIATION { &["name", "from_end", "to_end"] } else { &[] }
    }
    /// An association's name, or the role of its first or its second end.
    fn slot_text(&self, store: &Store, element: Obj, slot: &str) -> String {
        let end = |n: usize| {
            let ends = store.list(element, ASSOCIATION_OWNED_END).unwrap_or_default();
            ends.get(n).and_then(Value::as_ref).map(|e| archforge_core::name_of(store, e)).unwrap_or_default()
        };
        match slot {
            "name" => archforge_core::name_of(store, element),
            "from_end" => end(0),
            "to_end" => end(1),
            _ => String::new(),
        }
    }
    /// A class's card: its name, its attributes, then its operations.
    fn card(&self, store: &Store, element: Obj, _: Obj) -> Option<Card> {
        let rows = |prop| {
            store
                .list(element, prop)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_ref)
                .map(|o| Row { mark: None, text: archforge_core::name_of(store, o), linked: false, slanted: false, underlined: false })
                .collect()
        };
        Some(Card {
            eyebrow: Text::Plain("Class".into()),
            name: archforge_core::name_of(store, element),
            compartments: vec![rows(CLASS_OWNED_ATTRIBUTE), rows(CLASS_OWNED_OPERATION)],
            more: ("+ {n} more attribute", "+ {n} more attributes"),
            hidden: 0,
            elsewhere: false,
            tone: Tone::Tinted,
            slanted: false,
            braces: None,
            template: Vec::new(),
            owner_path: Vec::new(),
        })
    }
    /// A generalization's line: a text at the specific end, a triangle at the general one; an
    /// association's: a plain line.
    fn line(&self, store: &Store, element: Obj) -> Option<Line> {
        if store.class_of(element).ok()? == ASSOCIATION {
            return None;
        }
        Some(Line {
            ends: [
                End { mark: EndMark::None, text: Some("specific".into()), slot: "sketch.specific", role: None, qualifiers: Vec::new() },
                End::marked(EndMark::Triangle),
            ],
            styled: true,
            dashed: false,
            status: None,
            note: None,
            badge: true,
            resolvable: false,
            keyword: None,
        })
    }
    /// A skill may sit on a package or a class.
    fn skill_subjects(&self) -> &'static [ClassId] {
        &[PACKAGE, CLASS]
    }
    /// A skill may aim at every class or every attribute.
    fn skill_kinds(&self) -> &'static [(ClassId, &'static str)] {
        &[(CLASS, "class"), (PROPERTY, "attribute")]
    }
    /// Removes packages with their classes and associations, classes with their generalizations and
    /// associations both ways and their attributes, attributes, generalizations and associations,
    /// each with the figures that show it. An association's end goes only with its association:
    /// refused alone, named by its name or, without one, its id, the reason typed.
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<Removed, CommandError> {
        for o in objs {
            if store.container(*o).map_err(fail)?.is_some_and(|(_, prop)| prop == ASSOCIATION_OWNED_END) {
                let name = Some(archforge_core::name_of(store, *o)).filter(|n| !n.is_empty());
                let name = name.unwrap_or_else(|| store.uuid(*o).map(|u| u.to_string()).unwrap_or_default());
                return Err(CommandError::about("ids", format!("{name} cannot be removed yet")).because(Reason::ElementNotRemovable { name }));
            }
            if ![PACKAGE, CLASS, PROPERTY, OPERATION, GENERALIZATION, ASSOCIATION].contains(&store.class_of(*o).map_err(fail)?) {
                return Err(CommandError::about(
                    "ids",
                    "only the sketch's packages, classes, attributes, operations, generalizations and associations can be removed",
                ));
            }
        }
        let standing = Standing::before(store, |c| c == GENERALIZATION || c == ASSOCIATION);
        let mut gone = 0;
        for o in objs {
            if store.is_alive(*o) {
                gone += store.transaction(|st| remove_one(st, *o)).map_err(fail)?;
            }
        }
        Ok(standing.removed(store, gone))
    }
    /// Keeps the description as the body of the element's first owned comment, made for it; an
    /// empty text clears it, and one with a control character other than a line break or a tab
    /// is refused.
    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), CommandError> {
        if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(CommandError::about("text", "the text holds a control character").because(Reason::ControlCharacter));
        }
        let held = described(store, obj);
        let comment = match held {
            Some(c) => c,
            None => {
                let c = store.create(COMMENT).map_err(fail)?;
                store.push(obj, ELEMENT_OWNED_COMMENT, Value::Ref(c)).map_err(fail)?;
                c
            }
        };
        store.set(comment, COMMENT_BODY, if text.is_empty() { Value::Null } else { Value::str(text) }).map_err(fail)
    }
    /// The body of the element's first owned comment.
    fn description(&self, store: &Store, obj: Obj) -> String {
        described(store, obj).and_then(|c| store.get(c, COMMENT_BODY).ok()?.as_str().map(str::to_string)).unwrap_or_default()
    }
}

/// The comment that holds `obj`'s description, its first owned one.
fn described(store: &Store, obj: Obj) -> Option<Obj> {
    store.list(obj, ELEMENT_OWNED_COMMENT).ok()?.first()?.as_ref()
}

/// Removes `obj` with what goes with it and the figures that show it; how many objects went.
fn remove_one(st: &mut Store, obj: Obj) -> Result<usize, Invalid> {
    let mut gone = 0;
    let class = st.class_of(obj)?;
    if class == PACKAGE {
        for c in st.list(obj, PACKAGE_PACKAGED_ELEMENT)?.iter().filter_map(Value::as_ref).collect::<Vec<_>>() {
            if st.is_alive(c) {
                gone += remove_one(st, c)?;
            }
        }
    }
    if class == CLASS {
        let typing_ends: Vec<Obj> = st.referrers(obj).into_iter().filter(|(_, prop)| *prop == TYPED_ELEMENT_TYPE).map(|(e, _)| e).collect();
        let associations =
            typing_ends.into_iter().filter_map(|e| st.container(e).ok().flatten().filter(|(_, p)| *p == ASSOCIATION_OWNED_END).map(|(a, _)| a));
        let generalizations: Vec<Obj> = st
            .referrers(obj)
            .into_iter()
            .filter(|(_, prop)| *prop == GENERALIZATION_GENERAL)
            .map(|(g, _)| g)
            .chain(associations)
            .chain(st.list(obj, CLASS_GENERALIZATION)?.iter().filter_map(Value::as_ref))
            .collect();
        for g in generalizations {
            if st.is_alive(g) {
                gone += remove_one(st, g)?;
            }
        }
    }
    gone += archforge_diagram::remove_views_of(st, obj)?;
    gone += archforge_core::destroy_with_links(st, obj)?;
    Ok(gone)
}

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// A store refusal, as a command's failure.
fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

/// The text under `key`, empty when there is none.
fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

/// The element of `class` the id under `key` names, refused naming the key when it names none.
fn id_of(ctx: &C<'_>, p: &Params, key: &str, class: ClassId) -> Result<Obj, CommandError> {
    let id = text(p, key);
    archforge_mof::Uuid::parse(id)
        .and_then(|u| ctx.doc.by_uuid(u))
        .filter(|o| ctx.doc.is_alive(*o) && ctx.doc.class_of(*o).ok() == Some(class))
        .ok_or_else(|| {
            CommandError::about(
                key,
                format!(
                    "{id:?} names no {} of the sketch",
                    ctx.doc.universe().class(class).map_or_else(|| "element".to_string(), |c| c.name.to_string())
                ),
            )
        })
}

/// The id of `obj` as the commands answer it.
fn answer(ctx: &C<'_>, obj: Obj) -> R {
    Ok(json!({ "id": ctx.doc.uuid(obj).map_err(fail)?.to_string() }))
}

/// Refuses a name the shared name rules refuse under `key`, and one another element of `list` of
/// `owner` has, naming the
/// element of `kind` that has it.
fn named(ctx: &C<'_>, key: &str, name: &str, owner: Obj, list: PropId, except: Option<Obj>, kind: &'static str) -> Result<(), CommandError> {
    if let Some(problem) = archforge_core::skills::name_problem(name, MAX_NAME) {
        return Err(CommandError::about(key, format!("the name {problem}")).because(problem.reason));
    }
    let taken = ctx.doc.list(owner, list).map_err(fail)?.iter().filter_map(Value::as_ref).any(|o| {
        Some(o) != except && ctx.doc.get(o, NAMED_ELEMENT_NAME).ok().and_then(|v| v.as_str().map(str::to_lowercase)) == Some(name.to_lowercase())
    });
    if taken {
        Err(CommandError::about(key, format!("{name:?} is taken")).because(Reason::NameTaken { kind, name: name.to_string() }))
    } else {
        Ok(())
    }
}

/// The project's model root.
fn model(ctx: &C<'_>) -> Result<Obj, CommandError> {
    ctx.doc
        .alive()
        .find(|o| ctx.doc.class_of(*o).ok() == Some(archforge_core::MODEL))
        .ok_or_else(|| CommandError::new("the project has no model root"))
}

/// Whether the store holds an element of `class`.
fn any_of(store: &Store, class: ClassId) -> bool {
    store.alive().any(|o| store.class_of(o).ok() == Some(class))
}

/// Available once there is a package.
fn has_packages(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if any_of(f.doc, PACKAGE) { Ok(()) } else { Err("There is no package yet.") }
}

/// Available once there is a class.
fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if any_of(f.doc, CLASS) { Ok(()) } else { Err("There is no class yet.") }
}

/// `sketch.package.add`: a package of the model, selected.
fn package_add(ctx: &mut C<'_>, p: &Params) -> R {
    let name = text(p, "name");
    let m = model(ctx)?;
    named(ctx, "name", name, m, archforge_core::model::MEMBER, None, "package")?;
    let o = ctx.doc.create(PACKAGE).map_err(fail)?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    ctx.doc.push(m, archforge_core::model::MEMBER, Value::Ref(o)).map_err(fail)?;
    ctx.view.selection = vec![o];
    answer(ctx, o)
}

/// `sketch.package.rename`.
fn package_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let o = id_of(ctx, p, "package", PACKAGE)?;
    let name = text(p, "name");
    named(ctx, "name", name, model(ctx)?, archforge_core::model::MEMBER, Some(o), "package")?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    answer(ctx, o)
}

/// `sketch.package.list`: every package, its id and name.
fn package_list(ctx: &mut C<'_>, _: &Params) -> R {
    let st = &*ctx.doc;
    let all = st.alive().filter(|o| st.class_of(*o).ok() == Some(PACKAGE));
    Ok(Json::Array(
        all.map(|o| json!({ "id": st.uuid(o).map(|u| u.to_string()).unwrap_or_default(), "name": archforge_core::name_of(st, o) })).collect(),
    ))
}

/// `sketch.class.add`: a class of the package, selected.
fn class_add(ctx: &mut C<'_>, p: &Params) -> R {
    let package = id_of(ctx, p, "package", PACKAGE)?;
    let name = text(p, "name");
    named(ctx, "name", name, package, PACKAGE_PACKAGED_ELEMENT, None, "class")?;
    let o = ctx.doc.create(CLASS).map_err(fail)?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    ctx.doc.push(package, PACKAGE_PACKAGED_ELEMENT, Value::Ref(o)).map_err(fail)?;
    ctx.view.selection = vec![o];
    answer(ctx, o)
}

/// `sketch.class.rename`.
fn class_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let o = id_of(ctx, p, "class", CLASS)?;
    let name = text(p, "name");
    if let Some((package, _)) = ctx.doc.container(o).map_err(fail)? {
        named(ctx, "name", name, package, PACKAGE_PACKAGED_ELEMENT, Some(o), "class")?;
    }
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    answer(ctx, o)
}

/// `sketch.class.list`: the package's classes, each its id and name.
fn class_list(ctx: &mut C<'_>, p: &Params) -> R {
    let package = id_of(ctx, p, "package", PACKAGE)?;
    let st = &*ctx.doc;
    let all: Vec<Obj> = st
        .list(package, PACKAGE_PACKAGED_ELEMENT)
        .map_err(fail)?
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| st.class_of(*o).ok() == Some(CLASS))
        .collect();
    Ok(Json::Array(
        all.into_iter()
            .map(|o| json!({ "id": st.uuid(o).map(|u| u.to_string()).unwrap_or_default(), "name": archforge_core::name_of(st, o) }))
            .collect(),
    ))
}

/// `sketch.class.get`: the class's id and name, and each of its attributes' and operations'.
fn class_get(ctx: &mut C<'_>, p: &Params) -> R {
    let class = id_of(ctx, p, "class", CLASS)?;
    let st = &*ctx.doc;
    let named = |o: Obj| json!({ "id": st.uuid(o).map(|u| u.to_string()).unwrap_or_default(), "name": archforge_core::name_of(st, o) });
    let list = |prop| -> Result<Json, CommandError> {
        Ok(Json::Array(st.list(class, prop).map_err(fail)?.iter().filter_map(Value::as_ref).map(named).collect()))
    };
    let mut out = named(class);
    out["attributes"] = list(CLASS_OWNED_ATTRIBUTE)?;
    out["operations"] = list(CLASS_OWNED_OPERATION)?;
    Ok(out)
}

/// `sketch.attribute.add`: an attribute at the end of the class's.
fn attribute_add(ctx: &mut C<'_>, p: &Params) -> R {
    let class = id_of(ctx, p, "class", CLASS)?;
    let name = text(p, "name");
    named(ctx, "name", name, class, CLASS_OWNED_ATTRIBUTE, None, "attribute")?;
    let o = ctx.doc.create(PROPERTY).map_err(fail)?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    ctx.doc.push(class, CLASS_OWNED_ATTRIBUTE, Value::Ref(o)).map_err(fail)?;
    answer(ctx, o)
}

/// `sketch.operation.add`: an operation at the end of the class's.
fn operation_add(ctx: &mut C<'_>, p: &Params) -> R {
    let class = id_of(ctx, p, "class", CLASS)?;
    let name = text(p, "name");
    named(ctx, "name", name, class, CLASS_OWNED_OPERATION, None, "operation")?;
    let o = ctx.doc.create(OPERATION).map_err(fail)?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, Value::str(name)).map_err(fail)?;
    ctx.doc.push(class, CLASS_OWNED_OPERATION, Value::Ref(o)).map_err(fail)?;
    answer(ctx, o)
}

/// `sketch.inherit.add`: the specific class becomes a kind of the general one.
fn inherit_add(ctx: &mut C<'_>, p: &Params) -> R {
    let specific = id_of(ctx, p, "specific", CLASS)?;
    let general = id_of(ctx, p, "general", CLASS)?;
    if specific == general {
        return Err(CommandError::about("general", "a class is no kind of itself"));
    }
    let g = ctx.doc.create(GENERALIZATION).map_err(fail)?;
    ctx.doc.set(g, GENERALIZATION_GENERAL, Value::Ref(general)).map_err(fail)?;
    ctx.doc.push(specific, CLASS_GENERALIZATION, Value::Ref(g)).map_err(fail)?;
    answer(ctx, g)
}

/// `sketch.association.add`: an association between two classes, in the first one's package, each
/// end typed by its class and named by its role when one is given; its id and its two ends' ids.
fn association_add(ctx: &mut C<'_>, p: &Params) -> R {
    let from = id_of(ctx, p, "from", CLASS)?;
    let to = id_of(ctx, p, "to", CLASS)?;
    let package = ctx.doc.container(from).map_err(fail)?.map(|(o, _)| o).ok_or_else(|| CommandError::about("from", "the class is in no package"))?;
    let a = ctx.doc.create(ASSOCIATION).map_err(fail)?;
    if !text(p, "name").is_empty() {
        ctx.doc.set(a, NAMED_ELEMENT_NAME, Value::str(text(p, "name"))).map_err(fail)?;
    }
    let mut ends = Vec::new();
    for (class, role) in [(from, text(p, "from_role")), (to, text(p, "to_role"))] {
        let end = ctx.doc.create(PROPERTY).map_err(fail)?;
        ctx.doc.set(end, TYPED_ELEMENT_TYPE, Value::Ref(class)).map_err(fail)?;
        if !role.is_empty() {
            ctx.doc.set(end, NAMED_ELEMENT_NAME, Value::str(role)).map_err(fail)?;
        }
        ctx.doc.push(a, ASSOCIATION_OWNED_END, Value::Ref(end)).map_err(fail)?;
        ends.push(ctx.doc.uuid(end).map_err(fail)?.to_string());
    }
    ctx.doc.push(package, PACKAGE_PACKAGED_ELEMENT, Value::Ref(a)).map_err(fail)?;
    Ok(json!({ "id": ctx.doc.uuid(a).map_err(fail)?.to_string(), "ends": ends }))
}

/// The fake notation's commands.
fn commands() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!("sketch.package.add", "Add package", &["Package"], None, "name: string; the package's name", always, package_add),
        cmd!(
            "sketch.package.rename",
            "Rename package",
            &["Package"],
            None,
            "package: string; the package's id\nname: string; the new name",
            has_packages,
            package_rename
        ),
        cmd!(query "sketch.package.list", "List packages", &["Package"], None, "", always, package_list),
        cmd!(
            "sketch.class.add",
            "Add class",
            &["Class"],
            None,
            "package: string; the package's id\nname: string; the class's name",
            has_packages,
            class_add
        ),
        cmd!(
            "sketch.class.rename",
            "Rename class",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the new name",
            has_classes,
            class_rename
        ),
        cmd!(query "sketch.class.list", "List classes", &["Class"], None, "package: string; the package's id", has_packages, class_list),
        cmd!(query "sketch.class.get", "Get class", &["Class"], None, "class: string; the class's id", has_classes, class_get),
        cmd!(
            "sketch.attribute.add",
            "Add attribute",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the attribute's name",
            has_classes,
            attribute_add
        ),
        cmd!(
            "sketch.operation.add",
            "Add operation",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the operation's name",
            has_classes,
            operation_add
        ),
        cmd!(
            "sketch.inherit.add",
            "Add generalization",
            &["Class"],
            None,
            "specific: string; the specific class's id\ngeneral: string; the general class's id",
            has_classes,
            inherit_add
        ),
        cmd!(
            "sketch.association.add",
            "Add association",
            &["Class"],
            None,
            "from: string; the first class's id\nto: string; the second class's id\nname: string = \"\"; the association's name\nfrom_role: string = \"\"; the first end's role\nto_role: string = \"\"; the second end's role",
            has_classes,
            association_add
        ),
    ]
}
