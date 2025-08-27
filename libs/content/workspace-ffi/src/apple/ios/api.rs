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

    println!(">>> Inserting text: {:?} <<<", content);

    if content == "\n" {
        obj.context
            .push_markdown_event(Event::Newline { shift: false });
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
        // let markdown = match obj.workspace.current_tab_markdown_mut() {
        //     Some(markdown) => markdown,
        //     None => return,
        // };
        // let range = markdown.region_to_range(region);
        // let replaced_text = &markdown.buffer.current.text[range.0.0..range.1.0].to_string();
        // println!(">>> Replacing text: {:?} with {:?} <<<", replaced_text, text);

        obj.context
            .push_markdown_event(Event::Replace { region, text, advance_cursor: true });
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
    println!(
        "✅ set_selected - INPUT: range={{ none: {}, start: {{ none: {}, pos: {} }}, end: {{ none: {}, pos: {} }} }}",
        range.none, range.start.none, range.start.pos, range.end.none, range.end.pos
    );

    let obj = &mut *(obj as *mut WgpuWorkspace);
    if let Some(region) = range.into() {
        println!("✅ set_selected - Converting to region: {:?}", region);
        obj.context.push_markdown_event(Event::Select { region });
        println!("✅ set_selected - Selection event pushed");
    } else {
        println!("✅ set_selected - Range conversion failed, no selection event pushed");
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
#[instrument(level="trace", skip(obj) fields(frame = (*(obj as *mut WgpuWorkspace)).context.frame_nr()))]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    println!("👆 touches_began - INPUT: id={}, pos=({}, {}), force={}", id, x, y, force);

    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    println!("👆 touches_began - Processed force: {:?}", force);

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force,
    });
    println!("👆 touches_began - Added Touch event (Start phase)");

    obj.raw_input.events.push(egui::Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    println!("👆 touches_began - Added PointerButton event (pressed=true)");
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
    println!("🔚 touches_ended - INPUT: id={}, pos=({}, {}), force={}", id, x, y, force);

    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    println!("🔚 touches_ended - Processed force: {:?}", force);

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force,
    });
    println!("🔚 touches_ended - Added Touch event (End phase)");

    obj.raw_input.events.push(egui::Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    println!("🔚 touches_ended - Added PointerButton event (pressed=false)");

    obj.raw_input.events.push(egui::Event::PointerGone);
    println!("🔚 touches_ended - Added PointerGone event");
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
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn touches_predicted(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let force = if force == 0.0 { None } else { Some(force) };

    obj.context.push_event(workspace_rs::Event::PredictedTouch {
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

/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub extern "C" fn text_range(start: CTextPosition, end: CTextPosition) -> CTextRange {
    if start.pos <= end.pos {
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
    println!(
        "📍 position_offset - INPUT: start={{ none: {}, pos: {} }}, offset: {}",
        start.none, start.pos, offset
    );

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            println!("📍 position_offset - ERROR: No current tab markdown, returning default");
            return CTextPosition::default();
        }
    };

    let start: Option<DocCharOffset> = start.into();
    println!("📍 position_offset - Converted start to DocCharOffset: {:?}", start);

    if let Some(start) = start {
        let last_cursor_position = markdown.buffer.current.segs.last_cursor_position();
        println!(
            "📍 position_offset - Last cursor position: DocCharOffset({})",
            last_cursor_position.0
        );

        let result: DocCharOffset = if offset < 0 && -offset > start.0 as i32 {
            println!(
                "📍 position_offset - Negative offset exceeds start position, returning default"
            );
            DocCharOffset::default()
        } else if offset > 0 && (start.0).saturating_add(offset as usize) > last_cursor_position.0 {
            println!(
                "📍 position_offset - Positive offset exceeds document end, returning last position"
            );
            last_cursor_position
        } else {
            let new_offset = start + RelCharOffset(offset as _);
            println!("📍 position_offset - Calculated new offset: DocCharOffset({})", new_offset.0);
            new_offset
        };

        let final_result: CTextPosition = result.into();
        println!(
            "📍 position_offset - OUTPUT: CTextPosition {{ none: {}, pos: {} }}",
            final_result.none, final_result.pos
        );
        final_result
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
    println!(
        "🧭 position_offset_in_direction - INPUT: start={{ none: {}, pos: {} }}, direction: {:?}, offset: {}",
        start.none, start.pos, direction, offset
    );

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            println!(
                "🧭 position_offset_in_direction - ERROR: No current tab markdown, returning default"
            );
            return CTextPosition::default();
        }
    };

    let segs = &markdown.buffer.current.segs;
    let galleys = &markdown.galleys;

    let offset_type =
        if matches!(direction, CTextLayoutDirection::Right | CTextLayoutDirection::Left) {
            Offset::Next(Bound::Char)
        } else {
            Offset::By(Increment::Line)
        };
    let backwards = matches!(direction, CTextLayoutDirection::Left | CTextLayoutDirection::Up);

    println!(
        "🧭 position_offset_in_direction - Offset type: {:?}, backwards: {}",
        offset_type, backwards
    );

    let mut result: DocCharOffset = start.pos.into();
    println!("🧭 position_offset_in_direction - Starting from DocCharOffset({})", result.0);

    for i in 0..offset {
        let prev_result = result;
        result = result.advance(&mut None, offset_type, backwards, segs, galleys, &markdown.bounds);
        println!(
            "🧭 position_offset_in_direction - Step {}: {} -> {}",
            i + 1,
            prev_result.0,
            result.0
        );
    }

    let final_result = CTextPosition { none: start.none, pos: result.0 };
    println!(
        "🧭 position_offset_in_direction - OUTPUT: CTextPosition {{ none: {}, pos: {} }}",
        final_result.none, final_result.pos
    );
    final_result
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer/1614553-isposition
#[no_mangle]
pub unsafe extern "C" fn is_position_at_bound(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backwards: bool,
) -> bool {
    println!(
        "🎯 is_position_at_bound - INPUT: pos={{ none: {}, pos: {} }}, granularity: {:?}, backwards: {}",
        pos.none, pos.pos, granularity, backwards
    );

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            println!("🎯 is_position_at_bound - ERROR: No current tab markdown, returning false");
            return false;
        }
    };

    let text_position: DocCharOffset = pos.pos.into();
    let at_boundary = granularity.into();

    println!(
        "🎯 is_position_at_bound - Converted: text_position=DocCharOffset({}), at_boundary: {:?}",
        text_position.0, at_boundary
    );

    println!("**text**");
    println!("{:?}", markdown.buffer.current.text);
    println!("**boundaries**");
    markdown.print_bounds();

    let result = markdown
        .bounds
        .is_position_at_boundary(text_position, at_boundary, backwards);

    println!("🎯 is_position_at_bound - OUTPUT: {}", result);
    result
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

    let text_position: DocCharOffset = pos.pos.into();
    let at_boundary = granularity.into();

    println!(
        "🎯 is_position_within_bound - INPUT: text_position=DocCharOffset({}), at_boundary: {:?}",
        text_position.0, at_boundary
    );

    let result =
        markdown
            .bounds
            .is_position_within_text_unit(text_position, at_boundary, backwards);

    println!("🎯 is_position_within_bound - OUTPUT: {}", result);
    result
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

    let text_position: DocCharOffset = pos.pos.into();
    let to_boundary = granularity.into();

    println!(
        "🎯 bound_from_position - INPUT: text_position=DocCharOffset({}), to_boundary: {:?}, backwards: {:?}",
        text_position.0, to_boundary, backwards
    );

    let result = markdown
        .bounds
        .position_from(text_position, to_boundary, backwards);

    println!("🎯 bound_from_position - OUTPUT: {:?}", result);
    result.into()
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

    let text_position: DocCharOffset = pos.pos.into();
    let with_granularity = granularity.into();

    println!(
        "🎯 bound_at_position - INPUT: text_position=DocCharOffset({}), to_boundary: {:?}, backwards: {:?}",
        text_position.0, with_granularity, backwards
    );

    let result =
        markdown
            .bounds
            .range_enclosing_position(text_position, with_granularity, backwards);

    println!("🎯 bound_at_position - OUTPUT: {:?}", result);
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
    println!("🔍 position_at_point - INPUT: point=({}, {})", point.x, point.y);

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            println!("🔍 position_at_point - ERROR: No current tab markdown, returning default");
            return CTextPosition::default();
        }
    };

    let galleys = &markdown.galleys;
    println!("🔍 position_at_point - Galleys count: {}", galleys.galleys.len());

    let pos2 = Pos2 { x: point.x as f32, y: point.y as f32 };
    println!("🔍 position_at_point - Converting point to Pos2: ({}, {})", pos2.x, pos2.y);

    let offset = mutation::pos_to_char_offset(pos2, galleys);
    println!("🔍 position_at_point - pos_to_char_offset returned: DocCharOffset({})", offset.0);

    let result = CTextPosition { none: false, pos: offset.0 };
    println!("🔍 position_at_point - OUTPUT: CTextPosition {{ none: false, pos: {} }}", result.pos);

    result
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
    println!("📐 cursor_rect_at_position - INPUT: pos={{ none: {}, pos: {} }}", pos.none, pos.pos);

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            println!(
                "📐 cursor_rect_at_position - ERROR: No current tab markdown, returning default"
            );
            return CRect::default();
        }
    };

    let doc_char_offset = pos.pos.into();
    println!("📐 cursor_rect_at_position - Converting to DocCharOffset: {:?}", doc_char_offset);

    let line = markdown.cursor_line(doc_char_offset);
    println!(
        "📐 cursor_rect_at_position - Cursor line: point1=({}, {}), point2=({}, {})",
        line[0].x, line[0].y, line[1].x, line[1].y
    );

    let result = CRect {
        min_x: line[0].x as f64,
        min_y: line[0].y as f64,
        max_x: line[1].x as f64,
        max_y: line[1].y as f64,
    };

    println!(
        "📐 cursor_rect_at_position - OUTPUT: CRect {{ min_x: {}, min_y: {}, max_x: {}, max_y: {} }}",
        result.min_x, result.min_y, result.max_x, result.max_y
    );

    result
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

    println!("📐 📐 selection_rects() called with range {:?}", range);

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

    println!("📐 📐 selection_rects() result: {:?}", selection_rects);

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

    markdown.buffer.can_undo()
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

    markdown.buffer.can_redo()
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
