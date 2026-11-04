//! Renders the window offscreen to a PNG, without opening one. Usage:
//! `cargo run -p archforge-ui-egui --features snapshot --example snapshot -- <out.png> [light|dark] [width height [scale]] [--screen home|home-gallery|home-search|home-settings|home-about|home-dolt-missing|new-project|new-project-failed|new-project-folder|workspace|workspace-schema|workspace-table|workspace-skills|workspace-skill-tab|workspace-skill-preview|workspace-skill-link|workspace-skill-new|workspace-diagram-empty|workspace-diagram|workspace-toast|workspace-generate|workspace-chat|workspace-appearance|workspace-appearance-line|workspace-appearance-picker|workspace-entity-new|workspace-cardinality|workspace-relationship|workspace-key-editor|workspace-column-editor|workspace-index-editor|workspace-mapping-table|workspace-schema-types|workspace-subtypes|workspace-schema-objects|workspace-cross-schema|workspace-routing|workspace-search|workspace-column-selected|workspace-class-diagram|workspace-class-diagram-read|workspace-class-sizing|workspace-class-members|workspace-member-editor|workspace-inline-member|workspace-association|workspace-multiplicity-presets|workspace-multiplicity-refused|workspace-relate-kinds|workspace-close-ask]`

use archforge_ui_egui::offscreen::Screen;
use archforge_ui_egui::theme::ThemeKind;

fn main() {
    if let Err(e) = run() {
        eprintln!("snapshot: {e}");
        std::process::exit(1);
    }
}

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
                    "--screen {other:?} is not home, home-gallery, home-search, home-settings, home-about, home-dolt-missing, new-project, new-project-failed, new-project-folder, workspace, workspace-schema, workspace-table, workspace-skills, workspace-skill-tab, workspace-skill-preview, workspace-skill-link, workspace-skill-new, workspace-diagram-empty, workspace-diagram, workspace-toast, workspace-generate, workspace-chat, workspace-appearance, workspace-appearance-line, workspace-appearance-picker, workspace-entity-new, workspace-cardinality, workspace-relationship, workspace-key-editor, workspace-column-editor, workspace-index-editor, workspace-mapping-table, workspace-schema-types, workspace-subtypes, workspace-schema-objects, workspace-cross-schema, workspace-routing, workspace-search, workspace-column-selected, workspace-class-diagram, workspace-class-diagram-read, workspace-class-sizing, workspace-class-members, workspace-member-editor, workspace-inline-member, workspace-association, workspace-multiplicity-presets, workspace-multiplicity-refused, workspace-relate-kinds or workspace-close-ask; {USAGE}"
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
