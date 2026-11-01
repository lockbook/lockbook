//! A wiki embed of anything but an image (`![[Plan]]`, `![[Plan#Goals]]`,
//! `![[paper.pdf#page=3]]`) shows the file's tab in the embedded
//! presentation, in the slot images and link cards use; a link to a note or
//! a tab shows the same in its hover card. The slot reserves the region's
//! remembered height and holds a skeleton until the tab settles, then takes
//! its measured height once. Tabs open on a thread as their slots near the
//! view and close when they leave it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use comrak::nodes::{AstNode, NodeValue};
use egui::{CornerRadius, Rect, Sense, UiBuilder, Vec2, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::link::LinkMenuAction;
use super::link::hover::{CARD_PAD, CARD_RADIUS, FILE_CARD, ink_secondary};
use crate::file_cache::{FilesExt as _, ResolvedLink, split_fragment, wiki_fragment};
use crate::show::ElapsedHumanString as _;
use crate::style::button::icon_button;
use crate::style::phosphor::ARROW_SQUARE_OUT;
use crate::style::tree_metrics::INDENT_BASE;
use crate::style::{FileRow, ROW_H, Space, TypeRole, file_row_icon, parent_crumbs};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::image::image::image_url;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};
use crate::tab::{
    ContentState, Destination, Doc, ExtendedOutput as _, Fragment, Place, Presentation, Reveal,
    Tab, TabContent, TabFailure, TabResources,
};
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::{GlyphonLabel, TextOverflow};

/// The most documents one chain of embeds holds, the note itself included;
/// past it an embed is the link it names.
pub const EMBED_DEPTH: usize = 3;
/// A slot's height before its tab has ever been measured at its width.
const SKELETON_HEIGHT: f32 = 120.0;

/// The title a wiki embed names when it isn't an image's.
pub fn tab_embed_title(node: &AstNode<'_>) -> Option<String> {
    match &node.data.borrow().value {
        NodeValue::WikiLink(wl) if wl.embed && image_url(node).is_none() => Some(wl.url.clone()),
        _ => None,
    }
}

/// The tabs one editor shows in its embed slots and hover cards.
#[derive(Default)]
pub struct TabEmbeds {
    /// What the tabs are built from; none where nothing can embed.
    pub res: Option<TabResources>,
    /// The files from the outermost note to this one: an embed among them
    /// cycles, and the length is the depth.
    pub path: Vec<Uuid>,
    slots: RefCell<HashMap<SlotKey, Slot>>,
    /// Changes when a slot takes a new height.
    seq: Cell<u64>,
    /// A slot settled this frame: the next one draws it.
    settling: Cell<bool>,
    /// The slot the last press landed in: its selection is what Copy takes.
    active: RefCell<Option<SlotKey>>,
    /// Documents written, so the tabs showing them reload.
    #[cfg(not(target_family = "wasm"))]
    writes: RefCell<Option<lb_rs::service::events::Receiver<lb_rs::service::events::Event>>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SlotKey {
    url: String,
    /// Which of the title's embeds, in document order: the same note embedded twice
    /// is two tabs.
    nth: usize,
    card: bool,
}

struct Slot {
    place: Place,
    state: SlotState,
    /// A newer version of the document being read, to take in place.
    reload: Option<Receiver<Result<Option<Doc>, TabFailure>>>,
    /// The tab has settled once; the skeleton is gone for good.
    shown: bool,
    /// The slot's height is the tab's measured one, for this version of it.
    measured: bool,
    /// Its rect when last painted.
    rect: Rect,
    touched: bool,
}

enum SlotState {
    /// The document as read, or none for one that opens without its bytes.
    Loading(Receiver<Result<Option<Doc>, TabFailure>>),
    Open(Box<Tab>),
    Failed(String),
}

impl TabEmbeds {
    pub fn seq(&self) -> u64 {
        self.seq.get()
    }

