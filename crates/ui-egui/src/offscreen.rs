//! Offscreen rendering of the window, behind the `snapshot` feature: a fresh shell in
//! the headless harness, drawn once settled and rendered to RGBA.

use crate::theme::ThemeKind;
use crate::{Folder, Services, Shell};

/// What a render shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    /// Home with a pinned folder and four recent projects, shown as a gallery.
    HomeGallery,
    /// The same with its search open, nothing typed yet.
    HomeSearch,
    /// Home with the Settings dialog open, a name entered.
    HomeSettings,
    /// Home with About Archforge open, its support destinations not set up yet.
    HomeAbout,
    /// Settings → AI Agents with both programs found and nothing set up yet (A2).
    SettingsAgentsEmpty,
    /// The same with Claude's program missing, its install steps shown, and Codex on an API key
    /// (A3), checked.
    SettingsAgentsMissing,
    /// The same ready: Claude's key saved, Codex signed in with ChatGPT, Chat allowed the model,
    /// checked (A4).
    SettingsAgentsReady,
    /// The same with Claude's key refused by its provider and Codex's sign-in expired, checked
    /// (A5).
    SettingsAgentsRefused,
    /// The same with Codex signing in with a device code (A6).
    SettingsAgentsDeviceCode,
    /// The same on a system without a secure store: no key can be saved, Codex's ChatGPT sign-in
    /// still works (A7).
    SettingsAgentsNoStore,
    /// The same on a system agents do not run on yet (A8).
    SettingsAgentsNotAvailable,
    /// Home when no Dolt Archforge can use is found: the notice with Install Dolt and Look again.
    HomeDoltMissing,
    /// The New project dialog over Home, a name typed and the usual folder.
    NewProject,
    /// The same after a creation failed for want of Dolt.
    NewProjectFailed,
    /// The same with a folder chosen with Change… in place of the usual one.
    NewProjectFolder,
    /// The workspace of a project just created.
    Workspace,
    /// The same after `+` added the first schema, its name open for typing.
    WorkspaceSchema,
    /// A table with columns selected, its Properties shown.
    WorkspaceTable,
    /// The same table with the Skills tab open on it, a skill on the schema inherited.
    WorkspaceSkills,
    /// The table's naming skill open in its tab, its Markdown in Write.
    WorkspaceSkillTab,
    /// The same skill in Preview.
    WorkspaceSkillPreview,
    /// The same Preview with a link clicked, its confirmation open.
    WorkspaceSkillLink,
    /// A skill just made with the Skills panel's +: its tab, the template, the name selected.
    WorkspaceSkillNew,
    /// The same table with the key editor open on its unique constraint.
    WorkspaceKeyEditor,
    /// The same table with the column editor open on its email column, More open.
    WorkspaceColumnEditor,
    /// The same table with the index editor open on its index, More open.
    WorkspaceIndexEditor,
    /// An empty diagram with Party and its subtypes Person and Company drawn, Party selected
    /// (slice S6).
    WorkspaceSubtypes,
    /// The table's schema selected, with a domain and an enum type in its Types section and in
    /// the Models tree.
    WorkspaceSchemaTypes,
    /// The table's schema selected, with a sequence owned by its key and a view in its Sequences
    /// and Views sections and in the Models tree (slice S7).
    WorkspaceSchemaObjects,
    /// The diagram of two tables with a table of the CRM schema placed beside them, dimmed and
    /// dashed, its relationship to Order drawn (slice S7).
    WorkspaceCrossSchema,
    /// The diagram of two tables with Invoice and Product placed so that their lines from Customer
    /// would cross Order: right-angled lines go around it.
    WorkspaceRouting,
    /// A class diagram of a package with three classes, the package open in the tree
    ///.
    WorkspaceClassDiagram,
    /// The sample class model to read: members, every line kind and end.
    WorkspaceClassDiagramRead,
    /// New diagram's first step: a card for each kind, Class diagram chosen.
    WorkspaceNewDiagramKind,
    /// New diagram's second step for a class diagram: its name and the root package.
    WorkspaceNewDiagramDetails,
    /// A class diagram of the root package with two packages on it, Sales selected with its Open.
    WorkspacePackages,
    /// Inside Sales: its classes, a package of its own, and the path bar back to the root.
    WorkspaceInsidePackage,
    /// The root's diagram with Sales selected and Properties' Parent package menu open (AR-102).
    WorkspacePackageParent,
    /// The root's diagram zoomed out to 10 %, its notice saying no package is above (AR-110).
    WorkspacePackageTop,
    /// The Diagrams explorer with its groups: two class diagrams, one of a nested package and
    /// open, and an ER diagram.
    WorkspaceDiagrams,
    /// The sample class model with the Chat panel open: Coming soon.
    WorkspaceChat,
    /// A class diagram of three cards: a new class with its first operation at the default size, one
    /// grown to show its members, and two made short by hand saying the rows they hide.
    WorkspaceClassSizing,
    /// The sample class model with Order selected: its Properties, Abstract and its members
    ///.
    WorkspaceClassMembers,
    /// The same with Customer's operation rename open in its editor, its parameter on a row.
    WorkspaceMemberEditor,
    /// The sample class model with Catalog selected and a new attribute typed on its card.
    WorkspaceInlineMember,
    /// The sample class model with the association "paid by" selected: its line's end handles and
    /// its Properties with both ends.
    WorkspaceAssociation,
    /// The same with its Payment end's multiplicity presets open, the current one ticked.
    WorkspaceMultiplicityPresets,
    /// The same with `5..2` typed into the Payment end's multiplicity and refused under it.
    WorkspaceMultiplicityRefused,
    /// The sample class model with the Relationship tool active and its kinds' menu open.
    WorkspaceRelateKinds,
    /// A package of interfaces: Payable with its members, selected in Properties, Refundable
    /// specializing it, and Invoice associated with it.
    WorkspaceInterface,
    /// A package of enumerations: OrderStatus with its four literals, selected in Properties and
    /// open in Models, Priority with none, and the class Order whose attributes they type.
    WorkspaceEnumeration,
    /// A package of values: the data type Money with its attributes and operation, selected in
    /// Properties and open in Models, Price specializing it, and the class Order whose attribute
    /// and operation it types.
    WorkspaceDataType,
    /// A package of codes: the primitive types Sku and Gtin, Gtin specializing Sku, and the class
    /// Product whose attributes and operation they type; Sku selected in Properties, the package
    /// open in Models.
    WorkspacePrimitiveType,
    /// The same at a working zoom with a literal's draft row open at the end of OrderStatus's
    /// card, "Returned" typed, Properties listing the literals with their pencils.
    WorkspaceLiterals,
    /// The same with the classifier tool's kinds menu open, Interface the current kind.
    WorkspaceClassifierKinds,
    /// Two classes, Order and Invoice, each realizing the interface Payable: Order's realization
    /// selected with its class and interface in Properties, and the Relationship tool showing
    /// Interface realization.
    WorkspaceRealization,
    /// Two classes, Customer and Product, joined by the association class Review, its card with
    /// two attributes above its line's middle and tied to it: Review selected, its ends, attributes
    /// and operations in Properties, and the Relationship tool showing Association class.
    WorkspaceAssociationClass,
    /// The class Order with its attributes' modifiers on its card (`{readOnly}`, a derived total
    /// with its default, a status defaulting to a literal, ordered lines) and its association to
    /// OrderLine whose end says `/items 0..* = none {ordered}`; Order selected with the attribute
    /// editor open on total, its modifiers in Properties (AR-126).
    WorkspaceModifiers,
    /// The sample class model with PricingService final, `{final}` after its name on its card, and
    /// Payment selected after No subclasses was refused on it: Properties names its subclass
    /// CardPayment and offers to select the generalization (AR-137).
    WorkspaceFinal,
    /// The sample class model with Order's total() leaf (`+ total() : Money {leaf}`) and
    /// PricingService final and leaf (`{final, leaf}` after its name), CardPayment's amount
    /// redefining Payment's; Payment selected with its attribute editor open on amount, No
    /// redefinition (leaf) refused there, naming CardPayment::amount (AR-138).
    WorkspaceLeaf,
    /// The sample class model with BankTransfer and WalletPayment beside CardPayment, the three
    /// generalizations to Payment grouped in the set method {complete, disjoint}, its dashed line
    /// across them and its label beside it; the set selected, its Properties open (AR-129).
    WorkspaceGeneralizationSets,
    /// The sample class model with Order's composition to OrderLine named contains, its OrderLine
    /// end line at 0..1 and its Order end qualified by lineNo : Integer, the qualifier box drawn
    /// against Order; the association selected with the qualifier editor open on lineNo (AR-127).
    WorkspaceQualifiers,
    /// The same, a second qualifier lineNo : String applied on the Order end and refused in place:
    /// the name ringed and the reason in a tinted box, nothing written (AR-127).
    WorkspaceQualifierRefused,
    /// The sample class model with the enumeration Priority nested in Order: Models lists it under
    /// Order, `nested` after its name; on the diagram its card is tied to Order's by the line ending
    /// in ⊕ at Order; it is selected, Properties showing its Owner, Order (AR-132, D2).
    WorkspaceNestedClassifiers,
    /// The sample class model with Repository a template of T : Class and Key : DataType =
    /// Integer, its signature box over its card's top-right corner; Repository selected with the
    /// template parameter editor open on Key (AR-130, C1).
    WorkspaceTemplate,
    /// The same, the editor's Kind menu open: the supported kinds, Data type ticked, then Value and
    /// Operation disabled, saying not yet (AR-130, C4).
    WorkspaceTemplateKinds,
    /// The same, OrderRepository binding Key to Integer, and Key's removal refused under the rows
    /// with Show the binding (AR-130, C5).
    WorkspaceTemplateInUse,
    /// The sample class model with the profile Persistence beside Commerce, which applies it: its
    /// diagram open with the «stereotype» cards Entity, Id and Reference under the «metaclass» cards
    /// Class, Property and Association, joined by extensions; Models open on Persistence; Entity
    /// selected, its Properties showing Extends and its properties, the editor open on cached
    /// (AR-134, F1).
    WorkspaceProfiles,
    /// The sample with Persistence applied to Commerce and used there: Commerce selected, Unapply
    /// refused under its Applied profiles, the five uses counted (AR-134, F6).
    WorkspaceProfileRefused,
    /// OrderRepository binds Repository, T to Order and Key to Integer: its dashed line labelled
    /// `«bind» <T -> Order, Key -> Integer>` selected, Properties showing the binding and its
    /// substitutions (AR-131, C2).
    WorkspaceTemplateBinding,
    /// A binding being drawn from OrderRepository to Repository: its draft in Properties, Key on its
    /// default, Apply pressed with T still without an argument and the refusal said (AR-131, C3).
    WorkspaceTemplateBindingNew,
    /// The sample class model's diagram Recommendations, Customer and Product side by side and
    /// Catalog under them, the Relationship tool drawing an n-ary association: Customer and Product
    /// picked, the preview diamond between them, the pointer on Catalog, the gesture hint counting
    /// two ends and Properties showing the draft (AR-128, A3).
    WorkspaceNaryDrawing,
    /// The same diagram with the n-ary association recommends drawn, selected, its diamond and legs
    /// in the accent, Properties showing its ends with the editor open on its Catalog end (AR-128,
    /// A4).
    WorkspaceNary,
    /// The same, the bin of the Catalog end pressed: Properties asks to keep three ends or make it
    /// binary, Keep 3 ends focused (AR-128, A5).
    WorkspaceNaryBinary,
    /// The sample with Persistence applied and used: Commerce's diagram showing the stereotype
    /// values, Order selected, its Stereotypes section with «Entity», table set to orders and cached
    /// at its default, the cards reading «Entity», `«Id» + id : Integer` and «Reference» on places
    /// (AR-135, F3).
    WorkspaceStereotypes,
    /// The same, the Stereotypes picker open on Order: Entity applied and ticked, Auditable to
    /// apply, then Id, Reference and Tracked disabled with why (AR-135, F4).
    WorkspaceStereotypePicker,
    /// The user manual's class-model guide, first step: the root package's diagram Commerce with
    /// Customer (its name typed) and Order, a draft row open on Order's card with `number :
    /// Integer` typed.
    GuideClasses,
    /// The guide with Order's number and pay() typed, the Relationship tool active and its kinds'
    /// menu open, Association ticked.
    GuideRelateKinds,
    /// The guide with the association places drawn from Customer to Order, its ends set, selected
    /// with both ends in Properties.
    GuideAssociation,
    /// The guide after the association, the classifier tool's kinds menu open: Class ticked, the
    /// keys on Enumeration.
    GuideClassifierKinds,
    /// The guide with the enumeration OrderStatus beside Order: three literals, a draft row at its
    /// end with "Cancelled" typed, its literals in Properties.
    GuideLiterals,
    /// The guide with the data type Money, which types Order's total, selected; Order's status
    /// typed by OrderStatus.
    GuideDataType,
    /// The guide with the primitive type Email, which types Customer's email, selected.
    GuidePrimitiveType,
    /// The guide with the interface Payable and Order's realization of it, the realization
    /// selected.
    GuideRealization,
    /// The guide with the class Product and the association class OrderLine between Order and
    /// Product, OrderLine selected.
    GuideAssociationClass,
    /// The guide with Order's number read only and its status defaulting to Placed, Order
    /// selected and the attribute editor open on status.
    GuideModifiers,
    /// The guide with Order's two constraints (AR-133): positiveNumber in braces under its card and
    /// paidInFull as a constraint note linked to Order and OrderStatus, Order selected and the
    /// constraint editor open on paidInFull.
    GuideConstraints,
    /// The guide's finished model: the whole diagram, nothing selected, Properties folded away.
    GuideResult,
    /// The user manual's further sections (AR-141), on the guide's finished model: Order's static
    /// operation nextNumber() underlined on its card, Order selected and the operation editor open
    /// on it, Static ticked.
    GuideStatic,
    /// The guide's association places qualified at Customer's end by number : Integer, Order's end
    /// at 0..1 per qualifier value; places selected with the qualifier editor open on number.
    GuideQualifier,
    /// The guide's diagram Recommendations: the n-ary association recommends joining Customer,
    /// Product and Catalog by a diamond, selected, the editor open on its Catalog end.
    GuideNary,
    /// The guide's diagram Payments: CardPayment, BankTransfer and WalletPayment specialising
    /// Payment, grouped in the generalization set method {complete, disjoint}, the set selected.
    GuideGeneralizationSet,
    /// The same with CardPayment final and Payment's amount leaf: Payment selected and the
    /// attribute editor open on amount, No redefinition (leaf) ticked.
    GuideFinalLeaf,
    /// The guide's enumeration OrderStatus moved inside Order: its card tied to Order's by the line
    /// ending in ⊕, OrderStatus selected with its Owner, Models listing it under Order.
    GuideNested,
    /// The guide's diagram Storage: Repository a template of T : Class and Key : DataType = Integer
    /// with items : T [*] and find(key : Key) : T, selected, the parameter editor open on Key.
    GuideTemplate,
    /// The same with OrderRepository binding Repository, T to Order and Key on its default: the
    /// binding selected, its substitutions in Properties.
    GuideTemplateBinding,
    /// The guide's profile Persistence with the stereotype Entity, extending Class, with the
    /// property table : String: the profile's diagram open, Entity selected, Models open on
    /// Persistence.
    GuideProfile,
    /// The guide's root package applying Persistence and Order carrying «Entity» with table set to
    /// orders: Commerce showing the stereotype values, Order selected, its Stereotypes section open.
    GuideStereotype,
    /// The sample class model edited, unsaved, and its close asking Save, Don't save or Cancel
    ///.
    WorkspaceCloseAsk,
    /// The routing sample with the top bar's search open on "order".
    WorkspaceSearch,
    /// The Diagrams explorer with the diagram "dbo" open, its one table Company placed and its key
    /// column selected: the owner's screen of 2026-10-31.
    WorkspaceColumnSelected,
    /// The diagram of two tables with a mapping table between Order and Tag selected: its
    /// Properties offer to collapse it, Order's key column shows its link mark.
    WorkspaceMappingTable,
    /// A first diagram of a schema, just created and open, empty.
    WorkspaceDiagramEmpty,
    /// A diagram of two tables and their relationship, the first table selected.
    WorkspaceDiagram,
    /// The same with a removal's toast and its Undo, clear of the minimap.
    WorkspaceToast,
    /// The filled diagram with its schema selected, export skills on the project and the schema,
    /// and File → Generate… open over it.
    WorkspaceGenerate,
    /// The same with the Appearance panel open on the first table, its fill and border its own.
    WorkspaceAppearance,
    /// The same with the relationship selected and its own width.
    WorkspaceAppearanceLine,
    /// The first table's Appearance with the picker open on its fill.
    WorkspaceAppearancePicker,
    /// An empty diagram with its first entity just placed, its name open for typing.
    WorkspaceEntityNew,
    /// The diagram of two tables with the Relationship tool active and the cardinality popover open.
    WorkspaceCardinality,
    /// The same diagram with the relationship selected, its Properties shown.
    WorkspaceRelationship,
}

