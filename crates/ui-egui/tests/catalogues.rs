//! The shell registers the catalogues as its notations are set up (`Shell::new`), so a refusal the
//! shell's interfaces say in another language before anything else was looked up comes out
//! translated. A test binary of its own, so that nothing else in its process has looked a text up
//! first.

use archforge_engine::EngineError;
use archforge_shell_api::i18n::set_language;
use archforge_shell_api::panels::refusal;
use archforge_shell_api::screens::ScreensRegistry;
use archforge_ui_egui::{ER, Services, Shell, UML};

/// The app's screens parts, ER's then UML's, which the shell is given.
static SCREENS: ScreensRegistry = ScreensRegistry::new(&[ER, UML]);

/// English before a shell is made, German once one is, with nothing else looked up between.
#[test]
fn a_refusal_said_before_any_other_lookup_is_translated() {
    assert!(set_language("de"));
    let english = "The model cannot be changed at the moment; nothing was changed.";
    assert_eq!(refusal(&[], &EngineError::ReadOnly), english, "no catalogue is in before a shell is made");
    let _shell = Shell::new(Services::default(), &SCREENS);
    let german = "Das Modell kann gerade nicht geändert werden; nichts wurde geändert.";
    assert_eq!(refusal(&[], &EngineError::ReadOnly), german);
}
