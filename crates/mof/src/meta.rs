//! The meta-metamodel (M3): what a metamodel is made of. A `Metamodel` is an immutable description
//! of classes, properties and data types, built once by generated code (or a hand-written toy)
//! through the `Builder`, which checks it. A `Universe` resolves several metamodels into one id
//! space: inheritance flattened, subsets and derived unions wired, opposites checked.
//!
//! EMOF plus what UML needs: subsets, derived unions, redefinitions (resolved read-only) and
//! association-owned ends. No operations, no package merge, no OCL, no profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::value::Value;

/// Ids are global: the metamodel's index in the high half, the local index in the low half.
macro_rules! id_type {
    ($name:ident, $what:literal) => {
        #[doc = concat!("The id of ", $what, ": metamodel index << 16 | local index.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn new(metamodel: u16, local: u16) -> $name {
                $name(((metamodel as u32) << 16) | local as u32)
            }
            pub fn metamodel(self) -> u16 {
                (self.0 >> 16) as u16
            }
            pub fn local(self) -> u16 {
                (self.0 & 0xffff) as u16
            }
        }
    };
}

id_type!(ClassId, "a class");
id_type!(PropId, "a property");
id_type!(DataId, "a data type");

/// What a property is typed by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeRef {
    Class(ClassId),
    Data(DataId),
    /// Any object of any metamodel (MOF's Element), for trace links and comments.
    Any,
}

#[derive(Clone, PartialEq, Debug)]
pub enum DataKind {
    Boolean,
    Integer,
    Real,
    String,
    UnlimitedNatural,
    Enumeration(Vec<Arc<str>>),
}

#[derive(Clone, Debug)]
pub struct DataType {
    pub name: Arc<str>,
    pub uri: Arc<str>,
    pub kind: DataKind,
    pub doc: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: Arc<str>,
    pub uri: Arc<str>,
    pub is_abstract: bool,
    pub supers: Vec<ClassId>,
    /// The properties this class declares itself, in order.
    pub own_props: Vec<PropId>,
    pub doc: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct Property {
    pub name: Arc<str>,
    pub owner: ClassId,
    pub ty: TypeRef,
    pub lower: u32,
    /// None is `*`.
    pub upper: Option<u32>,
    pub is_ordered: bool,
    pub is_unique: bool,
    pub is_composite: bool,
    pub is_derived: bool,
    pub is_derived_union: bool,
    pub is_read_only: bool,
    pub is_id: bool,
    /// The property that names its owner, for the name index and namespace rules.
    pub is_name: bool,
    pub opposite: Option<PropId>,
    pub subsets: Vec<PropId>,
    pub redefines: Vec<PropId>,
    pub default: Option<Value>,
    pub doc: Arc<str>,
}

impl Property {
    pub fn is_many(&self) -> bool {
        self.upper != Some(1)
    }

    /// Whether values of it are kept in the store (not derived).
    pub fn is_stored(&self) -> bool {
        !self.is_derived && !self.is_derived_union
    }
}

/// One metamodel, as its generator describes it.
#[derive(Debug)]
pub struct Metamodel {
    pub index: u16,
    pub uri: Arc<str>,
    pub name: Arc<str>,
    pub version: Arc<str>,
    /// A hash of the description the generator read, so a project can say which it conforms to.
    pub content_hash: Arc<str>,
    pub classes: Vec<Class>,
    pub props: Vec<Property>,
    pub data_types: Vec<DataType>,
}

/// Why a metamodel or a universe is not well formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaError(pub String);

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Builds a metamodel, checking names and references as they come.
pub struct Builder {
    mm: Metamodel,
}

impl Builder {
    pub fn new(index: u16, uri: &str, name: &str, version: &str, content_hash: &str) -> Builder {
        Builder {
            mm: Metamodel {
                index,
                uri: uri.into(),
                name: name.into(),
                version: version.into(),
                content_hash: content_hash.into(),
                classes: Vec::new(),
                props: Vec::new(),
                data_types: Vec::new(),
            },
        }
    }

