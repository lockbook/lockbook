use crate::{
    CPoint, CRect, CTextGranularity, CTextLayoutDirection, CTextPosition, CTextRange,
    UITextSelectionRects, WgpuWorkspace,
};
use egui::{Event, Key, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use egui_editor::input::canonical::{Bound, Increment, Location, Modification, Offset, Region};
use egui_editor::input::cursor::Cursor;
use egui_editor::input::mutation;
use egui_editor::offset_types::{DocCharOffset, RangeExt};
use std::cmp;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::null;
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::EventManager as _;
use workspace_rs::tab::TabContent;

use super::keyboard::UIKeys;

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
            .push_markdown_event(Modification::Newline { advance_cursor: true });
    } else if content == "\t" {
        obj.context
            .push_markdown_event(Modification::Indent { deindent: false });
    } else {
        obj.raw_input.events.push(Event::Text(content));
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn backspace(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.raw_input.events.push(Event::Key {
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
    let text = CStr::from_ptr(text).to_str().unwrap().into();
    if !range.none {
        obj.context
            .push_markdown_event(Modification::Replace { region: range.into(), text });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn copy_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cut_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::Cut);
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

    let (start, end): (DocCharOffset, DocCharOffset) =
        (range.start.pos.into(), range.end.pos.into());
    let cursor: Cursor = (start, end).into();
    let buffer = &markdown.editor.buffer.current;
    let text = cursor.selection_text(buffer);

    CString::new(text)
        .expect("Could not Rust String -> C String")
        .into_raw()
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

    let (start, end) = markdown.editor.buffer.current.cursor.selection;

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
    if !range.none {
        obj.context
            .push_markdown_event(Modification::Select { region: range.into() });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_current_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::Select {
        region: Region::Bound { bound: Bound::Word, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_all(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::Select {
        region: Region::Bound { bound: Bound::Doc, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
#[no_mangle]
pub unsafe extern "C" fn get_marked(obj: *mut c_void) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextRange::default(),
    };

    match markdown.editor.buffer.current.cursor.mark {
        None => CTextRange { none: true, ..Default::default() },
        Some((start, end)) => CTextRange {
            none: false,
            start: CTextPosition { pos: start.0, ..Default::default() },
            end: CTextPosition { pos: end.0, ..Default::default() },
        },
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614465-setmarkedtext
#[no_mangle]
pub unsafe extern "C" fn set_marked(obj: *mut c_void, range: CTextRange, text: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let text =
        if text.is_null() { None } else { Some(CStr::from_ptr(text).to_str().unwrap().into()) };

    obj.context.push_markdown_event(Modification::StageMarked {
        highlighted: range.into(),
        text: text.unwrap_or_default(),
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614512-unmarktext
#[no_mangle]
pub unsafe extern "C" fn unmark_text(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::CommitMarked);
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

    CTextPosition {
        pos: markdown.editor.buffer.current.segs.last_cursor_position().0,
        ..Default::default()
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force: Some(force),
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn touches_moved(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force: Some(force),
    });

    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn touches_ended(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force: Some(force),
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });

    obj.raw_input.events.push(Event::PointerGone);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn touches_cancelled(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force: Some(force),
    });

    obj.raw_input.events.push(Event::PointerGone);
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
    obj: *mut c_void, mut start: CTextPosition, offset: i32,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return CTextPosition::default(),
    };

    let buffer = &markdown.editor.buffer.current;

    if (offset < 0 && -offset > start.pos as i32)
        || (offset > 0
            && (start.pos).saturating_add(offset as usize) > buffer.segs.last_cursor_position().0)
    {
        CTextPosition {
            pos: markdown.editor.buffer.current.segs.last_cursor_position().0,
            ..Default::default()
        }
    } else {
        start.pos += offset as usize;
        start
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

    let buffer = &markdown.editor.buffer.current;
    let galleys = &markdown.editor.galleys;

    let offset_type =
        if matches!(direction, CTextLayoutDirection::Right | CTextLayoutDirection::Left) {
            Offset::Next(Bound::Char)
        } else {
            Offset::By(Increment::Line)
        };
    let backwards = matches!(direction, CTextLayoutDirection::Left | CTextLayoutDirection::Up);

    let mut cursor: Cursor = start.pos.into();
    for _ in 0..offset {
        cursor.advance(offset_type, backwards, buffer, galleys, &markdown.editor.bounds);
    }
    CTextPosition { none: start.none, pos: cursor.selection.1 .0 }
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

    let bound: Bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    if let Some(range) =
        DocCharOffset(pos.pos).range_bound(bound, backwards, false, &markdown.editor.bounds)
    {
        // forwards: the provided position is at the end of the enclosing range
        // backwards: the provided position is at the start of the enclosing range
        if !backwards && pos.pos == range.end() || backwards && pos.pos == range.start() {
            return true;
        }
    }
    false
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

    let pos: DocCharOffset = pos.pos.into();

    let bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    if let Some(range) = pos.range_bound(bound, backwards, false, &markdown.editor.bounds) {
        // this implementation doesn't meet the specification in apple's docs, but the implementation that does creates word jumping bugs
        if range.contains_inclusive(pos) {
            return true;
        }
    }
    false
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

    let buffer = &markdown.editor.buffer.current;
    let galleys = &markdown.editor.galleys;

    let mut cursor: Cursor = pos.pos.into();
    let bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    cursor.advance(Offset::Next(bound), backwards, buffer, galleys, &markdown.editor.bounds);

    CTextPosition { none: false, pos: cursor.selection.1 .0 }
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

    let buffer = &markdown.editor.buffer.current;
    let galleys = &markdown.editor.galleys;

    let bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    let cursor = mutation::region_to_cursor(
        Region::BoundAt { bound, location: Location::DocCharOffset(pos.pos.into()), backwards },
        buffer.cursor,
        buffer,
        galleys,
        &markdown.editor.bounds,
    );

    CTextRange {
        none: false,
        start: CTextPosition { none: false, pos: cursor.selection.start().0 },
        end: CTextPosition { none: false, pos: cursor.selection.end().0 },
    }
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

    let buffer = &markdown.editor.buffer.current;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;
    let appearance = &markdown.editor.appearance;

    let cursor_representing_rect: Cursor = {
        let range: (DocCharOffset, DocCharOffset) = range.into();
        let selection_start = range.start();
        let selection_end = range.end();
        let mut cursor: Cursor = selection_start.into();
        cursor.advance(Offset::To(Bound::Line), false, buffer, galleys, &markdown.editor.bounds);
        let end_of_selection_start_line = cursor.selection.1;
        let end_of_rect = cmp::min(selection_end, end_of_selection_start_line);
        (selection_start, end_of_rect).into()
    };

    let start_line = cursor_representing_rect.start_line(galleys, text, appearance);
    let end_line = cursor_representing_rect.end_line(galleys, text, appearance);
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
    obj.context.push_markdown_event(Modification::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_copy(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context.push_markdown_event(Modification::Copy);
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

    let cursor: Cursor = pos.pos.into();
    let line = cursor.start_line(galleys, text, appearance);

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

    let buffer = &markdown.editor.buffer.current;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;

    let range: (DocCharOffset, DocCharOffset) = range.into();
    let mut cont_start = range.start();

    let mut selection_rects = vec![];

    while cont_start < range.end() {
        let mut new_end: Cursor = cont_start.into();
        new_end.advance(Offset::To(Bound::Line), false, buffer, galleys, &markdown.editor.bounds);
        let end_of_rect = cmp::min(new_end.selection.end(), range.end());

        let cursor_representing_rect: Cursor = (cont_start, end_of_rect).into();

        let start_line =
            cursor_representing_rect.start_line(galleys, text, &markdown.editor.appearance);
        let end_line =
            cursor_representing_rect.end_line(galleys, text, &markdown.editor.appearance);

        selection_rects.push(CRect {
            min_x: (start_line[1].x) as f64,
            min_y: start_line[0].y as f64,
            max_x: end_line[1].x as f64,
            max_y: end_line[1].y as f64,
        });

        new_end.advance(Offset::Next(Bound::Char), false, buffer, galleys, &markdown.editor.bounds);
        cont_start = new_end.selection.end();
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
pub unsafe extern "C" fn indent_at_cursor(obj: *mut c_void, deindent: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.context
        .push_markdown_event(Modification::Indent { deindent });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn undo_redo(obj: *mut c_void, redo: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if redo {
        obj.context.push_markdown_event(Modification::Redo);
    } else {
        obj.context.push_markdown_event(Modification::Undo);
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

    !markdown.editor.buffer.undo_queue.is_empty()
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

    !markdown.editor.buffer.redo_stack.is_empty()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn delete_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.raw_input.events.push(Event::Key {
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
        obj.raw_input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}
