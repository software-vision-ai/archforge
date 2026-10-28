//! The ER notation's commands. Each validates what it is given against the model before it
//! changes anything, keeps the model's invariants, and refuses with a message naming the key.

use std::collections::BTreeSet;

use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use serde_json::{Value, json};

use crate::model::*;

type C<'a> = Ctx<'a, ErModel, ErView>;
type R = Result<Value, CommandError>;

/// Why a command is not available.
pub const NO_ENTITIES: &str = "There is no entity yet.";
pub const NO_DIAGRAMS: &str = "There is no diagram yet.";
/// The size a box is given when none is.
pub const DEFAULT_WIDTH: f64 = 200.0;
pub const DEFAULT_HEIGHT: f64 = 120.0;

fn has_entities(f: &Facts<'_, ErModel, ErView>) -> Result<(), &'static str> {
    if f.doc.entities.is_empty() { Err(NO_ENTITIES) } else { Ok(()) }
}

fn has_diagrams(f: &Facts<'_, ErModel, ErView>) -> Result<(), &'static str> {
    if f.doc.diagrams.is_empty() { Err(NO_DIAGRAMS) } else { Ok(()) }
}

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

/// The id given for `key`, checked to be of `kind` and in the model.
fn id_of(ctx: &C<'_>, p: &Params, key: &str, kind: Kind) -> Result<Id, CommandError> {
    let id = text(p, key);
    match parse_id(id) {
        Some((k, _)) if k == kind && ctx.doc.contains(id) => Ok(id.to_string()),
        Some((k, _)) if k == kind => Err(refuse(key, format!("there is no {id}"))),
        _ => Err(refuse(key, format!("{id:?} is not {} id", kind.name()))),
    }
}

/// A new id of `kind` from the counter.
fn new_id(ctx: &mut C<'_>, kind: Kind) -> Result<Id, CommandError> {
    ctx.fresh().map(|n| make_id(kind, n)).ok_or_else(|| CommandError::new("the model has used every id it can make"))
}

fn check_name(key: &str, name: &str, max: usize) -> Result<(), CommandError> {
    match name_problem(name, max) {
        Some(p) => Err(refuse(key, format!("the name {p}"))),
        None => Ok(()),
    }
}

fn number(p: &Params, key: &str) -> f64 {
    p.number(key).unwrap_or(0.0)
}

fn integer(p: &Params, key: &str) -> i64 {
    p.integer(key).unwrap_or(0)
}

/// The nullable choice: auto makes a key not nullable and anything else nullable.
fn nullable(p: &Params, key: bool) -> Result<bool, CommandError> {
    match text(p, "nullable") {
        "yes" if key => Err(refuse("nullable", "a primary key cannot be nullable")),
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Ok(!key),
    }
}

fn entity_mut<'a>(ctx: &'a mut C<'_>, id: &str) -> Result<&'a mut Entity, CommandError> {
    ctx.doc.entities.get_mut(id).ok_or_else(|| CommandError::new(format!("there is no {id}")))
}

/// The entity and position of an attribute, for changing it.
fn attribute_mut<'a>(ctx: &'a mut C<'_>, id: &str) -> Result<&'a mut Attribute, CommandError> {
    ctx.doc
        .entities
        .values_mut()
        .find_map(|e| e.attributes.iter_mut().find(|a| a.id == id))
        .ok_or_else(|| CommandError::new(format!("there is no {id}")))
}

fn entity_name_free(doc: &ErModel, name: &str, except: Option<&str>) -> Result<(), CommandError> {
    if doc.entities.values().any(|e| Some(e.id.as_str()) != except && same_name(&e.name, name)) {
        return Err(refuse("name", format!("an entity is already called {name:?}")));
    }
    Ok(())
}

fn diagram_name_free(doc: &ErModel, name: &str, except: Option<&str>) -> Result<(), CommandError> {
    if doc.diagrams.values().any(|d| Some(d.id.as_str()) != except && same_name(&d.name, name)) {
        return Err(refuse("name", format!("a diagram is already called {name:?}")));
    }
    Ok(())
}

fn place_ok(x: f64, y: f64, width: f64, height: f64) -> Result<(), CommandError> {
    if !(coord_ok(x) && coord_ok(y)) {
        return Err(refuse("x", "the place is out of range"));
    }
    if !(size_ok(width) && size_ok(height)) {
        return Err(refuse("width", "the size is out of range"));
    }
    Ok(())
}

fn places_left(doc: &ErModel) -> Result<(), CommandError> {
    if doc.counts().1 >= MAX_PLACES { Err(CommandError::new(format!("the model holds {MAX_PLACES} places already"))) } else { Ok(()) }
}

// Entities.

fn entity_add(ctx: &mut C<'_>, p: &Params) -> R {
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    entity_name_free(ctx.doc, &name, None)?;
    if ctx.doc.entities.len() >= MAX_ENTITIES {
        return Err(CommandError::new(format!("the model holds {MAX_ENTITIES} entities already")));
    }
    let diagram = match text(p, "diagram") {
        "" => None,
        _ => Some(id_of(ctx, p, "diagram", Kind::Diagram)?),
    };
    let (x, y) = (number(p, "x"), number(p, "y"));
    if diagram.is_some() {
        place_ok(x, y, DEFAULT_WIDTH, DEFAULT_HEIGHT)?;
        places_left(ctx.doc)?;
    }
    let id = new_id(ctx, Kind::Entity)?;
    ctx.doc.entities.insert(id.clone(), Entity { id: id.clone(), name, attributes: Vec::new() });
    if let Some(d) = diagram.and_then(|d| ctx.doc.diagrams.get_mut(&d)) {
        d.places.insert(id.clone(), Place { x, y, width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT });
    }
    ctx.view.selection = vec![id.clone()];
    Ok(json!({ "id": id }))
}

fn entity_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "entity", Kind::Entity)?;
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    entity_name_free(ctx.doc, &name, Some(&id))?;
    entity_mut(ctx, &id)?.name = name;
    Ok(json!({ "id": id }))
}

