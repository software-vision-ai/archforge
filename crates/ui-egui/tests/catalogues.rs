//! The shell's interfaces register their own catalogue before their first lookup, so a refusal they
//! say in another language before anything else was looked up comes out translated, with no shell;
//! the shell registers its own and its notations' as they are set up (`Shell::new`), so a word of
//! the shell's catalogue the interfaces say is translated once a shell is made. A test binary of
//! its own, so that nothing else in its process has looked a text up first.

use archforge_engine::EngineError;
use archforge_shell_api::i18n::set_language;
use archforge_shell_api::notation_ui::Subject;
use archforge_shell_api::panels::refusal;
use archforge_shell_api::screens::ScreensRegistry;
use archforge_ui_egui::{ER, Services, Shell, UML};

/// The app's screens parts, ER's then UML's, which the shell is given.
static SCREENS: ScreensRegistry = ScreensRegistry::new(&[ER, UML]);

/// A refusal in German before a shell is made, from the interfaces' own catalogue, and a word of
/// the shell's catalogue in English until a shell is made and in German once one is, with nothing
/// else looked up between.
#[test]
fn a_refusal_said_before_any_other_lookup_is_translated() {
    assert!(set_language("de"));
    let german = "Das Modell kann gerade nicht geändert werden; nichts wurde geändert.";
    assert_eq!(refusal(&[], &EngineError::ReadOnly), german, "the interfaces' own row is in before a shell is made");
    let root = Subject::root("Demo");
    assert_eq!((root.kind.as_str(), root.word.as_str()), ("Modell · die Wurzel des Projekts", "Model"), "the shell's catalogue is not in yet");
    let _shell = Shell::new(Services::default(), &SCREENS);
    assert_eq!(Subject::root("Demo").word, "Modell", "the shell registered its catalogue");
    assert_eq!(refusal(&[], &EngineError::ReadOnly), german);
}