    /// An embed slot painted this frame still shows its skeleton, or only
    /// just stopped.
    pub fn pending(&self) -> bool {
        self.settling.get()
            || self.slots.borrow().iter().any(|(key, slot)| {
                let failed = matches!(slot.state, SlotState::Failed(_));
                !key.card && slot.touched && !slot.shown && !failed
            })
    }

    pub fn begin_frame(&self, ctx: &egui::Context) {
        self.settling.set(false);
        let mut slots = self.slots.borrow_mut();
        for slot in slots.values_mut() {
            slot.touched = false;
        }
        let press = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()));
        if let Some(press) = press {
            *self.active.borrow_mut() = slots
                .iter()
                .find(|(key, slot)| !key.card && press.is_some_and(|p| slot.rect.contains(p)))
                .map(|(key, _)| key.clone());
        }
        #[cfg(not(target_family = "wasm"))]
        self.reload_written(&mut slots);
    }

    /// Reads again the documents written since last frame that slots show.
    #[cfg(not(target_family = "wasm"))]
    fn reload_written(&self, slots: &mut HashMap<SlotKey, Slot>) {
        use lb_rs::service::events::Event;
        let Some(res) = &self.res else { return };
        let mut writes = self.writes.borrow_mut();
        let writes = writes.get_or_insert_with(|| res.core.subscribe());
        while let Ok(event) = writes.try_recv() {
            let Event::DocumentWritten(id, _) = event else { continue };
            for slot in slots.values_mut() {
                if slot.place.dest.backing_file() == Some(id) {
                    slot.reload = Some(load(res, slot.place.dest.clone()));
                }
            }
        }
    }

    /// The text selected in the slot the last press landed in, for Copy.
    pub fn copy(&self) -> Option<String> {
        let key = self.active.borrow().clone()?;
        let slots = self.slots.borrow();
        let SlotState::Open(tab) = &slots.get(&key)?.state else { return None };
        let buffer = &tab.markdown()?.edit.renderer.buffer;
        let (a, b) = buffer.current.selection;
        (a != b).then(|| buffer[(a.min(b), a.max(b))].to_string())
    }

    /// Where the slots painted this frame stand: a touch there is theirs.
    pub fn rects(&self) -> Vec<Rect> {
        let slots = self.slots.borrow();
        slots
            .iter()
            .filter(|(key, slot)| !key.card && slot.touched)
            .map(|(_, slot)| slot.rect)
            .collect()
    }

    /// Closes the tabs whose slots didn't paint, near the view or not.
    pub fn end_frame(&self) {
        self.slots.borrow_mut().retain(|_, slot| slot.touched);
    }

    /// The place `title` embeds, if it can: a document past no cycle and
    /// within the depth, while something can build its tab.
    fn place(&self, renderer: &MdRender, title: &str) -> Option<Place> {
        let place = self.reach(renderer, title)?;
        let id = place.dest.backing_file()?;
        (self.path.len() < EMBED_DEPTH && !self.path.contains(&id)).then_some(place)
    }

    /// The document `title` names, for a card, which nests nothing.
    fn reach(&self, renderer: &MdRender, title: &str) -> Option<Place> {
        let res = self.res.as_ref()?;
        let (file, fragment) = wiki_fragment(title);
        let id = renderer.resolve_wikilink(file)?;
        self.place_of(res, id, fragment)
    }

    fn place_of(&self, res: &TabResources, id: Uuid, fragment: Option<&str>) -> Option<Place> {
        let files = res.files.read().unwrap();
        files.get_by_id(id).filter(|f| f.is_document())?;
        let fragment = fragment.and_then(Fragment::from_url);
        Some(Place::at(Destination::File(id), fragment))
    }

    /// The height a slot reserves inside its frame at content width `width`:
    /// the open tab's measured region, else the region's remembered height,
    /// else a skeleton's.
    fn content_height(&self, url: &str, place: &Place, width: f32, card: bool) -> f32 {
        let key = SlotKey { url: url.into(), nth: 0, card };
        let measured = self.slots.borrow_mut().get_mut(&key).and_then(|slot| {
            let SlotState::Open(tab) = &mut slot.state else { return None };
            slot.shown
                .then(|| tab.region_height(place.fragment.as_ref(), width))?
        });
        let remembered = || {
            let res = self.res.as_ref()?;
            res.remembered_height(&place.dest, place.fragment.as_ref(), width)
        };
        measured.or_else(remembered).unwrap_or(SKELETON_HEIGHT)
    }

    /// Draws the slot `key` names into `rect`, opening its tab first.
    fn paint(&self, ui: &mut egui::Ui, key: SlotKey, place: Place, rect: Rect) {
        let Some(res) = self.res.clone() else { return };
        let mut slots = self.slots.borrow_mut();
        let slot = slots
            .entry(key.clone())
            .or_insert_with(|| Slot::open(&res, place.clone()));
        if slot.place != place {
            *slot = Slot::open(&res, place.clone());
        }
        slot.touched = true;
        slot.rect = rect;
        slot.poll(&res, &self.path, key.card);

        let t = ui.ctx().get_lb_theme();
        // a card's frame pads it; a slot pads itself
        let inner = if key.card { rect } else { rect.shrink(CARD_PAD.pts()) };
        match &mut slot.state {
            SlotState::Open(tab) => {
                // the pad takes what hangs past the content: a list's numbers, a last line's descent
                let clip = rect.intersect(ui.clip_rect());
                let child = &mut ui.new_child(UiBuilder::new().max_rect(inner));
                child.set_clip_rect(clip);
                if !slot.shown {
                    child.set_invisible();
                }
                let resp = match res.watched {
                    true => res.guard.show(tab, |tab| tab.show(child)),
                    false => tab.show(child),
                };
                if let Some(id) = resp.open_file {
                    ui.ctx().open_file(id, false);
                }
                if !slot.measured && tab.settled() {
                    slot.shown = true;
                    slot.measured = true;
                    self.settling.set(true);
                    let fragment = place.fragment.as_ref();
                    if let Some(height) = tab.region_height(fragment, inner.width()) {
                        let remembered =
                            res.remembered_height(&place.dest, fragment, inner.width());
                        res.remember_height(&place.dest, fragment, inner.width(), height);
                        // the slot re-stamps once a version, from the remembered height to the real one
                        if remembered != Some(height) {
                            self.seq.set(self.seq.get() + 1);
                            ui.ctx().request_repaint();
                        }
                    }
                }
                if !slot.shown {
                    paint_skeleton(ui, inner);
                    ui.ctx().request_repaint();
                } else if key.card {
                    let (above, below) = tab.overflow();
                    let (ground, clear) = (t.neutral_bg(), egui::Color32::TRANSPARENT);
                    let fade = crate::style::EDGE_FADE.pts();
                    let painter = ui.painter_at(clip);
                    if above {
                        let band = inner.with_max_y(inner.top() + fade);
                        crate::style::paint_fade(&painter, band, ground, clear, true);
                    }
                    if below {
                        let band = inner.with_min_y(inner.bottom() - fade);
                        crate::style::paint_fade(&painter, band, clear, ground, true);
                    }
                }
            }
            SlotState::Loading(_) => {
                paint_skeleton(ui, inner);
                ui.ctx().request_repaint();
            }
            SlotState::Failed(msg) => {
                let font = egui::FontId::proportional(crate::style::TypeRole::Body.size());
                let color = t.neutral_fg_secondary();
                ui.painter().text(
                    inner.left_top(),
                    egui::Align2::LEFT_TOP,
                    msg.as_str(),
                    font,
                    color,
                );
            }
        }
        if !key.card {
            crate::style::paint_doc_border(ui.painter(), rect, CARD_RADIUS.corner(), &t);
            open_button(ui, rect, &place);
        }
    }
}

