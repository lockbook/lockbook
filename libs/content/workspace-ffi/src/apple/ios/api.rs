use egui::{Key, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use lb_c::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt, RelCharOffset};
use std::cmp;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::null;
use tracing::instrument;
use workspace_rs::tab::markdown_editor::bounds::RangesExt;
use workspace_rs::tab::markdown_editor::input::advance::AdvanceExt as _;
use workspace_rs::tab::markdown_editor::input::{
    Bound, Event, Increment, Offset, Region, mutation,
};
use workspace_rs::tab::markdown_editor::output::ui_text_input_tokenizer::UITextInputTokenizer as _;
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::{ContentState, ExtendedInput as _, TabContent};

use super::super::response::*;
use super::response::*;
use crate::WgpuWorkspace;
use crate::apple::keyboard::UIKeys;

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).renderer.context.frame_nr()))]
pub unsafe extern "C" fn ios_frame(obj: *mut c_void) -> IOSResponse {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.frame().into()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn insert_text(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content = CStr::from_ptr(content).to_str().unwrap().into();

    if content == "\n" {
        obj.renderer
            .context
            .push_markdown_event(Event::Newline { shift: false });
    } else if content == "\t" {
        obj.renderer
            .context
            .push_markdown_event(Event::Indent { deindent: false });
    } else {
        obj.renderer
            .raw_input
            .events
            .push(egui::Event::Text(content));
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn backspace(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.renderer.raw_input.events.push(egui::Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614457-hastext
#[no_mangle]
pub unsafe extern "C" fn has_text(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    !markdown.buffer.is_empty()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614558-replace
#[no_mangle]
pub unsafe extern "C" fn replace_text(obj: *mut c_void, range: CTextRange, text: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let text: String = CStr::from_ptr(text).to_str().unwrap().into();

    let region: Option<Region> = range.into();
    if let Some(region) = region {
        obj.renderer.context.push_markdown_event(Event::Replace {
            region,
            text,
            advance_cursor: true,
        });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn copy_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cut_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614527-text
#[no_mangle]
pub unsafe extern "C" fn text_in_range(obj: *mut c_void, range: CTextRange) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return null(),
    };

    let range: Option<(DocCharOffset, DocCharOffset)> = range.into();
    if let Some(range) = range {
        CString::new(&markdown.buffer[range])
            .expect("Could not Rust String -> C String")
            .into_raw()
    } else {
        println!("warning: text_in_range() called with nil range");
        CString::new("")
            .expect("Could not Rust String -> C String")
            .into_raw()
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614541-selectedtextrange
#[no_mangle]
pub unsafe extern "C" fn get_selected(obj: *mut c_void) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextRange::default(),
    };

    CTextRange {
        none: false,
        start: CTextPosition { pos: markdown.buffer.current.selection.start().0, none: false },
        end: CTextPosition { pos: markdown.buffer.current.selection.end().0, none: false },
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614541-selectedtextrange
#[no_mangle]
pub unsafe extern "C" fn set_selected(obj: *mut c_void, range: CTextRange) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if let Some(region) = range.into() {
        obj.renderer
            .context
            .push_markdown_event(Event::Select { region });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_current_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Select {
        region: Region::Bound { bound: Bound::Word, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_all(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Select {
        region: Region::Bound { bound: Bound::Doc, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
#[no_mangle]
pub unsafe extern "C" fn get_marked(_obj: *mut c_void) -> CTextRange {
    // I wanted to put `unimplemented!()` but this function is occasionally called. If I return a `CTextRange` for
    // (0, 0), iOS opens a context menu on every tap toward the top of the screen. This value, which was a lucky guess,
    // prevents that from happening.
    CTextRange::default()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614465-setmarkedtext
#[no_mangle]
pub unsafe extern "C" fn set_marked(_obj: *mut c_void, _range: CTextRange, _text: *const c_char) {
    unimplemented!()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614512-unmarktext
#[no_mangle]
pub unsafe extern "C" fn unmark_text(_obj: *mut c_void) {
    unimplemented!()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
/// isn't this always just going to be 0?
/// should we be returning a subset of the document? https://stackoverflow.com/questions/12676851/uitextinput-is-it-ok-to-return-incorrect-beginningofdocument-endofdocumen
#[no_mangle]
pub unsafe extern "C" fn beginning_of_document(_obj: *mut c_void) -> CTextPosition {
    DocCharOffset(0).into()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
/// should we be returning a subset of the document? https://stackoverflow.com/questions/12676851/uitextinput-is-it-ok-to-return-incorrect-beginningofdocument-endofdocumen
#[no_mangle]
pub unsafe extern "C" fn end_of_document(obj: *mut c_void) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    markdown.buffer.current.segs.last_cursor_position().into()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).renderer.context.frame_nr()))]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerButton {
            pos: Pos2 { x, y },
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).renderer.context.frame_nr()))]
pub unsafe extern "C" fn touches_moved(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerMoved(Pos2 { x, y }));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).renderer.context.frame_nr()))]
pub unsafe extern "C" fn touches_ended(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerButton {
            pos: Pos2 { x, y },
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });

    obj.renderer.raw_input.events.push(egui::Event::PointerGone);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).renderer.context.frame_nr()))]
pub unsafe extern "C" fn touches_cancelled(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let force = if force == 0.0 { None } else { Some(force) };

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force,
    });

    obj.renderer.raw_input.events.push(egui::Event::PointerGone);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn touches_predicted(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let force = if force == 0.0 { None } else { Some(force) };

    obj.renderer
        .context
        .push_event(workspace_rs::Event::PredictedTouch {
            id: TouchId(id),
            force,
            pos: Pos2 { x, y },
        });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn tab_count(obj: *mut c_void) -> i64 {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.workspace.tabs.len() as i64
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn canvas_detect_islands_interaction(
    obj: *mut c_void, x: f32, y: f32,
) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let mut has_islands_interaction = false;
    if let Some(tab) = obj.workspace.current_tab() {
        if let ContentState::Open(TabContent::Svg(svg)) = &tab.content {
            has_islands_interaction = svg.detect_islands_interaction(egui::pos2(x, y));
        }
    }
    has_islands_interaction
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn pan(obj: *mut c_void, scroll_x: f32, scroll_y: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer
        .context
        .push_event(workspace_rs::Event::KineticPan { x: scroll_x, y: scroll_y });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn zoom(obj: *mut c_void, scale: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.renderer.raw_input.events.push(egui::Event::Zoom(scale));
}

/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub extern "C" fn text_range(start: CTextPosition, end: CTextPosition) -> CTextRange {
    if start.pos < end.pos {
        CTextRange { none: false, start, end }
    } else {
        CTextRange { none: false, start: end, end: start }
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn position_offset(
    obj: *mut c_void, start: CTextPosition, offset: i32,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    let start: Option<DocCharOffset> = start.into();
    if let Some(start) = start {
        let last_cursor_position = markdown.buffer.current.segs.last_cursor_position();

        let result = if offset < 0 && -offset > start.0 as i32 {
            DocCharOffset::default()
        } else if offset > 0 && (start.0).saturating_add(offset as usize) > last_cursor_position.0 {
            last_cursor_position
        } else {
            start + RelCharOffset(offset as _)
        };

        result.into()
    } else {
        println!("warning: position_offset() called with nil start position");

        CTextPosition::default()
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn position_offset_in_direction(
    obj: *mut c_void, start: CTextPosition, direction: CTextLayoutDirection, offset: i32,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    let segs = &markdown.buffer.current.segs;
    let galleys = &markdown.galleys;

    let offset_type =
        if matches!(direction, CTextLayoutDirection::Right | CTextLayoutDirection::Left) {
            Offset::Next(Bound::Char)
        } else {
            Offset::By(Increment::Lines(1))
        };
    let backwards = matches!(direction, CTextLayoutDirection::Left | CTextLayoutDirection::Up);

    let mut result: DocCharOffset = start.pos.into();
    for _ in 0..offset {
        result = result.advance(&mut None, offset_type, backwards, segs, galleys, &markdown.bounds);
    }

    CTextPosition { none: start.none, pos: result.0 }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614553-isposition
#[no_mangle]
pub unsafe extern "C" fn is_position_at_bound(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    let text_position = pos.pos.into();
    let at_boundary = granularity.into();

    markdown.is_position_at_boundary(text_position, at_boundary, backwards)
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614491-isposition
#[no_mangle]
pub unsafe extern "C" fn is_position_within_bound(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    let text_position = pos.pos.into();
    let at_boundary = granularity.into();

    markdown.is_position_within_text_unit(text_position, at_boundary, backwards)
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614513-position
#[no_mangle]
pub unsafe extern "C" fn bound_from_position(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    let text_position = pos.pos.into();
    let to_boundary = granularity.into();

    markdown
        .position_from(text_position, to_boundary, backwards)
        .into()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614464-rangeenclosingposition
#[no_mangle]
pub unsafe extern "C" fn bound_at_position(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextRange::default(),
    };

    let text_position = pos.pos.into();
    let with_granularity = granularity.into();

    let result = markdown.range_enclosing_position(text_position, with_granularity, backwards);

    result.into()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614570-firstrect
#[no_mangle]
pub unsafe extern "C" fn first_rect(obj: *mut c_void, range: CTextRange) -> CRect {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CRect::default(),
    };

    let segs = &markdown.buffer.current.segs;
    let galleys = &markdown.galleys;

    let selection_representing_rect = {
        let range: Option<(DocCharOffset, DocCharOffset)> = range.into();
        let range = match range {
            Some(range) => range,
            None => {
                println!("warning: first_rect() called with nil range");
                return CRect::default();
            }
        };
        let mut selection_start = range.start();
        let selection_end = range.end();
        selection_start = selection_start.advance(
            &mut None,
            Offset::To(Bound::Line),
            false,
            segs,
            galleys,
            &markdown.bounds,
        );
        let end_of_selection_start_line = selection_start;
        let end_of_rect = cmp::min(selection_end, end_of_selection_start_line);
        (selection_start, end_of_rect)
    };

    let start_line = markdown.cursor_line(selection_representing_rect.start());
    let end_line = markdown.cursor_line(selection_representing_rect.end());

    CRect {
        min_x: (start_line[1].x + 1.0) as f64,
        min_y: start_line[0].y as f64,
        max_x: end_line[0].x as f64,
        max_y: end_line[1].y as f64,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_cut(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_copy(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.context.push_markdown_event(Event::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn position_at_point(obj: *mut c_void, point: CPoint) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    let galleys = &markdown.galleys;

    let offset =
        mutation::pos_to_char_offset(Pos2 { x: point.x as f32, y: point.y as f32 }, galleys);

    CTextPosition { none: false, pos: offset.0 }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn get_text(obj: *mut c_void) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return null(),
    };

    let value = markdown.buffer.current.text.as_str();

    CString::new(value)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cursor_rect_at_position(obj: *mut c_void, pos: CTextPosition) -> CRect {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CRect::default(),
    };

    let line = markdown.cursor_line(pos.pos.into());

    CRect {
        min_x: line[0].x as f64,
        min_y: line[0].y as f64,
        max_x: line[1].x as f64,
        max_y: line[1].y as f64,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn update_virtual_keyboard(obj: *mut c_void, showing: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return,
    };

    markdown.virtual_keyboard_shown = showing;
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn selection_rects(
    obj: *mut c_void, range: CTextRange,
) -> UITextSelectionRects {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return UITextSelectionRects::default(),
    };

    let bounds = &markdown.bounds;

    let range: Option<(DocCharOffset, DocCharOffset)> = range.into();
    let range = match range {
        Some(range) => range,
        None => {
            println!("warning: selection_rects() called with nil range");
            return UITextSelectionRects::default();
        }
    };

    let mut selection_rects = vec![];

    let lines = bounds.wrap_lines.find_intersecting(range, false);
    for line in lines.iter() {
        let mut line = bounds.wrap_lines[line];
        if line.0 < range.start() {
            line.0 = range.start();
        }
        if line.1 > range.end() {
            line.1 = range.end();
        }
        if line.is_empty() {
            continue;
        }

        let start_line = markdown.cursor_line(line.0);
        let end_line = markdown.cursor_line(line.1);
        selection_rects.push(CRect {
            min_x: (start_line[1].x) as f64,
            min_y: start_line[0].y as f64,
            max_x: end_line[1].x as f64,
            max_y: end_line[1].y as f64,
        });
    }

    UITextSelectionRects {
        size: selection_rects.len() as i32,
        rects: Box::into_raw(selection_rects.into_boxed_slice()) as *const CRect,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn free_selection_rects(rects: UITextSelectionRects) {
    let _ = Box::from_raw(std::slice::from_raw_parts_mut(
        rects.rects as *mut CRect,
        rects.size as usize,
    ));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn get_tabs_ids(obj: *mut c_void) -> TabsIds {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let ids: Vec<CUuid> = obj
        .workspace
        .tabs
        .iter()
        .flat_map(|tab| tab.id())
        .map(|id| id.into())
        .collect();

    TabsIds { size: ids.len() as i32, ids: Box::into_raw(ids.into_boxed_slice()) as *const CUuid }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn free_tab_ids(ids: TabsIds) {
    let _ = Box::from_raw(std::slice::from_raw_parts_mut(ids.ids as *mut CUuid, ids.size as usize));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn indent_at_cursor(obj: *mut c_void, deindent: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer
        .context
        .push_markdown_event(Event::Indent { deindent });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn undo_redo(obj: *mut c_void, redo: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if redo {
        obj.renderer.context.push_markdown_event(Event::Redo);
    } else {
        obj.renderer.context.push_markdown_event(Event::Undo);
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn can_undo(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    !markdown.readonly && markdown.buffer.can_undo()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn can_redo(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    !markdown.readonly && markdown.buffer.can_redo()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn delete_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.renderer.raw_input.events.push(egui::Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn current_tab(obj: *mut c_void) -> i64 {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    match obj.workspace.current_tab() {
        Some(tab) => match &tab.content {
            ContentState::Open(tab) => match tab {
                TabContent::Image(_) => 2,
                TabContent::Markdown(_) => 3,
                // TabContent::PlainText(_) => 4,
                TabContent::Pdf(_) => 5,
                TabContent::Svg(_) => 6,
                TabContent::MindMap(_) => 7,
                TabContent::SpaceInspector(_) => 8,
            },
            _ => 1,
        },
        None => 0,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn is_current_tab_editable(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.workspace
        .current_tab()
        .map(|tab| !tab.read_only)
        .unwrap_or(true)
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn toggle_drawing_tool(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        svg.toolbar.set_tool(
            svg.toolbar.previous_tool.unwrap_or(Tool::Pen),
            &mut svg.settings,
            &mut svg.cfg,
        );
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn show_tool_popover_at_cursor(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        svg.toolbar.toggle_at_cursor_tool_popover();
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn toggle_drawing_tool_between_eraser(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        svg.toolbar
            .toggle_tool_between_eraser(&mut svg.settings, &mut svg.cfg)
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn set_pencil_only_drawing(obj: *mut c_void, val: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.workspace.tabs.iter_mut().for_each(|t| {
        if let ContentState::Open(TabContent::Svg(svg)) = &mut t.content {
            svg.settings.pencil_only_drawing = val
        }
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn unfocus_title(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(tab) = obj.workspace.current_tab_mut() {
        tab.rename = None;
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn show_hide_tabs(obj: *mut c_void, show: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.workspace.show_tabs = show;
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn close_active_tab(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if !obj.workspace.tabs.is_empty() {
        obj.workspace.close_tab(obj.workspace.current_tab)
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn close_all_tabs(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    for i in 0..obj.workspace.tabs.len() {
        obj.workspace.close_tab(i);
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn ios_key_event(
    obj: *mut c_void, key_code: isize, shift: bool, ctrl: bool, option: bool, command: bool,
    pressed: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };

    obj.renderer.raw_input.modifiers = modifiers;

    let Some(key) = UIKeys::from(key_code) else { return };

    // Event::Key
    if let Some(key) = key.egui_key() {
        obj.renderer.raw_input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn set_ws_inset(
    obj: *mut c_void, inset: f32,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.renderer.bottom_inset = Some(inset as u32);
}
