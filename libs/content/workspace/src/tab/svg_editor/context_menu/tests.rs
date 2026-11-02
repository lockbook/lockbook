use super::*;
use crate::tab::CanvasPasteContent;
use crate::tab::svg_editor::tools::selection::SelectedElement;
use crate::workspace::WsPersistentStore;
use egui::{Context, Event, Modifiers, PointerButton, RawInput, Rect, pos2, vec2};
use lb_rs::blocking::Lb;
use lb_rs::model::core_config::{ClientType, Config};
use std::sync::LazyLock;

static CORE: LazyLock<Lb> = LazyLock::new(|| {
    Lb::init(Config {
        writeable_path: std::env::temp_dir()
            .join(Uuid::new_v4().to_string())
            .to_string_lossy()
            .into_owned(),
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
        client_type: ClientType::Unknown,
    })
    .unwrap()
});
const EMPTY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
const DRAWING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300">
    <path d="M100 100 L160 100 L160 160 L100 160 Z"/>
    <path d="M250 100 L310 100 L310 160 L250 160 Z"/>
</svg>"#;

fn editor(ctx: &Context, drawing: &str) -> SVGEditor {
    let cfg =
        WsPersistentStore::new(false, std::env::temp_dir().join(Uuid::new_v4().to_string()), false);
    let mut svg =
        SVGEditor::new(drawing.as_bytes(), ctx, CORE.clone(), Uuid::new_v4(), None, &cfg, false);
    svg.viewport_settings.container_rect = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
    svg.viewport_settings.working_rect = svg.viewport_settings.container_rect;
    svg
}
fn context() -> Context {
    let ctx = crate::tab::markdown_editor::test_egui_ctx();
    crate::register_font_system(&ctx);
    ctx
}
fn frame(ctx: &Context, events: Vec<Event>, f: impl FnOnce(&mut egui::Ui)) {
    ctx.begin_pass(RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
        events,
        ..Default::default()
    });
    ctx.root_ui(|root| {
        egui::CentralPanel::default().show(root, f);
    });
    ctx.end_pass().textures_delta.clear();
    ctx.pop_events();
}
fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }
}
fn open(ctx: &Context, svg: &mut SVGEditor, pos: Pos2) {
    frame(ctx, vec![button(pos, PointerButton::Secondary, true)], |ui| svg.process_events(ui));
    assert!(svg.context_menu.quick);
}

#[test]
fn right_click_resolves_selection_and_preserves_groups() {
    let ctx = context();
    let mut svg = editor(&ctx, DRAWING);
    svg.toolbar.selection.selected_elements = svg
        .buffer
        .elements
        .keys()
        .map(|&id| SelectedElement { id, transform: Transform::identity() })
        .collect();
    open(&ctx, &mut svg, pos2(130., 130.));
    assert_eq!(svg.toolbar.selection.selected_elements.len(), 2);
    assert_eq!(svg.context_actions(&ctx) & 0xffff, 1 | 2 | 4);
    open(&ctx, &mut svg, pos2(500., 400.));
    assert!(svg.toolbar.selection.selected_elements.is_empty());
    assert_eq!(svg.context_actions(&ctx) & 0xffff, 8 | 16 | 32 | 64);
    open(&ctx, &mut svg, pos2(280., 130.));
    assert_eq!(svg.toolbar.selection.selected_elements.len(), 1);
}

#[test]
fn outside_click_consumes_press_move_release_without_a_stroke() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    open(&ctx, &mut svg, pos2(400., 300.));
    let pos = pos2(80., 400.);
    frame(&ctx, vec![button(pos, PointerButton::Primary, true)], |ui| svg.process_events(ui));
    assert!(!svg.context_menu.quick);
    frame(
        &ctx,
        vec![Event::PointerMoved(pos2(140., 420.)), button(pos, PointerButton::Primary, false)],
        |ui| svg.process_events(ui),
    );
    assert!(svg.buffer.elements.is_empty());
    assert!(!svg.history.has_undo());
    // A new gesture is a normal drawing interaction.
    frame(&ctx, vec![button(pos, PointerButton::Primary, true)], |ui| svg.process_events(ui));
    frame(&ctx, vec![button(pos2(120., 420.), PointerButton::Primary, false)], |ui| {
        svg.process_events(ui)
    });
    assert!(svg.history.has_undo());
}

