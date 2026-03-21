use comrak::nodes::AstNode;
use egui::{Color32, Pos2, Ui};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _, RangeIterExt as _};
use syntect::easy::HighlightLines;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> Editor {
    pub fn text_format_document(&self) -> Format {
        Format {
            family: FontFamily::Sans,
            bold: false,
            italic: false,
            color: self.ctx.get_lb_theme().neutral_fg(),
            underline: false,
            strikethrough: false,
            background: egui::Color32::TRANSPARENT,
            border: egui::Color32::TRANSPARENT,
            spoiler: false,
            superscript: false,
            subscript: false,
        }
    }

    pub fn height_document(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext_mode {
            self.block_children_height(node)
        } else {
            let highlighter_syntax = self.syntax_set.find_syntax_by_extension(&self.syntax_ext);
            let mut result = 0.;
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let mut wrap = self.new_wrap(width);
                if let Some(syntax) = highlighter_syntax {
                    let mut highlighter = HighlightLines::new(syntax, &self.syntax_theme);
                    let line_text = &self.buffer[line];
                    let regions = if let Some(regions) = self.syntax.get(line_text, line) {
                        regions
                    } else {
                        let mut regions = Vec::new();
                        let mut region_start = self.offset_to_byte(line.start());
                        for (style, region_str) in highlighter
                            .highlight_line(line_text, &self.syntax_set)
                            .unwrap()
                        {
                            let region_end = region_start + region_str.len();
                            let region = self.range_to_char((region_start, region_end));
                            regions.push((style, region));
                            region_start = region_end;
                        }
                        regions
                    };
                    for (_, region) in regions {
                        wrap.offset += self.span_section(&wrap, region, self.text_format_syntax());
                    }
                } else {
                    wrap.offset += self.span_section(&wrap, line, self.text_format_syntax());
                }
                result += wrap.height();
                result += self.layout.row_spacing;
            }
            result
        }
    }

    pub fn show_document(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext_mode {
            self.show_block_children(ui, node, top_left);
        } else {
            let has_syntax = self
                .syntax_set
                .find_syntax_by_extension(&self.syntax_ext)
                .is_some();
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let mut wrap = self.new_wrap(width);

                if has_syntax {
                    let syntax = self
                        .syntax_set
                        .find_syntax_by_extension(&self.syntax_ext)
                        .unwrap();
                    let mut highlighter = HighlightLines::new(syntax, &self.syntax_theme);
                    let line_text = &self.buffer[line];

                    let regions = if let Some(regions) = self.syntax.get(line_text, line) {
                        regions
                    } else {
                        let mut regions = Vec::new();
                        let mut region_start = self.offset_to_byte(line.start());
                        for (style, region_str) in highlighter
                            .highlight_line(line_text, &self.syntax_set)
                            .unwrap()
                        {
                            let region_end = region_start + region_str.len();
                            let region = self.range_to_char((region_start, region_end));
                            regions.push((style, region));
                            region_start = region_end;
                        }
                        self.syntax.insert(line_text.into(), line, regions.clone());
                        regions
                    };

                    let mut text_format = self.text_format_syntax();
                    if regions.is_empty() {
                        self.show_section(
                            ui,
                            top_left,
                            &mut wrap,
                            line.start().into_range(),
                            text_format.clone(),
                        );
                    }
                    for (style, region) in regions {
                        let theme = self.ctx.get_lb_theme();
                        let hex = Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        )
                        .to_hex();
                        let hex = hex.strip_suffix("ff").unwrap();
                        text_format.color = match hex {
                            "#000000" => theme.neutral_fg(),
                            "#111111" => theme.neutral_fg_secondary(),
                            "#222222" => theme.fg().get_color(theme.prefs().primary),
                            "#333333" => theme.fg().get_color(theme.prefs().secondary),
                            "#444444" => theme.fg().get_color(theme.prefs().tertiary),
                            _ => theme.neutral_fg(),
                        };
                        self.show_section(ui, top_left, &mut wrap, region, text_format.clone());
                    }
                } else {
                    self.show_section(ui, top_left, &mut wrap, line, self.text_format_syntax());
                }

                top_left.y += wrap.height();
                top_left.y += self.layout.row_spacing;
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            }
        }
    }

    pub fn compute_bounds_document(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                self.bounds.inline_paragraphs.push(line);
            }
        }
    }
}
