use egui::{Key, Modifiers, PointerButton, TouchDeviceId, TouchId, TouchPhase};
use lb_c::model::text::offset_types::{Grapheme, RangeExt as _};
use std::cmp;
use std::ffi::{CStr, CString, c_char, c_void};
use tracing::instrument;
use workspace_rs::tab::markdown_editor::TouchTarget;
use workspace_rs::tab::markdown_editor::bounds::BoundExt as _;
use workspace_rs::tab::markdown_editor::input::{
    Advance, Bound, Event, Increment, Location, Region,
};
use workspace_rs::tab::markdown_editor::text_units::Unit;
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::{ContentState, ExtendedInput as _, TabContent};

use super::super::response::*;
use super::position::{grapheme_at, graphemes_in, position};
use super::response::*;
use crate::WgpuWorkspace;
use crate::apple::keyboard::UIKeys;

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level = "trace", skip(obj))]
pub unsafe extern "C" fn ios_frame(obj: *mut c_void) -> IOSResponse {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let mut response: IOSResponse = obj.frame().into();
    response.chrome_text_focused = obj.workspace.chrome_text_focused();
    response
}

/// A frame that consumes no queued input, for geometry right after a UIKit
/// edit. Input queued meanwhile waits for the next frame.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_layout_frame(obj: *mut c_void) -> IOSResponse {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let events = std::mem::take(&mut obj.renderer.raw_input.events);
    let mut response: IOSResponse = obj.frame().into();
    obj.renderer.raw_input.events = events;
    response.chrome_text_focused = obj.workspace.chrome_text_focused();
    response
}

/// Page scroll from the text view's pan; `dy` is finger travel in points.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_scroll(obj: *mut c_void, dy: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let standalone = obj.workspace.current_tab_markdown().is_none();
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        if standalone {
            md.overflow_scroll_by(-dy);
        } else {
            md.scroll_area.gesture_scroll(-dy);
        }
    }
}

/// Coast from the pan's release velocity in points per second.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_fling(obj: *mut c_void, vy: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if obj.workspace.current_tab_markdown().is_none() {
        return;
    }
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.scroll_area.gesture_fling(-vy);
    }
}

/// Stop coasting. Returns whether the page was coasting.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_stop_scroll(obj: *mut c_void) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace
        .focused_mdedit_mut()
        .is_some_and(|md| md.scroll_area.gesture_stop())
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614543-inserttext
#[no_mangle]
pub unsafe extern "C" fn insert_text(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Ok(content) = CStr::from_ptr(content).to_str() else { return };

    // The find field takes keystrokes as egui events, as a desktop types them.
    if obj.workspace.chrome_text_focused() {
        let event = match content {
            "\n" => key_event(Key::Enter),
            "\t" => key_event(Key::Tab),
            _ => egui::Event::Text(content.into()),
        };
        obj.renderer.raw_input.events.push(event);
        obj.renderer.context.request_repaint();
        return;
    }

    if content == "\n" {
        // An open completion popup submits on the Enter it listens for.
        let completions_active = obj
            .workspace
            .focused_mdedit_mut()
            .map(|md| md.emoji_completions.active || md.link_completions.active)
            .unwrap_or(false);
        if completions_active {
            obj.renderer.raw_input.events.push(key_event(Key::Enter));
            obj.renderer.context.request_repaint();
        } else {
            obj.workspace
                .apply_platform_event(Event::Newline { shift: false });
        }
    } else if content == "\t" {
        obj.workspace
            .apply_platform_event(Event::Indent { deindent: false });
    } else {
        obj.workspace.apply_platform_event(Event::Replace {
            region: Region::Selection,
            text: content.into(),
            advance_cursor: true,
        });
    }
}

