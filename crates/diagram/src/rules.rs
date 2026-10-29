//! The diagram rules the MOF core cannot know, checked by the commands before they change anything
//! and available over a whole store.

use archforge_mof::{Obj, Store, Value};

use crate::{DIAGRAM, EDGE, LABEL, NODE, POINT, STYLE, diagram, diagram_of, edge, label, node, node_tree, point, style};

/// The farthest coordinate from the origin, in the logical unit, either way.
pub const MAX_COORD: f64 = 1_000_000.0;
/// A node's smallest and largest width and height.
pub const MIN_SIZE: f64 = 20.0;
pub const MAX_SIZE: f64 = 10_000.0;
/// The widest stroke a style may ask for; 0 is the sheet's.
pub const MAX_LINE: f64 = 100.0;
/// The most characters in a diagram's name.
pub const MAX_NAME: usize = 128;
/// The most characters in a kind: a diagram kind, a node kind, a label's slot, a sheet style's name.
pub const MAX_KIND: usize = 64;
/// The most characters in a note's text.
pub const MAX_TEXT: usize = 4096;
/// How deep nodes nest (a diagram's node is at depth 0), how many waypoints an edge takes and how
/// many labels.
pub const MAX_DEPTH: usize = 16;
pub const MAX_WAYPOINTS: usize = 256;
pub const MAX_LABELS: usize = 16;

/// Why a kind (a diagram kind, a node kind, a label's slot, a sheet style's name) is not acceptable,
/// as the end of the sentence "the kind ...": plain identifiers only, `a-z`, digits, `_`, `-` and
/// `.` between them.
pub fn kind_problem(kind: &str) -> Option<String> {
    if kind.is_empty() {
        return Some("is empty".into());
    }
    if kind.chars().count() > MAX_KIND {
        return Some(format!("is longer than {MAX_KIND} characters"));
    }
    if !kind.starts_with(|c: char| c.is_ascii_lowercase())
        || !kind.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
    {
        return Some("is not a plain identifier of lower case letters, digits, `_`, `-` and `.`".into());
    }
    None
}

/// Why a name or a note's text is not acceptable, as the end of the sentence "the name ...".
pub fn text_problem(text: &str, max: usize) -> Option<String> {
    if text.chars().count() > max {
        return Some(format!("is longer than {max} characters"));
    }
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Some("holds a control character".into());
    }
    if text.trim() != text {
        return Some("starts or ends with a space".into());
    }
    None
}

/// Why a colour is not acceptable: `#rrggbb` only.
pub fn colour_problem(colour: &str) -> Option<String> {
    let hex = colour.strip_prefix('#').filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
    if hex.is_none() { Some(format!("{colour:?} is not a colour of the form #rrggbb")) } else { None }
}

/// Why a coordinate is not acceptable.
pub fn coord_problem(v: f64) -> Option<String> {
    if !v.is_finite() {
        return Some("is not a number".into());
    }
    if v.abs() > MAX_COORD {
        return Some(format!("is farther than {MAX_COORD} from the origin"));
    }
    None
}

/// Why a width or height is not acceptable.
pub fn size_problem(v: f64) -> Option<String> {
    if !v.is_finite() {
        return Some("is not a number".into());
    }
    if !(MIN_SIZE..=MAX_SIZE).contains(&v) {
        return Some(format!("is not between {MIN_SIZE} and {MAX_SIZE}"));
    }
    None
}

