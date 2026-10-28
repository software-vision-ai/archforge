//! Parameters: their documentation, one line per key (`key: type = default, range or choices;
//! meaning`), and the validation every call passes before its handler runs.

use serde_json::{Map, Value};

use crate::EngineError;
use crate::error::echo;

/// The most a call's parameters may weigh, and how deep they may nest.
pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 32;
/// The longest string value, in bytes, and the longest coalescing key.
pub const MAX_STRING: usize = 64 * 1024;
pub const MAX_COALESCE: usize = 256;
/// The largest integer a float holds exactly.
const SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;
/// The key every undoable command accepts, to merge repeated calls into one undo step.
pub const COALESCE: &str = "coalesce";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Number,
    Integer,
    Bool,
    String,
    Array,
    Object,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Number => "number",
            Kind::Integer => "integer",
            Kind::Bool => "bool",
            Kind::String => "string",
            Kind::Array => "array",
            Kind::Object => "object",
        }
    }
}

/// One documented parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDoc {
    pub key: String,
    pub kind: Kind,
    /// None makes the key required.
    pub default: Option<Value>,
    /// An inclusive range, for numbers and integers.
    pub range: Option<(f64, f64)>,
    /// The accepted values, for strings.
    pub choices: Vec<String>,
    pub meaning: String,
}

impl ParamDoc {
    /// The type's name as the documentation writes it.
    pub fn kind_name(&self) -> &'static str {
        self.kind.name()
    }
}

/// Reads a command's parameter documentation; an empty text documents no parameters.
pub fn parse_params(doc: &str) -> Result<Vec<ParamDoc>, String> {
    let mut out: Vec<ParamDoc> = Vec::new();
    for (n, line) in doc.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let at = |why: &str| format!("line {}: {why}: {line:?}", n + 1);
        let (head, meaning) = line.split_once(';').ok_or_else(|| at("no meaning after `;`"))?;
        let meaning = meaning.trim();
        if meaning.is_empty() {
            return Err(at("an empty meaning"));
        }
        let (key, rest) = head.split_once(':').ok_or_else(|| at("no `key:`"))?;
        let key = key.trim();
        if key.is_empty()
            || !key.starts_with(|c: char| c.is_ascii_lowercase())
            || !key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(at("a key must be lower case letters, digits and `_`"));
        }
        if key == COALESCE || out.iter().any(|p| p.key == key) {
            return Err(at("a key named twice, or the reserved `coalesce`"));
        }
        // `type`, then an optional `= default`, then an optional `, range or choices`.
        let (kind_text, after) = match rest.find(['=', ',']) {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        let kind = match kind_text.trim() {
            "number" => Kind::Number,
            "integer" => Kind::Integer,
            "bool" => Kind::Bool,
            "string" => Kind::String,
            "array" => Kind::Array,
            "object" => Kind::Object,
            other => return Err(at(&format!("{other:?} is not number, integer, bool, string, array or object"))),
        };
        let (default_text, limit_text) = match after.strip_prefix('=') {
            Some(d) => match split_outside_quotes(d) {
                Some((d, l)) => (Some(d), Some(l)),
                None => (Some(d), None),
            },
            None => (None, after.strip_prefix(',')),
        };
        let mut p = ParamDoc { key: key.to_string(), kind, default: None, range: None, choices: Vec::new(), meaning: meaning.to_string() };
        if let Some(limit) = limit_text.map(str::trim).filter(|l| !l.is_empty()) {
            match kind {
                Kind::Number | Kind::Integer => {
                    let (lo, hi) = limit.split_once("..").ok_or_else(|| at("a range is `low..high`"))?;
                    let (lo, hi) = (lo.trim().parse::<f64>(), hi.trim().parse::<f64>());
                    match (lo, hi) {
                        (Ok(lo), Ok(hi)) if lo.is_finite() && hi.is_finite() && lo <= hi => p.range = Some((lo, hi)),
                        _ => return Err(at("a range needs two finite numbers, low first")),
                    }
                }
                Kind::String => p.choices = limit.split('|').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect(),
                _ => return Err(at("only numbers take a range and only strings choices")),
            }
        }
        if let Some(d) = default_text {
            let value: Value = serde_json::from_str(d.trim()).map_err(|_| at("the default is not a JSON value"))?;
            check(&p, &value).map_err(|e| at(&format!("the default does not fit: {e}")))?;
            p.default = Some(value);
        }
        out.push(p);
    }
    Ok(out)
}

/// Splits at the first `,` outside a JSON string.
fn split_outside_quotes(s: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            '\\' if quoted && !escaped => {
                escaped = true;
                continue;
            }
            '"' if !escaped => quoted = !quoted,
            ',' if !quoted => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
        escaped = false;
    }
    None
}

/// The normalised parameters a handler receives: every documented key with a value of its type,
/// defaults filled in; a required key is always there.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    map: Map<String, Value>,
    coalesce: Option<String>,
}

