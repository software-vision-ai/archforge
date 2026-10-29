//! A toy metamodel in exactly the shape the generator will emit for a real one: a `Metamodel`
//! description built with the `Builder`, one struct per concrete metaclass with every inherited
//! property flattened into a field, a `Reflect` for each, and the `Slabs`. Twenty metaclasses cut
//! from UML's kernel: packages, classes with attributes, operations and generalizations,
//! associations with member ends, multiplicities, comments, literals, primitive types and
//! enumerations. Tests only (the `testing` feature).

use std::sync::{Arc, LazyLock};

use crate::id::Uuid;
use crate::meta::{Builder, ClassId, DataId, DataKind, MetaError, Metamodel, PropId, PropertySpec, TypeRef, Universe};
use crate::store::{Obj, Reflect, Slabs, Store};
use crate::value::Value;

pub const INDEX: u16 = 1;
pub const URI: &str = "archforge:toy";

const fn c(local: u16) -> ClassId {
    ClassId::new(INDEX, local)
}
const fn p(local: u16) -> PropId {
    PropId::new(INDEX, local)
}
const fn d(local: u16) -> DataId {
    DataId::new(INDEX, local)
}

// Data types.
pub const BOOLEAN: DataId = d(0);
pub const INTEGER: DataId = d(1);
pub const REAL: DataId = d(2);
pub const STRING: DataId = d(3);
pub const UNLIMITED: DataId = d(4);
pub const AGGREGATION_KIND: DataId = d(5);
pub const DIRECTION_KIND: DataId = d(6);

// Classes, supers before subclasses.
pub const ELEMENT: ClassId = c(0);
pub const COMMENT: ClassId = c(1);
pub const NAMED_ELEMENT: ClassId = c(2);
pub const NAMESPACE: ClassId = c(3);
pub const PACKAGEABLE_ELEMENT: ClassId = c(4);
pub const TYPE: ClassId = c(5);
pub const PACKAGE: ClassId = c(6);
pub const CLASS: ClassId = c(7);
pub const TYPED_ELEMENT: ClassId = c(8);
pub const MULTIPLICITY_ELEMENT: ClassId = c(9);
pub const PROPERTY: ClassId = c(10);
pub const OPERATION: ClassId = c(11);
pub const PARAMETER: ClassId = c(12);
pub const GENERALIZATION: ClassId = c(13);
pub const ASSOCIATION: ClassId = c(14);
pub const LITERAL_INTEGER: ClassId = c(15);
pub const LITERAL_UNLIMITED_NATURAL: ClassId = c(16);
pub const PRIMITIVE_TYPE: ClassId = c(17);
pub const ENUMERATION: ClassId = c(18);
pub const ENUMERATION_LITERAL: ClassId = c(19);

// Properties, in declaration order.
pub const ELEMENT_OWNED_ELEMENT: PropId = p(0);
pub const ELEMENT_OWNED_COMMENT: PropId = p(1);
pub const COMMENT_BODY: PropId = p(2);
pub const COMMENT_ANNOTATED_ELEMENT: PropId = p(3);
pub const NAMED_ELEMENT_NAME: PropId = p(4);
pub const NAMESPACE_OWNED_MEMBER: PropId = p(5);
pub const PACKAGE_PACKAGED_ELEMENT: PropId = p(6);
pub const CLASS_IS_ABSTRACT: PropId = p(7);
pub const CLASS_FEATURE: PropId = p(8);
pub const CLASS_OWNED_ATTRIBUTE: PropId = p(9);
pub const CLASS_OWNED_OPERATION: PropId = p(10);
pub const CLASS_GENERALIZATION: PropId = p(11);
pub const TYPED_ELEMENT_TYPE: PropId = p(12);
pub const MULTIPLICITY_LOWER_VALUE: PropId = p(13);
pub const MULTIPLICITY_UPPER_VALUE: PropId = p(14);
pub const MULTIPLICITY_IS_ORDERED: PropId = p(15);
pub const MULTIPLICITY_IS_UNIQUE: PropId = p(16);
pub const PROPERTY_AGGREGATION: PropId = p(17);
pub const PROPERTY_ASSOCIATION: PropId = p(18);
pub const OPERATION_OWNED_PARAMETER: PropId = p(19);
pub const PARAMETER_DIRECTION: PropId = p(20);
pub const GENERALIZATION_GENERAL: PropId = p(21);
pub const ASSOCIATION_MEMBER_END: PropId = p(22);
pub const LITERAL_INTEGER_VALUE: PropId = p(23);
pub const LITERAL_UNLIMITED_VALUE: PropId = p(24);
pub const ENUMERATION_OWNED_LITERAL: PropId = p(25);
pub const PROPERTY_CLASS: PropId = p(26);

