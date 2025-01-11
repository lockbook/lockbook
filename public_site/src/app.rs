use std::sync::Arc;

use egui::{FontData, FontFamily};
use futures::executor::block_on;
use lb_rs::{blocking::Lb, model::core_config::Config, Uuid};
use workspace_rs::{
    tab::{markdown_editor::Editor, svg_editor::SVGEditor},
    workspace::{Workspace, WsConfig},
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct LbWebApp {
    workspace: Workspace,
    editor: Option<Editor>,
    canvas: Option<SVGEditor>,
    label: String,
}

impl LbWebApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();

        let config = WsConfig::new("/".into(), false, false, true);
        let lb = Lb::init(Config {
            logs: false,
            colored_logs: false,
            writeable_path: "".into(),
            background_work: false,
        })
        .unwrap();

        let martian = include_bytes!("../assets/martian.ttf");
        let martian_bold = include_bytes!("../assets/martian-bold.ttf");
        let mut fonts = egui::FontDefinitions::default();

        workspace_rs::register_fonts(&mut fonts);
        fonts
            .font_data
            .insert("pt_sans".to_string(), FontData::from_static(martian));
        fonts
            .font_data
            .insert("pt_mono".to_string(), FontData::from_static(martian));
        fonts
            .font_data
            .insert("pt_bold".to_string(), FontData::from_static(martian_bold));

        fonts
            .families
            .insert(FontFamily::Name(Arc::from("Bold")), vec!["pt_bold".to_string()]);

        ctx.set_fonts(fonts);
        ctx.set_zoom_factor(0.8);
        Self {
            workspace: Workspace::new(config, &lb, &ctx),
            label: "hey there".to_owned(),
            editor: None,
            canvas: None,
        }
    }
}

impl eframe::App for LbWebApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // maybe persist the demo document here.
        // eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| {
                if self.canvas.is_none(){
                    self.canvas = Some(
                        SVGEditor::new(&[], &ctx.clone(), self.workspace.core.clone(), Uuid::new_v4(), None, None)
                    )
                }
                if self.editor.is_none() {
                    self.editor = Some(Editor::new(
                        self.workspace.core.clone(),
                        r#"# Hello web surfer

Welcome to Lockbook! This is an example note to help you get started with our note editor. You can keep it to use as a cheat sheet or delete it anytime.

Lockbook uses Markdown, a lightweight language for formatting plain text. You can use all our supported formatting just by typing. Here’s how it works:

# This is a heading

## This is a smaller heading

### This is an even smaller heading

###### Headings have 6 levels

For italic, use single *asterisks* or _underscores_.

For bold, use double **asterisks** or __underscores__.

For inline code, use single `backticks`

For code blocks, use
```
triple
backticks
```

>For block quotes,
use a greater-than sign

Bulleted list items
* start
* with
* asterisks
- or
- hyphens
+ or
+ plus
+ signs

Numbered list items
1. start
2. with
3. numbers
4. and
5. periods

Happy note taking! You can report any issues to our [Github project](https://github.com/lockbook/lockbook/issues/new) or join our [Discord server](https://discord.gg/qv9fmAZCm6)."#,
                        Uuid::new_v4(),
                        None,
                        false,
                        false,
                    ));
                }
                // if let Some(md) = &mut self.editor {
                //     ui.centered_and_justified(|ui| {
                //         ui.vertical(|ui| {
                //             ui.centered_and_justified(|ui| {
                //                 md.show(ui);
                //             });
                //         });
                //     });
                // }
                if let Some(svg) = &mut self.canvas {
                    ui.centered_and_justified(|ui| {
                        ui.vertical(|ui| {
                            ui.centered_and_justified(|ui| {
                                svg.show(ui);
                            });
                        });
                    });
                }

                // ui.ctx().memory_mut(|r| {
                //     r.request_focus(md.id());
                //     log::debug!("widget with focus {:#?}", r.focused());
                // });
                // ui.text_edit_multiline(&mut self.label);
            });
    }
}
