use comrak::nodes::{AstNode, NodeTaskItem, NodeValue};
use egui::emath::GuiRounding as _;
use egui::emath::easing::{back_out, cubic_in, cubic_out};
use egui::{Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, lerp};
use lb_rs::model::text::offset_types::{Grapheme, Graphemes, RangeExt as _};

use crate::style::chrome::{ANIM_MAX_SECS, Radius, STROKE_HAIRLINE, TOGGLE_ANIM_SECS};

use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::utils::{
    consume_indent_columns_ceil, leading_indent_cols_ceil,
};
use crate::tab::markdown_editor::{Event, MdRender};
use crate::theme::palette_v2::{Theme, ThemeExt};

/// Checking: the box pops and the check springs in over this long.
const POP_SECS: f32 = ANIM_MAX_SECS;
/// The fill grows or collapses, and the check draws, over this long.
const FILL_SECS: f32 = TOGGLE_ANIM_SECS;
/// Checking: the check starts this long after the fill.
const CHECK_DELAY_SECS: f32 = 0.04;
/// The ghost check that previews a click.
const GHOST_ALPHA: f32 = 0.35;
/// Pressing shrinks the box to this scale.
const PRESS_SCALE: f32 = 0.9;
/// The check's polyline in a unit box.
const CHECK: [(f32, f32); 3] = [(0.27, 0.525), (0.44, 0.69), (0.74, 0.325)];

/// How the box looks `secs` after it was toggled (`None`: at rest).
struct Look {
    scale: f32,
    fill: f32,
    draw: f32,
    check_scale: f32,
}

impl Look {
    fn at(checked: bool, secs: Option<f32>) -> Self {
        let s = secs.unwrap_or(f32::INFINITY);
        let ramp = |from: f32, len: f32| ((s - from) / len).clamp(0.0, 1.0);
        if checked {
            // pressed size, up past full, settle
            let pop = ramp(0.0, POP_SECS);
            let scale = if pop < 0.55 {
                lerp(PRESS_SCALE..=1.1, cubic_out(pop / 0.55))
            } else {
                lerp(1.1..=1.0, cubic_out((pop - 0.55) / 0.45))
            };
            Self {
                scale,
                fill: cubic_out(ramp(0.0, FILL_SECS)),
                draw: cubic_out(ramp(CHECK_DELAY_SECS, FILL_SECS)),
                check_scale: lerp(
                    0.5..=1.0,
                    back_out(ramp(CHECK_DELAY_SECS, POP_SECS - CHECK_DELAY_SECS)),
                ),
            }
        } else {
            // the check leaves with the fill
            Self {
                scale: 1.0,
                fill: 1.0 - cubic_in(ramp(0.0, FILL_SECS)),
                draw: 1.0,
                check_scale: 1.0,
            }
        }
    }
}

/// The check in `box_rect`, its first `draw` (0..=1) drawn, scaled by `scale`.
fn paint_check(ui: &Ui, box_rect: Rect, draw: f32, scale: f32, color: egui::Color32) {
    if draw <= 0.0 || color.a() == 0 {
        return;
    }
    let size = box_rect.width() * scale;
    let corner = box_rect.center() - Vec2::splat(size / 2.0);
    let points: Vec<Pos2> = CHECK
        .iter()
        .map(|&(x, y)| corner + Vec2::new(x, y) * size)
        .collect();
    ui.painter().add(Shape::line(
        partial_polyline(&points, draw),
        Stroke::new(box_rect.width() / 8.0, color),
    ));
}

/// The first `draw` (0..=1) of `points`' length.
fn partial_polyline(points: &[Pos2], draw: f32) -> Vec<Pos2> {
    let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * draw;
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        let len = w[0].distance(w[1]);
        if left >= len {
            out.push(w[1]);
            left -= len;
        } else {
            out.push(w[0] + (w[1] - w[0]) * (left / len));
            break;
        }
    }
    out
}

