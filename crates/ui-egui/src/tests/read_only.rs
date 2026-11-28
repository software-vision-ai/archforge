/// Only what changes nothing stays on the canvas; every other action goes.
#[test]
fn only_looking_stays_on_the_canvas() {
    use crate::editing::{Action, Gesture, only_looks};
    let (id, at) = (archforge_mof::Uuid([1; 16]), egui::Pos2::ZERO);
    assert!(
        [Action::Nothing, Action::Select(vec![id]), Action::Toggle(id), Action::Commit, Action::Start(Gesture::Marquee { from: at }, None)]
            .iter()
            .all(only_looks)
    );
    let move_it = Action::Start(Gesture::Move { nodes: vec![id], from: at }, None);
    let edits = [
        move_it,
        Action::Place("x", 0.0, 0.0),
        Action::Rename(id, id),
        Action::Open(id, Some(0)),
        Action::PlaceNote(0.0, 0.0),
        Action::EditNote(id),
        Action::ResetEnd(id, 0),
        Action::ResetRoute(id),
        // Whatever a diagram's kind makes of a press: a UML n-ary association's next end, Add end,
        // or a menu of its own.
        Action::Own(Box::new(|_| {})),
    ];
    assert!(!edits.iter().any(only_looks));
}