/// Removes an entity with its relationships and its places.
fn remove_entity(doc: &mut ErModel, id: &str) {
    doc.entities.remove(id);
    doc.relationships.retain(|_, r| r.from != id && r.to != id);
    for d in doc.diagrams.values_mut() {
        d.places.remove(id);
    }
}

fn entity_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "entity", Kind::Entity)?;
    remove_entity(ctx.doc, &id);
    Ok(json!({ "removed": id }))
}

// Attributes.

fn attribute_add(ctx: &mut C<'_>, p: &Params) -> R {
    let entity = id_of(ctx, p, "entity", Kind::Entity)?;
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    let kind = text(p, "kind").to_string();
    if let Some(problem) = name_problem(&kind, MAX_KIND) {
        return Err(refuse("kind", format!("the kind {problem}")));
    }
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key)?;
    if ctx.doc.counts().0 >= MAX_ATTRIBUTES {
        return Err(CommandError::new(format!("the model holds {MAX_ATTRIBUTES} attributes already")));
    }
    let e = ctx.doc.entities.get(&entity).ok_or_else(|| refuse("entity", format!("there is no {entity}")))?;
    if e.attributes.len() >= MAX_ATTRIBUTES_PER_ENTITY {
        return Err(refuse("entity", format!("{entity} holds {MAX_ATTRIBUTES_PER_ENTITY} attributes already")));
    }
    if e.attributes.iter().any(|a| same_name(&a.name, &name)) {
        return Err(refuse("name", format!("{entity} already has an attribute called {name:?}")));
    }
    let len = e.attributes.len();
    let at = match integer(p, "at") {
        -1 => len,
        n => usize::try_from(n)
            .ok()
            .filter(|n| *n <= len)
            .ok_or_else(|| refuse("at", format!("the position must be from 0 to {len}, or -1 for the end")))?,
    };
    let id = new_id(ctx, Kind::Attribute)?;
    let attribute = Attribute { id: id.clone(), name, kind, primary_key: key, nullable: null, unique: p.bool("unique").unwrap_or(false) };
    entity_mut(ctx, &entity)?.attributes.insert(at, attribute);
    Ok(json!({ "id": id }))
}

fn attribute_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "attribute", Kind::Attribute)?;
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    let (e, _) = ctx.doc.attribute(&id).ok_or_else(|| refuse("attribute", format!("there is no {id}")))?;
    if e.attributes.iter().any(|a| a.id != id && same_name(&a.name, &name)) {
        return Err(refuse("name", format!("{} already has an attribute called {name:?}", e.id)));
    }
    attribute_mut(ctx, &id)?.name = name;
    Ok(json!({ "id": id }))
}

fn attribute_set_kind(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "attribute", Kind::Attribute)?;
    let kind = text(p, "kind").to_string();
    if let Some(problem) = name_problem(&kind, MAX_KIND) {
        return Err(refuse("kind", format!("the kind {problem}")));
    }
    attribute_mut(ctx, &id)?.kind = kind;
    Ok(json!({ "id": id }))
}

