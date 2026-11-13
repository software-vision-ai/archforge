//! Skills: text the user writes on a model element to guide the AI when it
//! draws, names, changes the model or exports it. A skill sits on a subject (a schema, a package,
//! a table, a column, a diagram, or the whole model when it has none), may aim at a kind below the subject
//! (every table, every relationship, columns of a type) and is inherited down the hierarchy: the
//! effective skills of an element are those of its levels outermost first, then its own, so
//! the most specific is read last; a skill with the name of an earlier one replaces it. The core
//! knows no notation: what may carry a skill and what a skill may aim at comes from the
//! notation's view through [`SkillHost`], as the diagram commands take their policy from theirs.

use std::collections::{BTreeSet, HashMap};

use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, View, cmd};
use archforge_mof::{ClassId, Invalid, Obj, PropId, Store, TypeRef, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::{MODEL, SKILL, SkillTrigger, model, named_element, skill};

type C<'a, V> = Ctx<'a, Store, V>;
type R = Result<Json, CommandError>;

/// The most characters in a skill's name, description, body, dialect and type pattern.
pub const MAX_NAME: usize = 128;
pub const MAX_DESCRIPTION: usize = 256;
pub const MAX_BODY: usize = 16_000;
pub const MAX_DIALECT: usize = 32;
pub const MAX_TYPE_PATTERN: usize = 64;
/// The most triggers one skill names: every literal once.
pub const MAX_TRIGGERS: usize = 4;

/// The type categories a type pattern may name instead of an exact type.
pub const CATEGORIES: &[&str] = &["text", "number", "date", "boolean", "binary"];

/// Why a command is not available.
pub const NO_MODEL: &str = "The project has no model root yet.";
pub const NO_SKILLS: &str = "There is no skill yet.";

/// What a notation declares about skills: which of its elements may carry one, which kinds a
/// skill may aim at, and where a column's type is read from.
pub trait SkillHost: View<Store> {
    /// The selection, which a new skill joins.
    fn selection_mut(&mut self) -> &mut Vec<Obj>;
    /// The classes whose elements may be a skill's subject, besides the model root.
    fn skill_subjects(&self) -> Vec<ClassId>;
    /// The classes a skill may aim at, each with the word the user sees (`table`, `relationship`,
    /// `column`).
    fn skill_kinds(&self) -> Vec<(ClassId, &'static str)>;
    /// Classes that are kinds of their own when a skill is aimed, though no skill names them: an
    /// element of one is reached only by a skill for its own kind, never by one for a broader kind
    /// it conforms to. None by default.
    fn aim_only_kinds(&self) -> Vec<ClassId> {
        Vec::new()
    }
    /// The classes whose elements have a type a pattern can match, each with the property holding it.
    fn typed_classes(&self) -> Vec<(ClassId, PropId)>;
    /// Why `element`, of a subject or kind class, still carries no skill and is aimed at by none,
    /// when it does not: in UML an association's end is a property but no attribute.
    fn skill_problem(&self, store: &Store, element: Obj) -> Option<&'static str> {
        let _ = (store, element);
        None
    }
    /// The level a skill is inherited from above `element` when it is not the element's
    /// container: a diagram's is the element it shows. None follows the container.
    fn level_above(&self, store: &Store, element: Obj) -> Option<Obj> {
        let _ = (store, element);
        None
    }
}

// Rules.

/// What is wrong with a name or a text: the end of the English sentence "the name ...", which
/// it displays as, and the same as a typed reason, for an interface to say in its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The problem typed.
    pub reason: Reason,
    words: String,
}

impl Problem {
    /// The problem `reason`, said in English as `words`, the end of a sentence "the name ...".
    pub fn new(reason: Reason, words: impl Into<String>) -> Problem {
        Problem { reason, words: words.into() }
    }

    /// Longer than `max` characters.
    pub fn too_long(max: usize) -> Problem {
        Problem::new(Reason::TooLong { max }, format!("is longer than {max} characters"))
    }

    /// Starts or ends with a space.
    pub fn edge_space() -> Problem {
        Problem::new(Reason::EdgeSpace, "starts or ends with a space")
    }
}