    /// Adds a class; its id is its position.
    pub fn class(&mut self, name: &str, is_abstract: bool, supers: &[ClassId], doc: &str) -> Result<ClassId, MetaError> {
        plain_name(name)?;
        if self.mm.classes.len() >= u16::MAX as usize {
            return Err(MetaError("too many classes".into()));
        }
        if self.mm.classes.iter().any(|c| *c.name == *name) {
            return Err(MetaError(format!("the class {name} is declared twice")));
        }
        let id = ClassId::new(self.mm.index, self.mm.classes.len() as u16);
        for s in supers {
            if *s == id || (s.metamodel() == self.mm.index && s.local() as usize >= self.mm.classes.len()) {
                return Err(MetaError(format!("the class {name} extends a class that is not declared before it")));
            }
        }
        self.mm.classes.push(Class {
            name: name.into(),
            uri: format!("{}#{name}", self.mm.uri).into(),
            is_abstract,
            supers: supers.to_vec(),
            own_props: Vec::new(),
            doc: doc.into(),
        });
        Ok(id)
    }

    pub fn data_type(&mut self, name: &str, kind: DataKind, doc: &str) -> Result<DataId, MetaError> {
        plain_name(name)?;
        if self.mm.data_types.len() >= u16::MAX as usize {
            return Err(MetaError("too many data types".into()));
        }
        if let DataKind::Enumeration(literals) = &kind {
            let set: BTreeSet<&str> = literals.iter().map(|l| &**l).collect();
            if set.len() != literals.len() || literals.is_empty() {
                return Err(MetaError(format!("the enumeration {name} needs distinct literals")));
            }
            for l in literals {
                plain_name(l).map_err(|e| MetaError(format!("the enumeration {name}: {e}")))?;
            }
        }
        let id = DataId::new(self.mm.index, self.mm.data_types.len() as u16);
        self.mm.data_types.push(DataType { name: name.into(), uri: format!("{}#{name}", self.mm.uri).into(), kind, doc: doc.into() });
        Ok(id)
    }

    /// Adds a property to a class of this metamodel; its id is its position.
    pub fn property(&mut self, spec: PropertySpec<'_>) -> Result<PropId, MetaError> {
        plain_name(spec.name)?;
        if self.mm.props.len() >= u16::MAX as usize {
            return Err(MetaError("too many properties".into()));
        }
        let is_string = self.is_string(spec.ty);
        let owner = self
            .mm
            .classes
            .get_mut(spec.owner.local() as usize)
            .filter(|_| spec.owner.metamodel() == self.mm.index)
            .ok_or_else(|| MetaError(format!("the property {} belongs to a class of another metamodel", spec.name)))?;
        if spec.lower > spec.upper.unwrap_or(u32::MAX) || spec.upper == Some(0) {
            return Err(MetaError(format!("the property {}.{} has an impossible multiplicity", owner.name, spec.name)));
        }
        if (spec.is_derived || spec.is_derived_union) && spec.default.is_some() {
            return Err(MetaError(format!("the derived property {}.{} has a default", owner.name, spec.name)));
        }
        if !spec.is_unique && matches!(spec.ty, TypeRef::Class(_) | TypeRef::Any) {
            return Err(MetaError(format!(
                "the reference property {}.{} is not unique; the store's indexes need unique references",
                owner.name, spec.name
            )));
        }
        if spec.is_composite && !matches!(spec.ty, TypeRef::Class(_) | TypeRef::Any) {
            return Err(MetaError(format!("the composite property {}.{} is not typed by a class", owner.name, spec.name)));
        }
        if spec.is_name && !is_string {
            return Err(MetaError(format!("the name property {}.{} is not a string", owner.name, spec.name)));
        }
        let id = PropId::new(self.mm.index, self.mm.props.len() as u16);
        owner.own_props.push(id);
        let owner_name = owner.name.clone();
        self.mm.props.push(Property {
            name: spec.name.into(),
            owner: spec.owner,
            ty: spec.ty,
            lower: spec.lower,
            upper: spec.upper,
            is_ordered: spec.is_ordered,
            is_unique: spec.is_unique,
            is_composite: spec.is_composite,
            is_derived: spec.is_derived,
            is_derived_union: spec.is_derived_union,
            is_read_only: spec.is_read_only,
            is_id: spec.is_id,
            is_name: spec.is_name,
            opposite: spec.opposite,
            subsets: spec.subsets.to_vec(),
            redefines: spec.redefines.to_vec(),
            default: spec.default,
            doc: format!("{owner_name}::{}: {}", spec.name, spec.doc).into(),
        });
        Ok(id)
    }

    fn is_string(&self, ty: TypeRef) -> bool {
        match ty {
            TypeRef::Data(d) if d.metamodel() == self.mm.index => {
                matches!(self.mm.data_types.get(d.local() as usize), Some(dt) if dt.kind == DataKind::String)
            }
            _ => false,
        }
    }

