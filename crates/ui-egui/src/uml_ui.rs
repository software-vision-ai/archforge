//! What UML brings to the shell: the class diagram's tools and gestures (the
//! classifier tool places a class, an interface, an enumeration or a data type named in place, its
//! card renames in place), and its packages, classes, interfaces, enumerations and data types in
//! the Models tree, the search and Properties. An enumeration's literals are typed on its card and
//! edited in Properties and Models, as a class's members are (AR-122); a data type's attributes and
//! operations as a class's (AR-123).

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::commands::{classifiers, package_of, packages};
use egui::Rect;
use serde_json::json;

use crate::Shell;
use crate::explorer::{Row, Tree, fresh_name};
use crate::i18n::tr;
use crate::kinds::{DiagramKind, KindTool, ToolUse};
use crate::notation_ui::{Added, Entry, NotationUi, Offer, Words};
use crate::panels::Subject;
use crate::uml_classifier::Classifier;
use crate::uml_panel::{MemberKind, member_of};
use crate::ux::{Palette, icons};

fn class(store: &Store, o: Obj) -> Option<archforge_mof::ClassId> {
    store.class_of(o).ok()
}

/// The name of `o`, empty when it has none.
fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// Whether `o` is a class, an interface, an enumeration or a data type.
fn classifier(store: &Store, o: Obj) -> bool {
    class(store, o).is_some_and(archforge_uml::is_classifier)
}

/// Whether `o` is an enumeration.
fn is_enumeration(store: &Store, o: Obj) -> bool {
    class(store, o) == Some(archforge_uml::ENUMERATION)
}

/// The enumeration `o` is, or the one whose literal it is: where Models' `+` adds a literal.
fn enumeration_at(store: &Store, o: Obj) -> Option<Obj> {
    if is_enumeration(store, o) {
        return Some(o);
    }
    member_of(store, o).filter(|(_, kind)| *kind == MemberKind::Literal).map(|(e, _)| e)
}

/// The classifier a member or a literal belongs to: what Properties shows for it.
fn owner_of(store: &Store, o: Obj) -> Option<Obj> {
    member_of(store, o).map(|(c, _)| c)
}

/// The class diagram in the shell.
pub(crate) struct UmlKind;

const TOOLS: &[KindTool] = &[
    KindTool {
        key: crate::uml_classifier::TOOL,
        icon: &icons::TOOL_CLASS,
        hint: "Class",
        use_: ToolUse::Place,
        chevron: Some((crate::uml_classifier::KINDS, "Classifier kinds")),
        wide: false,
    },
    KindTool { key: "package", icon: &icons::PACKAGE, hint: "Package", use_: ToolUse::Place, chevron: None, wide: false },
    KindTool {
        key: "relationship",
        icon: &icons::TOOL_RELATIONSHIP,
        hint: "Relationship",
        use_: ToolUse::Connect,
        chevron: Some(("relationship-kinds", "Relationship kinds")),
        wide: true,
    },
];

