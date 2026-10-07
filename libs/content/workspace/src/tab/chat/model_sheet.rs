//! The model sheet: the choice and how hard it thinks, the favorites, then
//! every provider's models, as one tree on the sticky rows the folder picker
//! uses. A section or provider row folds and pins while its rows scroll
//! under it; a model row picks; its pin makes it a favorite.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash as _, Hasher as _};

use egui::{
    Align, Align2, Area, Color32, Id, Key, LayerId, Layout, Modifiers, Order, Rect, ScrollArea,
    Sense, Ui, UiBuilder, Vec2, pos2, vec2,
};
use lb_rs::Uuid;

use super::view::effort_name;
use super::{Chat, ListingState};
use crate::style::chrome::{control_height, is_touch, shortcut_enter};
use crate::style::sheet_panel_fixed;
use crate::style::tree_metrics::{INDENT_BASE, INDENT_STEP, ROW_H};
use crate::style::{
    FG_HOVER, FG_PRESS, Field, FileRow, FlatRow, Icon, Radius, RowGeom, SheetFooterOpts, Space,
    Spacer, Theme, TreeRowChrome, TypeRole, control_icon_hit, icon_button_hit, paint_plate_stroke,
    paint_sticky_viewport, paint_tree_file_row, phosphor, place_at, sense_click, sheet_dim,
    sheet_footer, sheet_panel_fit, sheet_title_muted, sticky_band_above, tip_text,
    with_overlay_scroll,
};
use crate::tab::ExtendedOutput as _;

/// The sheet's content width, as the folder sheet's.
const SHEET_W: f32 = 360.0;
/// The list's height on a pointer, in rows.
const LIST_ROWS: f32 = 12.0;
/// Sections fold under names no provider file can have.
pub(super) const THINKING: &str = "/thinking";
const FAVORITES: &str = "/favorites";
/// The pin's gap to its row's trailing edge: the row's own pad above and
/// below it.
const PIN_INSET: Space = Space::Xs;

/// One row of the tree.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Item {
    /// The choice, under its provider's name.
    Current {
        selection: String,
        label: String,
        provider: String,
    },
    /// How hard the model thinks; folds open onto its values.
    Thinking {
        label: String,
        open: bool,
    },
    Effort {
        value: Option<String>,
        label: String,
        chosen: bool,
    },
    Favorites {
        open: bool,
    },
    /// A pinned model, marked with its provider since it stands alone.
    Favorite {
        selection: String,
        label: String,
        provider: String,
    },
    Provider {
        name: String,
        open: bool,
    },
    Model {
        selection: String,
        label: String,
    },
    /// Why an open provider has no model rows: still listing, failed, empty.
    Note {
        provider: String,
        text: String,
    },
    /// The provider has no key yet; the row opens the form that takes one.
    Connect {
        provider: String,
    },
    /// Opens the form for a provider not yet set up.
    AddProvider,
}

/// A one-shot scroll for the list.
pub(super) enum Reveal {
    /// Keep a just-folded pinned row where it was on screen.
    Hold { row: String, vy: f32 },
}

enum Action {
    Fold { name: String, hold: Option<f32> },
    Pick(String),
    Pin(String),
    Effort(Option<String>),
    Connect(String),
    AddProvider,
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    let first = chars.next().map(|c| c.to_uppercase().to_string());
    first.unwrap_or_default() + chars.as_str()
}

/// The provider a `provider/model` selection names.
fn provider_of(selection: &str) -> String {
    selection.split_once('/').map_or("", |(p, _)| p).to_string()
}

