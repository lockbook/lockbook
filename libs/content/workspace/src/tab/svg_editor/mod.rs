mod clip;
mod eraser;
mod history;
mod pen;
mod selection;
mod toolbar;
mod util;
mod zoom;

use crate::tab::svg_editor::toolbar::{ColorSwatch, Component, Toolbar};
use crate::theme::palette::ThemePalette;
pub use eraser::Eraser;
pub use history::Buffer;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use minidom::Element;
pub use pen::CubicBezBuilder;
pub use pen::Pen;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, ImageHrefResolver, ImageKind, Size};
use std::str::FromStr;
use std::sync::Arc;
pub use toolbar::Tool;
use usvg_parser::Options;
pub use util::node_by_id;

use self::clip::handle_clip_input;
use self::zoom::handle_zoom_input;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    buffer: Buffer,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    core: lb_rs::Core,
}

impl SVGEditor {
    pub fn new(bytes: &[u8], core: lb_rs::Core) -> Self {
        // todo: handle invalid utf8
        let mut content = std::str::from_utf8(bytes).unwrap().to_string();
        if content.is_empty() {
            content = "<svg xmlns=\"http://www.w3.org/2000/svg\" ></svg>".to_string();
        } else {
            content = content.replace("xlink:href", "href"); //risky
        }

        let root: Element = content.parse().unwrap();
        let mut buffer = Buffer::new(root);

        let max_id = buffer
            .current
            .children()
            .map(|el| {
                let id: usize = el.attr("id").unwrap_or("0").parse().unwrap_or_default();
                id
            })
            .max_by(|x, y| x.cmp(y))
            .unwrap_or_default()
            + 1;

        let mut toolbar = Toolbar::new(max_id);

        Self::define_dynamic_colors(&mut buffer, &mut toolbar, false, true);

        Self { buffer, toolbar, inner_rect: egui::Rect::NOTHING, core }
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
                    self.toolbar.show(ui, &mut self.buffer, self.inner_rect);
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });

        handle_zoom_input(ui, self.inner_rect, &mut self.buffer);

        if ui.input(|r| r.multi_touch().is_some()) {
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                self.toolbar
                    .pen
                    .handle_input(ui, self.inner_rect, &mut self.buffer);
            }
            Tool::Eraser => {
                self.toolbar.eraser.setup_events(ui, self.inner_rect);
                while let Ok(event) = self.toolbar.eraser.rx.try_recv() {
                    self.toolbar.eraser.handle_events(event, &mut self.buffer);
                }
            }
            Tool::Selection => {
                self.toolbar
                    .selection
                    .handle_input(ui, self.inner_rect, &mut self.buffer);
            }
        }

        handle_clip_input(ui, &mut self.buffer);

        Self::define_dynamic_colors(
            &mut self.buffer,
            &mut self.toolbar,
            ui.visuals().dark_mode,
            false,
        );
    }

    pub fn get_minimal_content(&self) -> String {
        self.buffer.to_string()
    }

    fn render_svg(&mut self, ui: &mut egui::Ui) {
        let lb_local_resolver = ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Self::lb_local_resolver(&self.core),
        };

        let options =
            usvg::Options { image_href_resolver: lb_local_resolver, ..Default::default() };

        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_str(&self.buffer.to_string(), &options).unwrap();
        let available_rect = ui.available_rect_before_wrap();
        utree.size = Size::from_wh(available_rect.width(), available_rect.height()).unwrap();

        utree.view_box.rect = usvg::NonZeroRect::from_ltrb(
            available_rect.left(),
            available_rect.top(),
            available_rect.right(),
            available_rect.bottom(),
        )
        .unwrap();

        if self.buffer.needs_path_map_update {
            self.buffer.recalc_paths(&utree);
        }

        let tree = resvg::Tree::from_usvg(&utree);

        let pixmap_size = tree.size.to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(usvg::Transform::default(), &mut pixmap.as_mut());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );

        let texture = ui
            .ctx()
            .load_texture("svg_image", image, egui::TextureOptions::LINEAR);

        ui.add(
            egui::Image::new(
                &texture,
                egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
            )
            .sense(egui::Sense::click()),
        );
    }

    fn lb_local_resolver(core: &lb_rs::Core) -> ImageHrefStringResolverFn {
        let lb_link_prefix = "lb://";
        let core = core.clone();
        Box::new(move |href: &str, _opts: &Options| {
            if !href.starts_with(lb_link_prefix) {
                return None;
            }
            let id = &href[lb_link_prefix.len()..];
            let id = lb_rs::Uuid::from_str(id).ok()?;
            let raw = core.read_document(id).ok()?;

            let name = core.get_file_by_id(id).ok()?.name;
            let ext = name.split('.').last().unwrap_or_default();
            match ext {
                "jpg" | "jpeg" => Some(ImageKind::JPEG(Arc::new(raw))),
                "png" => Some(ImageKind::PNG(Arc::new(raw))),
                // "svg" => Some(ImageKind::SVG(Arc::new(raw))), todo: handle nested svg
                "gif" => Some(ImageKind::GIF(Arc::new(raw))),
                _ => None,
            }
        })
    }

    // if the data-dark mode is different from the ui dark mode, or if this is the first time running the editor
    fn define_dynamic_colors(
        buffer: &mut Buffer, toolbar: &mut Toolbar, is_dark_mode: bool, force_update: bool,
    ) {
        let needs_update;
        if let Some(svg_flag) = buffer.current.attr("data-dark-mode") {
            let svg_flag: bool = svg_flag.parse().unwrap_or(false);

            needs_update = svg_flag != is_dark_mode;
        } else {
            needs_update = true;
        }

        if !needs_update && !force_update {
            return;
        }

        let gradient_group_id = "lb:gg";
        buffer.current.remove_child(gradient_group_id);

        let theme_colors = ThemePalette::as_array(is_dark_mode);
        if toolbar.pen.active_color.is_none() {
            toolbar.pen.active_color = Some(ColorSwatch {
                id: "fg".to_string(),
                color: theme_colors.iter().find(|p| p.0.eq("fg")).unwrap().1,
            });
        }

        let btns = theme_colors.iter().map(|theme_color| {
            Component::ColorSwatch(ColorSwatch { id: theme_color.0.clone(), color: theme_color.1 })
        });
        toolbar.components = toolbar
            .components
            .clone()
            .into_iter()
            .filter(|c| !matches!(c, Component::ColorSwatch(_)))
            .chain(btns)
            .collect();

        let mut gradient_group = Element::builder("g", "")
            .attr("id", gradient_group_id)
            .build();

        theme_colors.iter().for_each(|theme_color| {
            let rgb_color =
                format!("rgb({} {} {})", theme_color.1.r(), theme_color.1.g(), theme_color.1.b());
            let gradient = Element::builder("linearGradient", "")
                .attr("id", theme_color.0.as_str())
                .append(
                    Element::builder("stop", "")
                        .attr("stop-color", rgb_color)
                        .build(),
                )
                .build();
            gradient_group.append_child(gradient);
        });

        buffer.current.append_child(gradient_group);
        buffer
            .current
            .set_attr("data-dark-mode", format!("{}", is_dark_mode));
    }
}
