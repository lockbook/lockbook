use egui::{Align, Area, Id, Layout, Order, Stroke, StrokeKind};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::interact::{ControlFills, interact_fill, sense_click};
use crate::components::{
    Button, EqualCells, FG_HOVER, FG_PRESS, Field, Radius, SheetFooterOpts, Space, Spacer, Theme,
    TypeRole, claim, display_file_name, measure_file_name, origin, paint_file_name, phosphor,
    phosphor_ui_font_id, place_at, segmented, segmented_width, sheet_band, sheet_band_centered,
    sheet_dim, sheet_equal_row, sheet_footer, sheet_panel_fixed, sheet_title_muted, shortcut_enter,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, CreateKind, CreateLoc, Modal};

/// Wide enough for location plates (username · Alongside {note} · Choose…).
const CREATE_SHEET_W: f32 = 480.0;

fn create_sheet_h_key() -> Id {
    Id::new("shell_create_inner_h")
}

/// Fixed footer band shared by form + location wizard (Md + control row).
/// Summary sits in the body with Xl above it; Md groups copy with the actions.
fn create_footer_band_h() -> f32 {
    Space::Md.pts() + crate::components::control_height()
}

/// Create sheet: type · name · compact location, or a same-size location wizard.
///
/// **Measure / draw:** first form frame lays out naturally and locks that
/// inner height once. Later frames draw into a fixed slot:
/// `body (exact residual) · Space::Md · footer (control_height)`.
/// Form and Choose… swap only the body; plate size stays put.
pub(crate) fn show_create(
    app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>,
) {
    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_create"));
    if sheet_dim(ctx, Id::new("shell_create_dim"), layer) {
        queue.push(A::CloseModal);
    }

    let (parent, loc, alongside, picking) = match &app.modal {
        Some(Modal::Create { parent, loc, alongside, picking, .. }) => {
            (*parent, *loc, alongside.clone(), *picking)
        }
        _ => return,
    };

    // Folder tree expand seed when the location wizard is first opened.
    let exp_id = Id::new("shell_create_folder_exp");
    let mut expanded: std::collections::HashSet<Uuid> =
        ctx.data(|d| d.get_temp(exp_id)).unwrap_or_default();
    if picking && expanded.is_empty() {
        if let Some(ready) = app.session.ready() {
            let files = ready.workspace.files.read().unwrap();
            expanded.insert(files.root().id);
            let focus = match loc {
                CreateLoc::Alongside => alongside.as_ref().map(|(id, _)| *id).or(parent),
                _ => parent,
            };
            if let Some(p) = focus {
                super::tree::expand_ancestors_of(&*files, p, &mut expanded);
            }
        }
        ctx.data_mut(|d| {
            d.remove::<bool>(super::tree::folder_tree_scroll_key("shell_create_folder_tree"));
        });
    }

    let locked_h = ctx.data(|d| d.get_temp::<f32>(create_sheet_h_key()));

    Area::new(Id::new("shell_create"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fixed(ui, t, CREATE_SHEET_W, locked_h.unwrap_or(280.0), |ui| {
                match locked_h {
                    None if !picking => {
                        // Measure pass: natural form (body + Md + footer).
                        let measured = ui
                            .scope(|ui| {
                                create_form_body(app, ui, t, queue);
                                ui.add(Spacer::new(Space::Md));
                                create_form_footer(app, ui, t, queue);
                            })
                            .response
                            .rect
                            .height()
                            .max(1.0);
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(create_sheet_h_key(), measured));
                    }
                    Some(h) => {
                        // Draw pass: fixed plate — body residual + Md + footer.
                        ui.set_height(h);
                        let foot_h = crate::components::control_height();
                        let body_h = (h - create_footer_band_h()).max(1.0);

                        sheet_band(ui, body_h, |ui| {
                            if picking {
                                create_location_body(
                                    app,
                                    ui,
                                    t,
                                    queue,
                                    parent,
                                    loc,
                                    alongside.as_ref(),
                                    &mut expanded,
                                );
                            } else {
                                create_form_body(app, ui, t, queue);
                            }
                        });
                        ui.add(Spacer::new(Space::Md));
                        sheet_band(ui, foot_h, |ui| {
                            if picking {
                                create_location_footer(ui, t, queue);
                            } else {
                                create_form_footer(app, ui, t, queue);
                            }
                        });
                    }
                    // Picking before measure shouldn't happen; fall back to natural form.
                    None => {
                        create_form_body(app, ui, t, queue);
                        ui.add(Spacer::new(Space::Md));
                        create_form_footer(app, ui, t, queue);
                    }
                }
            });
        });

    if picking {
        ctx.data_mut(|d| d.insert_temp(exp_id, expanded));
    }
}

