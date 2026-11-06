//! What a class diagram draws: a class, an interface or a data type as a plain card, an
//! interface's eyebrow its keyword «interface» and a data type's «dataType»; a primitive type as a
//! plain card with the keyword «primitive» over its name and no compartment; an enumeration as a plain card with the keyword «enumeration» and one
//! compartment of its literals, in their order; a class's, an interface's or a data type's attributes
//! (visibility, name, type, multiplicity) and its operations (visibility, name, parameters, return
//! type), its name slanted when it is abstract, an abstract operation slanted and a static member
//! underlined, and any classifier's name followed by `{final}` when nothing may specialize it
//! (AR-137), `{leaf}` when nothing may redefine it, both as `{final, leaf}`, and a leaf member's
//! row ending in `{leaf}` (AR-138); an association as a line with its name, its ends'
//! roles and multiplicities, an open arrow at every end it navigates to (owner decision), and a
//! diamond, which takes the place of an arrow at its end, at the whole of an aggregation (hollow) or a composition (filled); a
//! generalization as a line to a hollow triangle at the general class; a dependency dashed, with an
//! open arrow at its supplier; an interface realization dashed in the muted ink, with a hollow
//! triangle at its interface; an association class as an association's line and a class's card,
//! its eyebrow Association class (AR-125), which the canvas ties to the line's middle. The canvas
//! draws these descriptions.

use archforge_diagram::figure::{Card, End, EndMark, Line, Row, Text, Tone};
use archforge_mof::{Obj, Store, Value};

use crate::{
    ASSOCIATION, ASSOCIATION_CLASS, AggregationKind, CLASS, DATA_TYPE, DEPENDENCY, ENUMERATION, GENERALIZATION, INTERFACE, INTERFACE_REALIZATION,
    PRIMITIVE_TYPE, ParameterDirectionKind, VisibilityKind, association, behavioral_feature, class, dependency, generalization,
    interface_realization, literal_integer, literal_unlimited_natural, multiplicity_element, named_element, operation, parameter, property,
    typed_element,
};

/// The most parameters an operation's row names, and the longest row a card draws: a row is
/// built every frame, so neither grows with what the model holds.
const MAX_PARAMETERS: usize = 8;
const MAX_ROW: usize = 200;
/// The most rows a card lists, across its compartments: a card is at most
/// [`archforge_diagram::rules::MAX_SIZE`] tall, which holds no more than this many rows, so the
/// rows built every frame never grow with the model. The rest are counted in `hidden`, which the
/// canvas says in the card's `+ n more` row.
pub const MAX_CARD_ROWS: usize = 500;

/// The rows of `items` a card lists, made by `row`, at most `room` of them: `room` and `hidden`
/// are counted down and up by what is left out.
fn capped(items: Vec<Obj>, room: &mut usize, hidden: &mut usize, row: impl Fn(Obj) -> Row) -> Vec<Row> {
    let shown = items.len().min(*room);
    *hidden = hidden.saturating_add(items.len().saturating_sub(shown));
    *room = room.saturating_sub(shown);
    items.into_iter().take(shown).map(row).collect()
}

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

/// Whether a feature (an attribute or an operation) is static, a feature of its classifier rather
/// than of each instance: its card's row is underlined (AR-136).
pub fn is_static(store: &Store, feature: Obj) -> bool {
    flag(store, feature, crate::feature::IS_STATIC)
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

/// An attribute as a class's card lists it: `+ total : Money [0..1]`; a derived one with `/ `
/// before its name, then its default after `=` and what differs from the usual in braces (AR-126,
/// as the design writes it: `+ / total : Money = 0`, `+ lines : OrderLine [1..*] {ordered}`).
pub fn attribute_text(store: &Store, a: Obj) -> String {
    let mut text = visibility(store, a).map(|v| format!("{v} ")).unwrap_or_default();
    if crate::modifiers::is_derived(store, a) {
        text.push_str("/ ");
    }
    text.push_str(&name(store, a));
    if let Some(ty) = type_name(store, a) {
        text.push_str(&format!(" : {ty}"));
    }
    let m = multiplicity(store, a);
    if !m.is_empty() && m != "1" {
        text.push_str(&format!(" [{m}]"));
    }
    text.push_str(&crate::modifiers::suffix(store, a));
    cut(text)
}

/// An association end's role as its line writes it: `/` before it when the end is derived; empty
/// when it has neither.
pub fn end_role(store: &Store, end: Obj) -> String {
    let role = name(store, end);
    if crate::modifiers::is_derived(store, end) { cut(format!("/{role}")) } else { role }
}

/// What an association end's line writes beside its end after the role: its multiplicity, then
/// `= default` and the braces, as in `0..* = none {ordered}` (AR-126); empty when it has none.
pub fn end_text(store: &Store, end: Obj) -> String {
    let text = format!("{}{}", multiplicity(store, end), crate::modifiers::suffix(store, end));
    cut(text.trim_start().to_string())
}

/// An operation as a class's card lists it: `+ total(currency : Currency) : Money`, then `{leaf}`
/// when nothing may redefine it (AR-138).
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
    if crate::relations::is_leaf(store, op) {
        text.push_str(" {leaf}");
    }
    cut(text)
}