impl Params {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        self.map.get(key).and_then(Value::as_str)
    }
    pub fn integer(&self, key: &str) -> Option<i64> {
        self.map.get(key).and_then(Value::as_i64)
    }
    pub fn number(&self, key: &str) -> Option<f64> {
        self.map.get(key).and_then(Value::as_f64)
    }
    pub fn bool(&self, key: &str) -> Option<bool> {
        self.map.get(key).and_then(Value::as_bool)
    }
    /// The coalescing key, when the call gave one.
    pub fn coalesce(&self) -> Option<&str> {
        self.coalesce.as_deref()
    }
    /// The parameters as one JSON object, as the journal records them.
    pub fn to_value(&self) -> Value {
        let mut m = self.map.clone();
        if let Some(c) = &self.coalesce {
            m.insert(COALESCE.to_string(), Value::String(c.clone()));
        }
        Value::Object(m)
    }
}

/// Validates a call's parameters against their documentation, before any handler runs.
pub fn validate(docs: &[ParamDoc], params: &Value, undoable: bool) -> Result<Params, EngineError> {
    let given = match params {
        Value::Object(m) => m,
        Value::Null => &Map::new(),
        _ => return Err(EngineError::NotAnObject),
    };
    measure(params)?;
    let accepted = || {
        let mut keys: Vec<String> = docs.iter().map(|d| d.key.clone()).collect();
        if undoable {
            keys.push(COALESCE.to_string());
        }
        keys
    };
    let mut coalesce = None;
    for (k, v) in given {
        if k == COALESCE && undoable {
            match v {
                Value::Null => {}
                Value::String(s) if s.len() <= MAX_COALESCE => coalesce = Some(s.clone()),
                Value::String(_) => return Err(EngineError::TooLong { key: k.clone(), max: MAX_COALESCE }),
                _ => return Err(EngineError::WrongType { key: k.clone(), expected: "string" }),
            }
        } else if !docs.iter().any(|d| &d.key == k) {
            return Err(EngineError::UnknownKey { key: echo(k), accepted: accepted() });
        }
    }
    let mut map = Map::new();
    for d in docs {
        let value = match given.get(&d.key) {
            None | Some(Value::Null) => match &d.default {
                Some(v) => v.clone(),
                None => return Err(EngineError::MissingKey { key: d.key.clone() }),
            },
            Some(v) => {
                check(d, v)?;
                // An integer written as a float (3.0, 1e2) is stored as the integer it is.
                match (d.kind, v.as_f64()) {
                    (Kind::Integer, Some(n)) if v.as_i64().is_none() => Value::from(n as i64),
                    _ => v.clone(),
                }
            }
        };
        map.insert(d.key.clone(), value);
    }
    Ok(Params { map, coalesce })
}

/// Whether one value fits its documentation.
fn check(d: &ParamDoc, v: &Value) -> Result<(), EngineError> {
    let wrong = || EngineError::WrongType { key: d.key.clone(), expected: d.kind.name() };
    match d.kind {
        Kind::Number | Kind::Integer => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(wrong)?;
            // An integer: a JSON integer, or a float with no fraction within 2^53, where every
            // integer is exact.
            if d.kind == Kind::Integer && v.as_i64().is_none() && !(n.fract() == 0.0 && n.abs() <= SAFE_INTEGER) {
                return Err(wrong());
            }
            if let Some((lo, hi)) = d.range
                && !(lo..=hi).contains(&n)
            {
                return Err(EngineError::OutOfRange { key: d.key.clone(), range: format!("{lo}..{hi}") });
            }
        }
        Kind::Bool => {
            v.as_bool().ok_or_else(wrong)?;
        }
        Kind::String => {
            let s = v.as_str().ok_or_else(wrong)?;
            if s.len() > MAX_STRING {
                return Err(EngineError::TooLong { key: d.key.clone(), max: MAX_STRING });
            }
            if !d.choices.is_empty() && !d.choices.iter().any(|c| c == s) {
                return Err(EngineError::NotAChoice { key: d.key.clone(), choices: d.choices.clone() });
            }
        }
        Kind::Array => {
            v.as_array().ok_or_else(wrong)?;
        }
        Kind::Object => {
            v.as_object().ok_or_else(wrong)?;
        }
    }
    Ok(())
}

