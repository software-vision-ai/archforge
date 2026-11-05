# archforge-uml

**Purpose.** The UML 2.5.1 metamodel as a metamodel of the MOF core: 242 metaclasses (193
concrete), 622 class-owned properties (73 derived, without a field), 13 enumerations and the five
primitive types, generated from OMG's `UML.xmi` by `cargo xtask generate uml`. Only `lib.rs` is
written by hand.

**Layer.** 7, the notations' layer above the MOF core (4), the shared core (5) and the diagrams (6),
whose project view it joins as `UmlNotation` (class diagrams of a package, classes as plain cards
with their attributes and operations; skills on packages, classes, attributes and operations,
aiming at every class, attribute, operation or association below) and whose commands it adds to (`commands`: `uml.package.{add,
rename, list}`, `uml.class.{add, rename, list}`; a name told apart from its siblings', whatever the
case; removing a package or a class goes through `element.remove`, with its views); pure, builds for
the portable target.

**Binding rules.** The metamodel is data read at build time and compiled in; nothing
at runtime redefines it. The XMI is read from the folder `ARCHFORGE_SPECS_DIR` names, outside the
repository, until the owner decides where the files may live; the generated code is committed, so the
repository builds without them, and the gate's `generated` step fails when the committed code is not
what the XMI gives (it says when it skipped because the folder is not there). The spec's comments
are not emitted (`--docs`, off until that decision). The content hash of the XMI is `CONTENT_HASH`.

**What the generated code is.** `meta.rs`: `metamodel()` built with the core's `Builder`, the
class and data type constants (`CLASS`, `AGGREGATION_KIND`) and one module per declaring class with
its property constants (`class::OWNED_ATTRIBUTE`, `named_element::NAME`). `classes.rs`: one struct
per concrete metaclass with every inherited stored property as a field (`ClassObj`, with
`Reflect`, typed reads such as `name()`, and typed writes through the store such as
`ClassObj::set_name(store, obj, ..)` and `ClassObj::push_owned_attribute(..)`), and one Rust enum
per enumeration (`VisibilityKind::from_index`, `literal`). `slabs.rs`: `UmlSlabs`. `schema.rs`:
`TABLES` and `JUNCTIONS`, the storage schema as data for the project crate. `report.txt`: what was
read, dropped and skipped, and the 51 derived properties that need a hand-written getter.

**Where the generator departs from the file.** Association-owned ends (328) give no stored
opposite: 180 properties have one, the rest none. Subsets or redefinitions of association-owned
ends (34) are dropped. The one default on a derived property is dropped. Operations (205) and OCL
constraints (449) are counted, not read. Where a class redefines an inherited property (Class
redefines `isAbstract`), the redefining property is the class's: `class::IS_ABSTRACT`, not
`classifier::IS_ABSTRACT`.