impl std::fmt::Display for Problem {
    /// The English words, as "is empty".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

/// Why a name is not acceptable, as the end of the sentence "the name ...".
pub fn name_problem(name: &str, max: usize) -> Option<Problem> {
    if name.is_empty() {
        return Some(Problem::new(Reason::NameEmpty, "is empty"));
    }
    if name.chars().count() > max {
        return Some(Problem::too_long(max));
    }
    if name.chars().any(hidden) {
        return Some(hidden_character());
    }
    if name.trim() != name {
        return Some(Problem::edge_space());
    }
    None
}

/// Why a text is not acceptable: longer than `max` characters, or a control character other than
/// a line break or a tab.
pub fn text_problem(text: &str, max: usize) -> Option<Problem> {
    if text.chars().count() > max {
        return Some(Problem::too_long(max));
    }
    if text.chars().any(|c| hidden(c) && c != '\n' && c != '\t' && c != '\r') {
        return Some(hidden_character());
    }
    None
}

/// Holds a control or hidden character.
fn hidden_character() -> Problem {
    Problem::new(Reason::ControlCharacter, "holds a control or hidden character")
}

/// A character that renders as nothing or reorders what follows: a control character, a zero-width
/// or bidirectional format character. A skill is text the AI reads as instructions, so a body that
/// shows one thing and reads as another is refused.
pub fn hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// The trigger a literal names.
pub fn trigger_named(word: &str) -> Option<SkillTrigger> {
    let w = word.trim().to_lowercase();
    (0..).map_while(SkillTrigger::from_index).find(|t| t.literal() == w)
}

/// The trigger literals, for a refusal: "modelling, drawing, naming or export".
pub fn trigger_words() -> String {
    let all: Vec<&str> = (0..).map_while(SkillTrigger::from_index).map(SkillTrigger::literal).collect();
    match all.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        Some((last, _)) => (*last).to_string(),
        None => String::new(),
    }
}

/// The project's model root, when there is one.
pub fn model_of(store: &Store) -> Option<Obj> {
    store.alive().find(|o| store.class_of(*o).ok() == Some(MODEL))
}

/// The skills of the store, in the order the model lists them.
pub fn skills(store: &Store) -> Vec<Obj> {
    match model_of(store) {
        Some(m) => store.list(m, model::SKILL).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|s| store.is_alive(*s)).collect(),
        None => Vec::new(),
    }
}

/// The subject a skill sits on, or None for the whole model.
pub fn subject_of(store: &Store, s: Obj) -> Option<Obj> {
    store.get(s, skill::SUBJECT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o))
}

/// A skill's name, or empty.
pub fn name_of(store: &Store, s: Obj) -> String {
    crate::name_of(store, s)
}

fn string_of(store: &Store, o: Obj, prop: PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// A skill's triggers, in order; empty means it applies always.
pub fn triggers_of(store: &Store, s: Obj) -> Vec<SkillTrigger> {
    store
        .list(s, skill::TRIGGER)
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Enum(i) => SkillTrigger::from_index(*i),
            _ => None,
        })
        .collect()
}

/// The skills whose subject is `obj` or anything it contains: what goes with `obj`.
pub fn skills_on(store: &Store, obj: Obj) -> Vec<Obj> {
    skills(store).into_iter().filter(|s| subject_of(store, *s).is_some_and(|o| o == obj || store.contains(obj, o))).collect()
}

/// Whether a skill named `name` may sit on `subject` (None: the model): no other skill of that
/// name sits there.
pub fn skill_name_free(store: &Store, subject: Option<Obj>, name: &str, except: Option<Obj>) -> bool {
    name_taken(store, subject, name, except, "name").is_ok()
}

/// Refuses `name`, about the parameter `key`, when another skill of that name, whatever the case,
/// sits on `subject` (None: the model), but for `except`: the English names that skill, and the
/// reason is typed with its name.
fn name_taken(store: &Store, subject: Option<Obj>, name: &str, except: Option<Obj>, key: &str) -> Result<(), CommandError> {
    let wanted = name.to_lowercase();
    match skills(store).into_iter().find(|s| Some(*s) != except && subject_of(store, *s) == subject && name_of(store, *s).to_lowercase() == wanted) {
        Some(other) => {
            let existing = name_of(store, other);
            Err(refuse(key, format!("a skill named {existing:?} already sits there")).because(Reason::NameTaken { kind: "skill", name: existing }))
        }
        None => Ok(()),
    }
}

/// The key under which a class is named in a skill: the metamodel's name, a dot, the class's
/// name, such as `er.Entity`.
pub fn class_key(store: &Store, class: ClassId) -> Option<String> {
    let u = store.universe();
    let mm = u.metamodels().find(|m| m.index == class.metamodel())?;
    let c = u.class(class)?;
    Some(format!("{}.{}", mm.name, c.name))
}

