//! What the top bar's search finds in an open project: its diagrams and its
//! elements (schemas, tables, columns, relationships, domains, enum types, sequences, views,
//! skills and notes) by name, each with what it is and where it lives. The index is made once the
//! list opens and again only when the model changes, so typing stays quick in a large model. A
//! diagram opens in its tab; an element is selected, its row shows in the Models tree, and the
//! canvas brings it into view on the open diagram, or on the first diagram that shows it.

use archforge_diagram::{DIAGRAM, NODE, diagram, node};
use archforge_mof::{Obj, Store, Uuid};
use serde_json::json;

use craft_i18n::Folded;

use crate::Shell;
use crate::i18n::tr;
use crate::ux::{self, icons};

/// The most diagrams and elements the list shows.
pub(crate) const MAX_DIAGRAMS: usize = 5;
pub(crate) const MAX_ELEMENTS: usize = 10;
/// The longest label or detail kept for a row; a note's first line is cut there too.
const MAX_TEXT: usize = 120;

/// One thing the search can find: a diagram, or an element (a note's node is one too).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Found {
    Diagram(Uuid),
    Element(Uuid),
}

/// A row of the index: what it finds, its icon, its name, what it is and where, its kind's place
/// in the list, its name folded for ranking and its words folded for matching.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub found: Found,
    pub icon: &'static ux::Icon,
    pub label: String,
    pub detail: String,
    kind: u8,
    name: Folded,
    text: Folded,
}

/// `s` cut to [`MAX_TEXT`] characters, never inside a letter: a letter the cut would part from
/// its combining marks or the rest of its Hangul syllable goes whole, unless it is the first.
fn cut(s: &str) -> String {
    let mut end = s.char_indices().nth(MAX_TEXT).map_or(s.len(), |(at, _)| at);
    while let (Some(next), Some((at, prev))) = (s.get(end..).and_then(|t| t.chars().next()), s.get(..end).and_then(|t| t.char_indices().next_back()))
        && at > 0
        && crate::home_search::joins(Some(prev), next)
    {
        end = at;
    }
    s.get(..end).unwrap_or_default().to_string()
}

/// The open project's diagrams and elements, in a stable order.
pub(crate) fn index(
    kinds: &[&'static dyn crate::kinds::DiagramKind],
    uis: &[&'static dyn crate::notation_ui::NotationUi],
    store: &Store,
) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut push = |found: Found, icon: &'static ux::Icon, kind: u8, label: String, detail: String| {
        if label.trim().is_empty() {
            return;
        }
        // Matched in full: the cut is only for the row, so it never parts a match from its accent.
        let text = crate::home_search::fold(&format!("{label}\n{detail}"));
        let name = crate::home_search::fold(&label);
        out.push(Entry { found, icon, label: cut(&label), detail: cut(&detail), kind, name, text });
    };
    for o in store.alive() {
        let Ok(id) = store.uuid(o) else { continue };
        let Ok(class) = store.class_of(o) else { continue };
        let place = |kind: String, at: String| if at.is_empty() { kind } else { format!("{kind} · {at}") };
        match class {
            c if c == DIAGRAM => {
                let label = store.get(o, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                push(Found::Diagram(id), crate::kinds::diagram_icon(kinds, store, o), 0, label, tr("Diagram"));
            }
            c if c == archforge_core::SKILL => push(Found::Element(id), &icons::SKILL, 6, archforge_core::skills::name_of(store, o), tr("Skill")),
            // A note: a node that shows no element, found by its text's first line.
            c if c == NODE && store.get(o, node::ELEMENT).ok().and_then(|v| v.as_ref()).is_none() => {
                let text = store.get(o, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
                let on = archforge_diagram::diagram_of(store, o)
                    .and_then(|d| store.get(d, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)))
                    .unwrap_or_default();
                push(Found::Element(id), &icons::TOOL_NOTE, 7, first, place(tr("Note"), on));
            }
            // An element of a notation, as its notation lists it.
            _ => {
                if let Some(e) = crate::notation_ui::of(uis, store, o).and_then(|ui| ui.entry(store, o)) {
                    push(Found::Element(id), e.icon, e.order, e.label, e.detail);
                }
            }
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.as_str().cmp(b.name.as_str())).then_with(|| a.detail.cmp(&b.detail)));
    out
}

/// How well a row's name answers the query's first word, both folded: the name itself, its
/// start, a word's start inside it, anywhere. Starts are folded offsets on the search's own
/// boundaries (`home_search::starts`), so `ハ` doesn't start `バ`, `fi` doesn't start `ﬃles` and
/// an accent left out of `Réorder` parts no word. A word starts after a character that is no
/// letter or digit, looking past the marks a letter keeps (`バ`'s voicing mark).
fn rank(e: &Entry, first: &str) -> u8 {
    let text = e.name.as_str();
    if text == first {
        return 0;
    }
    let before = |at: usize| text.get(..at).and_then(|t| t.chars().rev().find(|&c| !crate::home_search::joins(None, c)));
    let word = |at: usize| before(at).is_some_and(|c| !c.is_alphanumeric());
    crate::home_search::starts(&e.name, first).map(|at| if at == 0 { 1 } else { 3 - u8::from(word(at)) }).min().unwrap_or(3)
}

/// The diagrams and the elements whose name or place holds every word of `query`, whatever its
/// capitals and accents: the best answers first, at most [`MAX_DIAGRAMS`] and [`MAX_ELEMENTS`].
pub(crate) fn hits<'a>(index: &'a [Entry], query: &str) -> (Vec<&'a Entry>, Vec<&'a Entry>) {
    let typed = crate::home_search::Typed::words(query);
    let mut found: Vec<&Entry> = index.iter().filter(|e| typed.in_folded(&[&e.text])).collect();
    if let Some(first) = typed.first() {
        found.sort_by_key(|e| (rank(e, first), e.kind));
    }
    let diagrams = found.iter().filter(|e| matches!(e.found, Found::Diagram(_))).take(MAX_DIAGRAMS).copied().collect();
    let elements = found.iter().filter(|e| matches!(e.found, Found::Element(_))).take(MAX_ELEMENTS).copied().collect();
    (diagrams, elements)
}