/// Create form body (no footer). Top-aligned inside a fixed body slot when locked.
fn create_form_body(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>) {
    let (kind, parent, loc, alongside, chosen, error) = match &app.modal {
        Some(Modal::Create { kind, parent, loc, alongside, chosen, error, .. }) => {
            (*kind, *parent, *loc, alongside.clone(), *chosen, error.clone())
        }
        _ => return,
    };
    let mut kind_i = kind.index();
    let labels: Vec<&str> = CreateKind::ALL.iter().map(|k| k.label()).collect();
    let edit_id = Id::new("shell_create_field").with("edit");
    let need_focus = ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::new("shell_create_need_focus"))
            .unwrap_or(false)
    });

    let root_id = app
        .session
        .ready()
        .map(|r| r.workspace.files.read().unwrap().root().id);

    let root_sel = matches!(loc, CreateLoc::Root);
    let along_sel = matches!(loc, CreateLoc::Alongside);
    let custom_sel = matches!(loc, CreateLoc::Custom) && chosen.is_some() && chosen != root_id;

    if sheet_title_muted(ui, t, "Create") {
        queue.push(A::CloseModal);
    }
    ui.add(Spacer::new(Space::Md));

    // Type + name share one centered column (segmented natural width). Location
    // plates keep the full sheet.
    let col_w = segmented_width(ui, t, &labels);
    sheet_band_centered(ui, crate::components::segmented_h(), |ui| {
        if segmented(ui, t, &labels, &mut kind_i).changed() {
            queue.push(A::CreateSetKind(CreateKind::from_index(kind_i)));
        }
    });
    ui.add(Spacer::new(Space::Md));

    let total = crate::components::ui_width(ui);
    let top_left = origin(ui);
    let col_left = top_left.x + ((total - col_w) / 2.0).max(0.0);
    let budget = egui::Rect::from_min_size(
        egui::pos2(col_left, top_left.y),
        egui::vec2(col_w.max(1.0), crate::components::remaining_height(ui).max(1.0)),
    );
    let (_, used) = place_at(ui, budget, Layout::top_down(Align::Min), |ui| {
        ui.set_width(col_w.max(1.0));
        ui.label(TypeRole::Body.rich("Name").color(t.neutral_fg_secondary()));
        ui.add(Spacer::new(Space::Xs));

        let Some(Modal::Create { name, name_dirty, error, .. }) = &mut app.modal else {
            return;
        };
        let before = name.clone();
        let mut field = Field::new(t, name)
            .id("shell_create_field")
            .width(col_w)
            .select_all_on_focus(true);
        if let Some(ext) = kind.ext() {
            field = field.trailing_static(ext);
        }
        let _ = field.show(ui);
        if need_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            if ui.memory(|m| m.has_focus(edit_id)) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::new("shell_create_need_focus"), false);
                });
            }
        }
        if *name != before {
            *name_dirty = true;
            *error = None;
        }
    });
    claim(ui, egui::Rect::from_min_size(top_left, egui::vec2(total, used.height().max(1.0))));

    ui.add(Spacer::new(Space::Md));
    ui.label(
        TypeRole::Body
            .rich("Location")
            .color(t.neutral_fg_secondary()),
    );
    ui.add(Spacer::new(Space::Xs));

    // Exclusive location plates: unselected = surface rest; selected = canvas
    // + accent hairline (destination, not the type segmented).
    // Compact labels keep natural width; name-bearing plates take the leftover so
    // “Alongside {note}” can show a real title instead of permanent ellipsis.
    let username = app
        .session
        .ready()
        .map(|r| r.workspace.account.username.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Home".into());
    let custom_folder = chosen.filter(|id| Some(*id) != root_id).and_then(|id| {
        app.session.ready().and_then(|r| {
            r.workspace
                .files
                .read()
                .unwrap()
                .get_by_id(id)
                .map(|f| f.name.clone())
        })
    });

    let mut plates: Vec<CreateLocPlate> = Vec::new();
    plates.push(CreateLocPlate {
        icon: phosphor::FOLDER,
        label: LocPlateLabel::Text(username),
        selected: root_sel,
        flex: false,
        action: CreateLocPlateAction::Set(CreateLoc::Root),
    });
    if let Some((_, doc_name)) = alongside.as_ref() {
        plates.push(CreateLocPlate {
            icon: phosphor::ARROW_BEND_DOWN_RIGHT,
            label: LocPlateLabel::Alongside(display_file_name(doc_name).to_owned()),
            selected: along_sel,
            flex: true,
            action: CreateLocPlateAction::Set(CreateLoc::Alongside),
        });
    }
    // Third slot is always flex (Choose… or a picked folder). Flipping flex
    // on pick used to steal width from Alongside. A chosen folder stays on
    // the plate even when Home / Alongside is selected; click selects it,
    // click-again (already Custom) reopens the picker.
    if let Some(folder) = custom_folder {
        plates.push(CreateLocPlate {
            icon: phosphor::FOLDER,
            label: LocPlateLabel::FileName(folder),
            selected: custom_sel,
            flex: true,
            action: if custom_sel {
                CreateLocPlateAction::Pick
            } else {
                CreateLocPlateAction::Set(CreateLoc::Custom)
            },
        });
    } else {
        plates.push(CreateLocPlate {
            icon: phosphor::FOLDERS,
            label: LocPlateLabel::Text("Choose…".into()),
            selected: false,
            flex: true,
            action: CreateLocPlateAction::Pick,
        });
    }

    // Inter-plate gap from EqualCells only (measure + draw share CHIP_GAP).
    let gap = EqualCells::gap_pts();
    let total = crate::components::ui_width(ui);
    let n = plates.len().max(1);
    let gaps = gap * (n as f32 - 1.0).max(0.0);
    let mut widths: Vec<f32> = plates
        .iter()
        .map(|p| create_loc_plate_natural_w(ui, t, p))
        .collect();
    let fixed: f32 = plates
        .iter()
        .zip(widths.iter())
        .filter(|(p, _)| !p.flex)
        .map(|(_, w)| *w)
        .sum();
    let flex_n = plates.iter().filter(|p| p.flex).count().max(1);
    let flex_budget = (total - gaps - fixed).max(0.0);
    let flex_each = flex_budget / flex_n as f32;
    for (p, w) in plates.iter().zip(widths.iter_mut()) {
        if p.flex {
            // Prefer leftover, but never thinner than a usable name slot.
            *w = flex_each.max(Space::Xl.pts() * 2.5);
        }
    }
    // If flex mins overflow, scale all down proportionally.
    let sum: f32 = widths.iter().sum::<f32>() + gaps;
    if sum > total && sum > 0.0 {
        let scale = (total - gaps).max(0.0) / (sum - gaps).max(1.0);
        for w in &mut widths {
            *w *= scale;
        }
    }

    let plates_snap = plates;
    sheet_equal_row(ui, LOC_PLATE_H, &widths, EqualCells::gap_token(), |ui, i| {
        let plate = &plates_snap[i];
        let action = plate.action;
        if create_loc_plate(ui, t, plate).clicked() {
            match action {
                CreateLocPlateAction::Set(loc) => queue.push(A::CreateSetLoc(loc)),
                CreateLocPlateAction::Pick => queue.push(A::CreateSetPicking(true)),
            }
        }
    });

    ui.add(Spacer::new(Space::Xl));
    let name_snap = match &app.modal {
        Some(Modal::Create { name, .. }) => name.clone(),
        _ => String::new(),
    };
    create_summary_line(ui, t, app, kind, name_snap.trim(), parent);

    if let Some(err) = error.as_deref().filter(|e| !e.is_empty()) {
        ui.add(Spacer::new(Space::Sm));
        ui.label(TypeRole::Body.rich(err).color(t.danger()));
    }
}

