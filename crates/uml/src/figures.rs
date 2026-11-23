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
//! its eyebrow Association class (AR-125), which the canvas ties to the line's middle; a template
//! binding dashed, with an open arrow at its template and its `«bind» <T -> Order>` label at its
//! middle (AR-131). An element's stereotypes (AR-135) take the place of a card's kind word (a
//! keyword kind keeps its keyword first), start a member's row and sit at a line's middle; a diagram
//! that shows them lists the values set in a compartment per stereotype. A package's card names
//! its owners from the model's root down to its parent (AR-564), and is drawn under the diagram's
//! other figures (AR-570). The canvas draws these descriptions; rows, ends and keywords are written
//! from plain values the cards fill from the model, as values from elsewhere can be (AR-669).

use archforge_diagram::figure::{Card, End, EndMark, Line, Row, Text, Tone};
use archforge_mof::{Obj, Store, Value};

use crate::{
    ASSOCIATION, ASSOCIATION_CLASS, AggregationKind, CLASS, DATA_TYPE, DEPENDENCY, ENUMERATION, GENERALIZATION, INTERFACE, INTERFACE_REALIZATION,
    PRIMITIVE_TYPE, ParameterDirectionKind, TEMPLATE_BINDING, VisibilityKind, association, behavioral_feature, class, dependency, generalization,
    interface_realization, literal_integer, literal_unlimited_natural, multiplicity_element, named_element, operation, parameter, property,
    typed_element,
};

/// The most parameters an operation's row names, and the longest row a card draws: a row is
/// built every frame, so neither grows with what the model holds.
pub const MAX_PARAMETERS: usize = 8;
const MAX_ROW: usize = 200;
/// The most rows a card lists, across its compartments: a card is at most
/// [`archforge_diagram::rules::MAX_SIZE`] tall, which holds no more than this many rows, so the
/// rows built every frame never grow with the model. The rest are counted in `hidden`, which the
/// canvas says in the card's `+ n more` row.
pub const MAX_CARD_ROWS: usize = 500;
/// The most owners a package's card names in its path, the nearest first kept: the path is built
/// every frame, so it never grows with how deep the model nests.
const MAX_OWNERS: usize = 256;

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

/// The sign of a visibility: `+` public, `-` private, `#` protected, `~` package.
pub fn visibility_mark(v: VisibilityKind) -> &'static str {
    match v {
        VisibilityKind::Public => "+",
        VisibilityKind::Private => "-",
        VisibilityKind::Protected => "#",
        VisibilityKind::Package => "~",
    }
}

/// An element's visibility, when it has one the model knows.
fn visibility_kind(store: &Store, o: Obj) -> Option<VisibilityKind> {
    match store.get(o, named_element::VISIBILITY).ok()? {
        Value::Enum(i) => VisibilityKind::from_index(i),
        _ => None,
    }
}

/// The sign of an element's visibility, when it has one.
pub fn visibility(store: &Store, o: Obj) -> Option<&'static str> {
    visibility_kind(store, o).map(visibility_mark)
}

/// A multiplicity's bounds as plain values (AR-669): `lower` and `upper` None when the element
/// names none, `upper` `Some(None)` when it is unlimited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub lower: Option<i64>,
    pub upper: Option<Option<u64>>,
}

/// A multiplicity's bounds as written: `1`, `0..1`, `*` (none to many), `1..*`; empty when the
/// element names none.
pub fn multiplicity(store: &Store, o: Obj) -> String {
    multiplicity_text(bounds(store, o))
}

/// The bounds of element `o`'s multiplicity, as the model holds them.
pub fn bounds(store: &Store, o: Obj) -> Bounds {
    let lower = one(store, o, multiplicity_element::LOWER_VALUE).and_then(|v| match store.get(v, literal_integer::VALUE).ok()? {
        Value::Int(n) => Some(n),
        _ => None,
    });
    let upper = one(store, o, multiplicity_element::UPPER_VALUE).and_then(|v| match store.get(v, literal_unlimited_natural::VALUE).ok()? {
        Value::Unlimited(None) => Some(None),
        Value::Unlimited(Some(n)) => Some(Some(n)),
        _ => None,
    });
    Bounds { lower, upper }
}