fn build() -> Result<Arc<Metamodel>, MetaError> {
    let mut b = Builder::new(INDEX, URI, "toy", "1", "toy-content-hash");
    let expect_data = |b: &mut Builder, id: DataId, name: &str, kind: DataKind| -> Result<(), MetaError> {
        let got = b.data_type(name, kind, "")?;
        if got != id { Err(MetaError(format!("{name} got id {}, not {}", got.0, id.0))) } else { Ok(()) }
    };
    expect_data(&mut b, BOOLEAN, "Boolean", DataKind::Boolean)?;
    expect_data(&mut b, INTEGER, "Integer", DataKind::Integer)?;
    expect_data(&mut b, REAL, "Real", DataKind::Real)?;
    expect_data(&mut b, STRING, "String", DataKind::String)?;
    expect_data(&mut b, UNLIMITED, "UnlimitedNatural", DataKind::UnlimitedNatural)?;
    expect_data(&mut b, AGGREGATION_KIND, "AggregationKind", DataKind::Enumeration(vec!["none".into(), "shared".into(), "composite".into()]))?;
    expect_data(
        &mut b,
        DIRECTION_KIND,
        "ParameterDirectionKind",
        DataKind::Enumeration(vec!["in".into(), "inout".into(), "out".into(), "return".into()]),
    )?;

    let class = |b: &mut Builder, id: ClassId, name: &str, abs: bool, supers: &[ClassId]| -> Result<(), MetaError> {
        let got = b.class(name, abs, supers, "")?;
        if got != id { Err(MetaError(format!("{name} got id {}, not {}", got.0, id.0))) } else { Ok(()) }
    };
    let prop = |b: &mut Builder, id: PropId, spec: PropertySpec<'_>| -> Result<(), MetaError> {
        let name = spec.name.to_string();
        let got = b.property(spec)?;
        if got != id { Err(MetaError(format!("{name} got id {}, not {}", got.0, id.0))) } else { Ok(()) }
    };
    let str_ = TypeRef::Data(STRING);
    let bool_ = TypeRef::Data(BOOLEAN);

    class(&mut b, ELEMENT, "Element", true, &[])?;
    prop(&mut b, ELEMENT_OWNED_ELEMENT, PropertySpec::new("ownedElement", ELEMENT, TypeRef::Class(ELEMENT)).many().composite().derived_union())?;
    prop(
        &mut b,
        ELEMENT_OWNED_COMMENT,
        PropertySpec::new("ownedComment", ELEMENT, TypeRef::Class(COMMENT)).many().composite().subsets(&[ELEMENT_OWNED_ELEMENT]),
    )?;
    class(&mut b, COMMENT, "Comment", false, &[ELEMENT])?;
    prop(&mut b, COMMENT_BODY, PropertySpec::new("body", COMMENT, str_))?;
    prop(&mut b, COMMENT_ANNOTATED_ELEMENT, PropertySpec::new("annotatedElement", COMMENT, TypeRef::Class(ELEMENT)).many())?;
    class(&mut b, NAMED_ELEMENT, "NamedElement", true, &[ELEMENT])?;
    prop(&mut b, NAMED_ELEMENT_NAME, PropertySpec::new("name", NAMED_ELEMENT, str_).name_of_owner())?;
    class(&mut b, NAMESPACE, "Namespace", true, &[NAMED_ELEMENT])?;
    prop(
        &mut b,
        NAMESPACE_OWNED_MEMBER,
        PropertySpec::new("ownedMember", NAMESPACE, TypeRef::Class(NAMED_ELEMENT))
            .many()
            .composite()
            .derived_union()
            .subsets(&[ELEMENT_OWNED_ELEMENT]),
    )?;
    class(&mut b, PACKAGEABLE_ELEMENT, "PackageableElement", true, &[NAMED_ELEMENT])?;
    class(&mut b, TYPE, "Type", true, &[PACKAGEABLE_ELEMENT])?;
    class(&mut b, PACKAGE, "Package", false, &[NAMESPACE, PACKAGEABLE_ELEMENT])?;
    prop(
        &mut b,
        PACKAGE_PACKAGED_ELEMENT,
        PropertySpec::new("packagedElement", PACKAGE, TypeRef::Class(PACKAGEABLE_ELEMENT)).many().composite().subsets(&[NAMESPACE_OWNED_MEMBER]),
    )?;
    class(&mut b, CLASS, "Class", false, &[NAMESPACE, TYPE])?;
    prop(&mut b, CLASS_IS_ABSTRACT, PropertySpec::new("isAbstract", CLASS, bool_).required().default(Value::Bool(false)))?;
    prop(&mut b, CLASS_FEATURE, PropertySpec::new("feature", CLASS, TypeRef::Class(NAMED_ELEMENT)).many())?;
    prop(
        &mut b,
        CLASS_OWNED_ATTRIBUTE,
        PropertySpec::new("ownedAttribute", CLASS, TypeRef::Class(PROPERTY))
            .many()
            .ordered()
            .composite()
            .opposite(PROPERTY_CLASS)
            .subsets(&[NAMESPACE_OWNED_MEMBER, CLASS_FEATURE]),
    )?;
    prop(
        &mut b,
        CLASS_OWNED_OPERATION,
        PropertySpec::new("ownedOperation", CLASS, TypeRef::Class(OPERATION))
            .many()
            .ordered()
            .composite()
            .subsets(&[NAMESPACE_OWNED_MEMBER, CLASS_FEATURE]),
    )?;
    prop(
        &mut b,
        CLASS_GENERALIZATION,
        PropertySpec::new("generalization", CLASS, TypeRef::Class(GENERALIZATION)).many().composite().subsets(&[ELEMENT_OWNED_ELEMENT]),
    )?;
    class(&mut b, TYPED_ELEMENT, "TypedElement", true, &[NAMED_ELEMENT])?;
    prop(&mut b, TYPED_ELEMENT_TYPE, PropertySpec::new("type", TYPED_ELEMENT, TypeRef::Class(TYPE)))?;
    class(&mut b, MULTIPLICITY_ELEMENT, "MultiplicityElement", true, &[ELEMENT])?;
    prop(
        &mut b,
        MULTIPLICITY_LOWER_VALUE,
        PropertySpec::new("lowerValue", MULTIPLICITY_ELEMENT, TypeRef::Class(LITERAL_INTEGER)).composite().subsets(&[ELEMENT_OWNED_ELEMENT]),
    )?;
    prop(
        &mut b,
        MULTIPLICITY_UPPER_VALUE,
        PropertySpec::new("upperValue", MULTIPLICITY_ELEMENT, TypeRef::Class(LITERAL_UNLIMITED_NATURAL))
            .composite()
            .subsets(&[ELEMENT_OWNED_ELEMENT]),
    )?;
    prop(&mut b, MULTIPLICITY_IS_ORDERED, PropertySpec::new("isOrdered", MULTIPLICITY_ELEMENT, bool_).required().default(Value::Bool(false)))?;
    prop(&mut b, MULTIPLICITY_IS_UNIQUE, PropertySpec::new("isUnique", MULTIPLICITY_ELEMENT, bool_).required().default(Value::Bool(true)))?;
    class(&mut b, PROPERTY, "Property", false, &[TYPED_ELEMENT, MULTIPLICITY_ELEMENT])?;
    prop(
        &mut b,
        PROPERTY_AGGREGATION,
        PropertySpec::new("aggregation", PROPERTY, TypeRef::Data(AGGREGATION_KIND)).required().default(Value::Enum(0)),
    )?;
    prop(&mut b, PROPERTY_ASSOCIATION, PropertySpec::new("association", PROPERTY, TypeRef::Class(ASSOCIATION)).opposite(ASSOCIATION_MEMBER_END))?;
    class(&mut b, OPERATION, "Operation", false, &[NAMESPACE])?;
    prop(
        &mut b,
        OPERATION_OWNED_PARAMETER,
        PropertySpec::new("ownedParameter", OPERATION, TypeRef::Class(PARAMETER)).many().ordered().composite().subsets(&[NAMESPACE_OWNED_MEMBER]),
    )?;
    class(&mut b, PARAMETER, "Parameter", false, &[TYPED_ELEMENT, MULTIPLICITY_ELEMENT])?;
    prop(&mut b, PARAMETER_DIRECTION, PropertySpec::new("direction", PARAMETER, TypeRef::Data(DIRECTION_KIND)).required().default(Value::Enum(0)))?;
    class(&mut b, GENERALIZATION, "Generalization", false, &[ELEMENT])?;
    prop(&mut b, GENERALIZATION_GENERAL, PropertySpec::new("general", GENERALIZATION, TypeRef::Class(CLASS)).required())?;
    class(&mut b, ASSOCIATION, "Association", false, &[TYPE])?;
    prop(
        &mut b,
        ASSOCIATION_MEMBER_END,
        PropertySpec::new("memberEnd", ASSOCIATION, TypeRef::Class(PROPERTY)).many().ordered().opposite(PROPERTY_ASSOCIATION),
    )?;
    class(&mut b, LITERAL_INTEGER, "LiteralInteger", false, &[ELEMENT])?;
    prop(&mut b, LITERAL_INTEGER_VALUE, PropertySpec::new("value", LITERAL_INTEGER, TypeRef::Data(INTEGER)).required().default(Value::Int(0)))?;
    class(&mut b, LITERAL_UNLIMITED_NATURAL, "LiteralUnlimitedNatural", false, &[ELEMENT])?;
    prop(
        &mut b,
        LITERAL_UNLIMITED_VALUE,
        PropertySpec::new("value", LITERAL_UNLIMITED_NATURAL, TypeRef::Data(UNLIMITED)).required().default(Value::Unlimited(Some(1))),
    )?;
    class(&mut b, PRIMITIVE_TYPE, "PrimitiveType", false, &[TYPE])?;
    class(&mut b, ENUMERATION, "Enumeration", false, &[NAMESPACE, TYPE])?;
    prop(
        &mut b,
        ENUMERATION_OWNED_LITERAL,
        PropertySpec::new("ownedLiteral", ENUMERATION, TypeRef::Class(ENUMERATION_LITERAL))
            .many()
            .ordered()
            .composite()
            .subsets(&[NAMESPACE_OWNED_MEMBER]),
    )?;
    class(&mut b, ENUMERATION_LITERAL, "EnumerationLiteral", false, &[NAMED_ELEMENT])?;
    prop(&mut b, PROPERTY_CLASS, PropertySpec::new("class", PROPERTY, TypeRef::Class(CLASS)).opposite(CLASS_OWNED_ATTRIBUTE))?;
    Ok(b.finish())
}

