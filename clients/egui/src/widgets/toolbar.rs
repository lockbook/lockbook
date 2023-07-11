use std::borrow::BorrowMut;

use eframe::egui;
use egui_winit::egui::Color32;
use lbeditor::{
    element::Element,
    input::canonical::{Modification, Region},
    Editor,
};

use crate::theme::Icon;

use super::Button;

struct ToolbarButton {
    icon: Icon,
    callback: fn(&mut Editor),
}
struct Toolbar {
    margin: egui::Margin,
    buttons: Vec<ToolbarButton>,
}
impl Toolbar {
    fn new() -> Self {
        let buttons = vec![
            ToolbarButton {
                icon: Icon::HEADER_1,
                callback: |e| {
                    e.custom_events.push(Modification::Heading(1));
                },
            },
            ToolbarButton {
                icon: Icon::BOLD,
                callback: |e| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::Strong,
                    })
                },
            },
            ToolbarButton {
                icon: Icon::ITALIC,
                callback: |e| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::Emphasis,
                    })
                },
            },
            ToolbarButton {
                icon: Icon::CODE,
                callback: |e| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::InlineCode,
                    })
                },
            },
            ToolbarButton {
                icon: Icon::NUMBER_LIST,
                callback: |e| e.custom_events.push(Modification::NumberListItem),
            },
            ToolbarButton {
                icon: Icon::TODO_LIST,
                callback: |e| e.custom_events.push(Modification::TodoListItem),
            },
        ];
        Toolbar { margin: egui::Margin::symmetric(15.0, 10.0), buttons }
    }
    fn width(&self) -> f32 {
        let icon_width = 44;
        let width = icon_width * self.buttons.len() + self.margin.sum().x as usize;
        width as f32
    }
}

pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor) {
    let toolbar = Toolbar::new();
    println!("{:#?} max rect screen", ui.max_rect());
    println!("{:#?} editor ui rect", editor.ui_rect);
    println!("{:#?} editor scroll area rect", editor.scroll_area_rect);

    println!("--\n\n",);
    let rect = egui::Rect {
        min: egui::Pos2 {
            x: (editor.ui_rect.width() - toolbar.width()) / 2.0 + editor.ui_rect.left(),
            y: editor.ui_rect.bottom() - 100.0,
        },
        max: egui::Pos2 {
            x: editor.ui_rect.right() - (editor.ui_rect.width() - toolbar.width()) / 2.0,
            y: editor.ui_rect.bottom(),
        },
    };
    println!("{:#?} toolbar rect", rect);

    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::default()
            .fill(ui.visuals().code_bg_color)
            .inner_margin(toolbar.margin)
            .shadow(egui::epaint::Shadow {
                extrusion: ui.visuals().window_shadow.extrusion,
                color: ui.visuals().window_shadow.color.gamma_multiply(0.3),
            })
            .rounding(egui::Rounding::same(20.0))
            .show(ui, |ui| map_buttons(ui, editor, toolbar.buttons))
    });
}

fn map_buttons(ui: &mut egui::Ui, editor: &mut Editor, buttons: Vec<ToolbarButton>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);

        buttons.iter().for_each(|btn| {
            if Button::default().icon(&btn.icon).show(ui).clicked() {
                (btn.callback)(editor);
            }
        });
    });
}
