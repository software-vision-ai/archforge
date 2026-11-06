//! Renders the window offscreen to a PNG, without opening one. Usage:
//! `cargo run -p archforge-ui-egui --features snapshot --example snapshot -- <out.png> [light|dark] [width height [scale]] [--screen home|home-gallery|home-search|home-settings|home-about|settings-ai-agents-empty|settings-ai-agents-missing|settings-ai-agents-ready|settings-ai-agents-refused|settings-ai-agents-device-code|settings-ai-agents-no-store|settings-ai-agents-not-available|home-dolt-missing|new-project|new-project-failed|new-project-folder|workspace|workspace-schema|workspace-table|workspace-skills|workspace-skill-tab|workspace-skill-preview|workspace-skill-link|workspace-skill-new|workspace-diagram-empty|workspace-diagram|workspace-toast|workspace-generate|workspace-chat|workspace-appearance|workspace-appearance-line|workspace-appearance-picker|workspace-entity-new|workspace-cardinality|workspace-relationship|workspace-key-editor|workspace-column-editor|workspace-index-editor|workspace-mapping-table|workspace-schema-types|workspace-subtypes|workspace-schema-objects|workspace-cross-schema|workspace-routing|workspace-search|workspace-column-selected|workspace-class-diagram|workspace-class-diagram-read|workspace-class-sizing|workspace-class-members|workspace-member-editor|workspace-inline-member|workspace-association|workspace-multiplicity-presets|workspace-multiplicity-refused|workspace-relate-kinds|workspace-interface|workspace-enumeration|workspace-data-type|workspace-primitive-type|workspace-literals|workspace-classifier-kinds|workspace-realization|workspace-association-class|workspace-modifiers|workspace-final|workspace-leaf|workspace-generalization-sets|workspace-qualifiers|workspace-qualifier-refused|workspace-nested-classifiers|workspace-profiles|workspace-profile-refused|workspace-stereotypes|workspace-stereotype-picker|workspace-template|workspace-template-kinds|workspace-template-in-use|workspace-nary-drawing|workspace-nary|workspace-nary-binary|workspace-template-binding|workspace-template-binding-new|workspace-guide-classes|workspace-guide-relate-kinds|workspace-guide-association|workspace-guide-classifier-kinds|workspace-guide-literals|workspace-guide-data-type|workspace-guide-primitive-type|workspace-guide-realization|workspace-guide-association-class|workspace-guide-modifiers|workspace-guide-constraints|workspace-guide-result|workspace-close-ask]`

use archforge_ui_egui::offscreen::Screen;
use archforge_ui_egui::theme::ThemeKind;

fn main() {
    if let Err(e) = run() {
        eprintln!("snapshot: {e}");
        std::process::exit(1);
    }
}