/// The rows to show: each provider, then its models unless it is folded. A
/// filter keeps the models it matches, opens their providers, and drops the
/// providers with none.
pub(super) fn tree(
    providers: &[String], listings: &HashMap<String, ListingState>, folded: &HashSet<String>,
    filter: &str,
) -> Vec<Item> {
    let filter = filter.trim().to_lowercase();
    let mut out = Vec::new();
    for name in providers {
        let listing = listings.get(name);
        let models: Vec<Item> = match listing {
            Some(ListingState::Ready(list)) => list
                .iter()
                .filter(|m| {
                    filter.is_empty()
                        || m.label().to_lowercase().contains(&filter)
                        || m.id.to_lowercase().contains(&filter)
                })
                .map(|m| Item::Model { selection: format!("{name}/{}", m.id), label: m.label() })
                .collect(),
            _ => Vec::new(),
        };
        if !filter.is_empty() && models.is_empty() {
            continue;
        }
        let open = !filter.is_empty() || !folded.contains(name);
        out.push(Item::Provider { name: name.clone(), open });
        if !open {
            continue;
        }
        let note = |text: String| Item::Note { provider: name.clone(), text };
        match listing {
            Some(ListingState::Ready(_)) if models.is_empty() => {
                out.push(note("No models listed".into()))
            }
            Some(ListingState::Ready(_)) => out.extend(models),
            Some(ListingState::NeedsKey) => out.push(Item::Connect { provider: name.clone() }),
            // The failure alone: a row has room for little more.
            Some(ListingState::Failed(err)) => {
                let mut chars = err.chars();
                let first = chars.next().map(|c| c.to_uppercase().to_string());
                out.push(note(first.unwrap_or_default() + chars.as_str()))
            }
            Some(ListingState::Loading) | None => out.push(note("Loading…".into())),
        }
    }
    out
}

impl Item {
    /// The sticky tree's row: sections and providers are its folders. Ids
    /// only need to be stable and distinct, so they are hashed from what the
    /// row names.
    pub(super) fn flat(&self) -> FlatRow {
        let (kind, key, depth) = match self {
            Item::Provider { name, .. } => (0u8, name.as_str(), 0),
            Item::Model { selection, .. } => (1, selection.as_str(), 1),
            Item::Note { provider, .. } => (2, provider.as_str(), 1),
            Item::Connect { provider } => (3, provider.as_str(), 1),
            Item::Current { .. } => (4, "", 0),
            Item::Thinking { .. } => (5, THINKING, 0),
            Item::Effort { value, .. } => (6, value.as_deref().unwrap_or(""), 1),
            Item::Favorites { .. } => (7, FAVORITES, 0),
            Item::Favorite { selection, .. } => (8, selection.as_str(), 1),
            Item::AddProvider => (9, "", 0),
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (kind, key).hash(&mut hasher);
        FlatRow {
            id: Uuid::from_u128(hasher.finish() as u128),
            depth,
            is_folder: depth == 0,
            kids_empty: false,
        }
    }

    /// The name a folding row folds under.
    fn fold_key(&self) -> Option<&str> {
        match self {
            Item::Provider { name, .. } => Some(name),
            Item::Thinking { .. } => Some(THINKING),
            Item::Favorites { .. } => Some(FAVORITES),
            _ => None,
        }
    }
}

impl Chat {
    pub(super) fn open_model_sheet(&mut self) {
        self.model_dest = self.selection();
        self.model_filter.clear();
        self.model_reveal = None;
        self.model_scroll_top = true;
        self.models_open = true;
        self.ctx.set_virtual_keyboard_shown(false);
        // A listing that failed gets another try each time the sheet opens.
        self.listings
            .retain(|_, state| !matches!(state, ListingState::Failed(_)));
        for name in self.provider_names() {
            self.fetch_listing(&name);
        }
    }

    fn provider_names(&self) -> Vec<String> {
        self.providers.iter().map(|o| o.name.clone()).collect()
    }