/// The kind a key names among those the notation declares, and its word: three comparisons, not a
/// walk over the universe.
pub fn kind_by_key<V: SkillHost>(view: &V, store: &Store, key: &str) -> Option<(ClassId, &'static str)> {
    view.skill_kinds().into_iter().find(|(c, _)| class_key(store, *c).as_deref() == Some(key))
}

/// The class a key names, when the universe has it (a walk over every class: for `problems`).
pub fn class_by_key(store: &Store, key: &str) -> Option<ClassId> {
    store.universe().classes().find(|c| class_key(store, *c).as_deref() == Some(key))
}

/// The category of an SQL-ish type name, when it is one of [`CATEGORIES`].
pub fn category_of(type_name: &str) -> Option<&'static str> {
    let t = type_name.trim().to_uppercase();
    let base = t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap_or_default();
    let is = |names: &[&str]| names.contains(&base);
    if is(&["BOOL", "BOOLEAN", "BIT"]) {
        Some("boolean")
    } else if is(&["BLOB", "BYTEA", "BINARY", "VARBINARY", "BYTES", "RAW"]) {
        Some("binary")
    } else if is(&["DATE", "TIME", "TIMESTAMP", "TIMESTAMPTZ", "DATETIME", "INTERVAL"]) {
        Some("date")
    } else if is(&[
        "INT",
        "INTEGER",
        "BIGINT",
        "SMALLINT",
        "TINYINT",
        "DECIMAL",
        "NUMERIC",
        "FLOAT",
        "DOUBLE",
        "REAL",
        "MONEY",
        "NUMBER",
        "SERIAL",
        "BIGSERIAL",
    ]) {
        Some("number")
    } else if is(&["CHAR", "VARCHAR", "NVARCHAR", "NCHAR", "TEXT", "CLOB", "STRING", "CHARACTER"]) {
        Some("text")
    } else {
        None
    }
}

/// Whether a type pattern (an exact type name or a category) matches a type name.
pub fn type_matches(pattern: &str, type_name: &str) -> bool {
    let p = pattern.trim();
    let lower = p.to_lowercase();
    if CATEGORIES.contains(&lower.as_str()) {
        return category_of(type_name).is_some_and(|c| c == lower);
    }
    p.eq_ignore_ascii_case(type_name.trim())
}

/// Whether a skill aims at `element`: it names no kind (it applies to everything below its
/// subject), or the element's own kind, with a type pattern its type matches. The element's own
/// kind is the most specific of the notations' kinds its class conforms to, the aim-only kinds
/// among them: a UML enumeration and a primitive type are DataTypes in the metamodel, yet a skill
/// for every data type reaches neither; one for every enumeration reaches an enumeration, and one
/// for every primitive type a primitive type. An element its notation says carries no skill (a
/// built-in type) is aimed at by no kind.
pub fn aims_at<V: SkillHost>(view: &V, store: &Store, s: Obj, element: Obj) -> bool {
    let applies = string_of(store, s, skill::APPLIES_TO);
    if applies.is_empty() {
        return true;
    }
    let Some((kind, _)) = kind_by_key(view, store, &applies) else { return false };
    let Ok(class) = store.class_of(element) else { return false };
    let universe = store.universe();
    if !universe.conforms(class, TypeRef::Class(kind)) || view.skill_problem(store, element).is_some() {
        return false;
    }
    // A kind between the element's class and the skill's is closer: the element is of that one.
    let mut kinds = view.skill_kinds().into_iter().map(|(k, _)| k).chain(view.aim_only_kinds());
    let closer = kinds.any(|k| k != kind && universe.conforms(class, TypeRef::Class(k)) && universe.conforms(k, TypeRef::Class(kind)));
    if closer {
        return false;
    }
    let pattern = string_of(store, s, skill::TYPE_PATTERN);
    if pattern.is_empty() {
        return true;
    }
    match view.typed_classes().into_iter().find(|(typed, _)| store.universe().conforms(class, TypeRef::Class(*typed))) {
        Some((_, prop)) => type_matches(&pattern, &string_of(store, element, prop)),
        None => false,
    }
}

/// How specific a skill's target is: nothing named, a kind, a kind with a type.
fn specificity(store: &Store, s: Obj) -> u8 {
    if string_of(store, s, skill::APPLIES_TO).is_empty() {
        0
    } else if string_of(store, s, skill::TYPE_PATTERN).is_empty() {
        1
    } else {
        2
    }
}