/// Bounds as a multiplicity writes them: `1`, `0..1`, `*` (none to many), `1..*`, a bound not
/// named read as 1; empty when neither is named.
pub fn multiplicity_text(Bounds { lower, upper }: Bounds) -> String {
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

/// An attribute or an association end as plain values (AR-669), from the model by
/// [`PropertyValues::of`] or from a caller that has them elsewhere, so both write the same text;
/// `default` as a card shows it, `braces` the words between them (`readOnly`, `subsets lines`, …).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyValues {
    pub visibility: Option<VisibilityKind>,
    pub derived: bool,
    pub name: String,
    pub type_name: Option<String>,
    pub bounds: Bounds,
    pub default: Option<String>,
    pub braces: Vec<String>,
}

impl PropertyValues {
    /// The values property `p` has in the model.
    pub fn of(store: &Store, p: Obj) -> Self {
        Self {
            visibility: visibility_kind(store, p),
            derived: crate::modifiers::is_derived(store, p),
            name: name(store, p),
            type_name: type_name(store, p),
            bounds: bounds(store, p),
            default: crate::modifiers::default_shown(store, p),
            braces: crate::modifiers::braces(store, p),
        }
    }

    /// The property as a card's row writes it: `+ / total : Money [0..1] = 0 {ordered}`, the
    /// multiplicity left out when it is `1` or unnamed.
    pub fn attribute_text(&self) -> String {
        let mut text = self.visibility.map(|v| format!("{} ", visibility_mark(v))).unwrap_or_default();
        if self.derived {
            text.push_str("/ ");
        }
        text.push_str(&self.name);
        if let Some(ty) = &self.type_name {
            text.push_str(&format!(" : {ty}"));
        }
        let m = multiplicity_text(self.bounds);
        if !m.is_empty() && m != "1" {
            text.push_str(&format!(" [{m}]"));
        }
        text.push_str(&modifier_text(self.default.as_deref(), &self.braces));
        cut(text)
    }

    /// The end's role as its line writes it: its name, `/` before it when the end is derived.
    pub fn end_role(&self) -> String {
        if self.derived { cut(format!("/{}", self.name)) } else { self.name.clone() }
    }

    /// What the end's line writes after the role: its multiplicity, then `= default` and the
    /// braces, as in `0..* = none {ordered}`; empty when it has none of them.
    pub fn end_text(&self) -> String {
        cut(format!("{}{}", multiplicity_text(self.bounds), modifier_text(self.default.as_deref(), &self.braces)).trim_start().to_string())
    }
}

/// What follows a property's multiplicity: ` = default` when it has one, then ` {…}` when
/// `braces` holds any word, as in ` = 0 {readOnly, ordered}`; empty otherwise.
pub fn modifier_text(default: Option<&str>, braces: &[String]) -> String {
    let mut text = default.map(|d| format!(" = {d}")).unwrap_or_default();
    if !braces.is_empty() {
        text.push_str(&format!(" {{{}}}", braces.join(", ")));
    }
    text
}

/// An attribute as a class's card lists it: `+ total : Money [0..1]`; a derived one with `/ `
/// before its name, then its default after `=` and what differs from the usual in braces (AR-126,
/// as the design writes it: `+ / total : Money = 0`, `+ lines : OrderLine [1..*] {ordered}`).
pub fn attribute_text(store: &Store, a: Obj) -> String {
    PropertyValues::of(store, a).attribute_text()
}

/// An association end's role as its line writes it: `/` before it when the end is derived; empty
/// when it has neither. Only the name and the derived flag are read; a line that writes the end's
/// text too reads [`PropertyValues::of`] once for both.
pub fn end_role(store: &Store, end: Obj) -> String {
    PropertyValues { derived: crate::modifiers::is_derived(store, end), name: name(store, end), ..PropertyValues::default() }.end_role()
}