static METAMODEL: LazyLock<Result<Arc<Metamodel>, MetaError>> = LazyLock::new(build);

/// The toy metamodel, as the generator would hand it over.
pub fn metamodel() -> Result<Arc<Metamodel>, MetaError> {
    METAMODEL.clone()
}

pub fn universe() -> Result<Arc<Universe>, MetaError> {
    Ok(Arc::new(Universe::new(vec![metamodel()?])?))
}

/// A store over the toy with deterministic ids: a counter in the random bits, a fixed time.
pub fn store() -> Result<Store, MetaError> {
    Ok(Store::new(universe()?, Box::new(ToySlabs::default()), counter_ids()))
}

pub fn counter_ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

// The generated shape: a struct per concrete metaclass with the inherited fields flattened.

macro_rules! field_get {
    ($self:ident, str $f:ident) => {
        $self.$f.clone().map_or(Value::Null, Value::Str)
    };
    ($self:ident, bool $f:ident) => {
        Value::Bool($self.$f)
    };
    ($self:ident, int $f:ident) => {
        Value::Int($self.$f)
    };
    ($self:ident, unl $f:ident) => {
        Value::Unlimited($self.$f)
    };
    ($self:ident, enm $f:ident) => {
        Value::Enum($self.$f)
    };
    ($self:ident, obj $f:ident) => {
        $self.$f.map_or(Value::Null, Value::Ref)
    };
}

