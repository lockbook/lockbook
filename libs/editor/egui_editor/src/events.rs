use crate::cursor::Cursor;
use crate::cursor_types::DocCharOffset;
use crate::editor::Editor;
use egui::{Event, Key, PointerButton, Ui, Vec2};
use std::ops::Range;

impl Editor {
    pub fn key_events(&mut self, ui: &mut Ui) {
        if !self.galleys.is_empty() {
            let (galley_idx, _) = self.galley_and_cursor();
            self.fix_cursor(galley_idx, false);
        }

        for event in &ui.ctx().input().events {
            let mut event_matched = true;
            println!("{:?}", event);
            match event {
                Event::Key { key: Key::ArrowRight, pressed: true, modifiers } => {
                    self.cursor_unprocessed = true;
                    self.cursor.x_target = None;

                    let (galley_idx, cur_cursor) = self.galley_and_cursor();
                    if modifiers.shift {
                        self.cursor.set_selection_origin();
                    } else {
                        self.cursor.selection_origin = None;
                    }
                    if modifiers.alt {
                        let (new_galley_idx, new_cursor) =
                            self.cursor_to_next_word_boundary(galley_idx, cur_cursor, false);
                        self.set_galley_and_cursor(new_galley_idx, &new_cursor);
                        break;
                    }
                    if modifiers.command {
                        let galley = &self.galleys[galley_idx];
                        let new_cursor = galley.galley.cursor_end_of_row(&cur_cursor);
                        self.set_galley_and_cursor(galley_idx, &new_cursor);
                        break;
                    }
                    self.cursor_to_next_char(galley_idx, false);
                }
                Event::Key { key: Key::ArrowLeft, pressed: true, modifiers } => {
                    self.cursor_unprocessed = true;
                    self.cursor.x_target = None;

                    let (galley_idx, cur_cursor) = self.galley_and_cursor();
                    if modifiers.shift {
                        self.cursor.set_selection_origin();
                    } else {
                        self.cursor.selection_origin = None;
                    }
                    if modifiers.alt {
                        let (new_galley_idx, new_cursor) =
                            self.cursor_to_next_word_boundary(galley_idx, cur_cursor, true);
                        self.set_galley_and_cursor(new_galley_idx, &new_cursor);
                        break;
                    }
                    if modifiers.command {
                        let galley = &self.galleys[galley_idx];
                        let new_cursor = galley.galley.cursor_begin_of_row(&cur_cursor);
                        self.set_galley_and_cursor(galley_idx, &new_cursor);
                        break;
                    }
                    self.cursor_to_next_char(galley_idx, true);
                }
                Event::Key { key: Key::ArrowDown, pressed: true, modifiers } => {
                    self.cursor_unprocessed = true;
                    if modifiers.shift {
                        self.cursor.set_selection_origin();
                    } else {
                        self.cursor.selection_origin = None;
                    }
                    if modifiers.command {
                        self.cursor.loc = self.last_cursor_position();
                        let (galley_idx, _) = self.galley_and_cursor();
                        self.fix_cursor(galley_idx, false);
                        self.cursor.x_target = None;
                        break;
                    }

                    let (cur_galley_idx, cur_cursor) = self.galley_and_cursor();
                    let cur_galley = &self.galleys[cur_galley_idx];

                    // the first time we use an up or down arrow, remember the x we started at
                    let x_target = self.cursor.set_x_target(cur_galley, cur_cursor);

                    let at_bottom_of_cur_galley =
                        cur_cursor.rcursor.row == cur_galley.galley.rows.len() - 1;
                    let in_last_galley = cur_galley_idx == self.galleys.len() - 1;
                    let (mut new_cursor, new_galley_idx) =
                        if at_bottom_of_cur_galley && !in_last_galley {
                            // move to the first row of the next galley
                            let new_galley_idx = cur_galley_idx + 1;
                            let new_galley = &self.galleys[new_galley_idx];
                            let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                                x: 0.0, // overwritten below
                                y: 0.0, // top of new galley
                            });
                            (new_cursor, new_galley_idx)
                        } else {
                            // move down one row in the current galley
                            let new_cursor = cur_galley.galley.cursor_down_one_row(&cur_cursor);
                            (new_cursor, cur_galley_idx)
                        };

                    if !(at_bottom_of_cur_galley && in_last_galley) {
                        // move to the x_target in the new row/galley
                        new_cursor = Cursor::move_to_x_target(
                            &self.galleys[new_galley_idx],
                            new_cursor,
                            x_target,
                        );
                    } else {
                        // we moved to the end of the last line
                        self.cursor.x_target = None;
                    }

                    self.set_galley_and_cursor(new_galley_idx, &new_cursor);
                }
                Event::Key { key: Key::ArrowUp, pressed: true, modifiers } => {
                    self.cursor_unprocessed = true;
                    if modifiers.shift {
                        self.cursor.set_selection_origin();
                    } else {
                        self.cursor.selection_origin = None;
                    }
                    if modifiers.command {
                        self.cursor.loc = DocCharOffset(0);
                        let (galley_idx, _) = self.galley_and_cursor();
                        self.fix_cursor(galley_idx, false);
                        self.cursor.x_target = None;
                        break;
                    }

                    let (cur_galley_idx, cur_cursor) = self.galley_and_cursor();
                    let cur_galley = &self.galleys[cur_galley_idx];

                    // the first time we use an up or down arrow, remember the x we started at
                    let x_target = self.cursor.set_x_target(cur_galley, cur_cursor);

                    let at_top_of_cur_galley = cur_cursor.rcursor.row == 0;
                    let in_first_galley = cur_galley_idx == 0;
                    let (mut new_cursor, new_galley_idx) =
                        if at_top_of_cur_galley && !in_first_galley {
                            // move to the last row of the previous galley
                            let new_galley_idx = cur_galley_idx - 1;
                            let new_galley = &self.galleys[new_galley_idx];
                            let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                                x: 0.0,                          // overwritten below
                                y: new_galley.galley.rect.max.y, // bottom of new galley
                            });
                            (new_cursor, new_galley_idx)
                        } else {
                            // move up one row in the current galley
                            let new_cursor = cur_galley.galley.cursor_up_one_row(&cur_cursor);
                            (new_cursor, cur_galley_idx)
                        };

