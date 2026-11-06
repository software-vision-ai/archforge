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
operation_text, multiplicity, visibility, is_static, member_ends, navigable, MAX_CARD_ROWS}` (a class's card with its
members' visibility, type and multiplicity, abstract names slanted, static members underlined; an interface's and a data
type's the same under `«interface»` and `«dataType»`; a primitive type's `«primitive»` over its
name and no compartment; an enumeration's with
`«enumeration»` and one compartment of its literals; at most `MAX_CARD_ROWS` rows a card, the rest
counted in its `hidden`; an association's line with its
name, roles, multiplicities, an arrow at every end it navigates to and a diamond at the whole of
an aggregation or a composition; a generalization's triangle; a dependency dashed with an open
arrow; an interface realization dashed in the muted ink to a hollow triangle at its interface; an
association class's line an association's and its card a class's under `Association class`),
`constraints::{specs, constraints_of, context_of, constrained, expression, language, braces, is_context, is_constrainable,
in_model, expression_problem, suggested_form, form_for, shown_on, form_of, linked_nodes, show, dropped_by, Dropped, Form, BRACES, NOTE,
MAX_CONSTRAINED}` (AR-133: `uml.constraint.{add, set, move, show, list}`; a Constraint owned by a classifier of the model's own
or a package, naming the elements it constrains in order and stating its rule as an OpaqueExpression of one body and at most one
language, kept exactly as typed and never evaluated; the only refusals an empty expression and an element outside the model;
shown per diagram as a node of the kind `uml.constraint.braces`, nested in its element's node, or `uml.constraint.note`, linked
to its elements by `Notation::links`; removed through `element.remove`, and a constraint left naming nothing by a removal goes
with it in the same step), `generalization_sets::{specs, is_set, is_generalization, members, sets_of, general, specific,
general_of, sets_for, flags, braces, label, shared_general, Grouping, generalization_name, reconnect_problem, emptied_by,
label_node, is_label, place, set_json, LABEL, MAX_MEMBERS}` (AR-129: `uml.generalization-set.{add, set, place, list}`; a
GeneralizationSet in its general classifier's package, grouping generalizations that all specialize that classifier, each
named once and at most 256, complete or not (isCovering) and disjoint or not (isDisjoint); a generalization may be in several
sets; a member is never reconnected (`uml.relationship.reconnect` refuses it, naming its sets); removed through
`element.remove`, which keeps its generalizations, and a set left with no member by a removal, or by `uml.generalization-set.set`,
goes in the same step; where a diagram shows its label, once moved, a node of the kind `uml.generalization-set.label` nested in
the general classifier's node; the set line itself is the canvas's, worked out from the model), `nesting::{specs, nested,
nested_within, classifier_owner, owner_of, home_package, owner_chain, qualified_name, names_under, classifier_clash,
owner_problem, problem_text, candidates, can_own, can_nest, is_nested, OwnerProblem, OWNERS, NESTABLE, KINDS}` (AR-132:
`uml.nested.add`, `uml.classifier.set-owner`, `uml.nested.list`, `uml.classifier.owners`; a class, an interface, an
enumeration or a data type in a class's, an interface's or an association class's `nestedClassifier`, its home package its
owner's; an owner changed keeping the id, members, relationships and type references; a cycle, an owner of a kind that owns
none, a primitive type or an association class put in a classifier, and a name another classifier of the owner has, whatever
the case, refused with the reason; an attribute or an operation of the owner may share it (AR-208); removing an owner removes what
it nests, their relationships and views, in the same step; `UmlNotation::nested_in` names the owner the canvas ties a nested
card to with a line ending in ⊕, worked out from the model; a card of a nested classifier whose owner the diagram does not show
reads `Order::Priority`), `profiles::{specs, library, in_library, is_metaclass, metaclass, metaclass_kind, metaclass_problem, library_problem, profiles, all_profiles, package_of, stereotypes, profile_of, extensions, extended, properties, extension_ends, metaclasses_of, is_base, applied, applying, home_of, in_scope, available, extends_kind, element_of, applications_of, stereotype_uses, profile_uses, unapply_uses, moving_uses, removal_uses, extension_uses, values_set, uses_text, stranding, removal_problem, Extension, Application, Uses, UML_URI, METACLASSES, NOT_DRAWN, BASE, APPLICATION_LINK, MAX_STEREOTYPES}` (AR-134: `uml.profile.{add, rename, apply, unapply, diagram, list, uses}`, `uml.stereotype.{add, set, get}`, `uml.package.profiles`, `uml.metaclass.list`; a Profile in a package beside its packages, a profile's name a package's kind; its Stereotypes, each extending metaclasses as UML has it, an Extension in the profile whose ends are the stereotype's `base_<Metaclass>` attribute typed by the metaclass and an ExtensionEnd typed by the stereotype, and owning properties through the attribute commands, typed by a primitive type, an enumeration or a data type, the names starting `base_` kept; the metaclasses Classes of the hidden UML library, an unnamed UML Model at the project's top level known by `UML_URI`, each made once on first use and never changed, shown in no list or type; no `Profile::metaclassReference` is stored, since it subsets the composite `elementImport` and a project's rows would list each import twice, which the reader refuses: the extensions name the metaclasses; a ProfileApplication per package that applies a profile, its stereotypes then available inside the package at any depth; a stereotype application, which AR-135 makes, an InstanceSpecification classified by the stereotype with Slots for its values and a core Link to its element, found by `applications_of`, and every unapply, removal, dropped extension, stereotype move, element or package move and removal of a property with values set that would strand one refused, the uses named; a profile's diagram a class diagram of the profile, its stereotypes' cards `«stereotype»`, its metaclasses' `«metaclass»`, an extension's line ending in a filled triangle at the metaclass), `relations::{specs, relationships_of, generalization_problem, GeneralizationProblem, is_final, specializations, final_problem, is_leaf, redefiners, redefiner_name, leaf_problem, association_end_problem, association_class_end_problem, realization_problem, RealizationProblem}`
(`uml.classifier.set` (AR-137): a classifier of any kind made final, UML's isFinalSpecialization, only while nothing specializes it and never a built-in type, after which no generalization is drawn to it or has its general end moved onto it; and made leaf (AR-138), UML's isLeaf, only while nothing redefines it, `{leaf}` after its name, as a property is with the attribute commands and `uml.association.set-end` and an operation with the operation commands, after which no property redefines a leaf one (`modifiers::TargetProblem::Leaf`, `modifiers::redefines_leaf`); an enumeration specializes only an enumeration, a data type only a data type and a primitive type
only a primitive type, each specialized only by its own kind, a built-in type specializing nothing,
and none of them joins an association; `uml.realization.add`: a class
realizes an interface, owned by the class, its client and supplier kept in step with its
implementing classifier and contract; reconnected through `uml.relationship.reconnect` and removed
through `element.remove`; `uml.association-class.add` (AR-125): one AssociationClass between two
classes, owning both its ends as an association does, its ends set with `uml.association.set-end`,
its members with the member commands, renamed and moved with `uml.association-class.rename` and
`.move` among its package's classifiers, refused at anything but a class; nothing specializes it),
`templates::{specs, signature, parameters, is_template, is_parameter, template_of, element_of,
parameter_of, is_parameter_element, parameter_element_problem, type_scope_problem, kind_of,
constraint_of, default_of, parameter_name, parameter_text, signature_rows, name_clash,
substitutions, binding_of, bound_element, actual_of, users, bindings_to, removal_problem, fits,
parameter_json, parameters_json, can_be_template, Kind, TEMPLATES, MAX_PARAMETERS}` (AR-130:
`uml.template-parameter.{add, set, move, list}`; a class, an interface or a data type made a
template by its first ClassifierTemplateParameter, its RedefinableTemplateSignature made with the
first and removed with the last; each parameter owning a classifier of its kind (class, interface,
data type, enumeration or primitive type; value and operation refused as not supported yet, never
converted), named as the parameter, constrained to at most one classifier of its kind, with a
default of its kind that fits the constraint (UML's `isCompatibleWith`), `allowSubstitutable`
false; names told apart across the signature by the shared rule of `names`; its identity, and
what it types, kept across rename, reorder and a change of kind; that classifier types only its
template's own members (`members` refuses it elsewhere) and is never listed, placed or edited as
one of the model's; a parameter a binding's substitution names (`formal`) never removed, nor
changed to a kind or constraint the binding's argument would not fit, nor a template's last
parameter removed while any binding names its signature, nor a generalization or a classifier
removed when a surviving parameter's default or a binding's argument would then no longer specialize
its constraint (`constraint_problem`) (no command makes a binding yet; AR-131 brings them), all checked before anything is written
(`removals_problem`, `last_parameter_problem`); removed through `element.remove`; the card's `template` rows drawn as the signature box), `commands::going_with` (the relationships that go with classifiers, an association class's own
among them, and the bindings to a template), `names::{kind, clash, CLASSIFIER}` (AR-208: which elements may share a
name in one owner, as UML has it: two of one kind never, whatever the case; every classifier is one kind, a package, a
property, an operation, a parameter, a literal and a generalization set each one of their own, relationships and constraints
labels that never clash; the check a project gets as it opens and every command that names, adds or moves an element ask this
one rule), `qualifiers::{specs, qualifiers, qualified_end, is_qualifier, association_of, name_clash,
qualifier_text, qualifiers_json, MAX_QUALIFIERS}` (AR-127: `uml.qualifier.{add, set, move}`; UML's
`Property::qualifier`, an ordered list of properties an association's or an association class's end
owns, each with a name, a type and a multiplicity; the qualified end's box drawn at its class
(`figures::line` gives its rows in the end's `qualifiers`); a name told apart from its end's other
qualifiers whatever the case, at most 32 an end; never an attribute or a role; kept through renaming
and reconnecting; removed through `element.remove`), `modifiers::{read, apply, parse_default, default_text, braces, suffix, Shape,
subsets_candidates, redefines_candidates, redefines_leaf, subsets_problem, redefines_problem}` (AR-126: a
property's default value read by its type, derived, read only, ordered, unique, and the properties
it subsets and redefines, set by `uml.attribute.add`/`set` and `uml.association.set-end`, checked
against UML's rules before anything changes; the card's and the line's notation, `figures::end_role`
and `figures::end_text`), `RELATIONSHIPS`, `is_relationship` and `is_association`, and the card's `WORDS` and `PLURALS` for the shell's catalogue. Where an association's two
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
going with its element, a nested package's too, and back with undo. `tests/generalization_sets.rs` (AR-129): a set made, renamed, made complete and disjoint, given and
losing members, each one undo step; a mixed selection and the other wrong sets refused, changing nothing; a generalization
in two sets; a member's reconnection refused; a set emptied by a deletion going in the same step; its label placed on two
diagrams; rows read back identical; the sweep. `tests/templates.rs` (AR-130): parameters added, edited,
moved and removed, each one undo step, the signature made and gone with them; identity and the
members typed by a parameter kept across rename, reorder and kind; value and operation refused;
defaults and constraints checked by kind; a parameter's classifier kept the template's own; a
binding built in the store keeping its parameters; the box on two diagrams; rows read back
identical; the sweep. `tests/profiles.rs` (AR-134): profiles and stereotypes made, extending and owning properties, one undo step each; unsupported metaclasses and property types refused; a profile applied and unapplied; unapplying, removing and moving refused while stereotype applications built in the store use them, the uses counted; the library out of types and reads; a profile's names a package's; rows read back identical; the profile's diagram; the sweep. `tests/qualifiers.rs` (AR-127): qualifiers added, edited, moved and removed, each one undo
step; a duplicate name and the other wrong edits refused, changing nothing; a built-in type or the
model's own; kept apart from attributes and roles and gone with their association; kept through
renaming and reconnecting; the box on two diagrams; rows read back identical; the sweep. `tests/modifiers.rs` (AR-126):
each modifier set and cleared on an attribute and an end, one undo step each; defaults for every
built-in type, an enumeration and opaque text, and the type-change refusal; Subsets and Redefines
valid and refused; the card's and the line's notation; rows read back identical; the sweep.

**Known lacks.** No derived getters yet (`Classifier::general`, `MultiplicityElement::lower`, …
read as Derived until a notation writes them). No hand-written rules beyond the core's. No
documentation strings.

**Non-goals.** Rendering, commands, OCL, XMI import or export of models.