impl DiagramKind for UmlKind {
    fn kind(&self) -> &'static str {
        archforge_uml::CLASS_DIAGRAM
    }

    fn tools(&self) -> &'static [KindTool] {
        TOOLS
    }

    fn word(&self) -> String {
        tr("Class diagram")
    }

    fn group_word(&self) -> String {
        tr("Class diagrams")
    }

    fn description(&self) -> String {
        tr("Classes with their attributes and operations, and the lines between them.")
    }

    fn new_word(&self) -> String {
        tr("New class diagram")
    }

    /// Two class cards joined by an association: Order with its compartments, and Customer.
    fn sketch(&self, paint: &egui::Painter, p: &crate::ux::Palette, r: egui::Rect) {
        use egui::{Stroke, StrokeKind, pos2, vec2};
        let at = |x: f32, y: f32| r.center() + vec2(x - 134.0, y - 50.0);
        let order = egui::Rect::from_min_size(at(30.0, 14.0), vec2(96.0, 70.0));
        let customer = egui::Rect::from_min_size(at(170.0, 26.0), vec2(70.0, 46.0));
        // The sample classes' names, in the user's language, as the rest of the card is.
        for (card, name, rule) in [(order, tr("Order"), 22.0), (customer, tr("Customer"), 20.0)] {
            paint.rect_filled(card, 5.0, p.surface);
            paint.rect_stroke(card, 5.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            crate::ux::text(paint, card.min + vec2(10.0, 4.0), &name, crate::ux::types::TINY, p.text, Some(card.width() - 14.0));
            paint.line_segment([pos2(card.left(), card.top() + rule), pos2(card.right(), card.top() + rule)], Stroke::new(1.0, p.border));
        }
        paint.line_segment([pos2(order.left(), order.top() + 44.0), pos2(order.right(), order.top() + 44.0)], Stroke::new(1.0, p.border));
        for (y, w) in [(28.0, 50.0), (35.0, 38.0), (51.0, 44.0)] {
            paint.rect_filled(egui::Rect::from_min_size(order.min + vec2(10.0, y), vec2(w, 3.0)), 1.5, p.text_muted);
        }
        paint.line_segment([pos2(order.right(), r.center().y - 1.0), pos2(customer.left(), r.center().y - 1.0)], Stroke::new(1.2, p.text_secondary));
    }

    fn icon(&self) -> &'static crate::ux::Icon {
        &icons::CLASS_DIAGRAM
    }

    fn owner_word(&self) -> String {
        tr("Package")
    }

    fn new_owner(&self) -> String {
        tr("New package")
    }

    /// Before the model has its root package, a new class diagram goes into the root, named after
    /// the project and made on Create (owner, 2026-11-04); after, "New package" makes a package in it.
    fn new_owner_in(&self, store: &Store) -> String {
        if archforge_uml::commands::root_package(store).is_some() {
            return self.new_owner();
        }
        archforge_uml::commands::model(store)
            .map(|m| archforge_core::name_of(store, m))
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| tr("Model"))
    }

    fn owners(&self, store: &Store) -> Vec<Obj> {
        archforge_uml::commands::all_packages(store)
    }

    fn owner_of(&self, store: &Store, selected: Obj) -> Option<Obj> {
        if class(store, selected) == Some(archforge_uml::PACKAGE) { Some(selected) } else { package_of(store, selected) }
    }

    /// A package named `name` for a new class diagram to be of, or the root package while the model
    /// has none yet.
    fn add_owner(&self, shell: &mut Shell, name: &str) -> Result<String, String> {
        let has_root = shell.session.as_ref().is_some_and(|s| archforge_uml::commands::root_package(s.doc()).is_some());
        let made = if has_root {
            crate::editing::run(shell, "uml.package.add", json!({ "name": name }))
        } else {
            crate::editing::run(shell, "uml.model.root", json!({}))
        };
        made.map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    }

    /// A class, an interface, an enumeration or a data type takes part in the class diagram's
    /// gestures.
    fn takes_part(&self, store: &Store, element: Obj) -> bool {
        classifier(store, element)
    }

    /// A class, an interface, an enumeration or a data type, as the classifier tool's kind says, in the diagram's
    /// package, its card centred under the click, as one undo step; then its name is open for typing.
    fn place(&self, shell: &mut Shell, tool: &'static str, d: Obj, x: f64, y: f64) {
        if tool == "package" {
            place_package(shell, d, x, y);
            return;
        }
        shell.canvas.notice = None;
        let Some(session) = shell.session.as_mut() else { return };
        let store = session.doc();
        let Some(package) = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()) else {
            shell.canvas.notice = Some(tr("The diagram is of no package"));
            return;
        };
        let (Ok(package_id), Ok(diagram_id)) = (store.uuid(package), store.uuid(d)) else {
            shell.canvas.notice = Some(tr("The diagram is of no package"));
            return;
        };
        let kind = shell.canvas.uml_classifier;
        let Some(name) = fresh_name(kind.stem(), classifiers(store, package).into_iter().map(|c| name(store, c)), false) else {
            shell.canvas.notice = Some(tr("No free name is left"));
            return;
        };
        let (w, h) = archforge_uml::CLASS_SIZE;
        let (px, py) = ((x - w / 2.0).round(), (y - h / 2.0).round());
        let name_for_batch = name.clone();
        let out = session.batch(&kind.add_step(), |b| {
            let c = b.execute(kind.add_command(), &json!({ "package": package_id.to_string(), "name": name_for_batch }))?;
            let class = c.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            b.execute("diagram.place", &json!({ "diagram": diagram_id.to_string(), "element": class, "x": px, "y": py, "width": w, "height": h }))?;
            Ok(())
        });
        match out {
            Ok(o) => {
                let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
                if let (Some(class), Some(node)) = (id_at(0), id_at(1)) {
                    let _ = crate::editing::run(shell, "view.select", json!({ "ids": [class.to_string()] }));
                    shell.canvas.rename = Some(crate::editing::NodeRename { node, entity: class, text: name, opened: true, refused: None });
                    shell.canvas.focused = true;
                }
                shell.canvas.tool = crate::canvas::Tool::Select;
            }
            Err(e) => shell.canvas.notice = Some(e.to_string()),
        }
    }

    /// The classifier tool shows the kind it places, the Relationship tool the kind it draws.
    fn tool_face(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, key: &str, b: Rect, ink: egui::Color32, active: bool) -> bool {
        match key {
            crate::uml_classifier::TOOL => crate::uml_classifier::tool_face(shell, paint, b, ink),
            "relationship" => crate::uml_relate::tool_face(shell, paint, p, b, ink, active),
            _ => return false,
        }
        true
    }

    /// The classifier tool's tooltip names the kind it places; the Relationship tool's says which
    /// way a realization runs.
    fn tool_hint(&self, shell: &Shell, key: &str) -> Option<String> {
        match key {
            crate::uml_classifier::TOOL => Some(shell.canvas.uml_classifier.word()),
            "relationship" => crate::uml_relate::tool_hint(shell),
            _ => None,
        }
    }

    /// A realization starts only at a class: from any other classifier the tool starts no line.
    fn line_start_problem(&self, shell: &Shell, store: &Store, tool: &'static str, element: Obj) -> Option<String> {
        (tool == "relationship").then(|| crate::uml_relate::start_problem(shell, store, element)).flatten()
    }

    /// The classifier tool's menu of kinds, or the Relationship tool's.
    fn popover(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect, key: &str) {
        if key == crate::uml_classifier::KINDS {
            crate::uml_classifier::popover(shell, ui, p, r, chevron);
        } else {
            crate::uml_relate::popover(shell, ui, p, r, chevron);
        }
    }

    fn connect(&self, shell: &mut Shell, _tool: &'static str, d: Obj, from_node: Uuid, to_node: Uuid) {
        crate::uml_relate::connect(shell, d, from_node, to_node);
    }

    fn connect_to_new(&self, shell: &mut Shell, _tool: &'static str, d: Obj, from_node: Uuid, x: f64, y: f64) {
        crate::uml_relate::connect_to_new(shell, d, from_node, x, y);
    }

    fn reconnects(&self) -> bool {
        true
    }

    /// Moves an end of a selected line onto the classifier of `to_node`, the Relationship tool's rules kept.
    fn reconnect(&self, shell: &mut Shell, element: Uuid, source: bool, to_node: Uuid) {
        crate::uml_relate::reconnect(shell, element, source, to_node);
    }

    /// Renames a card's element in place: a package, a class, an interface, an enumeration or a data type,
    /// through its own command.
    fn rename(&self, shell: &mut Shell, element: Uuid, name: &str) -> Result<(), String> {
        let store = shell.session.as_ref().map(|s| s.doc());
        let (cmd, key) = match store.and_then(|st| st.by_uuid(element).map(|o| (st, o))) {
            Some((st, o)) if class(st, o) == Some(archforge_uml::PACKAGE) => ("uml.package.rename", "package"),
            Some((st, o)) => Classifier::of(st, o).unwrap_or_default().rename_command(),
            None => Classifier::Class.rename_command(),
        };
        crate::editing::run(shell, cmd, json!({ key: element.to_string(), "name": name })).map(|_| ())
    }

    /// A class, an interface, an enumeration, a data type, or a package on the diagram, goes from the model with
    /// ⇧⌫ (AR-95).
    fn deletes(&self, store: &Store, element: Obj) -> bool {
        classifier(store, element) || class(store, element) == Some(archforge_uml::PACKAGE)
    }

    /// A package on a class diagram renames in place with a double click, as a class does.
    fn renames_in_place(&self, store: &Store, element: Obj) -> bool {
        class(store, element) == Some(archforge_uml::PACKAGE)
    }

    /// The member at `row` of the classifier's card (its attributes, then its operations; an
    /// enumeration's literals), or a new attribute (a new literal), typed in place on the card, or
    /// in Properties' editor where the card cannot hold it.
    fn open(&self, shell: &mut Shell, element: Uuid, row: Option<usize>) {
        let enumeration = shell.session.as_ref().is_some_and(|s| s.doc().by_uuid(element).is_some_and(|e| is_enumeration(s.doc(), e)));
        let member = row.and_then(|row| {
            let store = shell.session.as_ref()?.doc();
            let c = store.by_uuid(element)?;
            if enumeration {
                return Some((MemberKind::Literal, store.uuid(*archforge_uml::commands::literals(store, c).get(row)?).ok()?));
            }
            let attributes = archforge_uml::members::attributes(store, c);
            let (kind, o) = match attributes.get(row) {
                Some(a) => (MemberKind::Attribute, *a),
                None => (MemberKind::Operation, *archforge_uml::members::operations(store, c).get(row - attributes.len())?),
            };
            Some((kind, store.uuid(o).ok()?))
        });
        let new = if enumeration { MemberKind::Literal } else { MemberKind::Attribute };
        let (kind, target) = member.map_or((new, None), |(kind, id)| (kind, Some(id)));
        // Typed in place on the card; Properties' editor where the card cannot hold it.
        if crate::uml_inline::open(shell, element, kind, target) {
            return;
        }
        let _ = crate::editing::run(shell, "view.select", json!({ "ids": [element.to_string()] }));
        shell.state.show_panel(crate::state::Panel::Properties);
        crate::uml_panel::open(shell, element, kind, target);
    }

    /// A selected card's second adorner adds an attribute, or an enumeration's a literal.
    fn adorner(&self, store: &Store, element: Obj) -> Option<(&'static str, &'static str)> {
        Some(if is_enumeration(store, element) { ("add-literal", "Add literal") } else { ("add-attribute", "Add attribute") })
    }

    /// A class's operation, typed in place as its attribute is; an enumeration takes no operation,
    /// so ⇧⌘↩ adds nothing to it.
    fn open_second(&self, shell: &mut Shell, element: Uuid) {
        if shell.session.as_ref().is_some_and(|s| s.doc().by_uuid(element).is_some_and(|e| is_enumeration(s.doc(), e))) {
            return;
        }
        if !crate::uml_inline::open(shell, element, MemberKind::Operation, None) {
            let _ = crate::editing::run(shell, "view.select", json!({ "ids": [element.to_string()] }));
            shell.state.show_panel(crate::state::Panel::Properties);
            crate::uml_panel::open(shell, element, MemberKind::Operation, None);
        }
    }

    /// The + of each compartment: a class's attributes and operations, an enumeration's literals.
    fn card_controls(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, clip: Rect, rect: Rect, element: Uuid) {
        crate::uml_inline::add_buttons(shell, ui, p, clip, rect, element);
    }

    /// A class's menu: open in Models, add an attribute, remove from the diagram, delete from the
    /// model.
    fn menu(&self) -> bool {
        true
    }

    /// A class, an interface, an enumeration or a data type from the tree, with its lines to the classifiers the
    /// diagram shows; one already there is said so and left.
    fn drop(&self, shell: &mut Shell, d: Obj, element: Uuid, at: (f64, f64)) {
        shell.canvas.notice = None;
        let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
        if store.by_uuid(element).is_some_and(|o| class(store, o) == Some(archforge_uml::PACKAGE)) {
            drop_package(shell, d, element, at);
            return;
        }
        let Some((c, kind)) = store.by_uuid(element).and_then(|c| Some((c, Classifier::of(store, c)?))) else { return };
        if crate::editing::on_diagram(store, d, c) {
            shell.canvas.notice = Some(tr("Already on this diagram"));
            return;
        }
        let notation = archforge_uml::UmlNotation;
        let links: Vec<Obj> = store
            .alive()
            .filter(|l| class(store, *l).is_some_and(archforge_uml::is_relationship))
            .filter(|l| archforge_diagram::Notation::ends(&notation, store, *l).is_some_and(|(a, b)| a == c || b == c))
            .collect();
        let (width, _) = archforge_uml::CLASS_SIZE;
        crate::editing::place_with_lines(shell, d, c, at, (width, content_height(store, c)), &links, &notation, &kind.place_step());
    }
}