/// What an association end's line writes beside its end after the role: its multiplicity, then
/// `= default` and the braces, as in `0..* = none {ordered}` (AR-126); empty when it has none.
/// Only those are read.
pub fn end_text(store: &Store, end: Obj) -> String {
    let (default, braces) = (crate::modifiers::default_shown(store, end), crate::modifiers::braces(store, end));
    PropertyValues { bounds: bounds(store, end), default, braces, ..PropertyValues::default() }.end_text()
}

/// An operation as plain values (AR-669), from the model by [`OperationValues::of`] or from a
/// caller: `inputs` its parameters but the return one, name and type, past [`MAX_PARAMETERS`]
/// written `…`; `returns` its return type; `leaf` when nothing may redefine it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationValues {
    pub visibility: Option<VisibilityKind>,
    pub name: String,
    pub inputs: Vec<(String, Option<String>)>,
    pub returns: Option<String>,
    pub leaf: bool,
}

impl OperationValues {
    /// The values operation `op` has in the model; of its inputs, one more than
    /// [`MAX_PARAMETERS`] at most, which is enough to say there are more.
    pub fn of(store: &Store, op: Obj) -> Self {
        let direction = |p: Obj| match store.get(p, parameter::DIRECTION).ok() {
            Some(Value::Enum(i)) => ParameterDirectionKind::from_index(i),
            _ => None,
        };
        let params = list(store, op, operation::OWNED_PARAMETER);
        let inputs = params
            .iter()
            .filter(|p| direction(**p) != Some(ParameterDirectionKind::Return))
            .take(MAX_PARAMETERS.saturating_add(1))
            .map(|p| (name(store, *p), type_name(store, *p)))
            .collect();
        let returns = params.iter().find(|p| direction(**p) == Some(ParameterDirectionKind::Return)).and_then(|p| type_name(store, *p));
        Self { visibility: visibility_kind(store, op), name: name(store, op), inputs, returns, leaf: crate::relations::is_leaf(store, op) }
    }

    /// The operation as a card's row writes it: `+ total(currency : Currency) : Money`, then
    /// `{leaf}` when nothing may redefine it.
    pub fn text(&self) -> String {
        let mut inputs: Vec<String> = self
            .inputs
            .iter()
            .take(MAX_PARAMETERS)
            .map(|(name, ty)| match ty {
                Some(ty) => format!("{name} : {ty}"),
                None => name.clone(),
            })
            .collect();
        if self.inputs.len() > MAX_PARAMETERS {
            inputs.push("…".into());
        }
        let mut text = self.visibility.map(|v| format!("{} ", visibility_mark(v))).unwrap_or_default();
        text.push_str(&format!("{}({})", self.name, inputs.join(", ")));
        if let Some(ty) = &self.returns {
            text.push_str(&format!(" : {ty}"));
        }
        if self.leaf {
            text.push_str(" {leaf}");
        }
        cut(text)
    }
}

/// An operation as a class's card lists it: `+ total(currency : Currency) : Money`, then `{leaf}`
/// when nothing may redefine it (AR-138).
pub fn operation_text(store: &Store, op: Obj) -> String {
    OperationValues::of(store, op).text()
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
        // A stereotype and a metaclass on a profile's diagram (AR-134, F1).
        k if k == crate::STEREOTYPE => return Some(stereotype_card(store, c)),
        k if k == CLASS && crate::profiles::is_metaclass(store, c) => return Some(metaclass_card(store, c)),
        k if k == ENUMERATION => return Some(enumeration_card(store, c, diagram)),
        k if k == PRIMITIVE_TYPE => return Some(primitive_type_card(store, c, diagram)),
        k if k == CLASS => ("Class", "Class · {package}", true),
        k if k == INTERFACE => ("«interface»", "«interface» · {package}", false),
        k if k == DATA_TYPE => ("«dataType»", "«dataType» · {package}", false),
        k if k == ASSOCIATION_CLASS => ("Association class", "Association class · {package}", false),
        k if k == crate::CONSTRAINT => return Some(constraint_card(store, c, diagram)),
        k if k == ASSOCIATION => return crate::nary::is_nary(store, c).then(|| diamond_card(store, c)),
        _ => return None,
    };
    let elsewhere = elsewhere(store, c, diagram);
    let eyebrow = eyebrow(store, c, here, away, elsewhere);
    // A static member underlined, an abstract operation slanted: a static abstract one is both.
    let row = |text, member, slanted| Row { mark: None, text, linked: false, slanted, underlined: is_static(store, member) };
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    // An association's end a class owns draws as the line's role, not as a row: the members'
    // attributes leave it out. A member's stereotypes go before its text (AR-135).
    let attributes = capped(crate::members::attributes(store, c), &mut room, &mut hidden, |a| row(member_text(store, a), a, false));
    let operations = capped(crate::members::operations(store, c), &mut room, &mut hidden, |op| {
        row(member_text(store, op), op, flag(store, op, behavioral_feature::IS_ABSTRACT))
    });
    let mut compartments = vec![attributes, operations];
    compartments.extend(value_compartments(store, c, diagram, &mut room, &mut hidden));
    Some(Card {
        eyebrow,
        name: card_name(store, c, diagram),
        compartments,
        more: ("+ {n} more member", "+ {n} more members"),
        hidden,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: is_class && flag(store, c, class::IS_ABSTRACT),
        braces: braces(store, c),
        template: crate::templates::signature_rows(store, c),
        owner_path: Vec::new(),
    })
}