/// Reads `dest` on a thread, or nothing for a document that opens without
/// its bytes.
fn load(res: &TabResources, dest: Destination) -> Receiver<Result<Option<Doc>, TabFailure>> {
    let (tx, rx) = mpsc::channel();
    let res = res.clone();
    std::thread::spawn(move || {
        let doc = match dest {
            Destination::File(id) if !res.is_image(id) => res.load(id).map(Some),
            _ => Ok(None),
        };
        let _ = tx.send(doc);
        res.ctx.request_repaint();
    });
    rx
}

impl Slot {
    /// Reads the document on a thread; the tab builds when it lands.
    fn open(res: &TabResources, place: Place) -> Self {
        let state = SlotState::Loading(load(res, place.dest.clone()));
        let rect = Rect::NOTHING;
        Self { place, state, reload: None, shown: false, measured: false, rect, touched: true }
    }

    /// Builds the tab once its document has been read, and takes a newer
    /// version in place.
    fn poll(&mut self, res: &TabResources, path: &[Uuid], card: bool) {
        if let Some(Ok(Ok(Some(doc)))) = self.reload.as_ref().map(|rx| rx.try_recv()) {
            self.reload = None;
            if let SlotState::Open(tab) = &mut self.state {
                if let ContentState::Open(content) = &mut tab.content {
                    if content.reload(doc.hmac, &doc.bytes) {
                        self.measured = false;
                    } else {
                        *self = Self::open(res, self.place.clone());
                    }
                }
            }
        }
        let SlotState::Loading(rx) = &self.state else { return };
        let Ok(doc) = rx.try_recv() else { return };
        let dest = &self.place.dest;
        let salt = egui::Id::unique(Uuid::new_v4());
        let content = doc.and_then(|doc| TabContent::open(dest, doc, res, salt, true));
        self.state = match content {
            Ok(mut content) => {
                if let TabContent::Markdown(md) = &mut content {
                    let mut nested = path.to_vec();
                    nested.extend(dest.backing_file());
                    md.edit.renderer.tab_embeds.path = nested;
                    md.edit.renderer.tab_embeds.res = (!card).then(|| res.clone());
                }
                let mut tab = Tab::new(dest.clone(), ContentState::Open(content), salt, true);
                tab.presentation = if card { Presentation::Card } else { Presentation::Embedded };
                tab.reveal = Some(Reveal { fragment: self.place.fragment.clone(), flash: false });
                SlotState::Open(Box::new(tab))
            }
            Err(failure) => SlotState::Failed(failure.msg()),
        };
    }
}