/// A class's, an interface's, a data type's or an association class's card on `diagram`: its name,
/// a class's slanted when it is abstract, `{final}` after it when nothing may specialize it and
/// `{leaf}` when nothing may redefine it (`{final, leaf}` for both), its
/// attributes and its operations; one of another package
/// than the diagram's is drawn as an outsider, its package named in the eyebrow. A class's eyebrow
/// says Class, an association class's Association class, an interface's and a data type's their
/// keyword («interface», «dataType»); an enumeration's card is its own, a primitive type's too, and
/// a package's.
pub fn card(store: &Store, c: Obj, diagram: Obj) -> Option<Card> {
    // The eyebrow alone, and with the outsider's package beside it.
    let (here, away, is_class) = match store.class_of(c).ok()? {
        k if k == crate::PACKAGE => return Some(package_card(store, c)),
        k if k == ENUMERATION => return Some(enumeration_card(store, c, diagram)),
        k if k == PRIMITIVE_TYPE => return Some(primitive_type_card(store, c, diagram)),
        k if k == CLASS => ("Class", "Class · {package}", true),
        k if k == INTERFACE => ("«interface»", "«interface» · {package}", false),
        k if k == DATA_TYPE => ("«dataType»", "«dataType» · {package}", false),
        k if k == ASSOCIATION_CLASS => ("Association class", "Association class · {package}", false),
        k if k == crate::CONSTRAINT => return Some(constraint_card(store, c, diagram)),
        _ => return None,
    };
    let elsewhere = elsewhere(store, c, diagram);
    let eyebrow = match elsewhere {
        Some(p) => Text::Said(away, vec![("package", name(store, p))]),
        None => Text::said(here),
    };
    // A static member underlined, an abstract operation slanted: a static abstract one is both.
    let row = |text, member, slanted| Row { mark: None, text, linked: false, slanted, underlined: is_static(store, member) };
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    // An association's end a class owns draws as the line's role, not as a row: the members'
    // attributes leave it out.
    let attributes = capped(crate::members::attributes(store, c), &mut room, &mut hidden, |a| row(attribute_text(store, a), a, false));
    let operations = capped(crate::members::operations(store, c), &mut room, &mut hidden, |op| {
        row(operation_text(store, op), op, flag(store, op, behavioral_feature::IS_ABSTRACT))
    });
    Some(Card {
        eyebrow,
        name: card_name(store, c, diagram),
        compartments: vec![attributes, operations],
        more: ("+ {n} more member", "+ {n} more members"),
        hidden,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: is_class && flag(store, c, class::IS_ABSTRACT),
        braces: braces(store, c),
        template: crate::templates::signature_rows(store, c),
    })
}

/// What a classifier's name row says in braces after its name: `final` when nothing may
/// specialize it (AR-137), `leaf` when nothing may redefine it (AR-138), in one pair of braces
/// (`{final, leaf}`); None when it says nothing.
fn braces(store: &Store, c: Obj) -> Option<String> {
    let words: Vec<&str> = [(crate::relations::is_final(store, c), "final"), (crate::relations::is_leaf(store, c), "leaf")]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, w)| w)
        .collect();
    (!words.is_empty()).then(|| format!("{{{}}}", words.join(", ")))
}

/// The package of `c` when it is another than `diagram`'s: `c` is drawn there as an outsider. A
/// nested classifier's package is its owner's (AR-132).
fn elsewhere(store: &Store, c: Obj, diagram: Obj) -> Option<Obj> {
    let here = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o));
    crate::nesting::home_package(store, c).filter(|p| here.is_some_and(|h| h != *p))
}

/// The name a classifier's card shows on `diagram` (AR-132): its own, or, nested in a classifier
/// the diagram does not show, its owners' names before it, `Order::Priority`, so its context is
/// never lost. With its owner on the diagram the line ending in ⊕ shows whose it is.
pub fn card_name(store: &Store, c: Obj, diagram: Obj) -> String {
    match crate::nesting::classifier_owner(store, c) {
        Some(owner) if !shows(store, diagram, owner) => crate::nesting::qualified_name(store, c),
        _ => name(store, c),
    }
}