    pub fn finish(self) -> Arc<Metamodel> {
        Arc::new(self.mm)
    }
}

/// What `Builder::property` takes; the defaults are MOF's.
pub struct PropertySpec<'a> {
    pub name: &'a str,
    pub owner: ClassId,
    pub ty: TypeRef,
    pub lower: u32,
    pub upper: Option<u32>,
    pub is_ordered: bool,
    pub is_unique: bool,
    pub is_composite: bool,
    pub is_derived: bool,
    pub is_derived_union: bool,
    pub is_read_only: bool,
    pub is_id: bool,
    pub is_name: bool,
    pub opposite: Option<PropId>,
    pub subsets: &'a [PropId],
    pub redefines: &'a [PropId],
    pub default: Option<Value>,
    pub doc: &'a str,
}

impl<'a> PropertySpec<'a> {
    /// A single-valued optional property with MOF's defaults.
    pub fn new(name: &'a str, owner: ClassId, ty: TypeRef) -> PropertySpec<'a> {
        PropertySpec {
            name,
            owner,
            ty,
            lower: 0,
            upper: Some(1),
            is_ordered: false,
            is_unique: true,
            is_composite: false,
            is_derived: false,
            is_derived_union: false,
            is_read_only: false,
            is_id: false,
            is_name: false,
            opposite: None,
            subsets: &[],
            redefines: &[],
            default: None,
            doc: "",
        }
    }
    pub fn many(mut self) -> Self {
        self.upper = None;
        self
    }
    pub fn required(mut self) -> Self {
        self.lower = 1;
        self
    }
    pub fn ordered(mut self) -> Self {
        self.is_ordered = true;
        self
    }
    pub fn composite(mut self) -> Self {
        self.is_composite = true;
        self
    }
    pub fn derived_union(mut self) -> Self {
        self.is_derived_union = true;
        self.is_read_only = true;
        self
    }
    pub fn derived(mut self) -> Self {
        self.is_derived = true;
        self.is_read_only = true;
        self
    }
    pub fn name_of_owner(mut self) -> Self {
        self.is_name = true;
        self
    }
    pub fn opposite(mut self, p: PropId) -> Self {
        self.opposite = Some(p);
        self
    }
    pub fn subsets(mut self, p: &'a [PropId]) -> Self {
        self.subsets = p;
        self
    }
    pub fn default(mut self, v: Value) -> Self {
        self.default = Some(v);
        self
    }
    pub fn doc(mut self, d: &'a str) -> Self {
        self.doc = d;
        self
    }
}

fn plain_name(name: &str) -> Result<(), MetaError> {
    let ok = !name.is_empty()
        && name.len() <= 128
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok { Ok(()) } else { Err(MetaError(format!("{name:?} is not a plain identifier"))) }
}

/// A class as the universe resolved it.
#[derive(Debug, Clone, Default)]
pub struct ClassInfo {
    /// The class and every ancestor, sorted.
    pub all_supers: Vec<ClassId>,
    /// Every property the class has, inherited first, in declaration order, redefined ones replaced.
    pub all_props: Vec<PropId>,
    /// The stored properties among them.
    pub stored: Vec<PropId>,
    pub name_prop: Option<PropId>,
    /// For each derived union of the class: the stored properties whose values make it up.
    pub unions: BTreeMap<PropId, Vec<PropId>>,
    /// For each stored property: the stored properties it (transitively) subsets, which follow it.
    pub subsetted: BTreeMap<PropId, Vec<PropId>>,
}

/// Several metamodels resolved into one id space.
#[derive(Debug)]
pub struct Universe {
    metamodels: BTreeMap<u16, Arc<Metamodel>>,
    infos: BTreeMap<ClassId, ClassInfo>,
}

impl Universe {
    pub fn new(metamodels: Vec<Arc<Metamodel>>) -> Result<Universe, MetaError> {
        let mut map = BTreeMap::new();
        for mm in metamodels {
            if map.insert(mm.index, mm.clone()).is_some() {
                return Err(MetaError(format!("two metamodels have the index {}", mm.index)));
            }
        }
        let mut u = Universe { metamodels: map, infos: BTreeMap::new() };
        u.resolve()?;
        Ok(u)
    }

    pub fn metamodels(&self) -> impl Iterator<Item = &Arc<Metamodel>> {
        self.metamodels.values()
    }

    pub fn class(&self, id: ClassId) -> Option<&Class> {
        self.metamodels.get(&id.metamodel())?.classes.get(id.local() as usize)
    }

    pub fn prop(&self, id: PropId) -> Option<&Property> {
        self.metamodels.get(&id.metamodel())?.props.get(id.local() as usize)
    }

