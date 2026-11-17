//! The engine's errors. Every variant names the value it is about and, where it helps, the values
//! that would have been accepted.

use std::fmt;

/// Why a call did not run, or failed; in every case nothing changed.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    UnknownCommand {
        id: String,
        close: Vec<String>,
        count: usize,
    },
    NotAnObject,
    TooLarge {
        bytes: usize,
        max: usize,
    },
    TooDeep {
        max: usize,
    },
    UnknownKey {
        key: String,
        accepted: Vec<String>,
    },
    MissingKey {
        key: String,
    },
    WrongType {
        key: String,
        expected: &'static str,
    },
    OutOfRange {
        key: String,
        range: String,
    },
    NotAChoice {
        key: String,
        choices: Vec<String>,
    },
    TooLong {
        key: String,
        max: usize,
    },
    Disabled {
        id: String,
        reason: &'static str,
    },
    Failed {
        id: String,
        error: CommandError,
    },
    /// A handler panicked; its message is kept for the log and the report.
    Internal {
        id: String,
        message: String,
    },
    /// Rolling a failed call back failed too, so the session's state can no longer be trusted:
    /// every edit is refused until the project is opened again.
    Poisoned {
        message: String,
    },
    /// The session is read-only for now ([`crate::Session::set_read_only`]); nothing was changed.
    ReadOnly,
    InteractionRunning,
    NoInteraction,
    /// The command cannot run inside a batch (undo and redo).
    NotInBatch {
        id: String,
    },
}

// An error travels in every `Result` of a call, so it stays small: a new reason boxes what it names
// rather than grow `EngineError` (120 bytes) or `Reason` (48) on a 64-bit target.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<EngineError>() == 120 && std::mem::size_of::<Reason>() == 48);

/// The most of a caller's own text an error repeats, in characters.
const ECHO: usize = 200;

/// A caller's text as an error repeats it: whole when short, else cut at a character boundary and
/// marked as cut.
pub(crate) fn echo(s: &str) -> String {
    if s.chars().count() <= ECHO {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(ECHO).collect();
        out.push_str("... (truncated)");
        out
    }
}

/// What a handler reports when it refuses: a message, the parameter it is about, and, for a reason
/// an interface says in its own words, that reason typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// The refusal as an English sentence, which the command line, the control channel and the log
    /// show.
    pub message: String,
    pub key: Option<String>,
    /// The refusal as a reason with its values, for an interface to translate; none for a reason
    /// not typed yet, which an interface shows as `message`.
    pub reason: Option<Reason>,
}

