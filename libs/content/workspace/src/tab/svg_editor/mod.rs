mod clip;
mod eraser;
mod history;
mod parser;
mod pen;
// mod selection;
mod toolbar;
mod util;
mod zoom;

use crate::tab::svg_editor::toolbar::{ColorSwatch, Toolbar};
use crate::theme::palette::ThemePalette;
use egui::load::SizedTexture;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::Uuid;
pub use parser::Buffer;
pub use pen::CubicBezBuilder;
pub use pen::Pen;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, ImageHrefResolver, ImageKind, Rect, Size};
use std::any::Any;
use std::str::FromStr;
use std::sync::Arc;
pub use toolbar::Tool;
use usvg_parser::Options;
pub use util::node_by_id;

use self::history::History;
use self::util::deserialize_transform;
use self::zoom::handle_zoom_input;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    buffer: parser::Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    content_area: Option<Rect>,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
}

impl SVGEditor {
    pub fn new(bytes: &[u8], core: lb_rs::Core, open_file: Uuid) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = parser::Buffer::new(content);
        let max_id = buffer
            .elements
            .keys()
            .filter_map(|key_str| key_str.parse::<usize>().ok())
            .max()
            .unwrap_or_default()
            + 1;

        let toolbar = Toolbar::new(max_id);

        // Self::define_dynamic_colors(&mut buffer, &mut toolbar, false, true);

        Self {
            buffer,
            history: History::default(),
            toolbar,
            inner_rect: egui::Rect::NOTHING,
            content_area: None,
            core,
            open_file,
            skip_frame: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(
                        ui,
                        &mut self.buffer,
                        &mut self.history,
                        &mut self.skip_frame,
                    );
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });

        // handle_zoom_input(ui, self.inner_rect, &mut self.buffer);

        if ui.input(|r| r.multi_touch().is_some()) || self.skip_frame {
            self.skip_frame = false;
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                if let Some(res) = self.toolbar.pen.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                ) {
                    // let pen::PenResponse::ToggleSelection(id) = res;
                    // self.toolbar.set_tool(Tool::Selection);
                    // self.toolbar.selection.select_el_by_id(
                    //     id.to_string().as_str(),
                    //     ui.ctx().pointer_hover_pos().unwrap_or_default(),
                    //     &mut self.buffer,
                    // );
                }
            }
            _ => {} // Tool::Eraser => {
                    //     self.toolbar.eraser.setup_events(ui, self.inner_rect);
                    //     while let Ok(event) = self.toolbar.eraser.rx.try_recv() {
                    //         self.toolbar.eraser.handle_events(event, &mut self.buffer);
                    //     }
                    // }
                    // Tool::Selection => {
                    //     self.toolbar
                    //         .selection
                    //         .handle_input(ui, self.inner_rect, &mut self.buffer);
                    // }
        }

        // self.handle_clip_input(ui);

        // Self::define_dynamic_colors(
        //     &mut self.buffer,
        //     &mut self.toolbar,
        //     ui.visuals().dark_mode,
        //     false,
        // );
    }

    pub fn get_minimal_content(&self) -> String {
        "".to_string()
        // self.buffer.to_string()
    }

    fn render_svg(&mut self, ui: &mut egui::Ui) {
        for el in self.buffer.elements.values() {
            match el {
                parser::Element::Path(path) => {
                    if path.data.len() < 1 {
                        continue;
                    }
                    path.data.iter().for_each(|bezier| {
                        let bezier = bezier.to_cubic();
                        let points: Vec<egui::Pos2> = bezier
                            .get_points()
                            .map(|dvec| egui::pos2(dvec.x as f32, dvec.y as f32))
                            .collect();
                        let epath = epaint::CubicBezierShape::from_points_stroke(
                            points.try_into().unwrap(),
                            false,
                            egui::Color32::TRANSPARENT,
                            egui::Stroke { width: 2.0, color: egui::Color32::BLACK }, // todo determine stroke thickness based on scale
                        );
                        ui.painter().add(epath);
                    });
                }
                parser::Element::Image(img) => todo!(),
                parser::Element::Text(text) => todo!(),
            }
        }
    }

    // if the data-dark mode is different from the ui dark mode, or if this is the first time running the editor
    // fn define_dynamic_colors(
    //     buffer: &mut Buffer, toolbar: &mut Toolbar, is_dark_mode: bool, force_update: bool,
    // ) {
    //     let needs_update;
    //     if let Some(svg_flag) = buffer.current.attr("data-dark-mode") {
    //         let svg_flag: bool = svg_flag.parse().unwrap_or(false);

    //         needs_update = svg_flag != is_dark_mode;
    //     } else {
    //         needs_update = true;
    //     }

    //     if !needs_update && !force_update {
    //         return;
    //     }

    //     let gradient_group_id = "lb:gg";
    //     buffer.current.remove_child(gradient_group_id);

    //     let theme_colors = ThemePalette::as_array(is_dark_mode);
    //     if toolbar.pen.active_color.is_none() {
    //         toolbar.pen.active_color = Some(ColorSwatch {
    //             id: "fg".to_string(),
    //             color: theme_colors.iter().find(|p| p.0.eq("fg")).unwrap().1,
    //         });
    //     }

    //     let mut gradient_group = Element::builder("g", "")
    //         .attr("id", gradient_group_id)
    //         .build();

    //     theme_colors.iter().for_each(|theme_color| {
    //         let rgb_color =
    //             format!("rgb({} {} {})", theme_color.1.r(), theme_color.1.g(), theme_color.1.b());
    //         let gradient = Element::builder("linearGradient", "")
    //             .attr("id", theme_color.0.as_str())
    //             .append(
    //                 Element::builder("stop", "")
    //                     .attr("stop-color", rgb_color)
    //                     .build(),
    //             )
    //             .build();
    //         gradient_group.append_child(gradient);
    //     });

    //     buffer.current.append_child(gradient_group);
    //     buffer
    //         .current
    //         .set_attr("data-dark-mode", format!("{}", is_dark_mode));
    // }
}