/// The Package tool: a package in the diagram's package, its card centred under the click, and the
/// package's own diagram, as one undo step (AR-95); then its name is open for typing.
fn place_package(shell: &mut Shell, d: Obj, x: f64, y: f64) {
    shell.canvas.notice = None;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(parent) = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()) else {
        shell.canvas.notice = Some(tr("The diagram is of no package"));
        return;
    };
    let (Ok(parent_id), Ok(diagram_id)) = (store.uuid(parent), store.uuid(d)) else { return };
    let taken = archforge_uml::commands::subpackages(store, parent).into_iter().map(|p| name(store, p));
    let Some(package_name) = fresh_name("Package", taken, false) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (w, h) = archforge_uml::PACKAGE_SIZE;
    let (px, py) = ((x - w / 2.0).round(), (y - h / 2.0).round());
    let for_batch = package_name.clone();
    let out = session.batch(&tr("Add package"), |b| {
        let made = b.execute("uml.package.add", &json!({ "name": for_batch, "parent": parent_id.to_string() }))?;
        let package = made.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute("diagram.place", &json!({ "diagram": diagram_id.to_string(), "element": package, "x": px, "y": py, "width": w, "height": h }))?;
        b.execute("uml.package.diagram", &json!({ "package": package }))?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(package), Some(node)) = (id_at(0), id_at(1)) {
                let _ = crate::editing::run(shell, "view.select", json!({ "ids": [package.to_string()] }));
                shell.canvas.rename = Some(crate::editing::NodeRename { node, entity: package, text: package_name, opened: true, refused: None });
                shell.canvas.focused = true;
            }
            shell.canvas.tool = crate::canvas::Tool::Select;
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// A package dragged from Models onto a class diagram: placed where it was let go, with its own
/// diagram made when it has none, as one undo step. Only a package the diagram's package holds
/// goes on it.
fn drop_package(shell: &mut Shell, d: Obj, element: Uuid, at: (f64, f64)) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(package) = store.by_uuid(element) else { return };
    if crate::editing::on_diagram(store, d, package) {
        shell.canvas.notice = Some(tr("Already on this diagram"));
        return;
    }
    let here = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
    if here.is_none() || package_of(store, package) != here {
        shell.canvas.notice = Some(tr("Only a package this diagram's package holds goes on it"));
        return;
    }
    let Ok(diagram_id) = store.uuid(d) else { return };
    let (w, h) = archforge_uml::PACKAGE_SIZE;
    let (px, py) = ((at.0 - w / 2.0).round(), (at.1 - h / 2.0).round());
    let out = session.batch(&tr("Place package"), |b| {
        b.execute(
            "diagram.place",
            &json!({ "diagram": diagram_id.to_string(), "element": element.to_string(), "x": px, "y": py, "width": w, "height": h }),
        )?;
        b.execute("uml.package.diagram", &json!({ "package": element.to_string() }))?;
        Ok(())
    });
    match out {
        Ok(_) => {
            let _ = crate::editing::run(shell, "view.select", json!({ "ids": [element.to_string()] }));
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// How tall a class's, an interface's or a data type's card is to show its members: its attributes, the dividing line and its
/// operations, an empty compartment a row; an enumeration's its literals; never less than a new
/// class nor more than a node may be.
fn content_height(store: &Store, c: Obj) -> f64 {
    if is_enumeration(store, c) {
        return literals_height(archforge_uml::commands::literals(store, c).len());
    }
    members_height(archforge_uml::members::attributes(store, c).len(), archforge_uml::members::operations(store, c).len())
}

/// How tall an enumeration's card is to show `literals` rows in its one compartment, none a row.
pub(crate) fn literals_height(literals: usize) -> f64 {
    use crate::canvas::{ENTITY_FIRST_ROW, ENTITY_ROW};
    let slots = literals.max(1) as f64;
    (f64::from(ENTITY_FIRST_ROW) + 9.0 + slots * f64::from(ENTITY_ROW)).clamp(archforge_uml::CLASS_SIZE.1, archforge_diagram::rules::MAX_SIZE)
}

/// How tall a card is to show `attributes` and `operations` rows.
pub(crate) fn members_height(attributes: usize, operations: usize) -> f64 {
    use crate::canvas::{ENTITY_FIRST_ROW, ENTITY_ROW};
    let rows = |n: usize| n.max(1) as f64;
    let slots = rows(attributes) + 0.5 + rows(operations);
    (f64::from(ENTITY_FIRST_ROW) + 9.0 + slots * f64::from(ENTITY_ROW)).clamp(archforge_uml::CLASS_SIZE.1, archforge_diagram::rules::MAX_SIZE)
}

/// Every card of class `c`, on every diagram, too short for its members once it has `more`
/// attributes and operations besides those it has (an enumeration: `more.0` literals): each with
/// its width and the height that shows them all. A card is only ever grown, so one the user made
/// taller keeps its height.
pub(crate) fn short_cards(store: &Store, c: Obj, more: (usize, usize)) -> Vec<(Uuid, f64, f64)> {
    let needed = if is_enumeration(store, c) {
        literals_height(archforge_uml::commands::literals(store, c).len().saturating_add(more.0))
    } else {
        let attributes = archforge_uml::members::attributes(store, c).len().saturating_add(more.0);
        let operations = archforge_uml::members::operations(store, c).len().saturating_add(more.1);
        members_height(attributes, operations)
    };
    archforge_diagram::views_of(store, c)
        .2
        .into_iter()
        .filter_map(|n| {
            let b = archforge_diagram::grid::bounds_of(store, n);
            (b.height < needed).then_some((store.uuid(n).ok()?, b.width, needed))
        })
        .collect()
}

/// UML in the shell's explorers, search and panels.
pub(crate) struct UmlUi;

impl NotationUi for UmlUi {
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }

    /// A package is the top of its rows in Models.
    fn is_top(&self, store: &Store, member: Obj) -> bool {
        class(store, member) == Some(archforge_uml::PACKAGE)
    }

    /// A package with the packages it holds, its classes, interfaces, enumerations and data types, and each
    /// one's attributes and operations or literals, every level folded until opened.
    fn tree(&self, store: &Store, package: Obj, t: &Tree<'_>) -> Vec<Row> {
        package_rows(store, package, 0, t)
    }

    /// What Models' `+` offers at the selection: a literal first at an enumeration or one of its
    /// literals, then a class, an interface, an enumeration, a data type and a package in its package, else a
    /// package.
    fn offers(&self, store: &Store, selection: Option<Obj>) -> Vec<Offer> {
        let offer = |key, word: String| Offer { key, word };
        // A classifier of each kind or a package goes into the selected package, or the package of
        // what is selected.
        match selection.and_then(|o| home_of(store, o)) {
            Some(_) => {
                let literal = selection.and_then(|o| enumeration_at(store, o)).map(|_| offer("literal", tr("Literal")));
                let mut out: Vec<Offer> = literal.into_iter().chain(Classifier::ALL.iter().map(|k| offer(k.key(), k.word()))).collect();
                out.push(offer("package", tr("Package")));
                out
            }
            None => vec![offer("package", tr("Package"))],
        }
    }

    /// Adds what the offer `key` names at the selection, with a fresh name free among its siblings.
    fn add(&self, shell: &mut Shell, key: &str, sel: Option<Obj>) -> Option<Result<Added, String>> {
        let store = shell.session.as_ref()?.doc();
        let no_name = || Some(Err(tr("No free name is left")));
        let home = sel.and_then(|o| home_of(store, o));
        // The packages it sits under open, so its row shows.
        let mut parents: Vec<Uuid> = home.map(|h| chain(store, h)).unwrap_or_default().into_iter().filter_map(|p| store.uuid(p).ok()).collect();
        let out = match key {
            "literal" => {
                // At the end of the selected enumeration, or of the selected literal's, its card
                // growing to show it.
                let e = sel.and_then(|o| enumeration_at(store, o))?;
                let taken = archforge_uml::commands::literals(store, e).into_iter().map(|l| name(store, l));
                let Some(name) = fresh_name("Literal", taken, false) else { return no_name() };
                let id = store.uuid(e).ok()?;
                parents.push(id);
                let edit = crate::uml_panel::MemberEdit { name, ..crate::uml_panel::edit_of(Some(store), id, MemberKind::Literal, None) };
                let (cmd, params) = crate::uml_panel::command_of(&edit);
                crate::uml_panel::write_in_sight(shell, &edit, cmd, params)
            }
            "package" => {
                // With nothing selected it goes into the root package, so its name is free there;
                // without a root yet, the packages at the top are the ones the root takes in.
                let home = home.or_else(|| {
                    let root = archforge_uml::commands::root_package(store)?;
                    parents.extend(store.uuid(root).ok());
                    Some(root)
                });
                let siblings = home.map_or_else(|| packages(store), |h| archforge_uml::commands::subpackages(store, h));
                let Some(name) = fresh_name("Package", siblings.into_iter().map(|p| name(store, p)), false) else { return no_name() };
                let parent = home.and_then(|h| store.uuid(h).ok()).map(|u| u.to_string()).unwrap_or_default();
                crate::explorer::run(shell, "uml.package.add", json!({ "name": name, "parent": parent }))
            }
            key => {
                let kind = Classifier::ALL.into_iter().find(|k| k.key() == key)?;
                let package = home?;
                let Some(name) = fresh_name(kind.stem(), classifiers(store, package).into_iter().map(|c| name(store, c)), false) else {
                    return no_name();
                };
                let package_id = store.uuid(package).ok()?;
                parents.push(package_id);
                crate::explorer::run(shell, kind.add_command(), json!({ "package": package_id.to_string(), "name": name }))
            }
        };
        match out {
            Ok(out) => out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse).map(|id| Ok(Added { id, parents })),
            Err(e) => Some(Err(e)),
        }
    }

    /// The command that renames `obj`, and the key that names it: a package, a classifier or a
    /// literal renames in place.
    fn rename(&self, store: &Store, obj: Obj) -> Option<(&'static str, &'static str)> {
        match class(store, obj)? {
            c if c == archforge_uml::PACKAGE => Some(("uml.package.rename", "package")),
            c if c == archforge_uml::ENUMERATION_LITERAL => Some(("uml.literal.rename", "literal")),
            _ => Classifier::of(store, obj).map(Classifier::rename_command),
        }
    }

    /// A literal's name another literal of its enumeration has, whatever the case, said as the
    /// card and Properties say it.
    fn rename_refusal(&self, store: &Store, obj: Obj, name: &str) -> Option<String> {
        let (e, kind) = member_of(store, obj).filter(|(_, k)| *k == MemberKind::Literal)?;
        let (Ok(class), Ok(target)) = (store.uuid(e), store.uuid(obj)) else { return None };
        let edit = crate::uml_panel::MemberEdit { name: name.to_string(), ..crate::uml_panel::edit_of(Some(store), class, kind, Some(target)) };
        crate::uml_panel::literal_refusal(store, &edit)
    }

    /// The relationships that go with a class, an interface, an enumeration or a data type, or with a package's.
    fn consequences(&self, store: &Store, obj: Obj) -> (usize, usize, usize) {
        let held = match class(store, obj) {
            Some(c) if archforge_uml::is_classifier(c) => vec![obj],
            Some(c) if c == archforge_uml::PACKAGE => classifiers(store, obj),
            _ => Vec::new(),
        };
        let mut going: Vec<Obj> = held.into_iter().flat_map(|c| archforge_uml::relations::relationships_of(store, c)).collect();
        going.sort();
        going.dedup();
        (going.len(), 0, 0)
    }

    /// Whether `obj` has rows under it: a package what it holds, a classifier its members (an
    /// enumeration its literals).
    fn has_children(&self, store: &Store, obj: Obj) -> bool {
        match class(store, obj) {
            Some(c) if c == archforge_uml::PACKAGE => {
                !classifiers(store, obj).is_empty() || !archforge_uml::commands::subpackages(store, obj).is_empty()
            }
            Some(c) if archforge_uml::is_classifier(c) => has_members(store, obj),
            _ => false,
        }
    }

    /// The package and every package and classifier with members or literals under it.
    fn foldable(&self, store: &Store, package: Obj) -> Vec<Obj> {
        let mut out = vec![package];
        out.extend(classifiers(store, package).into_iter().filter(|c| has_members(store, *c)));
        for sub in archforge_uml::commands::subpackages(store, package) {
            out.extend(self.foldable(store, sub));
        }
        out
    }

    /// The packages above, and a member's class or a literal's enumeration.
    fn parents(&self, store: &Store, obj: Obj) -> Vec<Obj> {
        let (start, mut out) = match owner_of(store, obj) {
            Some(c) => (Some(c), vec![c]),
            None => (package_of(store, obj), Vec::new()),
        };
        let home = if out.is_empty() { start } else { start.and_then(|c| package_of(store, c)) };
        out.extend(home.map(|h| chain(store, h)).unwrap_or_default());
        out
    }

    /// A nested package hands what it holds to its parent when it is removed.
    fn hands_up(&self, store: &Store, obj: Obj) -> Option<Obj> {
        if class(store, obj) == Some(archforge_uml::PACKAGE) { package_of(store, obj) } else { None }
    }

    /// A class, an interface, an enumeration or a data type dragged onto a package, or onto a classifier of
    /// another package, moves there; a package dragged onto another package moves into it.
    fn drop_on(&self, store: &Store, dragged: Obj, target: Obj) -> Option<(&'static str, serde_json::Value)> {
        let to = match class(store, target)? {
            c if c == archforge_uml::PACKAGE => target,
            c if archforge_uml::is_classifier(c) => package_of(store, target)?,
            _ => return None,
        };
        let id = |o: Obj| store.uuid(o).ok().map(|u| u.to_string());
        match class(store, dragged)? {
            c if archforge_uml::is_classifier(c) && package_of(store, dragged) != Some(to) => {
                let (cmd, key) = Classifier::of(store, dragged)?.move_command();
                Some((cmd, json!({ key: id(dragged)?, "package": id(to)? })))
            }
            // Into itself or a package it holds too: the engine refuses it and says why (AR-102).
            c if c == archforge_uml::PACKAGE && package_of(store, dragged) != Some(to) => {
                Some(("uml.package.move", json!({ "package": id(dragged)?, "to": id(to)? })))
            }
            _ => None,
        }
    }

    /// A package held over itself or a package it holds: a drop there is refused.
    fn drop_refused(&self, store: &Store, dragged: Obj, target: Obj) -> bool {
        let to = match class(store, target) {
            Some(c) if c == archforge_uml::PACKAGE => target,
            Some(c) if archforge_uml::is_classifier(c) => match package_of(store, target) {
                Some(p) => p,
                None => return false,
            },
            _ => return false,
        };
        class(store, dragged) == Some(archforge_uml::PACKAGE) && archforge_uml::commands::is_within(store, to, dragged)
    }

    /// A package or a classifier as the search lists it: its icon, its name and its kind with its
    /// package.
    fn entry(&self, store: &Store, o: Obj) -> Option<Entry> {
        match class(store, o)? {
            c if c == archforge_uml::PACKAGE => {
                let at = package_of(store, o).map(|p| name(store, p)).unwrap_or_default();
                let detail = if at.is_empty() { tr("Package") } else { format!("{} · {at}", tr("Package")) };
                Some(Entry { icon: &icons::PACKAGE, order: 1, label: name(store, o), detail })
            }
            c if archforge_uml::is_classifier(c) => {
                let kind = Classifier::of(store, o)?;
                let at = package_of(store, o).map(|p| name(store, p)).unwrap_or_default();
                let detail = if at.is_empty() { kind.word() } else { format!("{} · {at}", kind.word()) };
                Some(Entry { icon: kind.icon(), order: 2, label: name(store, o), detail })
            }
            _ => None,
        }
    }

    /// A member's class or a literal's enumeration, which the canvas brings into view for it.
    fn shown(&self, store: &Store, o: Obj) -> Option<Obj> {
        Some(owner_of(store, o).unwrap_or(o))
    }

    /// A member's problems show on its classifier, a literal's on its enumeration.
    fn problem_holder(&self, store: &Store, o: Obj) -> Obj {
        owner_of(store, o).unwrap_or(o)
    }

    /// A literal and a relationship carry no skills: the Skills panel says so in their words.
    fn no_skills(&self, store: &Store, o: Obj) -> Option<String> {
        match class(store, o)? {
            c if c == archforge_uml::ENUMERATION_LITERAL => Some(tr("Literals carry no skills")),
            c if archforge_uml::is_relationship(c) => Some(tr("Relationships carry no skills")),
            _ => None,
        }
    }

    /// How Properties names `o`: its kind line, its word, This …, and On this … · {n}.
    fn words(&self, store: &Store, o: Obj) -> Option<Words> {
        let words = |kind: String, word: String, this: String, on_this: String| Words { kind, word, this, on_this, name: name(store, o) };
        Some(match class(store, o)? {
            c if c == archforge_uml::PACKAGE => words(tr("Package · model"), tr("Package"), tr("This package"), tr("On this package · {n}")),
            c if c == archforge_uml::CLASS => words(tr("Class · model, in every diagram"), tr("Class"), tr("This class"), tr("On this class · {n}")),
            c if c == archforge_uml::INTERFACE => {
                words(tr("Interface · model, in every diagram"), tr("Interface"), tr("This interface"), tr("On this interface · {n}"))
            }
            c if c == archforge_uml::ENUMERATION => {
                words(tr("Enumeration · model, in every diagram"), tr("Enumeration"), tr("This enumeration"), tr("On this enumeration · {n}"))
            }
            c if c == archforge_uml::DATA_TYPE => {
                words(tr("Data type · model, in every diagram"), tr("Data type"), tr("This data type"), tr("On this data type · {n}"))
            }
            c if c == archforge_uml::ASSOCIATION => {
                words(tr("Association · model"), tr("Association"), tr("This association"), tr("On this association · {n}"))
            }
            c if c == archforge_uml::GENERALIZATION => {
                words(tr("Generalization · model"), tr("Generalization"), tr("This generalization"), tr("On this generalization · {n}"))
            }
            c if c == archforge_uml::DEPENDENCY => {
                words(tr("Dependency · model"), tr("Dependency"), tr("This dependency"), tr("On this dependency · {n}"))
            }
            c if c == archforge_uml::INTERFACE_REALIZATION => words(
                tr("Interface realization · model"),
                tr("Interface realization"),
                tr("This interface realization"),
                tr("On this interface realization · {n}"),
            ),
            _ => match member_of(store, o)?.1 {
                MemberKind::Attribute => words(tr("Attribute · model"), tr("Attribute"), tr("This attribute"), tr("On this attribute · {n}")),
                MemberKind::Operation => words(tr("Operation · model"), tr("Operation"), tr("This operation"), tr("On this operation · {n}")),
                MemberKind::Literal => words(tr("Literal · model"), tr("Literal"), tr("This literal"), tr("On this literal · {n}")),
            },
        })
    }

    /// A member's panel is its classifier's, a literal's its enumeration's.
    fn heading(&self, store: &Store, o: Obj) -> Option<String> {
        owner_of(store, o).and_then(|c| Classifier::of(store, c)).map(Classifier::word)
    }

    /// A package's name; a classifier's panel, which a member's or a literal's selection shows too.
    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
        let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
        let Some(o) = subject.obj else { return };
        match class(store, o) {
            Some(c) if archforge_uml::is_classifier(c) => return crate::uml_panel::class_body(shell, ui, p, r, y, subject),
            Some(c) if archforge_uml::is_relationship(c) => {
                return crate::uml_relate::body(shell, ui, p, r, y, subject);
            }
            _ => {}
        }
        if let Some(c) = owner_of(store, o) {
            let Some(words) = self.words(store, c) else { return };
            let (Ok(id), Ok(metaclass)) = (store.uuid(c), store.class_of(c)) else { return };
            let of = Subject::of(c, id, metaclass, words);
            return crate::uml_panel::class_body(shell, ui, p, r, y, &of);
        }
        if class(store, o) == Some(archforge_uml::PACKAGE) && archforge_uml::commands::root_package(store) != Some(o) {
            return crate::uml_panel::package_body(shell, ui, p, r, y, subject);
        }
        let Some((cmd, key)) = self.rename(store, o) else { return };
        let width = r.width() - 2.0 * crate::panels::PAD;
        crate::panels::name_field(shell, ui, p, egui::pos2(r.left() + crate::panels::PAD, y), width, subject, cmd, key);
    }
}

