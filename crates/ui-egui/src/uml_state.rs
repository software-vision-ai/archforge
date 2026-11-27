//! What UML keeps between frames for its own editors, menus, drawings and tools: the Properties
//! editors of a member, a constraint, a qualifier, a template parameter, an extension or applied
//! profile, an n-ary end and an association's ends, the refusals said under their sections, the
//! Stereotypes section, a template binding before it is written and the type menus' choices; and
//! on the canvas a member typed in place, the generalization lines' and n-ary associations' menus,
//! an n-ary association being drawn, what the Relationship and classifier tools make, and package
//! navigation's ways back and landings. The
//! shell holds it as one notation's state among others ([`crate::notation_ui::NotationState`]);
//! UML's screens reach it through [`uml`] and [`uml_mut`].

use archforge_mof::{Obj, Store, Uuid};
use egui::Pos2;

use crate::notation_ui::{NotationState, NotationStates};
use crate::panels::{Editor, MenuId, OpenEditor};

/// A class member editor's card.
pub(crate) const MEMBER: Editor = Editor { notation: archforge_uml::INDEX, card: "member" };
/// A constraint editor's card.
pub(crate) const CONSTRAINT: Editor = Editor { notation: archforge_uml::INDEX, card: "constraint" };
/// A qualifier editor's card.
pub(crate) const QUALIFIER: Editor = Editor { notation: archforge_uml::INDEX, card: "qualifier" };
/// A template parameter editor's card.
pub(crate) const TEMPLATE: Editor = Editor { notation: archforge_uml::INDEX, card: "template" };
/// An extension's or an applied profile's editor's card.
pub(crate) const PROFILE: Editor = Editor { notation: archforge_uml::INDEX, card: "profile" };
/// An n-ary association's end editor's card.
pub(crate) const NARY_END: Editor = Editor { notation: archforge_uml::INDEX, card: "nary-end" };

/// UML's menus in the shell's one open-menu slot, each by its key among UML's: only UML's screens
/// open, recognise and close them. A menu shown once per row is the first of its kind here, and
/// [`MenuId::at`] names the row's.
pub(crate) mod menus {
    use crate::panels::MenuId;

    /// The menu `key` of UML.
    const fn uml(key: &'static str) -> MenuId {
        MenuId::of(archforge_uml::INDEX, key)
    }