/// An element's levels outermost first, then the element itself: its containers, or where the
/// notation's view names another level above an element (a diagram's element), that one.
pub fn levels<V: SkillHost>(view: &V, store: &Store, element: Obj) -> Vec<Obj> {
    let mut chain = vec![element];
    let mut seen = BTreeSet::from([element]);
    let mut cur = element;
    let above = |o: Obj| view.level_above(store, o).or_else(|| store.container(o).ok().flatten().map(|(parent, _)| parent));
    while let Some(parent) = above(cur) {
        if !seen.insert(parent) {
            break;
        }
        chain.push(parent);
        cur = parent;
    }
    chain.reverse();
    chain
}

/// One effective skill of an element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSkill {
    pub skill: Obj,
    /// The level it comes from: the element, one of its levels above, or None for the model root.
    pub origin: Option<Obj>,
    /// The deeper skill of the same name that replaces it, when one does.
    pub overridden_by: Option<Obj>,
}

/// The skills that apply to `element` for `trigger` (None: for every trigger), outermost first
/// and, within a level, the least specific first, so the AI reads the most specific last. A
/// skill named like an earlier one is kept, marked as overriding it. An element whose containment
/// does not reach the model root (a second root, which `problems` reports) gets no model-level skill.
pub fn effective_skills<V: SkillHost>(view: &V, store: &Store, element: Obj, trigger: Option<SkillTrigger>) -> Vec<EffectiveSkill> {
    let all = skills(store);
    let model = model_of(store);
    let mut out: Vec<EffectiveSkill> = Vec::new();
    for level in levels(view, store, element) {
        let at_level = |s: &Obj| {
            let subject = subject_of(store, *s);
            subject == Some(level) || (subject.is_none() && Some(level) == model)
        };
        let mut here: Vec<Obj> = all.iter().copied().filter(at_level).collect();
        here.sort_by_key(|s| specificity(store, *s));
        for s in here {
            if !aims_at(view, store, s, element) {
                continue;
            }
            let triggers = triggers_of(store, s);
            if let Some(t) = trigger
                && !triggers.is_empty()
                && !triggers.contains(&t)
            {
                continue;
            }
            let name = name_of(store, s).to_lowercase();
            for earlier in out.iter_mut().filter(|e| e.overridden_by.is_none()) {
                if name_of(store, earlier.skill).to_lowercase() == name {
                    earlier.overridden_by = Some(s);
                }
            }
            out.push(EffectiveSkill { skill: s, origin: if Some(level) == model { None } else { Some(level) }, overridden_by: None });
        }
    }
    out
}

/// What the core's skill rules report over a whole store.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillProblem {
    /// Two skills of one name on one subject.
    DuplicateName { subject: Option<Obj>, name: String, skills: [Obj; 2] },
    /// A skill whose subject is gone, or whose kind no metamodel has.
    Dangling(Obj),
}

/// The skill rules over the whole store: names unique per subject, subjects alive, kinds known.
pub fn problems(store: &Store) -> Vec<SkillProblem> {
    let mut out = Vec::new();
    let mut seen: HashMap<(Option<Obj>, String), Obj> = HashMap::new();
    for s in skills(store) {
        let subject_ref = store.get(s, skill::SUBJECT).ok().and_then(|v| v.as_ref());
        if subject_ref.is_some_and(|o| !store.is_alive(o)) {
            out.push(SkillProblem::Dangling(s));
            continue;
        }
        let applies = string_of(store, s, skill::APPLIES_TO);
        if !applies.is_empty() && class_by_key(store, &applies).is_none() {
            out.push(SkillProblem::Dangling(s));
            continue;
        }
        let name = name_of(store, s).to_lowercase();
        match seen.get(&(subject_ref, name.clone())) {
            Some(first) => out.push(SkillProblem::DuplicateName { subject: subject_ref, name, skills: [*first, s] }),
            None => {
                seen.insert((subject_ref, name), s);
            }
        }
    }
    out
}

/// The first two skills of one name on one subject, found in time linear in the skills: the name
/// rule a model is checked by as it opens. Skills of one name on different subjects are not one:
/// the more specific overrides the other.
pub fn first_duplicate(store: &Store) -> Option<SkillProblem> {
    let mut seen: HashMap<(Option<Obj>, String), Obj> = HashMap::new();
    for s in skills(store) {
        let subject = store.get(s, skill::SUBJECT).ok().and_then(|v| v.as_ref());
        let name = name_of(store, s).to_lowercase();
        if let Some(first) = seen.insert((subject, name.clone()), s) {
            return Some(SkillProblem::DuplicateName { subject, name, skills: [first, s] });
        }
    }
    None
}