/// The package an element belongs in: a package itself, a classifier's, or a member's or a
/// literal's owner's.
fn home_of(store: &Store, o: Obj) -> Option<Obj> {
    match class(store, o)? {
        c if c == archforge_uml::PACKAGE => Some(o),
        c if archforge_uml::is_classifier(c) => package_of(store, o),
        _ => owner_of(store, o).and_then(|c| package_of(store, c)),
    }
}

/// A package and every package above it, innermost first.
fn chain(store: &Store, package: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut at = Some(package);
    while let Some(p) = at.filter(|_| out.len() < 256) {
        out.push(p);
        at = package_of(store, p);
    }
    out
}

/// Whether a class, an interface or a data type has an attribute or an operation, or an
/// enumeration a literal.
fn has_members(store: &Store, c: Obj) -> bool {
    !archforge_uml::members::attributes(store, c).is_empty()
        || !archforge_uml::members::operations(store, c).is_empty()
        || !archforge_uml::commands::literals(store, c).is_empty()
}

/// A package's rows at `depth`: the package, then, when open, its skills, the packages it holds,
/// its classes, interfaces, enumerations and data types, in its order, and their members. A row whose name holds the filter shows with the rows above it.
fn package_rows(store: &Store, package: Obj, depth: u8, t: &Tree<'_>) -> Vec<Row> {
    let mut under = t.skill_rows(package, depth.saturating_add(1));
    for sub in archforge_uml::commands::subpackages(store, package) {
        under.extend(package_rows(store, sub, depth.saturating_add(1), t));
    }
    for c in classifiers(store, package) {
        under.extend(class_rows(store, c, depth.saturating_add(1), t));
    }
    let label = name(store, package);
    let empty =
        t.skills(package).is_empty() && archforge_uml::commands::subpackages(store, package).is_empty() && classifiers(store, package).is_empty();
    if !t.matches(&label) && under.is_empty() {
        return Vec::new();
    }
    let expandable = if t.filtering() { !under.is_empty() } else { !empty };
    let expanded = expandable && t.open(package);
    let mut out: Vec<Row> = t
        .row(package, depth, "package", &icons::PACKAGE, label)
        .map(|row| Row { expandable, expanded, draggable: true, ..row })
        .into_iter()
        .collect();
    if expanded {
        out.extend(under);
    }
    out
}