/// The serialised size of `v`, estimated by an iterative walk that stops as soon as it passes
/// [`MAX_BYTES`] or [`MAX_DEPTH`], so measuring a hostile value is itself bounded.
pub fn measure(v: &Value) -> Result<usize, EngineError> {
    let mut total = 0usize;
    let mut stack = vec![(v, 1usize)];
    while let Some((v, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(EngineError::TooDeep { max: MAX_DEPTH });
        }
        total = total.saturating_add(match v {
            Value::Null | Value::Bool(_) => 5,
            Value::Number(_) => 24,
            Value::String(s) => s.len().saturating_add(2),
            Value::Array(a) => {
                // Each element weighs at least two bytes: a value too wide is refused before the
                // walk grows its stack for it.
                if total.saturating_add(a.len().saturating_mul(2)) > MAX_BYTES {
                    return Err(EngineError::TooLarge { bytes: total.saturating_add(a.len().saturating_mul(2)), max: MAX_BYTES });
                }
                stack.extend(a.iter().map(|x| (x, depth + 1)));
                a.len().saturating_add(2)
            }
            Value::Object(m) => {
                if total.saturating_add(m.len().saturating_mul(4)) > MAX_BYTES {
                    return Err(EngineError::TooLarge { bytes: total.saturating_add(m.len().saturating_mul(4)), max: MAX_BYTES });
                }
                stack.extend(m.values().map(|x| (x, depth + 1)));
                m.keys().map(|k| k.len().saturating_add(4)).fold(2usize, usize::saturating_add)
            }
        });
        if total > MAX_BYTES {
            return Err(EngineError::TooLarge { bytes: total, max: MAX_BYTES });
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const DOC: &str = "name: string; the item's name\nindex: integer = 0, 0..1000; where it goes\nkind: string = \"one\", one|many; how many\nscale: number = 1.5, 0.5..4; how big";

    #[test]
    fn the_documentation_convention_reads_types_defaults_ranges_and_choices() {
        let p = parse_params(DOC).unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!((p[0].kind, p[0].default.clone()), (Kind::String, None));
        assert_eq!((p[1].kind, p[1].default.clone(), p[1].range), (Kind::Integer, Some(json!(0)), Some((0.0, 1000.0))));
        assert_eq!(p[2].choices, ["one", "many"]);
        for bad in [
            "name: string",
            "name: text; x",
            "Name: string; x",
            "a: string; x\na: bool; y",
            "coalesce: string; x",
            "n: integer = 5000, 0..10; x",
            "n: bool, 0..1; x",
            "n: number = oops; x",
        ] {
            assert!(parse_params(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_params("").unwrap(), vec![]);
    }

    #[test]
    fn every_bad_value_is_refused_before_a_handler_and_defaults_fill_in() {
        let docs = parse_params(DOC).unwrap();
        let ok = validate(&docs, &json!({"name": "A", "coalesce": "drag-1", "kind": null}), true).unwrap();
        assert_eq!((ok.str("name"), ok.integer("index"), ok.str("kind"), ok.coalesce()), (Some("A"), Some(0), Some("one"), Some("drag-1")));
        let refused = |v: Value, undoable: bool| validate(&docs, &v, undoable).unwrap_err();
        assert!(matches!(refused(json!([1]), true), EngineError::NotAnObject));
        assert!(
            matches!(refused(json!({"name": "A", "colour": 1}), true), EngineError::UnknownKey { accepted, .. } if accepted.contains(&"coalesce".to_string()))
        );
        assert!(matches!(refused(json!({"name": "A", "coalesce": "x"}), false), EngineError::UnknownKey { .. }), "only undoable commands coalesce");
        assert!(matches!(refused(json!({}), true), EngineError::MissingKey { .. }));
        assert!(matches!(refused(json!({"name": 3}), true), EngineError::WrongType { .. }));
        assert!(matches!(refused(json!({"name": "A", "index": 2.5}), true), EngineError::WrongType { .. }), "an integer is not 2.5");
        assert!(
            matches!(refused(json!({"name": "A", "index": 1e300}), true), EngineError::WrongType { .. }),
            "nor beyond what a float holds exactly"
        );
        assert_eq!(validate(&docs, &json!({"name": "A", "index": 3.0}), true).unwrap().integer("index"), Some(3), "3.0 is the integer 3");
        match refused(json!({"name": "A", "k".repeat(500): 1}), true) {
            EngineError::UnknownKey { key, .. } => assert!(key.len() < 250 && key.ends_with("(truncated)"), "{key}"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(refused(json!({"name": "A", "index": 1001}), true), EngineError::OutOfRange { .. }));
        assert!(matches!(refused(json!({"name": "A", "kind": "some"}), true), EngineError::NotAChoice { .. }));
        assert!(matches!(refused(json!({"name": "x".repeat(MAX_STRING + 1)}), true), EngineError::TooLong { .. }));
    }

    #[test]
    fn measuring_a_hostile_value_stops_at_its_budget() {
        let mut deep = json!(1);
        for _ in 0..40 {
            deep = json!([deep]);
        }
        assert!(matches!(measure(&deep), Err(EngineError::TooDeep { .. })));
        let wide = Value::Array(vec![json!("x".repeat(1000)); 2000]);
        assert!(matches!(measure(&wide), Err(EngineError::TooLarge { .. })));
        assert!(measure(&json!({"a": [1, 2, "three"]})).unwrap() < 100);
    }
}