    /// A class member editor's visibility, its type or return type, and a parameter's type, by the
    /// parameter.
    pub(crate) const MEMBER_VISIBILITY: MenuId = uml("member-visibility");
    pub(crate) const MEMBER_TYPE: MenuId = uml("member-type");
    pub(crate) const PARAM_TYPE: MenuId = uml("param-type");
    /// An attribute editor's default literal (for an enumeration's type), Subsets and Redefines
    /// (AR-126).
    pub(crate) const MEMBER_DEFAULT: MenuId = uml("member-default");
    pub(crate) const MEMBER_SUBSETS: MenuId = uml("member-subsets");
    pub(crate) const MEMBER_REDEFINES: MenuId = uml("member-redefines");
    /// The type menu of a member typed in place on its class's card.
    pub(crate) const INLINE_TYPE: MenuId = uml("inline-type");
    /// An association end's aggregation and its multiplicity's presets, by the end.
    pub(crate) const END_AGGREGATION: MenuId = uml("end-aggregation");
    pub(crate) const END_MULTIPLICITY: MenuId = uml("end-multiplicity");
    /// An association end's default literal (for an enumeration's type), Subsets and Redefines, by
    /// the end (AR-126).
    pub(crate) const END_DEFAULT: MenuId = uml("end-default");
    pub(crate) const END_SUBSETS: MenuId = uml("end-subsets");
    pub(crate) const END_REDEFINES: MenuId = uml("end-redefines");
    /// A qualifier editor's type (AR-127).
    pub(crate) const QUALIFIER_TYPE: MenuId = uml("qualifier-type");
    /// A template parameter editor's kind, constraint and default (AR-130).
    pub(crate) const TEMPLATE_KIND: MenuId = uml("template-kind");
    pub(crate) const TEMPLATE_CONSTRAINT: MenuId = uml("template-constraint");
    pub(crate) const TEMPLATE_DEFAULT: MenuId = uml("template-default");
    /// A template binding's bound element, template, and a parameter's argument, by the parameter
    /// (AR-131).
    pub(crate) const BINDING_BOUND: MenuId = uml("binding-bound");
    pub(crate) const BINDING_TEMPLATE: MenuId = uml("binding-template");
    pub(crate) const BINDING_ARGUMENT: MenuId = uml("binding-argument");
    /// An n-ary association's end editor's type and multiplicity (AR-128).
    pub(crate) const NARY_END_TYPE: MenuId = uml("nary-end-type");
    pub(crate) const NARY_END_MULTIPLICITY: MenuId = uml("nary-end-multiplicity");
    /// A classifier's owner, a package or a classifier (AR-132), and the kinds its Nested
    /// classifiers `+` offers.
    pub(crate) const CLASS_OWNER: MenuId = uml("class-owner");
    pub(crate) const NESTED_KIND: MenuId = uml("nested-kind");
    /// The package a package sits in (AR-102).
    pub(crate) const PACKAGE_PARENT: MenuId = uml("package-parent");
    /// A generalization set's Add member, and a generalization's sets' `+` (AR-129).
    pub(crate) const SET_MEMBER: MenuId = uml("set-member");
    pub(crate) const SET_JOIN: MenuId = uml("set-join");
    /// A constraint editor's language presets and the elements it may name (AR-133).
    pub(crate) const CONSTRAINT_LANGUAGE: MenuId = uml("constraint-language");
    pub(crate) const CONSTRAINT_ELEMENT: MenuId = uml("constraint-element");
    /// A stereotype's profile, its Extends `+` and an extension editor's metaclass, and a
    /// package's Applied profiles `+` (AR-134).
    pub(crate) const STEREOTYPE_PROFILE: MenuId = uml("stereotype-profile");
    pub(crate) const STEREOTYPE_EXTENDS: MenuId = uml("stereotype-extends");
    pub(crate) const EXTENSION_METACLASS: MenuId = uml("extension-metaclass");
    pub(crate) const APPLIED_PROFILE: MenuId = uml("applied-profile");
    /// The Stereotypes section's picker, and a property's value select by its place among the
    /// section's properties (AR-135).
    pub(crate) const STEREOTYPE_PICK: MenuId = uml("stereotype-pick");
    pub(crate) const STEREOTYPE_VALUE: MenuId = uml("stereotype-value");
}

