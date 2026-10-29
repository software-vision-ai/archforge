//! The object store: every object of every metamodel in one id space, its data in the typed slabs
//! the generated code brings, changed only through ops that have exact inverses.
//!
//! Two levels: the raw level applies one op, checks that it fits the state (so undo and redo can
//! never half-apply), and keeps the indexes exact; the semantic level (`create`, `set`, `insert`,
//! `remove_at`, `move_item`, `destroy`) checks what MOF demands, makes the raw ops that keep
//! containment, opposites and subsets consistent, and is atomic: a refusal half way is taken back
//! by the inverse ops, so a refused change changes nothing. While a recording is on, every raw op
//! is kept; the engine takes them as the edit's delta.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use crate::id::Uuid;
use crate::meta::{ClassId, DataKind, PropId, Property, TypeRef, Universe};
use crate::value::{MAX_STRING, Value};

/// The most live objects a store holds.
pub const MAX_OBJECTS: usize = 10_000_000;
/// The most ops one recording may hold: a cascade over a huge subtree is refused above it.
pub const MAX_OPS: usize = 1_000_000;
/// The most values one list holds.
pub const MAX_LIST: usize = 1_000_000;

/// A handle to an object inside one store; never leaves the process. The uuid does.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Obj(pub u32);

impl fmt::Display for Obj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object {}", self.0)
    }
}

/// What the generated code implements for each metaclass: its stored properties as values.
pub trait Reflect: Send {
    fn class(&self) -> ClassId;
    /// A single-valued stored property's value; None for a property the object does not have, or
    /// a derived one the generated code does not compute.
    fn get(&self, prop: PropId) -> Option<Value>;
    /// Sets a single-valued stored property; an error names what did not fit (an internal error:
    /// the store checked the value first).
    fn set(&mut self, prop: PropId, value: Value) -> Result<(), String>;
    /// A multi-valued stored property's values.
    fn list(&self, prop: PropId) -> Option<&[Value]>;
    fn list_mut(&mut self, prop: PropId) -> Option<&mut Vec<Value>>;
}

/// What the generated code implements for a metamodel: a typed slab per concrete metaclass.
pub trait Slabs: Send + 'static {
    /// A new object of `class` with its defaults; its slot. None for a class this slab set has not.
    fn create(&mut self, class: ClassId) -> Option<u32>;
    fn object(&self, class: ClassId, slot: u32) -> Option<&dyn Reflect>;
    fn object_mut(&mut self, class: ClassId, slot: u32) -> Option<&mut dyn Reflect>;
    /// An estimate of the bytes held.
    fn bytes(&self) -> usize;
}

/// The slabs of several metamodels as one, dispatching on the class's metamodel index, so one store
/// holds objects of every metamodel a project uses.
#[derive(Default)]
pub struct CompositeSlabs {
    parts: Vec<(u16, Box<dyn Slabs>)>,
}

impl CompositeSlabs {
    pub fn new() -> CompositeSlabs {
        CompositeSlabs::default()
    }

    /// Adds a metamodel's slabs; a second set for the same index is refused.
    pub fn with(mut self, index: u16, slabs: Box<dyn Slabs>) -> Result<CompositeSlabs, String> {
        if self.parts.iter().any(|(i, _)| *i == index) {
            return Err(format!("two slab sets for the metamodel index {index}"));
        }
        self.parts.push((index, slabs));
        Ok(self)
    }

    fn part(&self, class: ClassId) -> Option<&dyn Slabs> {
        self.parts.iter().find(|(i, _)| *i == class.metamodel()).map(|(_, s)| s.as_ref())
    }

    fn part_mut(&mut self, class: ClassId) -> Option<&mut Box<dyn Slabs>> {
        self.parts.iter_mut().find(|(i, _)| *i == class.metamodel()).map(|(_, s)| s)
    }
}

impl Slabs for CompositeSlabs {
    fn create(&mut self, class: ClassId) -> Option<u32> {
        self.part_mut(class)?.create(class)
    }
    fn object(&self, class: ClassId, slot: u32) -> Option<&dyn Reflect> {
        self.part(class)?.object(class, slot)
    }
    fn object_mut(&mut self, class: ClassId, slot: u32) -> Option<&mut dyn Reflect> {
        self.part_mut(class)?.object_mut(class, slot)
    }
    fn bytes(&self) -> usize {
        self.parts.iter().map(|(_, s)| s.bytes()).fold(0usize, usize::saturating_add)
    }
}

/// One change with an exact inverse.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Create {
        obj: Obj,
        class: ClassId,
        uuid: Uuid,
    },
    /// The object's data stays; the inverse brings it back.
    Destroy {
        obj: Obj,
        class: ClassId,
        uuid: Uuid,
    },
    Set {
        obj: Obj,
        prop: PropId,
        old: Value,
        new: Value,
    },
    Insert {
        obj: Obj,
        prop: PropId,
        index: u32,
        value: Value,
    },
    Remove {
        obj: Obj,
        prop: PropId,
        index: u32,
        value: Value,
    },
    Move {
        obj: Obj,
        prop: PropId,
        from: u32,
        to: u32,
    },
}

impl Op {
    pub fn inverse(&self) -> Op {
        match self.clone() {
            Op::Create { obj, class, uuid } => Op::Destroy { obj, class, uuid },
            Op::Destroy { obj, class, uuid } => Op::Create { obj, class, uuid },
            Op::Set { obj, prop, old, new } => Op::Set { obj, prop, old: new, new: old },
            Op::Insert { obj, prop, index, value } => Op::Remove { obj, prop, index, value },
            Op::Remove { obj, prop, index, value } => Op::Insert { obj, prop, index, value },
            Op::Move { obj, prop, from, to } => Op::Move { obj, prop, from: to, to: from },
        }
    }

    fn bytes(&self) -> usize {
        let payload = match self {
            Op::Set { old, new, .. } => old.heap_bytes().saturating_add(new.heap_bytes()),
            Op::Insert { value, .. } | Op::Remove { value, .. } => value.heap_bytes(),
            _ => 0,
        };
        payload.saturating_add(48)
    }
}

/// One edit's ops, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delta(pub Vec<Op>);

