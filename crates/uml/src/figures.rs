//! What a class diagram draws: a class as a plain card with its attributes
//! (visibility, name, type, multiplicity) and its operations (visibility, name, parameters, return
//! type), its name slanted when it is abstract; an association as a line with its name, its ends'
//! roles and multiplicities, an open arrow at an end only it navigates to (navigable both ways, it
//! shows none), and a diamond at the whole of an aggregation (hollow) or a composition (filled); a
//! generalization as a line to a hollow triangle at the general class; a dependency dashed, with an
//! open arrow at its supplier. The canvas draws these descriptions.

use archforge_diagram::figure::{Card, End, EndMark, Line, Row, Text, Tone};
use archforge_mof::{Obj, Store, Value};

use crate::{
    ASSOCIATION, AggregationKind, CLASS, DEPENDENCY, GENERALIZATION, ParameterDirectionKind, VisibilityKind, association, behavioral_feature, class,
    dependency, generalization, literal_integer, literal_unlimited_natural, multiplicity_element, named_element, operation, parameter, property,
    typed_element,
};

/// The most parameters an operation's row names, and the longest row a card draws: a row is
/// built every frame, so neither grows with what the model holds.
const MAX_PARAMETERS: usize = 8;
const MAX_ROW: usize = 200;

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// A row cut to [`MAX_ROW`] characters.
fn cut(text: String) -> String {
    if text.chars().count() <= MAX_ROW { text } else { text.chars().take(MAX_ROW - 1).chain(['…']).collect() }
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn flag(store: &Store, o: Obj, prop: archforge_mof::PropId) -> bool {
    store.get(o, prop).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// The sign of an element's visibility, when it has one.
pub fn visibility(store: &Store, o: Obj) -> Option<&'static str> {
    match store.get(o, named_element::VISIBILITY).ok()? {
        Value::Enum(i) => Some(match VisibilityKind::from_index(i)? {
            VisibilityKind::Public => "+",
            VisibilityKind::Private => "-",
            VisibilityKind::Protected => "#",
            VisibilityKind::Package => "~",
        }),
        _ => None,
    }
}

/// A multiplicity's bounds as written: `1`, `0..1`, `*` (none to many), `1..*`; empty when the
/// element names none.
pub fn multiplicity(store: &Store, o: Obj) -> String {
    let lower = one(store, o, multiplicity_element::LOWER_VALUE).and_then(|v| match store.get(v, literal_integer::VALUE).ok()? {
        Value::Int(n) => Some(n),
        _ => None,
    });
    let upper = one(store, o, multiplicity_element::UPPER_VALUE).and_then(|v| match store.get(v, literal_unlimited_natural::VALUE).ok()? {
        Value::Unlimited(None) => Some(None),
        Value::Unlimited(Some(n)) => Some(Some(n)),
        _ => None,
    });
    if lower.is_none() && upper.is_none() {
        return String::new();
    }
    let lower = lower.unwrap_or(1);
    let upper = match upper {
        Some(None) => "*".to_string(),
        Some(Some(n)) => n.to_string(),
        None => "1".to_string(),
    };
    if lower == 0 && upper == "*" {
        "*".into()
    } else if lower.to_string() == upper {
        upper
    } else {
        format!("{lower}..{upper}")
    }
}

/// The name of an element's type, when it has one.
fn type_name(store: &Store, o: Obj) -> Option<String> {
    one(store, o, typed_element::TYPE).map(|t| name(store, t)).filter(|n| !n.is_empty())
}

/// An attribute as a class's card lists it: `+ total : Money [0..1]`.
pub fn attribute_text(store: &Store, a: Obj) -> String {
    let mut text = visibility(store, a).map(|v| format!("{v} ")).unwrap_or_default();
    text.push_str(&name(store, a));
    if let Some(ty) = type_name(store, a) {
        text.push_str(&format!(" : {ty}"));
    }
    let m = multiplicity(store, a);
    if !m.is_empty() && m != "1" {
        text.push_str(&format!(" [{m}]"));
    }
    cut(text)
}

/// An operation as a class's card lists it: `+ total(currency : Currency) : Money`.
pub fn operation_text(store: &Store, op: Obj) -> String {
    let direction = |p: Obj| match store.get(p, parameter::DIRECTION).ok() {
        Some(Value::Enum(i)) => ParameterDirectionKind::from_index(i),
        _ => None,
    };
    let params = list(store, op, operation::OWNED_PARAMETER);
    let all_inputs: Vec<Obj> = params.iter().copied().filter(|p| direction(*p) != Some(ParameterDirectionKind::Return)).collect();
    let mut inputs: Vec<String> = all_inputs
        .iter()
        .take(MAX_PARAMETERS)
        .map(|p| match type_name(store, *p) {
            Some(ty) => format!("{} : {ty}", name(store, *p)),
            None => name(store, *p),
        })
        .collect();
    if all_inputs.len() > MAX_PARAMETERS {
        inputs.push("…".into());
    }
    let returns = params.iter().find(|p| direction(**p) == Some(ParameterDirectionKind::Return)).and_then(|p| type_name(store, *p));
    let mut text = visibility(store, op).map(|v| format!("{v} ")).unwrap_or_default();
    text.push_str(&format!("{}({})", name(store, op), inputs.join(", ")));
    if let Some(ty) = returns {
        text.push_str(&format!(" : {ty}"));
    }
    cut(text)
}

/// A class's card on `diagram`: its name, slanted when it is abstract, its attributes and its
/// operations; a class of another package than the diagram's is drawn as an outsider, its package
/// named in the eyebrow.
pub fn card(store: &Store, c: Obj, diagram: Obj) -> Option<Card> {
    if store.class_of(c).ok()? != CLASS {
        return None;
    }
    let here = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o));
    let elsewhere = crate::commands::package_of(store, c).filter(|p| here.is_some_and(|h| h != *p));
    let eyebrow = match elsewhere {
        Some(p) => Text::Said("Class · {package}", vec![("package", name(store, p))]),
        None => Text::said("Class"),
    };
    let row = |text, slanted| Row { mark: None, text, linked: false, slanted };
    let attributes = list(store, c, class::OWNED_ATTRIBUTE)
        .into_iter()
        // An association's end a class owns draws as the line's role, not as a row.
        .filter(|a| one(store, *a, property::ASSOCIATION).is_none())
        .map(|a| row(attribute_text(store, a), false))
        .collect();
    let operations = list(store, c, class::OWNED_OPERATION)
        .into_iter()
        .map(|op| row(operation_text(store, op), flag(store, op, behavioral_feature::IS_ABSTRACT)))
        .collect();
    Some(Card {
        eyebrow,
        name: name(store, c),
        compartments: vec![attributes, operations],
        more: ("+ {n} more member", "+ {n} more members"),
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: flag(store, c, class::IS_ABSTRACT),
    })
}