    /// The sheet's rows: the choice, how hard it thinks, the favorites, a
    /// way to add a provider, then every provider's models. A filter leaves
    /// only the models it matches.
    pub(super) fn rows(&self) -> Vec<Item> {
        let names = self.provider_names();
        let tree = tree(&names, &self.listings, &self.model_folded, &self.model_filter);
        if !self.model_filter.trim().is_empty() {
            return tree;
        }
        let mut out = Vec::new();
        if let Some(selection) = self.model_dest.clone().or_else(|| self.selection()) {
            let label = self.selection_label(&selection);
            let provider = provider_of(&selection);
            out.push(Item::Current { selection, label, provider });
        }
        let efforts = self.efforts();
        if !efforts.is_empty() {
            let effort = self.effort().map(str::to_string);
            let open = !self.model_folded.contains(THINKING);
            let level = effort.as_deref().map_or("default", effort_name);
            out.push(Item::Thinking { label: format!("Thinking: {level}"), open });
            if open {
                let chosen = effort.is_none();
                out.push(Item::Effort { value: None, label: "Default".into(), chosen });
                for value in efforts {
                    out.push(Item::Effort {
                        value: Some(value.to_string()),
                        label: capitalize(effort_name(value)),
                        chosen: effort.as_deref() == Some(*value),
                    });
                }
            }
        }
        // A favorite whose provider is not offered here cannot run here.
        let favorites: Vec<&String> = self
            .favorites
            .iter()
            .filter(|s| self.providers.iter().any(|o| o.name == provider_of(s)))
            .collect();
        if !favorites.is_empty() {
            let open = !self.model_folded.contains(FAVORITES);
            out.push(Item::Favorites { open });
            if open {
                for selection in favorites {
                    out.push(Item::Favorite {
                        selection: selection.clone(),
                        label: self.selection_label(selection),
                        provider: provider_of(selection),
                    });
                }
            }
        }
        out.push(Item::AddProvider);
        out.extend(tree);
        out
    }

    fn close_model_sheet(&mut self) {
        self.models_open = false;
        self.model_dest = None;
        self.model_reveal = None;
    }

    /// The sheet while it is open: a filter, the tree, and Done to choose.
    /// Esc clears the filter first, then closes.
    pub(super) fn show_model_sheet(&mut self, ui: &mut Ui, t: &Theme, opened_now: bool) {
        if !self.models_open {
            return;
        }
        let ctx = ui.ctx().clone();
        // The filter field would swallow these keys; take them first.
        let (enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Enter)
                    || i.consume_key(Modifiers::COMMAND, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        let sheet_id = Id::new(("chat_models", self.id));
        let filter_id = sheet_id.with("filter");
        let dim_clicked =
            sheet_dim(&ctx, sheet_id.with("dim"), LayerId::new(Order::Foreground, sheet_id));
        let (mut dismiss, mut confirm) = (false, false);
        let touch = is_touch(&ctx);
        let fill = self.sheet_fill();
        let mut body = |ui: &mut Ui, w: f32, tree_h: f32| {
            dismiss |= sheet_title_muted(ui, t, "Model");
            ui.add(Spacer::new(Space::Md));
            let field = Field::new(t, &mut self.model_filter)
                .hint("Filter models")
                .leading(phosphor::SEARCH)
                .clearable(true)
                .width(w)
                .id(filter_id)
                .show(ui);
            if field.clicked() {
                ctx.set_virtual_keyboard_shown(true);
            }
            ui.add(Spacer::new(Space::Sm));
            self.show_model_tree(ui, t, w, tree_h);
            ui.add(Spacer::new(Space::Md));
            let footer = sheet_footer(
                ui,
                t,
                "Done",
                SheetFooterOpts::default()
                    .divider(false)
                    .quiet_primary(true)
                    .primary_shortcut(shortcut_enter())
                    .primary_enabled(self.model_dest.is_some()),
            );
            dismiss |= footer.cancel;
            confirm |= footer.primary;
        };
        let area = Area::new(sheet_id).order(Order::Foreground);
        if touch {
            let inner = fill.shrink(Space::Md.pts());
            let chrome = control_height() * 3.0 + Space::Md.pts() * 2.0 + Space::Sm.pts();
            let tree_h = (inner.height() - chrome).max(ROW_H * 3.0);
            area.fixed_pos(fill.min).show(&ctx, |ui| {
                sheet_panel_fixed(ui, t, inner.width(), inner.height(), |ui| {
                    body(ui, inner.width(), tree_h)
                });
            });
        } else {
            area.anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    sheet_panel_fit(ui, t, SHEET_W, |ui| body(ui, SHEET_W, ROW_H * LIST_ROWS));
                });
        }
        // Nothing else focused: the filter takes it, so typing always lands.
        // (A request made before the sheet's first frame would be dropped:
        // that frame is an invisible sizing pass, and egui surrenders the
        // focus of a widget created disabled.)
        if ctx.memory(|m| m.focused().is_none()) {
            ctx.memory_mut(|m| m.request_focus(filter_id.with("edit")));
        }
        if esc && !self.model_filter.is_empty() {
            self.model_filter.clear();
        } else if esc || ((dismiss || dim_clicked) && !opened_now) {
            self.close_model_sheet();
        } else if confirm || (enter && self.model_dest.is_some()) {
            if let Some(selection) = self.model_dest.clone() {
                self.select(selection);
            }
            self.close_model_sheet();
        }
    }