/// Why a change was refused; nothing changed.
#[derive(Clone, Debug, PartialEq)]
pub enum Invalid {
    NoSuchObject(Obj),
    Dead(Obj),
    NoSuchProperty(PropId),
    /// The object's class does not have the property.
    NotOfClass {
        obj: Obj,
        prop: String,
    },
    Derived(String),
    ReadOnly(String),
    SingleValued(String),
    MultiValued(String),
    WrongKind {
        prop: String,
        expected: &'static str,
        got: &'static str,
    },
    DoesNotConform {
        prop: String,
        class: String,
    },
    UpperBound {
        prop: String,
        max: u32,
    },
    Duplicate(String),
    IndexOutOfRange {
        prop: String,
        index: u32,
        len: usize,
    },
    /// The object would contain itself.
    Cycle,
    /// Destroying would leave a required reference dangling.
    InUse {
        obj: Obj,
        by: Obj,
        prop: String,
    },
    Abstract(String),
    TooManyObjects,
    TooManyOps,
    TooLong {
        prop: String,
        max: usize,
    },
    NotFinite(String),
    /// A raw op does not fit the state (an undo or redo out of order).
    NoFit(String),
    /// A rollback failed, so the store's state is unknown: it refuses every change from now on.
    Corrupt(String),
    /// A notation's own rule refused the change; its words.
    Refused(String),
    /// The generated code refused; its message.
    Internal(String),
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::NoSuchObject(o) => write!(f, "there is no {o}"),
            Invalid::Dead(o) => write!(f, "{o} was destroyed"),
            Invalid::NoSuchProperty(p) => write!(f, "there is no property {}", p.0),
            Invalid::NotOfClass { obj, prop } => write!(f, "{obj} has no property {prop}"),
            Invalid::Derived(p) => write!(f, "{p} is derived and cannot be set"),
            Invalid::ReadOnly(p) => write!(f, "{p} is read-only"),
            Invalid::SingleValued(p) => write!(f, "{p} holds one value, not a list"),
            Invalid::MultiValued(p) => write!(f, "{p} holds a list, not one value"),
            Invalid::WrongKind { prop, expected, got } => write!(f, "{prop} takes {expected}, not {got}"),
            Invalid::DoesNotConform { prop, class } => write!(f, "{prop} does not take {class}"),
            Invalid::UpperBound { prop, max } => write!(f, "{prop} holds at most {max}"),
            Invalid::Duplicate(p) => write!(f, "{p} already holds that value"),
            Invalid::IndexOutOfRange { prop, index, len } => write!(f, "{prop} has {len} values, not an index {index}"),
            Invalid::Cycle => write!(f, "an element cannot contain itself"),
            Invalid::InUse { obj, by, prop } => write!(f, "{obj} is required by {by} through {prop}"),
            Invalid::Abstract(c) => write!(f, "{c} is abstract"),
            Invalid::TooManyObjects => write!(f, "the model holds {MAX_OBJECTS} objects already"),
            Invalid::TooManyOps => write!(f, "the change needs more than {MAX_OPS} steps"),
            Invalid::TooLong { prop, max } => write!(f, "{prop} takes at most {max} characters"),
            Invalid::NotFinite(p) => write!(f, "{p} takes a finite number"),
            Invalid::NoFit(why) => write!(f, "the change does not fit the model's state: {why}"),
            Invalid::Corrupt(why) => write!(f, "the model's state is unknown after a failed rollback ({why}); reopen the project"),
            Invalid::Refused(why) => f.write_str(why),
            Invalid::Internal(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Invalid {}

/// What `validate` reports.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Fewer values than the lower bound.
    Missing { obj: Obj, prop: PropId, lower: u32, found: usize },
    /// Two siblings in one container with the same name, ignoring case.
    DuplicateName { obj: Obj, other: Obj, name: String },
}

/// Every object's class, slot, container and uuid, as a struct of arrays.
#[derive(Default)]
struct Registry {
    class: Vec<ClassId>,
    slot: Vec<u32>,
    container: Vec<Option<(Obj, PropId)>>,
    uuid: Vec<Uuid>,
    alive: Vec<bool>,
}

pub struct Store {
    universe: Arc<Universe>,
    slabs: Box<dyn Slabs>,
    reg: Registry,
    by_uuid: HashMap<Uuid, Obj>,
    /// (target, source, property) for every stored non-composite reference of a live object.
    refs: BTreeSet<(Obj, Obj, PropId)>,
    /// (name property, lower-case name) to the live objects so named.
    names: BTreeMap<(PropId, String), BTreeSet<Obj>>,
    recording: Option<Vec<Op>>,
    ids: Box<dyn FnMut() -> Uuid + Send>,
    live: usize,
    /// Why the state is unknown, once a rollback failed.
    corrupt: Option<String>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Store({} objects, {} live)", self.reg.class.len(), self.live)
    }
}

type R<T> = Result<T, Invalid>;

impl Store {
    /// A store over `universe` with the generated slabs and the host's id source.
    pub fn new(universe: Arc<Universe>, slabs: Box<dyn Slabs>, ids: Box<dyn FnMut() -> Uuid + Send>) -> Store {
        Store {
            universe,
            slabs,
            reg: Registry::default(),
            by_uuid: HashMap::new(),
            refs: BTreeSet::new(),
            names: BTreeMap::new(),
            recording: None,
            ids,
            live: 0,
            corrupt: None,
        }
    }

    pub fn universe(&self) -> &Arc<Universe> {
        &self.universe
    }

    /// Handles given out so far, live or not.
    pub fn len(&self) -> usize {
        self.reg.class.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn live(&self) -> usize {
        self.live
    }

    /// Why the store refuses every change, once a rollback failed.
    pub fn corrupt(&self) -> Option<&str> {
        self.corrupt.as_deref()
    }

    pub fn is_alive(&self, obj: Obj) -> bool {
        self.reg.alive.get(obj.0 as usize).copied().unwrap_or(false)
    }

    pub fn alive(&self) -> impl Iterator<Item = Obj> + '_ {
        (0..self.reg.class.len() as u32).map(Obj).filter(|o| self.is_alive(*o))
    }

    pub fn class_of(&self, obj: Obj) -> R<ClassId> {
        self.check_alive(obj)?;
        self.reg.class.get(obj.0 as usize).copied().ok_or(Invalid::NoSuchObject(obj))
    }

    pub fn uuid(&self, obj: Obj) -> R<Uuid> {
        self.check_alive(obj)?;
        self.reg.uuid.get(obj.0 as usize).copied().ok_or(Invalid::NoSuchObject(obj))
    }

    /// The live object with that uuid.
    pub fn by_uuid(&self, uuid: Uuid) -> Option<Obj> {
        self.by_uuid.get(&uuid).copied().filter(|o| self.is_alive(*o))
    }