/// Grows every node of `element`, on every diagram, too short for the rows of the card it draws
/// there, to the card's height (AR-241): its width is kept and no node is ever made shorter, so a
/// card made taller by hand keeps its height. The commands that add or edit a member call it, so
/// the window, the control channel and Chat grow a card alike, in the command's own undo step.
pub fn grow_cards(store: &mut Store, element: Obj) -> Result<(), archforge_mof::Invalid> {
    for n in archforge_diagram::views_of(store, element).2 {
        grow_node(store, element, n)?;
    }
    Ok(())
}

/// Grows node `n`, which shows `element`, to the height of the card it draws on its diagram when
/// it is shorter, its width kept; leaves it as it is otherwise. Only a plain card, a classifier's,
/// which lists rows, ever grows: a package's folder, a note and an n-ary association's diamond keep
/// their size, as does a node that draws no card.
pub fn grow_node(store: &mut Store, element: Obj, n: Obj) -> Result<(), archforge_mof::Invalid> {
    let Some(d) = archforge_diagram::diagram_of(store, n) else { return Ok(()) };
    let Some(card) = card(store, element, d).filter(|c| c.tone == Tone::Plain) else { return Ok(()) };
    let needed = f64::from(card.height()).min(archforge_diagram::rules::MAX_SIZE);
    if archforge_diagram::grid::bounds_of(store, n).height < needed {
        store.set(n, archforge_diagram::node::HEIGHT, Value::Real(needed))?;
    }
    Ok(())
}

/// A member's row as a card or Properties lists it: its stereotypes, then its text, `«Id» + id :
/// Integer` (AR-135, F3).
pub fn member_text(store: &Store, m: Obj) -> String {
    let text = if store.class_of(m).ok() == Some(crate::OPERATION) { operation_text(store, m) } else { attribute_text(store, m) };
    row_text(&stereotypes(store, m), &text)
}

/// The names of the stereotypes applied to `element`, in the order its keywords list them.
pub fn stereotypes(store: &Store, element: Obj) -> Vec<String> {
    crate::applications::applied_to(store, element).into_iter().map(|(s, _)| name(store, s)).collect()
}

/// Stereotypes' names as an element shows them, `«Entity»` or `«Entity, Auditable»`; None for
/// none. A stereotype's name is the model's, never translated.
pub fn stereotype_keywords(stereotypes: &[String]) -> Option<String> {
    (!stereotypes.is_empty()).then(|| format!("«{}»", stereotypes.join(", ")))
}

/// A member's row from its stereotypes' names and its text: `«Id» + id : Integer`, the text alone
/// when it has none (AR-135, F3).
pub fn row_text(stereotypes: &[String], text: &str) -> String {
    match stereotype_keywords(stereotypes) {
        Some(k) => cut(format!("{k} {text}")),
        None => cut(text.to_string()),
    }
}