/// A classifier's rows: itself, draggable onto a diagram or a package, then, when open, its
/// skills, its attributes and operations, each with its type, or an enumeration's literals in
/// their order, each renamed in place and removed with Delete. A folded row without a filter
/// builds none of them.
fn class_rows(store: &Store, c: Obj, depth: u8, t: &Tree<'_>) -> Vec<Row> {
    // Its members' rows are built only when they can show: the row open, or a filter that may
    // match one; a folded row builds none, however many it holds.
    let mut members = Vec::new();
    if t.filtering() || t.open(c) {
        members.extend(t.skill_rows(c, depth.saturating_add(1)));
        for l in archforge_uml::commands::literals(store, c) {
            let label = name(store, l);
            if t.matches(&label) {
                members.extend(t.row(l, depth.saturating_add(1), "literal", &icons::LITERAL, label));
            }
        }
        for a in archforge_uml::members::attributes(store, c) {
            let label = name(store, a);
            if t.matches(&label) {
                let ty = archforge_uml::figures::attribute_text(store, a);
                let detail = ty.split_once(" : ").map(|(_, rest)| rest.to_string());
                members.extend(t.row(a, depth.saturating_add(1), "attribute", &icons::ATTRIBUTE, label).map(|row| Row { detail, ..row }));
            }
        }
        for op in archforge_uml::members::operations(store, c) {
            let label = format!("{}()", name(store, op));
            if t.matches(&label) {
                let text = archforge_uml::figures::operation_text(store, op);
                let detail = text.rsplit_once(") : ").map(|(_, ty)| ty.to_string());
                members.extend(t.row(op, depth.saturating_add(1), "operation", &icons::OPERATION, label).map(|row| Row { detail, ..row }));
            }
        }
    }
    let label = name(store, c);
    if !t.matches(&label) && members.is_empty() {
        return Vec::new();
    }
    let expandable = if t.filtering() { !members.is_empty() } else { !t.skills(c).is_empty() || has_members(store, c) };
    let expanded = expandable && t.open(c);
    let kind = Classifier::of(store, c).unwrap_or_default();
    let mut out: Vec<Row> =
        t.row(c, depth, kind.key(), kind.icon(), label).map(|row| Row { expandable, expanded, draggable: true, ..row }).into_iter().collect();
    if expanded {
        out.extend(members);
    }
    out
}