    pub fn container(&self, obj: Obj) -> R<Option<(Obj, PropId)>> {
        self.check_alive(obj)?;
        Ok(self.reg.container.get(obj.0 as usize).copied().flatten())
    }

    /// The object as the generated code sees it.
    pub fn reflect(&self, obj: Obj) -> R<&dyn Reflect> {
        self.check_alive(obj)?;
        let (class, slot) = self.place(obj)?;
        self.slabs.object(class, slot).ok_or(Invalid::NoSuchObject(obj))
    }

    fn reflect_mut(&mut self, obj: Obj) -> R<&mut dyn Reflect> {
        let (class, slot) = self.place(obj)?;
        self.slabs.object_mut(class, slot).ok_or(Invalid::NoSuchObject(obj))
    }

    fn place(&self, obj: Obj) -> R<(ClassId, u32)> {
        let i = obj.0 as usize;
        match (self.reg.class.get(i), self.reg.slot.get(i)) {
            (Some(c), Some(s)) => Ok((*c, *s)),
            _ => Err(Invalid::NoSuchObject(obj)),
        }
    }

    fn check_alive(&self, obj: Obj) -> R<()> {
        match self.reg.alive.get(obj.0 as usize) {
            None => Err(Invalid::NoSuchObject(obj)),
            Some(false) => Err(Invalid::Dead(obj)),
            Some(true) => Ok(()),
        }
    }

    fn healthy(&self) -> R<()> {
        match &self.corrupt {
            Some(why) => Err(Invalid::Corrupt(why.clone())),
            None => Ok(()),
        }
    }

    fn pname(&self, prop: PropId) -> String {
        self.universe.prop_name(prop)
    }

    fn property(&self, prop: PropId) -> R<&Property> {
        self.universe.prop(prop).ok_or(Invalid::NoSuchProperty(prop))
    }

    /// The property, which the object's class must have and which must be stored and writable.
    fn writable(&self, obj: Obj, prop: PropId) -> R<Property> {
        let p = self.property(prop)?;
        if !self.universe.has(self.class_of(obj)?, prop) {
            return Err(Invalid::NotOfClass { obj, prop: self.pname(prop) });
        }
        if p.is_derived || p.is_derived_union {
            return Err(Invalid::Derived(self.pname(prop)));
        }
        if p.is_read_only {
            return Err(Invalid::ReadOnly(self.pname(prop)));
        }
        Ok(p.clone())
    }

    /// Whether `value` may be a value of `p`: its kind, its length, and for a reference a live
    /// object of a conforming class.
    fn check_value(&self, prop: PropId, p: &Property, value: &Value) -> R<()> {
        let name = || self.pname(prop);
        match value {
            Value::Null => Ok(()),
            Value::Ref(o) => {
                if !matches!(p.ty, TypeRef::Class(_) | TypeRef::Any) {
                    return Err(Invalid::WrongKind { prop: name(), expected: "a data value", got: value.kind() });
                }
                let class = self.class_of(*o)?;
                if !self.universe.conforms(class, p.ty) {
                    return Err(Invalid::DoesNotConform { prop: name(), class: self.universe.class_name(class) });
                }
                Ok(())
            }
            Value::Real(r) if !r.is_finite() => Err(Invalid::NotFinite(name())),
            Value::Str(s) if s.chars().count() > MAX_STRING => Err(Invalid::TooLong { prop: name(), max: MAX_STRING }),
            _ => {
                if self.universe.value_fits(p.ty, value) {
                    Ok(())
                } else {
                    let expected = match p.ty {
                        TypeRef::Class(_) | TypeRef::Any => "a reference",
                        TypeRef::Data(d) => match self.universe.data_type(d).map(|t| &t.kind) {
                            Some(DataKind::Boolean) => "a boolean",
                            Some(DataKind::Integer) => "an integer",
                            Some(DataKind::Real) => "a real",
                            Some(DataKind::String) => "a string",
                            Some(DataKind::UnlimitedNatural) => "an unlimited natural",
                            Some(DataKind::Enumeration(_)) => "a literal of its enumeration",
                            None => "a data value",
                        },
                    };
                    Err(Invalid::WrongKind { prop: name(), expected, got: value.kind() })
                }
            }
        }
    }

    // Reading.

    /// A single-valued property's value, computed for a derived one when the generated code can.
    pub fn get(&self, obj: Obj, prop: PropId) -> R<Value> {
        let p = self.property(prop)?;
        if !self.universe.has(self.class_of(obj)?, prop) {
            return Err(Invalid::NotOfClass { obj, prop: self.pname(prop) });
        }
        if p.is_many() {
            return Err(Invalid::MultiValued(self.pname(prop)));
        }
        match self.reflect(obj)?.get(prop) {
            Some(v) => Ok(v),
            None if p.is_stored() => Ok(Value::Null),
            None => Err(Invalid::Derived(self.pname(prop))),
        }
    }

    /// A multi-valued property's values; a derived union is the values of the stored properties
    /// that subset it, in property order.
    pub fn list(&self, obj: Obj, prop: PropId) -> R<Vec<Value>> {
        let p = self.property(prop)?;
        let class = self.class_of(obj)?;
        if !self.universe.has(class, prop) {
            return Err(Invalid::NotOfClass { obj, prop: self.pname(prop) });
        }
        if !p.is_many() {
            return Err(Invalid::SingleValued(self.pname(prop)));
        }
        let r = self.reflect(obj)?;
        if let Some(l) = r.list(prop) {
            return Ok(l.to_vec());
        }
        if p.is_derived_union {
            let members = self.universe.info(class).and_then(|i| i.unions.get(&prop)).cloned().unwrap_or_default();
            let mut out = Vec::new();
            for m in members {
                let mp = self.property(m)?;
                if mp.is_many() {
                    out.extend(r.list(m).unwrap_or(&[]).iter().cloned());
                } else if let Some(v) = r.get(m).filter(|v| !v.is_null()) {
                    out.push(v);
                }
            }
            return Ok(out);
        }
        if p.is_stored() { Ok(Vec::new()) } else { Err(Invalid::Derived(self.pname(prop))) }
    }

    /// The stored list of `prop` on a live `obj`, borrowed; empty when there is none.
    fn slice(&self, obj: Obj, prop: PropId) -> R<&[Value]> {
        Ok(self.reflect(obj)?.list(prop).unwrap_or(&[]))
    }