impl<'ast> MdRender {
    /// The place `node`'s embed shows and its slot's size, if it embeds a
    /// tab here: on a line of its own's width in a block that isn't revealed.
    fn tab_embed_slot(
        &self, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) -> Option<(Place, Vec2)> {
        let title = tab_embed_title(node)?;
        let node_range = self.node_range(node);
        let single_line = range.contains_range(&node_range, true, true);
        if self.disable_images || !single_line || self.range_revealed_interior(node_range) {
            return None;
        }
        let place = self.tab_embeds.place(self, &title)?;
        let block = node
            .ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block())?;
        let width = self.width(block);
        let pad = 2.0 * CARD_PAD.pts();
        if width <= pad {
            return None;
        }
        let height = self
            .tab_embeds
            .content_height(&title, &place, width - pad, false);
        Some((place, vec2(width, height + pad)))
    }

    /// The slot's height for the block-height estimate, if `node` embeds.
    pub fn tab_embed_height(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let range = self.node_range(node);
        self.tab_embed_slot(node, range).map(|(_, size)| size.y)
    }

    /// A slot for the tab a wiki embed names; `false` (caller lays out the
    /// wiki link) when it can't embed here.
    pub fn layout_tab_embed(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) -> bool {
        let Some((_, size)) = self.tab_embed_slot(node, range) else { return false };
        let Some(url) = tab_embed_title(node) else { return false };
        layout.push_embed(EmbedSpec {
            advance: size.x,
            ascent: size.y,
            descent: 0.0,
            source_range: self.node_range(node),
            url,
            kind: EmbedKind::Tab,
        });
        true
    }

    /// Paints the tab an embed slot holds; `at` is where its source starts,
    /// which tells repeated embeds of one title apart.
    pub fn paint_tab_embed(&self, ui: &mut egui::Ui, title: &str, rect: Rect, at: Grapheme) {
        let Some(place) = self.tab_embeds.place(self, title) else { return };
        let segs = &self.buffer.current.segs;
        let before = &self.buffer.current.text[..segs.offset_to_byte(at).0];
        // this title's embeds, not ones whose title it begins
        let open = format!("![[{title}");
        let nth = before
            .match_indices(&open)
            .filter(|(i, _)| before[i + open.len()..].starts_with([']', '|']))
            .count();
        let key = SlotKey { url: title.into(), nth, card: false };
        self.tab_embeds.paint(ui, key, place, rect);
    }

    /// The place a link reaches, when it's a tab a card can show.
    pub(super) fn link_place(&self, url: &str, wikilink: bool) -> Option<Place> {
        let res = self.tab_embeds.res.as_ref()?;
        if wikilink {
            return self.tab_embeds.reach(self, url);
        }
        let Some(ResolvedLink::File(id)) = self.resolve_link(url) else { return None };
        self.tab_embeds.place_of(res, id, split_fragment(url).1)
    }

    /// The hover card for a link to a tab: the file's row, the tab at the
    /// card's width, then who edited it last and the card's actions.
    pub(super) fn show_tab_card(
        &self, ctx: &egui::Context, response: &egui::Response, url: &str, place: Place,
    ) -> bool {
        let width = FILE_CARD.x - 2.0 * CARD_PAD.pts();
        crate::style::tip_card_reachable(
            ctx,
            response,
            response.rect,
            width,
            egui::Margin::same(CARD_PAD.pts() as i8),
            |ui| {
                let folder = self.card_folder(&place);
                let height = self
                    .tab_embeds
                    .content_height(url, &place, width, true)
                    .min(card_view_height(folder.is_some()));
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = HEADER_GAP.pts();
                self.card_header(ui, &place, folder, width);
                ui.spacing_mut().item_spacing.y = Space::Sm.pts();
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                let key = SlotKey { url: url.into(), nth: 0, card: true };
                self.tab_embeds.paint(ui, key, place.clone(), rect);
                if let Some(action) = self.card_file_footer(ui, &place) {
                    *self.card_action.borrow_mut() = Some((url.to_string(), action));
                    crate::style::dismiss_tip(ui.ctx());
                    ui.ctx().request_repaint();
                }
            },
        );
        true
    }

    /// The folders `place`'s file sits in, as crumbs, unless it's at the root.
    fn card_folder(&self, place: &Place) -> Option<String> {
        let files = self.files.read().unwrap();
        let file = files.get_by_id(place.dest.backing_file()?)?;
        (file.parent != files.root.id).then(|| parent_crumbs(&files.path(file.id)))
    }

    /// A file card's top: `place`'s file as a row, under its name the
    /// `folder` it's in, its icon in line with the tab's content under it.
    fn card_header(&self, ui: &mut egui::Ui, place: &Place, folder: Option<String>, width: f32) {
        let t = ui.ctx().get_lb_theme();
        let height = card_header_height(folder.is_some());
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        // the row's pad above its name falls inside the card's
        let row =
            Rect::from_min_max(pos2(rect.left() - INDENT_BASE, rect.top() - row_pad()), rect.max);
        let files = self.files.read().unwrap();
        let Some(file) = place.dest.backing_file().and_then(|id| files.get_by_id(id)) else {
            return;
        };
        FileRow::new(&t, file.name.clone())
            .icon(file_row_icon(&file.name, false))
            .subtitle(folder.unwrap_or_default())
            .interactive(false)
            .paint_at_hit(ui, row, row, ui.scope_id().with("card_header"));
    }

    /// A file card's last row: who edited the file last and when, then the
    /// Open button and `⋯` menu, whose choice this returns.
    fn card_file_footer(&self, ui: &mut egui::Ui, place: &Place) -> Option<LinkMenuAction> {
        let t = ui.ctx().get_lb_theme();
        let edited = {
            let files = self.files.read().unwrap();
            let file = place.dest.backing_file().and_then(|id| files.get_by_id(id));
            file.map(|file| {
                let now = web_time::SystemTime::now()
                    .duration_since(web_time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                let since =
                    std::time::Duration::from_millis(now.saturating_sub(file.last_modified));
                let when = since.elapsed_human_string();
                match file.last_modified_by.as_str() {
                    by if by.is_empty() || by == files.root.owner => {
                        vec![(format!("Edited {when}"), false)]
                    }
                    by => vec![(by.to_string(), true), (format!(" edited {when}"), false)],
                }
            })
        };
        let mut chosen = None;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                chosen = self.card_actions(ui, false);
                let Some(edited) = &edited else { return };
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let spans = edited.iter().map(|(s, b)| (s.as_str(), *b)).collect();
                    ui.add(
                        GlyphonLabel::new_rich(spans, ink_secondary(&t))
                            .role(TypeRole::Mono)
                            .max_width(ui.available_width())
                            .text_overflow(TextOverflow::EndEllipsis),
                    );
                });
            });
        });
        chosen
    }
}