/// Reads the arguments, renders the screen asked for offscreen and writes it as a PNG.
fn run() -> Result<(), String> {
    const USAGE: &str = "usage: snapshot <out.png> [light|dark] [width height [scale]] [--screen home|new-project|new-project-failed|new-project-folder|workspace|workspace-schema]";
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut screen = Screen::Home;
    if let Some(at) = args.iter().position(|a| a == "--screen") {
        screen = match args.get(at + 1).map(String::as_str) {
            Some("home") => Screen::Home,
            Some("home-gallery") => Screen::HomeGallery,
            Some("home-search") => Screen::HomeSearch,
            Some("home-settings") => Screen::HomeSettings,
            Some("home-about") => Screen::HomeAbout,
            Some("settings-ai-agents-empty") => Screen::SettingsAgentsEmpty,
            Some("settings-ai-agents-missing") => Screen::SettingsAgentsMissing,
            Some("settings-ai-agents-ready") => Screen::SettingsAgentsReady,
            Some("settings-ai-agents-refused") => Screen::SettingsAgentsRefused,
            Some("settings-ai-agents-device-code") => Screen::SettingsAgentsDeviceCode,
            Some("settings-ai-agents-no-store") => Screen::SettingsAgentsNoStore,
            Some("settings-ai-agents-not-available") => Screen::SettingsAgentsNotAvailable,
            Some("home-dolt-missing") => Screen::HomeDoltMissing,
            Some("new-project") => Screen::NewProject,
            Some("new-project-failed") => Screen::NewProjectFailed,
            Some("new-project-folder") => Screen::NewProjectFolder,
            Some("workspace") => Screen::Workspace,
            Some("workspace-schema") => Screen::WorkspaceSchema,
            Some("workspace-table") => Screen::WorkspaceTable,
            Some("workspace-skills") => Screen::WorkspaceSkills,
            Some("workspace-skill-tab") => Screen::WorkspaceSkillTab,
            Some("workspace-skill-preview") => Screen::WorkspaceSkillPreview,
            Some("workspace-skill-link") => Screen::WorkspaceSkillLink,
            Some("workspace-skill-new") => Screen::WorkspaceSkillNew,
            Some("workspace-key-editor") => Screen::WorkspaceKeyEditor,
            Some("workspace-index-editor") => Screen::WorkspaceIndexEditor,
            Some("workspace-mapping-table") => Screen::WorkspaceMappingTable,
            Some("workspace-schema-types") => Screen::WorkspaceSchemaTypes,
            Some("workspace-subtypes") => Screen::WorkspaceSubtypes,
            Some("workspace-schema-objects") => Screen::WorkspaceSchemaObjects,
            Some("workspace-cross-schema") => Screen::WorkspaceCrossSchema,
            Some("workspace-routing") => Screen::WorkspaceRouting,
            Some("workspace-class-diagram") => Screen::WorkspaceClassDiagram,
            Some("workspace-class-diagram-read") => Screen::WorkspaceClassDiagramRead,
            Some("workspace-packages") => Screen::WorkspacePackages,
            Some("workspace-inside-package") => Screen::WorkspaceInsidePackage,
            Some("workspace-package-parent") => Screen::WorkspacePackageParent,
            Some("workspace-package-top") => Screen::WorkspacePackageTop,
            Some("workspace-diagrams") => Screen::WorkspaceDiagrams,
            Some("workspace-new-diagram-kind") => Screen::WorkspaceNewDiagramKind,
            Some("workspace-new-diagram-details") => Screen::WorkspaceNewDiagramDetails,
            Some("workspace-chat") => Screen::WorkspaceChat,
            Some("workspace-class-sizing") => Screen::WorkspaceClassSizing,
            Some("workspace-class-members") => Screen::WorkspaceClassMembers,
            Some("workspace-member-editor") => Screen::WorkspaceMemberEditor,
            Some("workspace-inline-member") => Screen::WorkspaceInlineMember,
            Some("workspace-association") => Screen::WorkspaceAssociation,
            Some("workspace-multiplicity-presets") => Screen::WorkspaceMultiplicityPresets,
            Some("workspace-multiplicity-refused") => Screen::WorkspaceMultiplicityRefused,
            Some("workspace-relate-kinds") => Screen::WorkspaceRelateKinds,
            Some("workspace-interface") => Screen::WorkspaceInterface,
            Some("workspace-enumeration") => Screen::WorkspaceEnumeration,
            Some("workspace-data-type") => Screen::WorkspaceDataType,
            Some("workspace-primitive-type") => Screen::WorkspacePrimitiveType,
            Some("workspace-literals") => Screen::WorkspaceLiterals,
            Some("workspace-classifier-kinds") => Screen::WorkspaceClassifierKinds,
            Some("workspace-realization") => Screen::WorkspaceRealization,
            Some("workspace-association-class") => Screen::WorkspaceAssociationClass,
            Some("workspace-modifiers") => Screen::WorkspaceModifiers,
            Some("workspace-final") => Screen::WorkspaceFinal,
            Some("workspace-leaf") => Screen::WorkspaceLeaf,
            Some("workspace-generalization-sets") => Screen::WorkspaceGeneralizationSets,
            Some("workspace-qualifiers") => Screen::WorkspaceQualifiers,
            Some("workspace-qualifier-refused") => Screen::WorkspaceQualifierRefused,
            Some("workspace-nested-classifiers") => Screen::WorkspaceNestedClassifiers,
            Some("workspace-template") => Screen::WorkspaceTemplate,
            Some("workspace-template-kinds") => Screen::WorkspaceTemplateKinds,
            Some("workspace-template-in-use") => Screen::WorkspaceTemplateInUse,
            Some("workspace-template-binding") => Screen::WorkspaceTemplateBinding,
            Some("workspace-template-binding-new") => Screen::WorkspaceTemplateBindingNew,
            Some("workspace-profiles") => Screen::WorkspaceProfiles,
            Some("workspace-profile-refused") => Screen::WorkspaceProfileRefused,
            Some("workspace-nary-drawing") => Screen::WorkspaceNaryDrawing,
            Some("workspace-nary") => Screen::WorkspaceNary,
            Some("workspace-nary-binary") => Screen::WorkspaceNaryBinary,
            Some("workspace-stereotypes") => Screen::WorkspaceStereotypes,
            Some("workspace-stereotype-picker") => Screen::WorkspaceStereotypePicker,
            Some("workspace-guide-classes") => Screen::GuideClasses,
            Some("workspace-guide-relate-kinds") => Screen::GuideRelateKinds,
            Some("workspace-guide-association") => Screen::GuideAssociation,
            Some("workspace-guide-classifier-kinds") => Screen::GuideClassifierKinds,
            Some("workspace-guide-literals") => Screen::GuideLiterals,
            Some("workspace-guide-data-type") => Screen::GuideDataType,
            Some("workspace-guide-primitive-type") => Screen::GuidePrimitiveType,
            Some("workspace-guide-realization") => Screen::GuideRealization,
            Some("workspace-guide-association-class") => Screen::GuideAssociationClass,
            Some("workspace-guide-modifiers") => Screen::GuideModifiers,
            Some("workspace-guide-constraints") => Screen::GuideConstraints,
            Some("workspace-guide-result") => Screen::GuideResult,
            Some("workspace-close-ask") => Screen::WorkspaceCloseAsk,
            Some("workspace-search") => Screen::WorkspaceSearch,
            Some("workspace-column-selected") => Screen::WorkspaceColumnSelected,
            Some("workspace-column-editor") => Screen::WorkspaceColumnEditor,
            Some("workspace-diagram-empty") => Screen::WorkspaceDiagramEmpty,
            Some("workspace-diagram") => Screen::WorkspaceDiagram,
            Some("workspace-toast") => Screen::WorkspaceToast,
            Some("workspace-generate") => Screen::WorkspaceGenerate,
            Some("workspace-appearance") => Screen::WorkspaceAppearance,
            Some("workspace-appearance-line") => Screen::WorkspaceAppearanceLine,
            Some("workspace-appearance-picker") => Screen::WorkspaceAppearancePicker,
            Some("workspace-entity-new") => Screen::WorkspaceEntityNew,
            Some("workspace-cardinality") => Screen::WorkspaceCardinality,
            Some("workspace-relationship") => Screen::WorkspaceRelationship,
            other => {
                return Err(format!(
                    "--screen {other:?} is not home, home-gallery, home-search, home-settings, home-about, settings-ai-agents-empty, settings-ai-agents-missing, settings-ai-agents-ready, settings-ai-agents-refused, settings-ai-agents-device-code, settings-ai-agents-no-store, settings-ai-agents-not-available, home-dolt-missing, new-project, new-project-failed, new-project-folder, workspace, workspace-schema, workspace-table, workspace-skills, workspace-skill-tab, workspace-skill-preview, workspace-skill-link, workspace-skill-new, workspace-diagram-empty, workspace-diagram, workspace-toast, workspace-generate, workspace-chat, workspace-appearance, workspace-appearance-line, workspace-appearance-picker, workspace-entity-new, workspace-cardinality, workspace-relationship, workspace-key-editor, workspace-column-editor, workspace-index-editor, workspace-mapping-table, workspace-schema-types, workspace-subtypes, workspace-schema-objects, workspace-cross-schema, workspace-routing, workspace-search, workspace-column-selected, workspace-class-diagram, workspace-class-diagram-read, workspace-class-sizing, workspace-class-members, workspace-member-editor, workspace-inline-member, workspace-association, workspace-multiplicity-presets, workspace-multiplicity-refused, workspace-relate-kinds, workspace-interface, workspace-enumeration, workspace-data-type, workspace-primitive-type, workspace-literals, workspace-classifier-kinds, workspace-realization, workspace-association-class, workspace-modifiers, workspace-final, workspace-leaf, workspace-generalization-sets, workspace-qualifiers, workspace-qualifier-refused, workspace-nested-classifiers, workspace-profiles, workspace-profile-refused, workspace-stereotypes, workspace-stereotype-picker, workspace-template, workspace-template-kinds, workspace-template-in-use, workspace-nary-drawing, workspace-nary, workspace-nary-binary, workspace-template-binding, workspace-template-binding-new, workspace-guide-classes, workspace-guide-relate-kinds, workspace-guide-association, workspace-guide-classifier-kinds, workspace-guide-literals, workspace-guide-data-type, workspace-guide-primitive-type, workspace-guide-realization, workspace-guide-association-class, workspace-guide-modifiers, workspace-guide-constraints, workspace-guide-result or workspace-close-ask; {USAGE}"
                ));
            }
        };
        args.drain(at..(at + 2).min(args.len()));
    }
    let out = args.first().ok_or(USAGE)?;
    let theme = match args.get(1).map(String::as_str) {
        None | Some("light") => ThemeKind::Light,
        Some("dark") => ThemeKind::Dark,
        Some(other) => return Err(format!("{other:?} is not light or dark; {USAGE}")),
    };
    let size = |i: usize, default: f32| args.get(i).map_or(Ok(default), |v| v.parse::<f32>().map_err(|_| format!("{v:?} is not a number; {USAGE}")));
    let (w, h, rgba) = archforge_ui_egui::offscreen::render_screen(size(2, 1440.0)?, size(3, 900.0)?, size(4, 1.0)?, theme, screen)?;
    let file = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut writer| writer.write_image_data(&rgba)).map_err(|e| format!("{out}: {e}"))
}