    fn list_len(&self, obj: Obj, prop: PropId) -> R<usize> {
        Ok(self.slice(obj, prop)?.len())
    }

    fn position(&self, obj: Obj, prop: PropId, value: &Value) -> R<Option<usize>> {
        Ok(self.slice(obj, prop)?.iter().position(|v| v == value))
    }

    fn single(&self, obj: Obj, prop: PropId) -> R<Value> {
        Ok(self.reflect(obj)?.get(prop).unwrap_or(Value::Null))
    }

    /// Every object `obj` contains, through its composite properties in property order: the
    /// derived union `ownedElement`.
    pub fn owned_elements(&self, obj: Obj) -> R<Vec<Obj>> {
        let class = self.class_of(obj)?;
        let r = self.reflect(obj)?;
        let mut out = Vec::new();
        if let Some(info) = self.universe.info(class) {
            for p in &info.stored {
                let Some(prop) = self.universe.prop(*p) else { continue };
                if !prop.is_composite {
                    continue;
                }
                if prop.is_many() {
                    out.extend(r.list(*p).unwrap_or(&[]).iter().filter_map(Value::as_ref));
                } else if let Some(o) = r.get(*p).and_then(|v| v.as_ref()) {
                    out.push(o);
                }
            }
        }
        Ok(out)
    }

    /// Who refers to `obj` through a stored non-composite property: (source, property).
    pub fn referrers(&self, obj: Obj) -> Vec<(Obj, PropId)> {
        self.refs.range((obj, Obj(0), PropId(0))..=(obj, Obj(u32::MAX), PropId(u32::MAX))).map(|(_, s, p)| (*s, *p)).collect()
    }

    /// The live objects whose name property `prop` is `name`, ignoring case, in handle order.
    pub fn by_name(&self, prop: PropId, name: &str) -> Vec<Obj> {
        self.names.get(&(prop, name.to_lowercase())).map(|v| v.iter().copied().collect()).unwrap_or_default()
    }

    /// The live objects whose name under `prop` starts with `prefix`, ignoring case.
    pub fn by_prefix(&self, prop: PropId, prefix: &str) -> Vec<Obj> {
        let p = prefix.to_lowercase();
        self.names.range((prop, p.clone())..).take_while(|((q, n), _)| *q == prop && n.starts_with(&p)).flat_map(|(_, v)| v.iter().copied()).collect()
    }

    /// Whether `ancestor` contains `obj`, at any depth.
    pub fn contains(&self, ancestor: Obj, obj: Obj) -> bool {
        let mut cur = obj;
        for _ in 0..self.reg.class.len() + 1 {
            match self.reg.container.get(cur.0 as usize).copied().flatten() {
                Some((c, _)) if c == ancestor => return true,
                Some((c, _)) => cur = c,
                None => return false,
            }
        }
        false
    }

    /// An estimate of the bytes the store holds.
    pub fn bytes(&self) -> usize {
        let names = self
            .names
            .iter()
            .map(|((_, n), v)| n.len().saturating_add(32).saturating_add(v.len().saturating_mul(4)))
            .fold(0usize, usize::saturating_add);
        self.slabs
            .bytes()
            .saturating_add(self.reg.class.len().saturating_mul(41))
            .saturating_add(self.by_uuid.len().saturating_mul(40))
            .saturating_add(self.refs.len().saturating_mul(16))
            .saturating_add(names)
    }

    // The semantic level. Each operation is atomic: on a refusal the ops it performed so far are
    // taken back, so the store is as it was.

    /// Runs `f`, several changes as one: a failure anywhere in it leaves nothing of it behind, inside
    /// or outside an engine recording (inside one, the ops stay part of the engine's delta).
    pub fn transaction<T>(&mut self, f: impl FnOnce(&mut Store) -> R<T>) -> R<T> {
        self.atomic(f)
    }

    /// Runs `f` so that a failure leaves nothing of it behind.
    fn atomic<T>(&mut self, f: impl FnOnce(&mut Store) -> R<T>) -> R<T> {
        self.healthy()?;
        let temporary = self.recording.is_none();
        if temporary {
            self.recording = Some(Vec::new());
        }
        let mark = self.recording.as_ref().map_or(0, Vec::len);
        let result = f(self);
        if result.is_err() {
            let undone: Vec<Op> = self.recording.as_mut().map(|ops| ops.split_off(mark)).unwrap_or_default();
            for op in undone.iter().rev() {
                if let Err(e) = self.apply_raw(&op.inverse()) {
                    let why = format!("taking back a refused change failed: {e}");
                    log::error!("{why}");
                    self.corrupt = Some(why.clone());
                    if temporary {
                        self.recording = None;
                    }
                    return Err(Invalid::Corrupt(why));
                }
            }
        }
        if temporary {
            self.recording = None;
        }
        result
    }

    /// A new object of `class`, with no container yet.
    pub fn create(&mut self, class: ClassId) -> R<Obj> {
        let c = self.universe.class(class).ok_or_else(|| Invalid::Internal(format!("there is no class {}", class.0)))?;
        if c.is_abstract {
            return Err(Invalid::Abstract(c.name.to_string()));
        }
        if self.live >= MAX_OBJECTS || self.reg.class.len() >= u32::MAX as usize - 1 {
            return Err(Invalid::TooManyObjects);
        }
        self.atomic(|st| {
            let uuid = (st.ids)();
            if st.by_uuid.contains_key(&uuid) {
                return Err(Invalid::Internal("the id source gave an id twice".into()));
            }
            let obj = Obj(st.reg.class.len() as u32);
            st.perform(Op::Create { obj, class, uuid })?;
            Ok(obj)
        })
    }

    /// Sets a single-valued property, keeping containment and the opposite consistent.
    pub fn set(&mut self, obj: Obj, prop: PropId, value: Value) -> R<()> {
        let p = self.writable(obj, prop)?;
        if p.is_many() {
            return Err(Invalid::MultiValued(self.pname(prop)));
        }
        self.check_value(prop, &p, &value)?;
        let old = self.single(obj, prop)?;
        if old == value {
            return Ok(());
        }
        if p.is_composite && matches!(value, Value::Ref(child) if child == obj || self.contains(child, obj)) {
            return Err(Invalid::Cycle);
        }
        if let (Some(o), Value::Ref(y)) = (p.opposite, &value) {
            self.room_in_opposite(*y, o, obj)?;
        }
        self.atomic(|st| {
            if let (true, Value::Ref(child)) = (p.is_composite, &value) {
                st.unlink(*child)?;
            }
            if let Some(o) = p.opposite {
                if let Value::Ref(x) = old {
                    st.detach(x, o, obj)?;
                }
                if let Value::Ref(y) = value {
                    st.attach(y, o, obj, prop)?;
                }
            }
            st.perform(Op::Set { obj, prop, old, new: value })
        })
    }