macro_rules! field_set {
    ($self:ident, $v:ident, str $f:ident) => {
        match $v {
            Value::Null => $self.$f = None,
            Value::Str(s) => $self.$f = Some(s),
            other => return Err(format!("not a string: {other}")),
        }
    };
    ($self:ident, $v:ident, bool $f:ident) => {
        match $v {
            Value::Bool(b) => $self.$f = b,
            other => return Err(format!("not a boolean: {other}")),
        }
    };
    ($self:ident, $v:ident, int $f:ident) => {
        match $v {
            Value::Int(i) => $self.$f = i,
            other => return Err(format!("not an integer: {other}")),
        }
    };
    ($self:ident, $v:ident, unl $f:ident) => {
        match $v {
            Value::Unlimited(u) => $self.$f = u,
            other => return Err(format!("not an unlimited natural: {other}")),
        }
    };
    ($self:ident, $v:ident, enm $f:ident) => {
        match $v {
            Value::Enum(e) => $self.$f = e,
            other => return Err(format!("not a literal: {other}")),
        }
    };
    ($self:ident, $v:ident, obj $f:ident) => {
        match $v {
            Value::Null => $self.$f = None,
            Value::Ref(o) => $self.$f = Some(o),
            other => return Err(format!("not a reference: {other}")),
        }
    };
}