#[test]
fn escape_focus_loss_and_zoom_dismiss_without_editing() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    for event in [
        Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        Event::WindowFocused(false),
        Event::Zoom(1.2),
    ] {
        open(&ctx, &mut svg, pos2(400., 300.));
        frame(&ctx, vec![event], |ui| svg.process_events(ui));
        assert!(!svg.context_menu.quick && !svg.context_menu.advanced);
        assert!(svg.context_menu.rect.is_none());
        assert!(!svg.history.has_undo());
    }
}

#[test]
fn selection_changes_and_other_popovers_close_advanced_menu() {
    let ctx = context();
    let mut svg = editor(&ctx, DRAWING);
    open(&ctx, &mut svg, pos2(130., 130.));
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::More as u32));
    assert!(svg.context_menu.advanced);
    svg.toolbar.selection.selected_elements.clear();
    frame(&ctx, vec![], |ui| svg.handle_context_menu_input(ui));
    assert!(!svg.context_menu.advanced);
    svg.toolbar.show_tool_popover = true;
    frame(&ctx, vec![], |ui| svg.request_context_menu(ui, pos2(130., 130.)));
    assert!(!svg.toolbar.has_visible_popover());
}

#[test]
fn native_dismissal_is_reflected_but_cannot_cancel_more_handoff() {
    let ctx = context();
    let mut svg = editor(&ctx, DRAWING);
    open(&ctx, &mut svg, pos2(130., 130.));
    svg.context_menu.native = true;
    svg.context_menu.quick = false;
    frame(&ctx, vec![], |ui| svg.context_action(ui, 0));
    assert!(!svg.context_menu.native);
    open(&ctx, &mut svg, pos2(130., 130.));
    svg.context_menu.native = true;
    svg.context_menu.quick = false;
    frame(&ctx, vec![], |ui| {
        svg.context_action(ui, CanvasAction::More as u32);
        svg.context_action(ui, 0); // A delayed platform dismissal after choosing More.
    });
    assert!(svg.context_menu.advanced);
    assert!(matches!(
        ctx.pop_context_menu(),
        Some((_, ContextMenuTarget::Canvas(DISMISS_NATIVE_MENU)))
    ));
}

#[test]
fn readonly_and_empty_document_actions_are_disabled_and_revalidated() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    svg.read_only = true;
    open(&ctx, &mut svg, pos2(400., 300.));
    assert_eq!(svg.context_actions(&ctx) >> 16, 0);
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::Paste as u32));
    assert_eq!(take_host_command(&ctx), 0);
    let mut svg = editor(&ctx, DRAWING);
    svg.read_only = true;
    open(&ctx, &mut svg, pos2(130., 130.));
    assert_eq!(svg.context_actions(&ctx) >> 16, 1 | 4);
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::Delete as u32));
    assert!(!svg.history.has_undo());
}

#[test]
fn document_change_discards_late_import_and_serializes_host_operations() {
    let ctx = context();
    let mut first = editor(&ctx, EMPTY);
    let mut second = editor(&ctx, EMPTY);
    open(&ctx, &mut first, pos2(400., 300.));
    frame(&ctx, vec![], |ui| first.context_action(ui, CanvasAction::ImportImage as u32));
    assert_eq!(take_host_command(&ctx), 16);
    first.deactivate_context_menu(&ctx);
    open(&ctx, &mut second, pos2(400., 300.));
    assert_eq!((second.context_actions(&ctx) >> 16) & (8 | 16), 0);
    ctx.push_event(crate::Event::CanvasContent(CanvasPasteContent::Svg(DRAWING.into())));
    frame(&ctx, vec![], |ui| {
        process_host_results(&ctx, Some(&mut second));
        second.handle_clip_input(ui);
    });
    assert!(first.buffer.elements.is_empty() && second.buffer.elements.is_empty());
    assert!(!import_pending(&ctx));
    // Returning to the original document cannot revive its old popup/import.
    assert!(!first.context_menu.quick && first.context_menu.pending_insert.is_none());
    assert_ne!((second.context_actions(&ctx) >> 16) & 8, 0);
}

