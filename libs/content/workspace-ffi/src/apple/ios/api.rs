use egui::{Key, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use lb_external_interface::lb_rs::text::offset_types::{
    DocCharOffset, RangeExt as _, RelCharOffset,
};
use std::cmp;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::null;
use tracing::instrument;
use workspace_rs::tab::markdown_editor::input::advance::AdvanceExt as _;
use workspace_rs::tab::markdown_editor::input::{cursor, mutation};
use workspace_rs::tab::markdown_editor::input::{Bound, Event, Increment, Offset, Region};
use workspace_rs::tab::markdown_editor::output::ui_text_input_tokenizer::UITextInputTokenizer as _;
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::ExtendedInput as _;
use workspace_rs::tab::TabContent;

use super::super::response::*;
use super::response::*;
use crate::apple::keyboard::UIKeys;
use crate::WgpuWorkspace;

#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
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
        obj.context
            .push_markdown_event(Event::Newline { advance_cursor: true });
    } else if content == "\t" {
        obj.context
            .push_markdown_event(Event::Indent { deindent: false });
    } else {
        obj.raw_input.events.push(egui::Event::Text(content));
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn backspace(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.raw_input.events.push(egui::Event::Key {
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

    !markdown.editor.buffer.is_empty()
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
        obj.context
            .push_markdown_event(Event::Replace { region, text });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn copy_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Event::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cut_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Event::Cut);
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
        CString::new(&markdown.editor.buffer[range])
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

    let (start, end) = markdown.editor.buffer.current.selection;

    CTextRange {
        none: false,
        start: CTextPosition { pos: start.0, ..Default::default() },
        end: CTextPosition { pos: end.0, ..Default::default() },
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
        obj.context.push_markdown_event(Event::Select { region });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_current_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Event::Select {
        region: Region::Bound { bound: Bound::Word, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_all(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Event::Select {
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
    CTextPosition { ..Default::default() }
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

    let result = markdown.editor.buffer.current.segs.last_cursor_position().0;
    CTextPosition { pos: result, ..Default::default() }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force,
    });

    obj.raw_input.events.push(egui::Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
pub unsafe extern "C" fn touches_moved(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force,
    });

    obj.raw_input
        .events
        .push(egui::Event::PointerMoved(Pos2 { x, y }));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
pub unsafe extern "C" fn touches_ended(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force,
    });

    obj.raw_input.events.push(egui::Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });

    obj.raw_input.events.push(egui::Event::PointerGone);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
pub unsafe extern "C" fn touches_cancelled(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let force = if force == 0.0 { None } else { Some(force) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force,
    });

    obj.raw_input.events.push(egui::Event::PointerGone);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_gone(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(egui::Event::PointerGone);
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
        let last_cursor_position = markdown.editor.buffer.current.segs.last_cursor_position();

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

    let segs = &markdown.editor.buffer.current.segs;
    let galleys = &markdown.editor.galleys;

    let offset_type =
        if matches!(direction, CTextLayoutDirection::Right | CTextLayoutDirection::Left) {
            Offset::Next(Bound::Char)
        } else {
            Offset::By(Increment::Line)
        };
    let backwards = matches!(direction, CTextLayoutDirection::Left | CTextLayoutDirection::Up);

    let mut result: DocCharOffset = start.pos.into();
    for _ in 0..offset {
        result = result.advance(
            &mut None,
            offset_type,
            backwards,
            segs,
            galleys,
            &markdown.editor.bounds,
        );
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

    markdown
        .editor
        .bounds
        .is_position_at_boundary(text_position, at_boundary, backwards)
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

    markdown
        .editor
        .bounds
        .is_position_within_text_unit(text_position, at_boundary, backwards)
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
        .editor
        .bounds
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

    let result =
        markdown
            .editor
            .bounds
            .range_enclosing_position(text_position, with_granularity, backwards);

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

    let segs = &markdown.editor.buffer.current.segs;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;
    let appearance = &markdown.editor.appearance;

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
            &markdown.editor.bounds,
        );
        let end_of_selection_start_line = selection_start;
        let end_of_rect = cmp::min(selection_end, end_of_selection_start_line);
        (selection_start, end_of_rect)
    };

    let start_line = cursor::line(selection_representing_rect.start(), galleys, text, appearance);
    let end_line = cursor::line(selection_representing_rect.end(), galleys, text, appearance);

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
    obj.context.push_markdown_event(Event::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_copy(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Event::Copy);
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

    let segs = &markdown.editor.buffer.current.segs;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;

    let offset = mutation::pos_to_char_offset(
        Pos2 { x: point.x as f32, y: point.y as f32 },
        galleys,
        segs,
        text,
    );

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

    let value = markdown.editor.buffer.current.text.as_str();

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

    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;
    let appearance = &markdown.editor.appearance;

    let line = cursor::line(pos.pos.into(), galleys, text, appearance);

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

    markdown.editor.is_virtual_keyboard_showing = showing;
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

    let segs = &markdown.editor.buffer.current.segs;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;
    let appearance = &markdown.editor.appearance;

    let range: Option<(DocCharOffset, DocCharOffset)> = range.into();
    let range = match range {
        Some(range) => range,
        None => {
            println!("warning: selection_rects() called with nil range");
            return UITextSelectionRects::default();
        }
    };
    let mut cont_start = range.start();

    let mut selection_rects = vec![];

    while cont_start < range.end() {
        let mut new_end = cont_start;
        new_end = new_end.advance(
            &mut None,
            Offset::Next(Bound::Line),
            false,
            segs,
            galleys,
            &markdown.editor.bounds,
        );
        let end_of_rect = cmp::min(new_end, range.end());

        let selection_representing_rect = (cont_start, end_of_rect);

        let start_line =
            cursor::line(selection_representing_rect.start(), galleys, text, appearance);
        let end_line = cursor::line(selection_representing_rect.end(), galleys, text, appearance);

        selection_rects.push(CRect {
            min_x: (start_line[1].x) as f64,
            min_y: start_line[0].y as f64,
            max_x: end_line[1].x as f64,
            max_y: end_line[1].y as f64,
        });

        new_end.advance(
            &mut None,
            Offset::Next(Bound::Char),
            false,
            segs,
            galleys,
            &markdown.editor.bounds,
        );
        cont_start = new_end;
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
    let ids: Vec<CUuid> = obj.workspace.tabs.iter().map(|tab| tab.id.into()).collect();

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
    obj.context.push_markdown_event(Event::Indent { deindent });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn undo_redo(obj: *mut c_void, redo: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if redo {
        obj.context.push_markdown_event(Event::Redo);
    } else {
        obj.context.push_markdown_event(Event::Undo);
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

    markdown.editor.buffer.can_undo()
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

    markdown.editor.buffer.can_redo()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn delete_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.raw_input.events.push(egui::Event::Key {
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
            Some(tab) => match tab {
                TabContent::Image(_) => 2,
                TabContent::Markdown(_) => 3,
                // TabContent::PlainText(_) => 4,
                TabContent::Pdf(_) => 5,
                TabContent::Svg(_) => 6,
                TabContent::MergeMarkdown { .. } => unreachable!(),
            },
            None => 1,
        },
        None => 0,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn toggle_drawing_tool(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        svg.toolbar
            .set_tool(svg.toolbar.previous_tool.unwrap_or(Tool::Pen));
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn toggle_drawing_tool_between_eraser(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        svg.toolbar.toggle_tool_between_eraser()
    }
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
        obj.workspace.close_tab(obj.workspace.active_tab)
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn close_all_tabs(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    while !obj.workspace.tabs.is_empty() {
        obj.workspace.close_tab(obj.workspace.tabs.len() - 1);
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

    obj.raw_input.modifiers = modifiers;

    let Some(key) = UIKeys::from(key_code) else { return };

    // Event::Key
    if let Some(key) = key.egui_key() {
        obj.raw_input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}
