# archforge-uml

**Purpose.** The UML 2.5.1 metamodel as a metamodel of the MOF core: 242 metaclasses (193
concrete), 622 class-owned properties (73 derived, without a field), 13 enumerations and the five
primitive types, generated from OMG's `UML.xmi` by `cargo xtask generate uml`. Only `lib.rs` is
written by hand.

**Layer.** 7, the notations' layer above the MOF core (4), the shared core (5) and the diagrams (6),
whose project view it joins as `UmlNotation` (class diagrams of a package, classes as plain cards
with their attributes and operations) and whose commands it adds to (`commands`: `uml.package.{add,
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
`CLASS_DIAGRAM`, `CLASS_SIZE`, `registry`, `commands::{specs, packages, classes, package_of,
remove_elements, MAX_NAME}`, `members::{specs, attributes, operations, parameters,
parse_multiplicity, BUILT_IN, TYPES, MAX_PARAMETERS, MAX_BOUND}` (`uml.class.{set, get}`,
`uml.attribute.{add, set, move}`, `uml.operation.{add, set, move}`, `uml.type.list`; a type is an
element's id or a built-in type's name, Integer, String, Boolean, Real or UnlimitedNatural, made in
the package Types the first time it is used; a multiplicity is typed, `1`, `0..1`, `*`, `1..*`,
`2..5`; members are removed through `element.remove`), `figures::{card, line, ends, text_slots, slot_text, attribute_text,
operation_text, multiplicity, visibility, member_ends, navigable}` (a class's card with its
members' visibility, type and multiplicity, abstract names slanted; an association's line with its
name, roles, multiplicities, an arrow at an end only it navigates to and a diamond at the whole of
an aggregation or a composition; a generalization's triangle; a dependency dashed with an open
arrow), and the card's `WORDS` and `PLURALS` for the shell's catalogue. Where an association's two
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
the million-object test (release only).

**Known lacks.** No derived getters yet (`Classifier::general`, `MultiplicityElement::lower`, …
read as Derived until a notation writes them). No hand-written rules beyond the core's. No
documentation strings.

**Non-goals.** Rendering, commands, OCL, profiles, XMI import or export of models.