/// A real property's value, or 0.
pub fn real(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> f64 {
    match store.get(obj, prop) {
        Ok(Value::Real(r)) => r,
        _ => 0.0,
    }
}

/// A string property's value, or empty.
pub fn text(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> String {
    store.get(obj, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The nesting depth of a node: 0 for a diagram's own node, or None when no diagram is above it
/// within the bound.
pub fn depth_of(store: &Store, node: Obj) -> Option<usize> {
    let mut cur = node;
    for depth in 0..=MAX_DEPTH {
        let (container, _) = store.container(cur).ok().flatten()?;
        if store.class_of(container).ok() == Some(DIAGRAM) {
            return Some(depth);
        }
        cur = container;
    }
    None
}

/// What the rules find wrong in a store, each as a sentence.
pub fn problems(store: &Store) -> Vec<String> {
    let mut out = Vec::new();
    for o in store.alive() {
        let Ok(class) = store.class_of(o) else { continue };
        if class == DIAGRAM {
            if let Some(p) = kind_problem(&text(store, o, diagram::KIND)) {
                out.push(format!("the kind of {o} {p}"));
            }
            if let Some(p) = text_problem(&text(store, o, diagram::NAME), MAX_NAME) {
                out.push(format!("the name of {o} {p}"));
            }
            let mut count = 0usize;
            for n in store.list(o, diagram::NODE).unwrap_or_default().iter().filter_map(Value::as_ref) {
                count = count.saturating_add(node_tree(store, n).len());
            }
            if count > archforge_mof::store::MAX_LIST {
                out.push(format!("{o} holds more than {} nodes", archforge_mof::store::MAX_LIST));
            }
        }
        if class == NODE {
            for (prop, what) in [(node::X, "x"), (node::Y, "y")] {
                if let Some(p) = coord_problem(real(store, o, prop)) {
                    out.push(format!("the {what} of {o} {p}"));
                }
            }
            for (prop, what) in [(node::WIDTH, "width"), (node::HEIGHT, "height")] {
                if let Some(p) = size_problem(real(store, o, prop)) {
                    out.push(format!("the {what} of {o} {p}"));
                }
            }
            let kind = text(store, o, node::KIND);
            if !kind.is_empty()
                && let Some(p) = kind_problem(&kind)
            {
                out.push(format!("the kind of {o} {p}"));
            }
            let has_element = store.get(o, node::ELEMENT).ok().and_then(|v| v.as_ref()).is_some();
            let has_text = store.get(o, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)).is_some();
            if has_element && has_text {
                out.push(format!("{o} shows an element and carries text of its own"));
            }
            if let Some(p) = text_problem(&text(store, o, node::TEXT), MAX_TEXT) {
                out.push(format!("the text of {o} {p}"));
            }
            if depth_of(store, o).is_none() {
                out.push(format!("{o} is nested deeper than {MAX_DEPTH} or belongs to no diagram"));
            }
        }
        if class == EDGE {
            let here = diagram_of(store, o);
            for (prop, what) in [(edge::SOURCE, "source"), (edge::TARGET, "target")] {
                match store.get(o, prop).ok().and_then(|v| v.as_ref()) {
                    Some(n) if here.is_some() && diagram_of(store, n) == here => {}
                    Some(n) => out.push(format!("the {what} of {o}, {n}, is in another diagram")),
                    None => out.push(format!("{o} lacks a {what}")),
                }
            }
            if store.list(o, edge::WAYPOINT).unwrap_or_default().len() > MAX_WAYPOINTS {
                out.push(format!("{o} has more than {MAX_WAYPOINTS} waypoints"));
            }
            if store.list(o, edge::LABEL).unwrap_or_default().len() > MAX_LABELS {
                out.push(format!("{o} has more than {MAX_LABELS} labels"));
            }
        }
        if class == POINT {
            for (prop, what) in [(point::X, "x"), (point::Y, "y")] {
                if let Some(p) = coord_problem(real(store, o, prop)) {
                    out.push(format!("the {what} of {o} {p}"));
                }
            }
        }
        if class == LABEL {
            if let Some(p) = kind_problem(&text(store, o, label::KIND)) {
                out.push(format!("the kind of {o} {p}"));
            }
            for (prop, what) in [(label::DX, "dx"), (label::DY, "dy")] {
                if let Some(p) = coord_problem(real(store, o, prop)) {
                    out.push(format!("the {what} of {o} {p}"));
                }
            }
        }
        if class == STYLE {
            let name = text(store, o, style::NAME);
            if !name.is_empty()
                && let Some(p) = kind_problem(&name)
            {
                out.push(format!("the style name of {o} {p}"));
            }
            for (prop, what) in [(style::FILL, "fill"), (style::STROKE, "stroke"), (style::TEXT, "text")] {
                let c = text(store, o, prop);
                if !c.is_empty()
                    && let Some(p) = colour_problem(&c)
                {
                    out.push(format!("the {what} of {o}: {p}"));
                }
            }
            let line = real(store, o, style::LINE);
            if !line.is_finite() || !(0.0..=MAX_LINE).contains(&line) {
                out.push(format!("the line width of {o} is not between 0 and {MAX_LINE}"));
            }
        }
    }
    out
}
