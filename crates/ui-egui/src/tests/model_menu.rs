//! The Model menu: its rows are what the open diagram's tools make, each arms that tool and no
//! other, and all of them answer to the one rule of when a tool can be picked.

use super::*;
use crate::actions::{available, label, run};
use crate::canvas::Tool;
use crate::model_menu::sections as model_sections;

const BOTH: [ThemeKind; 2] = [ThemeKind::Light, ThemeKind::Dark];
const SHARED: [&str; 2] = ["model.note", "model.link"];

/// Opens the Model menu and gives the actions of the rows it draws.
fn drawn_rows(h: &mut Harness<'static, Shell>) -> Vec<String> {
    click_menu(h, "strip.menu.model");
    menu_ids(h).iter().filter_map(|i| i.strip_prefix("strip.menu.row.")).map(String::from).collect()
}

/// The menu builds from what the open diagram's kind answers, a kind it never names included: the
/// sketch kind's sections in its order, then the shared ones, drawn so; the table of actions finds
/// its rows among the shell's kinds, on while its diagram shows, with its labels, arming its
/// tools; another kind's rows (the second sketch's, which the shell holds beside it) are off there,
/// and a project that cannot change turns its rows off as it does every other kind's.
#[test]
fn the_menu_builds_from_the_open_kinds_answer() {
    let own = ["model.sketch-class", "model.sketch-inherit"];
    for theme in BOTH {
        let (mut h, ..) = sketch_harness();
        h.state_mut().kinds = SKETCH_SCREENS.kinds();
        (h.state_mut().state.theme, h.state_mut().in_window_menu) = (theme, true);
        h.run();
        assert_eq!(model_sections(h.state()), [vec![own[0]], vec![own[1]], SHARED.to_vec()], "{theme:?}");
        assert_eq!(drawn_rows(&mut h), [own.as_slice(), &SHARED].concat(), "{theme:?}");
        let s = h.state_mut();
        s.menu_bar.open = None;
        assert!(own.iter().all(|id| available(s, id)), "{theme:?}: its rows are on while its diagram shows");
        assert_eq!(own.map(|id| label(s, id)), ["Class", "Inherit"], "{theme:?}");
        assert!(!available(s, "model.second-sketch-class") && available(s, "model.note"), "{theme:?}: another kind's rows are off");
        for (id, tool) in own.into_iter().zip([Tool::Place("class"), Tool::Connect("inherit")]) {
            s.canvas.tool = Tool::Select;
            (s.canvas.popover, s.canvas.menu_focus) = (Some("inherit"), Some(0));
            run(s, id);
            assert_eq!((s.canvas.tool, s.canvas.popover, s.canvas.menu_focus), (tool, None, None), "{theme:?}: {id}");
        }
        s.session.as_mut().unwrap().set_read_only(true).unwrap();
        s.canvas.tool = Tool::Select;
        assert!(own.iter().all(|id| !available(s, id)), "{theme:?}: off while the project cannot change");
        run(s, own[0]);
        assert_eq!(s.canvas.tool, Tool::Select, "{theme:?}: a row off arms nothing");
    }
}