/// What a classifier's eyebrow says for its stereotypes: their keywords in place of its kind's
/// word `kind` (`«Entity»` for `Class`), a kind written as a UML keyword kept first in the same
/// guillemets (`«interface, Entity»`); None when it has no stereotype, so the kind's word stays.
pub fn kind_keywords(kind: &str, stereotypes: &[String]) -> Option<String> {
    let keywords = stereotype_keywords(stereotypes)?;
    Some(match kind.strip_prefix('«').and_then(|k| k.strip_suffix('»')) {
        Some(kind) => format!("«{kind}, {}", keywords.trim_start_matches('«')),
        None => keywords,
    })
}

/// A classifier's eyebrow: its stereotypes in place of its kind's word, as [`kind_keywords`]
/// writes them; the package beside it for an outsider (AR-135, F3). `here` and `away` are the
/// kind's own eyebrow, alone and with the package.
fn eyebrow(store: &Store, c: Obj, here: &'static str, away: &'static str, elsewhere: Option<Obj>) -> Text {
    let Some(keywords) = kind_keywords(here, &stereotypes(store, c)) else {
        return match elsewhere {
            Some(p) => Text::Said(away, vec![("package", name(store, p))]),
            None => Text::said(here),
        };
    };
    match elsewhere {
        Some(p) => Text::Said("{keywords} · {package}", vec![("keywords", keywords), ("package", name(store, p))]),
        None => Text::Plain(keywords),
    }
}

/// The compartments of the values set on `c`'s stereotypes, one per stereotype with a value set,
/// headed by its `«Entity»`, when `diagram` shows them (AR-135, F3); none otherwise. They count
/// against `room` as the members do.
fn value_compartments(store: &Store, c: Obj, diagram: Obj, room: &mut usize, hidden: &mut usize) -> Vec<Vec<Row>> {
    if !crate::applications::values_shown(store, diagram) {
        return Vec::new();
    }
    let plain = |text: String| Row { mark: None, text: cut(text), linked: false, slanted: false, underlined: false };
    crate::applications::value_rows(store, c)
        .into_iter()
        .map(|(heading, rows)| {
            let all: Vec<String> = std::iter::once(heading).chain(rows).collect();
            let shown = all.len().min(*room);
            *hidden = hidden.saturating_add(all.len().saturating_sub(shown));
            *room = room.saturating_sub(shown);
            all.into_iter().take(shown).map(plain).collect::<Vec<Row>>()
        })
        .filter(|rows| !rows.is_empty())
        .collect()
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
    let eyebrow = eyebrow(store, t, "«primitive»", "«primitive» · {package}", elsewhere);
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    Card {
        eyebrow,
        name: card_name(store, t, diagram),
        compartments: value_compartments(store, t, diagram, &mut room, &mut hidden),
        more: ("+ {n} more member", "+ {n} more members"),
        hidden,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: false,
        braces: braces(store, t),
        template: Vec::new(),
        owner_path: Vec::new(),
    }
}

/// A stereotype's card on its profile's diagram (AR-134, F1): `«stereotype»` in its eyebrow, its
/// name, and its properties as a class's attributes, its extensions' ends left out.
fn stereotype_card(store: &Store, s: Obj) -> Card {
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    let row = |text, member| Row { mark: None, text, linked: false, slanted: false, underlined: is_static(store, member) };
    let properties = capped(crate::profiles::properties(store, s), &mut room, &mut hidden, |a| row(attribute_text(store, a), a));
    Card {
        eyebrow: Text::said("«stereotype»"),
        name: name(store, s),
        compartments: vec![properties],
        more: ("+ {n} more member", "+ {n} more members"),
        hidden,
        elsewhere: false,
        tone: Tone::Plain,
        slanted: false,
        braces: None,
        template: Vec::new(),
        owner_path: Vec::new(),
    }
}

/// A metaclass's card on a profile's diagram (AR-134, F1): `«metaclass»` over its UML name, with
/// no compartment.
fn metaclass_card(store: &Store, m: Obj) -> Card {
    Card {
        eyebrow: Text::said("«metaclass»"),
        name: name(store, m),
        compartments: Vec::new(),
        more: ("+ {n} more member", "+ {n} more members"),
        hidden: 0,
        elsewhere: false,
        tone: Tone::Plain,
        slanted: false,
        braces: None,
        template: Vec::new(),
        owner_path: Vec::new(),
    }
}

