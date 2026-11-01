//! The metamodel generator: a metamodel as XMI in, a crate's generated code out.
//!
//! `cargo xtask generate [name] [--docs]` reads the XMI of each target (OMG's files from the folder
//! `ARCHFORGE_SPECS_DIR` names, our own from the repository), builds the metamodel through the MOF
//! core's `Builder` so every check of the core runs on it, and writes into `crates/<mm>/src/
//! generated/` the metamodel description with its id constants (`meta.rs`), one struct per concrete
//! metaclass with its `Reflect` and typed accessors (`classes.rs`), the slabs (`slabs.rs`), the
//! storage schema as data (`schema.rs`) and a report of what was read and skipped (`report.txt`).
//! The gate's `generated` step regenerates into a temporary folder and compares, and says when it
//! skipped a target whose XMI is not there.
//!
//! The reader takes the XMI 2.5 dialect OMG writes and we write: classes with generalizations and
//! owned attributes, multiplicities as literals, defaults, associations as opposites, enumerations,
//! comments. Operations, constraints and everything else are counted, not read.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use archforge_mof::{Builder, ClassId, DataId, DataKind, MetaError, Metamodel, PropId, PropertySpec, TypeRef, Universe, Value};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::util;

/// The folder OMG's XMI files are read from.
pub const SPECS_VAR: &str = "ARCHFORGE_SPECS_DIR";