    /// Appends to a multi-valued property.
    pub fn push(&mut self, obj: Obj, prop: PropId, value: Value) -> R<u32> {
        let len = self.list_len(obj, prop)? as u32;
        self.insert(obj, prop, len, value)?;
        Ok(len)
    }

    /// Inserts into a multi-valued property at `index`, keeping uniqueness, the upper bound,
    /// containment, the opposite and the subsetted properties consistent.
    pub fn insert(&mut self, obj: Obj, prop: PropId, index: u32, value: Value) -> R<()> {
        let p = self.writable(obj, prop)?;
        if !p.is_many() {
            return Err(Invalid::SingleValued(self.pname(prop)));
        }
        if value.is_null() {
            return Err(Invalid::WrongKind { prop: self.pname(prop), expected: "a value", got: "nothing" });
        }
        self.check_value(prop, &p, &value)?;
        let len = self.list_len(obj, prop)?;
        if index as usize > len {
            return Err(Invalid::IndexOutOfRange { prop: self.pname(prop), index, len });
        }
        self.room_for(prop, &p, len)?;
        if p.is_unique && self.position(obj, prop, &value)?.is_some() {
            return Err(Invalid::Duplicate(self.pname(prop)));
        }
        if p.is_composite && matches!(value, Value::Ref(child) if child == obj || self.contains(child, obj)) {
            return Err(Invalid::Cycle);
        }
        if let (Some(o), Value::Ref(y)) = (p.opposite, &value) {
            self.room_in_opposite(*y, o, obj)?;
        }
        for q in self.subsetted_of(obj, prop)? {
            if self.position(obj, q, &value)?.is_none() {
                let qp = self.property(q)?.clone();
                let l = self.list_len(obj, q)?;
                self.room_for(q, &qp, l)?;
            }
        }
        self.atomic(|st| {
            if let (true, Value::Ref(child)) = (p.is_composite, &value) {
                st.unlink(*child)?;
            }
            if let (Some(o), Value::Ref(y)) = (p.opposite, &value) {
                st.attach(*y, o, obj, prop)?;
            }
            st.append_to_subsetted(obj, prop, &value)?;
            st.perform(Op::Insert { obj, prop, index, value })
        })
    }

    /// The upper bound and the list cap, for one more value in `prop`.
    fn room_for(&self, prop: PropId, p: &Property, len: usize) -> R<()> {
        if let Some(max) = p.upper
            && len >= max as usize
        {
            return Err(Invalid::UpperBound { prop: self.pname(prop), max });
        }
        if len >= MAX_LIST {
            return Err(Invalid::UpperBound { prop: self.pname(prop), max: MAX_LIST as u32 });
        }
        Ok(())
    }

    /// Whether `target.opposite` can take `source` (a bounded opposite may be full).
    fn room_in_opposite(&self, target: Obj, opposite: PropId, source: Obj) -> R<()> {
        let o = self.property(opposite)?;
        if !o.is_many() || !self.universe.has(self.class_of(target)?, opposite) {
            return Ok(());
        }
        if self.position(target, opposite, &Value::Ref(source))?.is_some() {
            return Ok(());
        }
        let len = self.list_len(target, opposite)?;
        self.room_for(opposite, o, len)
    }

    /// The stored properties `prop` subsets on `obj`'s class.
    fn subsetted_of(&self, obj: Obj, prop: PropId) -> R<Vec<PropId>> {
        let class = self.class_of(obj)?;
        Ok(self.universe.info(class).and_then(|i| i.subsetted.get(&prop)).cloned().unwrap_or_default())
    }

    /// Puts `value` into the stored properties `prop` subsets, where it is not yet.
    fn append_to_subsetted(&mut self, obj: Obj, prop: PropId, value: &Value) -> R<()> {
        for q in self.subsetted_of(obj, prop)? {
            if self.position(obj, q, value)?.is_none() {
                let at = self.list_len(obj, q)? as u32;
                self.perform(Op::Insert { obj, prop: q, index: at, value: value.clone() })?;
            }
        }
        Ok(())
    }

    /// Takes `value` out of the stored properties `prop` subsets, unless another subsetting
    /// property of the object still holds it.
    fn drop_from_subsetted(&mut self, obj: Obj, prop: PropId, value: &Value) -> R<()> {
        let class = self.class_of(obj)?;
        let Some(info) = self.universe.info(class).cloned() else { return Ok(()) };
        let Some(subsetted) = info.subsetted.get(&prop) else { return Ok(()) };
        for q in subsetted {
            let still_held = info.subsetted.iter().any(|(other, qs)| *other != prop && qs.contains(q) && self.holds(obj, *other, value));
            if !still_held && let Some(i) = self.position(obj, *q, value)? {
                self.perform(Op::Remove { obj, prop: *q, index: i as u32, value: value.clone() })?;
            }
        }
        Ok(())
    }

    fn holds(&self, obj: Obj, prop: PropId, value: &Value) -> bool {
        self.reflect(obj).ok().is_some_and(|r| r.list(prop).is_some_and(|l| l.contains(value)) || r.get(prop).as_ref() == Some(value))
    }

    /// Removes the value at `index` of a multi-valued property, keeping the opposite and the
    /// subsetted properties consistent; a contained value loses its container.
    pub fn remove_at(&mut self, obj: Obj, prop: PropId, index: u32) -> R<Value> {
        let p = self.writable(obj, prop)?;
        if !p.is_many() {
            return Err(Invalid::SingleValued(self.pname(prop)));
        }
        let len = self.list_len(obj, prop)?;
        let value = self.slice(obj, prop)?.get(index as usize).cloned().ok_or(Invalid::IndexOutOfRange { prop: self.pname(prop), index, len })?;
        self.atomic(|st| {
            if let (Some(o), Value::Ref(x)) = (p.opposite, &value) {
                st.detach(*x, o, obj)?;
            }
            st.drop_from_subsetted(obj, prop, &value)?;
            st.perform(Op::Remove { obj, prop, index, value: value.clone() })?;
            Ok(value)
        })
    }