/// An enumeration's card on `diagram`: the keyword «enumeration» in its eyebrow, its package beside
/// it when it is an outsider, its name, and one compartment of its literals in their order, empty
/// when it has none; past [`MAX_CARD_ROWS`] the rest are counted, not listed.
fn enumeration_card(store: &Store, e: Obj, diagram: Obj) -> Card {
    let elsewhere = elsewhere(store, e, diagram);
    let eyebrow = eyebrow(store, e, "«enumeration»", "«enumeration» · {package}", elsewhere);
    let (mut room, mut hidden) = (MAX_CARD_ROWS, 0);
    let literals = capped(crate::commands::literals(store, e), &mut room, &mut hidden, |l| Row {
        mark: None,
        text: cut(name(store, l)),
        linked: false,
        slanted: false,
        underlined: false,
    });
    let mut compartments = vec![literals];
    compartments.extend(value_compartments(store, e, diagram, &mut room, &mut hidden));
    Card {
        eyebrow,
        name: card_name(store, e, diagram),
        compartments,
        more: ("+ {n} more literal", "+ {n} more literals"),
        hidden,
        elsewhere: elsewhere.is_some(),
        tone: Tone::Plain,
        slanted: false,
        braces: braces(store, e),
        template: Vec::new(),
        owner_path: Vec::new(),
    }
}

/// A binary association's two ends, in its member order; None for an n-ary one, whose ends
/// [`crate::nary::ends`] gives (AR-128).
pub fn member_ends(store: &Store, a: Obj) -> Option<[Obj; 2]> {
    match list(store, a, association::MEMBER_END).as_slice() {
        [first, second] => Some([*first, *second]),
        _ => None,
    }
}

/// An n-ary association's diamond (AR-128): its name, which the canvas writes above the diamond,
/// and no rows; its stereotypes, when it has any, in its eyebrow, written above the name (AR-135).
fn diamond_card(store: &Store, a: Obj) -> Card {
    Card {
        eyebrow: crate::applications::keywords(store, a).map_or_else(|| Text::said("N-ary association"), Text::Plain),
        name: name(store, a),
        compartments: Vec::new(),
        more: ("+ {n} more member", "+ {n} more members"),
        hidden: 0,
        elsewhere: false,
        tone: Tone::Diamond,
        slanted: false,
        braces: None,
        template: Vec::new(),
        owner_path: Vec::new(),
    }
}

/// The leg of an n-ary association's end `end` (AR-128): from the diamond, unmarked, to the end's
/// class, an open arrow there when the association navigates to it, the end's role and its
/// multiplicity beside it.
fn leg_line(store: &Store, end: Obj) -> Option<Line> {
    let a = crate::nary::association_of_end(store, end)?;
    let values = PropertyValues::of(store, end);
    let (m, role) = (values.end_text(), values.end_role());
    let to = End {
        mark: if navigable(store, a, end) { EndMark::OpenArrow } else { EndMark::None },
        text: (!m.is_empty()).then_some(m),
        slot: "to_end",
        role: (!role.is_empty()).then_some(role),
        qualifiers: Vec::new(),
    };
    // A leg is its end's: the end's stereotypes sit at its middle (AR-135).
    Some(Line {
        ends: [End::marked(EndMark::None), to],
        styled: true,
        dashed: false,
        status: None,
        note: None,
        badge: false,
        resolvable: false,
        keyword: crate::applications::keywords(store, end),
    })
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

/// The classes an association, an association class, a generalization, a dependency, an interface
/// realization, a template binding or an extension joins, as its line runs: a realization from its
/// implementing class to its interface, a template binding from its bound element to its template,
/// an extension from its stereotype to its metaclass.
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
        // An extension runs from its stereotype to the metaclass it extends (AR-134).
        c if c == crate::EXTENSION => crate::profiles::extension_ends(store, element),
        c if c == TEMPLATE_BINDING => Some((crate::templates::bound_element(store, element)?, crate::bindings::template_of(store, element)?)),
        _ => None,
    }
}