macro_rules! field_type {
    (str) => { Option<Arc<str>> };
    (bool) => { bool };
    (int) => { i64 };
    (unl) => { Option<u64> };
    (enm) => { u32 };
    (obj) => { Option<Obj> };
}

/// Declares a metaclass struct: single-valued fields with their kind and property, then the lists.
macro_rules! metaclass {
    ($name:ident, $class:expr, { $($kind:ident $f:ident = $p:expr),* $(,)? } , { $($lf:ident = $lp:expr),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct $name {
            $(pub $f: field_type!($kind),)*
            $(pub $lf: Vec<Value>,)*
        }

        impl Reflect for $name {
            fn class(&self) -> ClassId {
                $class
            }
            #[allow(unused_variables)]
            fn get(&self, prop: PropId) -> Option<Value> {
                $(if prop == $p { return Some(field_get!(self, $kind $f)); })*
                None
            }
            #[allow(unused_variables, unreachable_code)]
            fn set(&mut self, prop: PropId, value: Value) -> Result<(), String> {
                $(if prop == $p { field_set!(self, value, $kind $f); return Ok(()); })*
                Err(format!("no single-valued property {} here", prop.0))
            }
            #[allow(unused_variables)]
            fn list(&self, prop: PropId) -> Option<&[Value]> {
                $(if prop == $lp { return Some(&self.$lf); })*
                None
            }
            #[allow(unused_variables)]
            fn list_mut(&mut self, prop: PropId) -> Option<&mut Vec<Value>> {
                $(if prop == $lp { return Some(&mut self.$lf); })*
                None
            }
        }
    };
}

metaclass!(CommentObj, COMMENT, { str body = COMMENT_BODY }, { owned_comment = ELEMENT_OWNED_COMMENT, annotated_element = COMMENT_ANNOTATED_ELEMENT });
metaclass!(PackageObj, PACKAGE, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT, packaged_element = PACKAGE_PACKAGED_ELEMENT });
metaclass!(ClassObj, CLASS, { str name = NAMED_ELEMENT_NAME, bool is_abstract = CLASS_IS_ABSTRACT }, {
    owned_comment = ELEMENT_OWNED_COMMENT,
    feature = CLASS_FEATURE,
    owned_attribute = CLASS_OWNED_ATTRIBUTE,
    owned_operation = CLASS_OWNED_OPERATION,
    generalization = CLASS_GENERALIZATION,
});
metaclass!(PropertyObj, PROPERTY, {
    str name = NAMED_ELEMENT_NAME,
    obj type_ = TYPED_ELEMENT_TYPE,
    obj lower_value = MULTIPLICITY_LOWER_VALUE,
    obj upper_value = MULTIPLICITY_UPPER_VALUE,
    bool is_ordered = MULTIPLICITY_IS_ORDERED,
    bool is_unique = MULTIPLICITY_IS_UNIQUE,
    enm aggregation = PROPERTY_AGGREGATION,
    obj association = PROPERTY_ASSOCIATION,
    obj class_ = PROPERTY_CLASS,
}, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(OperationObj, OPERATION, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT, owned_parameter = OPERATION_OWNED_PARAMETER });
metaclass!(ParameterObj, PARAMETER, {
    str name = NAMED_ELEMENT_NAME,
    obj type_ = TYPED_ELEMENT_TYPE,
    obj lower_value = MULTIPLICITY_LOWER_VALUE,
    obj upper_value = MULTIPLICITY_UPPER_VALUE,
    bool is_ordered = MULTIPLICITY_IS_ORDERED,
    bool is_unique = MULTIPLICITY_IS_UNIQUE,
    enm direction = PARAMETER_DIRECTION,
}, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(GeneralizationObj, GENERALIZATION, { obj general = GENERALIZATION_GENERAL }, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(AssociationObj, ASSOCIATION, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT, member_end = ASSOCIATION_MEMBER_END });
metaclass!(LiteralIntegerObj, LITERAL_INTEGER, { int value = LITERAL_INTEGER_VALUE }, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(LiteralUnlimitedNaturalObj, LITERAL_UNLIMITED_NATURAL, { unl value = LITERAL_UNLIMITED_VALUE }, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(PrimitiveTypeObj, PRIMITIVE_TYPE, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT });
metaclass!(EnumerationObj, ENUMERATION, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT, owned_literal = ENUMERATION_OWNED_LITERAL });
metaclass!(EnumerationLiteralObj, ENUMERATION_LITERAL, { str name = NAMED_ELEMENT_NAME }, { owned_comment = ELEMENT_OWNED_COMMENT });

/// The defaults the metamodel declares, applied at creation.
trait WithDefaults: Default {
    fn fresh() -> Self;
}
impl WithDefaults for ClassObj {
    fn fresh() -> Self {
        ClassObj::default()
    }
}
impl WithDefaults for PropertyObj {
    fn fresh() -> Self {
        PropertyObj { is_unique: true, ..Default::default() }
    }
}
impl WithDefaults for ParameterObj {
    fn fresh() -> Self {
        ParameterObj { is_unique: true, ..Default::default() }
    }
}
impl WithDefaults for LiteralUnlimitedNaturalObj {
    fn fresh() -> Self {
        LiteralUnlimitedNaturalObj { value: Some(1), ..Default::default() }
    }
}
macro_rules! plain_defaults {
    ($($t:ident),*) => { $(impl WithDefaults for $t { fn fresh() -> Self { Self::default() } })* };
}
plain_defaults!(
    CommentObj,
    PackageObj,
    OperationObj,
    GeneralizationObj,
    AssociationObj,
    LiteralIntegerObj,
    PrimitiveTypeObj,
    EnumerationObj,
    EnumerationLiteralObj
);

/// One typed slab per concrete metaclass.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ToySlabs {
    pub comments: Vec<CommentObj>,
    pub packages: Vec<PackageObj>,
    pub classes: Vec<ClassObj>,
    pub properties: Vec<PropertyObj>,
    pub operations: Vec<OperationObj>,
    pub parameters: Vec<ParameterObj>,
    pub generalizations: Vec<GeneralizationObj>,
    pub associations: Vec<AssociationObj>,
    pub literal_integers: Vec<LiteralIntegerObj>,
    pub literal_unlimited_naturals: Vec<LiteralUnlimitedNaturalObj>,
    pub primitive_types: Vec<PrimitiveTypeObj>,
    pub enumerations: Vec<EnumerationObj>,
    pub enumeration_literals: Vec<EnumerationLiteralObj>,
}