/// Between a file card's header and its view of the tab.
const HEADER_GAP: Space = Space::Md;

/// How tall a file card's view of its tab stands at most: the card's height
/// less its pad, the header (two lines with a `folder`), the footer, and the
/// gaps between.
fn card_view_height(folder: bool) -> f32 {
    FILE_CARD.y
        - 2.0 * CARD_PAD.pts()
        - card_header_height(folder)
        - HEADER_GAP.pts()
        - crate::style::control_height()
        - Space::Sm.pts()
}

/// A file card's row, less the pad above its name.
fn card_header_height(folder: bool) -> f32 {
    FileRow::height_for(folder) - row_pad()
}

/// The pad a file row keeps above and below its name line.
fn row_pad() -> f32 {
    (ROW_H - TypeRole::Body.line_height()) / 2.0
}

/// Opens the embedded file from the slot's corner, shown while hovered.
fn open_button(ui: &mut egui::Ui, rect: Rect, place: &Place) {
    if !ui.rect_contains_pointer(rect) {
        return;
    }
    let Some(id) = place.dest.backing_file() else { return };
    let t = ui.ctx().get_lb_theme();
    let side = crate::style::control_height();
    let at =
        Rect::from_min_size(pos2(rect.right() - side - 4.0, rect.top() + 4.0), vec2(side, side));
    let ui = &mut ui.new_child(UiBuilder::new().max_rect(at));
    let resp = icon_button(ui, &t, ARROW_SQUARE_OUT, false, t.neutral_bg());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
    if resp.clicked() {
        let new_tab = ui.input(|i| i.modifiers.command);
        let heading = match &place.fragment {
            Some(Fragment::Heading(slug)) => Some(slug.clone()),
            _ => None,
        };
        ui.ctx().open_link(id, heading, new_tab);
    }
}

/// Faint bars standing in for the content while the tab loads.
fn paint_skeleton(ui: &egui::Ui, rect: Rect) {
    let color = ui.style().visuals.text_color().gamma_multiply(0.12);
    let cr = CornerRadius::same(3);
    let bar = |y: f32, w: f32, h: f32| {
        let min = pos2(rect.min.x, rect.min.y + y);
        ui.painter()
            .rect_filled(Rect::from_min_size(min, vec2(w, h)), cr, color);
    };
    let w = rect.width().max(0.0);
    bar(0.0, w * 0.5, 14.0);
    bar(26.0, w, 10.0);
    bar(44.0, w * 0.8, 10.0);
}