/// Adds literals named `names` at the end of the enumeration `enumeration` through
/// `uml.literal.add`, as one step, for the snapshot screens and the tests; the project is then
/// marked saved as it stands.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn put_literals(shell: &mut Shell, enumeration: Uuid, names: &[&str]) -> Result<(), String> {
    let session = shell.session.as_mut().ok_or("the literals need a session")?;
    session
        .batch("Add literal", |b| {
            for name in names {
                b.execute("uml.literal.add", &json!({ "enumeration": enumeration.to_string(), "name": name }))?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    session.mark_saved().map_err(|e| e.to_string())
}

/// A class model to read, for the snapshot screens and the tests: in a package,
/// Commerce, classes with typed attributes and operations, an abstract Payment, associations that
/// navigate one way and both, a composition and a shared aggregation, a generalization and a
/// dependency. Built on the store, since commands for members and relationships come with their
/// own journeys. Gives the package.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn sample_model(st: &mut Store, model: Obj) -> Result<Obj, archforge_mof::Invalid> {
    use archforge_mof::Value;
    use archforge_uml as u;
    // An element without a name keeps none, as the commands store it.
    let named = |st: &mut Store, class, name: &str| -> Result<Obj, archforge_mof::Invalid> {
        let o = st.create(class)?;
        if !name.is_empty() {
            st.set(o, u::named_element::NAME, Value::str(name))?;
        }
        Ok(o)
    };
    let pkg = named(st, u::PACKAGE, "Commerce")?;
    st.push(model, archforge_core::model::MEMBER, Value::Ref(pkg))?;
    let mut types = std::collections::BTreeMap::new();
    for t in ["Integer", "String", "Date", "Money", "Boolean"] {
        let o = named(st, u::PRIMITIVE_TYPE, t)?;
        st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(o))?;
        types.insert(t, o);
    }
    let public = Value::Enum(0);
    let private = Value::Enum(1);
    // Multiplicity bounds: a lower bound and an upper one, None for many.
    let bounds = |st: &mut Store, o: Obj, lower: i64, upper: Option<u64>| -> Result<(), archforge_mof::Invalid> {
        let l = st.create(u::LITERAL_INTEGER)?;
        st.set(l, u::literal_integer::VALUE, Value::Int(lower))?;
        st.set(o, u::multiplicity_element::LOWER_VALUE, Value::Ref(l))?;
        let h = st.create(u::LITERAL_UNLIMITED_NATURAL)?;
        st.set(h, u::literal_unlimited_natural::VALUE, Value::Unlimited(upper))?;
        st.set(o, u::multiplicity_element::UPPER_VALUE, Value::Ref(h))
    };
    let class = |st: &mut Store, name: &str, abstract_: bool| -> Result<Obj, archforge_mof::Invalid> {
        let c = named(st, u::CLASS, name)?;
        if abstract_ {
            st.set(c, u::class::IS_ABSTRACT, Value::Bool(true))?;
        }
        st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(c))?;
        Ok(c)
    };
    let attribute = |st: &mut Store, c: Obj, name: &str, ty: Obj, vis: &Value| -> Result<Obj, archforge_mof::Invalid> {
        let a = named(st, u::PROPERTY, name)?;
        st.set(a, u::typed_element::TYPE, Value::Ref(ty))?;
        st.set(a, u::named_element::VISIBILITY, vis.clone())?;
        st.push(c, u::class::OWNED_ATTRIBUTE, Value::Ref(a))?;
        Ok(a)
    };
    let operation = |st: &mut Store, c: Obj, name: &str, params: &[(&str, Obj)], returns: Option<Obj>, abstract_: bool| {
        let op = named(st, u::OPERATION, name)?;
        st.set(op, u::named_element::VISIBILITY, Value::Enum(0))?;
        if abstract_ {
            st.set(op, u::behavioral_feature::IS_ABSTRACT, Value::Bool(true))?;
        }
        for (p, ty) in params {
            let param = named(st, u::PARAMETER, p)?;
            st.set(param, u::typed_element::TYPE, Value::Ref(*ty))?;
            st.push(op, u::operation::OWNED_PARAMETER, Value::Ref(param))?;
        }
        if let Some(ty) = returns {
            let r = st.create(u::PARAMETER)?;
            st.set(r, u::parameter::DIRECTION, Value::Enum(3))?;
            st.set(r, u::typed_element::TYPE, Value::Ref(ty))?;
            st.push(op, u::operation::OWNED_PARAMETER, Value::Ref(r))?;
        }
        st.push(c, u::class::OWNED_OPERATION, Value::Ref(op))
    };
    let t = |n: &str| types.get(n).copied().unwrap_or(pkg);
    let customer = class(st, "Customer", false)?;
    attribute(st, customer, "id", t("Integer"), &public)?;
    attribute(st, customer, "email", t("String"), &private)?;
    operation(st, customer, "rename", &[("name", t("String"))], None, false)?;
    let order = class(st, "Order", false)?;
    attribute(st, order, "id", t("Integer"), &public)?;
    attribute(st, order, "placedAt", t("Date"), &public)?;
    operation(st, order, "total", &[], Some(t("Money")), false)?;
    let line = class(st, "OrderLine", false)?;
    attribute(st, line, "quantity", t("Integer"), &public)?;
    let product = class(st, "Product", false)?;
    attribute(st, product, "sku", t("String"), &public)?;
    let price = attribute(st, product, "price", t("Money"), &public)?;
    bounds(st, price, 0, Some(1))?;
    let catalog = class(st, "Catalog", false)?;
    attribute(st, catalog, "name", t("String"), &public)?;
    let payment = class(st, "Payment", true)?;
    attribute(st, payment, "amount", t("Money"), &public)?;
    operation(st, payment, "authorise", &[], Some(t("Boolean")), true)?;
    let card = class(st, "CardPayment", false)?;
    attribute(st, card, "cardNumber", t("String"), &private)?;
    let pricing = class(st, "PricingService", false)?;
    operation(st, pricing, "price", &[("product", product)], Some(t("Money")), false)?;
    // An association from `a` to `b`: each end typed by its class with its bounds, its role and its
    // aggregation; an end a class owns is navigable, the others the association owns.
    type EndSpec<'a> = (Obj, &'a str, i64, Option<u64>, u32, Option<Obj>);
    let associate = |st: &mut Store, name: &str, ends: [EndSpec<'_>; 2]| -> Result<Obj, archforge_mof::Invalid> {
        let a = named(st, u::ASSOCIATION, name)?;
        st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(a))?;
        for (ty, role, lower, upper, aggregation, owner) in ends {
            let end = named(st, u::PROPERTY, role)?;
            st.set(end, u::typed_element::TYPE, Value::Ref(ty))?;
            st.set(end, u::property::AGGREGATION, Value::Enum(aggregation))?;
            bounds(st, end, lower, upper)?;
            // A member end first, which tells the end its association, then its owner.
            st.push(a, u::association::MEMBER_END, Value::Ref(end))?;
            match owner {
                Some(c) => {
                    st.push(c, u::class::OWNED_ATTRIBUTE, Value::Ref(end))?;
                }
                None if !st.list(a, u::association::OWNED_END)?.contains(&Value::Ref(end)) => {
                    st.push(a, u::association::OWNED_END, Value::Ref(end))?;
                }
                None => {}
            }
        }
        Ok(a)
    };
    associate(st, "places", [(customer, "customer", 1, Some(1), 0, None), (order, "orders", 0, None, 0, Some(customer))])?;
    associate(st, "", [(order, "order", 1, Some(1), 0, None), (line, "lines", 1, None, 2, Some(order))])?;
    associate(st, "", [(line, "", 0, None, 0, None), (product, "product", 1, Some(1), 0, Some(line))])?;
    associate(st, "lists", [(catalog, "", 0, None, 0, None), (product, "products", 0, None, 1, Some(catalog))])?;
    associate(st, "paid by", [(order, "order", 1, Some(1), 0, Some(payment)), (payment, "payment", 0, Some(1), 0, Some(order))])?;
    let g = st.create(u::GENERALIZATION)?;
    st.set(g, u::generalization::GENERAL, Value::Ref(payment))?;
    st.push(card, u::classifier::GENERALIZATION, Value::Ref(g))?;
    let dep = st.create(u::DEPENDENCY)?;
    st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(dep))?;
    st.push(dep, u::dependency::CLIENT, Value::Ref(order))?;
    st.push(dep, u::dependency::SUPPLIER, Value::Ref(pricing))?;
    Ok(pkg)
}