fn attribute_set_flags(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "attribute", Kind::Attribute)?;
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key)?;
    let unique = p.bool("unique").unwrap_or(false);
    let a = attribute_mut(ctx, &id)?;
    a.primary_key = key;
    a.nullable = null;
    a.unique = unique;
    Ok(json!({ "id": id, "nullable": null }))
}

fn attribute_move(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "attribute", Kind::Attribute)?;
    let (e, from) = ctx.doc.attribute(&id).ok_or_else(|| refuse("attribute", format!("there is no {id}")))?;
    let (entity, last) = (e.id.clone(), e.attributes.len().saturating_sub(1));
    let to = usize::try_from(integer(p, "to"))
        .ok()
        .filter(|t| *t <= last)
        .ok_or_else(|| refuse("to", format!("the position must be from 0 to {last}")))?;
    let e = entity_mut(ctx, &entity)?;
    let a = e.attributes.remove(from);
    e.attributes.insert(to, a);
    Ok(json!({ "id": id, "to": to }))
}

fn attribute_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "attribute", Kind::Attribute)?;
    for e in ctx.doc.entities.values_mut() {
        e.attributes.retain(|a| a.id != id);
    }
    Ok(json!({ "removed": id }))
}

// Relationships.

fn cardinality(p: &Params, key: &str) -> Result<Cardinality, CommandError> {
    Cardinality::parse(text(p, key)).ok_or_else(|| refuse(key, format!("must be one of {}", Cardinality::CHOICES.replace('|', ", "))))
}

fn relationship_name(p: &Params) -> Result<Option<String>, CommandError> {
    match text(p, "name") {
        "" => Ok(None),
        name => {
            check_name("name", name, MAX_NAME)?;
            Ok(Some(name.to_string()))
        }
    }
}

fn relationship_add(ctx: &mut C<'_>, p: &Params) -> R {
    let from = id_of(ctx, p, "from", Kind::Entity)?;
    let to = id_of(ctx, p, "to", Kind::Entity)?;
    let (from_end, to_end) = (cardinality(p, "from_end")?, cardinality(p, "to_end")?);
    let name = relationship_name(p)?;
    if ctx.doc.relationships.len() >= MAX_RELATIONSHIPS {
        return Err(CommandError::new(format!("the model holds {MAX_RELATIONSHIPS} relationships already")));
    }
    let id = new_id(ctx, Kind::Relationship)?;
    ctx.doc.relationships.insert(id.clone(), Relationship { id: id.clone(), name, from, to, from_end, to_end });
    Ok(json!({ "id": id }))
}

fn relationship_set_ends(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "relationship", Kind::Relationship)?;
    let (from_end, to_end) = (cardinality(p, "from_end")?, cardinality(p, "to_end")?);
    let r = ctx.doc.relationships.get_mut(&id).ok_or_else(|| refuse("relationship", format!("there is no {id}")))?;
    r.from_end = from_end;
    r.to_end = to_end;
    Ok(json!({ "id": id }))
}

fn relationship_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "relationship", Kind::Relationship)?;
    let name = relationship_name(p)?;
    ctx.doc.relationships.get_mut(&id).ok_or_else(|| refuse("relationship", format!("there is no {id}")))?.name = name;
    Ok(json!({ "id": id }))
}

fn relationship_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "relationship", Kind::Relationship)?;
    ctx.doc.relationships.remove(&id);
    Ok(json!({ "removed": id }))
}

/// The most ids a list may hold: one of every element the caps allow.
const MAX_LIST: usize = MAX_ENTITIES + MAX_ATTRIBUTES + MAX_RELATIONSHIPS + MAX_DIAGRAMS;

/// The ids of an array parameter: strings, none twice, at most [`MAX_LIST`].
fn id_list(p: &Params, key: &str) -> Result<Vec<String>, CommandError> {
    let items = p.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    if items.len() > MAX_LIST {
        return Err(refuse(key, format!("at most {MAX_LIST} ids")));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len().min(MAX_ENTITIES));
    for v in &items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string"))?;
        if !seen.insert(id.to_string()) {
            return Err(refuse(key, format!("{id} is listed twice")));
        }
        out.push(id.to_string());
    }
    Ok(out)
}