/// Renders Home at `width` by `height` logical pixels, `scale` pixels a point, in `theme`:
/// `(width, height, rgba)` in pixels.
pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String> {
    render_screen(width, height, scale, theme, Screen::Home)
}

/// [`render`] of a given screen.
pub fn render_screen(width: f32, height: f32, scale: f32, theme: ThemeKind, screen: Screen) -> Result<(u32, u32, Vec<u8>), String> {
    crate::i18n::set_language("en");
    let n = std::sync::atomic::AtomicU64::new(0);
    let ids: std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync> = std::sync::Arc::new(move || {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&i.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    });
    let mut shell = Shell::new(Services { ids: Some(ids), ..Services::default() });
    shell.state.theme = theme;
    if matches!(screen, Screen::HomeGallery | Screen::HomeSearch) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        let now = crate::home::now();
        for (name, ago) in [("Billing", 9 * 86_400), ("Warehouse", 2 * 86_400), ("Customer care", 3 * 3_600), ("Commerce platform", 600)] {
            shell.state.remember(name, std::path::Path::new(&format!("/Users/robin/Documents/Archforge/{name}.archforge")), now - ago);
        }
        shell.state.recent_view = crate::state::RecentView::Gallery;
        shell.state.pin(std::path::Path::new("/Users/robin/Projects/Architecture"));
        shell.home_search.open = screen == Screen::HomeSearch;
    }
    if screen == Screen::HomeSettings {
        shell.state.user_name = "Ada Lovelace".into();
        crate::settings::open(&mut shell);
    }
    if screen == Screen::HomeAbout {
        crate::support::open(&mut shell);
    }
    sample_agents(&mut shell, screen);
    if screen == Screen::HomeDoltMissing {
        shell.dolt_missing = Some("ARCHFORGE_DOLT is not set and no dolt is on the PATH".into());
    }
    if !matches!(
        screen,
        Screen::Home
            | Screen::HomeGallery
            | Screen::HomeSearch
            | Screen::HomeSettings
            | Screen::HomeAbout
            | Screen::SettingsAgentsEmpty
            | Screen::SettingsAgentsMissing
            | Screen::SettingsAgentsReady
            | Screen::SettingsAgentsRefused
            | Screen::SettingsAgentsDeviceCode
            | Screen::SettingsAgentsNoStore
            | Screen::SettingsAgentsNotAvailable
            | Screen::HomeDoltMissing
            | Screen::NewProject
            | Screen::NewProjectFailed
            | Screen::NewProjectFolder
    ) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        shell.open_project(crate::Opened {
            name: "Commerce platform".into(),
            path: "/Users/robin/Documents/Archforge/Commerce platform.archforge".into(),
            ..Default::default()
        });
    }
    if matches!(screen, Screen::NewProject | Screen::NewProjectFailed | Screen::NewProjectFolder) {
        shell.services.projects_folder = Some(Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() });
        crate::new_project::open(&mut shell);
        if let Some(d) = shell.new_project.as_mut() {
            d.name = "Commerce platform".into();
            if screen == Screen::NewProjectFailed {
                d.show_failure(crate::CreateError::DoltMissing {
                    detail: "no dolt found (ARCHFORGE_DOLT is not set and no dolt is on the PATH)".into(),
                });
            }
            if screen == Screen::NewProjectFolder {
                d.folder = Some("/Volumes/Work/Clients/Acme".into());
            }
        }
        // Change… as a host with a folder picker shows it.
        if screen == Screen::NewProjectFolder {
            shell.services.pick_folder = Some(std::sync::Arc::new(|_| None));
        }
    }
    if screen == Screen::WorkspaceSchema {
        // The session first, as the first frame would make it, then the schema the + adds.
        shell.adopt_project();
        crate::explorer::add(&mut shell, "schema");
    }
    if matches!(
        screen,
        Screen::WorkspaceTable
            | Screen::WorkspaceSkills
            | Screen::WorkspaceSkillTab
            | Screen::WorkspaceSkillPreview
            | Screen::WorkspaceSkillLink
            | Screen::WorkspaceSkillNew
            | Screen::WorkspaceKeyEditor
            | Screen::WorkspaceColumnEditor
            | Screen::WorkspaceIndexEditor
            | Screen::WorkspaceSchemaTypes
            | Screen::WorkspaceSchemaObjects
    ) {
        shell.adopt_project();
        let skill_tab = matches!(screen, Screen::WorkspaceSkillTab | Screen::WorkspaceSkillPreview | Screen::WorkspaceSkillLink);
        sample_table(&mut shell, screen == Screen::WorkspaceSkills || skill_tab || screen == Screen::WorkspaceSkillNew)?;
        if skill_tab {
            sample_skill_tab(&mut shell, screen != Screen::WorkspaceSkillTab)?;
        }
        if screen == Screen::WorkspaceSkillNew {
            let customer = shell.session.as_ref().and_then(|s| s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok()));
            crate::skill_tab::create(&mut shell, customer);
        }
        if screen == Screen::WorkspaceSkillLink
            && let Some(doc) = shell.canvas.skills.first_mut()
        {
            doc.link = Some("https://docs.example.com.sign-in.accounts.verify.example.org/naming/columns?section=keys".into());
        }
        if screen == Screen::WorkspaceSchemaTypes {
            sample_types(&mut shell)?;
        }
        if screen == Screen::WorkspaceSchemaObjects {
            sample_objects(&mut shell)?;
        }
        if screen == Screen::WorkspaceColumnEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let email = store
                .alive()
                .find(|o| archforge_core::name_of(store, *o) == "email")
                .and_then(|a| Some((store.uuid(a).ok()?, archforge_er::rules::entity_of(store, a)?)));
            if let Some((column, entity)) = email
                && let Ok(entity) = store.uuid(entity)
            {
                crate::panels::open_column(&mut shell, entity, Some(column));
                if let Some(c) = shell.panels.column.as_mut() {
                    c.more = true;
                }
            }
        }
        if screen == Screen::WorkspaceIndexEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let customer = store.alive().find(|o| archforge_core::name_of(store, *o) == "Customer");
            if let Some(e) = customer
                && let Ok(entity) = store.uuid(e)
                && let Some(index) = crate::indexes::rows(store, e).into_iter().next()
            {
                crate::indexes::open(&mut shell, entity, Some(&index));
                if let Some(i) = shell.panels.index.as_mut() {
                    i.more = true;
                }
            }
        }
        if screen == Screen::WorkspaceKeyEditor
            && let Some(session) = shell.session.as_ref()
        {
            let store = session.doc();
            let customer = store.alive().find(|o| archforge_core::name_of(store, *o) == "Customer");
            if let Some(e) = customer
                && let Ok(entity) = store.uuid(e)
                && let Some(unique) = crate::keys::rows(store, e).into_iter().find(|k| !k.primary)
            {
                crate::keys::open(&mut shell, entity, Some(&unique));
            }
        }
    }
    if screen == Screen::WorkspaceColumnSelected {
        shell.adopt_project();
        sample_column_selected(&mut shell)?;
    }
    if screen == Screen::WorkspaceClassDiagram {
        shell.adopt_project();
        sample_class_diagram(&mut shell)?;
    }
    if matches!(screen, Screen::WorkspaceNewDiagramKind | Screen::WorkspaceNewDiagramDetails) {
        shell.adopt_project();
        sample_diagram(&mut shell, true)?;
        crate::diagrams::open_new(&mut shell);
        if let Some(edit) = shell.diagrams.new.as_mut() {
            edit.kind = archforge_uml::CLASS_DIAGRAM;
            edit.schema = None;
            edit.name = "Domain model".into();
            if screen == Screen::WorkspaceNewDiagramDetails {
                edit.step = crate::diagrams::Step::Details;
            }
        }
    }
    if matches!(screen, Screen::WorkspacePackages | Screen::WorkspaceInsidePackage | Screen::WorkspacePackageParent) {
        shell.adopt_project();
        sample_packages(&mut shell, screen == Screen::WorkspaceInsidePackage)?;
        if screen == Screen::WorkspacePackageParent {
            shell.state.show_panel(crate::state::Panel::Properties);
            shell.panels.menu = Some(crate::panels::Menu::PackageParent);
        }
    }
    if screen == Screen::WorkspacePackageTop {
        shell.adopt_project();
        sample_packages(&mut shell, false)?;
        shell.canvas.fit_pending = false;
        shell.canvas.zoom = crate::canvas::ZOOM_RANGE.0;
        shell.canvas.pan = egui::vec2(420.0, 380.0);
        shell.canvas.notice = Some(crate::i18n::tr("This is the top package: zooming out goes no further"));
    }
    if screen == Screen::WorkspaceDiagrams {
        shell.adopt_project();
        sample_diagrams(&mut shell)?;
    }
    if screen == Screen::WorkspaceClassSizing {
        shell.adopt_project();
        sample_class_sizing(&mut shell)?;
    }
    if screen == Screen::WorkspaceChat {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        shell.state.show_panel(crate::state::Panel::Agent);
    }
    if screen == Screen::WorkspaceClassDiagramRead {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
    }
    if screen == Screen::WorkspaceCloseAsk {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        if let Some(s) = shell.session.as_mut() {
            s.mark_unsaved();
        }
        shell.close_project();
    }
    let association = matches!(screen, Screen::WorkspaceAssociation | Screen::WorkspaceMultiplicityPresets | Screen::WorkspaceMultiplicityRefused);
    if association || screen == Screen::WorkspaceRelateKinds {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        if association {
            let id = {
                let store = shell.session.as_ref().ok_or("no project is open")?.doc();
                let a = store
                    .alive()
                    .find(|o| store.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(store, *o) == "paid by")
                    .ok_or("the sample has no such association")?;
                store.uuid(a).map_err(|e| e.to_string())?
            };
            crate::panels::run(&mut shell, "view.select", serde_json::json!({ "ids": [id.to_string()] }))?;
            shell.state.show_panel(crate::state::Panel::Properties);
            if screen == Screen::WorkspaceMultiplicityPresets {
                shell.panels.menu = Some(crate::panels::Menu::EndMultiplicity(1));
            }
            if screen == Screen::WorkspaceMultiplicityRefused {
                crate::uml_relate::refuse_typed(&mut shell, id, 1, "5..2")?;
            }
        } else {
            shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
            shell.canvas.uml_relation = crate::uml_relate::Relate::Composition;
            shell.canvas.popover = Some("relationship-kinds");
        }
    }
    if matches!(screen, Screen::WorkspaceInterface | Screen::WorkspaceClassifierKinds) {
        shell.adopt_project();
        sample_interfaces(&mut shell)?;
        if screen == Screen::WorkspaceClassifierKinds {
            shell.canvas.uml_classifier = crate::uml_classifier::Classifier::Interface;
            shell.canvas.tool = crate::canvas::Tool::Place(crate::uml_classifier::TOOL);
            shell.canvas.popover = Some(crate::uml_classifier::KINDS);
        }
    }
    if matches!(screen, Screen::WorkspaceEnumeration | Screen::WorkspaceLiterals) {
        shell.adopt_project();
        sample_enumerations(&mut shell)?;
        if screen == Screen::WorkspaceLiterals {
            literal_draft(&mut shell)?;
        }
    }
    if screen == Screen::WorkspaceDataType {
        shell.adopt_project();
        sample_data_types(&mut shell)?;
    }
    if screen == Screen::WorkspacePrimitiveType {
        shell.adopt_project();
        sample_primitive_types(&mut shell)?;
    }
    if screen == Screen::WorkspaceRealization {
        shell.adopt_project();
        sample_realizations(&mut shell)?;
        // The Relationship tool armed after the line was selected, as a click on it leaves the
        // selection: the toolbar shows Interface realization active beside its Properties.
        shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
        shell.canvas.uml_relation = crate::uml_relate::Relate::Realization;
    }
    if screen == Screen::WorkspaceAssociationClass {
        shell.adopt_project();
        sample_association_class(&mut shell)?;
        // The Relationship tool armed after the element was selected, as the realization's screen.
        shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
        shell.canvas.uml_relation = crate::uml_relate::Relate::AssociationClass;
    }
    if screen == Screen::WorkspaceModifiers {
        shell.adopt_project();
        sample_modifiers(&mut shell)?;
    }
    if screen == Screen::WorkspaceFinal {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_final(&mut shell)?;
    }
    if screen == Screen::WorkspaceLeaf {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_leaf(&mut shell)?;
    }
    let mut hover: Option<String> = None;
    if matches!(screen, Screen::WorkspaceNaryDrawing | Screen::WorkspaceNary | Screen::WorkspaceNaryBinary) {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        hover = sample_nary(&mut shell, screen)?;
    }
    if screen == Screen::WorkspaceNestedClassifiers {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_nested(&mut shell)?;
    }
    if screen == Screen::WorkspaceProfiles || screen == Screen::WorkspaceProfileRefused {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_profiles(&mut shell, screen == Screen::WorkspaceProfileRefused)?;
    }
    if screen == Screen::WorkspaceStereotypes || screen == Screen::WorkspaceStereotypePicker {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        let order = crate::uml_stereotypes::put_values(&mut shell)?;
        crate::panels::run(&mut shell, "view.select", serde_json::json!({ "ids": [order.to_string()] }))?;
        shell.state.show_panel(crate::state::Panel::Properties);
        shell.state.show_explorer(crate::state::Explorer::Models);
        if screen == Screen::WorkspaceStereotypePicker {
            shell.panels.menu = Some(crate::panels::Menu::StereotypePick);
        }
    }
    if screen == Screen::WorkspaceGeneralizationSets {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_generalization_sets(&mut shell)?;
    }
    if matches!(screen, Screen::WorkspaceTemplate | Screen::WorkspaceTemplateKinds | Screen::WorkspaceTemplateInUse) {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_template(&mut shell, screen)?;
    }
    if matches!(screen, Screen::WorkspaceTemplateBinding | Screen::WorkspaceTemplateBindingNew) {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_binding(&mut shell, screen)?;
    }
    if screen == Screen::WorkspaceQualifiers || screen == Screen::WorkspaceQualifierRefused {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_qualifiers(&mut shell, screen == Screen::WorkspaceQualifierRefused)?;
    }
    if let Some(step) = guide_step(screen) {
        shell.adopt_project();
        sample_guide(&mut shell, step)?;
    }
    if matches!(
        screen,
        Screen::GuideStatic
            | Screen::GuideQualifier
            | Screen::GuideNary
            | Screen::GuideGeneralizationSet
            | Screen::GuideFinalLeaf
            | Screen::GuideNested
            | Screen::GuideTemplate
            | Screen::GuideTemplateBinding
            | Screen::GuideProfile
            | Screen::GuideStereotype
    ) {
        shell.adopt_project();
        sample_guide_more(&mut shell, screen)?;
    }
    if screen == Screen::WorkspaceInlineMember {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        let class = {
            let store = shell.session.as_ref().ok_or("no project is open")?.doc();
            let c = store
                .alive()
                .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == "Catalog")
                .ok_or("the sample has no such class")?;
            store.uuid(c).map_err(|e| e.to_string())?
        };
        if !crate::uml_inline::open(&mut shell, class, crate::uml_panel::MemberKind::Attribute, None) {
            return Err("the class's card cannot hold a row".into());
        }
        if let Some(inline) = shell.canvas.inline.as_mut() {
            inline.edit.name = "code".into();
            inline.ty = "Integer".into();
        }
        // Catalog at a working zoom near the canvas's top left.
        let at = shell.session.as_ref().and_then(|s| {
            let store = s.doc();
            let n = archforge_diagram::views_of(store, store.by_uuid(class)?).2.into_iter().next()?;
            Some(archforge_diagram::grid::bounds_of(store, n))
        });
        if let Some(b) = at {
            shell.canvas.fit_pending = false;
            shell.canvas.zoom = 1.25;
            shell.canvas.pan = egui::vec2(200.0 - b.x as f32 * 1.25, 160.0 - b.y as f32 * 1.25);
        }
    }
    if matches!(screen, Screen::WorkspaceClassMembers | Screen::WorkspaceMemberEditor) {
        shell.adopt_project();
        crate::uml_ui::open_sample(&mut shell)?;
        sample_class_members(&mut shell, screen == Screen::WorkspaceMemberEditor)?;
    }
    if matches!(
        screen,
        Screen::WorkspaceDiagramEmpty
            | Screen::WorkspaceDiagram
            | Screen::WorkspaceToast
            | Screen::WorkspaceGenerate
            | Screen::WorkspaceAppearance
            | Screen::WorkspaceAppearanceLine
            | Screen::WorkspaceAppearancePicker
            | Screen::WorkspaceEntityNew
            | Screen::WorkspaceCardinality
            | Screen::WorkspaceRelationship
            | Screen::WorkspaceMappingTable
            | Screen::WorkspaceSubtypes
            | Screen::WorkspaceCrossSchema
            | Screen::WorkspaceRouting
            | Screen::WorkspaceSearch
    ) {
        shell.adopt_project();
        sample_diagram(
            &mut shell,
            matches!(
                screen,
                Screen::WorkspaceDiagram
                    | Screen::WorkspaceToast
                    | Screen::WorkspaceGenerate
                    | Screen::WorkspaceAppearance
                    | Screen::WorkspaceAppearanceLine
                    | Screen::WorkspaceAppearancePicker
                    | Screen::WorkspaceCardinality
                    | Screen::WorkspaceRelationship
                    | Screen::WorkspaceMappingTable
                    | Screen::WorkspaceCrossSchema
                    | Screen::WorkspaceRouting
                    | Screen::WorkspaceSearch
            ),
        )?;
        if matches!(screen, Screen::WorkspaceRouting | Screen::WorkspaceSearch) {
            sample_routing(&mut shell)?;
        }
        if screen == Screen::WorkspaceSearch {
            crate::home_search::open_with(&mut shell, "order");
        }
        if screen == Screen::WorkspaceGenerate {
            sample_generate(&mut shell)?;
        }
        if screen == Screen::WorkspaceToast {
            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            let text = crate::i18n::fmt(&crate::i18n::tr("{what} removed"), &[("what", "region")]);
            shell.canvas.toast = Some(crate::connect::Toast { text, until: u64::MAX, undo_label });
        }
        if screen == Screen::WorkspaceCrossSchema {
            sample_cross_schema(&mut shell)?;
        }
        if screen == Screen::WorkspaceMappingTable {
            sample_mapping_table(&mut shell)?;
        }
        if screen == Screen::WorkspaceSubtypes {
            sample_subtypes(&mut shell)?;
        }
        if matches!(screen, Screen::WorkspaceAppearance | Screen::WorkspaceAppearanceLine | Screen::WorkspaceAppearancePicker) {
            sample_appearance(&mut shell, screen)?;
        }
        if screen == Screen::WorkspaceRelationship
            && let Some(session) = shell.session.as_mut()
        {
            let store = session.doc();
            let rel = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).and_then(|o| store.uuid(o).ok());
            if let Some(rel) = rel {
                session.execute("view.select", &serde_json::json!({ "ids": [rel.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
            }
        }
        if screen == Screen::WorkspaceEntityNew {
            sample_new_entity(&mut shell)?;
        }
        if screen == Screen::WorkspaceCardinality {
            shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
            shell.canvas.popover = Some("relationship-types");
        }
    }
    let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(width, height)).with_pixels_per_point(scale).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            // The harness frames its ui with an 8 px margin; the shell draws on the whole window,
            // as it does in the application.
            let window = ui.ctx().content_rect();
            let mut full = ui.new_child(egui::UiBuilder::new().max_rect(window));
            full.set_clip_rect(window);
            shell.ui(&mut full);
        },
        shell,
    );
    harness.run();
    // The pointer over a control the screen names, as the design draws it there.
    if let Some(id) = hover
        && let Some(w) = harness.state().widgets().iter().find(|w| w.id == id)
    {
        let at = egui::pos2((w.rect[0] + w.rect[2]) / 2.0, w.rect[1] + 12.0);
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
    }
    let image = harness.render()?;
    let (w, h) = (image.width(), image.height());
    Ok((w, h, image.into_raw()))
}