/// Paste text now, as the editor pastes: a URL over a selection links it.
///
/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn paste_text(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Ok(content) = CStr::from_ptr(content).to_str() else { return };
    if obj.workspace.chrome_text_focused() {
        obj.renderer
            .raw_input
            .events
            .push(egui::Event::Paste(content.into()));
        obj.renderer.context.request_repaint();
        return;
    }
    let event = obj
        .workspace
        .focused_mdedit_mut()
        .and_then(|md| md.paste_event(content.into()));
    if let Some(event) = event {
        obj.workspace.apply_platform_event(event);
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uikeyinput/1614572-deletebackward
#[no_mangle]
pub unsafe extern "C" fn backspace(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if obj.workspace.chrome_text_focused() {
        obj.renderer
            .raw_input
            .events
            .push(key_event(Key::Backspace));
        obj.renderer.context.request_repaint();
        return;
    }
    obj.workspace.apply_platform_event(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614558-replace
#[no_mangle]
pub unsafe extern "C" fn replace_text(obj: *mut c_void, range: CTextRange, text: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    // A replacement is UIKit's edit of the document; the find field has none.
    if obj.workspace.chrome_text_focused() {
        return;
    }
    let Ok(text) = CStr::from_ptr(text).to_str() else { return };
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return };
    let Some((start, end)) = graphemes_in(md, &range) else { return };
    obj.workspace.apply_platform_event(Event::Replace {
        region: Region::BetweenLocations {
            start: Location::Grapheme(start),
            end: Location::Grapheme(end),
        },
        text: text.into(),
        advance_cursor: true,
    });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn copy_image(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if let Some(image_viewer) = obj.workspace.current_tab_image() {
        image_viewer.copy_image(&obj.renderer.context);
    }
}

/// A key press as egui sees it, for the editor's chrome and popups.
fn key_event(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614527-text
#[no_mangle]
pub unsafe extern "C" fn text_in_range(obj: *mut c_void, range: CTextRange) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let text = obj
        .workspace
        .focused_mdedit_mut()
        .and_then(|md| graphemes_in(md, &range).map(|range| md.renderer.buffer[range].to_string()))
        .unwrap_or_default();
    CString::new(text).unwrap_or_default().into_raw()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614541-selectedtextrange
#[no_mangle]
pub unsafe extern "C" fn get_selected(obj: *mut c_void) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextRange::default() };
    let selection = md.renderer.buffer.current.selection;
    CTextRange {
        none: false,
        start: position(md, selection.start()),
        end: position(md, selection.end()),
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614541-selectedtextrange
///
/// `reveal` scrolls the caret into view afterwards; false for a write made
/// mid-gesture, where the caret is under the finger and the gesture scrolls.
#[no_mangle]
pub unsafe extern "C" fn set_selected(obj: *mut c_void, range: CTextRange, reveal: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return };
    let Some((lo, hi)) = graphemes_in(md, &range) else { return };

    // UIKit's range has no direction. Keep the end that didn't move as the
    // anchor so the head is the end that did, e.g. a dragged handle.
    let (anchor, head) = md.renderer.buffer.current.selection;
    let (old_lo, old_hi) = (anchor.min(head), anchor.max(head));
    let hi_moved = lo == old_lo && hi != old_hi;
    let lo_moved = hi == old_hi && lo != old_lo;
    let head_is_lo = !hi_moved && (lo_moved || head < anchor);
    let (start, end) = if head_is_lo { (hi, lo) } else { (lo, hi) };

    obj.workspace.apply_platform(
        Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(start),
                end: Location::Grapheme(end),
            },
        },
        reveal,
    );
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614489-markedtextrange
/// should we be returning a subset of the document? https://stackoverflow.com/questions/12676851/uitextinput-is-it-ok-to-return-incorrect-beginningofdocument-endofdocumen
#[no_mangle]
pub unsafe extern "C" fn end_of_document(obj: *mut c_void) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextPosition::default() };
    position(md, md.renderer.buffer.current.segs.last_cursor_position())
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level = "trace", skip(obj))]
pub unsafe extern "C" fn touches_began(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    let pos = obj.renderer.pos_from_points(x, y);
    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Start,
        pos,
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
#[instrument(level = "trace", skip(obj))]
pub unsafe extern "C" fn touches_moved(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    let pos = obj.renderer.pos_from_points(x, y);

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Move,
        pos,
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerMoved(pos));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
#[instrument(level = "trace", skip(obj))]
pub unsafe extern "C" fn touches_ended(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let force = if force == 0.0 { None } else { Some(force) };
    let pos = obj.renderer.pos_from_points(x, y);

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::End,
        pos,
        force,
    });

    obj.renderer
        .raw_input
        .events
        .push(egui::Event::PointerButton {
            pos,
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
#[instrument(level = "trace", skip(obj))]
pub unsafe extern "C" fn touches_cancelled(obj: *mut c_void, id: u64, x: f32, y: f32, force: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let force = if force == 0.0 { None } else { Some(force) };
    let pos = obj.renderer.pos_from_points(x, y);

    obj.renderer.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id),
        phase: TouchPhase::Cancel,
        pos,
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
    let pos = obj.renderer.pos_from_points(x, y);

    obj.renderer
        .context
        .push_event(workspace_rs::Event::PredictedTouch { id: TouchId(id), force, pos });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uiresponder/1621142-touchesbegan
#[no_mangle]
pub unsafe extern "C" fn tab_count(obj: *mut c_void) -> i64 {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.workspace.tab_strip.len() as i64
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn will_consume_touch(obj: *mut c_void, x: f32, y: f32) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let pos = obj.renderer.pos_from_points(x, y);
    if let Some(tab) = obj.workspace.current_tab() {
        if let ContentState::Open(TabContent::Svg(svg)) = &tab.content {
            svg.detect_islands_interaction(pos)
        } else if let ContentState::Open(TabContent::Image(image)) = &tab.content {
            image.detect_islands_interaction(pos)
        } else if let ContentState::Open(TabContent::Markdown(md)) = &tab.content {
            md.will_consume_touch(pos)
        } else {
            false
        }
    } else {
        false
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn multi_touch(
    obj: *mut c_void, x: f32, y: f32, factor: f32, focus_x: f32, focus_y: f32, start_x: *const f32,
    start_y: *const f32, start_count: usize,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let start_positions = parse_start_positions(obj, start_x, start_y, start_count);
    let translation_delta = egui::vec2(x, y);
    let center_pos = obj.renderer.pos_from_points(focus_x, focus_y);
    obj.renderer
        .context
        .push_event(workspace_rs::Event::MultiTouchGesture {
            rotation_delta: 0.0,
            translation_delta,
            zoom_factor: factor,
            center_pos,
            start_positions,
        });
}

fn parse_start_positions(
    obj: &WgpuWorkspace, start_x: *const f32, start_y: *const f32, count: usize,
) -> Vec<egui::Pos2> {
    (0..count)
        .map(|i| unsafe {
            let x = *start_x.add(i);
            let y = *start_y.add(i);
            obj.renderer.pos_from_points(x, y)
        })
        .collect()
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
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextPosition::default() };
    let Some(start) = grapheme_at(md, &start) else { return CTextPosition::default() };
    let n = offset.max(0) as usize;
    let last = md.renderer.buffer.current.segs.last_cursor_position();
    let result = match direction {
        CTextLayoutDirection::Right => md
            .renderer
            .snap_offset_out_of_folds((start + n).min(last), false),
        CTextLayoutDirection::Left => md
            .renderer
            .snap_offset_out_of_folds(Grapheme(start.0.saturating_sub(n)), true),
        CTextLayoutDirection::Down => md.lines_from(start, n, false),
        CTextLayoutDirection::Up => md.lines_from(start, n, true),
    };
    position(md, result)
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
///
/// https://developer.apple.com/documentation/uikit/uitextinput/1614570-firstrect
#[no_mangle]
pub unsafe extern "C" fn first_rect(obj: *mut c_void, range: CTextRange) -> CRect {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CRect::default() };
    let Some((start, end)) = graphemes_in(md, &range) else { return CRect::default() };

    // The range's first line: from its start to its end or the line's end.
    let line_end = start.advance_to_bound(Bound::Line, false, &md.renderer.bounds);
    let end = cmp::min(end, cmp::max(line_end, start));
    let (Some(start_line), Some(end_line)) = (md.cursor_line(start), md.cursor_line(end)) else {
        return CRect::default();
    };
    CRect {
        min_x: start_line[0].x as f64,
        min_y: start_line[0].y as f64,
        max_x: end_line[1].x as f64,
        max_y: end_line[1].y as f64,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn position_at_point(obj: *mut c_void, point: CPoint) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let pos = obj.renderer.pos_from_points(point.x as f32, point.y as f32);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextPosition::default() };
    let offset = md.pos_to_char_offset(pos);
    position(md, offset)
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn cursor_rect_at_position(obj: *mut c_void, pos: CTextPosition) -> CRect {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CRect::default() };
    let Some(offset) = grapheme_at(md, &pos) else { return CRect::default() };
    let Some(line) = md.cursor_line(offset) else { return CRect::default() };
    if !(line[0].x.is_finite() && line[0].y.is_finite() && line[1].y.is_finite()) {
        return CRect::default();
    }
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
    let Some(tab) = obj.workspace.current_tab_mut() else {
        return;
    };
    if let Some(markdown) = tab.markdown_mut() {
        markdown.virtual_keyboard_shown = showing;
        markdown.keyboard_visible = showing;
    } else if let Some(chat) = tab.chat_mut() {
        chat.set_keyboard_shown(showing);
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn selection_rects(
    obj: *mut c_void, range: CTextRange,
) -> UITextSelectionRects {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else {
        return UITextSelectionRects::default();
    };
    let Some(range) = graphemes_in(md, &range) else { return UITextSelectionRects::default() };

    let rects: Vec<CRect> = md
        .selection_rects(range)
        .into_iter()
        .map(|rect| CRect {
            min_x: rect.min.x as f64,
            min_y: rect.min.y as f64,
            max_x: rect.max.x as f64,
            max_y: rect.max.y as f64,
        })
        .collect();
    UITextSelectionRects {
        size: rects.len() as i32,
        rects: Box::into_raw(rects.into_boxed_slice()) as *const CRect,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn free_selection_rects(rects: UITextSelectionRects) {
    let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        rects.rects as *mut CRect,
        rects.size as usize,
    ));
}

fn tab_info(session: &workspace_rs::tab::Session) -> CTabInfo {
    CTabInfo {
        session_id: session.id.as_uuid().into(),
        dest_kind: session.dest.kind_code(),
        dest_file: session.dest.backing_file().unwrap_or_default().into(),
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn get_tabs(obj: *mut c_void) -> CTabs {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let tabs: Vec<CTabInfo> = obj.workspace.tab_strip.iter().map(tab_info).collect();

    CTabs {
        size: tabs.len() as i32,
        tabs: Box::into_raw(tabs.into_boxed_slice()) as *const CTabInfo,
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn free_tabs(tabs: CTabs) {
    let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        tabs.tabs as *mut CTabInfo,
        tabs.size as usize,
    ));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn reorder_tab(obj: *mut c_void, from: usize, to: usize) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.move_tab(from, to);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn get_recently_closed_tabs(obj: *mut c_void) -> CTabs {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let tabs: Vec<CTabInfo> = obj
        .workspace
        .recently_closed_tabs()
        .into_iter()
        .map(tab_info)
        .collect();

    CTabs {
        size: tabs.len() as i32,
        tabs: Box::into_raw(tabs.into_boxed_slice()) as *const CTabInfo,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn reopen_last_closed_tab(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.reopen_closed_tab();
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn reopen_closed_tab(obj: *mut c_void, id: CUuid) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace
        .reopen_closed_session(workspace_rs::tab::SessionId::from_uuid(id.into()));
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
    obj.workspace
        .apply_platform_event(if redo { Event::Redo } else { Event::Undo });
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn can_undo(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.focused_mdedit_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    !markdown.renderer.readonly && markdown.renderer.buffer.can_undo()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn can_redo(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let markdown = match obj.workspace.focused_mdedit_mut() {
        Some(markdown) => markdown,
        None => return false,
    };

    !markdown.renderer.readonly && markdown.renderer.buffer.can_redo()
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn current_tab(obj: *mut c_void) -> i64 {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    crate::current_tab_type(&obj.workspace) as i64
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

    obj.workspace.tabs.values_mut().for_each(|t| {
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

    if let Some(tab_id) = obj.workspace.current_tab {
        obj.workspace.clear_tab_rename(tab_id);
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

    if !obj.workspace.tab_strip.is_empty() {
        if let Some(idx) = obj.workspace.current_slot_index() {
            obj.workspace.close_tab(idx);
        }
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn close_all_tabs(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.close_all_tabs();
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
    if let Some(key) = key.egui_key(shift) {
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
pub unsafe extern "C" fn set_ws_inset(obj: *mut c_void, inset: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    obj.renderer.bottom_inset = Some(inset as u32);
}

/// UIKit's tokenizer, answered from the editor's text units in place. See
/// [`workspace_rs::tab::markdown_editor::text_units`].
fn unit_of(granularity: CTextGranularity) -> Unit {
    match granularity {
        CTextGranularity::Character => Unit::Character,
        CTextGranularity::Word => Unit::Word,
        CTextGranularity::Sentence => Unit::Sentence,
        CTextGranularity::Paragraph => Unit::Paragraph,
        CTextGranularity::Line => Unit::Line,
        CTextGranularity::Document => Unit::Document,
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn unit_at_boundary(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backward: bool,
) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return false };
    let Some(p) = grapheme_at(md, &pos) else { return false };
    md.unit_at_boundary(unit_of(granularity), p, backward)
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn unit_within(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backward: bool,
) -> bool {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return false };
    let Some(p) = grapheme_at(md, &pos) else { return false };
    md.unit_within(unit_of(granularity), p, backward)
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn unit_boundary_from(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backward: bool,
) -> CTextPosition {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextPosition::default() };
    let Some(p) = grapheme_at(md, &pos) else { return CTextPosition::default() };
    match md.unit_boundary_from(unit_of(granularity), p, backward) {
        Some(b) => position(md, b),
        None => CTextPosition::default(),
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn unit_enclosing(
    obj: *mut c_void, pos: CTextPosition, granularity: CTextGranularity, backward: bool,
) -> CTextRange {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextRange::default() };
    let Some(p) = grapheme_at(md, &pos) else { return CTextRange::default() };
    match md.unit_enclosing(unit_of(granularity), p, backward) {
        Some((start, end)) => {
            CTextRange { none: false, start: position(md, start), end: position(md, end) }
        }
        None => CTextRange::default(),
    }
}

/// What the editor painted under a point, for the host's hit test: 0 text,
/// 1 a tap target, 2 the scrollbar, 3 a popup egui handles itself.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_touch_target_at(obj: *mut c_void, x: f32, y: f32) -> u8 {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    obj.workspace
        .focused_mdedit_mut()
        .and_then(|md| {
            md.touch_target_at(pos).map(|target| match target {
                TouchTarget::Tap(_) => 1,
                TouchTarget::Scrollbar => 2,
                TouchTarget::Popup => 3,
            })
        })
        .unwrap_or(0)
}

/// Tap the target under a point. A link or an image answers the range the
/// host selects, with the atom menu; `none` when the editor acted or
/// nothing was there.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_tap(obj: *mut c_void, x: f32, y: f32) -> CTextRange {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    let Some(md) = obj.workspace.focused_mdedit_mut() else { return CTextRange::default() };
    match md.tap(pos) {
        Some((start, end)) => {
            CTextRange { none: false, start: position(md, start), end: position(md, end) }
        }
        None => CTextRange::default(),
    }
}

/// Whether a long press at a point can lift a list item. The host checks
/// the keyboard.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_reorder_can_start(obj: *mut c_void, x: f32, y: f32) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    obj.workspace
        .focused_mdedit_mut()
        .is_some_and(|md| md.reorder_can_start(pos))
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_reorder_start(obj: *mut c_void, x: f32, y: f32) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    obj.workspace
        .focused_mdedit_mut()
        .is_some_and(|md| md.reorder_start(pos))
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_reorder_move(obj: *mut c_void, x: f32, y: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.reorder_move(pos);
    }
}

/// Drop the lifted item at a point, or put it back if `cancelled`.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_reorder_end(obj: *mut c_void, x: f32, y: f32, cancelled: bool) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let pos = obj.renderer.pos_from_points(x, y);
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.reorder_end(pos, cancelled);
    }
}

/// A touch landed on the scrollbar at `y` points.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_scrollbar_begin(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.scroll_area.gesture_scrollbar_begin();
    }
}

/// Whether the point is on the scrollbar's thumb, where a drag may start.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_on_scroll_thumb(obj: *mut c_void, y: f32) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let y = obj.renderer.pos_from_points(0.0, y).y;
    obj.workspace
        .focused_mdedit_mut()
        .is_some_and(|md| md.scroll_area.on_thumb(y))
}

/// The scrollbar thumb was dragged `dy` points.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_scrollbar_drag(obj: *mut c_void, dy: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.scroll_area.gesture_scrollbar_drag(dy);
    }
}

/// One text row with its spacing, in points, for the host's edge crawl.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_row_height(obj: *mut c_void) -> f32 {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace
        .focused_mdedit_mut()
        .map(|md| md.renderer.layout.row_height + md.renderer.layout.row_spacing)
        .unwrap_or(28.0)
}

/// The platform's marked text, or none, for the editor to paint.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_set_marked(obj: *mut c_void, range: CTextRange) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.renderer.platform_marked = graphemes_in(md, &range).filter(|r| r.0 < r.1);
        md.renderer.ctx.request_repaint();
    }
}

/// An edge crawl of `dy` points of finger-equivalent travel: a pan that
/// stops at the document's end.
///
/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_crawl(obj: *mut c_void, dy: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let standalone = obj.workspace.current_tab_markdown().is_none();
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        if standalone {
            md.overflow_scroll_by(-dy);
        } else {
            md.crawl_by(-dy);
        }
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn ios_text_traits(obj: *mut c_void) -> CTextTraits {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let send_on_return = obj
        .workspace
        .current_tab()
        .and_then(|tab| tab.chat())
        .is_some_and(|chat| chat.composing());
    match obj.workspace.focused_mdedit_mut() {
        Some(md) => CTextTraits {
            valid: true,
            editable: !md.renderer.readonly,
            secure: md.renderer.mask,
            single_line: md.renderer.single_line,
            completions: md.emoji_completions.active || md.link_completions.active,
            send_on_return,
        },
        None => CTextTraits::default(),
    }
}