/// Why a handler refused, typed, so an interface can say it in the user's language while the
/// engine keeps no interface text. A kind is the element's kind as the notation names it in
/// English, a lower-case noun such as `"class"` or `"schema"`; an interface that doesn't know a kind
/// shows the message instead.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// Another element of the same kind where this one would be has the name already, whatever
    /// the case; `name` is that other element's name.
    NameTaken { kind: &'static str, name: String },
    /// An element of `kind` would depend on itself.
    DependsOnItself { kind: &'static str },
    /// A package would go into itself or a package it holds.
    PackageIntoItself,
    /// The model's root package would move: it holds the other packages and stays where it is.
    RootPackageStays,
    /// The package Types would take another name or leave its place directly under the root
    /// package, while it keeps the built-in types.
    TypesPackageKept,
    /// A package would become a second package Types directly under the root package, beside the
    /// one that keeps the built-in types.
    TypesPackageExists,
    /// A package would become the package Types while it holds an element of `kind` named `name`, a
    /// built-in type's name, which would then be taken for that built-in type.
    HoldsBuiltInName { kind: &'static str, name: String },
    /// A name is empty.
    NameEmpty,
    /// A text that has to say something, as a note's, is empty.
    TextEmpty,
    /// A name or a text is longer than `max` characters.
    TooLong { max: usize },
    /// A name or a text holds a control character, or a name a hidden one.
    ControlCharacter,
    /// A name or a text starts or ends with a space.
    EdgeSpace,
    /// An element of `kind` specializes only one of its own kind, as an interface only an
    /// interface.
    SpecializesOnlyItsKind { kind: &'static str },
    /// Only an element of `kind` specializes one, as only an enumeration an enumeration.
    OnlyItsKindSpecializes { kind: &'static str },
    /// An element of `kind` cannot specialize one of kind `general`, as a class an interface.
    CannotSpecialize { kind: &'static str, general: &'static str },
    /// An element of `kind` specializes nothing, as a built-in type.
    SpecializesNothing { kind: &'static str },
    /// Nothing specializes an element of `kind`, as an association class.
    NothingSpecializes { kind: &'static str },
    /// The element `name` is final: nothing may specialize it.
    Final { name: String },
    /// An element of `kind` would be at both ends of a `relationship`, which the notation names in
    /// English, as `"generalization"`.
    Itself { relationship: &'static str, kind: &'static str },
    /// This element is joined to that one by a `relationship` of this direction already.
    Again { relationship: &'static str },
    /// That element is joined to this one by `relationship`s already, so this one would close a
    /// cycle.
    Cycle { relationship: &'static str },
    /// The two ends of a `relationship` are of different kinds, where it joins two of one kind, as a
    /// class binds only a class template.
    KindsDiffer { relationship: &'static str },
    /// An element of `kind` cannot be the `end` of a `relationship`.
    WrongEnd { relationship: &'static str, end: End, kind: &'static str },
    /// An element holds at least `min` of `kind`, as a generalization set one generalization.
    TooFew { kind: &'static str, min: usize },
    /// The generalizations named, each as its specific and its general classifier's names,
    /// specialize different classifiers, where a set groups the generalizations of one.
    ParentsDiffer(Box<[(String, String)]>),
    /// The other end of a binary association aggregates already: only the part's end does.
    OtherEndAggregates,
    /// A composition's part: the other end, the whole's, would allow more than one.
    WholeAllowsMany,
    /// This end is a composition's whole, which allows one at most.
    WholeAllowsOne,
    /// The whole's end of an aggregation or a composition would be navigable.
    WholeNavigable,
    /// An n-ary association's end would aggregate: only a binary association has a whole and a
    /// part.
    NaryAggregates,
    /// A template parameter of `kind`, as `"value"`, is not supported yet.
    KindNotSupported { kind: &'static str },
    /// An element holds at most `max` of `kind`, as a template at most so many template parameters.
    TooMany { kind: &'static str, max: usize },
    /// The template parameters named, each with its kind, need an argument: they have no default.
    /// Every one lacking is named, in the template's order.
    NeedsArgument(Box<[Parameter]>),
    /// What the parameter is given does not fit it: its arguments are of its kind and, when it is
    /// constrained, the constraint or what specializes it.
    Misfit(Box<Parameter>),
    /// The parameter's default, its `argument`, does not fit it.
    DefaultMisfit(Box<Parameter>),
    /// A binding of the template gives the parameter no argument, so the parameter needs a default.
    NeedsDefault(Box<Parameter>),
    /// A binding's argument for the parameter would no longer fit it as changed.
    WouldNotFit(Box<Parameter>),
    /// A binding gives the parameter an argument, so the parameter stays.
    ParameterInUse(Box<Parameter>),
    /// The parameter is its template's last, and a binding names the template, so it stays.
    LastParameter(Box<Parameter>),
    /// An element of `kind` named `name` holds a built-in type's name where the built-in types are
    /// kept, so that built-in type cannot be used.
    BuiltInTaken { kind: &'static str, name: String },
    /// A profile, a stereotype, one of its extensions or a package's profile applications stays
    /// while stereotype applications need it: what is in use, and the uses.
    InUse(Box<InUse>),
    /// A domain or an enum type stays while columns take it: its name, the first of them and how
    /// many in all.
    TypeInUse(Box<TypeInUse>),
    /// The element is UML's own and never changes: the UML library that holds the metaclasses
    /// stereotypes extend, or its metaclass of this name.
    UmlOwn { metaclass: Option<String> },
    /// The package applies the profile already.
    ProfileApplied { package: Box<str>, profile: Box<str> },
    /// The package does not apply the profile, so there is nothing to unapply.
    ProfileNotApplied { package: Box<str>, profile: Box<str> },
    /// `name` is a UML metaclass the notation does not draw yet, so no stereotype extends it.
    MetaclassNotDrawn { name: String },
    /// `name`, cut to its first characters, is no UML metaclass.
    NotAMetaclass { name: String },
    /// `name` is given twice in a list that takes each of `kind` once.
    NamedTwice { kind: &'static str, name: String },
    /// The property `name` is a stereotype's extension end, which goes only with the extension.
    ExtensionEnd { name: String },
    /// The stereotype property `name` has values set on this many elements, so it stays, and keeps
    /// its type.
    ValuesSet { name: String, elements: usize },
    /// The stereotype sits in no profile, so nothing about it changes.
    InNoProfile,
    /// A list that takes names of `kind`, each as text, is no list or holds something else.
    NamesExpected { kind: &'static str },
    /// A list that takes items of `kind`, each as text or as an object of its keys, holds something
    /// else, or an object whose key holds a value of another type.
    ItemsExpected { kind: &'static str },
    /// The literal `name`, cut to its first characters, is no name: `why` says how, boxed so the
    /// error stays small.
    LiteralName { name: Box<str>, why: Box<Reason> },
    /// The element is a stereotype application, which takes no stereotype itself.
    IsApplication,
    /// The element is not inside the project's model, as the model itself, so no stereotype applies to it.
    NotInModel,
    /// The element `name` belongs to a profile, which defines stereotypes and takes none.
    InProfile { name: String },
    /// The element `name` is of a metaclass no stereotype may extend.
    TakesNoStereotype { name: String },
    /// The stereotype is applied to the element already.
    StereotypeApplied { stereotype: Box<str>, element: Box<str> },
    /// The stereotype does not extend the element's metaclass.
    Unextended(Box<Unextended>),
    /// No package around the element applies the stereotype's `profile`; `package` is the one the
    /// element sits in.
    ProfileOutOfScope { profile: Box<str>, package: Box<str> },
    /// The stereotype is not applied to the element, so it has no values there to set or take off.
    NotApplied { stereotype: Box<str>, element: Box<str> },
    /// The stereotype has no property of the name or id asked for; it has others.
    NoProperty(Box<NoProperty>),
    /// The stereotype has no property `name`, as asked for and cut to its first characters, nor any
    /// other yet.
    NoProperties { stereotype: Box<str>, name: Box<str> },
    /// The element `name` is not of `kind`, as `"stereotype"`, `"class diagram"` or, of the model,
    /// `"constraint"`, which the parameter takes; for a new diagram's element, of
    /// `"element a diagram is of"`.
    NotA { kind: &'static str, name: String },
    /// A stereotype property's values, each as text, are no list or hold something else.
    ValuesExpected,
    /// No id is given where the id of an element of `kind` is taken, as `"stereotype"` or
    /// `"constraint"`, or, for a `"property"`, its id or name; an item of the lists a property
    /// subsets or redefines, of `"subsetted property"` or `"redefined property"`, likewise.
    IdEmpty { kind: &'static str },
    /// The element is gone from the model.
    Gone,
    /// The element `name` is a template parameter of `template`, which it types alone.
    ParameterElement { name: Box<str>, template: Box<str> },
    /// A constraint's expression is empty or holds only spaces, so it states no rule.
    NoExpression,
    /// The element owns no constraint: a classifier of the model's own or a package does.
    NotAContext,
    /// The element is none a constraint names: an element of the model's own is.
    NotConstrainable,
    /// A constraint shown as braces sits by an element it names, and the diagram shows none of them.
    BracesNoElement,
    /// The constraint is among no element's constraints: it has no context, or its context does not
    /// list it.
    NoContext,
    /// No place `at` is among the elements of `kind`, as `"constraint"`, a list whose last place is
    /// `last`; -1 is the last.
    NoPlace { kind: &'static str, at: i64, last: usize },
    /// One of the coordinates `x` and `y` is given without the other.
    OneCoordinate,
    /// The coordinate `axis`, `"x"` or `"y"`, is not a number.
    CoordinateNotANumber { axis: &'static str },
    /// The coordinate `axis`, `"x"` or `"y"`, is farther from the origin than a diagram reaches.
    CoordinateTooFar { axis: &'static str },
    /// The element is of a kind that takes no description.
    TakesNoDescription,
    /// The element owns `max` comments or more, so its description, looked for among the first
    /// `max` alone, is neither set nor cleared.
    TooManyComments { max: usize },
    /// A list that takes ids of `kind`, each as text or as an object with its id, holds something
    /// else.
    IdsExpected { kind: &'static str },
    /// An item of a list of ids, given as an object, holds `key`, where it holds only `id` and `name`.
    NotAnItemKey { key: String },
    /// An item of a list, given as an object, holds `key`, cut to its first characters, where it
    /// takes only the keys `takes`.
    UnknownKey { key: String, takes: &'static [&'static str] },
    /// No element of the model has the id `id`, cut to its first characters.
    NoElement { id: String },
    /// The built-in type `name` is never made leaf.
    BuiltInNotLeaf { name: String },
    /// The elements named, a member with its owner as `Class::name()`, redefine the element, so it
    /// is made leaf only once their redefinitions are gone.
    Redefined(Box<[String]>),
    /// The values given a stereotype property do not fit it.
    Values(Box<Values>),
    /// The stereotype property `name` has values on this many `elements` that do not fit the
    /// `bounds` it would take, written as a multiplicity, as `0..1`.
    ValuesMisfit { name: Box<str>, elements: usize, bounds: Box<str> },
    /// A literal is the value of a stereotype property on an element that stays, so it stays.
    LiteralValued(Box<LiteralValued>),
    /// A property's default value is longer than `max` characters.
    DefaultTooLong { max: usize },
    /// A property's default `value`, as typed and cut short, does not read as its type's default,
    /// which is `kind`.
    DefaultUnread { value: Box<str>, kind: DefaultKind },
    /// A property's default `value`, cut short, does not fit `ty`, the type it would take (none for
    /// no type), so the type stays until the default is changed or cleared.
    DefaultUnfit { value: Box<str>, ty: Option<Box<str>> },
    /// The property `name` breaks `rule` as a property this one would subset, or, `redefining`,
    /// redefine.
    TargetBreaks { name: Box<str>, rule: TargetRule, redefining: bool },
    /// A box's `side`, `"width"` or `"height"`, is not between the least and the most a box takes.
    SizeOutOfRange { side: &'static str },
    /// A diagram of its kind does not show the element `name`, by the element's kind.
    NotShown { name: String },
    /// The element `name` is outside what the diagram shows, such as another package's.
    OutsideDiagram { name: String },
    /// The diagram shows the element `name` already.
    ShownAlready { name: String },
    /// The node named is in another diagram than the one the command is about.
    OtherDiagram,
    /// The node shows an element and draws its text, so it takes no text of its own.
    ShowsElement,
    /// A note's link is a straight line, so it takes no waypoints.
    StraightLink,
    /// A line between two boxes that both show an element shows a relationship, and none is named.
    NoRelationship,
    /// The element a line would show lacks one of its two ends, so no line shows it.
    LacksEnd,
    /// The box at the line's `end`, its source or its target, does not show the relationship's end
    /// there.
    NotAtEnd { end: End },
    /// What a note's link points at is neither a box that shows an element nor a relationship's line.
    NotLinkable,
    /// What a note's link points at is on another diagram than the note.
    TargetOtherDiagram,
    /// A note's link shows no element, so it has no text for a label to show.
    LinkUnlabelled,
    /// The line has a label that shows its element's text `slot` already.
    Labelled { slot: &'static str },
    /// A style's colour `part`, `"fill"`, `"stroke"` or `"text"`, is not of the form `#rrggbb`.
    NotAColour { part: &'static str },
    /// Arranged, a box would land farther from the origin than a diagram reaches.
    ArrangedTooFar,
    /// The elements of `notation`, such as `"ER"`, are not removed yet.
    NotRemovable { notation: &'static str },
    /// The element `name`, or its id when it has none, is not removed yet, where its notation
    /// removes others: a UML association's end, an element of a kind UML does not remove yet, or a
    /// classifier template parameter that is no template's parameter.
    ElementNotRemovable { name: String },
    /// The element is of a kind its notation removes where it is edited, not with the selection, as
    /// an ER domain, enum type, check or index.
    RemovedWhereEdited,
    /// The element is a part of a subtype's key, which goes only with its subtype, one of the
    /// subtype's two tables or their schema.
    SubtypeKeyPart,
    /// The elements of `notation` are not described yet.
    NotDescribable { notation: &'static str },
    /// A word that names a kind, as a diagram's kind, a box's role, a label's slot or a style's
    /// name, is empty or more than lower-case letters, digits, `_`, `-` and `.` after a letter.
    NotAPlainWord,
    /// A word is none of the words its place takes, as a diagram kind no notation knows.
    NotOneOf(Box<NotOneOf>),
    /// The element `column` is no column of the table `table`, whose keys take only its own.
    NotColumnOf { column: Box<str>, table: Box<str> },
    /// The column `name` may be null, which a primary key's column never is.
    NullableKeyColumn { name: String },
    /// A foreign key has `columns` columns, where the key of the table `table` it points at has
    /// `key`; they match by position.
    ColumnCount { columns: usize, table: Box<str>, key: usize },
    /// A foreign key's column `column` and the key's column `target` it matches by position differ
    /// in type; `types` holds the column's type and the target's, none for a column without one.
    TypesDiffer { column: Box<str>, target: Box<str>, types: Box<[Option<Box<str>>; 2]> },
    /// The foreign key `foreign_key` points at a key of the table `table`, so the key keeps its
    /// columns while it does.
    KeyInUse { foreign_key: Box<str>, table: Box<str> },
    /// The table `table` has no primary key.
    NoPrimaryKey { table: String },
    /// The key named is not one of the table `table`'s, which a foreign key is said to refer to.
    KeyNotOf { table: Box<str> },
    /// The key a foreign key would point at, of the table `table`, has no columns to copy.
    NoKeyColumns { table: Box<str> },
    /// Identifying relationships would run in a cycle: `child` would be built on `parent`, which is
    /// built on `child` already, or is `child` itself through its own primary key.
    IdentifyingCycle { child: Box<str>, parent: Box<str> },
    /// A foreign key sets its columns to null on a delete or an update, and its column `column` may
    /// never be null.
    SetNullNotNullable { column: Box<str> },
    /// The relationship a foreign key follows does not join the foreign key's table `table` and the
    /// table `references` it points at.
    RelationshipNotJoining { table: Box<str>, references: Box<str> },
    /// A foreign key follows an identifying relationship, and its column `column` is outside its
    /// table's primary key, which holds every column of such a foreign key.
    IdentifyingOutsideKey { column: Box<str> },
    /// The two ends of a `relationship` are in different schemas, where it joins two of one
    /// schema, as a subtype its supertype.
    SchemasDiffer { relationship: &'static str },
    /// The table `table` has a supertype already, where a table has one at most.
    HasSupertype { table: Box<str> },
    /// The element is part of a subtype's key, which its subtype owns: it changes or goes only with
    /// the subtype or the supertype's mapping.
    InSubtypeKey,
    /// A relationship asked to identify its child has no foreign key, or is added without one.
    IdentifyingWithoutForeignKey,
    /// A relationship added without its foreign key is given a key to point at.
    KeyWithoutForeignKey,
    /// A many-to-many relationship is asked for a foreign key, which only a mapping table holds.
    ManyToManyForeignKey,
    /// A relationship of a table with itself brings its foreign key without a role at the end the
    /// key points to, which names the key columns.
    RoleNeeded,
    /// A role has no letter or digit to name key columns by.
    RoleWithoutLetter,
    /// The key or column `name`, of `kind` (as `"primary key"` or `"column"`), would break if the
    /// step went ahead; `why` says how, boxed so the error stays small.
    WouldBreak { kind: &'static str, name: Box<str>, why: Box<Reason> },
    /// What of `kind` is built on each other (keys, as `"key"`) runs deeper than `max` levels, or
    /// in a loop.
    TooDeep { kind: &'static str, max: u32 },
    /// A relationship that does not become a mapping table, or a table that is no mapping table or
    /// does not collapse back into a relationship, by the rule it breaks.
    Mapping(MappingRule),
    /// A sequence's numbers do not fit each other, by the rule they break.
    Sequence(SequenceRule),
    /// The column `column`, as `table.column`, is of another schema than the sequence it would own.
    ColumnOfOtherSchema { column: Box<str> },
    /// The column `column` belongs to no table.
    InNoTable { column: Box<str> },
    /// The column `column` is of the type `ty`, empty for none, which is no whole-number type.
    NotIntegerType { column: Box<str>, ty: Box<str> },
    /// A column's generation does not fit it, by the rule it breaks.
    Generation(GenerationRule),
    /// A key or identity column is asked to be nullable, which it never is.
    NotNullable,
    /// An index column names the column `column` and an expression too, where it sorts by one of
    /// them.
    AttributeAndExpression { column: Box<str> },
    /// An index column names neither a column nor an expression to sort by.
    NoAttributeNorExpression,
    /// The size `value` of a column or a domain does not fit its type `ty` (empty when it has no
    /// type name of its own), by the rule it breaks.
    SizeMisfit { rule: SizeRule, ty: Box<str>, value: i64 },
    /// A type name holds its own size in parentheses, as `VARCHAR(20)`, where the length, precision
    /// and scale are given apart.
    SizeInTypeName,
    /// A column is given a domain and an enum type, where it takes one of them at most.
    DomainAndEnum,
    /// A column on a domain is given its own `what`, `"type"`, `"size"`, `"default"` or
    /// `"collation"`, which the domain gives it.
    DomainGives { what: &'static str },
    /// A column of an enum type is given its own `what`, `"type"` or `"size"`, of which it takes
    /// none.
    EnumTypeTakesNo { what: &'static str },
    /// The column `name` is unlinked, though it follows no column.
    FollowsNoColumn { name: Box<str> },
    /// A column's place is past the end of its table, which has `columns` columns.
    PastLastColumn { columns: usize },
    /// The call expected the model at `expected`, but it is at `current`: it changed since the
    /// caller read it, or the caller read another session's. None expected when the call gave no
    /// readable revision.
    StaleRevision { expected: Option<crate::cmd::Revision>, current: crate::cmd::Revision },
    /// A package's default diagram cannot be read, as its default-diagram binding is unsound in
    /// the way the fault names; one of the package's own diagrams has to be chosen as its default.
    InvalidBinding(BindingFault),
    /// The diagram stays while the default-diagram binding of `package` (its name, or its id when
    /// it has none) names it: with no fault, it is that package's own default, which another has to
    /// replace first; with [`BindingFault::WrongOwner`], it is another element's diagram, which the
    /// binding names unsoundly, so a diagram of the package's own has to be chosen first (AR-578).
    DefaultDiagramOf { package: Box<str>, fault: Option<BindingFault> },
    /// The class diagram `diagram` is not one of `package`'s own (each its name, or its id when it
    /// has none), so it cannot be that package's default diagram (AR-580).
    NotOwnDiagram { diagram: Box<str>, package: Box<str> },
}

/// What is wrong with a package's default-diagram binding, in a [`Reason::InvalidBinding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingFault {
    /// The package has diagrams, but none is its default.
    Missing,
    /// More than one binding names the package's default.
    Duplicate,
    /// The binding names no diagram that exists.
    DeadTarget,
    /// The binding names a diagram of a kind no default may be, as an ER diagram for a UML package.
    WrongKind,
    /// The binding names a diagram of another element.
    WrongOwner,
}

/// The rule a mapping table, or a relationship to become one, breaks, in a [`Reason::Mapping`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingRule {
    /// Only a many-to-many relationship becomes a mapping table.
    NotManyToMany,
    /// The relationship lacks one of its two tables.
    LacksEnd,
    /// A mapping table has exactly two foreign keys.
    TwoForeignKeys,
    /// The foreign key `foreign_key` stands for no relationship.
    NoRelationship { foreign_key: Box<str> },
    /// The foreign key `foreign_key` points at no key.
    NoKey { foreign_key: Box<str> },
    /// The table has a relationship besides its two foreign keys' own.
    OwnRelationship,
    /// The table has columns besides its foreign keys'.
    OwnColumns,
    /// Not all of the table's columns are in its primary key.
    ColumnsOutsideKey,
    /// The table has a unique constraint.
    UniqueConstraint,
    /// The table has a check.
    Check,
    /// The table has an index.
    Index,
    /// The table `table` has only `keys` foreign keys (0 or 1) whose columns are all in its primary
    /// key, where a mapping table has two.
    KeysInPrimaryKey { table: Box<str>, keys: usize },
}

/// The rule a sequence's numbers break, in a [`Reason::Sequence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceRule {
    /// The increment is 0, where a sequence moves by at least 1.
    IncrementZero,
    /// The minimum, as given and cut short, is no whole number of 64 bits.
    MinimumUnread { value: Box<str> },
    /// The maximum, as given and cut short, is no whole number of 64 bits.
    MaximumUnread { value: Box<str> },
    /// The maximum is not greater than the minimum.
    MaximumNotAbove { maximum: i64, minimum: i64 },
    /// The start is below the minimum.
    StartBelow { start: i64, minimum: i64 },
    /// The start is above the maximum.
    StartAbove { start: i64, maximum: i64 },
}

/// The rule a column's generation breaks, in a [`Reason::Generation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationRule {
    /// An expression is given to a column whose value is not computed.
    ExpressionNeedsComputed,
    /// A computed column has no expression.
    ComputedNeedsExpression,
    /// A computed column has a default.
    ComputedTakesNoDefault,
    /// An identity column's type is not an integer type.
    IdentityNeedsInteger,
    /// An identity column may be null.
    IdentityNotNullable,
    /// An identity column has a default.
    IdentityTakesNoDefault,
    /// An identity column has an expression.
    IdentityTakesNoExpression,
}

/// The rule a size breaks, in a [`Reason::SizeMisfit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeRule {
    /// A length, precision or scale is not between 1 and 65535.
    OutOfRange,
    /// A length is given to a type that takes none.
    TakesNoLength,
    /// A precision is given to a type that takes none.
    TakesNoPrecision,
    /// A scale is given to a type that takes none.
    TakesNoScale,
    /// A BINARY's length is more than the 255 bytes it holds.
    BinaryTooLong,
    /// The scale is more than the precision.
    ScaleOverPrecision,
    /// A scale is given without a precision.
    ScaleWithoutPrecision,
}

/// What a [`Reason::NotOneOf`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotOneOf {
    /// What the word names, which decides how an interface says the refusal.
    pub what: WordPlace,
    /// The word given.
    pub word: String,
    /// The words the place takes, in order; empty when it takes none.
    pub known: Vec<String>,
}

/// Where a [`NotOneOf`]'s word goes, which an interface says the refusal by; a notation that types
/// another place's refusal adds its own variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WordPlace {
    /// A diagram's kind, which the notations know.
    DiagramKind,
    /// A text slot a label shows, which the line's element has.
    TextSlot,
    /// The sort direction of an ER index column.
    IndexDirection,
    /// Where an ER index column sorts its nulls.
    IndexNulls,
}

/// What a [`Reason::Values`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Values {
    /// The stereotype property's name.
    pub property: String,
    /// What does not fit.
    pub misfit: ValueMisfit,
}

/// What does not fit in a [`Values`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueMisfit {
    /// More than `max` values.
    TooMany { max: usize },
    /// A value of nothing but spaces.
    Empty,
    /// A value longer than `max` characters.
    TooLong { max: usize },
    /// `value`, as typed and cut short, does not read as the property's type, which takes `accepted`.
    Unread { value: String, accepted: Accepted },
    /// `given` values, where the property's multiplicity allows `lower` to `upper`, none for no
    /// upper bound.
    Count { given: usize, lower: u64, upper: Option<u64> },
    /// `value`, as typed and cut short, twice in a property that holds each value once.
    Twice { value: String },
}

/// What a value of a stereotype property's type is, in a [`ValueMisfit::Unread`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accepted {
    /// A whole number.
    Integer,
    /// A number.
    Real,
    /// True or false.
    Boolean,
    /// A whole number of 0 or more, or `*`.
    UnlimitedNatural,
    /// Any text.
    Text,
    /// One of an enumeration's literals, by name.
    Literals(Vec<String>),
    /// One of the literals of the enumeration named `enumeration`, which has none yet.
    NoLiterals { enumeration: String },
}

/// What a [`Reason::LiteralValued`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralValued {
    /// The literal's name.
    pub literal: String,
    /// The stereotype property it is the value of.
    pub property: String,
    /// The stereotype applied.
    pub stereotype: String,
    /// The element the stereotype is applied to, by its name or, without one, its id.
    pub element: String,
}

/// What a property's default is, by its type, in a [`Reason::DefaultUnread`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultKind {
    /// A whole number, an Integer's.
    Integer,
    /// A number, a Real's.
    Real,
    /// True or false, a Boolean's.
    Boolean,
    /// A whole number of 0 or more, or `*`, an UnlimitedNatural's.
    UnlimitedNatural,
    /// One of the literals of the enumeration named so.
    Literal(Box<str>),
    /// Any text, a String's or another type's.
    Text,
}

/// The rule a property breaks as one another subsets or redefines, in a [`Reason::TargetBreaks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRule {
    /// It is no property at all.
    NotAProperty,
    /// It is the property itself.
    Itself,
    /// It has no name to be named by in the braces.
    Unnamed,
    /// It is not where the property's targets are: a property of its classifier or of one its
    /// classifier specializes, for a subset; of one its classifier specializes, for a redefinition.
    OutOfReach,
    /// It has the property's name, whatever the case, which a subset never has.
    SameName,
    /// It subsets the property already, and subsets never run round a cycle.
    Cycle,
    /// Its type is not one the property's type conforms to.
    Type,
    /// Its upper bound is below the property's.
    Upper,
    /// The property's bounds do not both lie within its bounds.
    Multiplicity,
    /// It is composite and the property is not.
    Composite,
    /// It is leaf: nothing redefines it.
    Leaf,
}

/// What a [`Reason::NoProperty`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoProperty {
    /// The stereotype's name.
    pub stereotype: String,
    /// The property's name or id as asked for, cut to its first characters.
    pub name: String,
    /// The names of the stereotype's properties; never empty, as [`Reason::NoProperties`] says a
    /// stereotype without any.
    pub has: Vec<String>,
}

/// What a [`Reason::Unextended`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unextended {
    /// The stereotype's name.
    pub stereotype: String,
    /// The element's metaclass by UML's name, as `"Class"`; empty when it has none a stereotype extends.
    pub metaclass: &'static str,
    /// The metaclasses the stereotype extends, by UML's names; none when it extends nothing yet.
    pub extends: Vec<String>,
}

/// What a [`Reason::InUse`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InUse {
    /// What is in use, and where.
    pub what: Used,
    /// The uses by stereotype: its name, the names of the elements it is applied to while a refusal
    /// names them (two at most, none when it counts them), and how many applications in all.
    pub uses: Vec<(String, Vec<String>, usize)>,
}

/// What a [`Reason::TypeInUse`] names; boxed in the reason, so the error stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInUse {
    /// The domain's or the enum type's name, or its id when it has none.
    pub name: String,
    /// The first columns that take it, each as `table.column`.
    pub columns: Vec<String>,
    /// How many columns take it in all.
    pub count: usize,
}

/// What is in use in a [`Reason::InUse`], each by its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Used {
    /// A profile or a stereotype, which would go.
    Element(String),
    /// A stereotype, which would move to another profile.
    Moved(String),
    /// A package or a classifier, which would move and take the uses inside it out of the packages
    /// that apply their profiles.
    MovedAway(String),
    /// A profile a package would no longer apply.
    ProfileIn { profile: String, package: String },
    /// A stereotype's extension of a metaclass, which would be dropped.
    Extension { stereotype: String, metaclass: String },
    /// A profile inside a package that would go and take it along.
    HeldProfile { package: String, profile: String },
    /// The profiles a package applies, which would go with it.
    AppliedProfiles { package: String },
}

/// A template parameter as a refusal names it, with the binding of its template or the default
/// the refusal is about; boxed in a [`Reason`], so the error stays small.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameter {
    /// The parameter's name.
    pub name: String,
    /// Its kind as an English noun, as `"class"` or `"data type"`.
    pub kind: &'static str,
    /// The classifier its arguments are constrained to, if any.
    pub constraint: Option<String>,
    /// The template it is a parameter of.
    pub template: String,
    /// The element a binding of the template binds; empty when the refusal names no binding.
    pub bound: String,
    /// What the parameter is given, a binding's argument or a default; empty when nothing.
    pub argument: String,
}

/// The end of a relationship a [`Reason::WrongEnd`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The end it is drawn from, as a generalization's specific classifier.
    Source,
    /// The end it is drawn to, as a generalization's general classifier.
    Target,
    /// One of an n-ary association's ends, which have no direction.
    Any,
}

impl CommandError {
    /// A refusal about no parameter in particular, its reason not typed.
    pub fn new(message: impl Into<String>) -> CommandError {
        CommandError { message: message.into(), key: None, reason: None }
    }

    /// A refusal about the parameter `key`, its reason not typed.
    pub fn about(key: &str, message: impl Into<String>) -> CommandError {
        CommandError { message: message.into(), key: Some(key.to_string()), reason: None }
    }

    /// The same refusal with its reason typed as `reason`; the message stays as it was.
    #[must_use]
    pub fn because(self, reason: Reason) -> CommandError {
        CommandError { reason: Some(reason), ..self }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key {
            Some(k) => write!(f, "{k}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownCommand { id, close, count } if close.is_empty() => write!(f, "no command {id:?}; {count} commands are known"),
            EngineError::UnknownCommand { id, close, count } => write!(f, "no command {id:?}; close ones: {} (of {count})", close.join(", ")),
            EngineError::NotAnObject => f.write_str("the parameters must be a JSON object"),
            EngineError::TooLarge { bytes, max } => write!(f, "the parameters weigh over {bytes} bytes; the limit is {max}"),
            EngineError::TooDeep { max } => write!(f, "the parameters nest deeper than {max} levels"),
            EngineError::UnknownKey { key, accepted } if accepted.is_empty() => write!(f, "unknown parameter {key:?}; this command takes none"),
            EngineError::UnknownKey { key, accepted } => write!(f, "unknown parameter {key:?}; accepted: {}", accepted.join(", ")),
            EngineError::MissingKey { key } => write!(f, "the parameter {key:?} is required"),
            EngineError::WrongType { key, expected } => write!(f, "the parameter {key:?} must be of type {expected}"),
            EngineError::OutOfRange { key, range } => write!(f, "the parameter {key:?} must be within {range}"),
            EngineError::NotAChoice { key, choices } => write!(f, "the parameter {key:?} must be one of {}", choices.join(", ")),
            EngineError::TooLong { key, max } => write!(f, "the parameter {key:?} is longer than {max} bytes"),
            EngineError::Disabled { id, reason } => write!(f, "{id} is not available: {reason}"),
            EngineError::Failed { id, error } => write!(f, "{id} failed: {error}"),
            EngineError::Internal { id, message } => write!(f, "{id} stopped on an internal error ({message}); nothing was changed"),
            EngineError::Poisoned { message } => {
                write!(f, "the session's state could not be restored after a failure ({message}); reopen the project")
            }
            EngineError::ReadOnly => f.write_str("the model cannot be changed at the moment; nothing was changed"),
            EngineError::InteractionRunning => f.write_str("a gesture is in progress; retry after the gesture"),
            EngineError::NoInteraction => f.write_str("no gesture is in progress"),
            EngineError::NotInBatch { id } => write!(f, "{id} cannot run inside a batch"),
        }
    }
}

impl std::error::Error for EngineError {}
