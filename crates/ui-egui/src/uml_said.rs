//! The UML commands' refusals in the user's words (AR-786): the typed reasons only UML's commands
//! give, each in the words of the editor or the canvas that meets it (relationships, templates and
//! bindings, generalization sets and association ends, profiles and stereotypes, constraints, what
//! a property subsets or redefines, defaults, packages and their default diagrams), and the reasons
//! a UML command is not available now, and the kinds of element only UML's commands name in a
//! refusal (AR-785). A reason the engine, the diagrams or ER give too is the
//! shell's own (`panels::said`, `engine_said`).

use archforge_engine::{BindingFault, Reason};

use crate::i18n::{fmt, tr, tr_ctx, trn};

/// A kind of UML element as a refusal names it, with its article where the language has one, as
/// `archforge_uml`'s commands alone type it; none for another kind.
pub(crate) fn a_kind(kind: &str) -> Option<String> {
    Some(match kind {
        "class" => tr_ctx("refusal", "a class"),
        "association class" => tr_ctx("refusal", "an association class"),
        "association" => tr_ctx("refusal", "an association"),
        "generalization" => tr_ctx("refusal", "a generalization"),
        "interface" => tr_ctx("refusal", "an interface"),
        "enumeration" => tr_ctx("refusal", "an enumeration"),
        "data type" => tr_ctx("refusal", "a data type"),
        "primitive type" => tr_ctx("refusal", "a primitive type"),
        "package" => tr_ctx("refusal", "a package"),
        "profile" => tr_ctx("refusal", "a profile"),
        "stereotype" => tr_ctx("refusal", "a stereotype"),
        "attribute" => tr_ctx("refusal", "an attribute"),
        "operation" => tr_ctx("refusal", "an operation"),
        "literal" => tr_ctx("refusal", "a literal"),
        "qualifier" => tr_ctx("refusal", "a qualifier"),
        "template parameter" => tr_ctx("refusal", "a template parameter"),
        "generalization set" => tr_ctx("refusal", "a generalization set"),
        "association end" => tr_ctx("refusal", "an association end"),
        _ => return None,
    })
}