// Commands.

fn has_model<V: SkillHost>(f: &Facts<'_, Store, V>) -> Result<(), &'static str> {
    if model_of(f.doc).is_some() { Ok(()) } else { Err(NO_MODEL) }
}

fn has_skills<V: SkillHost>(f: &Facts<'_, Store, V>) -> Result<(), &'static str> {
    if skills(f.doc).is_empty() { Err(NO_SKILLS) } else { Ok(()) }
}

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

/// A store refusal, as the command's failure.
fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

fn uuid_json(store: &Store, obj: Obj) -> Json {
    store.uuid(obj).map(|u| Json::String(u.to_string())).unwrap_or(Json::Null)
}

/// The live element the id under `key` names.
fn any_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{id:?} is not an id"))) };
    store.by_uuid(uuid).ok_or_else(|| refuse(key, format!("there is no element {id}")))
}

fn skill_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let s = any_of(store, p, key)?;
    if store.class_of(s).ok() != Some(SKILL) {
        return Err(refuse(key, format!("{} is not a skill", text(p, key))));
    }
    Ok(s)
}

/// The subject under `key`: empty or the model's id for the whole model, else an element of a
/// class the notation allows; a refusal names the id given and why it carries no skill.
fn subject_param<V: SkillHost>(view: &V, store: &Store, p: &Params, key: &str) -> Result<Option<Obj>, CommandError> {
    if text(p, key).is_empty() {
        return Ok(None);
    }
    let o = any_of(store, p, key)?;
    if store.class_of(o).map_err(fail)? == MODEL {
        return Ok(None);
    }
    match subject_problem(view, store, o) {
        Some(why) => Err(refuse(key, format!("{}: {why}", text(p, key)))),
        None => Ok(Some(o)),
    }
}

/// Why `o` cannot carry a skill, when it cannot: its class is none the notations name as a
/// subject, or its notation says why (an association's end). The model carries skills. The shell
/// asks this before it offers to add one, so it offers only what `skill.add` takes.
pub fn subject_problem<V: SkillHost>(view: &V, store: &Store, o: Obj) -> Option<String> {
    let class = store.class_of(o).ok()?;
    if class == MODEL {
        return None;
    }
    let subjects = view.skill_subjects();
    if !subjects.contains(&class) {
        let allowed: Vec<String> = subjects.iter().filter_map(|c| store.universe().class(*c).map(|c| c.name.to_string())).collect();
        return Some(format!("a skill cannot sit on {}; it sits on the model, {}", store.universe().class_name(class), allowed.join(", ")));
    }
    view.skill_problem(store, o).map(str::to_string)
}

/// The kind under `applies_to` (a word the subject's notation declares, any notation's for the
/// model, or empty) and the type pattern under `type_pattern`, checked together: a pattern needs
/// the typed kind.
fn target_params<V: SkillHost>(view: &V, store: &Store, p: &Params, subject: Option<Obj>) -> Result<(Option<String>, Option<String>), CommandError> {
    let word = text(p, "applies_to").trim().to_lowercase();
    let pattern = text(p, "type_pattern").trim().to_string();
    if word.is_empty() {
        if !pattern.is_empty() {
            return Err(refuse("type_pattern", "a type pattern needs applies_to to name the typed kind"));
        }
        return Ok((None, None));
    }
    let home = subject.and_then(|o| store.class_of(o).ok()).map(|c| c.metamodel());
    let kinds: Vec<(ClassId, &str)> = view.skill_kinds().into_iter().filter(|(c, _)| home.is_none_or(|m| c.metamodel() == m)).collect();
    let Some((class, _)) = kinds.iter().find(|(_, w)| *w == word) else {
        if kinds.is_empty() {
            return Err(refuse("applies_to", "a skill here aims at no kind; leave applies_to empty for the subject itself"));
        }
        let words: Vec<&str> = kinds.iter().map(|(_, w)| *w).collect();
        return Err(refuse("applies_to", format!("{word:?} is not one of {}", words.join(", "))));
    };
    let key = class_key(store, *class).ok_or_else(|| refuse("applies_to", "the kind is not in this project's metamodels"))?;
    if pattern.is_empty() {
        return Ok((Some(key), None));
    }
    if let Some(problem) = name_problem(&pattern, MAX_TYPE_PATTERN) {
        return Err(refuse("type_pattern", format!("the type pattern {problem}")).because(problem.reason));
    }
    let typed = view.typed_classes();
    if typed.iter().any(|(c, _)| c == class) {
        return Ok((Some(key), Some(pattern)));
    }
    let words: Vec<&str> = typed.iter().filter_map(|(c, _)| kinds.iter().find(|(k, _)| k == c)).map(|(_, w)| *w).collect();
    if words.is_empty() {
        return Err(refuse("type_pattern", "no kind a skill here aims at can name a type"));
    }
    Err(refuse("type_pattern", format!("only a skill for every {} can name a type", words.join(" or every "))))
}