/// Removes entities, attributes and relationships at once: the selection's delete.
fn remove_many(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(p, "ids")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to remove"));
    }
    let attributes = ctx.doc.attribute_ids();
    for id in &ids {
        match parse_id(id).map(|(k, _)| k) {
            Some(Kind::Entity | Kind::Attribute | Kind::Relationship) if ctx.doc.contains_with(id, Some(&attributes)) => {}
            Some(Kind::Diagram) => return Err(refuse("ids", format!("{id} is a diagram; remove it with er.diagram.remove"))),
            _ => return Err(refuse("ids", format!("there is no {id}"))),
        }
    }
    drop(attributes);
    // Relationships and attributes first, then entities, so a cascade finds nothing missing.
    let mut gone = BTreeSet::new();
    for id in &ids {
        match parse_id(id).map(|(k, _)| k) {
            Some(Kind::Relationship) => {
                ctx.doc.relationships.remove(id);
            }
            Some(Kind::Attribute) => {
                gone.insert(id.as_str());
            }
            _ => {}
        }
    }
    if !gone.is_empty() {
        for e in ctx.doc.entities.values_mut() {
            e.attributes.retain(|a| !gone.contains(a.id.as_str()));
        }
    }
    for id in ids.iter().filter(|id| id.starts_with('e')) {
        remove_entity(ctx.doc, id);
    }
    Ok(json!({ "removed": ids }))
}

// Diagrams.

fn diagram_add(ctx: &mut C<'_>, p: &Params) -> R {
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    diagram_name_free(ctx.doc, &name, None)?;
    if ctx.doc.diagrams.len() >= MAX_DIAGRAMS {
        return Err(CommandError::new(format!("the model holds {MAX_DIAGRAMS} diagrams already")));
    }
    let id = new_id(ctx, Kind::Diagram)?;
    ctx.doc.diagrams.insert(id.clone(), Diagram { id: id.clone(), name, places: Default::default() });
    ctx.view.diagram = Some(id.clone());
    Ok(json!({ "id": id }))
}

fn diagram_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let name = text(p, "name").to_string();
    check_name("name", &name, MAX_NAME)?;
    diagram_name_free(ctx.doc, &name, Some(&id))?;
    ctx.doc.diagrams.get_mut(&id).ok_or_else(|| refuse("diagram", format!("there is no {id}")))?.name = name;
    Ok(json!({ "id": id }))
}

fn diagram_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "diagram", Kind::Diagram)?;
    ctx.doc.diagrams.remove(&id);
    Ok(json!({ "removed": id }))
}

fn diagram_place(ctx: &mut C<'_>, p: &Params) -> R {
    let diagram = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let entity = id_of(ctx, p, "entity", Kind::Entity)?;
    let place = Place { x: number(p, "x"), y: number(p, "y"), width: number(p, "width"), height: number(p, "height") };
    place_ok(place.x, place.y, place.width, place.height)?;
    places_left(ctx.doc)?;
    let d = ctx.doc.diagrams.get_mut(&diagram).ok_or_else(|| refuse("diagram", format!("there is no {diagram}")))?;
    if d.places.contains_key(&entity) {
        return Err(refuse("entity", format!("{entity} is already on {diagram}")));
    }
    d.places.insert(entity.clone(), place);
    Ok(json!({ "diagram": diagram, "entity": entity }))
}

fn diagram_move(ctx: &mut C<'_>, p: &Params) -> R {
    let diagram = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let entities = id_list(p, "entities")?;
    if entities.is_empty() {
        return Err(refuse("entities", "nothing to move"));
    }
    let (dx, dy) = (number(p, "dx"), number(p, "dy"));
    let d = ctx.doc.diagrams.get_mut(&diagram).ok_or_else(|| refuse("diagram", format!("there is no {diagram}")))?;
    // Every box is checked before any moves.
    for e in &entities {
        let place = d.places.get(e).ok_or_else(|| refuse("entities", format!("{e} is not on {diagram}")))?;
        if !(coord_ok(place.x + dx) && coord_ok(place.y + dy)) {
            return Err(refuse("dx", format!("{e} would leave the diagram's range")));
        }
    }
    for e in &entities {
        if let Some(place) = d.places.get_mut(e) {
            place.x += dx;
            place.y += dy;
        }
    }
    Ok(json!({ "moved": entities }))
}

fn diagram_resize(ctx: &mut C<'_>, p: &Params) -> R {
    let diagram = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let entity = id_of(ctx, p, "entity", Kind::Entity)?;
    let (width, height) = (number(p, "width"), number(p, "height"));
    if !(size_ok(width) && size_ok(height)) {
        return Err(refuse("width", "the size is out of range"));
    }
    let place = ctx
        .doc
        .diagrams
        .get_mut(&diagram)
        .and_then(|d| d.places.get_mut(&entity))
        .ok_or_else(|| refuse("entity", format!("{entity} is not on {diagram}")))?;
    place.width = width;
    place.height = height;
    Ok(json!({ "entity": entity }))
}