/// A refusal only UML's commands give, in the user's words; none for another reason or for a kind
/// of element the catalogues don't name, which the engine's sentence then says.
pub(crate) fn said(reason: &Reason) -> Option<String> {
    let types = archforge_uml::members::TYPES;
    Some(match reason {
        Reason::DependsOnItself { kind } => {
            let template = tr("{what} cannot depend on itself.");
            let what = crate::panels::a_kind(kind)?;
            // A sentence that starts with the kind starts with a capital, as "A class cannot ...".
            let what = if template.starts_with("{what}") {
                what.chars().take(1).flat_map(char::to_uppercase).chain(what.chars().skip(1)).collect()
            } else {
                what
            };
            fmt(&template, &[("what", &what)])
        }
        Reason::PackageIntoItself => tr("A package cannot go into itself or a package it holds."),
        Reason::InUse(used) => crate::uml_profiles::in_use(used),
        Reason::UmlOwn { metaclass: None } => tr("The UML library holds the metaclasses that stereotypes extend. It never changes."),
        Reason::UmlOwn { metaclass: Some(name) } => {
            fmt(&tr("{name} is a UML metaclass, kept in the UML library. It never changes."), &[("name", name)])
        }
        Reason::RootPackageStays => tr("The model's root package holds all the other packages, so it stays where it is."),
        Reason::TypesPackageKept => fmt(
            &tr("The package {types} keeps the built-in types: while it holds them, it keeps its name and stays under the root package."),
            &[("types", types)],
        ),
        Reason::TypesPackageExists => {
            fmt(&tr("There is a package {types} already, which keeps the built-in types; a second one can't sit beside it."), &[("types", types)])
        }
        Reason::HoldsBuiltInName { kind, name } => fmt(
            &tr("The package holds {what} named {name}, a built-in type's name, which in {types} would mean that type. Rename {name} first."),
            &[("what", &crate::panels::a_kind(kind)?), ("name", name), ("types", types)],
        ),
        Reason::InvalidBinding(BindingFault::Missing) => {
            tr("The package has class diagrams, but none is its default. Choose one of them as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::Duplicate) => {
            tr("The package has more than one default diagram. Choose the one of its class diagrams that stays its default.")
        }
        Reason::InvalidBinding(BindingFault::DeadTarget) => {
            tr("The package's default diagram no longer exists. Choose one of its class diagrams as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::WrongKind) => {
            tr("The package's default diagram is not a class diagram. Choose one of its class diagrams as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::WrongOwner) => {
            tr("The package's default diagram belongs to another element. Choose one of its own class diagrams as its default diagram.")
        }
        Reason::ConnectionExists { dependency } => {
            fmt(&tr("This connection already exists: the dependency {dependency}. Select the existing relationship."), &[("dependency", dependency)])
        }
        Reason::StaleChoice { candidates } if candidates.is_empty() => {
            tr("The chosen relationship does not join these two elements, or no longer does, and none does now. Nothing was changed.")
        }
        Reason::StaleChoice { .. } => {
            tr("The chosen relationship does not join these two elements, or no longer does. Choose again among those that do. Nothing was changed.")
        }
        Reason::IdEmpty { kind: "package" } => tr("No package is given. Give its id."),
        Reason::NotA { kind: "package", name } => fmt(&tr("{name} is not a package."), &[("name", name)]),
        Reason::NotOwnDiagram { diagram, package } => fmt(
            &tr("{diagram} is not one of the class diagrams of {package}. Choose one of its own class diagrams as its default diagram."),
            &[("diagram", diagram), ("package", package)],
        ),
        Reason::NoDiagramYet { package } => {
            fmt(&tr("{package} has no class diagram yet, so there is nothing to open. Create a class diagram for it first."), &[("package", package)])
        }
        Reason::ParentDefaultUnsound { parent, fault: None } => fmt(
            &tr("{parent} has no diagram yet to take this package's diagrams. Create one for it first. Nothing was changed."),
            &[("parent", parent)],
        ),
        Reason::ParentDefaultUnsound { parent, fault: Some(_) } => fmt(
            &tr("{parent} has no valid default diagram to take this package's diagrams. Choose its default first. Nothing was changed."),
            &[("parent", parent)],
        ),
        other => {
            return relationship_said(other)
                .or_else(|| template_said(other))
                .or_else(|| set_and_end_said(other))
                .or_else(|| profile_said(other))
                .or_else(|| constraint_said(other))
                .or_else(|| target_said(other));
        }
    })
}

/// Why a UML command is not available now, in the user's words, by the reason its availability
/// check gives; none for another reason.
pub(crate) fn disabled(reason: &str) -> Option<String> {
    use archforge_uml::{
        applications, bindings, commands, constraints, generalization_sets, members, nary, nesting, profiles, qualifiers, templates,
    };
    Some(match reason {
        commands::NO_PACKAGES => tr("There is no package yet."),
        members::NO_CLASSES => tr("There is no class, interface, enumeration, data type or primitive type yet."),
        members::NO_CLASS => tr("There is no class yet."),
        members::NO_ENUMERATIONS => tr("There is no enumeration yet."),
        nesting::NO_OWNERS => tr("There is no class or interface yet."),
        qualifiers::NO_ASSOCIATIONS => tr("There is no association or association class yet."),
        nary::NO_NARY => tr("There is no n-ary association yet."),
        generalization_sets::NO_GENERALIZATIONS => tr("There is no generalization yet."),
        generalization_sets::NO_SETS => tr("There is no generalization set yet."),
        constraints::NO_CONTEXTS => tr("There is no class, interface, enumeration, data type, primitive type, association class or package yet."),
        constraints::NO_CONSTRAINTS => tr("There is no constraint yet."),
        templates::NO_TEMPLATES => tr("There is no class, interface or data type yet."),
        templates::NO_PARAMETERS => tr("There is no template parameter yet."),
        bindings::NO_TEMPLATES => tr("There is no template yet: give a class, an interface or a data type a template parameter first."),
        bindings::NO_BINDINGS => tr("There is no template binding yet."),
        profiles::NO_PROFILES => tr("There is no profile yet."),
        profiles::NO_STEREOTYPES => tr("There is no stereotype yet."),
        applications::NO_APPLICATIONS => tr("No stereotype is applied yet."),
        _ => return None,
    })
}

/// A constraint's rule broken, a description's comments too many to find it, or a constraint
/// command's parameter refused, in the constraint editor's own words where it has them; none for
/// another reason.
fn constraint_said(reason: &Reason) -> Option<String> {
    Some(match reason {
        Reason::IdEmpty { kind: "constraint" } => tr("No constraint is given. Give its id."),
        Reason::NotA { kind: "constraint", name } => fmt(&tr("{name} is not a constraint of this model."), &[("name", name)]),
        Reason::IdsExpected { kind: "constrained element" } => tr("Give the elements as a list of ids."),
        Reason::NoContext => tr("This constraint is among no element's constraints."),
        Reason::NoPlace { kind: "constraint", at, last } => fmt(
            &tr("There is no place {at} among the constraints. Give a place from 0 to {last}, or -1 for the last."),
            &[("at", &at.to_string()), ("last", &last.to_string())],
        ),
        Reason::OneCoordinate => tr("Give both x and y, or neither to place it where it reads."),
        Reason::BracesNoElement => tr("Braces sit by an element they constrain, and this diagram shows none of them. Show it as a linked note."),
        Reason::NotAContext => tr("Only a classifier of the model's own or a package owns constraints."),
        Reason::NotConstrainable => tr("A constraint names only elements of this model."),
        Reason::TooMany { kind: "constrained element", max } => fmt(&tr("A constraint names at most {max} elements."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: "constrained element", .. } => tr("An element is named twice. Name each element once."),
        Reason::TooManyComments { max } => fmt(
            &tr("This element owns {max} comments or more, so its description can't be found. Remove some in the file first."),
            &[("max", &max.to_string())],
        ),
        _ => return None,
    })
}

/// A rule broken by the properties a property subsets or redefines, as a list of ids or each
/// property named, by an element made leaf, or by a property's default; none for another reason.
fn target_said(reason: &Reason) -> Option<String> {
    const SUBSETTED: &str = "subsetted property";
    const REDEFINED: &str = "redefined property";
    Some(match reason {
        Reason::TooMany { kind: SUBSETTED, max } => fmt(&tr("A property subsets at most {max} properties."), &[("max", &max.to_string())]),
        Reason::TooMany { kind: REDEFINED, max } => fmt(&tr("A property redefines at most {max} properties."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: SUBSETTED | REDEFINED, .. } => tr("An element is named twice. Name each element once."),
        Reason::IdsExpected { kind: SUBSETTED | REDEFINED } => tr("Give the properties as a list of ids."),
        Reason::IdEmpty { kind: SUBSETTED | REDEFINED } => tr("An item in the list has no id. Give each property's id."),
        Reason::NotAnItemKey { key } => fmt(&tr("{key} is not a key here. An item holds an id and a name."), &[("key", key)]),
        Reason::BuiltInNotLeaf { name } => fmt(&tr("{name} is a built-in type, which is never made leaf."), &[("name", name)]),
        Reason::Redefined(names) => crate::uml_panel::leaf_refusal(names),
        Reason::TargetBreaks { name, rule, redefining } => crate::uml_panel::target_refusal(name, *rule, *redefining),
        other => return crate::uml_panel::default_said(other),
    })
}

/// A profile's or a stereotype's rule broken; none for a reason or a kind the catalogues don't say.
fn profile_said(reason: &Reason) -> Option<String> {
    let offered = || archforge_uml::profiles::METACLASSES.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(", ");
    Some(match reason {
        Reason::ProfileApplied { package, profile } => {
            fmt(&tr("{package} applies {profile} already."), &[("package", package), ("profile", profile)])
        }
        Reason::ProfileNotApplied { package, profile } => {
            fmt(&tr("{package} does not apply {profile}."), &[("package", package), ("profile", profile)])
        }
        Reason::TooMany { kind: "stereotype", max } => fmt(&tr("A profile holds at most {max} stereotypes."), &[("max", &max.to_string())]),
        Reason::TooMany { kind: "metaclass", max } => fmt(&tr("A stereotype extends at most {max} metaclasses."), &[("max", &max.to_string())]),
        Reason::MetaclassNotDrawn { name } => fmt(
            &tr("{name} is a UML metaclass Archforge does not draw yet. A stereotype extends {metaclasses}."),
            &[("name", name), ("metaclasses", &offered())],
        ),
        Reason::NotAMetaclass { name } => {
            fmt(&tr("{name} is not a metaclass Archforge draws. A stereotype extends {metaclasses}."), &[("name", name), ("metaclasses", &offered())])
        }
        Reason::NamedTwice { kind: "metaclass", name } => fmt(&tr("{name} is named twice. Name each metaclass once."), &[("name", name)]),
        Reason::NamesExpected { kind: "metaclass" } => tr("Give the metaclasses as a list of names."),
        Reason::InNoProfile => tr("This stereotype is in no profile."),
        Reason::ExtensionEnd { name } => {
            fmt(&tr("{name} is a stereotype's extension end. Drop the metaclass from what the stereotype extends instead."), &[("name", name)])
        }
        Reason::ValuesSet { name, elements } => {
            let n = u64::try_from(*elements).ok()?;
            let set = trn("{name} has values on {n} element. Remove those first.", "{name} has values on {n} elements. Remove those first.", n);
            fmt(&set, &[("name", name)])
        }
        other => return crate::uml_stereotypes::said(other),
    })
}

/// A relationship's rule broken, in the words the canvas says it with; none for a reason or a kind
/// the catalogues don't say.
fn relationship_said(reason: &Reason) -> Option<String> {
    Some(match reason {
        Reason::SpecializesOnlyItsKind { kind } => match *kind {
            "interface" => tr("An interface specializes only an interface"),
            "enumeration" => tr("An enumeration specializes only an enumeration"),
            "data type" => tr("A data type specializes only a data type"),
            "primitive type" => tr("A primitive type specializes only a primitive type"),
            _ => return None,
        },
        Reason::OnlyItsKindSpecializes { kind } => match *kind {
            "enumeration" => tr("Only an enumeration specializes an enumeration"),
            "data type" => tr("Only a data type specializes a data type"),
            "primitive type" => tr("Only a primitive type specializes a primitive type"),
            _ => return None,
        },
        Reason::CannotSpecialize { kind: "class" | "association class", general: "interface" } => tr("A class cannot specialize an interface"),
        Reason::SpecializesNothing { kind: "built-in type" } => tr("A built-in type specializes nothing"),
        Reason::NothingSpecializes { kind: "association class" } => tr("Nothing specializes an association class"),
        Reason::Final { name } => {
            fmt(&tr("{name} is final: nothing may specialise it. Clear \"No subclasses\" in its Properties first."), &[("name", name)])
        }
        Reason::Itself { relationship: "generalization", .. } => tr("A classifier cannot specialize itself"),
        Reason::Itself { relationship: "association class", .. } => tr("An association class does not join itself"),
        Reason::Again { relationship: "generalization" } => tr("This one specializes that one already"),
        Reason::Again { relationship: "realization" } => tr("This class realizes that interface already"),
        Reason::Cycle { relationship: "generalization" } => tr("That one specializes this one already, which would make a cycle"),
        Reason::WrongEnd { relationship: "binding", end: archforge_engine::End::Source, .. } => {
            tr("Only a class, an interface or a data type binds a template")
        }
        Reason::WrongEnd { relationship: "binding", end: archforge_engine::End::Target, .. } => {
            tr("A binding ends at a template: a class, an interface or a data type with template parameters")
        }
        Reason::KindsDiffer { relationship: "binding" } => tr("A template is bound by a classifier of its own kind"),
        Reason::Itself { relationship: "binding", .. } => tr("A template does not bind itself"),
        Reason::Again { relationship: "binding" } => tr("This one binds that template already"),
        Reason::Cycle { relationship: "binding" } => tr("That template binds this one already, which would make a cycle"),
        Reason::WrongEnd { relationship: "association", kind, .. } => match *kind {
            "enumeration" => tr("An enumeration is a data type and joins no association"),
            "data type" => tr("A data type joins no association"),
            "primitive type" => tr("A primitive type joins no association"),
            _ => return None,
        },
        Reason::WrongEnd { relationship: "association class", .. } => tr("An association class joins only classes"),
        Reason::WrongEnd { relationship: "realization", end: archforge_engine::End::Source, .. } => tr("Only a class implements an interface"),
        Reason::WrongEnd { relationship: "realization", end: archforge_engine::End::Target, .. } => tr("A class realizes only an interface"),
        _ => return None,
    })
}

/// A template rule broken, in the words the template and binding editors say it with; none for a
/// reason or a kind the catalogues don't say.
fn template_said(reason: &Reason) -> Option<String> {
    use archforge_uml::templates::Kind;
    let kind = |noun: &str| Kind::ALL.into_iter().find(|k| k.noun() == noun);
    Some(match reason {
        Reason::KindNotSupported { kind: k } => {
            fmt(&tr("{kind} template parameters are not supported yet."), &[("kind", &crate::uml_templates::kind_word(kind(k)?))])
        }
        Reason::TooMany { kind: "template parameter", max } => fmt(&tr("A template has at most {max} parameters."), &[("max", &max.to_string())]),
        Reason::NeedsArgument(params) => {
            crate::uml_bindings::lacking(&params.iter().map(|p| Some((p.name.as_str(), kind(p.kind)?))).collect::<Option<Vec<_>>>()?)?
        }
        Reason::Misfit(p) => crate::uml_templates::must(&p.name, kind(p.kind)?, p.constraint.as_deref()),
        Reason::DefaultMisfit(p) => {
            fmt(&tr("{default} does not fit {name}: pick a default its arguments could be."), &[("default", &p.argument), ("name", &p.name)])
        }
        Reason::NeedsDefault(p) => fmt(
            &tr("{bound} binds {template} and gives {name} no argument, so {name} needs a default."),
            &[("bound", &p.bound), ("template", &p.template), ("name", &p.name)],
        ),
        Reason::WouldNotFit(p) => fmt(
            &tr("{bound} binds {name} -> {actual}, which would no longer fit. Change or remove that binding first."),
            &[("bound", &p.bound), ("name", &p.name), ("actual", &p.argument)],
        ),
        Reason::ParameterInUse(p) => fmt(
            &tr("{bound} binds {name} -> {actual}. Change or remove that binding first, then remove {name}."),
            &[("bound", &p.bound), ("name", &p.name), ("actual", &p.argument)],
        ),
        Reason::LastParameter(p) => fmt(
            &tr("{bound} binds {template}, and {name} is its last parameter. Remove that binding first, then remove {name}."),
            &[("bound", &p.bound), ("template", &p.template), ("name", &p.name)],
        ),
        Reason::BuiltInTaken { kind, name } => fmt(
            &tr("The package {types} holds {what} named {name}, where that built-in type is kept: rename it to use the built-in type."),
            &[("what", &crate::panels::a_kind(kind)?), ("name", name), ("types", archforge_uml::members::TYPES)],
        ),
        _ => return None,
    })
}

/// A generalization set's or an association end's rule broken, in the words the set menu and the
/// end editor say it with; none for a reason or a kind the catalogues don't say.
fn set_and_end_said(reason: &Reason) -> Option<String> {
    Some(match reason {
        Reason::TooFew { kind: "generalization", min: 1 } => tr("A set groups at least one generalization."),
        Reason::TooMany { kind: "generalization", max } => {
            trn("A set groups at most {n} generalization.", "A set groups at most {n} generalizations.", u64::try_from(*max).ok()?)
        }
        Reason::ParentsDiffer(ends) => crate::uml_sets::parents_differ(ends),
        Reason::TooFew { kind: "association end", min: 3 } => tr("An n-ary association needs three ends or more."),
        Reason::TooMany { kind: "association end", max } => fmt(&tr("An association takes at most {n} ends"), &[("n", &max.to_string())]),
        Reason::OtherEndAggregates => tr("The other end has an aggregation already: only the part's end has one."),
        Reason::WholeAllowsMany => tr("A composition's whole allows one at most: set the other end's multiplicity to 1 or 0..1 first."),
        Reason::WholeAllowsOne => tr("This end is a composition's whole, which allows one at most."),
        Reason::WholeNavigable => tr("The whole's end of an aggregation or composition is not navigable."),
        Reason::NaryAggregates => tr("An n-ary association's ends are not aggregations: only a binary association has a whole and a part."),
        _ => return None,
    })
}