impl<'ast> MdRender {
    pub fn height_task_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_task_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_task_item: &NodeTaskItem,
    ) {
        let maybe_check = node_task_item.symbol;
        let checked = maybe_check.is_some();

        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);
        // Inflated when an image sits on the first row, so the
        // checkbox centers on the visual row.
        let annotation_row_height = self
            .first_content_row_height_inflated(node)
            .unwrap_or(row_height);

        let annotation_size = Vec2 { x: self.layout.indent, y: annotation_row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        // when revealed, the raw `- [ ]` marker occupies this column instead
        if !self.reveal_line(node, first_line) {
            // one size on every row: a body text row's height
            let checkbox_space = Rect::from_center_size(
                annotation_space.center(),
                Vec2::splat(self.layout.row_height),
            );
            let checkbox_id = ui.scope_id().with("task_checkbox");

            // allocate a slightly larger clickable space that makes the checkbox
            // easier to tap without overflowing the annotation space horizontally
            // or overlapping with another row vertically.
            let extra_width = (annotation_space.width() - checkbox_space.width()) / 2.;
            let extra_height = self.layout.row_spacing / 2.;
            let clickable_space = checkbox_space.expand(extra_width.min(extra_height));

            // On touch the checkbox still toggles (click) but doesn't drag —
            // reorder is the body's long-press (see `detect_touch_reorder`).
            let sense = if self.readonly {
                Sense::hover()
            } else if self.touch_mode {
                Sense::click()
            } else {
                Sense::click_and_drag()
            };
            // A finger's target: the gutter beside the box and the row's
            // height, stopping short of the item's text.
            let target = Rect::from_min_max(
                clickable_space.min - Vec2::new(11.0, 4.0),
                clickable_space.max + Vec2::new(4.0, 4.0),
            );
            let check_offset = self.check_offset(node);
            let checkbox_response = self.show_checkbox(
                ui,
                checkbox_id,
                [checkbox_space, clickable_space, target],
                checked,
                check_offset,
                sense,
            );
            // Click toggles; drag starts a reorder (`click_and_drag`
            // above distinguishes the two).
            self.handle_item_drag_resp(ui, node, &checkbox_response, false);
        }

        let any_children = node.children().next().is_some();
        let hovered = if any_children {
            self.show_block_children(ui, node, top_left + self.layout.indent * Vec2::X);

            // todo: proper hit-testing (this ignores anything covering the space)
            let children_height = self.block_children_height(node);
            let children_space =
                Rect::from_min_size(top_left, Vec2::new(self.width(node), children_height));
            children_space.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()))
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
            let width = self.width(node);
            let result = self.compute_section_layout_new(
                line_content,
                width,
                self.layout.row_height,
                self.text_format_document(),
            );
            self.show_wrap_layout(ui, top_left + self.layout.indent * Vec2::X, &result);
            self.show_block_line_prefixes(
                ui,
                node,
                line,
                top_left + self.layout.indent * Vec2::X,
                row_height,
            );
            false
        };

        // fold button
        // todo: proper hit-testing (this ignores anything covering the space)
        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());

        let (fold_button_size, fold_button_icon_size, fold_button_space) =
            Self::fold_button_size_icon_size_space(top_left, row_height, self.layout.indent);
        let show_fold_button = self.interactive
            && !self.painting_drag_float // chrome shouldn't travel with the drag card
            && !self.reveal_line(node, first_line) // the revealed marker occupies the gutter
            && (self.touch_mode
                || hovered
                || fold_button_space.contains(pointer)
                || annotation_space.contains(pointer)
                || self.fold(node).is_some()
                || self.selected_fold_item(node));
        if !show_fold_button {
            return;
        }

        self.show_fold_button(
            ui,
            node,
            (fold_button_size, fold_button_icon_size, fold_button_space),
            self.item_fold_reveal(node),
        );
    }

    /// The checkbox every task item and front matter checkbox is. `rects`
    /// are the box (a body row's height square), the area taking clicks,
    /// and a finger's target. A click or tap sends [`Event::ToggleTask`]
    /// for the mark at `check`, which animates the box.
    pub(crate) fn show_checkbox(
        &mut self, ui: &mut Ui, id: egui::Id, rects: [Rect; 3], checked: bool, check: Grapheme,
        sense: Sense,
    ) -> egui::Response {
        let [space, clickable, target] = rects;
        let response = ui.interact(clickable, id, sense);
        let toggle = Event::ToggleTask { check };
        if response.clicked() {
            self.render_events.push(toggle.clone());
        }
        if !self.readonly {
            self.touch_targets.push((target, TouchTarget::Tap(toggle)));
        }
        crate::style::control_cursor(ui, clickable);
        let hovered = response.hovered() && !self.readonly && !self.touch_mode;
        let pressed = response.is_pointer_button_down_on() && !self.readonly;
        self.paint_checkbox(ui, space, checked, check, hovered, pressed);
        response
    }

    /// A checkbox in `space`: an outline, or the accent fill and check,
    /// animated after a toggle at `check`.
    fn paint_checkbox(
        &self, ui: &Ui, space: Rect, checked: bool, check: Grapheme, hovered: bool, pressed: bool,
    ) {
        let theme = self.ctx.get_lb_theme();
        let secs = match self.task_toggled {
            Some((at_check, at)) if at_check == check => Some((ui.input(|i| i.time) - at) as f32),
            _ => None,
        };
        if secs.is_some_and(|s| s < POP_SECS) {
            ui.ctx().request_repaint();
        }
        let look = Look::at(checked, secs);
        // press snaps in, as other controls do
        let scale = if pressed { look.scale.min(PRESS_SCALE) } else { look.scale };

        // on the pixel grid at rest; in motion the rounding would wobble it
        let radius = Radius::Sm.pts() as f32;
        let box_rect = Rect::from_center_size(space.center(), space.size() * scale);
        let box_rect = if scale == 1.0 {
            box_rect.round_to_pixels(ui.ctx().pixels_per_point())
        } else {
            box_rect
        };
        if look.fill < 1.0 {
            let outline = if hovered { theme.accent() } else { theme.neutral() };
            ui.painter().rect_stroke(
                box_rect,
                radius,
                Stroke::new(STROKE_HAIRLINE, outline),
                StrokeKind::Inside,
            );
        }
        if look.fill > 0.0 {
            let fill = if hovered {
                theme.wash_toward_neutral_fg(theme.accent(), Theme::HOVER_WASH)
            } else {
                theme.accent()
            };
            ui.painter().rect_filled(
                Rect::from_center_size(box_rect.center(), box_rect.size() * look.fill),
                radius * look.fill,
                fill,
            );
        }

        // checked: page colour, which `readable()` keeps legible on every accent
        if look.fill > 0.0 {
            let color = theme.neutral_bg().gamma_multiply(look.fill);
            paint_check(ui, box_rect, look.draw, look.check_scale, color);
        }
        // unchecked and hovered: a faint preview of the click, rising as the fill leaves
        if hovered && !checked {
            let color = theme
                .accent()
                .gamma_multiply(GHOST_ALPHA * (1.0 - look.fill));
            paint_check(ui, box_rect, 1.0, 1.0, color);
        }
    }

    pub fn own_prefix_len_task_item(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<Graphemes> {
        let node_line = self.node_line(node, line);
        let first_line = self.node_first_line(node);
        let first_node_line = self.node_line(node, first_line);
        let first_text = &self.buffer[first_node_line];
        let bracket = self.check_offset(node) - 1;

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let indentation = consume_indent_columns_ceil(first_text, 3);
        let indent_cols = leading_indent_cols_ceil(first_text, 3);

        // "If a sequence of lines Ls constitute a sequence of blocks Bs starting
        // with a non-whitespace character, and M is a list marker of width W
        // followed by 1 ≤ N ≤ 4 spaces, then the result of prepending M and the
        // following spaces to the first line of Ls, and indenting subsequent lines
        // of Ls by W + N spaces, is a list item with Bs as its contents."
        //
        // "If a sequence of lines Ls starting with a single blank line
        // constitute a (possibly empty) sequence of blocks Bs, not separated
        // from each other by more than one blank line, and M is a list marker
        // of width W, then the result of prepending M to the first line of Ls,
        // and indenting subsequent lines of Ls by W + 1 spaces, is a list item
        // with Bs as its contents."
        //
        // Comrak parses `1. [ ] foo` as a `1. ` item whose content begins with
        // `[ ] `, so the content column is W + N, short of the `]`: an indented
        // code block inside the item starts 4 columns past it.
        let content_cols = if first_node_line.contains(bracket, true, true) {
            // tabs stop at columns of the source line, outer prefixes included
            let source_line_idx = self
                .bounds
                .source_lines
                .find_containing(first_line.start(), true, true)
                .start();
            let source_line_start = self.bounds.source_lines[source_line_idx].start();
            let marker_start = first_node_line.start() + indentation;
            columns(&self.buffer[(source_line_start, bracket)])
                - columns(&self.buffer[(source_line_start, marker_start)])
        } else {
            let marker =
                &self.buffer[(first_node_line.start() + indentation, first_node_line.end())];
            columns(marker.trim_end()) + 1
        };

        let len = if node_line.contains(bracket, true, true) {
            // through the `]` and up to 3 spaces after it
            let after = &self.buffer[(bracket + "[ ]".len(), node_line.end())];
            let spaces = after.len() - after.trim_start_matches(' ').len();
            (bracket - node_line.start()).0 + "[ ]".len() + spaces.min(3)
        } else if line == first_line {
            // a blank first line holds only the list marker
            node_line.len().0
        } else {
            // "If a string of lines Ls constitute a list item with contents Bs, then
            // the result of deleting some or all of the indentation from one or
            // more lines in which the next non-whitespace character after the
            // indentation is paragraph continuation text is a list item with the
            // same contents and attributes."
            //
            // "If a line is empty, then it need not be indented."
            //
            // Ceil so a straddling tab is claimed here rather than leaking
            // into a nested block's prefix (per-level tab attribution).
            consume_indent_columns_ceil(&self.buffer[node_line], indent_cols + content_cols)
        };

        Some(Graphemes(len))
    }

    pub fn compute_bounds_task_item(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
            self.bounds.inline_paragraphs.push(line_content);
        }
    }

    /// Where the task's mark sits, between its brackets.
    pub(crate) fn check_offset(&self, node: &'ast AstNode<'ast>) -> Grapheme {
        let NodeValue::TaskItem(task) = &node.data.borrow().value else {
            unreachable!("not a task item")
        };
        self.line_column_to_offset(task.symbol_sourcepos.start)
    }
}

/// Columns `text` spans, tabs stopping every 4.
fn columns(text: &str) -> usize {
    text.chars()
        .fold(0, |cols, c| if c == '\t' { (cols / 4 + 1) * 4 } else { cols + 1 })
}