/// Settings → AI Agents over Home, in the state `screen` shows, from the fake agents host: what
/// its look finds, Codex's account, and what Check connection or a sign-in says.
fn sample_agents(shell: &mut Shell, screen: Screen) {
    use crate::ai_agents::{AccountError, CodexAccount, CodexSignIn, Key, LoginEvent, LoginWay, Look, Program, Provider, Store, System, fake};
    if !matches!(
        screen,
        Screen::SettingsAgentsEmpty
            | Screen::SettingsAgentsMissing
            | Screen::SettingsAgentsReady
            | Screen::SettingsAgentsRefused
            | Screen::SettingsAgentsDeviceCode
            | Screen::SettingsAgentsNoStore
            | Screen::SettingsAgentsNotAvailable
    ) {
        return;
    }
    let saved = |last4: &str| Key::Saved { last4: last4.into() };
    let signed_in = CodexAccount::SignedIn { email: "you@example.com".into(), plan: Some("ChatGPT Plus".into()) };
    let mut look = Look {
        store: if cfg!(target_os = "linux") { Store::SecretService } else { Store::MacKeychain },
        claude: fake::found(Provider::Claude),
        codex: fake::found(Provider::Codex),
        claude_key: Key::Absent,
        codex_key: Key::Absent,
    };
    let mut account = CodexAccount::NotSignedIn;
    match screen {
        Screen::SettingsAgentsMissing => {
            look.claude = Program::Missing;
            look.codex_key = saved("9c1E");
            shell.state.agents.default_agent = Provider::Codex;
            shell.state.agents.codex_sign_in = CodexSignIn::ApiKey;
            shell.state.agents.chat_uses_model = true;
        }
        Screen::SettingsAgentsReady | Screen::SettingsAgentsRefused | Screen::SettingsAgentsDeviceCode => {
            look.claude_key = saved("4F2a");
            shell.state.agents.claude_model = "sonnet".into();
            shell.state.agents.chat_uses_model = true;
            account = match screen {
                Screen::SettingsAgentsReady => signed_in.clone(),
                Screen::SettingsAgentsRefused => CodexAccount::Expired { email: "you@example.com".into() },
                _ => CodexAccount::NotSignedIn,
            };
        }
        Screen::SettingsAgentsNoStore => {
            look.store = Store::Missing { why: "no Secret Service answers on the session bus".into() };
            account = signed_in.clone();
            shell.state.agents.claude_model = "sonnet".into();
            shell.state.agents.chat_uses_model = true;
        }
        _ => {}
    }
    let host = std::sync::Arc::new(fake::Fake {
        system: if screen == Screen::SettingsAgentsNotAvailable { System::NotYet } else { System::Supported },
        look: std::sync::Mutex::new(look.clone()),
        account: std::sync::Mutex::new(Ok(account.clone())),
        login: std::sync::Mutex::new(vec![LoginEvent::DeviceCode {
            code: "WXYZ-4821".into(),
            url: "https://auth.openai.com/codex/device".into(),
            minutes: 15,
        }]),
        test: std::sync::Mutex::new(Err(AccountError::KeyRefused { detail: "401, invalid key".into() })),
        ..fake::Fake::default()
    });
    shell.services.agents = Some(host.clone());
    shell.services.agent_accounts = Some(host);
    // What the look at start found, as its answer leaves it.
    shell.agents.set_look(look);
    shell.agents.codex = Some(Ok(account));
    if screen == Screen::SettingsAgentsRefused {
        shell.agents.refused = [Some("401, invalid key".into()), None];
    }
    if matches!(screen, Screen::SettingsAgentsMissing | Screen::SettingsAgentsReady | Screen::SettingsAgentsRefused | Screen::SettingsAgentsNoStore) {
        crate::ai_agents::check_connection(shell);
    }
    if screen == Screen::SettingsAgentsDeviceCode {
        crate::ai_agents::sign_in(shell, LoginWay::DeviceCode);
    }
    crate::settings::open_at(shell, crate::settings::Section::Agents);
}