    pub fn data_type(&self, id: DataId) -> Option<&DataType> {
        self.metamodels.get(&id.metamodel())?.data_types.get(id.local() as usize)
    }

    pub fn info(&self, id: ClassId) -> Option<&ClassInfo> {
        self.infos.get(&id)
    }

    pub fn classes(&self) -> impl Iterator<Item = ClassId> + '_ {
        self.infos.keys().copied()
    }

    /// Whether an object of `class` may be a value of a property typed `ty`.
    pub fn conforms(&self, class: ClassId, ty: TypeRef) -> bool {
        match ty {
            TypeRef::Any => true,
            TypeRef::Data(_) => false,
            TypeRef::Class(c) => self.infos.get(&class).is_some_and(|i| i.all_supers.binary_search(&c).is_ok()),
        }
    }

    /// Whether `class` has the property `prop`.
    pub fn has(&self, class: ClassId, prop: PropId) -> bool {
        self.infos.get(&class).is_some_and(|i| i.all_props.contains(&prop))
    }

    /// The class's name, for messages.
    pub fn class_name(&self, id: ClassId) -> String {
        self.class(id).map_or_else(|| format!("class {}", id.0), |c| c.name.to_string())
    }

    pub fn prop_name(&self, id: PropId) -> String {
        match self.prop(id) {
            Some(p) => format!("{}::{}", self.class_name(p.owner), p.name),
            None => format!("property {}", id.0),
        }
    }

    fn resolve(&mut self) -> Result<(), MetaError> {
        let ids: Vec<ClassId> =
            self.metamodels.values().flat_map(|mm| (0..mm.classes.len() as u16).map(move |i| ClassId::new(mm.index, i))).collect();
        // No inheritance cycle anywhere, before any walk that would follow one.
        for id in &ids {
            let mut seen = BTreeSet::new();
            let mut stack: Vec<ClassId> = self.class(*id).map(|c| c.supers.clone()).unwrap_or_default();
            while let Some(c) = stack.pop() {
                if c == *id {
                    return Err(MetaError(format!("{} inherits from itself", self.class_name(*id))));
                }
                if seen.insert(c) {
                    stack.extend(self.class(c).map(|k| k.supers.clone()).unwrap_or_default());
                }
            }
        }
        // The flattened properties.
        for id in &ids {
            let mut all = BTreeSet::new();
            let mut stack = vec![*id];
            while let Some(c) = stack.pop() {
                if !all.insert(c) {
                    continue;
                }
                if all.len() > 10_000 {
                    return Err(MetaError(format!("{} has a cyclic or absurd inheritance", self.class_name(*id))));
                }
                if let Some(cl) = self.class(c) {
                    for s in &cl.supers {
                        if *s == *id {
                            return Err(MetaError(format!("{} inherits from itself", self.class_name(*id))));
                        }
                        stack.push(*s);
                    }
                }
            }
            let all_supers: Vec<ClassId> = all.into_iter().collect();
            let mut ordered: Vec<ClassId> = Vec::new();
            self.linearize(*id, &mut ordered);
            let mut all_props: Vec<PropId> = Vec::new();
            // What a redefined property is called in this class: a subset of it follows.
            let mut redefined: BTreeMap<PropId, PropId> = BTreeMap::new();
            for c in &ordered {
                if let Some(cl) = self.class(*c) {
                    for p in &cl.own_props {
                        if let Some(prop) = self.prop(*p) {
                            for r in &prop.redefines {
                                all_props.retain(|x| x != r);
                                redefined.insert(*r, *p);
                            }
                        }
                        if !all_props.contains(p) {
                            all_props.push(*p);
                        }
                    }
                }
            }
            let follow = |mut q: PropId| {
                for _ in 0..64 {
                    match redefined.get(&q) {
                        Some(r) if *r != q => q = *r,
                        _ => break,
                    }
                }
                q
            };
            let stored: Vec<PropId> = all_props.iter().copied().filter(|p| self.prop(*p).is_some_and(Property::is_stored)).collect();
            let names: Vec<PropId> = all_props.iter().copied().filter(|p| self.prop(*p).is_some_and(|p| p.is_name)).collect();
            if names.len() > 1 {
                return Err(MetaError(format!("{} has two name properties", self.class_name(*id))));
            }
            let mut unions: BTreeMap<PropId, Vec<PropId>> = BTreeMap::new();
            let mut subsetted: BTreeMap<PropId, Vec<PropId>> = BTreeMap::new();
            for s in &stored {
                let mut seen = BTreeSet::new();
                let mut stack = vec![*s];
                while let Some(p) = stack.pop() {
                    let Some(prop) = self.prop(p) else { continue };
                    for q in &prop.subsets {
                        let q = follow(*q);
                        if !all_props.contains(&q) {
                            return Err(MetaError(format!(
                                "{} subsets {}, which {} does not have",
                                self.prop_name(*s),
                                self.prop_name(q),
                                self.class_name(*id)
                            )));
                        }
                        if q != *s && seen.insert(q) {
                            stack.push(q);
                            match self.prop(q) {
                                Some(t) if t.is_derived_union => unions.entry(q).or_default().push(*s),
                                Some(t) if t.is_stored() => subsetted.entry(*s).or_default().push(q),
                                _ => {}
                            }
                        }
                    }
                }
            }
            self.infos.insert(*id, ClassInfo { all_supers, all_props, stored, name_prop: names.first().copied(), unions, subsetted });
        }
        // Every referenced class, data type and property must exist, and opposites must fit
        // (checked after the inheritance is known).
        for mm in self.metamodels.values() {
            for c in &mm.classes {
                for s in &c.supers {
                    if self.class(*s).is_none() {
                        return Err(MetaError(format!("{} extends a class that does not exist", c.name)));
                    }
                }
            }
            for (local, p) in mm.props.iter().enumerate() {
                let own_id = PropId::new(mm.index, local as u16);
                let name = format!("{}::{}", self.class_name(p.owner), p.name);
                match p.ty {
                    TypeRef::Class(c) if self.class(c).is_none() => return Err(MetaError(format!("{name} is typed by a class that does not exist"))),
                    TypeRef::Data(d) if self.data_type(d).is_none() => {
                        return Err(MetaError(format!("{name} is typed by a data type that does not exist")));
                    }
                    _ => {}
                }
                for q in p.subsets.iter().chain(&p.redefines).chain(p.opposite.as_ref()) {
                    if self.prop(*q).is_none() {
                        return Err(MetaError(format!("{name} refers to a property that does not exist")));
                    }
                }
                if let Some(o) = p.opposite {
                    let other = self.prop(o).ok_or_else(|| MetaError(format!("{name}: no opposite")))?;
                    if other.opposite != Some(own_id) {
                        return Err(MetaError(format!("{name} and its opposite do not refer to each other")));
                    }
                    if other.is_composite && p.is_composite {
                        return Err(MetaError(format!("{name} and its opposite are both composite")));
                    }
                    if !matches!(other.ty, TypeRef::Class(c) if self.conforms(p.owner, TypeRef::Class(c))) && other.ty != TypeRef::Any {
                        return Err(MetaError(format!("{name}'s opposite is not typed by its owner")));
                    }
                }
                if let Some(d) = &p.default {
                    if p.is_many() {
                        return Err(MetaError(format!("{name} is multi-valued and has a default")));
                    }
                    if !self.value_fits(p.ty, d) || matches!(d, Value::Ref(_)) {
                        return Err(MetaError(format!("{name}'s default does not fit its type")));
                    }
                }
            }
        }
        Ok(())
    }