fn diagram_unplace(ctx: &mut C<'_>, p: &Params) -> R {
    let diagram = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let entity = id_of(ctx, p, "entity", Kind::Entity)?;
    let d = ctx.doc.diagrams.get_mut(&diagram).ok_or_else(|| refuse("diagram", format!("there is no {diagram}")))?;
    if d.places.remove(&entity).is_none() {
        return Err(refuse("entity", format!("{entity} is not on {diagram}")));
    }
    Ok(json!({ "diagram": diagram, "entity": entity }))
}

// The view.

fn open_diagram(ctx: &mut C<'_>, p: &Params) -> R {
    ctx.view.diagram = match text(p, "diagram") {
        "" => None,
        _ => Some(id_of(ctx, p, "diagram", Kind::Diagram)?),
    };
    Ok(Value::Null)
}

fn select(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(p, "ids")?;
    let attributes = ids.iter().any(|id| id.starts_with('a')).then(|| ctx.doc.attribute_ids());
    if let Some(missing) = ids.iter().find(|id| !ctx.doc.contains_with(id, attributes.as_ref())) {
        return Err(refuse("ids", format!("there is no {missing}")));
    }
    ctx.view.selection = ids;
    Ok(Value::Null)
}

// Queries.

fn entity_list(ctx: &mut C<'_>, _: &Params) -> R {
    Ok(Value::Array(ctx.doc.entities.values().map(|e| json!({ "id": e.id, "name": e.name })).collect()))
}

fn diagram_list(ctx: &mut C<'_>, _: &Params) -> R {
    Ok(Value::Array(ctx.doc.diagrams.values().map(|d| json!({ "id": d.id, "name": d.name, "places": d.places.len() })).collect()))
}

fn entity_get(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "entity", Kind::Entity)?;
    let e = ctx.doc.entities.get(&id).ok_or_else(|| refuse("entity", format!("there is no {id}")))?;
    serde_json::to_value(e).map_err(|e| CommandError::new(e.to_string()))
}

fn diagram_get(ctx: &mut C<'_>, p: &Params) -> R {
    let id = id_of(ctx, p, "diagram", Kind::Diagram)?;
    let d = ctx.doc.diagrams.get(&id).ok_or_else(|| refuse("diagram", format!("there is no {id}")))?;
    serde_json::to_value(d).map_err(|e| CommandError::new(e.to_string()))
}

fn model_get(ctx: &mut C<'_>, _: &Params) -> R {
    serde_json::to_value(&*ctx.doc).map_err(|e| CommandError::new(e.to_string()))
}

