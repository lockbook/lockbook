use crate::input::canonical::{Bound, Increment, Location, Modification, Offset, Region};
use crate::input::cursor::Cursor;
use crate::input::mutation;
use crate::offset_types::{DocCharOffset, RangeExt};
use crate::{
    CPoint, CRect, CTextGranularity, CTextLayoutDirection, CTextPosition, CTextRange, WgpuEditor,
};
use egui::{Event, Key, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use std::cmp;
use std::ffi::{c_char, c_void, CStr, CString};

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn insert_text(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuEditor);
    let content = CStr::from_ptr(content).to_str().unwrap().into();

    if content == "\n" {
        obj.editor
            .custom_events
            .push(Modification::Newline { advance_cursor: true });
    } else {
        obj.raw_input.events.push(Event::Text(content))
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn backspace(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::Key {
        key: Key::Backspace,
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
    let obj = &mut *(obj as *mut WgpuEditor);
    !obj.editor.buffer.is_empty()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614558-replace
#[no_mangle]
pub unsafe extern "C" fn replace_text(obj: *mut c_void, range: CTextRange, text: *const c_char) {
    let obj = &mut *(obj as *mut WgpuEditor);
    let text = CStr::from_ptr(text).to_str().unwrap().into();

    if !range.none {
        obj.editor
            .custom_events
            .push(Modification::Replace { region: range.into(), text });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn copy_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);

    obj.editor.custom_events.push(Modification::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cut_selection(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);

    obj.editor.custom_events.push(Modification::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614527-text
#[no_mangle]
pub unsafe extern "C" fn text_in_range(obj: *mut c_void, range: CTextRange) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuEditor);

    let (start, end): (DocCharOffset, DocCharOffset) =
        (range.start.pos.into(), range.end.pos.into());
    let cursor: Cursor = (start, end).into();
    let buffer = &obj.editor.buffer.current;
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
    let obj = &mut *(obj as *mut WgpuEditor);
    let (start, end) = obj.editor.buffer.current.cursor.selection;

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
    let obj = &mut *(obj as *mut WgpuEditor);

    if !range.none {
        obj.editor
            .custom_events
            .push(Modification::Select { region: range.into() });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_current_word(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);

    obj.editor.custom_events.push(Modification::Select {
        region: Region::Bound { bound: Bound::Word, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn select_all(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);

    obj.editor.custom_events.push(Modification::Select {
        region: Region::Bound { bound: Bound::Doc, backwards: true },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
#[no_mangle]
pub unsafe extern "C" fn get_marked(obj: *mut c_void) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuEditor);
    match obj.editor.buffer.current.cursor.mark {
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
    let obj = &mut *(obj as *mut WgpuEditor);
    let text =
        if text.is_null() { None } else { Some(CStr::from_ptr(text).to_str().unwrap().into()) };

    obj.editor.custom_events.push(Modification::StageMarked {
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
    let obj = &mut *(obj as *mut WgpuEditor);

    obj.editor.custom_events.push(Modification::CommitMarked);
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
    let obj = &mut *(obj as *mut WgpuEditor);
    CTextPosition {
        pos: obj.editor.buffer.current.segs.last_cursor_position().0,
        ..Default::default()
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force,
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
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn touches_moved(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force,
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
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force,
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
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force,
    });

    obj.raw_input.events.push(Event::PointerGone);
}

/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub extern "C" fn text_range(start: CTextPosition, end: CTextPosition) -> CTextRange {
    CTextRange { none: false, start, end }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn position_offset(
    obj: *mut c_void, mut start: CTextPosition, offset: i32,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;

    if (offset < 0 && -offset > start.pos as i32)
        || (offset > 0
            && (start.pos).saturating_add(offset as usize) > buffer.segs.last_cursor_position().0)
    {
        CTextPosition { none: true, ..Default::default() }
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
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;
    let galleys = &obj.editor.galleys;

    let offset_type =
        if matches!(direction, CTextLayoutDirection::Right | CTextLayoutDirection::Left) {
            Offset::By(Increment::Char)
        } else {
            Offset::By(Increment::Line)
        };
    let backwards = matches!(direction, CTextLayoutDirection::Left | CTextLayoutDirection::Up);

    let mut cursor: Cursor = start.pos.into();
    for _ in 0..offset {
        cursor.advance(offset_type, backwards, buffer, galleys, &obj.editor.paragraphs);
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
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;
    let galleys = &obj.editor.galleys;

    // if advancing the cursor then advancing it back leaves it in the original position, it's at a bound
    let mut cursor: Cursor = pos.pos.into();
    let bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    cursor.advance(Offset::To(bound), !backwards, buffer, galleys, &obj.editor.paragraphs);
    cursor.advance(Offset::To(bound), backwards, buffer, galleys, &obj.editor.paragraphs);

    cursor.selection.1 == pos.pos
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614491-isposition
#[no_mangle]
pub unsafe extern "C" fn is_position_within_bound(
    _obj: *mut c_void, _pos: CTextPosition, _granularity: CTextGranularity, _backwards: bool,
) -> bool {
    true // aren't we always within a bound of each type?
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614491-isposition
#[no_mangle]
pub unsafe extern "C" fn bound_from_position(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;
    let galleys = &obj.editor.galleys;

    let mut cursor: Cursor = pos.pos.into();
    let bound = match granularity {
        CTextGranularity::Character => Bound::Char,
        CTextGranularity::Word => Bound::Word,
        CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
        CTextGranularity::Paragraph => Bound::Paragraph,
        CTextGranularity::Line => Bound::Line,
        CTextGranularity::Document => Bound::Doc,
    };
    cursor.advance(Offset::To(bound), backwards, buffer, galleys, &obj.editor.paragraphs);

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
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;
    let galleys = &obj.editor.galleys;

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
        &obj.editor.paragraphs,
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
    let obj = &mut *(obj as *mut WgpuEditor);
    let buffer = &obj.editor.buffer.current;
    let galleys = &obj.editor.galleys;

    let cursor_representing_rect: Cursor = {
        let range: (DocCharOffset, DocCharOffset) = range.into();
        let selection_start = range.start();
        let selection_end = range.end();
        let mut cursor: Cursor = selection_start.into();
        cursor.advance(Offset::To(Bound::Line), false, buffer, galleys, &obj.editor.paragraphs);
        let end_of_selection_start_line = cursor.selection.1;
        let end_of_rect = cmp::min(selection_end, end_of_selection_start_line);
        (selection_start, end_of_rect).into()
    };

    let start_rect = cursor_representing_rect.start_rect(galleys);
    let end_rect = cursor_representing_rect.end_rect(galleys);
    CRect {
        min_x: (start_rect.max.x + 1.0) as f64,
        min_y: start_rect.min.y as f64,
        max_x: end_rect.min.x as f64,
        max_y: end_rect.max.y as f64,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_cut(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.editor.custom_events.push(Modification::Cut);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_copy(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.editor.custom_events.push(Modification::Copy);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_paste(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuEditor);
    let clip = obj.from_host.clone().unwrap_or_default();
    obj.raw_input.events.push(Event::Paste(clip));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn position_at_point(obj: *mut c_void, point: CPoint) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuEditor);
    let segs = &obj.editor.buffer.current.segs;
    let galleys = &obj.editor.galleys;

    let scroll = obj.editor.scroll_area_offset;
    let offset = mutation::pos_to_char_offset(
        Pos2 { x: point.x as f32 + scroll.x, y: point.y as f32 + scroll.y },
        galleys,
        segs,
    );
    CTextPosition { none: false, pos: offset.0 }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cursor_rect_at_position(obj: *mut c_void, pos: CTextPosition) -> CRect {
    let obj = &mut *(obj as *mut WgpuEditor);
    let galleys = &obj.editor.galleys;

    let cursor: Cursor = pos.pos.into();
    let rect = cursor.start_rect(galleys);
    let scroll = obj.editor.scroll_area_offset;
    CRect {
        min_x: (rect.min.x - scroll.x) as f64,
        min_y: (rect.min.y - scroll.y) as f64,
        max_x: (rect.max.x - scroll.x) as f64,
        max_y: (rect.max.y - scroll.y) as f64,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn indent_at_cursor(obj: *mut c_void, deindent: bool) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.editor
        .custom_events
        .push(Modification::Indent { deindent });
}