/// Whether `diagram` has a node of `element`.
pub fn shows(store: &Store, diagram: Obj, element: Obj) -> bool {
    archforge_diagram::views_of(store, element).2.into_iter().any(|n| archforge_diagram::diagram_of(store, n) == Some(diagram))
}

/// A primitive type's card on `diagram`: the keyword «primitive» in its eyebrow, its package beside
/// it when it is an outsider, and its name, with no compartment, since it has no members.
fn primitive_type_card(store: &Store, t: Obj, diagram: Obj) -> Card {
    let elsewhere = elsewhere(store, t, diagram);
    let eyebrow = match elsewhere {
        Some(p) => Text::Said("«primitive» · {package}", vec![("package", name(store, p))]),
        None => Text::said("«primitive»"),
    };
    Card {
        eyebrow,
        name: card_name(store, t, diagram),
        compartments: Vec::new(),
        more: ("+ {n} more member", "+ {n} more members"),
        hidden: 0,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: false,
        braces: braces(store, t),
        template: Vec::new(),
    }
}

/// An enumeration's card on `diagram`: the keyword «enumeration» in its eyebrow, its package beside
/// it when it is an outsider, its name, and one compartment of its literals in their order, empty
/// when it has none; past [`MAX_CARD_ROWS`] the rest are counted, not listed.
fn enumeration_card(store: &Store, e: Obj, diagram: Obj) -> Card {
    let elsewhere = elsewhere(store, e, diagram);
    let eyebrow = match elsewhere {
        Some(p) => Text::Said("«enumeration» · {package}", vec![("package", name(store, p))]),
        None => Text::said("«enumeration»"),
    };
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    let literals = capped(crate::commands::literals(store, e), &mut room, &mut hidden, |l| Row {
        mark: None,
        text: cut(name(store, l)),
        linked: false,
        slanted: false,
        underlined: false,
    });
    Card {
        eyebrow,
        name: card_name(store, e, diagram),
        compartments: vec![literals],
        more: ("+ {n} more literal", "+ {n} more literals"),
        hidden,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: false,
        braces: braces(store, e),
        template: Vec::new(),
    }
}

/// An association's two ends, in its member order.
pub fn member_ends(store: &Store, a: Obj) -> Option<[Obj; 2]> {
    match list(store, a, association::MEMBER_END).as_slice() {
        [first, second, ..] => Some([*first, *second]),
        _ => None,
    }
}

/// Whether the association navigates to its end `end`: a class or an interface owns the end, or the
/// association lists it as navigable. An association class is a class too, but the ends it owns as
/// an association are not its attributes: only an end in another list of a classifier's counts.
pub fn navigable(store: &Store, a: Obj, end: Obj) -> bool {
    let owned_by_class = store
        .container(end)
        .ok()
        .flatten()
        .is_some_and(|(c, prop)| prop != association::OWNED_END && store.class_of(c).is_ok_and(crate::is_classifier));
    owned_by_class || list(store, a, association::NAVIGABLE_OWNED_END).contains(&end)
}

fn aggregation(store: &Store, end: Obj) -> AggregationKind {
    match store.get(end, property::AGGREGATION).ok() {
        Some(Value::Enum(i)) => AggregationKind::from_index(i).unwrap_or(AggregationKind::NoneKind),
        _ => AggregationKind::NoneKind,
    }
}

/// The classes an association, an association class, a generalization, a dependency or an
/// interface realization joins, as its line runs: a realization from its implementing class to its
/// interface.
pub fn ends(store: &Store, element: Obj) -> Option<(Obj, Obj)> {
    match store.class_of(element).ok()? {
        c if crate::is_association(c) => {
            let [a, b] = member_ends(store, element)?;
            Some((one(store, a, typed_element::TYPE)?, one(store, b, typed_element::TYPE)?))
        }
        c if c == GENERALIZATION => {
            let specific = store.container(element).ok()??.0;
            Some((specific, one(store, element, generalization::GENERAL)?))
        }
        c if c == DEPENDENCY => Some((*list(store, element, dependency::CLIENT).first()?, *list(store, element, dependency::SUPPLIER).first()?)),
        c if c == INTERFACE_REALIZATION => {
            Some((one(store, element, interface_realization::IMPLEMENTING_CLASSIFIER)?, one(store, element, interface_realization::CONTRACT)?))
        }
        _ => None,
    }
}