#[test]
fn import_cancellation_and_results_without_a_current_svg_release_busy_state() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    open(&ctx, &mut svg, pos2(400., 300.));
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::Paste as u32));
    assert_eq!(take_host_command(&ctx), 8);
    svg.deactivate_context_menu(&ctx);
    ctx.push_event(crate::Event::CanvasContent(CanvasPasteContent::Image(vec![])));
    process_host_results(&ctx, None);
    assert!(!import_pending(&ctx));
    // A stale native action does not operate on the next document.
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::SelectAll as u32));
    assert!(svg.toolbar.selection.selected_elements.is_empty());
}

#[test]
fn imported_svg_uses_saved_document_anchor_after_pan_zoom_and_is_undoable() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    let anchor = pos2(200., 150.);
    open(&ctx, &mut svg, anchor);
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::Paste as u32));
    assert_eq!(take_host_command(&ctx), 8);
    svg.viewport_settings.master_transform = Transform::from_row(2., 0., 0., 2., 90., 80.);
    ctx.push_event(crate::Event::CanvasContent(CanvasPasteContent::Svg(DRAWING.into())));
    frame(&ctx, vec![], |ui| {
        process_host_results(&ctx, Some(&mut svg));
        svg.handle_clip_input(ui);
    });
    let bounds = svg
        .buffer
        .elements
        .values()
        .fold(Rect::NOTHING, |bounds, el| bounds.union(el.bounding_box()));
    assert!(bounds.center().distance(pos2(490., 380.)) < 0.01);
    svg.history.undo(&mut svg.buffer);
    assert!(svg.buffer.elements.values().all(|e| e.deleted()));
    svg.history.redo(&mut svg.buffer);
    assert!(svg.buffer.elements.values().all(|e| !e.deleted()));
}

#[test]
fn desktop_menu_and_advanced_popover_stay_inside_canvas() {
    let ctx = context();
    let mut svg = editor(&ctx, DRAWING);
    open(&ctx, &mut svg, pos2(790., 590.));
    for _ in 0..3 {
        frame(&ctx, vec![], |ui| svg.show_context_menu(ui));
    }
    assert!(
        svg.viewport_settings
            .container_rect
            .contains_rect(svg.context_menu.rect.unwrap()),
        "rect {:?}",
        svg.context_menu.rect
    );
    open(&ctx, &mut svg, pos2(130., 130.));
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::More as u32));
    for _ in 0..3 {
        frame(&ctx, vec![], |ui| svg.show_context_menu(ui));
    }
    assert!(
        svg.viewport_settings
            .container_rect
            .contains_rect(svg.context_menu.rect.unwrap()),
        "rect {:?}",
        svg.context_menu.rect
    );
}

#[test]
fn deactivation_cancels_commands_before_the_host_takes_them() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    open(&ctx, &mut svg, pos2(400., 300.));
    frame(&ctx, vec![], |ui| svg.context_action(ui, CanvasAction::ImportImage as u32));
    assert!(import_pending(&ctx));
    svg.deactivate_context_menu(&ctx);
    assert_eq!(take_host_command(&ctx), 0);
    assert!(!import_pending(&ctx));
}

#[test]
fn disabling_canvas_closes_menu_and_does_not_commit_a_gesture() {
    let ctx = context();
    let mut svg = editor(&ctx, EMPTY);
    open(&ctx, &mut svg, pos2(400., 300.));
    frame(&ctx, vec![], |ui| {
        ui.add_enabled_ui(false, |ui| svg.process_events(ui));
    });
    assert!(!svg.context_menu.quick);
    assert!(!svg.history.has_undo());
    assert!(svg.buffer.elements.values().all(|e| e.deleted()));
}

#[test]
fn replacing_an_editor_dismisses_its_native_presentation() {
    let ctx = context();
    let mut first = editor(&ctx, EMPTY);
    ctx.memory_mut(|m| {
        m.data
            .insert_temp(Id::unique("canvas_native_menu_owner"), first.context_menu.owner)
    });
    first.context_menu.native = true;
    let mut replacement = editor(&ctx, EMPTY);
    process_host_results(&ctx, Some(&mut replacement));
    assert!(matches!(
        ctx.pop_context_menu(),
        Some((_, ContextMenuTarget::Canvas(DISMISS_NATIVE_MENU)))
    ));
    assert!(!replacement.context_menu.native);
}