/// UML's editors, menus, drawings and tools between frames.
#[derive(Debug, Default)]
pub(crate) struct UmlState {
    /// A type menu's choices as built for one state of one project (its model root and its
    /// revision), each label with the value the commands take: the model is walked for them once
    /// per change, not on every frame the menu is open.
    pub type_choices: Option<crate::uml_panel::TypeChoices>,
    /// The properties a property's Subsets and Redefines offer, each list built once for one state
    /// of the project and one shape of the property being edited, not on every frame (AR-126).
    pub targets: Vec<crate::uml_panel::TargetChoices>,
    /// The classifier No subclasses (final) was refused on, while something specializes it: the
    /// refusal shows under the check box, naming those classifiers (AR-137).
    pub final_refused: Option<Uuid>,
    /// The classifier No redefinition (leaf) was refused on, while something redefines it: the
    /// refusal shows under the check box, naming what does (AR-138).
    pub leaf_refused: Option<Uuid>,
    /// A generalization set just made: its name field takes the keyboard once (AR-129).
    pub set_name_focus: Option<Uuid>,
    /// A reconnection refused on a generalization in a set, said in its Properties (AR-129).
    pub set_refused: Option<(Uuid, String)>,
    /// An extension's or an applied profile's editor, and a change refused for its uses, said
    /// under its section (AR-134).
    pub profile_edit: Option<crate::uml_profiles::ProfileEdit>,
    pub profile_refused: Option<crate::uml_profiles::ProfileRefusal>,
    /// The Stereotypes section: a value being typed, the picker's filter and a refusal (AR-135).
    pub stereotypes: crate::uml_stereotypes::Stereotypes,
    /// A class's attribute or operation editor.
    pub member: Option<crate::uml_panel::MemberEdit>,
    /// A classifier's or a package's constraint editor (AR-133).
    pub constraint: Option<crate::uml_constraints::ConstraintEdit>,
    /// An association's ends' roles and multiplicities as typed.
    pub ends: Option<crate::uml_relate::EndsEdit>,
    /// An association end's qualifier editor (AR-127).
    pub qualifier: Option<crate::uml_qualifiers::QualifierEdit>,
    /// A classifier's template parameter editor, and a removal refused while a binding uses the
    /// parameter (AR-130).
    pub template: Option<crate::uml_templates::TemplateEdit>,
    pub template_in_use: Option<crate::uml_templates::InUse>,
    /// A template binding before it is written: a new one drawn on the canvas, or another template
    /// picked for a binding (AR-131).
    pub binding: Option<crate::uml_bindings::BindingDraft>,
    /// An n-ary association's end editor, and the question removing one of three ends asks
    /// (AR-128).
    pub nary_end: Option<crate::uml_nary::EndEdit>,
    pub nary_remove: Option<crate::uml_nary::RemoveAsk>,
    /// A class's attribute or operation typed in place on its card.
    pub inline: Option<crate::uml_inline::InlineMember>,
    /// The context menu of selected generalization lines, open at a screen point (AR-129).
    pub line_menu: Option<Pos2>,
    /// An n-ary association being drawn, or a class click awaited for a new end (AR-128).
    pub nary: Option<crate::uml_nary::Nary>,
    /// The context menu of an n-ary association's diamond or leg, at a screen point (AR-128).
    pub nary_menu: Option<(Pos2, Uuid)>,
    /// What a class diagram's Relationship tool draws.
    pub relation: crate::uml_relate::Relate,
    /// What a class diagram's classifier tool places: kept while the window lives, a new window
    /// starting at Class.
    pub classifier: crate::uml_classifier::Classifier,
    /// Package navigation between a package's diagram and its parent's: each tab's way back and a
    /// landing still to be shown (AR-95).
    pub nav: crate::package_nav::Nav,
}

/// UML's state among the shell's; none while UML is not one of its notations.
pub(crate) fn uml(states: &NotationStates) -> Option<&UmlState> {
    states.own()
}

/// UML's state among the shell's, to change; none while UML is not one of its notations.
pub(crate) fn uml_mut(states: &mut NotationStates) -> Option<&mut UmlState> {
    states.own_mut()
}

impl NotationState for UmlState {
    /// Drops an editor, a draft or a refusal whose element is gone, an association's ends once the
    /// association is, and a drawing that no longer holds on the diagram `canvas` shows
    /// ([`crate::uml_nary::retain`]).
    fn retain_valid(&mut self, store: &Store, canvas: &crate::canvas::State) {
        crate::uml_panel::retain_valid(&mut self.member, store);
        crate::uml_constraints::retain_valid(&mut self.constraint, store);
        crate::uml_qualifiers::retain_valid(&mut self.qualifier, store);
        crate::uml_templates::retain_valid(&mut self.template, &mut self.template_in_use, store);
        if self.profile_edit.as_ref().is_some_and(|e| store.by_uuid(e.owner).is_none()) {
            self.profile_edit = None;
        }
        if self.profile_refused.as_ref().is_some_and(|r| store.by_uuid(r.owner).is_none()) {
            self.profile_refused = None;
        }
        crate::uml_bindings::retain_valid(&mut self.binding, store);
        crate::uml_nary::retain_valid(self, store);
        crate::uml_stereotypes::retain_valid(&mut self.stereotypes, store);
        if self.ends.as_ref().is_some_and(|e| store.by_uuid(e.association).is_none()) {
            self.ends = None;
        }
        crate::uml_nary::retain(self, canvas, store);
        crate::uml_inline::retain_valid(&mut self.inline, store);
    }