                    if !(at_top_of_cur_galley && in_first_galley) {
                        // move to the x_target in the new row/galley
                        new_cursor = Cursor::move_to_x_target(
                            &self.galleys[new_galley_idx],
                            new_cursor,
                            x_target,
                        );
                    } else {
                        // we moved to the start of the first line
                        self.cursor.x_target = None;
                    }

                    self.set_galley_and_cursor(new_galley_idx, &new_cursor);
                }
                Event::Paste(text) | Event::Text(text) => {
                    self.text_unprocessed = true;
                    self.cursor.x_target = None;

                    if let Some(range) = self.cursor.selection_range() {
                        self.replace(range, "");
                    }
                    self.insert_at_cursor(text);

                    self.cursor.selection_origin = None;
                }
                Event::Key { key: Key::Backspace, pressed: true, modifiers: _modifiers } => {
                    self.text_unprocessed = true;
                    self.cursor.x_target = None;

                    let range = self.cursor.selection_range().unwrap_or_else(|| Range {
                        start: if self.cursor.loc == 0 {
                            DocCharOffset(0)
                        } else {
                            self.cursor.loc - 1
                        },
                        end: self.cursor.loc,
                    });
                    self.replace(range, "");

                    self.cursor.selection_origin = None;
                }
                Event::Key { key: Key::Enter, pressed: true, modifiers: _ } => {
                    self.text_unprocessed = true;
                    self.cursor.x_target = None;

                    if let Some(range) = self.cursor.selection_range() {
                        self.replace(range, "");
                    }
                    self.insert_at_cursor("\n");

                    self.cursor.selection_origin = None;
                }
                Event::Key { key: Key::A, pressed: true, modifiers } => {
                    if modifiers.command {
                        self.cursor_unprocessed = true;
                        self.cursor.selection_origin = Some(DocCharOffset(0));
                        self.cursor.loc = self.last_cursor_position();
                    } else {
                        event_matched = false;
                    }
                }
                Event::Key { key: Key::F2, pressed: true, modifiers: _modifiers } => {
                    self.debug.enabled = !self.debug.enabled;
                }
                _ => {
                    event_matched = false;
                }
            }
            if event_matched {
                break;
            }
        }
    }

    pub fn mouse_events(&mut self, ui: &mut Ui) {
        for event in &ui.ctx().input().events {
            let pos = if let Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            } = event
            {
                if !modifiers.shift {
                    // click: end selection
                    self.cursor.selection_origin = None;
                } else {
                    // shift+click: begin selection
                    self.cursor.set_selection_origin();
                }
                // any click: begin drag; update cursor
                self.cursor.set_click_and_drag_origin();
                pos
            } else if let Event::PointerMoved(pos) = event {
                if self.cursor.click_and_drag_origin.is_some() {
                    // drag: begin selection; update cursor
                    self.cursor.set_selection_origin();
                    pos
                } else {
                    continue;
                }
            } else if let Event::PointerButton {
                button: PointerButton::Primary,
                pressed: false,
                ..
            } = event
            {
                // click released: end drag; don't update cursor
                self.cursor.click_and_drag_origin = None;
                continue;
            } else {
                continue;
            };

            // click, shift+click, drag: update cursor position
            self.cursor_unprocessed = true;
            if !self.galleys.is_empty() {
                if pos.y < self.galleys[0].ui_location.min.y {
                    // click position is above first galley
                    self.cursor.loc = DocCharOffset(0);
                    continue;
                }
                if pos.y >= self.galleys[self.galleys.len() - 1].ui_location.max.y {
                    // click position is below last galley
                    self.cursor.loc = self.last_cursor_position();
                    continue;
                }
            }
            for galley_idx in 0..self.galleys.len() {
                let galley = &self.galleys[galley_idx];
                if galley.ui_location.contains(*pos) {
                    // click position is in a galley
                    let pos = *pos;
                    let relative_pos = pos - galley.text_location;
                    let cursor = galley.galley.cursor_from_pos(relative_pos);
                    self.set_galley_and_cursor(galley_idx, &cursor);
                }
            }
        }
    }
}
