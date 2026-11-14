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
    /// The stereotype property `name` has values set on this many elements, so it stays.
    ValuesSet { name: String, elements: usize },
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
    /// A constraint's expression is empty or holds only spaces, so it states no rule.
    NoExpression,
    /// The element owns no constraint: a classifier of the model's own or a package does.
    NotAContext,
    /// The element is none a constraint names: an element of the model's own is.
    NotConstrainable,
    /// A constraint shown as braces sits by an element it names, and the diagram shows none of them.
    BracesNoElement,
    /// The element is of a kind that takes no description.
    TakesNoDescription,
    /// The element owns `max` comments or more, so its description, looked for among the first
    /// `max` alone, is neither set nor cleared.
    TooManyComments { max: usize },
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

/// What is in use in a [`Reason::InUse`], each by its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Used {
    /// A profile or a stereotype, which would go.
    Element(String),
    /// A stereotype, which would move to another profile.
    Moved(String),
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
