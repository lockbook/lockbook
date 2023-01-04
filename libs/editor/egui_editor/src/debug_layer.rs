use crate::ast::Ast;
use crate::editor::Editor;
use crate::theme::YELLOW;
use egui::{Align2, Color32, FontId, Pos2, Rounding, Stroke, Ui};
use std::time::Instant;

pub struct DebugLayer {
    pub enabled: bool,

    pub frame_count: usize,
    pub frame_start: Instant,
}

impl Default for DebugLayer {
    fn default() -> Self {
        Self { frame_count: 0, enabled: false, frame_start: Instant::now() }
    }
}

impl DebugLayer {
    pub fn frame_start(&mut self) {
        self.frame_start = Instant::now();
        self.frame_count += 1;
    }

    pub fn ms_elapsed(&self) -> u128 {
        (Instant::now() - self.frame_start).as_millis()
    }
}

impl Editor {
    pub fn debug_layer(&mut self, ui: &mut Ui) {
        if self.debug.enabled {
            self.show_layer(ui);
        }
    }

    pub fn show_layer(&mut self, ui: &mut Ui) {
        for galley in &self.galleys {
            let galley_rect = galley.galley.rect.translate(galley.text_location.to_vec2());
            ui.painter().rect(
                galley_rect,
                Rounding::none(),
                Color32::TRANSPARENT,
                Stroke { width: 0.5, color: YELLOW.light },
            );
            let line_pt_1 =
                Pos2::new(galley_rect.max.x, (galley_rect.max.y + galley_rect.min.y) / 2.0);
            let line_pt_2 =
                Pos2::new(galley_rect.max.x + 40.0, (galley_rect.max.y + galley_rect.min.y) / 2.0);
            ui.painter()
                .line_segment([line_pt_1, line_pt_2], Stroke { width: 0.5, color: YELLOW.light });

            ui.painter().text(
                line_pt_2,
                Align2::LEFT_CENTER,
                format!("{}-{}", galley.range.start.0, galley.range.end.0),
                FontId::default(),
                YELLOW.light,
            );

            let screen_size = format!(
                "screen: {} x {}",
                ui.ctx().input().screen_rect.width(),
                ui.ctx().input().screen_rect.height()
            );

            let doc_info = format!("last_cursor_position: {}", self.last_cursor_position().0);

            let cursor_info = format!(
                "character: {}, byte: {}, x_target: {}, selection_origin: {}",
                self.cursor.loc.0,
                self.char_offset_to_byte(self.cursor.loc).0,
                self.cursor
                    .x_target
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "None".to_string()),
                self.cursor
                    .selection_origin
                    .map(|x| x.0.to_string())
                    .unwrap_or_else(|| "None".to_string()),
            );

            let frames =
                format!("frame #{}, {}ms", self.debug.frame_count, self.debug.ms_elapsed());

            let output = format!("{}\n{}\n{}\n{}", doc_info, cursor_info, screen_size, frames);

            let loc = ui.input().screen_rect.max;
            ui.painter()
                .text(loc, Align2::RIGHT_BOTTOM, output, FontId::default(), YELLOW.light);
        }
    }

    pub fn print_ast(&self) {
        Self::print_ast_helper(&self.ast, self.ast.root, &self.raw, 0);
    }

    pub fn print_ast_helper(ast: &Ast, node: usize, raw: &str, nest: usize) {
        let node = &ast.nodes[node];
        let indent = "->".repeat(nest);
        println!("{indent}element: {:?}", node.element);
        println!("{indent}range: {}", &raw[node.range.start.0..node.range.end.0]);

        for &child in &node.children {
            Self::print_ast_helper(ast, child, raw, nest + 1);
        }
    }
}