/// The line of an association, an association class (an association's line: its card is drawn
/// apart), a generalization, a dependency or an interface realization.
pub fn line(store: &Store, element: Obj) -> Option<Line> {
    match store.class_of(element).ok()? {
        c if crate::is_association(c) => {
            let ends = member_ends(store, element)?;
            let end = |i: usize| {
                let (here, there) = (ends[i], ends[1 - i]);
                // The diamond sits at the whole: the end opposite the part that aggregates.
                let mark = match aggregation(store, there) {
                    AggregationKind::Composite => EndMark::FilledDiamond,
                    AggregationKind::Shared => EndMark::HollowDiamond,
                    AggregationKind::NoneKind if navigable(store, element, here) => EndMark::OpenArrow,
                    AggregationKind::NoneKind => EndMark::None,
                };
                let m = end_text(store, here);
                let role = end_role(store, here);
                End {
                    mark,
                    text: (!m.is_empty()).then_some(m),
                    slot: if i == 0 { "from_end" } else { "to_end" },
                    role: (!role.is_empty()).then_some(role),
                    // The qualifier box at the qualified end, its rows in model order (AR-127).
                    qualifiers: crate::qualifiers::qualifiers(store, here)
                        .into_iter()
                        .take(crate::qualifiers::MAX_QUALIFIERS)
                        .map(|q| crate::qualifiers::qualifier_text(store, q))
                        .collect(),
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
        // Thin in the muted ink as a generalization is, but dashed: realizing is not specializing.
        c if c == INTERFACE_REALIZATION => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::Triangle)],
            styled: false,
            dashed: true,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
        }),
        _ => None,
    }
}

/// The text slots a label of an association or a dependency may show; an association class's line
/// its ends' alone, its name being on its card.
pub fn text_slots(class: archforge_mof::ClassId) -> &'static [&'static str] {
    if class == ASSOCIATION {
        &["name", "from_end", "to_end", "from_role", "to_role"]
    } else if class == ASSOCIATION_CLASS {
        &["from_end", "to_end", "from_role", "to_role"]
    } else if class == DEPENDENCY {
        &["name"]
    } else {
        &[]
    }
}

/// The text of an association's, an association class's or a dependency's slot a label shows: an
/// end's role and its text after it, as [`end_role`] and [`end_text`] write them.
pub fn slot_text(store: &Store, element: Obj, slot: &str) -> String {
    let end = |i: usize| member_ends(store, element).map(|e| e[i]);
    let ends = store.class_of(element).is_ok_and(crate::is_association);
    match slot {
        "name" => name(store, element),
        "from_end" if ends => end(0).map(|e| end_text(store, e)).unwrap_or_default(),
        "to_end" if ends => end(1).map(|e| end_text(store, e)).unwrap_or_default(),
        "from_role" if ends => end(0).map(|e| end_role(store, e)).unwrap_or_default(),
        "to_role" if ends => end(1).map(|e| end_role(store, e)).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The most lines of a constraint's expression its note lists.
const MAX_EXPRESSION_LINES: usize = 64;

/// A constraint as `diagram` shows it (AR-133): braces, its one row `{expression}` as written; or a
/// constraint note, `Constraint · not evaluated` in its eyebrow, its name, and its expression's
/// lines between braces, as written. The expression is never read for meaning.
fn constraint_card(store: &Store, c: Obj, diagram: Obj) -> Card {
    let form = crate::constraints::shown_on(store, diagram, c).map(|(_, f)| f);
    let expression = crate::constraints::expression(store, c);
    let plain = |text: String| Row { mark: None, text, linked: false, slanted: false, underlined: false };
    let (tone, rows) = if form == Some(crate::constraints::Form::Braces) {
        (Tone::Braces, vec![plain(cut(crate::constraints::braces(store, c)))])
    } else {
        let lines: Vec<&str> = expression.lines().collect();
        let last = lines.len().saturating_sub(1);
        let rows = lines
            .iter()
            .take(MAX_EXPRESSION_LINES)
            .enumerate()
            .map(|(i, l)| {
                let open = if i == 0 { "{ " } else { "  " };
                let close = if i == last { " }" } else { "" };
                plain(cut(format!("{open}{l}{close}")))
            })
            .collect();
        (Tone::Constraint, rows)
    };
    Card {
        eyebrow: Text::said("Constraint · not evaluated"),
        name: name(store, c),
        compartments: vec![rows],
        more: ("+ {n} more line", "+ {n} more lines"),
        hidden: expression.lines().count().saturating_sub(MAX_EXPRESSION_LINES),
        elsewhere: false,
        tone,
        slanted: false,
        braces: None,
        template: Vec::new(),
    }
}

/// A package as a tabbed card: its name, which its tab shows, over a thumbnail of its own diagram
/// that the canvas draws (AR-113), so it lists no rows.
fn package_card(store: &Store, p: Obj) -> Card {
    Card {
        eyebrow: Text::said("Package"),
        name: name(store, p),
        compartments: Vec::new(),
        more: ("+ {n} more element", "+ {n} more elements"),
        hidden: 0,
        elsewhere: false,
        tone: Tone::Tabbed,
        slanted: false,
        braces: None,
        template: Vec::new(),
    }
}
