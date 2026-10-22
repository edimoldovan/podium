//! The window, driven as the user drives it.

use eframe::egui;
use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use podium::app::{PAGES, PodiumApp, TITLES};
use podium::deck::Deck;
use podium::session::Session;

fn harness(view: usize) -> Harness<'static, PodiumApp> {
    let mut app = PodiumApp::new(Session::new(Deck::new(), None), false);
    app.view = view;
    app.page = 1;
    Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_state(|ctx, app: &mut PodiumApp| app.frame(ctx), app)
}

/// An element inserted from the inspector is selected; Shift and an arrow
/// move it ten; undo takes the move back, then the insertion.
#[test]
fn an_inserted_element_moves_and_the_moves_undo() {
    let mut h = harness(PAGES);
    h.run();
    h.get_by_role_and_label(Role::Button, "Callout").click();
    h.run();
    let page = || 1;
    let callout = |h: &Harness<'_, PodiumApp>| h.state().session.deck.pages[page()].elements.iter().find(|e| e.body.kind() == "callout").map(|e| e.x);
    let x = callout(&h).expect("the callout is there");
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run();
    assert_eq!(callout(&h), Some(x + 10.0));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(callout(&h), Some(x), "the move is taken back");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(callout(&h), None, "then the insertion");
}

/// A title typed in the titles view is one step to undo, taken when the
/// field lets go.
#[test]
fn a_typed_title_is_one_undo_step() {
    let mut h = harness(TITLES);
    h.run();
    let field = h.get_by_role_and_label(Role::TextInput, "Title of page 2");
    field.focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Title of page 2").type_text("Demand doubles by 2028");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().session.deck.pages[1].title, "Demand doubles by 2028");
    assert!(h.state().session.can_undo());
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().session.deck.pages[1].title, "Executive summary");
}