    /// An editor card open, a binding drafted, the question removing an n-ary end asks, a
    /// stereotype's value or an association's ends typed and not written, a member typed in place,
    /// or an n-ary association being drawn. An association's ends count only while they hold
    /// something not yet written, so an open Modifiers fold stops nothing; a selection moved on has
    /// ended them ([`crate::uml_relate::finish_ends`]).
    fn editing(&self) -> bool {
        self.open_editors().iter().any(|(_, open)| open.is_some())
            || self.binding.is_some()
            || self.nary_remove.is_some()
            || self.stereotypes.value.is_some()
            || self.ends.as_ref().is_some_and(crate::uml_relate::EndsEdit::unwritten)
            || self.inline.is_some()
            || self.nary.is_some()
    }

    /// The member, constraint, qualifier, template parameter, profile and n-ary end editors: where
    /// each edits (a member's list is its kind, a qualifier's and an n-ary end's their end) and its
    /// refusal.
    fn open_editors(&self) -> Vec<(Editor, OpenEditor)> {
        vec![
            (MEMBER, self.member.as_ref().map(|e| ((e.target, e.kind as usize), e.refused.clone()))),
            (CONSTRAINT, self.constraint.as_ref().map(|e| ((e.target, 0), e.refused.clone()))),
            (QUALIFIER, self.qualifier.as_ref().map(|e| ((e.target, e.end), e.refused.clone()))),
            (TEMPLATE, self.template.as_ref().map(|e| ((e.target, 0), e.refused.clone()))),
            (PROFILE, self.profile_edit.as_ref().map(|e| ((e.target.as_deref().and_then(Uuid::parse), 0), None))),
            (NARY_END, self.nary_end.as_ref().map(|e| ((Some(e.association), e.end), e.refused.as_ref().map(|(_, why)| why.clone())))),
        ]
    }

    /// Closes each editor card not opened this frame, the refusals said under the sections, a
    /// stereotype's value typed and not entered, and the question about an n-ary end; a binding's
    /// menus belong to the binding they opened on (AR-131), so an open one closes too.
    fn close_editors(&mut self, menu: &mut Option<MenuId>) {
        if self.member.as_ref().is_some_and(|e| !e.opened) {
            self.member = None;
        }
        self.final_refused = None;
        self.leaf_refused = None;
        self.set_refused = None;
        self.profile_refused = None;
        // A value typed and not entered closes with the editors, as a member's editor does (AR-135).
        self.stereotypes = crate::uml_stereotypes::Stereotypes::default();
        if self.profile_edit.as_ref().is_some_and(|e| !e.opened) {
            self.profile_edit = None;
        }
        if self.constraint.as_ref().is_some_and(|e| !e.opened) {
            self.constraint = None;
        }
        if self.qualifier.as_ref().is_some_and(|e| !e.opened) {
            self.qualifier = None;
        }
        if self.template.as_ref().is_some_and(|e| !e.opened) {
            self.template = None;
        }
        self.template_in_use = None;
        if menu
            .is_some_and(|m| [menus::BINDING_BOUND, menus::BINDING_TEMPLATE, menus::BINDING_ARGUMENT].iter().any(|kind| m.index_in(*kind).is_some()))
        {
            *menu = None;
        }
        if self.nary_end.as_ref().is_some_and(|e| !e.opened) {
            self.nary_end = None;
        }
        self.nary_remove = None;
    }

    /// A member typed in place on its card, or the menu of generalization lines or of an n-ary
    /// association.
    fn holds_keys(&self) -> bool {
        self.inline.is_some() || self.line_menu.is_some() || self.nary_menu.is_some()
    }

    /// The Stereotypes picker, which takes typing, Backspace, the arrows and Enter (AR-135).
    fn menu_takes_keys(&self, menu: MenuId) -> bool {
        menu == menus::STEREOTYPE_PICK
    }

    /// An n-ary association being drawn and its menu end with the project they were on (AR-128).
    fn set_aside(&mut self) {
        self.nary = None;
        self.nary_menu = None;
    }

    /// The menus of generalization lines and of an n-ary association close with any press on the
    /// canvas (AR-128, AR-129).
    fn canvas_pressed(&mut self) {
        (self.line_menu, self.nary_menu) = (None, None);
    }

    /// The member typed in place on its class's card, which the card is drawn with
    /// ([`crate::uml_inline::draft`]).
    fn card_draft(&self, store: &Store) -> Option<(Obj, usize, Option<usize>)> {
        crate::uml_inline::draft(self.inline.as_ref()?, store)
    }
}