    /// Removes `value` from a multi-valued property, wherever it is.
    pub fn remove_value(&mut self, obj: Obj, prop: PropId, value: &Value) -> R<bool> {
        match self.position(obj, prop, value)? {
            Some(i) => {
                self.remove_at(obj, prop, i as u32)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Moves a value within an ordered property.
    pub fn move_item(&mut self, obj: Obj, prop: PropId, from: u32, to: u32) -> R<()> {
        let p = self.writable(obj, prop)?;
        if !p.is_many() {
            return Err(Invalid::SingleValued(self.pname(prop)));
        }
        let len = self.list_len(obj, prop)?;
        for i in [from, to] {
            if i as usize >= len {
                return Err(Invalid::IndexOutOfRange { prop: self.pname(prop), index: i, len });
            }
        }
        if from == to {
            return Ok(());
        }
        self.atomic(|st| st.perform(Op::Move { obj, prop, from, to }))
    }

    /// Destroys `obj` with everything it contains: references to any of them from outside are
    /// dropped where their property allows it and refused where it is required; opposites of the
    /// dying objects' own references are repaired; the subtree is unlinked and destroyed, leaves
    /// first. Nothing changes when any part is refused.
    pub fn destroy(&mut self, obj: Obj) -> R<usize> {
        self.check_alive(obj)?;
        let dying = self.subtree(obj)?;
        let dead: BTreeSet<Obj> = dying.iter().copied().collect();
        // What points in from outside, grouped by the list or slot that holds it; refused before
        // anything changes when a required reference would dangle.
        let mut lists: BTreeMap<(Obj, PropId), BTreeSet<Obj>> = BTreeMap::new();
        let mut slots: Vec<(Obj, PropId, Obj)> = Vec::new();
        for &d in &dying {
            for (src, prop) in self.referrers(d) {
                if dead.contains(&src) {
                    continue;
                }
                let p = self.property(prop)?;
                if p.is_many() {
                    lists.entry((src, prop)).or_default().insert(d);
                } else if p.lower == 0 {
                    slots.push((src, prop, d));
                } else {
                    return Err(Invalid::InUse { obj: d, by: src, prop: self.pname(prop) });
                }
            }
        }
        if dying.len().saturating_mul(2).saturating_add(slots.len()).saturating_add(lists.len()) > MAX_OPS {
            return Err(Invalid::TooManyOps);
        }
        self.atomic(|st| {
            for (src, prop, d) in slots {
                st.perform(Op::Set { obj: src, prop, old: Value::Ref(d), new: Value::Null })?;
            }
            for ((src, prop), gone) in lists {
                // One pass over the list, from the back, so earlier indices stay right.
                let doomed: Vec<(usize, Value)> = st
                    .slice(src, prop)?
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.as_ref().is_some_and(|o| gone.contains(&o)))
                    .map(|(i, v)| (i, v.clone()))
                    .collect();
                for (i, v) in doomed.into_iter().rev() {
                    st.drop_from_subsetted(src, prop, &v)?;
                    st.perform(Op::Remove { obj: src, prop, index: i as u32, value: v })?;
                }
            }
            // The dying objects' own references with opposites: the other side lets go.
            for &d in &dying {
                let class = st.class_of(d)?;
                let stored = st.universe.info(class).map(|i| i.stored.clone()).unwrap_or_default();
                for prop in stored {
                    let Some(p) = st.universe.prop(prop).cloned() else { continue };
                    let Some(o) = p.opposite else { continue };
                    if p.is_composite {
                        continue;
                    }
                    let values: Vec<Obj> = if p.is_many() {
                        st.slice(d, prop)?.iter().filter_map(Value::as_ref).collect()
                    } else {
                        st.single(d, prop)?.as_ref().into_iter().collect()
                    };
                    for t in values {
                        if !dead.contains(&t) && st.is_alive(t) {
                            st.detach(t, o, d)?;
                        }
                    }
                }
            }
            st.unlink(obj)?;
            for &d in dying.iter().rev() {
                let (class, _) = st.place(d)?;
                let uuid = st.uuid(d)?;
                st.perform(Op::Destroy { obj: d, class, uuid })?;
            }
            Ok(dying.len())
        })
    }

    /// `obj` and everything it contains, parents before children, without recursion.
    fn subtree(&self, obj: Obj) -> R<Vec<Obj>> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![obj];
        while let Some(o) = stack.pop() {
            if !seen.insert(o) || !self.is_alive(o) {
                continue;
            }
            out.push(o);
            if out.len() > MAX_OPS {
                return Err(Invalid::TooManyOps);
            }
            let children = self.owned_elements(o)?;
            stack.extend(children.into_iter().rev());
        }
        Ok(out)
    }

    /// Takes `child` out of its container, whatever property holds it.
    fn unlink(&mut self, child: Obj) -> R<()> {
        let Some((c, cp)) = self.container(child)? else { return Ok(()) };
        if !self.is_alive(c) {
            return Ok(());
        }
        let p = self.property(cp)?.clone();
        if p.is_many() {
            if let Some(i) = self.position(c, cp, &Value::Ref(child))? {
                self.drop_from_subsetted(c, cp, &Value::Ref(child))?;
                self.perform(Op::Remove { obj: c, prop: cp, index: i as u32, value: Value::Ref(child) })?;
            }
        } else if self.single(c, cp)? == Value::Ref(child) {
            self.perform(Op::Set { obj: c, prop: cp, old: Value::Ref(child), new: Value::Null })?;
        }
        Ok(())
    }

    /// Makes `target.opposite` hold `source` (the other end of a reference just made).
    fn attach(&mut self, target: Obj, opposite: PropId, source: Obj, prop: PropId) -> R<()> {
        let o = self.property(opposite)?.clone();
        if !self.universe.has(self.class_of(target)?, opposite) {
            return Ok(());
        }
        if o.is_many() {
            if self.position(target, opposite, &Value::Ref(source))?.is_some() {
                return Ok(());
            }
            let len = self.list_len(target, opposite)?;
            self.room_for(opposite, &o, len)?;
            self.append_to_subsetted(target, opposite, &Value::Ref(source))?;
            self.perform(Op::Insert { obj: target, prop: opposite, index: len as u32, value: Value::Ref(source) })
        } else {
            let old = self.single(target, opposite)?;
            if old == Value::Ref(source) {
                return Ok(());
            }
            // The previous holder of this end loses it (MOF 12.5.6).
            if let Value::Ref(z) = old
                && z != source
                && self.is_alive(z)
                && self.universe.has(self.class_of(z)?, prop)
            {
                let zp = self.property(prop)?.clone();
                if zp.is_many() {
                    if let Some(i) = self.position(z, prop, &Value::Ref(target))? {
                        self.drop_from_subsetted(z, prop, &Value::Ref(target))?;
                        self.perform(Op::Remove { obj: z, prop, index: i as u32, value: Value::Ref(target) })?;
                    }
                } else if self.single(z, prop)? == Value::Ref(target) {
                    self.perform(Op::Set { obj: z, prop, old: Value::Ref(target), new: Value::Null })?;
                }
            }
            self.perform(Op::Set { obj: target, prop: opposite, old, new: Value::Ref(source) })
        }
    }

    /// Makes `target.opposite` no longer hold `source`.
    fn detach(&mut self, target: Obj, opposite: PropId, source: Obj) -> R<()> {
        if !self.is_alive(target) {
            return Ok(());
        }
        let o = self.property(opposite)?.clone();
        if !self.universe.has(self.class_of(target)?, opposite) {
            return Ok(());
        }
        if o.is_many() {
            if let Some(i) = self.position(target, opposite, &Value::Ref(source))? {
                self.drop_from_subsetted(target, opposite, &Value::Ref(source))?;
                self.perform(Op::Remove { obj: target, prop: opposite, index: i as u32, value: Value::Ref(source) })?;
            }
            Ok(())
        } else {
            let old = self.single(target, opposite)?;
            if old == Value::Ref(source) {
                self.perform(Op::Set { obj: target, prop: opposite, old, new: Value::Null })?;
            }
            Ok(())
        }
    }

    /// Lower bounds and sibling names of one object.
    pub fn validate(&self, obj: Obj) -> R<Vec<Problem>> {
        let class = self.class_of(obj)?;
        let r = self.reflect(obj)?;
        let mut out = Vec::new();
        let Some(info) = self.universe.info(class) else { return Ok(out) };
        for p in &info.stored {
            let Some(prop) = self.universe.prop(*p) else { continue };
            if prop.lower == 0 {
                continue;
            }
            let found = if prop.is_many() { r.list(*p).map_or(0, <[Value]>::len) } else { usize::from(r.get(*p).is_some_and(|v| !v.is_null())) };
            if found < prop.lower as usize {
                out.push(Problem::Missing { obj, prop: *p, lower: prop.lower, found });
            }
        }
        if let (Some(np), Some((c, _))) = (info.name_prop, self.container(obj)?)
            && let Some(Value::Str(name)) = r.get(np)
        {
            for other in self.by_name(np, &name) {
                if other != obj && self.container(other)?.map(|(o, _)| o) == Some(c) {
                    out.push(Problem::DuplicateName { obj, other, name: name.to_string() });
                }
            }
        }
        Ok(out)
    }

    /// `validate` over every live object.
    pub fn validate_all(&self) -> Vec<Problem> {
        self.alive().flat_map(|o| self.validate(o).unwrap_or_default()).collect()
    }

    // The raw level.

    /// Applies one op, recording it.
    fn perform(&mut self, op: Op) -> R<()> {
        self.healthy()?;
        if let Some(ops) = &self.recording
            && ops.len() >= MAX_OPS
        {
            return Err(Invalid::TooManyOps);
        }
        self.apply_raw(&op)?;
        if let Some(ops) = &mut self.recording {
            ops.push(op);
        }
        Ok(())
    }

    /// Whether a raw value is one the store ever holds: finite, and not over the string cap.
    fn sound(&self, prop: PropId, value: &Value) -> R<()> {
        match value {
            Value::Real(r) if !r.is_finite() => Err(Invalid::NotFinite(self.pname(prop))),
            Value::Str(s) if s.chars().count() > MAX_STRING => Err(Invalid::TooLong { prop: self.pname(prop), max: MAX_STRING }),
            _ => Ok(()),
        }
    }

    /// Applies one op if it fits the state, and keeps the indexes exact.
    fn apply_raw(&mut self, op: &Op) -> R<()> {
        match op {
            Op::Create { obj, class, uuid } => {
                let n = self.reg.class.len();
                let i = obj.0 as usize;
                if i == n {
                    let slot = self.slabs.create(*class).ok_or_else(|| Invalid::Internal(format!("no slab for class {}", class.0)))?;
                    self.reg.class.push(*class);
                    self.reg.slot.push(slot);
                    self.reg.container.push(None);
                    self.reg.uuid.push(*uuid);
                    self.reg.alive.push(true);
                    self.by_uuid.insert(*uuid, *obj);
                } else {
                    let fits = i < n && !self.reg.alive[i] && self.reg.class[i] == *class && self.reg.uuid[i] == *uuid;
                    if !fits {
                        return Err(Invalid::NoFit(format!("{obj} cannot be created again")));
                    }
                    self.reg.alive[i] = true;
                }
                self.live += 1;
                self.index_object(*obj, true);
                Ok(())
            }
            Op::Destroy { obj, class, uuid } => {
                let i = obj.0 as usize;
                let fits = self.is_alive(*obj) && self.reg.class.get(i) == Some(class) && self.reg.uuid.get(i) == Some(uuid);
                if !fits {
                    return Err(Invalid::NoFit(format!("{obj} is not there to destroy")));
                }
                self.index_object(*obj, false);
                self.reg.alive[i] = false;
                self.live -= 1;
                Ok(())
            }
            Op::Set { obj, prop, old, new } => {
                self.check_alive(*obj)?;
                let p = self.property(*prop)?.clone();
                self.sound(*prop, new)?;
                let current = self.single(*obj, *prop)?;
                if current != *old {
                    return Err(Invalid::NoFit(format!("{} of {obj} is {current}, not {old}", self.pname(*prop))));
                }
                self.reflect_mut(*obj)?.set(*prop, new.clone()).map_err(Invalid::Internal)?;
                self.index_value(*obj, &p, *prop, old, false);
                self.index_value(*obj, &p, *prop, new, true);
                Ok(())
            }
            Op::Insert { obj, prop, index, value } => {
                self.check_alive(*obj)?;
                let p = self.property(*prop)?.clone();
                self.sound(*prop, value)?;
                let name = self.pname(*prop);
                let l = self.reflect_mut(*obj)?.list_mut(*prop).ok_or_else(|| Invalid::NoFit(format!("{name} is not a list")))?;
                if *index as usize > l.len() || l.len() >= MAX_LIST {
                    return Err(Invalid::NoFit(format!("{name} has no place {index}")));
                }
                l.insert(*index as usize, value.clone());
                self.index_value(*obj, &p, *prop, value, true);
                Ok(())
            }
            Op::Remove { obj, prop, index, value } => {
                self.check_alive(*obj)?;
                let p = self.property(*prop)?.clone();
                let name = self.pname(*prop);
                let l = self.reflect_mut(*obj)?.list_mut(*prop).ok_or_else(|| Invalid::NoFit(format!("{name} is not a list")))?;
                if l.get(*index as usize) != Some(value) {
                    return Err(Invalid::NoFit(format!("{name} does not hold {value} at {index}")));
                }
                l.remove(*index as usize);
                self.index_value(*obj, &p, *prop, value, false);
                Ok(())
            }
            Op::Move { obj, prop, from, to } => {
                self.check_alive(*obj)?;
                let name = self.pname(*prop);
                let l = self.reflect_mut(*obj)?.list_mut(*prop).ok_or_else(|| Invalid::NoFit(format!("{name} is not a list")))?;
                if *from as usize >= l.len() || *to as usize >= l.len() {
                    return Err(Invalid::NoFit(format!("{name} has no places {from} and {to}")));
                }
                let v = l.remove(*from as usize);
                l.insert(*to as usize, v);
                Ok(())
            }
        }
    }

    /// Keeps the indexes for one value of `prop` on `obj` arriving (`add`) or leaving.
    fn index_value(&mut self, obj: Obj, p: &Property, prop: PropId, value: &Value, add: bool) {
        match value {
            Value::Ref(target) if p.is_composite => {
                if let Some(c) = self.reg.container.get_mut(target.0 as usize) {
                    if add {
                        *c = Some((obj, prop));
                    } else if *c == Some((obj, prop)) {
                        *c = None;
                    }
                }
            }
            Value::Ref(target) => {
                if add {
                    self.refs.insert((*target, obj, prop));
                } else {
                    self.refs.remove(&(*target, obj, prop));
                }
            }
            Value::Str(s) if p.is_name => {
                let key = (prop, s.to_lowercase());
                if add {
                    self.names.entry(key).or_default().insert(obj);
                } else if let Some(v) = self.names.get_mut(&key) {
                    v.remove(&obj);
                    if v.is_empty() {
                        self.names.remove(&key);
                    }
                }
            }
            _ => {}
        }
    }

    /// Puts a live object's outgoing references and name into the indexes, or takes them out.
    fn index_object(&mut self, obj: Obj, add: bool) {
        let Some(class) = self.reg.class.get(obj.0 as usize).copied() else { return };
        let Some(info) = self.universe.info(class).cloned() else { return };
        for prop in info.stored {
            let Some(p) = self.universe.prop(prop).cloned() else { continue };
            if p.is_composite {
                continue;
            }
            let values: Vec<Value> = match self.reflect(obj) {
                Ok(r) if p.is_many() => r.list(prop).map(<[Value]>::to_vec).unwrap_or_default(),
                Ok(r) => r.get(prop).into_iter().collect(),
                Err(_) => Vec::new(),
            };
            for v in &values {
                self.index_value(obj, &p, prop, v, add);
            }
        }
    }

    /// Applies a whole delta outside a recording, all or nothing. If taking back a half-applied
    /// delta fails too, the store's state is unknown and it refuses every change from then on.
    pub fn apply_delta(&mut self, delta: &Delta) -> R<()> {
        self.healthy()?;
        for (k, op) in delta.0.iter().enumerate() {
            if let Err(e) = self.apply_raw(op) {
                for done in delta.0.get(..k).unwrap_or(&[]).iter().rev() {
                    if let Err(back) = self.apply_raw(&done.inverse()) {
                        let why = format!("taking back a half-applied change failed: {back} (after: {e})");
                        log::error!("{why}");
                        self.corrupt = Some(why.clone());
                        return Err(Invalid::Corrupt(why));
                    }
                }
                return Err(e);
            }
        }
        Ok(())
    }
}

/// The ops with pairs that cancel removed: a Set back to its old value, a Create then Destroy,
/// an Insert then its Remove, a Move and back.
fn normalize(ops: &[Op]) -> Vec<Op> {
    let mut out: Vec<Op> = Vec::with_capacity(ops.len());
    for op in ops {
        let cancels = match (out.last(), op) {
            (Some(Op::Set { obj: a, prop: p, old, .. }), Op::Set { obj: b, prop: q, new, .. }) => a == b && p == q && old == new,
            (Some(Op::Create { obj: a, .. }), Op::Destroy { obj: b, .. }) => a == b,
            (Some(Op::Insert { obj: a, prop: p, index: i, value: v }), Op::Remove { obj: b, prop: q, index: j, value: w }) => {
                a == b && p == q && i == j && v == w
            }
            (Some(Op::Move { obj: a, prop: p, from, to }), Op::Move { obj: b, prop: q, from: f2, to: t2 }) => {
                a == b && p == q && from == t2 && to == f2
            }
            _ => false,
        };
        if cancels {
            out.pop();
        } else if let (Some(Op::Set { obj: a, prop: p, .. }), Op::Set { obj: b, prop: q, new, .. }) = (out.last(), op) {
            if a == b && p == q {
                // Two sets of the same slot fold into one.
                if let Some(Op::Set { new: last_new, .. }) = out.last_mut() {
                    *last_new = new.clone();
                }
            } else {
                out.push(op.clone());
            }
        } else {
            out.push(op.clone());
        }
    }
    out
}

impl archforge_engine::Document for Store {
    type Delta = Delta;

    fn begin(&mut self) {
        self.recording = Some(Vec::new());
    }

    fn end(&mut self) -> Option<Delta> {
        let ops = self.recording.take()?;
        if ops.is_empty() { None } else { Some(Delta(ops)) }
    }

    fn apply(&mut self, delta: &Delta) -> Result<(), String> {
        let was = self.recording.take();
        let r = self.apply_delta(delta).map_err(|e| e.to_string());
        self.recording = was;
        r
    }

    fn invert(delta: &Delta) -> Delta {
        Delta(delta.0.iter().rev().map(Op::inverse).collect())
    }

    fn compose(mut first: Delta, second: Delta) -> Delta {
        first.0.extend(second.0);
        first
    }

    fn is_empty(delta: &Delta) -> bool {
        normalize(&delta.0).is_empty()
    }

    fn weight(delta: &Delta) -> usize {
        delta.0.iter().map(Op::bytes).fold(0usize, usize::saturating_add)
    }
}
