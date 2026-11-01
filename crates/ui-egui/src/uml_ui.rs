//! What UML brings to the shell (archforge-3c5.1.1): the class diagram's tools and gestures (the
//! Class tool places a class named in place, its card renames in place), and its packages and
//! classes in the Models tree, the search and Properties.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::commands::{classes, package_of, packages};
use egui::Rect;
use serde_json::json;

use crate::Shell;
use crate::explorer::{Row, Tree, fresh_name};
use crate::i18n::tr;
use crate::kinds::{DiagramKind, KindTool, ToolUse};
use crate::notation_ui::{Added, Entry, NotationUi, Offer, Words};
use crate::panels::Subject;
use crate::ux::{Palette, icons};

fn class(store: &Store, o: Obj) -> Option<archforge_mof::ClassId> {
    store.class_of(o).ok()
}

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// The class diagram in the shell.
pub(crate) struct UmlKind;

const TOOLS: &[KindTool] = &[KindTool { key: "class", icon: &icons::TOOL_ENTITY, hint: "Class", use_: ToolUse::Place, chevron: None }];

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

    fn icon(&self) -> &'static crate::ux::Icon {
        &icons::DIAGRAM
    }

    fn owner_word(&self) -> String {
        tr("Package")
    }

    fn new_owner(&self) -> String {
        tr("New package")
    }

    fn owners(&self, store: &Store) -> Vec<Obj> {
        packages(store)
    }

    fn owner_of(&self, store: &Store, selected: Obj) -> Option<Obj> {
        if class(store, selected) == Some(archforge_uml::PACKAGE) { Some(selected) } else { package_of(store, selected) }
    }

    fn add_owner(&self, shell: &mut Shell, name: &str) -> Result<String, String> {
        crate::editing::run(shell, "uml.package.add", json!({ "name": name }))
            .map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    }

    fn takes_part(&self, store: &Store, element: Obj) -> bool {
        class(store, element) == Some(archforge_uml::CLASS)
    }

    /// A class in the diagram's package, its card centred under the click, as one undo step; then
    /// its name is open for typing.
    fn place(&self, shell: &mut Shell, _tool: &'static str, d: Obj, x: f64, y: f64) {
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
        let Some(name) = fresh_name("Class", classes(store, package).into_iter().map(|c| name(store, c)), false) else {
            shell.canvas.notice = Some(tr("No free name is left"));
            return;
        };
        let (w, h) = archforge_uml::CLASS_SIZE;
        let (px, py) = ((x - w / 2.0).round(), (y - h / 2.0).round());
        let name_for_batch = name.clone();
        let out = session.batch(&tr("Add class"), |b| {
            let c = b.execute("uml.class.add", &json!({ "package": package_id.to_string(), "name": name_for_batch }))?;
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

    /// No connect tool yet: relating classes comes with its own journey.
    fn connect(&self, _: &mut Shell, _: &'static str, _: Obj, _: Uuid, _: Uuid) {}

    fn connect_to_new(&self, _: &mut Shell, _: &'static str, _: Obj, _: Uuid, _: f64, _: f64) {}

    fn rename(&self, shell: &mut Shell, element: Uuid, name: &str) -> Result<(), String> {
        crate::editing::run(shell, "uml.class.rename", json!({ "class": element.to_string(), "name": name })).map(|_| ())
    }

    /// The class in Properties; its members' editor comes with modelling classes.
    fn open(&self, shell: &mut Shell, element: Uuid, _row: Option<usize>) {
        let _ = crate::editing::run(shell, "view.select", json!({ "ids": [element.to_string()] }));
        shell.state.show_panel(crate::state::Panel::Properties);
    }
}

/// UML in the shell's explorers, search and panels.
pub(crate) struct UmlUi;

impl NotationUi for UmlUi {
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }

    fn is_top(&self, store: &Store, member: Obj) -> bool {
        class(store, member) == Some(archforge_uml::PACKAGE)
    }

    /// A package with its classes.
    fn tree(&self, store: &Store, package: Obj, t: &Tree<'_>) -> Vec<Row> {
        let all = classes(store, package);
        let under: Vec<Row> = all
            .iter()
            .filter_map(|c| {
                let label = name(store, *c);
                if t.matches(&label) { t.row(*c, 1, "class", &icons::TABLE, label) } else { None }
            })
            .collect();
        let expandable = if t.filtering() { !under.is_empty() } else { !all.is_empty() };
        let label = name(store, package);
        let mut out = Vec::new();
        if t.matches(&label) || !under.is_empty() {
            let expanded = expandable && t.open(package);
            out.extend(t.row(package, 0, "package", &icons::FOLDER, label).map(|row| Row { expandable, expanded, ..row }));
            if expanded {
                out.extend(under);
            }
        }
        out
    }

    fn offers(&self, store: &Store, selection: Option<Obj>) -> Vec<Offer> {
        let offer = |key, word: String| Offer { key, word };
        match selection.and_then(|o| class(store, o)) {
            // A class goes into the selected package, or the selected class's.
            Some(c) if c == archforge_uml::PACKAGE || (c == archforge_uml::CLASS && selection.and_then(|o| package_of(store, o)).is_some()) => {
                vec![offer("class", tr("Class")), offer("package", tr("Package"))]
            }
            _ => vec![offer("package", tr("Package"))],
        }
    }

    fn add(&self, shell: &mut Shell, key: &str, sel: Option<Obj>) -> Option<Result<Added, String>> {
        let store = shell.session.as_ref()?.doc();
        let no_name = || Some(Err(tr("No free name is left")));
        let mut parents = Vec::new();
        let out = match key {
            "package" => {
                let Some(name) = fresh_name("Package", packages(store).into_iter().map(|p| name(store, p)), false) else { return no_name() };
                crate::explorer::run(shell, "uml.package.add", json!({ "name": name }))
            }
            "class" => {
                let package = sel.and_then(|o| if class(store, o) == Some(archforge_uml::PACKAGE) { Some(o) } else { package_of(store, o) })?;
                let Some(name) = fresh_name("Class", classes(store, package).into_iter().map(|c| name(store, c)), false) else { return no_name() };
                let package_id = store.uuid(package).ok()?;
                parents.push(package_id);
                crate::explorer::run(shell, "uml.class.add", json!({ "package": package_id.to_string(), "name": name }))
            }
            _ => return None,
        };
        match out {
            Ok(out) => out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse).map(|id| Ok(Added { id, parents })),
            Err(e) => Some(Err(e)),
        }
    }

    fn rename(&self, store: &Store, obj: Obj) -> Option<(&'static str, &'static str)> {
        match class(store, obj)? {
            c if c == archforge_uml::PACKAGE => Some(("uml.package.rename", "package")),
            c if c == archforge_uml::CLASS => Some(("uml.class.rename", "class")),
            _ => None,
        }
    }

    fn has_children(&self, store: &Store, obj: Obj) -> bool {
        class(store, obj) == Some(archforge_uml::PACKAGE) && !classes(store, obj).is_empty()
    }

    fn foldable(&self, _store: &Store, package: Obj) -> Vec<Obj> {
        vec![package]
    }

    fn parents(&self, store: &Store, obj: Obj) -> Vec<Obj> {
        package_of(store, obj).into_iter().collect()
    }

    fn entry(&self, store: &Store, o: Obj) -> Option<Entry> {
        match class(store, o)? {
            c if c == archforge_uml::PACKAGE => Some(Entry { icon: &icons::FOLDER, order: 1, label: name(store, o), detail: tr("Package") }),
            c if c == archforge_uml::CLASS => {
                let at = package_of(store, o).map(|p| name(store, p)).unwrap_or_default();
                let detail = if at.is_empty() { tr("Class") } else { format!("{} · {at}", tr("Class")) };
                Some(Entry { icon: &icons::TABLE, order: 2, label: name(store, o), detail })
            }
            _ => None,
        }
    }

    fn words(&self, store: &Store, o: Obj) -> Option<Words> {
        let words = |kind: String, word: String, this: String, on_this: String| Words { kind, word, this, on_this, name: name(store, o) };
        Some(match class(store, o)? {
            c if c == archforge_uml::PACKAGE => words(tr("Package · model"), tr("Package"), tr("This package"), tr("On this package · {n}")),
            c if c == archforge_uml::CLASS => words(tr("Class · model, in every diagram"), tr("Class"), tr("This class"), tr("On this class · {n}")),
            _ => return None,
        })
    }

    /// A package's or a class's name; its members come with modelling classes.
    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
        let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
        let Some((cmd, key)) = subject.obj.and_then(|o| self.rename(store, o)) else { return };
        let width = r.width() - 2.0 * crate::panels::PAD;
        crate::panels::name_field(shell, ui, p, egui::pos2(r.left() + crate::panels::PAD, y), width, subject, cmd, key);
    }
}