/// The triggers under `triggers`: literals, none twice.
fn triggers_param(p: &Params) -> Result<Vec<SkillTrigger>, CommandError> {
    let items = p.get("triggers").and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > MAX_TRIGGERS {
        return Err(refuse("triggers", format!("at most {MAX_TRIGGERS} triggers")));
    }
    let mut out = Vec::new();
    for v in &items {
        let word = v.as_str().ok_or_else(|| refuse("triggers", "every trigger must be a string"))?;
        let t = trigger_named(word).ok_or_else(|| refuse("triggers", format!("{word:?} is not {}", trigger_words())))?;
        if out.contains(&t) {
            return Err(refuse("triggers", format!("{word} is listed twice")));
        }
        out.push(t);
    }
    Ok(out)
}

/// Every field of a skill, validated together.
struct Fields {
    name: String,
    description: String,
    body: String,
    triggers: Vec<SkillTrigger>,
    dialect: String,
    applies_to: Option<String>,
    type_pattern: Option<String>,
}

/// A skill's fields from `p`, each checked: its name free on `subject` but for `except`, its
/// description, body, triggers, dialect and the kind and type it aims at.
fn fields<V: SkillHost>(view: &V, store: &Store, p: &Params, subject: Option<Obj>, except: Option<Obj>) -> Result<Fields, CommandError> {
    let name = text(p, "name").to_string();
    if let Some(problem) = name_problem(&name, MAX_NAME) {
        return Err(refuse("name", format!("the name {problem}")).because(problem.reason));
    }
    name_taken(store, subject, &name, except, "name")?;
    let description = text(p, "description").to_string();
    if let Some(problem) = text_problem(&description, MAX_DESCRIPTION) {
        return Err(refuse("description", format!("the description {problem}")).because(problem.reason));
    }
    let body = text(p, "body").to_string();
    if let Some(problem) = text_problem(&body, MAX_BODY) {
        return Err(refuse("body", format!("the body {problem}")).because(problem.reason));
    }
    let triggers = triggers_param(p)?;
    let dialect = text(p, "dialect").trim().to_string();
    if !dialect.is_empty() {
        if let Some(problem) = name_problem(&dialect, MAX_DIALECT) {
            return Err(refuse("dialect", format!("the dialect {problem}")).because(problem.reason));
        }
        if !triggers.contains(&SkillTrigger::Export) {
            return Err(refuse("dialect", "a dialect belongs to a skill with the export trigger"));
        }
    }
    let (applies_to, type_pattern) = target_params(view, store, p, subject)?;
    Ok(Fields { name, description, body, triggers, dialect, applies_to, type_pattern })
}

fn opt(s: &str) -> Value {
    if s.is_empty() { Value::Null } else { Value::str(s) }
}

/// Writes every field onto a skill.
fn write(st: &mut Store, s: Obj, f: &Fields) -> Result<(), Invalid> {
    st.set(s, named_element::NAME, Value::str(&f.name))?;
    st.set(s, skill::DESCRIPTION, opt(&f.description))?;
    st.set(s, skill::BODY, opt(&f.body))?;
    st.set(s, skill::DIALECT, opt(&f.dialect))?;
    st.set(s, skill::APPLIES_TO, opt(f.applies_to.as_deref().unwrap_or_default()))?;
    st.set(s, skill::TYPE_PATTERN, opt(f.type_pattern.as_deref().unwrap_or_default()))?;
    for i in (0..st.list(s, skill::TRIGGER)?.len()).rev() {
        st.remove_at(s, skill::TRIGGER, u32::try_from(i).map_err(|_| Invalid::Refused("too many triggers".into()))?)?;
    }
    for t in &f.triggers {
        st.push(s, skill::TRIGGER, Value::Enum(t.index()))?;
    }
    Ok(())
}