/// Opens the sample class model (`sample_model`) on a class diagram in the shell's project: its
/// classes laid out in three rows, every association, generalization and dependency drawn, the
/// named ones labelled. Gives the diagram's id.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn open_sample(shell: &mut Shell) -> Result<Uuid, String> {
    use archforge_mof::rows;
    let fail = |e: &dyn std::fmt::Display| e.to_string();
    let session = shell.session.as_ref().ok_or("the sample needs a session")?;
    let (tables, junctions) = archforge_notations::storage();
    let counter = |start: u64| {
        let n = std::sync::atomic::AtomicU64::new(start);
        Box::new(move || {
            let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&i.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        })
    };
    let mut st = archforge_notations::store(counter(1 << 31)).map_err(|e| fail(&e))?;
    rows::restore(&mut st, &rows::snapshot(session.doc(), &tables, &junctions).map_err(|e| fail(&e))?, &tables, &junctions).map_err(|e| fail(&e))?;
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).ok_or("the project has no model root")?;
    let pkg = sample_model(&mut st, model).map_err(|e| fail(&e))?;
    let pkg_uuid = st.uuid(pkg).map_err(|e| fail(&e))?;
    let back = rows::snapshot(&st, &tables, &junctions).map_err(|e| fail(&e))?;
    let mut session = archforge_notations::session_from_rows(&back, counter(1 << 32))?;
    let id = |v: serde_json::Value| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        session.execute(command, &params).map(|o| id(o.value)).map_err(|e| format!("{command}: {e}"))
    };
    let store = session.doc();
    let pkg = store.by_uuid(pkg_uuid).ok_or("the sample's package")?;
    let uuid = |o: Obj| store.uuid(o).map(|u| u.to_string()).unwrap_or_default();
    let by_name = |n: &str| classifiers(store, pkg).into_iter().find(|c| name(store, *c) == n);
    let layout = [
        ("Customer", 80, 80),
        ("Order", 560, 80),
        ("PricingService", 1040, 80),
        ("Product", 80, 400),
        ("OrderLine", 560, 400),
        ("Payment", 1040, 400),
        ("Catalog", 80, 700),
        ("CardPayment", 1040, 700),
    ];
    let placed: Vec<(String, i32, i32, f64)> = layout
        .iter()
        .filter_map(|(n, x, y)| {
            let c = by_name(n)?;
            let count = |n: usize| n.max(1) as f64;
            let rows = count(archforge_uml::members::attributes(store, c).len()) + count(archforge_uml::members::operations(store, c).len());
            Some((uuid(c), *x, *y, 77.0 + (rows + 0.5) * 20.0 + 6.0))
        })
        .collect();
    let lines: Vec<(String, bool)> = store
        .alive()
        .filter(|o| store.class_of(*o).is_ok_and(archforge_uml::is_relationship))
        .map(|o| (uuid(o), !name(store, o).is_empty()))
        .collect();
    let pkg_id = uuid(pkg);
    let d = run(&mut session, "diagram.add", serde_json::json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": pkg_id, "name": "Commerce" }))?;
    for (c, x, y, h) in &placed {
        run(&mut session, "diagram.place", serde_json::json!({ "diagram": d, "element": c, "x": x, "y": y, "width": 290, "height": h }))?;
    }
    run(&mut session, "diagram.connect-all", serde_json::json!({ "diagram": d }))?;
    for (line, named) in lines.iter().filter(|(_, named)| *named) {
        let store = session.doc();
        let edge = store
            .by_uuid(archforge_mof::Uuid::parse(line).ok_or("a line's id")?)
            .and_then(|l| archforge_diagram::views_of(store, l).1.first().copied())
            .and_then(|e| store.uuid(e).ok());
        if let (Some(edge), true) = (edge, named) {
            run(&mut session, "diagram.label", serde_json::json!({ "edge": edge.to_string(), "kind": "name", "at": "middle" }))?;
        }
    }
    run(&mut session, "view.select", serde_json::json!({ "ids": [] }))?;
    session.mark_saved().map_err(|e| e.to_string())?;
    shell.session = Some(session);
    let d = Uuid::parse(&d).ok_or("the diagram's id")?;
    shell.explorer.expanded.extend(Uuid::parse(&pkg_id));
    crate::canvas::open(shell, d);
    Ok(d)
}