    /// The tree in a hairline plate, `list_h` tall.
    fn show_model_tree(&mut self, ui: &mut Ui, t: &Theme, w: f32, list_h: f32) {
        let (slot, _) = ui.allocate_exact_size(vec2(w, list_h), Sense::hover());
        paint_plate_stroke(ui, slot, Radius::Control.corner(), t.neutral());
        let items = self.rows();
        if items.is_empty() {
            ui.painter().text(
                slot.center(),
                Align2::CENTER_CENTER,
                "No models match",
                TypeRole::Body.font_id(),
                t.neutral_fg_secondary(),
            );
            return;
        }
        let flat: Vec<FlatRow> = items.iter().map(Item::flat).collect();
        let index: HashMap<Uuid, usize> = flat.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
        let heights: Vec<f32> = items
            .iter()
            .map(|it| FileRow::height_for(matches!(it, Item::Current { .. })))
            .collect();
        let geom = RowGeom::from_heights(&heights);
        // A viewport of trailing room lets any provider scroll up to the pin.
        let bottom_pad = list_h;
        let max_off = geom.total;

        // A new filter means a new list: it starts at its top, or a short
        // list is left scrolled past its rows with only a pinned provider in
        // view. The top is asked for until the list reports being there,
        // since a sheet's sizing pass keeps nothing.
        if self.model_filter != self.model_filter_shown {
            self.model_filter_shown = self.model_filter.clone();
            self.model_scroll_top = true;
        }
        let offset = match &self.model_reveal {
            _ if self.model_scroll_top => Some(0.0),
            Some(Reveal::Hold { row, vy }) => items
                .iter()
                .position(|it| it.fold_key() == Some(row.as_str()))
                .map(|i| {
                    (geom.top(i) - sticky_band_above(&flat, &geom, i) - vy).clamp(0.0, max_off)
                }),
            None => None,
        };
        self.model_reveal = None;

        let filtering = !self.model_filter.trim().is_empty();
        let mut names = self.provider_names();
        for item in &items {
            if let Item::Current { provider, .. } | Item::Favorite { provider, .. } = item {
                if !names.contains(provider) {
                    names.push(provider.clone());
                }
            }
        }
        let faces: HashMap<String, (Icon, String)> = names
            .iter()
            .map(|name| {
                let mark = self.mark(ui.ctx(), name, TypeRole::Body.size());
                (name.clone(), (mark, self.provider_label(name)))
            })
            .collect();
        let side = TypeRole::Body.size();
        let (chosen, favorites) = (&self.model_dest, &self.favorites);
        let row_salt = Id::new(("chat_model_row", self.id));
        let scroll_id = Id::new(("chat_model_scroll", self.id));
        let pin_top_r = Radius::Control.pts();
        let mut action = None;

        let mut child = ui.new_child(UiBuilder::new().max_rect(slot));
        child.spacing_mut().item_spacing = Vec2::ZERO;
        child.set_clip_rect(slot.intersect(ui.clip_rect()));
        let shown_offset = with_overlay_scroll(&mut child, scroll_id, |ui| {
            let mut scroll = ScrollArea::vertical()
                .id_salt(scroll_id)
                .max_height(list_h)
                .min_scrolled_height(list_h)
                .auto_shrink([false, false]);
            if let Some(offset) = offset {
                scroll = scroll.vertical_scroll_offset(offset);
            }
            let out = scroll.show_viewport(ui, |ui, viewport| {
                paint_sticky_viewport(
                    ui,
                    t,
                    &flat,
                    &geom,
                    viewport,
                    bottom_pad,
                    pin_top_r,
                    0.0,
                    |ui, t, row, paint_r, hit_r, elevated, pin_vy| {
                        let Some(item) = index.get(&row.id).map(|i| &items[*i]) else { return };
                        let id = row_salt.with((row.id, elevated));
                        let mark_at = |ui: &Ui, mark: &Icon, depth: usize| {
                            let x = paint_r.left() + INDENT_BASE + depth as f32 * INDENT_STEP;
                            let center = pos2(x + side / 2.0, paint_r.center().y);
                            mark.paint(ui.painter(), center, side, t.neutral_fg());
                        };
                        // Folding a pinned row shortens the list above the
                        // view; hold the row where it was.
                        let fold = |open: bool| {
                            let hold = pin_vy.filter(|_| elevated && open);
                            Action::Fold { name: item.fold_key().unwrap_or("").to_string(), hold }
                        };
                        match item {
                            Item::Current { label, provider, .. } => {
                                let Some((mark, who)) = faces.get(provider) else { return };
                                let chrome =
                                    TreeRowChrome::new(0).selected(true).interactive(false);
                                paint_tree_file_row(
                                    ui,
                                    t,
                                    label.clone(),
                                    "",
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.subtitle(who.clone()),
                                );
                                mark_at(ui, mark, 0);
                            }
                            Item::Thinking { label, open } => {
                                let chrome = TreeRowChrome::new(0)
                                    .with_sheet_pin(elevated, pin_vy, pin_top_r);
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    label.clone(),
                                    phosphor::LIGHTBULB,
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                if resp.clicked() {
                                    action = Some(fold(*open));
                                }
                            }
                            Item::Effort { value, label, chosen } => {
                                let chrome = TreeRowChrome::new(1).selected(*chosen);
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    label.clone(),
                                    "",
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                if resp.clicked() {
                                    action = Some(Action::Effort(value.clone()));
                                }
                            }
                            Item::Favorites { open } => {
                                let chrome = TreeRowChrome::new(0)
                                    .with_sheet_pin(elevated, pin_vy, pin_top_r);
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    "Favorites",
                                    phosphor::PUSH_PIN,
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                if resp.clicked() {
                                    action = Some(fold(*open));
                                }
                            }
                            Item::Provider { name, open } => {
                                let Some((mark, label)) = faces.get(name) else { return };
                                let chrome = TreeRowChrome::new(0)
                                    .with_sheet_pin(elevated, pin_vy, pin_top_r)
                                    .interactive(!filtering);
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    label.clone(),
                                    "",
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                mark_at(ui, mark, 0);
                                if !filtering && resp.clicked() {
                                    action = Some(fold(*open));
                                }
                            }
                            Item::Model { selection, label }
                            | Item::Favorite { selection, label, .. } => {
                                let chrome = TreeRowChrome::new(1)
                                    .selected(chosen.as_ref() == Some(selection));
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    label.clone(),
                                    "",
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| {
                                        r.sense(sense_click())
                                            .trail_reserve(control_icon_hit() + PIN_INSET.pts())
                                    },
                                );
                                if resp.clicked() {
                                    action = Some(Action::Pick(selection.clone()));
                                }
                                if let Item::Favorite { provider, .. } = item {
                                    if let Some((mark, _)) = faces.get(provider) {
                                        mark_at(ui, mark, 1);
                                    }
                                }
                                let pinned = favorites.contains(selection);
                                // The chosen row keeps its pin in reach of a finger.
                                let over = ui.ctx().rect_contains_pointer(ui.layer_id(), hit_r)
                                    || chosen.as_ref() == Some(selection);
                                // The row's own wash is the button's ground.
                                let wash = if chosen.as_ref() == Some(selection) {
                                    FG_PRESS
                                } else {
                                    FG_HOVER
                                };
                                let ground =
                                    t.wash_toward_neutral_fg(t.neutral_bg_secondary(), wash);
                                if (pinned || over)
                                    && pin_button(ui, t, paint_r, hit_r, pinned, ground)
                                {
                                    action = Some(Action::Pin(selection.clone()));
                                }
                            }
                            Item::Connect { provider } => {
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    "Add an API key…",
                                    "",
                                    TreeRowChrome::new(1),
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                if resp.clicked() {
                                    action = Some(Action::Connect(provider.clone()));
                                }
                            }
                            Item::AddProvider => {
                                let resp = paint_tree_file_row(
                                    ui,
                                    t,
                                    "Add a provider…",
                                    phosphor::FILE_PLUS,
                                    TreeRowChrome::new(0),
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.sense(sense_click()),
                                );
                                if resp.clicked() {
                                    action = Some(Action::AddProvider);
                                }
                            }
                            Item::Note { text, .. } => {
                                let chrome = TreeRowChrome::new(1).interactive(false);
                                paint_tree_file_row(
                                    ui,
                                    t,
                                    text.clone(),
                                    "",
                                    chrome,
                                    id,
                                    paint_r,
                                    hit_r,
                                    |r| r.muted(true),
                                );
                            }
                        }
                    },
                );
            });
            (out.state.offset.y, out.state.offset.y, out.id)
        });
        if shown_offset == 0.0 {
            self.model_scroll_top = false;
        }