/// Where a target's XMI lives.
#[derive(Clone, Copy)]
pub enum Source {
    /// Under the folder `ARCHFORGE_SPECS_DIR` names (OMG's files, outside the repository).
    Specs(&'static str),
    /// Under the repository root (our own metamodels).
    Repo(&'static str),
}

/// One metamodel crate the generator keeps.
pub struct Target {
    pub name: &'static str,
    pub index: u16,
    pub uri: &'static str,
    pub version: &'static str,
    pub source: Source,
    pub crate_dir: &'static str,
}

/// The metamodels and their fixed indices (decision gm2.18): core 1, uml 2, er 3, c4 4 (reserved, not
/// yet a target), diagram 5;
/// the toy of the MOF core's tests is 15.
pub const TARGETS: &[Target] = &[
    Target { name: "core", index: 1, uri: "archforge:core", version: "1", source: Source::Repo("crates/core/core.xmi"), crate_dir: "crates/core" },
    Target { name: "er", index: 3, uri: "archforge:er", version: "1", source: Source::Repo("crates/er/er.xmi"), crate_dir: "crates/er" },
    Target {
        name: "diagram",
        index: 5,
        uri: "archforge:diagram",
        version: "1",
        source: Source::Repo("crates/diagram/diagram.xmi"),
        crate_dir: "crates/diagram",
    },
    Target {
        name: "uml",
        index: 2,
        uri: "http://www.omg.org/spec/UML/20161101",
        version: "2.5.1",
        source: Source::Specs("UML/UML.xmi"),
        crate_dir: "crates/uml",
    },
];

const GENERATED: &str = "src/generated";

/// `cargo xtask generate [name] [--docs]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let docs = args.iter().any(|a| a == "--docs");
    let names: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let root = util::root();
    let mut done = 0;
    for t in TARGETS {
        if !names.is_empty() && !names.iter().any(|n| *n == t.name) {
            continue;
        }
        let Some(path) = source_path(&root, t.source) else {
            return Err(format!("{}: {SPECS_VAR} names no folder, so the XMI of {} cannot be read", t.name, t.name));
        };
        let out = root.join(t.crate_dir).join(GENERATED);
        let files = generate(t, &path, docs)?;
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        for (name, text) in &files {
            std::fs::write(out.join(name), text).map_err(|e| format!("{}: {e}", out.join(name).display()))?;
        }
        format_rust(&out, &files)?;
        println!("generate: {} written to {}", t.name, out.display());
        done += 1;
    }
    if done == 0 && !names.is_empty() {
        return Err(format!("no target named {:?}; the targets are {}", names, TARGETS.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")));
    }
    Ok(())
}

/// The gate's step: the committed generated code is what the XMI gives.
pub fn check() -> Result<(), String> {
    let root = util::root();
    let mut skipped = Vec::new();
    let mut checked = 0;
    for t in TARGETS {
        let Some(path) = source_path(&root, t.source) else {
            skipped.push(t.name);
            continue;
        };
        let files = generate(t, &path, false)?;
        let tmp = fresh_temp_dir(t.name)?;
        let compared = (|| -> Result<Vec<&'static str>, String> {
            for (name, text) in &files {
                std::fs::write(tmp.join(name), text).map_err(|e| format!("{}: {e}", tmp.join(name).display()))?;
            }
            format_rust(&tmp, &files)?;
            let committed = root.join(t.crate_dir).join(GENERATED);
            let mut differ = Vec::new();
            for (name, _) in &files {
                let fresh = std::fs::read(tmp.join(name)).map_err(|e| format!("{}: {e}", tmp.join(name).display()))?;
                let old = std::fs::read(committed.join(name)).unwrap_or_default();
                if fresh != old {
                    differ.push(*name);
                }
            }
            Ok(differ)
        })();
        let _ = std::fs::remove_dir_all(&tmp);
        let differ = compared?;
        if !differ.is_empty() {
            return Err(format!(
                "{}: the committed generated code differs from the XMI in {}; run `cargo xtask generate {}`",
                t.name,
                differ.join(", "),
                t.name
            ));
        }
        checked += 1;
    }
    if !skipped.is_empty() {
        println!("generated: skipped {} ({SPECS_VAR} names no folder with their XMI)", skipped.join(", "));
    }
    println!("generated: {checked} metamodel(s) match their XMI");
    Ok(())
}

/// A new, empty folder of our own under the temp dir: created, never adopted.
fn fresh_temp_dir(name: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    for attempt in 0..16u32 {
        let dir = std::env::temp_dir().join(format!("archforge-generated-{name}-{}-{nanos}-{attempt}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        }
    }
    Err("no free temporary folder for the generated code".into())
}

fn source_path(root: &Path, source: Source) -> Option<PathBuf> {
    let path = match source {
        Source::Specs(rel) => PathBuf::from(std::env::var_os(SPECS_VAR)?).join(rel),
        Source::Repo(rel) => root.join(rel),
    };
    path.is_file().then_some(path)
}

/// Formats the generated files with the repository's rustfmt settings, wherever they are written.
fn format_rust(dir: &Path, files: &[(&'static str, String)]) -> Result<(), String> {
    let mut cmd = Command::new("rustfmt");
    cmd.arg("--edition").arg("2024");
    let config = util::root().join("rustfmt.toml");
    if config.is_file() {
        cmd.arg("--config-path").arg(config);
    }
    for (name, _) in files.iter().filter(|(n, _)| n.ends_with(".rs")) {
        cmd.arg(dir.join(name));
    }
    let status = cmd.status().map_err(|e| format!("rustfmt: {e}"))?;
    if status.success() { Ok(()) } else { Err("rustfmt failed on the generated code".into()) }
}

// The XMI as read.

#[derive(Debug, Clone, PartialEq, Default)]
enum XType {
    Ref(String),
    Primitive(String),
    Any,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq)]
enum XDefault {
    Bool(bool),
    Int(i64),
    Unlimited(Option<u64>),
    Literal(String),
}

#[derive(Debug, Default)]
struct XProp {
    id: String,
    name: String,
    ty: XType,
    lower: u32,
    upper: Option<u32>,
    ordered: bool,
    unique: bool,
    composite: bool,
    derived: bool,
    derived_union: bool,
    read_only: bool,
    subsets: Vec<String>,
    redefines: Vec<String>,
    default: Option<XDefault>,
    doc: String,
}

#[derive(Debug, Default)]
struct XClass {
    id: String,
    name: String,
    is_abstract: bool,
    supers: Vec<String>,
    props: Vec<XProp>,
    doc: String,
}

#[derive(Debug, Default)]
struct XEnum {
    id: String,
    name: String,
    literals: Vec<(String, String)>,
    doc: String,
}

#[derive(Debug, Default)]
struct XAssoc {
    member_ends: Vec<String>,
    owned_ends: Vec<String>,
}

#[derive(Debug, Default)]
struct Xmi {
    name: String,
    uri: String,
    classes: Vec<XClass>,
    enums: Vec<XEnum>,
    assocs: Vec<XAssoc>,
    primitives: BTreeSet<String>,
    skipped: BTreeMap<String, usize>,
}

/// Where the reader is inside the document.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ctx {
    Root,
    Package,
    Class,
    Property,
    Enum,
    Assoc,
    AssocEnd,
    Comment,
    Body,
    Skipped,
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name.as_bytes())
        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::default()).ok())
        .map(|v| v.into_owned())
}

/// The class and property the reader is in, for messages.
fn where_am_i(x: &Xmi) -> String {
    match x.classes.last() {
        Some(c) => match c.props.last() {
            Some(p) => format!("{}::{}", c.name, p.name),
            None => c.name.clone(),
        },
        None => "the package".to_string(),
    }
}

/// The most bytes an XMI file may have (UML.xmi is about 2.5 MB).
const MAX_XMI_BYTES: u64 = 64 << 20;
/// The deepest element nesting read.
const MAX_DEPTH: usize = 256;

fn local(e: &BytesStart<'_>) -> String {
    let n = e.name();
    let full = String::from_utf8_lossy(n.as_ref()).into_owned();
    full.rsplit(':').next().unwrap_or_default().to_string()
}

fn read_xmi(bytes: &[u8]) -> Result<Xmi, String> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    // An empty element then comes as a start and an end, so the context stack stays right.
    reader.config_mut().expand_empty_elements = true;
    let mut x = Xmi::default();
    let mut stack: Vec<Ctx> = vec![Ctx::Root];
    // The comment's owner: a class, a property of the last class, or an enumeration.
    let mut comment_owner: Option<Ctx> = None;
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| format!("XMI at byte {}: {e}", reader.buffer_position()))?;
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if stack.len() > MAX_DEPTH {
                    return Err(format!("XMI at byte {}: elements nested deeper than {MAX_DEPTH}", reader.buffer_position()));
                }
                for a in e.attributes() {
                    a.map_err(|err| format!("XMI at byte {}: a malformed attribute: {err}", reader.buffer_position()))?;
                }
                let here = *stack.last().unwrap_or(&Ctx::Root);
                let name = local(&e);
                let kind = attr(&e, "xmi:type").unwrap_or_default();
                let next = match (here, name.as_str(), kind.as_str()) {
                    (Ctx::Root, _, "uml:Package") | (Ctx::Root, "Package", _) => {
                        x.name = attr(&e, "name").unwrap_or_default();
                        x.uri = attr(&e, "URI").unwrap_or_default();
                        Ctx::Package
                    }
                    (Ctx::Root, _, _) => Ctx::Root,
                    (Ctx::Package, "packagedElement", "uml:Package") => Ctx::Package,
                    (Ctx::Package, "packagedElement", "uml:Class") => {
                        x.classes.push(XClass {
                            id: attr(&e, "xmi:id").unwrap_or_default(),
                            name: attr(&e, "name").unwrap_or_default(),
                            is_abstract: attr(&e, "isAbstract").as_deref() == Some("true"),
                            ..Default::default()
                        });
                        Ctx::Class
                    }
                    (Ctx::Package, "packagedElement", "uml:Enumeration") => {
                        x.enums.push(XEnum {
                            id: attr(&e, "xmi:id").unwrap_or_default(),
                            name: attr(&e, "name").unwrap_or_default(),
                            ..Default::default()
                        });
                        Ctx::Enum
                    }
                    (Ctx::Package, "packagedElement", "uml:PrimitiveType") => {
                        x.primitives.insert(attr(&e, "name").unwrap_or_default());
                        Ctx::Skipped
                    }
                    (Ctx::Package, "packagedElement", "uml:Association") => {
                        let ends = attr(&e, "memberEnd").unwrap_or_default();
                        x.assocs.push(XAssoc { member_ends: ends.split_whitespace().map(str::to_string).collect(), owned_ends: Vec::new() });
                        Ctx::Assoc
                    }
                    (Ctx::Assoc, "memberEnd", _) => {
                        if let (Some(a), Some(id)) = (x.assocs.last_mut(), attr(&e, "xmi:idref")) {
                            a.member_ends.push(id);
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Property, "subsettedProperty", _) => {
                        if let (Some(p), Some(id)) = (x.classes.last_mut().and_then(|c| c.props.last_mut()), attr(&e, "xmi:idref")) {
                            p.subsets.push(id);
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Property, "redefinedProperty", _) => {
                        if let (Some(p), Some(id)) = (x.classes.last_mut().and_then(|c| c.props.last_mut()), attr(&e, "xmi:idref")) {
                            p.redefines.push(id);
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Assoc, "ownedEnd", _) => {
                        if let Some(a) = x.assocs.last_mut() {
                            a.owned_ends.push(attr(&e, "xmi:id").unwrap_or_default());
                        }
                        Ctx::AssocEnd
                    }
                    (Ctx::Class, "generalization", _) => {
                        if let (Some(c), Some(g)) = (x.classes.last_mut(), attr(&e, "general")) {
                            c.supers.push(g);
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Class, "ownedAttribute", _) => {
                        let ty = match (attr(&e, "type"), attr(&e, "href")) {
                            (Some(t), _) => XType::Ref(t),
                            _ => XType::None,
                        };
                        let p = XProp {
                            id: attr(&e, "xmi:id").unwrap_or_default(),
                            name: attr(&e, "name").unwrap_or_default(),
                            ty,
                            lower: 1,
                            upper: Some(1),
                            ordered: attr(&e, "isOrdered").as_deref() == Some("true"),
                            unique: attr(&e, "isUnique").as_deref() != Some("false"),
                            composite: attr(&e, "aggregation").as_deref() == Some("composite"),
                            derived: attr(&e, "isDerived").as_deref() == Some("true"),
                            derived_union: attr(&e, "isDerivedUnion").as_deref() == Some("true"),
                            read_only: attr(&e, "isReadOnly").as_deref() == Some("true"),
                            subsets: attr(&e, "subsettedProperty").unwrap_or_default().split_whitespace().map(str::to_string).collect(),
                            redefines: attr(&e, "redefinedProperty").unwrap_or_default().split_whitespace().map(str::to_string).collect(),
                            default: None,
                            doc: String::new(),
                        };
                        if let Some(c) = x.classes.last_mut() {
                            c.props.push(p);
                        }
                        Ctx::Property
                    }
                    (Ctx::Property, "type", _) => {
                        let href = attr(&e, "href").unwrap_or_default();
                        let idref = attr(&e, "xmi:idref");
                        if let Some(p) = x.classes.last_mut().and_then(|c| c.props.last_mut()) {
                            p.ty = if let Some(id) = idref {
                                XType::Ref(id)
                            } else if href == "archforge:mof#Any" {
                                XType::Any
                            } else if let Some((_, frag)) = href.rsplit_once('#') {
                                XType::Primitive(frag.to_string())
                            } else {
                                XType::None
                            };
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Property, "lowerValue", _) => {
                        let lower = match attr(&e, "value") {
                            Some(v) => v.parse::<u32>().map_err(|_| format!("{}: a lower bound {v:?} is not a whole number", where_am_i(&x)))?,
                            None => 0,
                        };
                        if let Some(p) = x.classes.last_mut().and_then(|c| c.props.last_mut()) {
                            p.lower = lower;
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Property, "upperValue", _) => {
                        let upper = match attr(&e, "value").as_deref() {
                            Some("*") => None,
                            Some(v) => {
                                Some(v.parse::<u32>().map_err(|_| format!("{}: an upper bound {v:?} is not a whole number or *", where_am_i(&x)))?)
                            }
                            None => Some(1),
                        };
                        if let Some(p) = x.classes.last_mut().and_then(|c| c.props.last_mut()) {
                            p.upper = upper;
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Property, "defaultValue", _) => {
                        let value = attr(&e, "value");
                        let here_is = where_am_i(&x);
                        let number =
                            |v: &str, what: &str| v.parse::<i64>().map_err(|_| format!("{here_is}: a default {what} {v:?} is not a whole number"));
                        let d = match kind.as_str() {
                            "uml:LiteralBoolean" => match value.as_deref() {
                                Some("true") => Some(XDefault::Bool(true)),
                                Some("false") | None => Some(XDefault::Bool(false)),
                                Some(v) => return Err(format!("{here_is}: a default boolean {v:?} is not true or false")),
                            },
                            "uml:LiteralInteger" => Some(XDefault::Int(match value.as_deref() {
                                Some(v) => number(v, "integer")?,
                                None => 0,
                            })),
                            "uml:LiteralUnlimitedNatural" => Some(XDefault::Unlimited(match value.as_deref() {
                                Some("*") => None,
                                Some(v) => Some(
                                    u64::try_from(number(v, "unlimited natural")?)
                                        .map_err(|_| format!("{here_is}: a default unlimited natural {v:?} is negative"))?,
                                ),
                                None => Some(0),
                            })),
                            "uml:InstanceValue" => attr(&e, "instance").map(XDefault::Literal),
                            _ => None,
                        };
                        if let Some(p) = x.classes.last_mut().and_then(|c| c.props.last_mut()) {
                            p.default = d;
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Enum, "ownedLiteral", _) => {
                        if let Some(en) = x.enums.last_mut() {
                            en.literals.push((attr(&e, "xmi:id").unwrap_or_default(), attr(&e, "name").unwrap_or_default()));
                        }
                        Ctx::Skipped
                    }
                    (Ctx::Class | Ctx::Property | Ctx::Enum, "ownedComment", _) => {
                        comment_owner = Some(here);
                        Ctx::Comment
                    }
                    (Ctx::Comment, "body", _) => Ctx::Body,
                    (
                        Ctx::Class | Ctx::Package | Ctx::Enum | Ctx::Property | Ctx::Assoc | Ctx::AssocEnd | Ctx::Comment | Ctx::Body | Ctx::Skipped,
                        _,
                        _,
                    ) => {
                        let key = if kind.is_empty() { name.clone() } else { kind.clone() };
                        if here != Ctx::Skipped
                            && !matches!(name.as_str(), "packageImport" | "lowerValue" | "upperValue" | "ownedComment" | "body" | "annotatedElement")
                        {
                            *x.skipped.entry(key).or_insert(0) += 1;
                        }
                        Ctx::Skipped
                    }
                };
                // The ids of what is read must be unique; OMG's own file repeats an id among the
                // operations it skips, so the ones skipped are not held to it.
                let read = matches!(next, Ctx::Class | Ctx::Property | Ctx::Enum | Ctx::Assoc | Ctx::AssocEnd)
                    || (here == Ctx::Enum && name == "ownedLiteral");
                if read
                    && let Some(id) = attr(&e, "xmi:id")
                    && !ids.insert(id.clone())
                {
                    return Err(format!("XMI at byte {}: the id {id:?} is used twice", reader.buffer_position()));
                }
                stack.push(next);
            }
            Event::Text(t) => {
                if stack.last() == Some(&Ctx::Body) {
                    let text = t.decode().map_err(|e| e.to_string())?.into_owned();
                    let doc = match comment_owner {
                        Some(Ctx::Class) => x.classes.last_mut().map(|c| &mut c.doc),
                        Some(Ctx::Property) => x.classes.last_mut().and_then(|c| c.props.last_mut()).map(|p| &mut p.doc),
                        Some(Ctx::Enum) => x.enums.last_mut().map(|e| &mut e.doc),
                        _ => None,
                    };
                    if let Some(d) = doc {
                        if !d.is_empty() {
                            d.push(' ');
                        }
                        d.push_str(text.split_whitespace().collect::<Vec<_>>().join(" ").as_str());
                    }
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(x)
}

/// The five primitives every metamodel gets, as data types 0 to 4.
const PRIMITIVES: [(&str, DataKind); 5] = [
    ("Boolean", DataKind::Boolean),
    ("Integer", DataKind::Integer),
    ("Real", DataKind::Real),
    ("String", DataKind::String),
    ("UnlimitedNatural", DataKind::UnlimitedNatural),
];

/// What the generator worked out from the XMI, ready to emit.
pub struct Built {
    pub metamodel: Arc<Metamodel>,
    pub universe: Arc<Universe>,
    report: String,
    hash: String,
}

/// Classes with their supers first, each once, in document order otherwise.
fn topological(classes: &[XClass]) -> Result<Vec<usize>, String> {
    let by_id: BTreeMap<&str, usize> = classes.iter().enumerate().map(|(i, c)| (c.id.as_str(), i)).collect();
    let mut order = Vec::new();
    let mut state = vec![0u8; classes.len()];
    fn visit(
        i: usize,
        classes: &[XClass],
        by_id: &BTreeMap<&str, usize>,
        state: &mut [u8],
        order: &mut Vec<usize>,
        depth: usize,
    ) -> Result<(), String> {
        if state[i] == 2 {
            return Ok(());
        }
        if state[i] == 1 {
            return Err(format!("{} is in an inheritance cycle", classes[i].name));
        }
        if depth > 1000 {
            return Err(format!("{} sits under more than 1000 levels of inheritance", classes[i].name));
        }
        state[i] = 1;
        for s in &classes[i].supers {
            let j = *by_id.get(s.as_str()).ok_or_else(|| format!("{} extends {s}, which is not a class of this metamodel", classes[i].name))?;
            visit(j, classes, by_id, state, order, depth + 1)?;
        }
        state[i] = 2;
        order.push(i);
        Ok(())
    }
    for i in 0..classes.len() {
        visit(i, classes, &by_id, &mut state, &mut order, 0)?;
    }
    Ok(order)
}

pub fn build(t: &Target, bytes: &[u8], docs: bool) -> Result<Built, String> {
    let x = read_xmi(bytes)?;
    if x.classes.is_empty() {
        return Err(format!("{}: the XMI holds no classes", t.name));
    }
    let hash = fnv(bytes);
    let mut b = Builder::new(t.index, t.uri, t.name, t.version, &hash);
    let mut data: BTreeMap<String, DataId> = BTreeMap::new();
    // Primitives are keyed apart from the document's ids, so no xmi:id can shadow one.
    for (name, kind) in PRIMITIVES {
        let id = b.data_type(name, kind, "").map_err(|e| e.to_string())?;
        data.insert(format!("primitive:{name}"), id);
    }
    let mut literal_index: BTreeMap<String, (DataId, u32)> = BTreeMap::new();
    for en in &x.enums {
        let names: Vec<Arc<str>> = en.literals.iter().map(|(_, n)| Arc::from(n.as_str())).collect();
        let mut forms = BTreeSet::new();
        for (_, l) in &en.literals {
            if !forms.insert(pascal(l)) {
                return Err(format!("the enumeration {} has two literals that both become {} in Rust", en.name, pascal(l)));
            }
        }
        let id = b.data_type(&en.name, DataKind::Enumeration(names), if docs { &en.doc } else { "" }).map_err(|e| e.to_string())?;
        if data.insert(en.id.clone(), id).is_some() {
            return Err(format!("the id {:?} names two data types", en.id));
        }
        for (i, (lid, _)) in en.literals.iter().enumerate() {
            let i = u32::try_from(i).map_err(|_| "too many literals".to_string())?;
            literal_index.insert(lid.clone(), (id, i));
        }
    }
    let order = topological(&x.classes)?;
    let mut class_of: BTreeMap<String, ClassId> = BTreeMap::new();
    for &i in &order {
        let c = &x.classes[i];
        let supers: Vec<ClassId> = c.supers.iter().filter_map(|s| class_of.get(s).copied()).collect();
        let id = b.class(&c.name, c.is_abstract, &supers, if docs { &c.doc } else { "" }).map_err(|e| e.to_string())?;
        if class_of.insert(c.id.clone(), id).is_some() {
            return Err(format!("the id {:?} names two classes", c.id));
        }
    }
    // Property ids are positional: assign them all first, so subsets, redefinitions and opposites
    // can refer forward.
    let mut prop_of: BTreeMap<String, PropId> = BTreeMap::new();
    for &i in &order {
        for p in &x.classes[i].props {
            let local = u16::try_from(prop_of.len()).map_err(|_| "more than 65535 properties".to_string())?;
            let id = PropId::new(t.index, local);
            if prop_of.insert(p.id.clone(), id).is_some() {
                return Err(format!("the id {:?} names two properties", p.id));
            }
        }
    }
    let owned_ends: BTreeSet<&str> = x.assocs.iter().flat_map(|a| a.owned_ends.iter().map(String::as_str)).collect();
    let mut opposites: BTreeMap<PropId, PropId> = BTreeMap::new();
    let mut ends_dropped = 0;
    for a in &x.assocs {
        match a.member_ends.as_slice() {
            [e1, e2] => match (prop_of.get(e1), prop_of.get(e2)) {
                (Some(p1), Some(p2)) => {
                    opposites.insert(*p1, *p2);
                    opposites.insert(*p2, *p1);
                }
                _ => ends_dropped += 1,
            },
            _ => ends_dropped += 1,
        }
    }
    let mut subsets_dropped = 0;
    let mut derived_defaults_dropped = 0;
    for &i in &order {
        let c = &x.classes[i];
        let owner = class_of[&c.id];
        for p in &c.props {
            let ty = match &p.ty {
                XType::Ref(id) => {
                    if let Some(c) = class_of.get(id) {
                        TypeRef::Class(*c)
                    } else if let Some(d) = data.get(id) {
                        TypeRef::Data(*d)
                    } else {
                        return Err(format!("{}::{} is typed by {id}, which is not a class or data type here", c.name, p.name));
                    }
                }
                XType::Primitive(name) => TypeRef::Data(
                    *data
                        .get(&format!("primitive:{name}"))
                        .ok_or_else(|| format!("{}::{} is typed by an unknown primitive {name}", c.name, p.name))?,
                ),
                XType::Any => TypeRef::Any,
                XType::None => return Err(format!("{}::{} has no type", c.name, p.name)),
            };
            let mut subsets = Vec::new();
            for s in &p.subsets {
                match prop_of.get(s) {
                    Some(q) => subsets.push(*q),
                    None if owned_ends.contains(s.as_str()) => subsets_dropped += 1,
                    None => return Err(format!("{}::{} subsets {s}, which is not a property here", c.name, p.name)),
                }
            }
            let mut redefines = Vec::new();
            for r in &p.redefines {
                match prop_of.get(r) {
                    Some(q) => redefines.push(*q),
                    None if owned_ends.contains(r.as_str()) => subsets_dropped += 1,
                    None => return Err(format!("{}::{} redefines {r}, which is not a property here", c.name, p.name)),
                }
            }
            let default = match &p.default {
                Some(XDefault::Bool(v)) => Some(Value::Bool(*v)),
                Some(XDefault::Int(v)) => Some(Value::Int(*v)),
                Some(XDefault::Unlimited(v)) => Some(Value::Unlimited(*v)),
                Some(XDefault::Literal(l)) => {
                    Some(Value::Enum(literal_index.get(l).ok_or_else(|| format!("{}::{} defaults to an unknown literal {l}", c.name, p.name))?.1))
                }
                None => None,
            };
            let is_name = p.name == "name" && Some(&ty) == data.get("primitive:String").map(|d| TypeRef::Data(*d)).as_ref();
            let derived = p.derived || p.derived_union;
            // A derived property is computed, so a default on it (UML.xmi has a few) means nothing here.
            let default = if derived {
                if default.is_some() {
                    derived_defaults_dropped += 1;
                }
                None
            } else {
                default
            };
            let spec = PropertySpec {
                name: &p.name,
                owner,
                ty,
                lower: p.lower,
                upper: p.upper,
                is_ordered: p.ordered,
                is_unique: p.unique,
                is_composite: p.composite,
                is_derived: p.derived && !p.derived_union,
                is_derived_union: p.derived_union,
                is_read_only: p.read_only || derived,
                is_id: false,
                is_name,
                opposite: opposites.get(&prop_of[&p.id]).copied(),
                subsets: &subsets,
                redefines: &redefines,
                default,
                doc: if docs { &p.doc } else { "" },
            };
            let id = b.property(spec).map_err(|e| format!("{}::{}: {e}", c.name, p.name))?;
            if id != prop_of[&p.id] {
                return Err(format!("{}::{}: the property ids went out of step", c.name, p.name));
            }
        }
    }
    let metamodel = b.finish();
    let universe = Arc::new(Universe::new(vec![metamodel.clone()]).map_err(|e: MetaError| e.to_string())?);
    let concrete = metamodel.classes.iter().filter(|c| !c.is_abstract).count();
    let derived = metamodel.props.iter().filter(|p| p.is_derived || p.is_derived_union).count();
    let mut report = String::new();
    let _ = writeln!(report, "{} {} from XMI (FNV-1a 64 {hash})", t.name, t.version);
    let _ = writeln!(report, "classes: {} ({concrete} concrete, {} abstract)", metamodel.classes.len(), metamodel.classes.len() - concrete);
    let _ = writeln!(
        report,
        "properties: {} ({derived} derived or derived unions without a field, {} composite, {} with an opposite)",
        metamodel.props.len(),
        metamodel.props.iter().filter(|p| p.is_composite).count(),
        metamodel.props.iter().filter(|p| p.opposite.is_some()).count()
    );
    let _ = writeln!(report, "data types: {} ({} enumerations)", metamodel.data_types.len(), x.enums.len());
    let _ = writeln!(report, "associations: {} ({ends_dropped} with an association-owned or missing end: no stored opposite)", x.assocs.len());
    let _ = writeln!(report, "subsets or redefinitions of association-owned ends dropped: {subsets_dropped}");
    let _ = writeln!(report, "defaults on derived properties dropped: {derived_defaults_dropped}");
    let _ = writeln!(report, "documentation: {}", if docs { "emitted" } else { "not emitted (gm2.23)" });
    if !x.primitives.is_empty() {
        let _ = writeln!(
            report,
            "primitive types declared in the XMI (the built-in five are used): {}",
            x.primitives.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    let _ = writeln!(report, "skipped, not read:");
    for (k, n) in &x.skipped {
        let _ = writeln!(report, "  {k}: {n}");
    }
    let needs_getter: Vec<String> =
        metamodel.props.iter().filter(|p| p.is_derived).map(|p| format!("{}::{}", universe.class_name(p.owner), p.name)).collect();
    let _ = writeln!(report, "derived properties that need a hand-written getter ({}):", needs_getter.len());
    for n in needs_getter {
        let _ = writeln!(report, "  {n}");
    }
    Ok(Built { metamodel, universe, report, hash })
}

/// Generates every file of a target from its XMI at `path`.
pub fn generate(t: &Target, path: &Path, docs: bool) -> Result<Vec<(&'static str, String)>, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if size > MAX_XMI_BYTES {
        return Err(format!("{}: {size} bytes is more than the {MAX_XMI_BYTES} an XMI file may have", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let built = build(t, &bytes, docs)?;
    Ok(emit(t, &built))
}

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

// Names.

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
    "gen",
];

fn snake(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if i > 0 && (prev_lower || (next_lower && chars[i - 1].is_ascii_uppercase())) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(*c);
        }
    }
    if KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

fn scream(name: &str) -> String {
    snake(name).trim_end_matches('_').to_ascii_uppercase()
}

fn pascal(name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in name.chars() {
        if c == '_' || c == '-' || c == ' ' {
            up = true;
        } else if up {
            out.push(c.to_ascii_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    if matches!(out.as_str(), "None" | "Some" | "Ok" | "Err" | "Self") {
        out.push_str("Kind");
    }
    out
}

fn plural(snake: &str) -> String {
    let s = snake.trim_end_matches('_');
    if s.ends_with('y') && !s.ends_with("ay") && !s.ends_with("ey") && !s.ends_with("oy") {
        format!("{}ies", &s[..s.len() - 1])
    } else if s.ends_with('s') || s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") {
        format!("{s}es")
    } else {
        format!("{s}s")
    }
}

/// How a stored property is held in a struct and read and written as a `Value`.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    List,
    Bool,
    Int,
    Real,
    Str,
    Unlimited,
    Enum,
    Ref,
}

impl Built {
    fn class_const(&self, id: ClassId) -> String {
        scream(&self.metamodel.classes[id.local() as usize].name)
    }

    /// `class::OWNED_ATTRIBUTE`: the property constants live in a module named after their class,
    /// so they never collide with the class constants (`Activity::edge` and the class `ActivityEdge`).
    fn prop_const(&self, id: PropId) -> String {
        let p = &self.metamodel.props[id.local() as usize];
        format!("{}::{}", snake(&self.metamodel.classes[p.owner.local() as usize].name), scream(&p.name))
    }

    fn data_const(&self, id: DataId) -> String {
        scream(&self.metamodel.data_types[id.local() as usize].name)
    }

    fn kind(&self, prop: PropId) -> Kind {
        let p = &self.metamodel.props[prop.local() as usize];
        if p.is_many() {
            return Kind::List;
        }
        match p.ty {
            TypeRef::Class(_) | TypeRef::Any => Kind::Ref,
            TypeRef::Data(d) => match &self.metamodel.data_types[d.local() as usize].kind {
                DataKind::Boolean => Kind::Bool,
                DataKind::Integer => Kind::Int,
                DataKind::Real => Kind::Real,
                DataKind::String => Kind::Str,
                DataKind::UnlimitedNatural => Kind::Unlimited,
                DataKind::Enumeration(_) => Kind::Enum,
            },
        }
    }

    /// The field names of a concrete class's stored properties, unique within the struct.
    fn fields(&self, class: ClassId) -> Vec<(PropId, String)> {
        let info = self.universe.info(class).cloned().unwrap_or_default();
        let mut used: BTreeSet<String> = BTreeSet::new();
        let mut out = Vec::new();
        for p in info.stored {
            let prop = &self.metamodel.props[p.local() as usize];
            let mut name = snake(&prop.name);
            if !used.insert(name.clone()) {
                name = format!("{}_{}", snake(&self.metamodel.classes[prop.owner.local() as usize].name), snake(&prop.name));
                used.insert(name.clone());
            }
            out.push((p, name));
        }
        out
    }
}

fn emit(t: &Target, b: &Built) -> Vec<(&'static str, String)> {
    let module = format!("{HEADER}pub mod classes;\npub mod meta;\npub mod schema;\npub mod slabs;\n");
    vec![
        ("mod.rs", module),
        ("meta.rs", emit_meta(t, b)),
        ("classes.rs", emit_classes(b)),
        ("slabs.rs", emit_slabs(b)),
        ("schema.rs", emit_schema(t, b)),
        ("report.txt", b.report.clone()),
    ]
}

const HEADER: &str = "//! Generated by `cargo xtask generate`; do not edit. The gate's `generated` step checks it against the XMI.\n\n";

fn emit_meta(t: &Target, b: &Built) -> String {
    let m = &b.metamodel;
    let mut s = String::from(HEADER);
    let _ = writeln!(s, "#![allow(clippy::too_many_lines, clippy::unreadable_literal)]\n");
    let _ = writeln!(s, "use std::sync::{{Arc, LazyLock}};\n");
    let _ = writeln!(s, "use archforge_mof::{{Builder, ClassId, DataId, DataKind, MetaError, Metamodel, PropId, PropertySpec, TypeRef, Value}};\n");
    let _ = writeln!(s, "pub const INDEX: u16 = {};", t.index);
    let _ = writeln!(s, "pub const URI: &str = {:?};", t.uri);
    let _ = writeln!(s, "pub const VERSION: &str = {:?};", t.version);
    let _ = writeln!(s, "pub const CONTENT_HASH: &str = {:?};\n", b.hash);
    for (i, d) in m.data_types.iter().enumerate() {
        let _ = writeln!(s, "pub const {}: DataId = DataId::new(INDEX, {i});", scream(&d.name));
    }
    s.push('\n');
    for (i, c) in m.classes.iter().enumerate() {
        let _ = writeln!(s, "pub const {}: ClassId = ClassId::new(INDEX, {i});", scream(&c.name));
    }
    s.push('\n');
    for (ci, c) in m.classes.iter().enumerate() {
        if c.own_props.is_empty() {
            continue;
        }
        let _ = writeln!(s, "/// The properties {} declares.\npub mod {} {{\n    use super::PropId;\n    use super::INDEX;", c.name, snake(&c.name));
        for p in &c.own_props {
            let _ = writeln!(s, "    pub const {}: PropId = PropId::new(INDEX, {});", scream(&m.props[p.local() as usize].name), p.local());
        }
        let _ = writeln!(s, "}}");
        let _ = ci;
    }
    let _ = writeln!(s, "\nfn build() -> Result<Arc<Metamodel>, MetaError> {{");
    let _ = writeln!(s, "    let mut b = Builder::new(INDEX, URI, {:?}, VERSION, CONTENT_HASH);", t.name);
    for d in &m.data_types {
        let kind = match &d.kind {
            DataKind::Boolean => "DataKind::Boolean".to_string(),
            DataKind::Integer => "DataKind::Integer".to_string(),
            DataKind::Real => "DataKind::Real".to_string(),
            DataKind::String => "DataKind::String".to_string(),
            DataKind::UnlimitedNatural => "DataKind::UnlimitedNatural".to_string(),
            DataKind::Enumeration(l) => {
                format!("DataKind::Enumeration(vec![{}])", l.iter().map(|x| format!("{x:?}.into()")).collect::<Vec<_>>().join(", "))
            }
        };
        let _ = writeln!(s, "    b.data_type({:?}, {kind}, {:?})?;", &*d.name, &*d.doc);
    }
    for c in &m.classes {
        let supers: Vec<String> = c.supers.iter().map(|x| b.class_const(*x)).collect();
        let _ = writeln!(s, "    b.class({:?}, {}, &[{}], {:?})?;", &*c.name, c.is_abstract, supers.join(", "), &*c.doc);
    }
    for (i, p) in m.props.iter().enumerate() {
        let ty = match p.ty {
            TypeRef::Class(c) => format!("TypeRef::Class({})", b.class_const(c)),
            TypeRef::Data(d) => format!("TypeRef::Data({})", b.data_const(d)),
            TypeRef::Any => "TypeRef::Any".to_string(),
        };
        let upper = match p.upper {
            Some(u) => format!("Some({u})"),
            None => "None".to_string(),
        };
        let default = match &p.default {
            None => "None".to_string(),
            Some(Value::Bool(v)) => format!("Some(Value::Bool({v}))"),
            Some(Value::Int(v)) => format!("Some(Value::Int({v}))"),
            Some(Value::Real(v)) => format!("Some(Value::Real({v:?}))"),
            Some(Value::Unlimited(Some(v))) => format!("Some(Value::Unlimited(Some({v})))"),
            Some(Value::Unlimited(None)) => "Some(Value::Unlimited(None))".to_string(),
            Some(Value::Enum(v)) => format!("Some(Value::Enum({v}))"),
            Some(Value::Str(v)) => format!("Some(Value::str({v:?}))"),
            Some(_) => "None".to_string(),
        };
        let opposite = p.opposite.map_or("None".to_string(), |o| format!("Some({})", b.prop_const(o)));
        let list = |ids: &[PropId]| ids.iter().map(|q| b.prop_const(*q)).collect::<Vec<_>>().join(", ");
        let doc = p.doc.split_once(": ").map(|(_, d)| d).unwrap_or("");
        let _ = writeln!(
            s,
            "    b.property(PropertySpec {{ name: {:?}, owner: {}, ty: {ty}, lower: {}, upper: {upper}, is_ordered: {}, is_unique: {}, is_composite: {}, is_derived: {}, is_derived_union: {}, is_read_only: {}, is_id: {}, is_name: {}, opposite: {opposite}, subsets: &[{}], redefines: &[{}], default: {default}, doc: {:?} }})?; // {}",
            &*p.name,
            b.class_const(p.owner),
            p.lower,
            p.is_ordered,
            p.is_unique,
            p.is_composite,
            p.is_derived,
            p.is_derived_union,
            p.is_read_only,
            p.is_id,
            p.is_name,
            list(&p.subsets),
            list(&p.redefines),
            doc,
            b.prop_const(PropId::new(t.index, i as u16)),
        );
    }
    let _ = writeln!(s, "    Ok(b.finish())\n}}\n");
    let _ = writeln!(s, "static METAMODEL: LazyLock<Result<Arc<Metamodel>, MetaError>> = LazyLock::new(build);\n");
    let _ = writeln!(
        s,
        "/// The metamodel, as its XMI describes it.\npub fn metamodel() -> Result<Arc<Metamodel>, MetaError> {{\n    METAMODEL.clone()\n}}"
    );
    s
}

fn emit_classes(b: &Built) -> String {
    let m = &b.metamodel;
    let mut s = String::from(HEADER);
    let _ = writeln!(s, "#![allow(clippy::too_many_lines)]\n");
    let _ =
        writeln!(s, "use std::sync::Arc;\n\nuse archforge_mof::{{ClassId, Invalid, Obj, PropId, Reflect, Store, Value}};\n\nuse super::meta::*;\n");
    // Enumerations.
    for (i, d) in m.data_types.iter().enumerate() {
        let DataKind::Enumeration(lits) = &d.kind else { continue };
        let name = pascal(&d.name);
        let _ = writeln!(s, "/// The enumeration {}.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {name} {{", d.name);
        for l in lits {
            let _ = writeln!(s, "    {},", pascal(l));
        }
        let _ = writeln!(
            s,
            "}}\n\nimpl {name} {{\n    pub const DATA_TYPE: DataId = {};\n    pub fn from_index(i: u32) -> Option<Self> {{\n        match i {{",
            scream(&d.name)
        );
        for (k, l) in lits.iter().enumerate() {
            let _ = writeln!(s, "            {k} => Some(Self::{}),", pascal(l));
        }
        let _ = writeln!(
            s,
            "            _ => None,\n        }}\n    }}\n    pub fn index(self) -> u32 {{\n        self as u32\n    }}\n    pub fn literal(self) -> &'static str {{\n        match self {{"
        );
        for l in lits {
            let _ = writeln!(s, "            Self::{} => {:?},", pascal(l), &**l);
        }
        let _ = writeln!(s, "        }}\n    }}\n}}\n");
        let _ = i;
    }
    let _ = writeln!(s, "use archforge_mof::DataId;\n");
    for (ci, c) in m.classes.iter().enumerate() {
        if c.is_abstract {
            continue;
        }
        let class = ClassId::new(m.index, ci as u16);
        let sname = format!("{}Obj", pascal(&c.name));
        let fields = b.fields(class);
        // Rust's own defaults serve when the metamodel's defaults are them.
        let plain = fields.iter().all(|(p, _)| {
            let prop = &m.props[p.local() as usize];
            match (b.kind(*p), &prop.default) {
                (Kind::List | Kind::Str | Kind::Ref, _) => true,
                (Kind::Bool, d) => matches!(d, None | Some(Value::Bool(false))),
                (Kind::Int, d) => matches!(d, None | Some(Value::Int(0))),
                (Kind::Real, d) => d.is_none() || matches!(d, Some(Value::Real(r)) if *r == 0.0),
                (Kind::Enum, d) => matches!(d, None | Some(Value::Enum(0))),
                (Kind::Unlimited, d) => matches!(d, Some(Value::Unlimited(None))),
            }
        });
        let derive = if plain { "#[derive(Debug, Clone, PartialEq, Default)]" } else { "#[derive(Debug, Clone, PartialEq)]" };
        let _ = writeln!(s, "/// An object of {}, with every inherited stored property as a field.\n{derive}\npub struct {sname} {{", c.name);
        for (p, f) in &fields {
            let ty = match b.kind(*p) {
                Kind::List => "Vec<Value>",
                Kind::Bool => "bool",
                Kind::Int => "i64",
                Kind::Real => "f64",
                Kind::Str => "Option<Arc<str>>",
                Kind::Unlimited => "Option<u64>",
                Kind::Enum => "u32",
                Kind::Ref => "Option<Obj>",
            };
            let _ = writeln!(s, "    pub {f}: {ty},");
        }
        let _ = writeln!(s, "}}\n");
        if !plain {
            let _ = writeln!(s, "impl Default for {sname} {{\n    fn default() -> Self {{\n        Self {{");
        }
        for (p, f) in &fields {
            if plain {
                break;
            }
            let prop = &m.props[p.local() as usize];
            let v = match (b.kind(*p), &prop.default) {
                (Kind::List, _) => "Vec::new()".to_string(),
                (Kind::Bool, Some(Value::Bool(v))) => v.to_string(),
                (Kind::Bool, _) => "false".to_string(),
                (Kind::Int, Some(Value::Int(v))) => v.to_string(),
                (Kind::Int, _) => "0".to_string(),
                (Kind::Real, Some(Value::Real(v))) => format!("{v:?}"),
                (Kind::Real, _) => "0.0".to_string(),
                (Kind::Str, Some(Value::Str(v))) => format!("Some(Arc::from({v:?}))"),
                (Kind::Str, _) => "None".to_string(),
                (Kind::Unlimited, Some(Value::Unlimited(Some(v)))) => format!("Some({v})"),
                (Kind::Unlimited, Some(Value::Unlimited(None))) => "None".to_string(),
                (Kind::Unlimited, _) => "Some(0)".to_string(),
                (Kind::Enum, Some(Value::Enum(v))) => v.to_string(),
                (Kind::Enum, _) => "0".to_string(),
                (Kind::Ref, _) => "None".to_string(),
            };
            let _ = writeln!(s, "            {f}: {v},");
        }
        if !plain {
            let _ = writeln!(s, "        }}\n    }}\n}}\n");
        }
        // Reflect.
        let singles = fields.iter().filter(|(p, _)| b.kind(*p) != Kind::List).count();
        let lists = fields.len() - singles;
        let _ = writeln!(s, "impl Reflect for {sname} {{\n    fn class(&self) -> ClassId {{\n        {}\n    }}", scream(&c.name));
        if singles == 0 {
            let _ = writeln!(s, "    fn get(&self, _prop: PropId) -> Option<Value> {{\n        None\n    }}");
        } else {
            let _ = writeln!(s, "    fn get(&self, prop: PropId) -> Option<Value> {{\n        match prop {{");
        }
        for (p, f) in &fields {
            let expr = match b.kind(*p) {
                Kind::List => continue,
                Kind::Bool => format!("Value::Bool(self.{f})"),
                Kind::Int => format!("Value::Int(self.{f})"),
                Kind::Real => format!("Value::Real(self.{f})"),
                Kind::Str => format!("self.{f}.clone().map_or(Value::Null, Value::Str)"),
                Kind::Unlimited => format!("Value::Unlimited(self.{f})"),
                Kind::Enum => format!("Value::Enum(self.{f})"),
                Kind::Ref => format!("self.{f}.map_or(Value::Null, Value::Ref)"),
            };
            let _ = writeln!(s, "            {} => Some({expr}),", b.prop_const(*p));
        }
        if singles > 0 {
            let _ = writeln!(s, "            _ => None,\n        }}\n    }}");
        }
        if singles == 0 {
            let _ = writeln!(
                s,
                "    fn set(&mut self, prop: PropId, _value: Value) -> Result<(), String> {{\n        Err(format!(\"no single-valued property {{}} on {}\", prop.0))\n    }}",
                c.name
            );
        } else {
            let _ = writeln!(s, "    fn set(&mut self, prop: PropId, value: Value) -> Result<(), String> {{\n        match prop {{");
        }
        for (p, f) in &fields {
            let arm = match b.kind(*p) {
                Kind::List => continue,
                Kind::Bool => format!("match value {{ Value::Bool(v) => self.{f} = v, other => return Err(format!(\"not a boolean: {{other}}\")) }}"),
                Kind::Int => format!("match value {{ Value::Int(v) => self.{f} = v, other => return Err(format!(\"not an integer: {{other}}\")) }}"),
                Kind::Real => format!("match value {{ Value::Real(v) => self.{f} = v, other => return Err(format!(\"not a real: {{other}}\")) }}"),
                Kind::Str => format!(
                    "match value {{ Value::Null => self.{f} = None, Value::Str(v) => self.{f} = Some(v), other => return Err(format!(\"not a string: {{other}}\")) }}"
                ),
                Kind::Unlimited => format!(
                    "match value {{ Value::Unlimited(v) => self.{f} = v, other => return Err(format!(\"not an unlimited natural: {{other}}\")) }}"
                ),
                Kind::Enum => format!("match value {{ Value::Enum(v) => self.{f} = v, other => return Err(format!(\"not a literal: {{other}}\")) }}"),
                Kind::Ref => format!(
                    "match value {{ Value::Null => self.{f} = None, Value::Ref(v) => self.{f} = Some(v), other => return Err(format!(\"not a reference: {{other}}\")) }}"
                ),
            };
            let _ = writeln!(s, "            {} => {{\n                {arm}\n                Ok(())\n            }}", b.prop_const(*p));
        }
        if singles > 0 {
            let _ = writeln!(s, "            _ => Err(format!(\"no single-valued property {{}} on {}\", prop.0)),\n        }}\n    }}", c.name);
        }
        if lists == 0 {
            let _ = writeln!(s, "    fn list(&self, _prop: PropId) -> Option<&[Value]> {{\n        None\n    }}");
            let _ = writeln!(s, "    fn list_mut(&mut self, _prop: PropId) -> Option<&mut Vec<Value>> {{\n        None\n    }}\n}}\n");
        } else {
            let _ = writeln!(s, "    fn list(&self, prop: PropId) -> Option<&[Value]> {{\n        match prop {{");
            for (p, f) in &fields {
                if b.kind(*p) == Kind::List {
                    let _ = writeln!(s, "            {} => Some(&self.{f}),", b.prop_const(*p));
                }
            }
            let _ = writeln!(s, "            _ => None,\n        }}\n    }}");
            let _ = writeln!(s, "    fn list_mut(&mut self, prop: PropId) -> Option<&mut Vec<Value>> {{\n        match prop {{");
            for (p, f) in &fields {
                if b.kind(*p) == Kind::List {
                    let _ = writeln!(s, "            {} => Some(&mut self.{f}),", b.prop_const(*p));
                }
            }
            let _ = writeln!(s, "            _ => None,\n        }}\n    }}\n}}\n");
        }
        // Typed reads on the struct, typed writes through the store.
        let _ = writeln!(s, "impl {sname} {{");
        for (p, f) in &fields {
            let (ret, body) = match b.kind(*p) {
                Kind::List => ("&[Value]", format!("&self.{f}")),
                Kind::Bool => ("bool", format!("self.{f}")),
                Kind::Int => ("i64", format!("self.{f}")),
                Kind::Real => ("f64", format!("self.{f}")),
                Kind::Str => ("Option<&str>", format!("self.{f}.as_deref()")),
                Kind::Unlimited => ("Option<u64>", format!("self.{f}")),
                Kind::Enum => ("u32", format!("self.{f}")),
                Kind::Ref => ("Option<Obj>", format!("self.{f}")),
            };
            let _ = writeln!(s, "    pub fn {f}(&self) -> {ret} {{\n        {body}\n    }}");
        }
        for (p, f) in &fields {
            let konst = b.prop_const(*p);
            let (param, value) = match b.kind(*p) {
                Kind::List => {
                    let _ = writeln!(
                        s,
                        "    pub fn push_{f}(store: &mut Store, obj: Obj, value: Value) -> Result<u32, Invalid> {{\n        store.push(obj, {konst}, value)\n    }}"
                    );
                    continue;
                }
                Kind::Bool => ("bool", "Value::Bool(v)"),
                Kind::Int => ("i64", "Value::Int(v)"),
                Kind::Real => ("f64", "Value::Real(v)"),
                Kind::Str => ("Option<&str>", "v.map_or(Value::Null, Value::str)"),
                Kind::Unlimited => ("Option<u64>", "Value::Unlimited(v)"),
                Kind::Enum => ("u32", "Value::Enum(v)"),
                Kind::Ref => ("Option<Obj>", "v.map_or(Value::Null, Value::Ref)"),
            };
            let _ = writeln!(
                s,
                "    pub fn set_{f}(store: &mut Store, obj: Obj, v: {param}) -> Result<(), Invalid> {{\n        store.set(obj, {konst}, {value})\n    }}"
            );
        }
        let _ = writeln!(s, "}}\n");
    }
    s
}

fn emit_slabs(b: &Built) -> String {
    let m = &b.metamodel;
    let mut s = String::from(HEADER);
    let _ = writeln!(s, "use archforge_mof::{{ClassId, Reflect, Slabs}};\n\nuse super::classes::*;\nuse super::meta::*;\n");
    let name = format!("{}Slabs", pascal(&m.name));
    let concrete: Vec<(ClassId, String, String)> = m
        .classes
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_abstract)
        .map(|(i, c)| (ClassId::new(m.index, i as u16), plural(&snake(&c.name)), format!("{}Obj", pascal(&c.name))))
        .collect();
    let _ = writeln!(s, "/// One typed slab per concrete metaclass.\n#[derive(Debug, Default)]\npub struct {name} {{");
    for (_, f, t) in &concrete {
        let _ = writeln!(s, "    pub {f}: Vec<{t}>,");
    }
    let _ = writeln!(
        s,
        "}}\n\nfn push<T: Default>(v: &mut Vec<T>) -> Option<u32> {{\n    let slot = u32::try_from(v.len()).ok()?;\n    v.push(T::default());\n    Some(slot)\n}}\n"
    );
    let _ =
        writeln!(s, "fn at<T: Reflect>(v: &[T], slot: u32) -> Option<&dyn Reflect> {{\n    v.get(slot as usize).map(|o| o as &dyn Reflect)\n}}\n");
    let _ = writeln!(
        s,
        "fn at_mut<T: Reflect>(v: &mut [T], slot: u32) -> Option<&mut dyn Reflect> {{\n    v.get_mut(slot as usize).map(|o| o as &mut dyn Reflect)\n}}\n"
    );
    let _ = writeln!(s, "impl Slabs for {name} {{\n    fn create(&mut self, class: ClassId) -> Option<u32> {{\n        match class {{");
    for (c, f, _) in &concrete {
        let _ = writeln!(s, "            {} => push(&mut self.{f}),", b.class_const(*c));
    }
    let _ = writeln!(
        s,
        "            _ => None,\n        }}\n    }}\n    fn object(&self, class: ClassId, slot: u32) -> Option<&dyn Reflect> {{\n        match class {{"
    );
    for (c, f, _) in &concrete {
        let _ = writeln!(s, "            {} => at(&self.{f}, slot),", b.class_const(*c));
    }
    let _ = writeln!(
        s,
        "            _ => None,\n        }}\n    }}\n    fn object_mut(&mut self, class: ClassId, slot: u32) -> Option<&mut dyn Reflect> {{\n        match class {{"
    );
    for (c, f, _) in &concrete {
        let _ = writeln!(s, "            {} => at_mut(&mut self.{f}, slot),", b.class_const(*c));
    }
    let _ = writeln!(s, "            _ => None,\n        }}\n    }}\n    fn bytes(&self) -> usize {{\n        0usize");
    for (_, f, t) in &concrete {
        let _ = writeln!(s, "            .saturating_add(self.{f}.capacity().saturating_mul(size_of::<{t}>()))");
    }
    let _ = writeln!(s, "    }}\n}}");
    s
}

fn emit_schema(t: &Target, b: &Built) -> String {
    let m = &b.metamodel;
    let mut s = String::from(HEADER);
    let _ = writeln!(s, "use archforge_mof::schema::{{Column, Junction, Table}};\n\nuse super::meta::*;\n");
    let mut tables = String::new();
    let mut junctions = String::new();
    // A list's table is named after its class and property, unless a class's table has that name
    // (Domain's check list and the class DomainCheck): then it ends in _list.
    let table_names: std::collections::BTreeSet<String> =
        m.classes.iter().filter(|c| !c.is_abstract).map(|c| format!("{}_{}", t.name, snake(&c.name).trim_end_matches('_'))).collect();
    for (ci, c) in m.classes.iter().enumerate() {
        if c.is_abstract {
            continue;
        }
        let class = ClassId::new(m.index, ci as u16);
        let table = format!("{}_{}", t.name, snake(&c.name).trim_end_matches('_'));
        let _ = writeln!(
            tables,
            "    Table {{\n        name: {table:?},\n        class: {},\n        columns: &[\n            Column {{ name: \"id\", prop: None, sql: \"BINARY(16) NOT NULL PRIMARY KEY\" }},",
            b.class_const(class)
        );
        for (p, f) in b.fields(class) {
            let prop = &m.props[p.local() as usize];
            let kind = b.kind(p);
            if kind == Kind::List {
                let reference = matches!(prop.ty, TypeRef::Class(_) | TypeRef::Any);
                let mut name = format!("{table}_{}", f.trim_end_matches('_'));
                if table_names.contains(&name) {
                    name.push_str("_list");
                }
                let _ = writeln!(
                    junctions,
                    "    Junction {{ name: {name:?}, class: {}, prop: {}, ordered: {}, reference: {reference} }},",
                    b.class_const(class),
                    b.prop_const(p),
                    prop.is_ordered
                );
                continue;
            }
            let sql = match kind {
                Kind::Bool => "TINYINT NOT NULL",
                Kind::Int => "BIGINT NOT NULL",
                Kind::Real => "DOUBLE NOT NULL",
                Kind::Str => "MEDIUMTEXT NULL",
                Kind::Unlimited => "BIGINT NULL",
                Kind::Enum => "INT NOT NULL",
                Kind::Ref => "BINARY(16) NULL",
                Kind::List => unreachable_sql(),
            };
            let _ = writeln!(tables, "            Column {{ name: {:?}, prop: Some({}), sql: {sql:?} }},", f.trim_end_matches('_'), b.prop_const(p));
        }
        let _ = writeln!(tables, "        ],\n    }},");
    }
    let _ = writeln!(s, "/// One table per concrete metaclass.\npub const TABLES: &[Table] = &[\n{tables}];\n");
    let _ = writeln!(s, "/// One table per multi-valued stored property.\npub const JUNCTIONS: &[Junction] = &[\n{junctions}];");
    s
}

fn unreachable_sql() -> &'static str {
    "MEDIUMTEXT NULL"
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOY: Target =
        Target { name: "toy", index: 15, uri: "archforge:toy", version: "1", source: Source::Repo("crates/mof/tests/toy.xmi"), crate_dir: "" };

    fn toy_xmi() -> Vec<u8> {
        std::fs::read(util::root().join("crates/mof/tests/toy.xmi")).unwrap_or_default()
    }

    #[test]
    fn the_toy_as_xmi_gives_the_hand_written_toy() {
        let built = build(&TOY, &toy_xmi(), false).unwrap_or_else(|e| panic!("{e}"));
        let hand = archforge_mof::toy::metamodel().unwrap();
        let gen_ = &built.metamodel;
        assert_eq!(gen_.index, hand.index);
        assert_eq!(gen_.uri, hand.uri);
        assert_eq!(gen_.classes.len(), hand.classes.len());
        for (g, h) in gen_.classes.iter().zip(&hand.classes) {
            assert_eq!((&*g.name, g.is_abstract, &g.supers, &g.own_props), (&*h.name, h.is_abstract, &h.supers, &h.own_props), "{}", h.name);
        }
        assert_eq!(gen_.data_types.len(), hand.data_types.len());
        for (g, h) in gen_.data_types.iter().zip(&hand.data_types) {
            assert_eq!((&*g.name, &g.kind), (&*h.name, &h.kind));
        }
        assert_eq!(gen_.props.len(), hand.props.len());
        for (g, h) in gen_.props.iter().zip(&hand.props) {
            let view = |p: &archforge_mof::Property| {
                format!(
                    "{} {:?} {:?} {}..{:?} ordered={} unique={} composite={} derived={} union={} ro={} name={} opp={:?} subsets={:?} redefines={:?} default={:?}",
                    p.name,
                    p.owner,
                    p.ty,
                    p.lower,
                    p.upper,
                    p.is_ordered,
                    p.is_unique,
                    p.is_composite,
                    p.is_derived,
                    p.is_derived_union,
                    p.is_read_only,
                    p.is_name,
                    p.opposite,
                    p.subsets,
                    p.redefines,
                    p.default
                )
            };
            assert_eq!(view(g), view(h), "{}", h.doc);
        }
        assert!(built.report.contains("uml:Operation: 1"), "{}", built.report);
    }

    #[test]
    fn the_emitted_code_names_things_as_rust_does() {
        assert_eq!(snake("ownedAttribute"), "owned_attribute");
        assert_eq!(snake("isAbstract"), "is_abstract");
        assert_eq!(snake("URI"), "uri");
        assert_eq!(snake("type"), "type_");
        assert_eq!(snake("class"), "class", "not a Rust keyword");
        assert_eq!(snake("XMLThing"), "xml_thing");
        assert_eq!(scream("ownedAttribute"), "OWNED_ATTRIBUTE");
        assert_eq!(scream("type"), "TYPE");
        assert_eq!(pascal("none"), "NoneKind");
        assert_eq!(pascal("in"), "In");
        assert_eq!(plural("class"), "classes");
        assert_eq!(plural("property"), "properties");
        assert_eq!(plural("package"), "packages");
        let built = build(&TOY, &toy_xmi(), false).unwrap();
        let files = emit(&TOY, &built);
        let classes = &files.iter().find(|(n, _)| *n == "classes.rs").unwrap().1;
        assert!(
            classes.contains("pub struct PropertyObj {")
                && classes.contains("pub type_: Option<Obj>,")
                && classes.contains("pub class: Option<Obj>,")
        );
        assert!(classes.contains("pub enum AggregationKind {") && classes.contains("NoneKind,"));
        let schema = &files.iter().find(|(n, _)| *n == "schema.rs").unwrap().1;
        assert!(schema.contains("name: \"toy_class_owned_attribute\"") && schema.contains("ordered: true"));
        let meta = &files.iter().find(|(n, _)| *n == "meta.rs").unwrap().1;
        assert!(
            meta.contains("pub mod property {")
                && meta.contains("pub const CLASS: PropId")
                && meta.contains("opposite: Some(class::OWNED_ATTRIBUTE)"),
            "{meta}"
        );
    }

    #[test]
    fn our_own_metamodels_read_as_their_specs_say() {
        let target = |name: &str| TARGETS.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no target {name}"));
        let core = build(target("core"), &std::fs::read(util::root().join("crates/core/core.xmi")).unwrap_or_default(), false)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(core.metamodel.classes.len(), 8);
        let er =
            build(target("er"), &std::fs::read(util::root().join("crates/er/er.xmi")).unwrap_or_default(), false).unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<&str> = er.metamodel.classes.iter().map(|c| &*c.name).collect();
        assert_eq!(
            names,
            [
                "Element",
                "NamedElement",
                "Schema",
                "Entity",
                "Attribute",
                "Relationship",
                "Key",
                "PrimaryKey",
                "UniqueConstraint",
                "ForeignKey",
                "CheckConstraint",
                "TableIndex",
                "IndexColumn",
                "Domain",
                "DomainCheck",
                "EnumType",
                "Generalization",
                "Sequence",
                "View"
            ]
        );
        let name = er.metamodel.props.iter().find(|p| &*p.name == "name" && er.universe.class_name(p.owner) == "NamedElement").unwrap();
        assert!(name.is_name && name.lower == 1, "an entity's name is required and indexed");
        let rel_name = er.metamodel.props.iter().find(|p| &*p.name == "name" && er.universe.class_name(p.owner) == "Relationship").unwrap();
        assert!(rel_name.lower == 0, "a relationship's name is optional");
        assert_eq!(er.metamodel.props.iter().filter(|p| p.is_composite).count(), 16);
        let diagram = build(target("diagram"), &std::fs::read(util::root().join("crates/diagram/diagram.xmi")).unwrap_or_default(), false)
            .unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<&str> = diagram.metamodel.classes.iter().map(|c| &*c.name).collect();
        // View is the abstract end of an edge: a node, or an edge a note links to.
        assert_eq!(names, ["Element", "Diagram", "View", "Node", "Edge", "Point", "Label", "Style"]);
        assert_eq!(
            diagram.metamodel.props.iter().filter(|p| p.is_composite).count(),
            9,
            "node, edge, child, three styles (a diagram's, a node's, an edge's), waypoint, label and the union"
        );
        let any = |class: &str, prop: &str| {
            diagram.metamodel.props.iter().any(|p| &*p.name == prop && diagram.universe.class_name(p.owner) == class && p.ty == TypeRef::Any)
        };
        assert!(any("Diagram", "element") && any("Node", "element") && any("Edge", "element"), "what a view shows is typed Any");
    }

    #[test]
    fn broken_xmi_is_refused_with_its_reason() {
        let cyclic =
            br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"><generalization xmi:type="uml:Generalization" general="B"/></packagedElement>
            <packagedElement xmi:type="uml:Class" xmi:id="B" name="B"><generalization xmi:type="uml:Generalization" general="A"/></packagedElement>
            </uml:Package></xmi:XMI>"#;
        let err = |r: Result<Built, String>| r.err().unwrap_or_default();
        assert!(err(build(&TOY, cyclic, false)).contains("inheritance cycle"));
        let untyped = br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"><ownedAttribute xmi:type="uml:Property" xmi:id="A-x" name="x"/></packagedElement>
            </uml:Package></xmi:XMI>"#;
        assert!(err(build(&TOY, untyped, false)).contains("has no type"));
        assert!(build(&TOY, b"<not xml", false).is_err());
        let twice = br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"/><packagedElement xmi:type="uml:Class" xmi:id="A" name="B"/>
            </uml:Package></xmi:XMI>"#;
        assert!(err(build(&TOY, twice, false)).contains("used twice"));
        let bad_bound = br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"><ownedAttribute xmi:type="uml:Property" xmi:id="A-x" name="x" type="A"><lowerValue xmi:type="uml:LiteralInteger" value="-1"/></ownedAttribute></packagedElement>
            </uml:Package></xmi:XMI>"#;
        assert!(err(build(&TOY, bad_bound, false)).contains("not a whole number"));
        let literal = br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Enumeration" xmi:id="E" name="E"><ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="E-a" name="a } fn x() {"/></packagedElement>
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"/>
            </uml:Package></xmi:XMI>"#;
        assert!(err(build(&TOY, literal, false)).contains("not a plain identifier"));
        let shadow = br#"<?xml version="1.0"?><xmi:XMI xmlns:uml="u" xmlns:xmi="x"><uml:Package xmi:type="uml:Package" name="bad" URI="archforge:bad">
            <packagedElement xmi:type="uml:Enumeration" xmi:id="String" name="Str"><ownedLiteral xmi:type="uml:EnumerationLiteral" xmi:id="S-a" name="a"/></packagedElement>
            <packagedElement xmi:type="uml:Class" xmi:id="A" name="A"><ownedAttribute xmi:type="uml:Property" xmi:id="A-name" name="name"><type href="PrimitiveTypes.xmi#String"/></ownedAttribute></packagedElement>
            </uml:Package></xmi:XMI>"#;
        let built = build(&TOY, shadow, false).unwrap_or_else(|e| panic!("{e}"));
        assert!(built.metamodel.props[0].is_name, "an enumeration whose id is String does not shadow the primitive");
        assert!(err(build(&TOY, b"<?xml version=\"1.0\"?><xmi:XMI xmlns:xmi=\"x\"/>", false)).contains("no classes"));
    }
}