fn create_form_footer(app: &ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>) {
    let name_ok = match &app.modal {
        Some(Modal::Create { name, .. }) => !name.trim().is_empty(),
        _ => false,
    };
    let foot = sheet_footer(
        ui,
        t,
        "Create",
        SheetFooterOpts::default()
            .divider(false)
            .primary_enabled(name_ok)
            .primary_shortcut(shortcut_enter()),
    );
    if foot.cancel {
        queue.push(A::CloseModal);
    }
    if foot.primary {
        queue.push(A::ConfirmCreate);
    }
}

#[derive(Clone, Copy)]
enum CreateLocPlateAction {
    Set(CreateLoc),
    Pick,
}

/// Label content for a location plate.
enum LocPlateLabel {
    /// Plain body text (username, “Choose…”).
    Text(String),
    /// “Alongside ” + emoji-safe file name (name gets the residual width).
    Alongside(String),
    /// Folder / file name only (Glyphon).
    FileName(String),
}

struct CreateLocPlate {
    icon: &'static str,
    label: LocPlateLabel,
    selected: bool,
    /// Take leftover width after compact plates (names need room to breathe).
    flex: bool,
    action: CreateLocPlateAction,
}

const LOC_PLATE_H: f32 = 30.0;

/// Location plate fills. Selected elevates to canvas; unselected stays surface.
fn create_loc_plate_fills(t: &Theme, selected: bool) -> ControlFills {
    if selected {
        let canvas = t.neutral_bg();
        ControlFills {
            rest: canvas,
            hover: canvas,
            press: t.wash_toward_neutral_fg(canvas, FG_HOVER),
        }
    } else {
        let ground = t.neutral_bg_secondary();
        ControlFills {
            rest: ground,
            hover: t.wash_toward_neutral_fg(ground, FG_HOVER),
            press: t.wash_toward_neutral_fg(ground, FG_PRESS),
        }
    }
}