fn push<T: WithDefaults>(v: &mut Vec<T>) -> Option<u32> {
    let slot = u32::try_from(v.len()).ok()?;
    v.push(T::fresh());
    Some(slot)
}

fn at<T: Reflect>(v: &[T], slot: u32) -> Option<&dyn Reflect> {
    v.get(slot as usize).map(|o| o as &dyn Reflect)
}

fn at_mut<T: Reflect>(v: &mut [T], slot: u32) -> Option<&mut dyn Reflect> {
    v.get_mut(slot as usize).map(|o| o as &mut dyn Reflect)
}

impl Slabs for ToySlabs {
    fn create(&mut self, class: ClassId) -> Option<u32> {
        match class {
            COMMENT => push(&mut self.comments),
            PACKAGE => push(&mut self.packages),
            CLASS => push(&mut self.classes),
            PROPERTY => push(&mut self.properties),
            OPERATION => push(&mut self.operations),
            PARAMETER => push(&mut self.parameters),
            GENERALIZATION => push(&mut self.generalizations),
            ASSOCIATION => push(&mut self.associations),
            LITERAL_INTEGER => push(&mut self.literal_integers),
            LITERAL_UNLIMITED_NATURAL => push(&mut self.literal_unlimited_naturals),
            PRIMITIVE_TYPE => push(&mut self.primitive_types),
            ENUMERATION => push(&mut self.enumerations),
            ENUMERATION_LITERAL => push(&mut self.enumeration_literals),
            _ => None,
        }
    }