/// A schema "Shop" with a table "Customer" of three columns, the table selected; with `skills`,
/// a naming skill on the schema and one on the table, the Skills tab open. The first refused
/// step is the error, so a sample never renders half-built.
fn sample_table(shell: &mut Shell, skills: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop" }))?;
    let customer = run(session, "er.entity.add", json!({ "schema": schema, "name": "Customer" }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR", "length": 255, "unique": true }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR", "length": 120 }))?;
    let created = run(session, "er.attribute.add", json!({ "entity": customer, "name": "created_at", "type": "TIMESTAMP" }))?;
    run(session, "er.check.add", json!({ "entity": customer, "expression": "email <> ''", "name": "ck_customer_email" }))?;
    run(
        session,
        "er.index.add",
        json!({
            "entity": customer,
            "columns": [{ "expression": "lower(email)" }, { "attribute": created, "direction": "descending" }],
            "name": "ix_customer_email_created",
            "where": "email <> ''",
            "method": "btree",
        }),
    )?;
    if skills {
        run(
            session,
            "skill.add",
            json!({ "subject": schema, "name": "Table naming", "applies_to": "table", "triggers": ["naming"], "body": "Tables are singular nouns in PascalCase." }),
        )?;
        run(
            session,
            "skill.add",
            json!({ "subject": customer, "name": "Customer naming", "triggers": ["naming"], "body": "Columns end in _id for keys." }),
        )?;
    }
    run(session, "view.select", json!({ "ids": [customer] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    if skills {
        shell.state.show_panel(crate::state::Panel::Skills);
    }
    Ok(())
}

/// Writes the table's naming skill out in Markdown and opens it in its tab, in Preview when
/// `preview`.
fn sample_skill_tab(shell: &mut Shell, preview: bool) -> Result<(), String> {
    use serde_json::json;
    const TEXT: &str = "# Customer naming\n\nName every column of **Customer** in `snake_case`, and keep each name under 30 characters.\n\n## Keys\n\n- The primary key is `customer_id`.\n- A foreign key to another table is named `<table>_id`.\n\n## Words\n\n> Prefer plain words over abbreviations: `address`, not `addr`.\n\n1. Singular table names: `Customer`, not `Customers`.\n2. No prefixes such as `tbl_` or `t_`.\n";
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let skill = archforge_core::skills::skills(store)
        .into_iter()
        .find(|s| archforge_core::skills::name_of(store, *s) == "Customer naming")
        .and_then(|s| store.uuid(s).ok())
        .ok_or_else(|| "the sample has no Customer naming skill".to_string())?;
    session.execute("skill.set-body", &json!({ "skill": skill.to_string(), "body": TEXT })).map_err(|e| format!("skill.set-body: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    crate::canvas::open(shell, skill);
    if let Some(doc) = shell.canvas.skills.iter_mut().find(|d| d.skill == skill) {
        doc.preview = preview;
    }
    Ok(())
}

/// Adds a domain and an enum type to the sample table's schema, puts email on the domain and a
/// status column on the enum type, and selects the schema.
fn sample_types(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(customer), Some(email)) = (by_name("Shop"), by_name("Customer"), by_name("email")) else {
        return Err("the sample table".into());
    };
    let domain = run(
        session,
        "er.domain.add",
        json!({ "schema": schema, "name": "email_address", "base_type": "VARCHAR", "length": 255, "not_null": true, "checks": ["position('@' in value) > 1"] }),
    )?;
    let status = run(session, "er.enum.add", json!({ "schema": schema, "name": "customer_status", "literals": ["active", "dormant", "closed"] }))?;
    run(session, "er.attribute.set-type", json!({ "attribute": email, "domain": domain }))?;
    run(session, "er.attribute.add", json!({ "entity": customer, "name": "status", "enum": status, "default": "'active'" }))?;
    run(session, "view.select", json!({ "ids": [schema] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// Adds Tag to the filled diagram, a many-to-many line from Order to it, and resolves the line into
/// the mapping table Order_Tag, which stays selected.
fn sample_mapping_table(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(order), Some(diagram)) = (
        by_name("Shop schema"),
        by_name("Order"),
        store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).and_then(|d| store.uuid(d).ok()).map(|u| u.to_string()),
    ) else {
        return Err("the filled diagram".into());
    };
    let order_node = archforge_diagram::views_of(store, store.by_uuid(archforge_mof::Uuid::parse(&order).ok_or("id")?).ok_or("order")?)
        .2
        .first()
        .and_then(|n| store.uuid(*n).ok())
        .map(|u| u.to_string())
        .ok_or("Order's node")?;
    let tag = run(session, "er.entity.add", json!({ "schema": schema, "name": "Tag" }))?;
    run(session, "er.attribute.add", json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true }))?;
    run(session, "er.attribute.add", json!({ "entity": tag, "name": "label", "type": "VARCHAR", "length": 40 }))?;
    let line = run(session, "er.relationship.add", json!({ "from": order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }))?;
    let t = run(session, "diagram.place", json!({ "diagram": diagram, "element": tag, "x": 920, "y": 360, "width": 240, "height": 120 }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": line, "source": order_node, "target": t }))?;
    run(session, "er.relationship.resolve", json!({ "relationship": line }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// A schema "dbo" with its diagram open in the Diagrams explorer, Company on it, its key column
/// selected.
fn sample_column_selected(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "dbo" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": "er", "element": schema, "name": "dbo" }))?;
    let company = run(session, "er.entity.add", json!({ "schema": schema, "name": "Company" }))?;
    let id = run(session, "er.attribute.add", json!({ "entity": company, "name": "id", "type": "UUID", "primary_key": true }))?;
    run(
        session,
        "diagram.place",
        json!({ "diagram": diagram, "element": company, "x": 160, "y": 180, "width": 240, "height": crate::editing::content_height(1) }),
    )?;
    let d = archforge_mof::Uuid::parse(&diagram).ok_or_else(|| "the diagram's id does not parse".to_string())?;
    crate::canvas::open(shell, d);
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    run(session, "view.select", json!({ "ids": [id] }))?;
    shell.state.show_explorer(crate::state::Explorer::Diagrams);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::Vec2::ZERO;
    Ok(())
}

/// Gives the first table of the open diagram its own fill and border, and the relationship its own
/// width, then opens Appearance on the table, on the relationship, or on the table with the picker.
fn sample_appearance(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let class = |c| store.alive().find(|o| store.class_of(*o).ok() == Some(c));
    let selected = session.view().selection.first().copied().ok_or_else(|| "nothing is selected".to_string())?;
    let node = store
        .alive()
        .find(|o| {
            store.class_of(*o).ok() == Some(archforge_diagram::NODE)
                && store.get(*o, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(selected)
        })
        .and_then(|o| store.uuid(o).ok())
        .ok_or_else(|| "the selected table has no node".to_string())?;
    let rel = class(archforge_er::RELATIONSHIP).and_then(|o| store.uuid(o).ok()).ok_or_else(|| "the sample has no relationship".to_string())?;
    let edge = class(archforge_diagram::EDGE).and_then(|o| store.uuid(o).ok()).ok_or_else(|| "the sample has no line".to_string())?;
    let run = |session: &mut archforge_notations::Session, params: serde_json::Value| {
        session.execute("diagram.style", &params).map(|_| ()).map_err(|e| format!("diagram.style: {e:?}"))
    };
    run(session, json!({ "ids": [node.to_string()], "fill": "#EBF0FE", "stroke": "#9EAFE7", "line": 1.5 }))?;
    run(session, json!({ "ids": [edge.to_string()], "line": 2 }))?;
    if screen == Screen::WorkspaceAppearanceLine {
        session.execute("view.select", &json!({ "ids": [rel.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.state.show_panel(crate::state::Panel::Appearance);
    if screen == Screen::WorkspaceAppearancePicker {
        let c = egui::Color32::from_rgb(0xC2, 0x5E, 0x0A);
        shell.panels.picker = Some(crate::appearance::Picker::new(crate::appearance::Group::Fill, c));
    }
    Ok(())
}

/// A sequence owned by the sample table's key and a view of its schema; the schema selected.
fn sample_objects(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(schema), Some(key)) = (by_name("Shop"), by_name("customer_id")) else {
        return Err("the sample table".into());
    };
    run(session, "er.sequence.add", json!({ "schema": schema, "name": "customer_id_seq", "start": 1000, "minimum": "1000", "owned_by": key }))?;
    run(
        session,
        "er.view.add",
        json!({ "schema": schema, "name": "recent_customers", "query": "SELECT customer_id, email\nFROM \"Customer\"\nWHERE created_at > now() - interval '30 days'" }),
    )?;
    run(session, "view.select", json!({ "ids": [schema] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// A CRM schema whose Account each Order of the shop belongs to, placed on the shop's diagram.
fn sample_cross_schema(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(order), Some(diagram)) = (by_name("Order"), shell.canvas.active.map(|d| d.to_string())) else {
        return Err("the sample diagram".into());
    };
    let order_node = {
        let store = session.doc();
        let o = store.by_uuid(archforge_mof::Uuid::parse(&order).ok_or("the order's id")?).ok_or("the order")?;
        archforge_diagram::views_of(store, o).2.first().and_then(|n| store.uuid(*n).ok()).map(|u| u.to_string()).ok_or("the order's node")?
    };
    let crm = run(session, "er.schema.add", json!({ "name": "CRM" }))?;
    let account = run(session, "er.entity.add", json!({ "schema": crm, "name": "Account" }))?;
    run(session, "er.attribute.add", json!({ "entity": account, "name": "account_id", "type": "BIGINT", "primary_key": true }))?;
    let owns = run(
        session,
        "er.relationship.add",
        json!({ "from": account, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "owns", "create_foreign_key": true }),
    )?;
    let a = run(
        session,
        "diagram.place",
        json!({ "diagram": diagram, "element": account, "x": 560, "y": 680, "width": 240, "height": crate::editing::content_height(1) }),
    )?;
    let edge = run(session, "diagram.connect", json!({ "diagram": diagram, "element": owns, "source": a, "target": order_node }))?;
    run(session, "diagram.label", json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
    run(session, "view.select", json!({ "ids": [order] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    // The three tables framed on the canvas.
    shell.canvas.pan = egui::vec2(-150.0, -280.0);
    Ok(())
}

/// Selects a class of the sample class model and shows its Properties: Order, or Customer with its
/// operation open in the editor.
fn sample_class_members(shell: &mut Shell, editor: bool) -> Result<(), String> {
    let name = if editor { "Customer" } else { "Order" };
    let (class, op) = {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let c = store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == name)
            .ok_or("the sample has no such class")?;
        let op = archforge_uml::members::operations(store, c).first().and_then(|o| store.uuid(*o).ok());
        (store.uuid(c).map_err(|e| e.to_string())?, op)
    };
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [class.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    if editor && let Some(op) = op {
        crate::uml_panel::open(shell, class, crate::uml_panel::MemberKind::Operation, Some(op));
    }
    Ok(())
}

/// In the sample class model, PricingService made final and Payment selected with No subclasses
/// refused on it, as a click on it leaves Properties (AR-137).
fn sample_final(shell: &mut Shell) -> Result<(), String> {
    let id = |shell: &Shell, name: &str| -> Result<archforge_mof::Uuid, String> {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let c = store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == name)
            .ok_or("the sample has no such class")?;
        store.uuid(c).map_err(|e| e.to_string())
    };
    let pricing = id(shell, "PricingService")?;
    let payment = id(shell, "Payment")?;
    crate::panels::run(shell, "uml.classifier.set", serde_json::json!({ "classifier": pricing.to_string(), "final": true }))?;
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [payment.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    shell.panels.editors_for = Some(payment);
    shell.panels.final_refused = Some(payment);
    // At full size, PricingService over Payment and CardPayment, so its `{final}` reads.
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::vec2(260.0 - 1040.0, 60.0 - 80.0);
    Ok(())
}

/// The n-ary association's screens (AR-128): the sample's Recommendations diagram at 100 %, the
/// draft of Customer and Product on it, or recommends drawn and selected with the Catalog end's
/// editor or the question its removal asks. Gives the control the pointer rests on.
fn sample_nary(shell: &mut Shell, screen: Screen) -> Result<Option<String>, String> {
    let drawn = screen != Screen::WorkspaceNaryDrawing;
    let ids = crate::uml_nary::put_recommends(shell, drawn)?;
    shell.state.show_panel(crate::state::Panel::Properties);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::vec2(20.0, 30.0);
    let uuid = |k: &str| ids.get(k).and_then(|v| archforge_mof::Uuid::parse(v)).ok_or_else(|| format!("the sample's {k}"));
    if !drawn {
        shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
        shell.canvas.uml_relation = crate::uml_relate::Relate::NAry;
        let diagram = uuid("diagram")?;
        let ends = vec![
            crate::uml_nary::DraftEnd { class: uuid("Customer")?, node: uuid("customer")? },
            crate::uml_nary::DraftEnd { class: uuid("Product")?, node: uuid("product")? },
        ];
        shell.canvas.nary = Some(crate::uml_nary::Nary::Draft { diagram, ends });
        return Ok(Some(format!("workspace.canvas.node.{}", uuid("catalog")?)));
    }
    let a = uuid("recommends")?;
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [a.to_string()] }))?;
    shell.panels.editors_for = Some(a);
    if screen == Screen::WorkspaceNary {
        crate::uml_nary::open(shell, a, 2);
    } else {
        shell.panels.nary_remove = Some(crate::uml_nary::RemoveAsk { association: a, end: 2, opened: true });
    }
    Ok(None)
}

/// In the sample class model, Priority nested in Order, placed under it with the ⊕ line, selected
/// with its Properties open and its row in Models in sight (AR-132, D2).
fn sample_nested(shell: &mut Shell) -> Result<(), String> {
    let (priority, order) = crate::uml_ui::put_nested_priority(shell)?;
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [priority] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    // At 80 %, Priority's card over Order's, the ⊕ line between them in sight.
    let store = shell.session.as_ref().ok_or("no project is open")?.doc();
    let order = store.by_uuid(archforge_mof::Uuid::parse(&order).ok_or("Order's id")?).ok_or("Order")?;
    let node = archforge_diagram::views_of(store, order).2.into_iter().next().ok_or("Order's card")?;
    let b = archforge_diagram::grid::bounds_of(store, node);
    let z = 0.8_f32;
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = z;
    shell.canvas.pan = egui::vec2(140.0 - b.x as f32 * z, 360.0 - b.y as f32 * z);
    Ok(())
}

/// The profile Persistence of F1 in the sample: its diagram open with Entity selected and the
/// editor on cached; or, `refused`, Commerce selected with Unapply refused for its uses (F6).
fn sample_profiles(shell: &mut Shell, refused: bool) -> Result<(), String> {
    let (commerce, persistence, entity) = crate::uml_profiles::put_profiles(shell, refused)?;
    let id = |s: &str| archforge_mof::Uuid::parse(s).ok_or_else(|| "an id".to_string());
    shell.state.show_panel(crate::state::Panel::Properties);
    shell.state.show_explorer(crate::state::Explorer::Models);
    if refused {
        crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [commerce] }))?;
        shell.explorer.expanded.insert(id(&commerce)?);
        crate::uml_profiles::unapply_profile(shell, id(&commerce)?, id(&persistence)?);
        shell.panels.editors_for = Some(id(&commerce)?);
        return Ok(());
    }
    crate::uml_profiles::open_profile_diagram(shell, id(&persistence)?)?;
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [entity] }))?;
    shell.explorer.expanded.insert(id(&persistence)?);
    let cached = {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let e = store.by_uuid(id(&entity)?).ok_or("Entity")?;
        let c = archforge_uml::profiles::properties(store, e).into_iter().find(|a| archforge_core::name_of(store, *a) == "cached").ok_or("cached")?;
        store.uuid(c).map_err(|e| e.to_string())?
    };
    crate::uml_panel::open(shell, id(&entity)?, crate::uml_panel::MemberKind::Attribute, Some(cached));
    shell.panels.editors_for = Some(id(&entity)?);
    if let Some(s) = shell.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.8;
    shell.canvas.pan = egui::vec2(60.0, 60.0);
    Ok(())
}

/// In the sample class model, BankTransfer and WalletPayment beside CardPayment, the three
/// generalizations to Payment grouped in the set method {complete, disjoint}, the set selected with
/// its Properties open, Payment and its three kinds in sight (AR-129).
fn sample_generalization_sets(shell: &mut Shell) -> Result<(), String> {
    let (gens, _) = crate::uml_ui::put_payment_kinds(shell)?;
    let out = crate::panels::run(
        shell,
        "uml.generalization-set.add",
        serde_json::json!({ "generalizations": gens, "name": "method", "complete": true, "disjoint": true }),
    )?;
    let set = out["id"].as_str().ok_or("the set's id")?.to_string();
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [set] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    if let Some(s) = shell.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.75;
    shell.canvas.pan = egui::vec2(-470.0, -230.0);
    Ok(())
}

/// In the sample class model, Repository made a template of T and Key, selected, with the editor
/// open on Key; its Kind menu open for `WorkspaceTemplateKinds`; for `WorkspaceTemplateInUse`
/// OrderRepository binding it and Key's removal refused (AR-130).
fn sample_template(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    let in_use = screen == Screen::WorkspaceTemplateInUse;
    let (repo, [_, key]) = crate::uml_ui::put_repository(shell, in_use)?;
    shell.state.show_panel(crate::state::Panel::Properties);
    crate::uml_templates::open(shell, repo, Some(key));
    if in_use {
        crate::uml_templates::remove_sample(shell);
    } else if screen == Screen::WorkspaceTemplateKinds {
        shell.panels.menu = Some(crate::panels::Menu::TemplateKind);
    }
    shell.panels.editors_for = Some(repo);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.85;
    shell.canvas.pan = egui::vec2(-280.0, -420.0);
    Ok(())
}

/// In the sample class model, Repository a template bound by OrderRepository (AR-131): the binding
/// selected for `WorkspaceTemplateBinding`; for `WorkspaceTemplateBindingNew` the binding removed
/// again and a new one's draft opened from OrderRepository's card to Repository's, Apply pressed
/// with T still to pick.
fn sample_binding(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    let (repo, _) = crate::uml_ui::put_repository(shell, true)?;
    let d = shell.canvas.active.ok_or("the sample's diagram")?;
    let session = shell.session.as_mut().ok_or("the binding sample needs a session")?;
    let store = session.doc();
    let r = store.by_uuid(repo).ok_or("Repository")?;
    let b = archforge_uml::templates::bindings_to(store, r).first().copied().ok_or("the sample's binding")?;
    let bound = archforge_uml::templates::bound_element(store, b).ok_or("OrderRepository")?;
    let b_id = store.uuid(b).map_err(|e| e.to_string())?.to_string();
    shell.state.show_panel(crate::state::Panel::Properties);
    if screen == Screen::WorkspaceTemplateBinding {
        session.execute("view.select", &serde_json::json!({ "ids": [b_id] })).map_err(|e| e.to_string())?;
    } else {
        session.execute("element.remove", &serde_json::json!({ "ids": [b_id] })).map_err(|e| e.to_string())?;
        session.execute("view.select", &serde_json::json!({ "ids": [] })).map_err(|e| e.to_string())?;
        session.mark_saved().map_err(|e| e.to_string())?;
        let store = session.doc();
        let d_obj = store.by_uuid(d).ok_or("the diagram")?;
        let node_of = |c| {
            archforge_diagram::views_of(store, c)
                .2
                .into_iter()
                .find(|n| archforge_diagram::diagram_of(store, *n) == Some(d_obj))
                .and_then(|n| store.uuid(n).ok())
        };
        let (Some(from), Some(to)) = (node_of(bound), node_of(r)) else { return Err("the sample's nodes".into()) };
        crate::uml_bindings::open_new(shell, d_obj, (from, to), bound, r);
        crate::uml_bindings::apply_sample(shell);
    }
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.75;
    shell.canvas.pan = egui::vec2(-20.0, -400.0);
    Ok(())
}

/// In the sample class model, Order's composition to OrderLine named contains, its OrderLine end
/// `line` at 0..1, its Order end qualified by lineNo : Integer, selected with the qualifier editor
/// open on lineNo; or, `refused`, a second lineNo applied there and refused (AR-127).
fn sample_qualifiers(shell: &mut Shell, refused: bool) -> Result<(), String> {
    use serde_json::json;
    let a_id = {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let named = |o: archforge_mof::Obj, n: &str| archforge_core::name_of(store, o) == n;
        let a = store
            .alive()
            .filter(|o| store.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION))
            .find(|o| archforge_uml::figures::ends(store, *o).is_some_and(|(from, to)| named(from, "Order") && named(to, "OrderLine")))
            .ok_or("the sample has no association from Order to OrderLine")?;
        store.uuid(a).map_err(|e| e.to_string())?
    };
    let id = a_id.to_string();
    crate::panels::run(shell, "uml.relationship.rename", json!({ "relationship": id, "name": "contains" }))?;
    crate::panels::run(shell, "uml.association.set-end", json!({ "association": id, "end": 1, "role": "line", "multiplicity": "0..1" }))?;
    let out = crate::panels::run(shell, "uml.qualifier.add", json!({ "association": id, "end": 0, "name": "lineNo", "type": "Integer" }))?;
    let q = archforge_mof::Uuid::parse(out["id"].as_str().ok_or("the qualifier's id")?).ok_or("the qualifier's id")?;
    crate::panels::run(shell, "view.select", json!({ "ids": [id] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    if let Some(s) = shell.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    if refused {
        crate::uml_qualifiers::refuse_sample(shell, a_id, 0, "lineNo", "String");
    } else {
        crate::uml_qualifiers::open(shell, a_id, 0, Some(q));
    }
    shell.panels.editors_for = Some(a_id);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.85;
    shell.canvas.pan = egui::vec2(-330.0, -20.0);
    Ok(())
}

/// In the sample class model, Order's total() and PricingService made leaf, PricingService final
/// too, CardPayment given an amount that redefines Payment's, and Payment selected with its
/// attribute editor open on amount after No redefinition was refused there, as a click on it
/// leaves the editor (AR-138).
fn sample_leaf(shell: &mut Shell) -> Result<(), String> {
    use crate::uml_panel::{MemberEdit, MemberKind, command_of, edit_of, write_in_sight};
    use archforge_mof::Obj;
    let store = shell.session.as_ref().ok_or("no project is open")?.doc();
    let class = |name: &str| {
        store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(store, *o) == name)
            .ok_or("the sample has no such class")
    };
    let member = |c: Obj, name: &str, operation: bool| {
        let all = if operation { archforge_uml::members::operations(store, c) } else { archforge_uml::members::attributes(store, c) };
        all.into_iter().find(|m| archforge_core::name_of(store, *m) == name).ok_or("the sample has no such member")
    };
    let uuid = |o: Obj| store.uuid(o).map_err(|e| e.to_string());
    let (order, pricing, payment, card) = (class("Order")?, class("PricingService")?, class("Payment")?, class("CardPayment")?);
    let (order_id, pricing_id, payment_id, card_id) = (uuid(order)?, uuid(pricing)?, uuid(payment)?, uuid(card)?);
    let total = uuid(member(order, "total", true)?)?;
    let amount = member(payment, "amount", false)?;
    let amount_id = uuid(amount)?;
    let money = archforge_uml::typed_element::TYPE;
    let money = store.get(amount, money).ok().and_then(|v| v.as_ref()).ok_or("amount has no type")?;
    let money_id = uuid(money)?;
    let total_edit = MemberEdit { is_leaf: true, ..edit_of(Some(store), order_id, MemberKind::Operation, Some(total)) };
    let card_amount = MemberEdit {
        name: "amount".into(),
        ty: money_id.to_string(),
        redefines: vec![amount_id],
        ..edit_of(Some(store), card_id, MemberKind::Attribute, None)
    };
    let amount_edit = edit_of(Some(store), payment_id, MemberKind::Attribute, Some(amount_id));
    for edit in [&total_edit, &card_amount] {
        let (cmd, params) = command_of(edit);
        write_in_sight(shell, edit, cmd, params)?;
    }
    crate::panels::run(shell, "uml.classifier.set", serde_json::json!({ "classifier": pricing_id.to_string(), "final": true, "leaf": true }))?;
    crate::panels::run(shell, "view.select", serde_json::json!({ "ids": [payment_id.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    shell.panels.editors_for = Some(payment_id);
    let refused = shell.session.as_ref().and_then(|s| crate::uml_panel::member_leaf_refusal(s.doc(), &amount_edit, true));
    shell.panels.member = Some(MemberEdit { refused, refused_at: Some("leaf"), opened: false, ..amount_edit });
    // Order, PricingService, Payment and CardPayment in sight, so `{leaf}` reads on Order's total()
    // and `{final, leaf}` after PricingService's name.
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 0.75;
    shell.canvas.pan = egui::vec2(20.0 - 560.0 * 0.75, 40.0 - 80.0 * 0.75);
    Ok(())
}

/// A package, Commerce, with Order, Customer and Payment on its class diagram; Order selected.
fn sample_class_diagram(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Commerce" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Commerce" }))?;
    let mut order = String::new();
    for (name, x, y) in [("Order", 120, 120), ("Customer", 480, 120), ("Payment", 120, 360)] {
        let class = run(session, "uml.class.add", json!({ "package": package, "name": name }))?;
        run(session, "diagram.place", json!({ "diagram": diagram, "element": class, "x": x, "y": y }))?;
        if name == "Order" {
            order = class;
        }
    }
    run(session, "view.select", json!({ "ids": [order] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    // The model's root package, which holds it, open too.
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    Ok(())
}

/// The root package's class diagram with Customer and two packages, Sales and Billing, each with
/// its own diagram; Sales holds Order, OrderLine and a package Payments. Sales is selected, or,
/// `inside`, its diagram is open.
fn sample_packages(shell: &mut Shell, inside: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let root = run(session, "uml.model.root", json!({}))?;
    let top = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": root, "name": "Commerce platform" }))?;
    let customer = run(session, "uml.class.add", json!({ "package": root, "name": "Customer" }))?;
    run(session, "diagram.place", json!({ "diagram": top, "element": customer, "x": 80, "y": 120 }))?;
    let (w, h) = archforge_uml::PACKAGE_SIZE;
    let mut sales = String::new();
    for (name, x) in [("Sales", 420), ("Billing", 760)] {
        let p = run(session, "uml.package.add", json!({ "name": name, "parent": root }))?;
        run(session, "diagram.place", json!({ "diagram": top, "element": p, "x": x, "y": 120, "width": w, "height": h }))?;
        run(session, "uml.package.diagram", json!({ "package": p }))?;
        if name == "Sales" {
            sales = p;
        }
    }
    let inner = run(session, "uml.package.diagram", json!({ "package": sales }))?;
    // Sales's own diagram, which its card shows as a thumbnail (AR-113): two classes with their
    // attributes, the association between them, and its own package Payments with a class.
    let mut classes = Vec::new();
    for (name, x, attributes) in [("Order", 80, ["id", "placedAt"].as_slice()), ("OrderLine", 420, ["quantity"].as_slice())] {
        let c = run(session, "uml.class.add", json!({ "package": sales, "name": name }))?;
        for a in attributes {
            run(session, "uml.attribute.add", json!({ "class": c, "name": a }))?;
        }
        run(session, "diagram.place", json!({ "diagram": inner, "element": c, "x": x, "y": 120 }))?;
        classes.push(c);
    }
    run(session, "uml.association.add", json!({ "from": classes[0], "to": classes[1], "name": "lines" }))?;
    let payments = run(session, "uml.package.add", json!({ "name": "Payments", "parent": sales }))?;
    run(session, "diagram.place", json!({ "diagram": inner, "element": payments, "x": 760, "y": 120, "width": w, "height": h }))?;
    let own = run(session, "uml.package.diagram", json!({ "package": payments }))?;
    let payment = run(session, "uml.class.add", json!({ "package": payments, "name": "Payment" }))?;
    run(session, "diagram.place", json!({ "diagram": own, "element": payment, "x": 80, "y": 80 }))?;
    let _ = session.execute("diagram.connect-all", &json!({ "diagram": inner }));
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let id = |s: &str| archforge_mof::Uuid::parse(s).ok_or_else(|| "the sample's ids".to_string());
    crate::canvas::open(shell, id(if inside { &inner } else { &top })?);
    if !inside {
        let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
        run(session, "view.select", json!({ "ids": [sales] }))?;
    }
    shell.canvas.fit_pending = true;
    Ok(())
}

/// A schema and two packages, Orders inside Sales, each with its diagram; the Orders diagram open
/// in the Diagrams explorer, which groups them by kind.
fn sample_diagrams(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop" }))?;
    run(session, "diagram.add", json!({ "kind": "er", "element": schema, "name": "Shop schema" }))?;
    let sales = run(session, "uml.package.add", json!({ "name": "Sales" }))?;
    let orders = run(session, "uml.package.add", json!({ "name": "Orders", "parent": sales }))?;
    run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": sales, "name": "Sales" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": orders, "name": "Orders" }))?;
    for (name, x, y) in [("Order", 120, 120), ("OrderLine", 480, 120)] {
        let class = run(session, "uml.class.add", json!({ "package": orders, "name": name }))?;
        run(session, "diagram.place", json!({ "diagram": diagram, "element": class, "x": x, "y": y }))?;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let d = archforge_mof::Uuid::parse(&diagram).ok_or_else(|| "the diagram's id does not parse".to_string())?;
    crate::canvas::open(shell, d);
    shell.state.show_explorer(crate::state::Explorer::Diagrams);
    Ok(())
}

/// A package, Billing, on its class diagram: ReviewInvoice new with its first operation, Invoice
/// as tall as its members, Order made shorter by hand than its members need, Payment shorter than
/// its header and a row.
fn sample_class_sizing(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Billing" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Billing" }))?;
    let members: [(&str, &[&str], &[&str], f64); 4] = [
        ("ReviewInvoice", &[], &["calculateTotal"], archforge_uml::CLASS_SIZE.1),
        ("Invoice", &["number", "issuedAt", "total"], &["send", "cancel"], crate::uml_ui::members_height(3, 2)),
        ("Order", &["id", "placedAt"], &["submit", "cancel"], 120.0),
        ("Payment", &["amount"], &["authorise"], 60.0),
    ];
    for (i, (name, attributes, operations, height)) in members.into_iter().enumerate() {
        let class = run(session, "uml.class.add", json!({ "package": package, "name": name }))?;
        for a in attributes {
            run(session, "uml.attribute.add", json!({ "class": class, "name": a }))?;
        }
        for o in operations {
            run(session, "uml.operation.add", json!({ "class": class, "name": o }))?;
        }
        let x = 80.0 + 320.0 * i as f64;
        run(
            session,
            "diagram.place",
            json!({ "diagram": diagram, "element": class, "x": x, "y": 120, "width": archforge_uml::CLASS_SIZE.0, "height": height }),
        )?;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    // The model's root package, which holds it, open too.
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    Ok(())
}

/// A package Payments on its own class diagram: the interface Payable with an attribute and two
/// operations, Refundable specializing it, and the class Invoice with a directed association
/// "settles" to it; Payable selected, its Properties shown.
fn sample_interfaces(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Payments" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Payments" }))?;
    let payable = run(session, "uml.interface.add", json!({ "package": package, "name": "Payable" }))?;
    run(session, "uml.attribute.add", json!({ "class": payable, "name": "amount", "type": "Real" }))?;
    run(
        session,
        "uml.operation.add",
        json!({ "class": payable, "name": "pay", "parameters": [{ "name": "amount", "type": "Real" }], "returns": "Boolean" }),
    )?;
    run(session, "uml.operation.add", json!({ "class": payable, "name": "refund", "returns": "Boolean" }))?;
    let refundable = run(session, "uml.interface.add", json!({ "package": package, "name": "Refundable" }))?;
    run(
        session,
        "uml.operation.add",
        json!({ "class": refundable, "name": "refundTo", "parameters": [{ "name": "account", "type": "String" }], "returns": "Boolean" }),
    )?;
    let invoice = run(session, "uml.class.add", json!({ "package": package, "name": "Invoice" }))?;
    run(session, "uml.attribute.add", json!({ "class": invoice, "name": "number", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": invoice, "name": "total", "type": "Real" }))?;
    run(session, "uml.generalization.add", json!({ "specific": refundable, "general": payable }))?;
    let settles = run(session, "uml.association.add", json!({ "from": invoice, "to": payable, "kind": "directed", "name": "settles" }))?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    for (element, x, y, attributes, operations) in [(&invoice, 80, 140, 2, 0), (&payable, 520, 140, 1, 2), (&refundable, 520, 440, 0, 1)] {
        let height = crate::uml_ui::members_height(attributes, operations);
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w + 80.0, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    let edge = {
        let store = session.doc();
        let line = archforge_mof::Uuid::parse(&settles).and_then(|u| store.by_uuid(u)).ok_or("the association")?;
        archforge_diagram::views_of(store, line).1.first().and_then(|e| store.uuid(*e).ok()).ok_or("the association's line")?
    };
    run(session, "diagram.label", json!({ "edge": edge.to_string(), "kind": "name", "at": "middle" }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [payable] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// A package Orders on its own class diagram: the enumeration OrderStatus with the literals Placed,
/// Paid, Shipped and Cancelled, the enumeration Priority with none yet, and the class Order whose
/// attributes they type; OrderStatus selected, open in Models, its Properties shown.
fn sample_enumerations(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Orders" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Orders" }))?;
    let status = run(session, "uml.enumeration.add", json!({ "package": package, "name": "OrderStatus" }))?;
    let priority = run(session, "uml.enumeration.add", json!({ "package": package, "name": "Priority" }))?;
    let order = run(session, "uml.class.add", json!({ "package": package, "name": "Order" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "number", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "priority", "type": priority, "multiplicity": "0..1" }))?;
    run(session, "uml.operation.add", json!({ "class": order, "name": "advance", "parameters": [{ "name": "to", "type": status }] }))?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    let cards = [
        (&order, 80, 140, crate::uml_ui::members_height(3, 1)),
        (&status, 560, 140, crate::uml_ui::literals_height(4)),
        (&priority, 560, 400, crate::uml_ui::literals_height(0)),
    ];
    for (element, x, y, height) in cards {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w + 40.0, "height": height }))?;
    }
    let (Some(package), Some(diagram), Some(status)) =
        (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram), archforge_mof::Uuid::parse(&status))
    else {
        return Err("the sample's ids".into());
    };
    crate::uml_ui::put_literals(shell, status, &["Placed", "Paid", "Shipped", "Cancelled"])?;
    shell.explorer.expanded.extend([package, status]);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [status.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// A package Payments on its own class diagram: the data type Money with the attributes amount and
/// currency and the operation add, Price specializing it with a tax rate, and the class Order whose
/// total and refund it types; Money selected, open in Models, its Properties shown.
fn sample_data_types(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Payments" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Payments" }))?;
    let money = run(session, "uml.data-type.add", json!({ "package": package, "name": "Money" }))?;
    run(session, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real" }))?;
    run(session, "uml.attribute.add", json!({ "class": money, "name": "currency", "type": "String" }))?;
    run(
        session,
        "uml.operation.add",
        json!({ "class": money, "name": "add", "parameters": [{ "name": "other", "type": money }], "returns": money }),
    )?;
    let price = run(session, "uml.data-type.add", json!({ "package": package, "name": "Price" }))?;
    run(session, "uml.attribute.add", json!({ "class": price, "name": "taxRate", "type": "Real" }))?;
    run(session, "uml.generalization.add", json!({ "specific": price, "general": money }))?;
    let order = run(session, "uml.class.add", json!({ "package": package, "name": "Order" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "number", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "total", "type": money }))?;
    run(session, "uml.operation.add", json!({ "class": order, "name": "refund", "parameters": [{ "name": "amount", "type": money }] }))?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    let cards = [
        (&order, 80, 140, crate::uml_ui::members_height(2, 1)),
        (&money, 560, 140, crate::uml_ui::members_height(2, 1)),
        (&price, 560, 400, crate::uml_ui::members_height(1, 0)),
    ];
    for (element, x, y, height) in cards {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w + 40.0, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    run(session, "view.select", json!({ "ids": [] }))?;
    session.mark_saved().map_err(|e| e.to_string())?;
    let (Some(package), Some(diagram), Some(money)) =
        (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram), archforge_mof::Uuid::parse(&money))
    else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.extend([package, money]);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [money.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// A package Catalog on its own class diagram: the primitive types Sku and Gtin, Gtin specializing
/// Sku, and the class Product whose sku, gtin and price they and a built-in type type, and whose
/// operation find takes a Sku; Sku described, selected, the package open in Models, its Properties
/// shown.
fn sample_primitive_types(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Catalog" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Catalog" }))?;
    let sku = run(session, "uml.primitive-type.add", json!({ "package": package, "name": "Sku" }))?;
    let gtin = run(session, "uml.primitive-type.add", json!({ "package": package, "name": "Gtin" }))?;
    run(session, "uml.generalization.add", json!({ "specific": gtin, "general": sku }))?;
    let product = run(session, "uml.class.add", json!({ "package": package, "name": "Product" }))?;
    run(session, "uml.attribute.add", json!({ "class": product, "name": "sku", "type": sku }))?;
    run(session, "uml.attribute.add", json!({ "class": product, "name": "gtin", "type": gtin }))?;
    run(session, "uml.attribute.add", json!({ "class": product, "name": "price", "type": "Real" }))?;
    run(
        session,
        "uml.operation.add",
        json!({ "class": product, "name": "find", "parameters": [{ "name": "code", "type": sku }], "returns": product }),
    )?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    let (_, h) = archforge_uml::PRIMITIVE_TYPE_SIZE;
    let cards = [(&product, 80, 140, crate::uml_ui::members_height(3, 1)), (&sku, 560, 140, h), (&gtin, 560, 340, h)];
    for (element, x, y, height) in cards {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w + 40.0, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    // Sku's description, kept as a comment it owns (AR-186), shows under its name in Properties.
    run(session, "element.describe", json!({ "element": sku, "text": "The shop's own product code." }))?;
    run(session, "view.select", json!({ "ids": [] }))?;
    session.mark_saved().map_err(|e| e.to_string())?;
    let (Some(package), Some(diagram), Some(sku)) =
        (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram), archforge_mof::Uuid::parse(&sku))
    else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [sku.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// OrderStatus's card at a working zoom near the canvas's top left, with a literal's draft row
/// open at its end and "Returned" typed in it.
fn literal_draft(shell: &mut Shell) -> Result<(), String> {
    let (status, bounds) = {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let e = store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_uml::ENUMERATION) && archforge_core::name_of(store, *o) == "OrderStatus")
            .ok_or("the sample has no OrderStatus")?;
        let n = archforge_diagram::views_of(store, e).2.into_iter().next().ok_or("OrderStatus is on no diagram")?;
        (store.uuid(e).map_err(|e| e.to_string())?, archforge_diagram::grid::bounds_of(store, n))
    };
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.25;
    shell.canvas.pan = egui::vec2(160.0 - bounds.x as f32 * 1.25, 120.0 - bounds.y as f32 * 1.25);
    if !crate::uml_inline::open(shell, status, crate::uml_panel::MemberKind::Literal, None) {
        return Err("OrderStatus's card cannot hold a row".into());
    }
    if let Some(inline) = shell.canvas.inline.as_mut() {
        inline.edit.name = "Returned".into();
    }
    Ok(())
}

/// A package Payments on its diagram: the interface Payable with two operations, realized by the
/// classes Order, beside it, and Invoice, below it; Order's realization selected, Properties
/// shown.
fn sample_realizations(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Payments" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Payments" }))?;
    let payable = run(session, "uml.interface.add", json!({ "package": package, "name": "Payable" }))?;
    run(
        session,
        "uml.operation.add",
        json!({ "class": payable, "name": "pay", "parameters": [{ "name": "amount", "type": "Real" }], "returns": "Boolean" }),
    )?;
    run(session, "uml.operation.add", json!({ "class": payable, "name": "refund", "returns": "Boolean" }))?;
    let order = run(session, "uml.class.add", json!({ "package": package, "name": "Order" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "placedAt", "type": "String" }))?;
    run(session, "uml.operation.add", json!({ "class": order, "name": "total", "returns": "Real" }))?;
    let invoice = run(session, "uml.class.add", json!({ "package": package, "name": "Invoice" }))?;
    run(session, "uml.attribute.add", json!({ "class": invoice, "name": "number", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": invoice, "name": "total", "type": "Real" }))?;
    let realized = run(session, "uml.realization.add", json!({ "class": order, "interface": payable }))?;
    run(session, "uml.realization.add", json!({ "class": invoice, "interface": payable }))?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    // Order and Payable side by side, the same height, so Order's line runs straight; Invoice under
    // Payable, its line straight up.
    let top = crate::uml_ui::members_height(2, 2);
    for (element, x, y, height) in [(&order, 80, 140, top), (&payable, 560, 140, top), (&invoice, 560, 460, crate::uml_ui::members_height(2, 0))] {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w + 40.0, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [realized] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// A package Reviews: the classes Customer and Product side by side, the association class Review
/// between them with its attributes stars and text and its operation publish, its card above its
/// line's middle; Review selected, its Properties shown and the package open in Models.
fn sample_association_class(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Reviews" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Reviews" }))?;
    let customer = run(session, "uml.class.add", json!({ "package": package, "name": "Customer" }))?;
    run(session, "uml.attribute.add", json!({ "class": customer, "name": "name", "type": "String" }))?;
    let product = run(session, "uml.class.add", json!({ "package": package, "name": "Product" }))?;
    run(session, "uml.attribute.add", json!({ "class": product, "name": "sku", "type": "String" }))?;
    let review = run(session, "uml.association-class.add", json!({ "from": customer, "to": product, "name": "Review" }))?;
    run(session, "uml.attribute.add", json!({ "class": review, "name": "stars", "type": "Integer" }))?;
    run(session, "uml.attribute.add", json!({ "class": review, "name": "text", "type": "String" }))?;
    run(session, "uml.operation.add", json!({ "class": review, "name": "publish" }))?;
    run(session, "uml.association.set-end", json!({ "association": review, "end": 0, "multiplicity": "*", "navigable": false }))?;
    run(session, "uml.association.set-end", json!({ "association": review, "end": 1, "multiplicity": "*", "navigable": false }))?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    let low = crate::uml_ui::members_height(1, 0);
    for (element, x, y, height) in [(&customer, 80, 420, low), (&product, 640, 420, low), (&review, 360, 120, crate::uml_ui::members_height(2, 1))] {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": w, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram)) = (archforge_mof::Uuid::parse(&package), archforge_mof::Uuid::parse(&diagram)) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [review] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    Ok(())
}

/// A package Orders: the class Order with `id : Integer {readOnly}`, a derived `total : Money = 0`,
/// `status : OrderStatus = Placed` and `lines : OrderLine [1..*] {ordered}`, and the operations
/// `create() : Order`, static and so underlined (AR-136), and `place()`; the data type Money,
/// the enumeration OrderStatus and the class OrderLine, Order associated with it, its end at
/// OrderLine `/items 0..* = none {ordered}`. Order selected, the attribute editor open on total in
/// Properties, the package open in Models (AR-126).
fn sample_modifiers(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let package = run(session, "uml.package.add", json!({ "name": "Orders" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": package, "name": "Orders" }))?;
    let money = run(session, "uml.data-type.add", json!({ "package": package, "name": "Money" }))?;
    let status = run(session, "uml.enumeration.add", json!({ "package": package, "name": "OrderStatus" }))?;
    for l in ["Placed", "Paid", "Shipped"] {
        run(session, "uml.literal.add", json!({ "enumeration": status, "name": l }))?;
    }
    let line = run(session, "uml.class.add", json!({ "package": package, "name": "OrderLine" }))?;
    run(session, "uml.attribute.add", json!({ "class": line, "name": "quantity", "type": "Integer", "default": "1" }))?;
    let order = run(session, "uml.class.add", json!({ "package": package, "name": "Order" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer", "read_only": true }))?;
    let total = run(session, "uml.attribute.add", json!({ "class": order, "name": "total", "type": money, "default": "0", "derived": true }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status, "default": "Placed" }))?;
    run(session, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": line, "multiplicity": "1..*", "ordered": true }))?;
    // A static operation, underlined on the card (AR-136).
    run(session, "uml.operation.add", json!({ "class": order, "name": "create", "returns": order, "static": true }))?;
    run(session, "uml.operation.add", json!({ "class": order, "name": "place" }))?;
    let holds = run(session, "uml.association.add", json!({ "from": order, "to": line, "kind": "directed" }))?;
    run(
        session,
        "uml.association.set-end",
        json!({ "association": holds, "end": 1, "role": "items", "multiplicity": "0..*", "derived": true, "default": "none", "ordered": true }),
    )?;
    let (w, _) = archforge_uml::CLASS_SIZE;
    let order_height = crate::uml_ui::members_height(4, 2);
    let line_height = crate::uml_ui::members_height(1, 0);
    // OrderLine's middle level with Order's, so the line between them runs straight.
    let line_y = (120.0 + (order_height - line_height) / 2.0).round();
    for (element, x, y, width, height) in [
        (&order, 80.0, 120.0, w + 80.0, order_height),
        (&line, 700.0, line_y, w, line_height),
        (&money, 80.0, 470.0, w, crate::uml_ui::members_height(0, 0)),
        (&status, 420.0, 470.0, w, crate::uml_ui::members_height(3, 0)),
    ] {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": width, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let (Some(package), Some(diagram), Some(order), Some(total)) = (
        archforge_mof::Uuid::parse(&package),
        archforge_mof::Uuid::parse(&diagram),
        archforge_mof::Uuid::parse(&order),
        archforge_mof::Uuid::parse(&total),
    ) else {
        return Err("the sample's ids".into());
    };
    shell.explorer.expanded.insert(package);
    if let Some(root) = shell.session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|r| s.doc().uuid(r).ok())) {
        shell.explorer.expanded.insert(root);
    }
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": [order.to_string()] }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    crate::uml_panel::open(shell, order, crate::uml_panel::MemberKind::Attribute, Some(total));
    Ok(())
}

/// How far the user manual's class-model guide has come in `screen`, from 1 (the first classes)
/// to 12 (the finished model); none for a screen that is not the guide's.
fn guide_step(screen: Screen) -> Option<u8> {
    Some(match screen {
        Screen::GuideClasses => 1,
        Screen::GuideRelateKinds => 2,
        Screen::GuideAssociation => 3,
        Screen::GuideClassifierKinds => 4,
        Screen::GuideLiterals => 5,
        Screen::GuideDataType => 6,
        Screen::GuidePrimitiveType => 7,
        Screen::GuideRealization => 8,
        Screen::GuideAssociationClass => 9,
        Screen::GuideModifiers => 10,
        Screen::GuideConstraints => 11,
        Screen::GuideResult => 12,
        _ => return None,
    })
}

/// The user manual's class-model guide as it stands after `step` (see [`guide_step`]): one
/// diagram, Commerce, of the root package, built up as the chapter builds it, with the screen's
/// selection, open menu or draft row, and the view on the cards the step is about. Nothing is
/// saved yet, so the status bar shows unsaved changes, as the reader's own window does.
fn sample_guide(shell: &mut Shell, step: u8) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let root = run(session, "uml.model.root", json!({}))?;
    let diagram = run(session, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": root, "name": "Commerce" }))?;
    let customer = run(session, "uml.class.add", json!({ "package": root, "name": "Customer" }))?;
    run(session, "uml.attribute.add", json!({ "class": customer, "name": "name", "type": "String" }))?;
    let order = run(session, "uml.class.add", json!({ "package": root, "name": "Order" }))?;
    // What each step selects, and every card with its place, width and height.
    let mut selected = None;
    let mut cards: Vec<(String, f64, f64, f64, f64)> = Vec::new();
    let (w, _) = archforge_uml::CLASS_SIZE;
    let mut order_rows = (0, 0);
    let mut customer_rows = 1;
    let mut status = String::new();
    let mut status_literals = 0;
    let mut places = String::new();
    // The cards the step is about, besides Order.
    let mut about = vec![customer.clone()];
    if step >= 2 {
        run(session, "uml.attribute.add", json!({ "class": order, "name": "number", "type": "Integer", "read_only": step >= 10 }))?;
        run(session, "uml.operation.add", json!({ "class": order, "name": "pay", "returns": "Boolean" }))?;
        order_rows = (1, 1);
    }
    if step >= 3 {
        places = run(session, "uml.association.add", json!({ "from": customer, "to": order, "name": "places" }))?;
        run(session, "uml.association.set-end", json!({ "association": places, "end": 0, "multiplicity": "1" }))?;
        run(
            session,
            "uml.association.set-end",
            json!({ "association": places, "end": 1, "role": "orders", "multiplicity": "*", "navigable": true }),
        )?;
        if step == 3 {
            selected = Some(places.clone());
        }
    }
    if step >= 5 {
        status = run(session, "uml.enumeration.add", json!({ "package": root, "name": "OrderStatus" }))?;
        let literals: &[&str] = if step == 5 { &["Placed", "Paid", "Shipped"] } else { &["Placed", "Paid", "Shipped", "Cancelled"] };
        for l in literals {
            run(session, "uml.literal.add", json!({ "enumeration": status, "name": l }))?;
        }
        status_literals = literals.len();
    }
    if step >= 6 {
        let default = if step >= 10 { json!("Placed") } else { serde_json::Value::Null };
        let mut params = json!({ "class": order, "name": "status", "type": status });
        if !default.is_null()
            && let Some(p) = params.as_object_mut()
        {
            p.insert("default".into(), default);
        }
        run(session, "uml.attribute.add", params)?;
        let money = run(session, "uml.data-type.add", json!({ "package": root, "name": "Money" }))?;
        run(session, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real" }))?;
        run(session, "uml.attribute.add", json!({ "class": money, "name": "currency", "type": "String" }))?;
        run(session, "uml.attribute.add", json!({ "class": order, "name": "total", "type": money }))?;
        order_rows = (3, 1);
        cards.push((money.clone(), 720.0, 500.0, w, crate::uml_ui::members_height(2, 0)));
        if step == 6 {
            selected = Some(money.clone());
            about = vec![money.clone()];
        }
    }
    if step >= 7 {
        let email = run(session, "uml.primitive-type.add", json!({ "package": root, "name": "Email" }))?;
        run(session, "uml.attribute.add", json!({ "class": customer, "name": "email", "type": email }))?;
        customer_rows = 2;
        cards.push((email.clone(), 0.0, 500.0, w, archforge_uml::PRIMITIVE_TYPE_SIZE.1));
        if step == 7 {
            selected = Some(email.clone());
            about = vec![customer.clone(), email];
        }
    }
    if step >= 8 {
        let payable = run(session, "uml.interface.add", json!({ "package": root, "name": "Payable" }))?;
        run(session, "uml.operation.add", json!({ "class": payable, "name": "pay", "returns": "Boolean" }))?;
        let realized = run(session, "uml.realization.add", json!({ "class": order, "interface": payable }))?;
        cards.push((payable.clone(), 510.0, -20.0, w, crate::uml_ui::members_height(0, 1)));
        if step == 8 {
            selected = Some(realized);
            about = vec![payable];
        }
    }
    if step >= 9 {
        let product = run(session, "uml.class.add", json!({ "package": root, "name": "Product" }))?;
        run(session, "uml.attribute.add", json!({ "class": product, "name": "name", "type": "String" }))?;
        let line = run(session, "uml.association-class.add", json!({ "from": order, "to": product, "name": "OrderLine" }))?;
        run(session, "uml.attribute.add", json!({ "class": line, "name": "quantity", "type": "Integer" }))?;
        for end in [0, 1] {
            run(session, "uml.association.set-end", json!({ "association": line, "end": end, "multiplicity": "*" }))?;
        }
        // Product's middle level with Order's, so the line between them runs straight.
        let (order_height, product_height) = (crate::uml_ui::members_height(3, 1), crate::uml_ui::members_height(1, 0));
        cards.push((product.clone(), 1040.0, (220.0 + (order_height - product_height) / 2.0).round(), w, product_height));
        cards.push((line.clone(), 790.0, -20.0, w, crate::uml_ui::members_height(1, 0)));
        if step == 9 {
            selected = Some(line.clone());
            about = vec![product, line];
        }
    }
    if step >= 5 {
        // The draft row of step 5 needs a slot below the literals there are.
        let slots = if step == 5 { status_literals + 1 } else { status_literals };
        cards.push((status.clone(), 420.0, 500.0, w, crate::uml_ui::literals_height(slots)));
        if step == 5 {
            selected = Some(status.clone());
            about = vec![status.clone()];
        }
    }
    // Step 1's draft row needs a slot on Order's card.
    let order_height = crate::uml_ui::members_height(order_rows.0.max(1), order_rows.1);
    // Customer's middle level with Order's, so the association between them runs straight.
    let customer_height = crate::uml_ui::members_height(customer_rows, 0);
    cards.push((customer.clone(), 0.0, (220.0 + (order_height - customer_height) / 2.0).round(), w, customer_height));
    cards.push((order.clone(), 480.0, 220.0, w + 60.0, order_height));
    for (element, x, y, width, height) in &cards {
        run(session, "diagram.place", json!({ "diagram": diagram, "element": element, "x": x, "y": y, "width": width, "height": height }))?;
    }
    run(session, "diagram.connect-all", json!({ "diagram": diagram }))?;
    // The association's name at its line's middle, where drawing it puts it.
    if let Some(p) = archforge_mof::Uuid::parse(&places) {
        let store = session.doc();
        let edge = store.by_uuid(p).and_then(|l| archforge_diagram::views_of(store, l).1.first().copied()).and_then(|e| store.uuid(e).ok());
        if let Some(edge) = edge {
            run(session, "diagram.label", json!({ "edge": edge.to_string(), "kind": "name", "at": "middle" }))?;
        }
    }
    // Order's rules (AR-133): a short one in braces under its card, a long one naming OrderStatus
    // too as a note linked to both.
    let mut paid = String::new();
    if step >= 11 {
        let positive =
            run(session, "uml.constraint.add", json!({ "context": order, "name": "positiveNumber", "expression": "number > 0", "language": "OCL" }))?;
        run(session, "uml.constraint.show", json!({ "diagram": diagram, "constraint": positive, "as": "braces" }))?;
        paid = run(
            session,
            "uml.constraint.add",
            json!({
                "context": order,
                "name": "paidInFull",
                "expression": "status = OrderStatus::Paid\n  implies total.amount > 0",
                "language": "OCL",
                "constrained": [order, status],
            }),
        )?;
        run(session, "uml.constraint.show", json!({ "diagram": diagram, "constraint": paid, "as": "note", "x": 1000, "y": 480 }))?;
    }
    if matches!(step, 1 | 10 | 11) {
        selected = Some(order.clone());
    }
    if step == 10 {
        about.clear();
    }
    if step == 11 {
        about = vec![status.clone(), paid.clone()];
    }
    run(session, "view.select", json!({ "ids": [] }))?;
    // Left unsaved, as the reader's project is while they work: the status bar says so.
    let id = |s: &str| archforge_mof::Uuid::parse(s).ok_or_else(|| "the sample's ids".to_string());
    let (root, diagram, order) = (id(&root)?, id(&diagram)?, id(&order)?);
    let mut view = vec![order];
    for e in &about {
        view.push(id(e)?);
    }
    shell.explorer.expanded.insert(root);
    crate::canvas::open(shell, diagram);
    crate::panels::run(shell, "view.select", json!({ "ids": selected.into_iter().collect::<Vec<_>>() }))?;
    shell.state.show_panel(crate::state::Panel::Properties);
    // The view: Order and the cards the step is about, at a zoom that reads; the finished model
    // whole.
    let zoom = match step {
        1..=4 | 7 => 0.8,
        5 => 0.9,
        6 | 8 | 9 | 11 => 0.8,
        _ => 1.1,
    };
    match step {
        1 => {
            if !crate::uml_inline::open(shell, order, crate::uml_panel::MemberKind::Attribute, None) {
                return Err("Order's card cannot hold a row".into());
            }
            if let Some(inline) = shell.canvas.inline.as_mut() {
                inline.edit.name = "number".into();
                inline.ty = "Integer".into();
            }
        }
        2 => {
            shell.canvas.tool = crate::canvas::Tool::Connect("relationship");
            shell.canvas.uml_relation = crate::uml_relate::Relate::Association;
            shell.canvas.popover = Some("relationship-kinds");
        }
        4 => {
            shell.canvas.tool = crate::canvas::Tool::Place(crate::uml_classifier::TOOL);
            shell.canvas.uml_classifier = crate::uml_classifier::Classifier::Class;
            shell.canvas.popover = Some(crate::uml_classifier::KINDS);
            shell.canvas.menu_focus = Some(2);
        }
        5 => {
            if !crate::uml_inline::open(shell, id(&status)?, crate::uml_panel::MemberKind::Literal, None) {
                return Err("OrderStatus's card cannot hold a row".into());
            }
            if let Some(inline) = shell.canvas.inline.as_mut() {
                inline.edit.name = "Cancelled".into();
            }
        }
        10 => {
            let status_attribute = {
                let store = shell.session.as_ref().ok_or("no project is open")?.doc();
                let o = store.by_uuid(order).ok_or("Order is gone")?;
                archforge_uml::members::attributes(store, o)
                    .into_iter()
                    .find(|a| archforge_core::name_of(store, *a) == "status")
                    .and_then(|a| store.uuid(a).ok())
                    .ok_or("Order has no status")?
            };
            crate::uml_panel::open(shell, order, crate::uml_panel::MemberKind::Attribute, Some(status_attribute));
        }
        11 => crate::uml_constraints::open(shell, order, Some(id(&paid)?)),
        12 => shell.state.panel_collapsed = true,
        _ => {}
    }
    if step == 12 {
        shell.canvas.fit_pending = true;
    } else {
        guide_view(shell, &view, zoom)?;
    }
    Ok(())
}

/// The user manual's further class-model sections (AR-141): the guide's finished model (step 12)
/// with Properties open again, the screen's feature added as its section adds it, its selection or
/// editor, and the view on what the section is about.
fn sample_guide_more(shell: &mut Shell, screen: Screen) -> Result<(), String> {
    use archforge_mof::Uuid;
    use serde_json::{Value, json};
    sample_guide(shell, 12)?;
    shell.state.panel_collapsed = false;
    shell.state.show_panel(crate::state::Panel::Properties);
    let run = |shell: &mut Shell, command: &str, params: Value| crate::panels::run(shell, command, params).map_err(|e| format!("{command}: {e}"));
    let id = |v: &Value| v.get("id").and_then(Value::as_str).and_then(Uuid::parse).ok_or_else(|| "a command gave no id".to_string());
    let find = |shell: &Shell, class: Option<archforge_mof::ClassId>, name: &str| -> Result<Uuid, String> {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let o = store
            .alive()
            .filter(|o| store.class_of(*o).is_ok_and(|c| class.map_or(archforge_uml::is_classifier(c), |k| k == c)))
            .find(|o| archforge_core::name_of(store, *o) == name)
            .ok_or_else(|| format!("the guide has no {name}"))?;
        store.uuid(o).map_err(|e| e.to_string())
    };
    // An element's card on the open diagram.
    let node = |shell: &Shell, element: Uuid| -> Result<Uuid, String> {
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let d = shell.canvas.active.and_then(|d| store.by_uuid(d)).ok_or("no diagram is open")?;
        let o = store.by_uuid(element).ok_or("an element of the guide is gone")?;
        let n = archforge_diagram::views_of(store, o)
            .2
            .into_iter()
            .find(|n| archforge_diagram::diagram_of(store, *n) == Some(d))
            .ok_or("an element of the guide is not on the open diagram")?;
        store.uuid(n).map_err(|e| e.to_string())
    };
    let root = id(&run(shell, "uml.model.root", json!({}))?)?;
    let (order, customer) = (find(shell, None, "Order")?, find(shell, None, "Customer")?);
    let w = archforge_uml::CLASS_SIZE.0;
    // A class diagram of the root package, opened.
    let new_diagram = |shell: &mut Shell, name: &str| -> Result<Uuid, String> {
        let d = id(&run(shell, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": root.to_string(), "name": name }))?)?;
        crate::canvas::open(shell, d);
        Ok(d)
    };
    let place = |shell: &mut Shell, d: Uuid, element: Uuid, x: f64, y: f64, width: f64, height: f64| {
        run(
            shell,
            "diagram.place",
            json!({ "diagram": d.to_string(), "element": element.to_string(), "x": x, "y": y, "width": width, "height": height }),
        )
        .and_then(|v| id(&v))
    };
    let select = |shell: &mut Shell, ids: &[Uuid]| run(shell, "view.select", json!({ "ids": ids.iter().map(Uuid::to_string).collect::<Vec<_>>() }));
    // Order's rule's braces under its card again, where showing them put them, once the card has
    // grown.
    let braces_under_order = |shell: &mut Shell| -> Result<(), String> {
        let n = node(shell, order)?;
        let (d, rule) = (shell.canvas.active.ok_or("no diagram is open")?, find(shell, Some(archforge_uml::CONSTRAINT), "positiveNumber")?);
        let store = shell.session.as_ref().ok_or("no project is open")?.doc();
        let at = archforge_diagram::grid::bounds_of(store, store.by_uuid(n).ok_or("Order's card")?);
        run(
            shell,
            "uml.constraint.show",
            json!({ "diagram": d.to_string(), "constraint": rule.to_string(), "as": "braces", "x": at.x, "y": at.y + at.height + 4.0 }),
        )?;
        Ok(())
    };
    // Order's card made taller for `rows` (attributes, operations).
    let grow_order = |shell: &mut Shell, rows: (usize, usize)| -> Result<(), String> {
        let n = node(shell, order)?;
        let height = crate::uml_ui::members_height(rows.0, rows.1);
        run(shell, "diagram.resize", json!({ "node": n.to_string(), "width": w + 60.0, "height": height }))?;
        braces_under_order(shell)
    };
    match screen {
        Screen::GuideStatic => {
            let op = id(&run(
                shell,
                "uml.operation.add",
                json!({ "class": order.to_string(), "name": "nextNumber", "returns": "Integer", "static": true }),
            )?)?;
            grow_order(shell, (3, 2))?;
            select(shell, &[order])?;
            crate::uml_panel::open(shell, order, crate::uml_panel::MemberKind::Operation, Some(op));
            guide_view(shell, &[order], 1.0)?;
        }
        Screen::GuideQualifier => {
            let places = find(shell, Some(archforge_uml::ASSOCIATION), "places")?;
            let q =
                id(&run(shell, "uml.qualifier.add", json!({ "association": places.to_string(), "end": 0, "name": "number", "type": "Integer" }))?)?;
            run(shell, "uml.association.set-end", json!({ "association": places.to_string(), "end": 1, "multiplicity": "0..1" }))?;
            // Customer a little further left, so the box leaves room for the line's labels.
            let n = node(shell, customer)?;
            run(shell, "diagram.move", json!({ "nodes": [n.to_string()], "dx": -100.0, "dy": 0.0 }))?;
            select(shell, &[places])?;
            crate::uml_qualifiers::open(shell, places, 0, Some(q));
            guide_view(shell, &[customer, order], 0.75)?;
        }
        Screen::GuideNary => {
            let catalog = id(&run(shell, "uml.class.add", json!({ "package": root.to_string(), "name": "Catalog" }))?)?;
            run(shell, "uml.attribute.add", json!({ "class": catalog.to_string(), "name": "season", "type": "String" }))?;
            sample_nary(shell, Screen::WorkspaceNary)?;
            let product = find(shell, None, "Product")?;
            guide_view(shell, &[customer, product, catalog], 0.85)?;
        }
        Screen::GuideGeneralizationSet | Screen::GuideFinalLeaf => {
            let d = new_diagram(shell, "Payments")?;
            let money = find(shell, None, "Money")?;
            let payment = id(&run(shell, "uml.class.add", json!({ "package": root.to_string(), "name": "Payment" }))?)?;
            let amount = id(&run(shell, "uml.attribute.add", json!({ "class": payment.to_string(), "name": "amount", "type": money.to_string() }))?)?;
            place(shell, d, payment, 280.0, 0.0, w, crate::uml_ui::members_height(1, 0))?;
            let mut generalizations = Vec::new();
            let mut card = None;
            let mut cards = vec![payment];
            for (name, x) in [("CardPayment", 0.0), ("BankTransfer", 280.0), ("WalletPayment", 560.0)] {
                let c = id(&run(shell, "uml.class.add", json!({ "package": root.to_string(), "name": name }))?)?;
                place(shell, d, c, x, 340.0, w, crate::uml_ui::members_height(0, 0))?;
                cards.push(c);
                let g = run(shell, "uml.generalization.add", json!({ "specific": c.to_string(), "general": payment.to_string() }))?;
                generalizations.push(id(&g)?.to_string());
                card.get_or_insert(c);
            }
            run(shell, "diagram.connect-all", json!({ "diagram": d.to_string() }))?;
            let set = id(&run(
                shell,
                "uml.generalization-set.add",
                json!({ "generalizations": generalizations, "name": "method", "complete": true, "disjoint": true }),
            )?)?;
            if screen == Screen::GuideGeneralizationSet {
                select(shell, &[set])?;
            } else {
                let card = card.ok_or("the guide has no CardPayment")?;
                run(shell, "uml.classifier.set", json!({ "classifier": card.to_string(), "final": true }))?;
                run(shell, "uml.attribute.set", json!({ "attribute": amount.to_string(), "leaf": true }))?;
                select(shell, &[payment])?;
                crate::uml_panel::open(shell, payment, crate::uml_panel::MemberKind::Attribute, Some(amount));
            }
            guide_view(shell, &cards, 0.85)?;
        }
        Screen::GuideNested => {
            let status = find(shell, None, "OrderStatus")?;
            run(shell, "uml.classifier.set-owner", json!({ "classifier": status.to_string(), "owner": order.to_string() }))?;
            select(shell, &[status])?;
            shell.state.show_explorer(crate::state::Explorer::Models);
            shell.explorer.expanded.insert(order);
            guide_view(shell, &[status, order], 0.9)?;
        }
        Screen::GuideTemplate | Screen::GuideTemplateBinding => {
            let d = new_diagram(shell, "Storage")?;
            let repo = id(&run(shell, "uml.class.add", json!({ "package": root.to_string(), "name": "Repository" }))?)?;
            let t = run(shell, "uml.template-parameter.add", json!({ "classifier": repo.to_string(), "name": "T" }))?;
            let key = run(
                shell,
                "uml.template-parameter.add",
                json!({ "classifier": repo.to_string(), "name": "Key", "kind": "data-type", "default": "Integer" }),
            )?;
            let element = |v: &Value| v.get("element").and_then(Value::as_str).unwrap_or_default().to_string();
            let (te, ke) = (element(&t), element(&key));
            run(shell, "uml.attribute.add", json!({ "class": repo.to_string(), "name": "items", "type": te, "multiplicity": "*" }))?;
            run(
                shell,
                "uml.operation.add",
                json!({ "class": repo.to_string(), "name": "find", "parameters": [{ "name": "key", "type": ke }], "returns": te }),
            )?;
            let repo_node = place(shell, d, repo, 380.0, 40.0, 300.0, crate::uml_ui::members_height(1, 1))?;
            if screen == Screen::GuideTemplate {
                select(shell, &[repo])?;
                crate::uml_templates::open(shell, repo, Some(id(&key)?));
                guide_view(shell, &[repo], 1.0)?;
            } else {
                let bound = id(&run(shell, "uml.class.add", json!({ "package": root.to_string(), "name": "OrderRepository" }))?)?;
                let bound_node = place(shell, d, bound, 410.0, 340.0, w, crate::uml_ui::members_height(0, 0))?;
                let binding = id(&run(
                    shell,
                    "uml.template-binding.add",
                    json!({ "bound": bound.to_string(), "template": repo.to_string(), "arguments": { "T": order.to_string() } }),
                )?)?;
                let edge = id(&run(
                    shell,
                    "diagram.connect",
                    json!({ "diagram": d.to_string(), "element": binding.to_string(), "source": bound_node.to_string(), "target": repo_node.to_string() }),
                )?)?;
                run(shell, "diagram.label", json!({ "edge": edge.to_string(), "kind": "name", "at": "middle" }))?;
                select(shell, &[binding])?;
                guide_view(shell, &[repo, bound], 0.9)?;
            }
        }
        Screen::GuideProfile | Screen::GuideStereotype => {
            let profile = id(&run(shell, "uml.profile.add", json!({ "name": "Persistence" }))?)?;
            let entity = id(&run(shell, "uml.stereotype.add", json!({ "profile": profile.to_string(), "name": "Entity", "extends": ["Class"] }))?)?;
            run(shell, "uml.attribute.add", json!({ "class": entity.to_string(), "name": "table", "type": "String" }))?;
            shell.state.show_explorer(crate::state::Explorer::Models);
            if screen == Screen::GuideProfile {
                crate::uml_profiles::open_profile_diagram(shell, profile)?;
                select(shell, &[entity])?;
                shell.explorer.expanded.insert(profile);
                shell.canvas.fit_pending = false;
                shell.canvas.zoom = 1.0;
                shell.canvas.pan = egui::vec2(80.0, 80.0);
            } else {
                run(shell, "uml.profile.apply", json!({ "package": root.to_string(), "profile": profile.to_string() }))?;
                run(shell, "uml.stereotype.apply", json!({ "element": order.to_string(), "stereotype": entity.to_string() }))?;
                run(
                    shell,
                    "uml.stereotype.set-value",
                    json!({ "element": order.to_string(), "stereotype": entity.to_string(), "property": "table", "values": ["orders"] }),
                )?;
                let d = shell.canvas.active.ok_or("no diagram is open")?;
                crate::uml_stereotypes::show_values(shell, d, true)?;
                braces_under_order(shell)?;
                select(shell, &[order])?;
                guide_view(shell, &[order], 1.0)?;
            }
        }
        _ => return Err("not a screen of the guide's further sections".into()),
    }
    Ok(())
}

/// Zooms the open diagram to `zoom` with the cards of `elements` centred in the part of the canvas
/// a 1440 by 900 window leaves free beside Models and Properties, above the toolbar and the
/// minimap.
fn guide_view(shell: &mut Shell, elements: &[archforge_mof::Uuid], zoom: f32) -> Result<(), String> {
    let store = shell.session.as_ref().ok_or("no project is open")?.doc();
    let mut bounds: Option<egui::Rect> = None;
    for e in elements {
        let o = store.by_uuid(*e).ok_or("an element of the guide is gone")?;
        // Its card on the open diagram, or its first.
        let cards = archforge_diagram::views_of(store, o).2;
        let open = shell.canvas.active.and_then(|d| store.by_uuid(d));
        let on_open = cards.iter().copied().find(|n| open.is_some() && archforge_diagram::diagram_of(store, *n) == open);
        let n = on_open.or_else(|| cards.first().copied()).ok_or("an element of the guide is on no diagram")?;
        let b = archforge_diagram::grid::bounds_of(store, n);
        let r = egui::Rect::from_min_size(egui::pos2(b.x as f32, b.y as f32), egui::vec2(b.width as f32, b.height as f32));
        bounds = Some(bounds.map_or(r, |all| all.union(r)));
    }
    let b = bounds.ok_or("the guide's view has no cards")?;
    // The free canvas's middle: 700 points wide, its top clear of the path bar, its foot of the
    // toolbar.
    let middle = egui::vec2(350.0, 330.0);
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = zoom;
    shell.canvas.pan = middle - b.center().to_vec2() * zoom;
    Ok(())
}

/// Invoice above right and Product right of Order, each related to Customer, so the straight or
/// plain right-angled way runs through Order; the diagram's lines go around the tables.
fn sample_routing(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let store = session.doc();
    let by_name =
        |name: &str| store.alive().find(|o| archforge_core::name_of(store, *o) == name).and_then(|o| store.uuid(o).ok()).map(|u| u.to_string());
    let (Some(customer), Some(schema), Some(diagram)) = (by_name("Customer"), by_name("Shop schema"), shell.canvas.active.map(|d| d.to_string()))
    else {
        return Err("the sample diagram".into());
    };
    let customer_node = {
        let store = session.doc();
        let c = store.by_uuid(archforge_mof::Uuid::parse(&customer).ok_or("the customer's id")?).ok_or("the customer")?;
        archforge_diagram::views_of(store, c).2.first().and_then(|n| store.uuid(*n).ok()).map(|u| u.to_string()).ok_or("the customer's node")?
    };
    for (name, rel, x, y) in [("Invoice", "bills", 920, 120), ("Product", "favours", 920, 380)] {
        let table = run(session, "er.entity.add", json!({ "schema": schema, "name": name }))?;
        run(
            session,
            "er.attribute.add",
            json!({ "entity": table, "name": format!("{}_id", name.to_lowercase()), "type": "BIGINT", "primary_key": true }),
        )?;
        let r = run(session, "er.relationship.add", json!({ "from": customer, "to": table, "name": rel, "create_foreign_key": true }))?;
        let n = run(
            session,
            "diagram.place",
            json!({ "diagram": diagram, "element": table, "x": x, "y": y, "width": 240, "height": crate::editing::content_height(2) }),
        )?;
        run(session, "diagram.connect", json!({ "diagram": diagram, "element": r, "source": customer_node, "target": n }))?;
    }
    run(session, "view.select", json!({ "ids": [] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.canvas.zoom = 0.75;
    shell.canvas.pan = egui::vec2(-40.0, 40.0);
    Ok(())
}

/// A schema "Shop schema" with its diagram open: empty, as the journey's first diagram; or with
/// Customer and Order placed, the relationship `places` between them and Customer selected, as
/// the design's ER frame.
/// Export skills on the project and on the sample's schema, the schema selected, and File →
/// Generate… open for it.
fn sample_generate(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let schema = store
        .alive()
        .find(|o| store.class_of(*o).ok() == Some(archforge_er::SCHEMA))
        .and_then(|s| store.uuid(s).ok())
        .ok_or("the sample has no schema")?
        .to_string();
    for params in [
        json!({ "name": "DDL conventions", "body": "Plain CREATE TABLE statements.", "triggers": ["export"] }),
        json!({ "subject": schema, "name": "DDL conventions", "body": "PostgreSQL; a named constraint per key.", "triggers": ["export"], "dialect": "PostgreSQL" }),
        json!({ "subject": schema, "name": "Audit columns", "body": "Every table gets created_at.", "triggers": ["export"] }),
    ] {
        session.execute("skill.add", &params).map_err(|e| format!("skill.add: {e:?}"))?;
    }
    session.execute("view.select", &json!({ "ids": [schema] })).map_err(|e| format!("view.select: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.open_generate();
    Ok(())
}

fn sample_diagram(shell: &mut Shell, filled: bool) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let schema = run(session, "er.schema.add", json!({ "name": "Shop schema" }))?;
    let diagram = run(session, "diagram.add", json!({ "kind": "er", "element": schema, "name": "Shop schema" }))?;
    let mut select = diagram.clone();
    if filled {
        let customer = run(session, "er.entity.add", json!({ "schema": schema, "name": "Customer" }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "region", "type": "TEXT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR", "length": 255, "unique": true }))?;
        run(session, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "VARCHAR", "length": 120 }))?;
        let order = run(session, "er.entity.add", json!({ "schema": schema, "name": "Order" }))?;
        run(session, "er.attribute.add", json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true }))?;
        run(session, "er.attribute.add", json!({ "entity": order, "name": "placed_at", "type": "TIMESTAMP" }))?;
        let places = run(
            session,
            "er.relationship.add",
            json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places", "create_foreign_key": true }),
        )?;
        let c = run(session, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 200, "y": 360, "width": 240, "height": 166 }))?;
        let o = run(session, "diagram.place", json!({ "diagram": diagram, "element": order, "x": 560, "y": 360, "width": 240, "height": 166 }))?;
        let edge = run(session, "diagram.connect", json!({ "diagram": diagram, "element": places, "source": c, "target": o }))?;
        run(session, "diagram.label", json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        select = customer;
    }
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    let id = archforge_mof::Uuid::parse(&diagram).ok_or_else(|| "the diagram's id does not parse".to_string())?;
    crate::canvas::open(shell, id);
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    run(session, "view.select", json!({ "ids": [select] }))?;
    // The canvas fits the diagram on its first frame; the sample wants the design's 100 %.
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::Vec2::ZERO;
    Ok(())
}

/// Party with its subtypes Person and Company on the empty sample diagram, drawn as the design's
/// "Subtypes" card, Party selected.
fn sample_subtypes(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let diagram = shell.canvas.active.and_then(|id| store.by_uuid(id)).ok_or_else(|| "the sample has no open diagram".to_string())?;
    let schema = crate::diagrams::schema_of(store, diagram).ok_or_else(|| "the sample diagram has no schema".to_string())?;
    let (schema, diagram) = (store.uuid(schema).map_err(|e| e.to_string())?.to_string(), store.uuid(diagram).map_err(|e| e.to_string())?.to_string());
    let run = |session: &mut archforge_notations::Session, command: &str, params: serde_json::Value| -> Result<String, String> {
        let out = session.execute(command, &params).map_err(|e| format!("{command}: {e:?}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let party = run(session, "er.entity.add", json!({ "schema": schema, "name": "Party" }))?;
    run(session, "er.attribute.add", json!({ "entity": party, "name": "party_id", "type": "BIGINT", "primary_key": true }))?;
    let kind = run(session, "er.attribute.add", json!({ "entity": party, "name": "party_type", "type": "TEXT" }))?;
    let person = run(session, "er.entity.add", json!({ "schema": schema, "name": "Person" }))?;
    let company = run(session, "er.entity.add", json!({ "schema": schema, "name": "Company" }))?;
    let g1 = run(session, "er.generalization.add", json!({ "supertype": party, "subtype": person }))?;
    let g2 = run(session, "er.generalization.add", json!({ "supertype": party, "subtype": company }))?;
    run(session, "er.attribute.add", json!({ "entity": person, "name": "birth_date", "type": "DATE" }))?;
    run(session, "er.attribute.add", json!({ "entity": company, "name": "vat_no", "type": "TEXT" }))?;
    run(
        session,
        "er.entity.set-subtyping",
        json!({ "entity": party, "complete": true, "disjoint": true, "discriminator": kind, "mapping": "table-per-type" }),
    )?;
    let n_party = run(session, "diagram.place", json!({ "diagram": diagram, "element": party, "x": 420, "y": 200, "width": 240, "height": 126 }))?;
    let n_person = run(session, "diagram.place", json!({ "diagram": diagram, "element": person, "x": 200, "y": 460, "width": 240, "height": 126 }))?;
    let n_company =
        run(session, "diagram.place", json!({ "diagram": diagram, "element": company, "x": 640, "y": 460, "width": 240, "height": 126 }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": g1, "source": n_person, "target": n_party }))?;
    run(session, "diagram.connect", json!({ "diagram": diagram, "element": g2, "source": n_company, "target": n_party }))?;
    run(session, "view.select", json!({ "ids": [party] }))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    Ok(())
}

/// The first entity of the empty sample diagram, placed by the Entity tool with its name open for
/// typing, as the design's "First entity named in place".
fn sample_new_entity(shell: &mut Shell) -> Result<(), String> {
    use serde_json::json;
    let session = shell.session.as_mut().ok_or_else(|| "the sample needs a session".to_string())?;
    let store = session.doc();
    let diagram = shell.canvas.active.and_then(|id| store.by_uuid(id)).ok_or_else(|| "the sample has no open diagram".to_string())?;
    let schema = crate::diagrams::schema_of(store, diagram).ok_or_else(|| "the sample diagram has no schema".to_string())?;
    let (schema_id, diagram_id) = (store.uuid(schema).map_err(|e| e.to_string())?, store.uuid(diagram).map_err(|e| e.to_string())?);
    let out = session
        .batch("Add table", |b| {
            let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": "Entity" }))?;
            let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            b.execute(
                "diagram.place",
                &json!({ "diagram": diagram_id.to_string(), "element": entity, "x": 200, "y": 360, "width": crate::editing::ENTITY_WIDTH, "height": crate::editing::ENTITY_EMPTY_HEIGHT }),
            )?;
            Ok(())
        })
        .map_err(|e| format!("Add table: {e:?}"))?;
    let id_at = |i: usize| out.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(archforge_mof::Uuid::parse);
    let (Some(entity), Some(node)) = (id_at(0), id_at(1)) else { return Err("the sample's table has no id".to_string()) };
    session.execute("view.select", &json!({ "ids": [entity.to_string()] })).map_err(|e| format!("view.select: {e:?}"))?;
    session.mark_saved().map_err(|e| format!("mark_saved: {e:?}"))?;
    shell.canvas.tool = crate::canvas::Tool::Place("entity");
    shell.canvas.rename = Some(crate::editing::NodeRename { node, entity, text: "Entity".to_string(), opened: true, refused: None });
    Ok(())
}