fn create_loc_plate_chrome_w(ui: &egui::Ui, t: &Theme, icon: &str) -> f32 {
    let ig = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_ui_font_id(), t.neutral_fg());
    Space::Sm.pts() * 2.0 + ig.size().x + Space::Xs.pts()
}

fn create_loc_plate_natural_w(ui: &egui::Ui, t: &Theme, plate: &CreateLocPlate) -> f32 {
    let chrome = create_loc_plate_chrome_w(ui, t, plate.icon);
    let label_w = match &plate.label {
        LocPlateLabel::Text(s) => {
            ui.painter()
                .layout_no_wrap(s.clone(), TypeRole::Body.font_id(), t.neutral_fg())
                .size()
                .x
        }
        // Same stack as paint (glyphon) so width budget matches draw.
        LocPlateLabel::Alongside(name) => {
            measure_file_name(ui, "Alongside ") + measure_file_name(ui, name)
        }
        LocPlateLabel::FileName(name) => measure_file_name(ui, name),
    };
    chrome + label_w
}

/// Location plate: left icon + label. Selected = canvas + accent hairline.
fn create_loc_plate(ui: &mut egui::Ui, t: &Theme, plate: &CreateLocPlate) -> egui::Response {
    let w = crate::components::ui_width(ui);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, LOC_PLATE_H), sense_click());

    let fills = create_loc_plate_fills(t, plate.selected);
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let fill = interact_fill(
        ui.ctx(),
        resp.id.with("create_loc"),
        over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    let r = Radius::Control.corner();
    ui.painter().rect_filled(rect, r, fill);
    if plate.selected {
        ui.painter().rect_stroke(
            rect,
            r,
            Stroke::new(crate::components::STROKE_HAIRLINE, t.accent()),
            StrokeKind::Inside,
        );
    }

    let icon_ink = if plate.icon == phosphor::FOLDER
        || plate.icon == phosphor::FOLDERS
        || plate.icon == phosphor::FOLDER_OPEN
        || plate.icon == phosphor::ARROW_BEND_DOWN_RIGHT
    {
        t.accent()
    } else {
        t.neutral_fg()
    };
    let ig = ui
        .painter()
        .layout_no_wrap(plate.icon.into(), phosphor_ui_font_id(), icon_ink);
    let pad = Space::Sm.pts();
    let gap = Space::Xs.pts();
    let max_label_w = (rect.width() - pad * 2.0 - ig.size().x - gap).max(8.0);
    // Fixed body line box (line-box metrics): layout-box mid for phosphor; glyphon
    // labels share one line height so side-by-side text keeps one baseline.
    let lh = TypeRole::Body.line_height();
    let cy = rect.center().y;
    let line_top = cy - lh / 2.0;
    let mut x = rect.left() + pad;
    ui.painter()
        .galley(egui::pos2(x, line_top + (lh - ig.size().y) / 2.0), ig.clone(), icon_ink);
    x += ig.size().x + gap;

    match &plate.label {
        LocPlateLabel::Text(s) => {
            let g = ui.painter().layout(
                s.clone(),
                TypeRole::Body.font_id(),
                t.neutral_fg(),
                max_label_w,
            );
            ui.painter().galley(
                egui::pos2(x, line_top + (lh - g.size().y) / 2.0),
                g,
                t.neutral_fg(),
            );
        }
        LocPlateLabel::Alongside(name) => {
            // Both halves through glyphon (same metrics). egui prefix + glyphon
            // name was #21 in a mixed stack: descender on “g” pulled the layout
            // box; caps-only names sat on a different baseline.
            let pre = "Alongside ";
            let pre_w = measure_file_name(ui, pre).min(max_label_w);
            paint_file_name(
                ui,
                pre,
                t.neutral_fg(),
                egui::Rect::from_min_size(egui::pos2(x, line_top), egui::vec2(pre_w, lh)),
            );
            let name_w = (max_label_w - pre_w).max(0.0);
            if name_w > 4.0 {
                paint_file_name(
                    ui,
                    name,
                    t.neutral_fg(),
                    egui::Rect::from_min_size(
                        egui::pos2(x + pre_w, line_top),
                        egui::vec2(name_w, lh),
                    ),
                );
            }
        }
        LocPlateLabel::FileName(name) => {
            paint_file_name(
                ui,
                name,
                t.neutral_fg(),
                egui::Rect::from_min_size(egui::pos2(x, line_top), egui::vec2(max_label_w, lh)),
            );
        }
    }
    resp
}