/// Goes to what a row found: a diagram opens in its tab; an element is selected and shown on the
/// open diagram, or on the first diagram that draws it, which then opens.
pub(crate) fn go(shell: &mut Shell, found: Found) {
    match found {
        Found::Diagram(id) => crate::canvas::open(shell, id),
        Found::Element(id) => {
            let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
            let Some(o) = store.by_uuid(id) else { return };
            // What the canvas draws for it: a column's table, a relationship's line's first table,
            // a note's own node.
            let shown = match crate::notation_ui::of(shell.notation_uis, store, o) {
                Some(ui) => ui.shown(store, o),
                None => Some(o),
            };
            let nodes: Vec<Obj> = match shown {
                Some(n) if store.class_of(n).ok() == Some(NODE) => vec![n],
                Some(e) => archforge_diagram::views_of(store, e).2,
                None => Vec::new(),
            };
            let active = shell.canvas.active.and_then(|a| store.by_uuid(a));
            let node = nodes.iter().copied().find(|n| archforge_diagram::diagram_of(store, *n) == active).or_else(|| nodes.first().copied());
            let target = node.and_then(|n| {
                let d = archforge_diagram::diagram_of(store, n)?;
                Some((store.uuid(d).ok()?, store.uuid(n).ok()?))
            });
            if let Some((d, n)) = target {
                if shell.canvas.active != Some(d) {
                    crate::canvas::open(shell, d);
                }
                shell.canvas.centre_on = Some(n);
            }
            if let Some(session) = shell.session.as_mut()
                && let Err(e) = session.execute("view.select", &json!({ "ids": [id.to_string()] }))
            {
                shell.canvas.notice = Some(crate::panels::refusal(&e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project named Shop, its ids from a counter.
    fn session() -> archforge_notations::Session {
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        });
        archforge_notations::new_project_session("Shop", ids).unwrap()
    }

    /// Every word must be found, and the name itself, then its start, then a word's start inside
    /// it comes first.
    #[test]
    fn the_index_finds_diagrams_and_elements_best_answers_first() {
        let mut s = session();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &json!({ "name": "shop" })).unwrap().value);
        let order = id(s.execute("er.entity.add", &json!({ "schema": schema, "name": "Order" })).unwrap().value);
        id(s.execute("er.entity.add", &json!({ "schema": schema, "name": "Customer_order" })).unwrap().value);
        s.execute("er.attribute.add", &json!({ "entity": order, "name": "order_id", "type": "BIGINT" })).unwrap();
        s.execute("diagram.add", &json!({ "kind": "er", "element": schema, "name": "Orders" })).unwrap();
        let index = index(crate::kinds::KINDS, crate::notation_ui::UIS, s.doc());
        let (diagrams, elements) = hits(&index, "order");
        assert_eq!(diagrams.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), ["Orders"]);
        // The table named so first, then what starts so, then a word inside a name.
        assert_eq!(elements.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), ["Order", "order_id", "Customer_order"]);
        // Every word must be found, in the name or where it lives.
        let (_, both) = hits(&index, "ORDER_ID order");
        assert_eq!(both.len(), 1);
        assert!(hits(&index, "nothing like it").1.is_empty());
    }

    /// Capitals, accents and `ß` count for nothing, and the best answers still come first.
    #[test]
    fn the_index_ignores_capitals_and_accents_and_keeps_the_best_answers_first() {
        let mut s = session();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &json!({ "name": "shop" })).unwrap().value);
        let check = id(s.execute("er.entity.add", &json!({ "schema": schema, "name": "Prüfung" })).unwrap().value);
        s.execute("er.entity.add", &json!({ "schema": schema, "name": "Kunde_prüfung" })).unwrap();
        for name in ["Bestellungen prüfen", "Straße"] {
            s.execute("er.entity.add", &json!({ "schema": schema, "name": name })).unwrap();
        }
        s.execute("er.attribute.add", &json!({ "entity": check, "name": "prüfung_id", "type": "BIGINT" })).unwrap();
        let index = index(crate::kinds::KINDS, crate::notation_ui::UIS, s.doc());
        let labels = |query: &str| hits(&index, query).1.iter().map(|e| e.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels("PRUFUNG"), ["Prüfung", "prüfung_id", "Kunde_prüfung"]);
        assert_eq!(labels("bestellung"), ["Bestellungen prüfen"]);
        assert_eq!(labels("prufen"), ["Bestellungen prüfen"]);
        assert_eq!(labels("STRASSE"), ["Straße"]);
        // A zero-width space before the first word leaves the ranking to it.
        assert_eq!(labels("\u{200B} PRUFUNG"), ["Prüfung", "prüfung_id", "Kunde_prüfung"]);
        // A name past the row's cut is matched in full, its last letter's accent with it.
        let long = format!("{}Cafe\u{301}", "a".repeat(116));
        s.execute("er.entity.add", &json!({ "schema": schema, "name": long })).unwrap();
        assert_eq!(hits(&super::index(crate::kinds::KINDS, crate::notation_ui::UIS, s.doc()), "cafe").1.len(), 1);
    }

    /// A name starts with the first word only where the search would match it, after what folds
    /// to nothing: `ハ` doesn't start `バ`, whose voicing mark makes it another letter.
    #[test]
    fn a_name_starts_with_a_word_only_where_the_search_matches_it() {
        let fold = crate::home_search::fold;
        let entry = |label: &str| Entry {
            found: Found::Element(Uuid([1; 16])),
            icon: &icons::TABLE,
            label: label.into(),
            detail: String::new(),
            kind: 2,
            name: fold(label),
            text: fold(label),
        };
        let ranks = ["Prüfung", "\u{AD}Prüfungen", "Kunde_prüfung", "Abprüfung"].map(|label| rank(&entry(label), fold("PRUFUNG").as_str()));
        assert_eq!(ranks, [0, 1, 2, 3]);
        let cases = [
            ("ﬃles", "fi", 3),
            ("ffiles", "fi", 3),
            ("Réorder", "order", 3),
            ("Re\u{301}order", "order", 3),
            ("Kunde_order", "order", 2),
            ("xid-id-id", "id-id", 2),
            ("ㅋㅠ", "ㅋ", 1),
            ("バナナ", "ナナ", 3),
            ("猫_ナナ", "ナナ", 2),
        ];
        for (label, word, wanted) in cases {
            assert_eq!(rank(&entry(label), fold(word).as_str()), wanted, "{label} {word}");
        }
        // The cut never parts a letter from its voicing mark: ハ alone isn't kept.
        assert_eq!(cut(&format!("{}ハ\u{3099}", "a".repeat(119))), "a".repeat(119));
        assert_eq!((rank(&entry("ハ予定"), fold("ハ").as_str()), rank(&entry("バ"), fold("ハ").as_str())), (1, 3));
    }
}