/// The line of an association, an association class (an association's line: its card is drawn
/// apart), a generalization, a dependency or an interface realization.
pub fn line(store: &Store, element: Obj) -> Option<Line> {
    match store.class_of(element).ok()? {
        c if c == crate::PROPERTY => leg_line(store, element),
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
                let values = PropertyValues::of(store, here);
                let (m, role) = (values.end_text(), values.end_role());
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
            Some(Line {
                ends: [end(0), end(1)],
                styled: true,
                dashed: false,
                status: None,
                note: None,
                badge: false,
                resolvable: false,
                keyword: crate::applications::keywords(store, element),
            })
        }
        // An extension: a solid line with a filled triangle at the metaclass (AR-134, F1).
        c if c == crate::EXTENSION => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::FilledTriangle)],
            styled: true,
            dashed: false,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
            keyword: crate::applications::keywords(store, element),
        }),
        c if c == GENERALIZATION => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::Triangle)],
            styled: false,
            dashed: false,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
            keyword: crate::applications::keywords(store, element),
        }),
        c if c == DEPENDENCY => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::OpenArrow)],
            styled: true,
            dashed: true,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
            keyword: crate::applications::keywords(store, element),
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
            keyword: crate::applications::keywords(store, element),
        }),
        // Dashed with an open arrow at the template, as a dependency's, and labelled «bind» with its
        // substitutions: a binding is neither a dependency nor a realization (AR-131).
        c if c == TEMPLATE_BINDING => Some(Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::OpenArrow)],
            styled: true,
            dashed: true,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
            keyword: crate::applications::keywords(store, element),
        }),
        _ => None,
    }
}

/// The text slots a label of an association or a dependency may show; an association class's line
/// its ends' alone, its name being on its card; a template binding's its `«bind»` label, in the
/// name's slot, as it has no name.
pub fn text_slots(class: archforge_mof::ClassId) -> &'static [&'static str] {
    if class == ASSOCIATION {
        &["name", "from_end", "to_end", "from_role", "to_role"]
    } else if class == ASSOCIATION_CLASS {
        &["from_end", "to_end", "from_role", "to_role"]
    } else if class == DEPENDENCY || class == TEMPLATE_BINDING {
        &["name"]
    } else if class == crate::PROPERTY {
        // An n-ary association's leg: its end's texts at the class (AR-128).
        &["to_end", "to_role"]
    } else {
        &[]
    }
}

/// The text of an association's, an association class's or a dependency's slot a label shows: an
/// end's role and its text after it, as [`end_role`] and [`end_text`] write them.
pub fn slot_text(store: &Store, element: Obj, slot: &str) -> String {
    let end = |i: usize| member_ends(store, element).map(|e| e[i]);
    let ends = store.class_of(element).is_ok_and(crate::is_association);
    // An n-ary association's leg writes its own end's texts (AR-128).
    if crate::nary::association_of_end(store, element).is_some() {
        return match slot {
            "to_end" => end_text(store, element),
            "to_role" => end_role(store, element),
            _ => String::new(),
        };
    }
    match slot {
        "name" if store.class_of(element).ok() == Some(TEMPLATE_BINDING) => crate::bindings::label(store, element),
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
        owner_path: Vec::new(),
    }
}

/// A package as a tabbed card: its name, which its tab shows, over a thumbnail of its own diagram
/// that the canvas draws (AR-113), so it lists no rows; and the names of its owners, which say where
/// it lives (AR-564).
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
        owner_path: owner_path(store, p),
    }
}

/// The names of the packages that own the package `p`, from the model's root down to its parent:
/// empty for the root, which has none. At most [`MAX_OWNERS`] of them, the nearest kept, since a
/// card is built every frame.
pub(crate) fn owner_path(store: &Store, p: Obj) -> Vec<String> {
    let mut owners = Vec::new();
    let mut at = p;
    while owners.len() < MAX_OWNERS {
        let Some(owner) = crate::commands::package_of(store, at) else { break };
        owners.push(name(store, owner));
        at = owner;
    }
    owners.reverse();
    owners
}