/// Location wizard body: title + hint + tree filling the rest of the body slot.
fn create_location_body(
    app: &ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, parent: Option<Uuid>,
    loc: CreateLoc, alongside: Option<&(Uuid, String)>,
    expanded: &mut std::collections::HashSet<Uuid>,
) {
    let root_id = app
        .session
        .ready()
        .map(|r| r.workspace.files.read().unwrap().root().id);
    let tree_selected = match loc {
        CreateLoc::Root => root_id,
        CreateLoc::Alongside => alongside.map(|(id, _)| *id).or(parent),
        CreateLoc::Custom => parent,
    };

    // X closes the whole create sheet; Back is the wizard “previous step”.
    if sheet_title_muted(ui, t, "Location") {
        queue.push(A::CloseModal);
    }
    ui.add(Spacer::new(Space::Md));
    ui.label(
        TypeRole::Body
            .rich("Choose a folder for the new file.")
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Sm));

    // Tree fills residual of the parent-owned body slot (passed via max_rect height).
    // Outside stroke is free of layout (no Frame Inside / #30 budget).
    let tree_h = crate::components::remaining_height(ui).max(crate::components::ROW_H * 4.0);
    let w = crate::components::ui_width(ui).max(1.0);
    let radius = crate::components::Radius::Control.corner();

    ui.allocate_ui_with_layout(egui::vec2(w, tree_h), Layout::top_down(Align::Min), |ui| {
        ui.set_width(w);
        ui.set_height(tree_h);
        let (slot, _) = ui.allocate_exact_size(egui::vec2(w, tree_h), egui::Sense::hover());
        crate::components::paint_plate_stroke(ui, slot, radius, t.neutral());
        ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_clip_rect(slot.intersect(ui.clip_rect()));
            if let Some(id) = super::tree::show_folder_tree(
                app,
                ui,
                t,
                expanded,
                tree_selected,
                &[],
                "shell_create_folder_tree",
                tree_h,
            ) {
                queue.push(A::CreatePickFolder(id));
            }
        });
    });
}

