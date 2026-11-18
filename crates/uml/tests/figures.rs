//! The value formatter of a card's rows and a line's ends (AR-669): the text of a member, an end,
//! a multiplicity and a classifier's keywords, written from plain values with no model behind them,
//! as the card code writes it from the model.

use archforge_uml::VisibilityKind;
use archforge_uml::figures::{self, Bounds, MAX_PARAMETERS, OperationValues, PropertyValues};

fn s(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn every_visibility_has_its_mark_and_none_writes_none() {
    let marks = [(VisibilityKind::Public, "+"), (VisibilityKind::Private, "-"), (VisibilityKind::Protected, "#"), (VisibilityKind::Package, "~")];
    for (v, mark) in marks {
        assert_eq!(figures::visibility_mark(v), mark);
        let p = PropertyValues { visibility: Some(v), name: "id".into(), ..Default::default() };
        assert_eq!(p.attribute_text(), format!("{mark} id"));
    }
    assert_eq!(PropertyValues { name: "id".into(), ..Default::default() }.attribute_text(), "id");
}

#[test]
fn multiplicities_are_written_in_their_short_forms() {
    let cases = [
        (None, None, ""),
        (Some(1), None, "1"),
        (None, Some(Some(1)), "1"),
        (Some(3), Some(Some(3)), "3"),
        (Some(0), None, "0..1"),
        (Some(0), Some(Some(1)), "0..1"),
        (Some(2), Some(Some(5)), "2..5"),
        (Some(0), Some(None), "*"),
        (Some(1), Some(None), "1..*"),
        (None, Some(None), "1..*"),
    ];
    for (lower, upper, text) in cases {
        assert_eq!(figures::multiplicity_text(Bounds { lower, upper }), text, "{lower:?}..{upper:?}");
    }
}

#[test]
fn an_attribute_writes_every_part_in_order_and_leaves_out_a_single_multiplicity() {
    let full = PropertyValues {
        visibility: Some(VisibilityKind::Public),
        derived: true,
        name: "total".into(),
        type_name: Some("Money".into()),
        bounds: Bounds { lower: Some(0), upper: Some(Some(1)) },
        default: Some("0".into()),
        braces: s(&["readOnly", "ordered"]),
    };
    assert_eq!(full.attribute_text(), "+ / total : Money [0..1] = 0 {readOnly, ordered}");
    let one = PropertyValues {
        name: "id".into(),
        type_name: Some("Integer".into()),
        bounds: Bounds { lower: Some(1), upper: Some(Some(1)) },
        ..Default::default()
    };
    assert_eq!(one.attribute_text(), "id : Integer", "exactly one is not written");
    let braces =
        PropertyValues { name: "lines".into(), bounds: Bounds { lower: Some(1), upper: Some(None) }, braces: s(&["leaf"]), ..Default::default() };
    assert_eq!(braces.attribute_text(), "lines [1..*] {leaf}");
    let long = PropertyValues { name: "n".repeat(300), ..Default::default() };
    assert_eq!(long.attribute_text().chars().count(), 200);
    assert!(long.attribute_text().ends_with('…'));
}

#[test]
fn an_end_writes_its_role_and_its_multiplicity_with_its_modifiers() {
    let mut end = PropertyValues {
        name: "items".into(),
        bounds: Bounds { lower: Some(0), upper: Some(None) },
        default: Some("none".into()),
        ..Default::default()
    };
    end.braces = s(&["ordered"]);
    assert_eq!((end.end_role(), end.end_text()), ("items".to_string(), "* = none {ordered}".to_string()));
    end.derived = true;
    assert_eq!(end.end_role(), "/items");
    let bare = PropertyValues { braces: s(&["readOnly", "subsets lines"]), ..Default::default() };
    assert_eq!((bare.end_role(), bare.end_text()), (String::new(), "{readOnly, subsets lines}".to_string()), "no leading space");
    assert_eq!(PropertyValues::default().end_text(), "");
}

#[test]
fn an_operation_writes_its_parameters_its_return_and_leaf() {
    let op = OperationValues {
        visibility: Some(VisibilityKind::Protected),
        name: "total".into(),
        inputs: vec![("currency".into(), Some("Currency".into())), ("rounded".into(), None)],
        returns: Some("Money".into()),
        leaf: true,
    };
    assert_eq!(op.text(), "# total(currency : Currency, rounded) : Money {leaf}");
    let bare = OperationValues { name: "run".into(), ..Default::default() };
    assert_eq!(bare.text(), "run()");
    let params = |n: usize| (0..n).map(|i| (format!("p{i}"), None)).collect::<Vec<_>>();
    let all = OperationValues { name: "f".into(), inputs: params(MAX_PARAMETERS), ..Default::default() };
    assert_eq!(all.text(), "f(p0, p1, p2, p3, p4, p5, p6, p7)");
    let more = OperationValues { name: "f".into(), inputs: params(MAX_PARAMETERS + 1), ..Default::default() };
    assert_eq!(more.text(), "f(p0, p1, p2, p3, p4, p5, p6, p7, …)");
}

#[test]
fn stereotypes_start_a_row_and_take_a_kinds_word() {
    assert_eq!(figures::stereotype_keywords(&[]), None);
    assert_eq!(figures::row_text(&[], "+ id : Integer"), "+ id : Integer");
    assert_eq!(figures::row_text(&s(&["Id"]), "+ id : Integer"), "«Id» + id : Integer");
    assert_eq!(figures::row_text(&s(&["Id", "Key"]), "run()"), "«Id, Key» run()");
    assert_eq!(figures::kind_keywords("Class", &[]), None, "the kind's word stays");
    assert_eq!(figures::kind_keywords("Class", &s(&["Entity", "Auditable"])).as_deref(), Some("«Entity, Auditable»"));
    assert_eq!(figures::kind_keywords("«interface»", &s(&["Entity"])).as_deref(), Some("«interface, Entity»"));
}