    /// Supers first (depth first, each once), then the class itself.
    fn linearize(&self, id: ClassId, out: &mut Vec<ClassId>) {
        if out.contains(&id) {
            return;
        }
        if let Some(c) = self.class(id) {
            for s in &c.supers {
                self.linearize(*s, out);
            }
        }
        out.push(id);
    }

    /// Whether `value` is of the data type or class `ty` (a reference only by kind here; conformance
    /// of its object is the store's check).
    pub fn value_fits(&self, ty: TypeRef, value: &Value) -> bool {
        match (ty, value) {
            (_, Value::Null) => true,
            (TypeRef::Class(_) | TypeRef::Any, Value::Ref(_)) => true,
            (TypeRef::Data(d), v) => match self.data_type(d).map(|t| &t.kind) {
                Some(DataKind::Boolean) => matches!(v, Value::Bool(_)),
                Some(DataKind::Integer) => matches!(v, Value::Int(_)),
                Some(DataKind::Real) => matches!(v, Value::Real(r) if r.is_finite()),
                Some(DataKind::String) => matches!(v, Value::Str(s) if s.chars().count() <= crate::value::MAX_STRING),
                Some(DataKind::UnlimitedNatural) => matches!(v, Value::Unlimited(_)),
                Some(DataKind::Enumeration(lits)) => matches!(v, Value::Enum(i) if (*i as usize) < lits.len()),
                None => false,
            },
            _ => false,
        }
    }
}