    fn object(&self, class: ClassId, slot: u32) -> Option<&dyn Reflect> {
        match class {
            COMMENT => at(&self.comments, slot),
            PACKAGE => at(&self.packages, slot),
            CLASS => at(&self.classes, slot),
            PROPERTY => at(&self.properties, slot),
            OPERATION => at(&self.operations, slot),
            PARAMETER => at(&self.parameters, slot),
            GENERALIZATION => at(&self.generalizations, slot),
            ASSOCIATION => at(&self.associations, slot),
            LITERAL_INTEGER => at(&self.literal_integers, slot),
            LITERAL_UNLIMITED_NATURAL => at(&self.literal_unlimited_naturals, slot),
            PRIMITIVE_TYPE => at(&self.primitive_types, slot),
            ENUMERATION => at(&self.enumerations, slot),
            ENUMERATION_LITERAL => at(&self.enumeration_literals, slot),
            _ => None,
        }
    }

    fn object_mut(&mut self, class: ClassId, slot: u32) -> Option<&mut dyn Reflect> {
        match class {
            COMMENT => at_mut(&mut self.comments, slot),
            PACKAGE => at_mut(&mut self.packages, slot),
            CLASS => at_mut(&mut self.classes, slot),
            PROPERTY => at_mut(&mut self.properties, slot),
            OPERATION => at_mut(&mut self.operations, slot),
            PARAMETER => at_mut(&mut self.parameters, slot),
            GENERALIZATION => at_mut(&mut self.generalizations, slot),
            ASSOCIATION => at_mut(&mut self.associations, slot),
            LITERAL_INTEGER => at_mut(&mut self.literal_integers, slot),
            LITERAL_UNLIMITED_NATURAL => at_mut(&mut self.literal_unlimited_naturals, slot),
            PRIMITIVE_TYPE => at_mut(&mut self.primitive_types, slot),
            ENUMERATION => at_mut(&mut self.enumerations, slot),
            ENUMERATION_LITERAL => at_mut(&mut self.enumeration_literals, slot),
            _ => None,
        }
    }

    fn bytes(&self) -> usize {
        self.comments.capacity() * size_of::<CommentObj>()
            + self.packages.capacity() * size_of::<PackageObj>()
            + self.classes.capacity() * size_of::<ClassObj>()
            + self.properties.capacity() * size_of::<PropertyObj>()
            + self.operations.capacity() * size_of::<OperationObj>()
            + self.parameters.capacity() * size_of::<ParameterObj>()
            + self.generalizations.capacity() * size_of::<GeneralizationObj>()
            + self.associations.capacity() * size_of::<AssociationObj>()
            + self.literal_integers.capacity() * size_of::<LiteralIntegerObj>()
            + self.literal_unlimited_naturals.capacity() * size_of::<LiteralUnlimitedNaturalObj>()
            + self.primitive_types.capacity() * size_of::<PrimitiveTypeObj>()
            + self.enumerations.capacity() * size_of::<EnumerationObj>()
            + self.enumeration_literals.capacity() * size_of::<EnumerationLiteralObj>()
    }
}