**Public API.** `metamodel()`, `universe()`, the constants, the structs and enums, `UmlSlabs`,
`TABLES`, `JUNCTIONS`, `INDEX`, `URI`, `VERSION`, `CONTENT_HASH`; `UmlNotation`, `ALONE`,
`CLASS_DIAGRAM`, `CLASS_SIZE`, `PRIMITIVE_TYPE_SIZE`, `CLASSIFIERS` and `is_classifier`, `registry`, `commands::{specs, packages, classes,
interfaces, enumerations, data_types, primitive_types, literals, classifiers, package_of, remove_elements, MAX_NAME}` (`uml.enumeration.{add,
rename, move, list}`: an enumeration in a package, its name shared with the classes and interfaces
there, removed with its literals through `element.remove`; `uml.data-type.{add, rename, move,
list}` (AR-123): a DataType in a package, the DataType metaclass alone, never an enumeration or a
primitive type, its name shared with the package's other classifiers, its attributes and operations
kept in DataType's own lists by the member commands, removed with them through `element.remove`; `uml.primitive-type.{add,
rename, move, list}` (AR-124): a PrimitiveType of the model's own in a package, with no members, its
name shared with the package's other classifiers and, in a package named Types, never a built-in
type's; a built-in type is never renamed or moved; the list says which are built-in), `members::{specs, is_built_in, attributes, operations, parameters,
parse_multiplicity, MultiplicityError, literal_owner, literal_clash, MAX_LITERALS, BUILT_IN, TYPES, MAX_PARAMETERS, MAX_BOUND,
NO_ENUMERATIONS}` (`uml.class.{set, get}`, `uml.attribute.{add, set, move}`, `uml.operation.{add, set, move}`,
`uml.literal.{add, rename, move}`: an enumeration's literals at a place asked for or the end, their
names told apart within their enumeration whatever the case, at most `MAX_LITERALS` (256, as ER's
enum types) in one, a place past the end refused, removed
through `element.remove`; `uml.type.list`; a type is an
element's id or a built-in type's name, Integer, String, Boolean, Real or UnlimitedNatural, made in
the package Types the first time it is used and never twice, a built-in name always meaning the one
in Types; a multiplicity is typed, `0`, `1`, `0..1`, `*`, `1..*`,
`2..5`, and one that does not read is a `MultiplicityError` naming why; members are removed through `element.remove`), `figures::{card, line, ends, text_slots, slot_text, attribute_text,
operation_text, multiplicity, visibility, member_ends, navigable, MAX_CARD_ROWS}` (a class's card with its
members' visibility, type and multiplicity, abstract names slanted; an interface's and a data
type's the same under `«interface»` and `«dataType»`; a primitive type's `«primitive»` over its
name and no compartment; an enumeration's with
`«enumeration»` and one compartment of its literals; at most `MAX_CARD_ROWS` rows a card, the rest
counted in its `hidden`; an association's line with its
name, roles, multiplicities, an arrow at every end it navigates to and a diamond at the whole of
an aggregation or a composition; a generalization's triangle; a dependency dashed with an open
arrow; an interface realization dashed in the muted ink to a hollow triangle at its interface),
`relations::{specs, relationships_of, generalization_problem, GeneralizationProblem, association_end_problem, realization_problem, RealizationProblem}`
(an enumeration specializes only an enumeration, a data type only a data type and a primitive type
only a primitive type, each specialized only by its own kind, a built-in type specializing nothing,
and none of them joins an association; `uml.realization.add`: a class
realizes an interface, owned by the class, its client and supplier kept in step with its
implementing classifier and contract; reconnected through `uml.relationship.reconnect` and removed
through `element.remove`), `RELATIONSHIPS` and `is_relationship`, and the card's `WORDS` and `PLURALS` for the shell's catalogue. Where an association's two
ends pair a derived end with a stored one, the generator gives neither an opposite: a derived end
keeps no field to keep in step.

**Measured** (release, M4 Max): a million UML objects (classes with three typed attributes and
their multiplicity literals), rename p99 well inside a frame; see the test's output.

**Tests.** `tests/core_over_uml.rs`: the shared core and UML in one store (a Folder holding a
Package, comments, tags and links across the two, destroy with links and undo, the core's rules, the
sweep). `tests/uml.rs`: the universe builds with the file's counts and the key inheritance and
derived-union facts; a package with a class and a typed attribute behaves as UML says through the
store (both ends of `ownedAttribute` and `class`, containment, `ownedElement`, `ownedMember`,
`attribute`, defaults, destroy and undo); the adversarial sweep over a session of generic commands;
the million-object test (release only). `tests/skills.rs`: skills on a package, a class, an
attribute and an operation, each one undo step; a relationship and an association's end refused;
the kinds a skill aims at; inheritance down the package tree with an override by name; a skill
going with its element, a nested package's too, and back with undo.

**Known lacks.** No derived getters yet (`Classifier::general`, `MultiplicityElement::lower`, …
read as Derived until a notation writes them). No hand-written rules beyond the core's. No
documentation strings.

**Non-goals.** Rendering, commands, OCL, profiles, XMI import or export of models.