/// The ER notation's command table, after the edit area.
pub fn specs() -> Vec<CommandSpec<ErModel, ErView>> {
    let mut s = archforge_engine::cmd::edit::specs();
    s.extend([
        cmd!("er.entity.add", "Add entity", &["Model"], Some("Ctrl+E"),
            "name: string; the entity's name\ndiagram: string = \"\"; a diagram to place it in, none when empty\nx: number = 0, -1000000..1000000; where in that diagram\ny: number = 0, -1000000..1000000; where in that diagram",
            always, entity_add),
        cmd!("er.entity.rename", "Rename entity", &["Model"], None, "entity: string; the entity's id\nname: string; its new name", has_entities, entity_rename),
        cmd!("er.entity.remove", "Remove entity", &["Model"], None, "entity: string; the entity's id, removed with its relationships and places", has_entities, entity_remove),
        cmd!("er.attribute.add", "Add attribute", &["Model"], None,
            "entity: string; the entity's id\nname: string; the attribute's name\nkind: string = \"TEXT\"; the column type as written\nprimary_key: bool = false; whether it is part of the key\nnullable: string = \"auto\", auto|yes|no; auto: nullable unless it is a key\nunique: bool = false; whether its values are unique\nat: integer = -1, -1..199; its position, the end when -1",
            has_entities, attribute_add),
        cmd!("er.attribute.rename", "Rename attribute", &["Model"], None, "attribute: string; the attribute's id\nname: string; its new name", has_entities, attribute_rename),
        cmd!("er.attribute.set-kind", "Change attribute type", &["Model"], None, "attribute: string; the attribute's id\nkind: string; the column type as written", has_entities, attribute_set_kind),
        cmd!("er.attribute.set-flags", "Change attribute flags", &["Model"], None,
            "attribute: string; the attribute's id\nprimary_key: bool; whether it is part of the key\nnullable: string, auto|yes|no; auto: nullable unless it is a key\nunique: bool; whether its values are unique",
            has_entities, attribute_set_flags),
        cmd!("er.attribute.move", "Move attribute", &["Model"], None, "attribute: string; the attribute's id\nto: integer, 0..199; its new position", has_entities, attribute_move),
        cmd!("er.attribute.remove", "Remove attribute", &["Model"], None, "attribute: string; the attribute's id", has_entities, attribute_remove),
        cmd!("er.relationship.add", "Add relationship", &["Model"], None,
            "from: string; the id of the entity at one end\nto: string; the id of the entity at the other end\nfrom_end: string, zero-or-one|exactly-one|zero-or-more|one-or-more; how many take part at the from end\nto_end: string, zero-or-one|exactly-one|zero-or-more|one-or-more; how many take part at the to end\nname: string = \"\"; its name, none when empty",
            has_entities, relationship_add),
        cmd!("er.relationship.set-ends", "Change relationship ends", &["Model"], None,
            "relationship: string; the relationship's id\nfrom_end: string, zero-or-one|exactly-one|zero-or-more|one-or-more; how many take part at the from end\nto_end: string, zero-or-one|exactly-one|zero-or-more|one-or-more; how many take part at the to end",
            has_entities, relationship_set_ends),
        cmd!("er.relationship.rename", "Rename relationship", &["Model"], None, "relationship: string; the relationship's id\nname: string; its new name, none when empty", has_entities, relationship_rename),
        cmd!("er.relationship.remove", "Remove relationship", &["Model"], None, "relationship: string; the relationship's id", has_entities, relationship_remove),
        cmd!("er.remove", "Delete", &["Model"], Some("Delete"), "ids: array; the entities, attributes and relationships to remove", has_entities, remove_many),
        cmd!("er.diagram.add", "Add diagram", &["Diagram"], None, "name: string; the diagram's name", always, diagram_add),
        cmd!("er.diagram.rename", "Rename diagram", &["Diagram"], None, "diagram: string; the diagram's id\nname: string; its new name", has_diagrams, diagram_rename),
        cmd!("er.diagram.remove", "Remove diagram", &["Diagram"], None, "diagram: string; the diagram's id", has_diagrams, diagram_remove),
        cmd!("er.diagram.place", "Show entity on diagram", &["Diagram"], None,
            "diagram: string; the diagram's id\nentity: string; the entity's id\nx: number, -1000000..1000000; where its box goes\ny: number, -1000000..1000000; where its box goes\nwidth: number = 200, 20..10000; the box's width\nheight: number = 120, 20..10000; the box's height",
            has_diagrams, diagram_place),
        cmd!("er.diagram.move", "Move", &["Diagram"], None,
            "diagram: string; the diagram's id\nentities: array; the ids of the entities to move, all on that diagram\ndx: number, -2000000..2000000; how far across\ndy: number, -2000000..2000000; how far down",
            has_diagrams, diagram_move),
        cmd!("er.diagram.resize", "Resize", &["Diagram"], None,
            "diagram: string; the diagram's id\nentity: string; the entity's id\nwidth: number, 20..10000; the box's width\nheight: number, 20..10000; the box's height",
            has_diagrams, diagram_resize),
        cmd!("er.diagram.unplace", "Hide entity from diagram", &["Diagram"], None, "diagram: string; the diagram's id\nentity: string; the entity's id", has_diagrams, diagram_unplace),
        cmd!(noundo "view.open-diagram", "Open diagram", &["View"], None, "diagram: string; the diagram's id, none when empty", always, open_diagram),
        cmd!(noundo "view.select", "Select", &["View"], None, "ids: array = []; the elements to select", always, select),
        cmd!(query "er.entity.list", "List entities", &["Model"], None, "", always, entity_list),
        cmd!(query "er.diagram.list", "List diagrams", &["Diagram"], None, "", always, diagram_list),
        cmd!(query "er.entity.get", "Show entity", &["Model"], None, "entity: string; the entity's id", has_entities, entity_get),
        cmd!(query "er.diagram.get", "Show diagram", &["Diagram"], None, "diagram: string; the diagram's id", has_diagrams, diagram_get),
        cmd!(query "er.model.get", "Show model", &["Model"], None, "", always, model_get),
    ]);
    s
}