/// A skill as the queries report it.
pub fn skill_json<V: SkillHost>(view: &V, store: &Store, s: Obj) -> Json {
    let applies = string_of(store, s, skill::APPLIES_TO);
    // The word the notation uses for the kind; a key no declared kind has is reported as it is
    // stored, never as "" (which means the subject itself).
    let word = kind_by_key(view, store, &applies).map(|(_, w)| w).unwrap_or(applies.as_str());
    json!({
        "id": uuid_json(store, s),
        "name": name_of(store, s),
        "description": string_of(store, s, skill::DESCRIPTION),
        "body": string_of(store, s, skill::BODY),
        "triggers": triggers_of(store, s).iter().map(|t| t.literal()).collect::<Vec<_>>(),
        "dialect": string_of(store, s, skill::DIALECT),
        "subject": subject_of(store, s).map(|o| uuid_json(store, o)).unwrap_or(Json::Null),
        "applies_to": word,
        "type_pattern": string_of(store, s, skill::TYPE_PATTERN),
    })
}

fn skill_add<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let model = model_of(ctx.doc).ok_or_else(|| CommandError::new(NO_MODEL))?;
    let subject = subject_param(&*ctx.view, ctx.doc, p, "subject")?;
    let f = fields(&*ctx.view, ctx.doc, p, subject, None)?;
    let s = ctx
        .doc
        .transaction(|st| {
            let s = st.create(SKILL)?;
            st.push(model, model::SKILL, Value::Ref(s))?;
            st.set(s, skill::SUBJECT, subject.map(Value::Ref).unwrap_or(Value::Null))?;
            write(st, s, &f)?;
            Ok(s)
        })
        .map_err(fail)?;
    *SkillHost::selection_mut(ctx.view) = vec![s];
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn skill_set<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let s = skill_of(ctx.doc, p, "skill")?;
    let subject = subject_of(ctx.doc, s);
    let f = fields(&*ctx.view, ctx.doc, p, subject, Some(s))?;
    ctx.doc.transaction(|st| write(st, s, &f)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn skill_rename<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let s = skill_of(ctx.doc, p, "skill")?;
    let name = text(p, "name");
    if let Some(problem) = name_problem(name, MAX_NAME) {
        return Err(refuse("name", format!("the name {problem}")).because(problem.reason));
    }
    name_taken(ctx.doc, subject_of(ctx.doc, s), name, Some(s), "name")?;
    ctx.doc.set(s, named_element::NAME, Value::str(name)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

/// Writes a skill's text alone, as its editor saves it while it is written.
fn skill_set_body<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let s = skill_of(ctx.doc, p, "skill")?;
    let body = text(p, "body");
    if let Some(problem) = text_problem(body, MAX_BODY) {
        return Err(refuse("body", format!("the body {problem}")).because(problem.reason));
    }
    ctx.doc.set(s, skill::BODY, opt(body)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn skill_remove<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let s = skill_of(ctx.doc, p, "skill")?;
    let gone = crate::destroy_with_links(ctx.doc, s).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

fn skill_move<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let s = skill_of(ctx.doc, p, "skill")?;
    let subject = subject_param(&*ctx.view, ctx.doc, p, "subject")?;
    // A skill that aims at a kind stays within that kind's notation.
    let applies = string_of(ctx.doc, s, skill::APPLIES_TO);
    if let (Some(o), Some((kind, word))) = (subject, kind_by_key(&*ctx.view, ctx.doc, &applies))
        && ctx.doc.class_of(o).ok().map(|c| c.metamodel()) != Some(kind.metamodel())
    {
        let class = ctx.doc.class_of(o).map(|c| ctx.doc.universe().class_name(c)).unwrap_or_default();
        return Err(refuse("subject", format!("a skill for every {word} cannot sit on {class}")));
    }
    let name = name_of(ctx.doc, s);
    name_taken(ctx.doc, subject, &name, Some(s), "subject")?;
    ctx.doc.set(s, skill::SUBJECT, subject.map(Value::Ref).unwrap_or(Value::Null)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn skill_list<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let subject = text(p, "subject");
    let wanted = if subject.is_empty() { None } else { Some(subject_param(&*ctx.view, ctx.doc, p, "subject")?) };
    let items: Vec<Json> = skills(ctx.doc)
        .into_iter()
        .filter(|s| wanted.is_none_or(|w| subject_of(ctx.doc, *s) == w))
        .map(|s| skill_json(&*ctx.view, ctx.doc, s))
        .collect();
    Ok(json!({ "skills": items }))
}

fn skill_effective<V: SkillHost>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let element = any_of(ctx.doc, p, "element")?;
    let word = text(p, "trigger").trim().to_lowercase();
    let trigger = if word.is_empty() {
        None
    } else {
        Some(trigger_named(&word).ok_or_else(|| refuse("trigger", format!("{word:?} is not {}", trigger_words())))?)
    };
    let items: Vec<Json> = effective_skills(&*ctx.view, ctx.doc, element, trigger)
        .into_iter()
        .map(|e| {
            let mut j = skill_json(&*ctx.view, ctx.doc, e.skill);
            if let Json::Object(m) = &mut j {
                m.insert("origin".into(), e.origin.map(|o| uuid_json(ctx.doc, o)).unwrap_or(Json::Null));
                m.insert("overridden_by".into(), e.overridden_by.map(|o| uuid_json(ctx.doc, o)).unwrap_or(Json::Null));
            }
            j
        })
        .collect();
    Ok(json!({ "skills": items }))
}

/// The skill commands for a notation whose view is `V`.
pub fn specs<V: SkillHost>() -> Vec<CommandSpec<Store, V>> {
    vec![
        cmd!(
            "skill.add",
            "Add skill",
            &["Skills"],
            None,
            concat!(
                "subject: string = \"\"; the id of the element the skill sits on, or empty for the whole model\n",
                "name: string; the skill's name\n",
                "description: string = \"\"; one line on when it applies\n",
                "body: string = \"\"; the instructions, Markdown\n",
                "triggers: array = []; when it applies: modelling, drawing, naming, export; none for always\n",
                "dialect: string = \"\"; the SQL dialect of an export skill, or empty for every dialect\n",
                "applies_to: string = \"\"; what it aims at below the subject, a kind of the subject's notation (ER: table, relationship, column; UML: class, interface, enumeration, data type, primitive type, association class, attribute, operation, association; a diagram takes none), or empty for the subject itself\n",
                "type_pattern: string = \"\"; with applies_to column: an exact type or a category (text, number, date, boolean, binary)"
            ),
            has_model::<V>,
            skill_add::<V>
        ),
        cmd!(
            "skill.set",
            "Edit skill",
            &["Skills"],
            None,
            concat!(
                "skill: string; the skill's id\n",
                "name: string; the skill's name\n",
                "description: string; one line on when it applies, or empty\n",
                "body: string; the instructions, Markdown, or empty\n",
                "triggers: array; when it applies: modelling, drawing, naming, export; none for always\n",
                "dialect: string; the SQL dialect of an export skill, or empty for every dialect\n",
                "applies_to: string; what it aims at below the subject, a kind of the subject's notation (ER: table, relationship, column; UML: class, interface, enumeration, data type, primitive type, association class, attribute, operation, association; a diagram takes none), or empty for the subject itself\n",
                "type_pattern: string; with applies_to column: an exact type or a category, or empty"
            ),
            has_skills::<V>,
            skill_set::<V>
        ),
        cmd!(
            "skill.rename",
            "Rename skill",
            &["Skills"],
            None,
            "skill: string; the skill's id\nname: string; the new name",
            has_skills::<V>,
            skill_rename::<V>
        ),
        cmd!(
            "skill.set-body",
            "Edit skill text",
            &["Skills"],
            None,
            "skill: string; the skill's id\nbody: string; the instructions, Markdown, or empty",
            has_skills::<V>,
            skill_set_body::<V>
        ),
        cmd!("skill.remove", "Remove skill", &["Skills"], None, "skill: string; the skill's id", has_skills::<V>, skill_remove::<V>),
        cmd!(
            "skill.move",
            "Move skill",
            &["Skills"],
            None,
            "skill: string; the skill's id\nsubject: string = \"\"; the id of the element it moves to, or empty for the whole model",
            has_skills::<V>,
            skill_move::<V>
        ),
        cmd!(query "skill.list", "List skills", &["Skills"], None, "subject: string = \"\"; only the skills on this element, or all", has_model::<V>, skill_list::<V>),
        cmd!(
            query "skill.effective",
            "Effective skills",
            &["Skills"],
            None,
            "element: string; the element's id\ntrigger: string = \"\"; modelling, drawing, naming or export, or empty for all",
            has_model::<V>,
            skill_effective::<V>
        ),
    ]
}