        match action {
            Some(Action::Fold { name, hold }) => {
                if !self.model_folded.remove(&name) {
                    self.model_folded.insert(name.clone());
                }
                if let Some(vy) = hold {
                    self.model_reveal = Some(Reveal::Hold { row: name, vy });
                }
            }
            // A level picked folds the list back onto the one line.
            Some(Action::Effort(effort)) => {
                self.set_effort(effort);
                self.model_folded.insert(THINKING.into());
            }
            // A finger picks and is done; a pointer picks and confirms.
            Some(Action::Pick(selection)) if is_touch(ui.ctx()) => {
                self.select(selection);
                self.close_model_sheet();
            }
            Some(Action::Pick(selection)) => self.model_dest = Some(selection),
            Some(Action::Pin(selection)) => self.toggle_favorite(&selection),
            Some(Action::Connect(name)) => {
                self.close_model_sheet();
                self.begin_connect(Some(&name));
            }
            Some(Action::AddProvider) => {
                self.close_model_sheet();
                self.begin_connect(None);
            }
            None => {}
        }
    }
}

/// The pin at a model row's trailing edge, clipped to the row's hit rect so
/// it cannot be reached under a pinned provider. Returns whether it was
/// clicked.
fn pin_button(ui: &mut Ui, t: &Theme, row: Rect, hit: Rect, pinned: bool, ground: Color32) -> bool {
    let side = control_icon_hit();
    let slot = Rect::from_center_size(
        pos2(row.right() - PIN_INSET.pts() - side / 2.0, row.center().y),
        Vec2::splat(side),
    );
    Spacer::paint_at(
        ui,
        PIN_INSET,
        Rect::from_min_max(slot.right_top(), pos2(row.right(), slot.bottom())),
    );
    let clip = ui.clip_rect();
    ui.set_clip_rect(clip.intersect(hit));
    let (resp, _) = place_at(ui, slot, Layout::left_to_right(Align::Center), |ui| {
        icon_button_hit(ui, t, phosphor::PUSH_PIN, pinned, ground, side)
    });
    ui.set_clip_rect(clip);
    tip_text(ui.ctx(), &resp, if pinned { "Unpin" } else { "Pin to the model menu" });
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_chat::ModelInfo;

    fn model(id: &str) -> ModelInfo {
        ModelInfo { id: id.into(), display_name: None, window: None, output: None }
    }

    fn listings() -> (Vec<String>, HashMap<String, ListingState>) {
        let providers =
            vec!["anthropic".to_string(), "cerebras".to_string(), "localhost".to_string()];
        let mut listings = HashMap::new();
        listings.insert(
            "anthropic".into(),
            ListingState::Ready(vec![model("claude-opus-5-5"), model("claude-haiku-4-5")]),
        );
        listings.insert("cerebras".into(), ListingState::Ready(vec![model("gpt-oss-120b")]));
        listings
            .insert("localhost".into(), ListingState::Failed("can't reach localhost:11434".into()));
        (providers, listings)
    }

    /// (depth, text) of each row, providers suffixed with their fold state.
    fn outline(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                Item::Provider { name, open } => {
                    format!("{name} {}", if *open { "v" } else { ">" })
                }
                Item::Model { label, .. } => format!("  {label}"),
                Item::Note { text, .. } => format!("  ({text})"),
                Item::Connect { .. } => "  [add a key]".to_string(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn providers_show_their_models_until_folded() {
        let (providers, listings) = listings();
        let mut folded = HashSet::new();
        assert_eq!(
            outline(&tree(&providers, &listings, &folded, "")),
            [
                "anthropic v",
                "  Claude Opus 5.5",
                "  Claude Haiku 4.5",
                "cerebras v",
                "  GPT OSS 120B",
                "localhost v",
                "  (Can't reach localhost:11434)",
            ]
        );
        folded.insert("anthropic".to_string());
        folded.insert("localhost".to_string());
        assert_eq!(
            outline(&tree(&providers, &listings, &folded, "")),
            ["anthropic >", "cerebras v", "  GPT OSS 120B", "localhost >"]
        );
    }

    #[test]
    fn a_filter_opens_what_matches_and_drops_the_rest() {
        let (providers, listings) = listings();
        let folded: HashSet<String> = providers.iter().cloned().collect();
        assert_eq!(
            outline(&tree(&providers, &listings, &folded, " OPUS ")),
            ["anthropic v", "  Claude Opus 5.5"]
        );
        assert_eq!(
            outline(&tree(&providers, &listings, &folded, "oss-120")),
            ["cerebras v", "  GPT OSS 120B"],
            "ids match too"
        );
        assert!(tree(&providers, &listings, &folded, "nothing").is_empty());
    }

    /// A filter that spells a provider's name still lists its models.
    #[test]
    fn a_filter_naming_the_provider_keeps_its_models() {
        let providers = vec!["apple".to_string()];
        let mut listings = HashMap::new();
        listings.insert(
            "apple".into(),
            ListingState::Ready(vec![ModelInfo {
                id: "on-device".into(),
                display_name: Some("Apple Intelligence".into()),
                window: None,
                output: None,
            }]),
        );
        for filter in ["appl", "apple", "Apple Intelligence"] {
            assert_eq!(
                outline(&tree(&providers, &listings, &HashSet::new(), filter)),
                ["apple v", "  Apple Intelligence"],
                "{filter}"
            );
        }
    }

    #[test]
    fn a_provider_without_rows_says_why() {
        let providers = vec!["a".to_string(), "b".to_string()];
        let mut listings = HashMap::new();
        listings.insert("a".into(), ListingState::Loading);
        listings.insert("b".into(), ListingState::Ready(Vec::new()));
        assert_eq!(
            outline(&tree(&providers, &listings, &HashSet::new(), "")),
            ["a v", "  (Loading…)", "b v", "  (No models listed)"]
        );
        listings.insert("a".into(), ListingState::NeedsKey);
        assert_eq!(
            outline(&tree(&providers, &listings, &HashSet::new(), ""))[..2],
            ["a v", "  [add a key]"]
        );
    }

    #[test]
    fn rows_have_distinct_ids() {
        let (providers, listings) = listings();
        let items = tree(&providers, &listings, &HashSet::new(), "");
        let ids: HashSet<Uuid> = items.iter().map(|i| i.flat().id).collect();
        assert_eq!(ids.len(), items.len());
    }
}