/// An association's two ends, in its member order.
pub fn member_ends(store: &Store, a: Obj) -> Option<[Obj; 2]> {
    match list(store, a, association::MEMBER_END).as_slice() {
        [first, second, ..] => Some([*first, *second]),
        _ => None,
    }
}

/// Whether the association navigates to its end `end`: a class owns the end, or the association
/// lists it as navigable.
pub fn navigable(store: &Store, a: Obj, end: Obj) -> bool {
    let owned_by_class = store.container(end).ok().flatten().is_some_and(|(c, _)| store.class_of(c).ok() == Some(CLASS));
    owned_by_class || list(store, a, association::NAVIGABLE_OWNED_END).contains(&end)
}

fn aggregation(store: &Store, end: Obj) -> AggregationKind {
    match store.get(end, property::AGGREGATION).ok() {
        Some(Value::Enum(i)) => AggregationKind::from_index(i).unwrap_or(AggregationKind::NoneKind),
        _ => AggregationKind::NoneKind,
    }
}

/// The classes an association, a generalization or a dependency joins, as its line runs.
pub fn ends(store: &Store, element: Obj) -> Option<(Obj, Obj)> {
    match store.class_of(element).ok()? {
        c if c == ASSOCIATION => {
            let [a, b] = member_ends(store, element)?;
            Some((one(store, a, typed_element::TYPE)?, one(store, b, typed_element::TYPE)?))
        }
        c if c == GENERALIZATION => {
            let specific = store.container(element).ok()??.0;
            Some((specific, one(store, element, generalization::GENERAL)?))
        }
        c if c == DEPENDENCY => Some((*list(store, element, dependency::CLIENT).first()?, *list(store, element, dependency::SUPPLIER).first()?)),
        _ => None,
    }
}

/// The line of an association, a generalization or a dependency.
pub fn line(store: &Store, element: Obj) -> Option<Line> {
    match store.class_of(element).ok()? {
        c if c == ASSOCIATION => {
            let ends = member_ends(store, element)?;
            let both = ends.iter().all(|e| navigable(store, element, *e));
            let end = |i: usize| {
                let (here, there) = (ends[i], ends[1 - i]);
                // The diamond sits at the whole: the end opposite the part that aggregates.
                let mark = match aggregation(store, there) {
                    AggregationKind::Composite => EndMark::FilledDiamond,
                    AggregationKind::Shared => EndMark::HollowDiamond,
                    AggregationKind::NoneKind if !both && navigable(store, element, here) => EndMark::OpenArrow,
                    AggregationKind::NoneKind => EndMark::None,
                };
                let m = multiplicity(store, here);
                let role = name(store, here);
                End {
                    mark,
                    text: (!m.is_empty()).then_some(m),
                    slot: if i == 0 { "from_end" } else { "to_end" },
                    role: (!role.is_empty()).then_some(role),
                }
            };
            Some(Line { ends: [end(0), end(1)], styled: true, dashed: false, status: None, note: None, badge: false, resolvable: false })
        }
        c if c == GENERALIZATION => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::Triangle)],
            styled: false,
            dashed: false,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
        }),
        c if c == DEPENDENCY => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::OpenArrow)],
            styled: true,
            dashed: true,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
        }),
        _ => None,
    }
}

/// The text slots a label of an association or a dependency may show.
pub fn text_slots(class: archforge_mof::ClassId) -> &'static [&'static str] {
    if class == ASSOCIATION {
        &["name", "from_end", "to_end", "from_role", "to_role"]
    } else if class == DEPENDENCY {
        &["name"]
    } else {
        &[]
    }
}

/// The text of an association's or a dependency's slot a label shows.
pub fn slot_text(store: &Store, element: Obj, slot: &str) -> String {
    let end = |i: usize| member_ends(store, element).map(|e| e[i]);
    match (store.class_of(element).ok(), slot) {
        (_, "name") => name(store, element),
        (Some(c), "from_end") if c == ASSOCIATION => end(0).map(|e| multiplicity(store, e)).unwrap_or_default(),
        (Some(c), "to_end") if c == ASSOCIATION => end(1).map(|e| multiplicity(store, e)).unwrap_or_default(),
        (Some(c), "from_role") if c == ASSOCIATION => end(0).map(|e| name(store, e)).unwrap_or_default(),
        (Some(c), "to_role") if c == ASSOCIATION => end(1).map(|e| name(store, e)).unwrap_or_default(),
        _ => String::new(),
    }
}