fn create_location_footer(ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>) {
    let h = crate::components::control_height();
    if Button::quiet(t, "Back")
        .height(h)
        .max_width(crate::components::ui_width(ui))
        .show(ui)
        .clicked()
    {
        queue.push(A::CreateSetPicking(false));
    }
}

/// Folder path with leading + trailing `/` (`/notes/`, `/` for root).
fn create_summary_line(
    ui: &mut egui::Ui, t: &Theme, app: &ShellApp, kind: CreateKind, name: &str,
    parent: Option<Uuid>,
) {
    use workspace_rs::widgets::GlyphonLabel;

    let kind_l = kind.label();
    let ext = kind.ext().unwrap_or("");
    let full = if name.is_empty() {
        format!("untitled{ext}")
    } else if ext.is_empty() || name.ends_with(ext) {
        name.to_owned()
    } else {
        format!("{name}{ext}")
    };
    let display_name = display_file_name(&full);

    let dest = super::sheet_folder::folder_path_slash(app, parent);

    let max_w = crate::components::ui_width(ui).max(1.0);
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let ink = t.neutral_fg();

    let measure = |text: &str, bold: bool| -> f32 {
        GlyphonLabel::new_rich(vec![(text, bold)], ink)
            .font_size(fs)
            .line_height(lh)
            .measure(ui)
            .x
    };

    let head = format!("{kind_l} ");
    let mid = " will be created at: ";
    let path_w = measure(&dest, true);
    let one_line_w =
        measure(&head, false) + measure(display_name, true) + measure(mid, false) + path_w;

    if one_line_w <= max_w {
        ui.add(
            GlyphonLabel::new_rich(
                vec![(&head, false), (display_name, true), (mid, false), (&dest, true)],
                ink,
            )
            .font_size(fs)
            .line_height(lh)
            .max_width(f32::MAX),
        );
    } else {
        // Line 1: prose; line 2: path fitted to full sheet width (no Clip chop).
        ui.add(
            GlyphonLabel::new_rich(vec![(&head, false), (display_name, true), (mid, false)], ink)
                .font_size(fs)
                .line_height(lh)
                .max_width(max_w)
                .text_overflow(workspace_rs::widgets::TextOverflow::EndEllipsis),
        );
        let path_shown = super::sheet_folder::fit_slash_path(ui, &dest, max_w, fs, lh, ink);
        ui.add(
            GlyphonLabel::new_rich(vec![(&path_shown, true)], ink)
                .font_size(fs)
                .line_height(lh)
                .max_width(f32::MAX),
        );
    }
}
