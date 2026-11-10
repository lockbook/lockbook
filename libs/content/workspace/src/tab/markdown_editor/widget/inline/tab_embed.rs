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
use lb_rs::model::text::operation_types::Operation;

use super::link::LinkMenuAction;
use super::link::hover::{CARD_PAD, CARD_RADIUS, FILE_CARD, ink_secondary};
use crate::file_cache::{FilesExt as _, ResolvedLink, split_fragment, wiki_fragment};
use crate::resolvers::image_embed::wiki_embed_key;
use crate::show::ElapsedHumanString as _;
use crate::style::button::Button;
use crate::style::tree_metrics::INDENT_BASE;
use crate::style::{FileRow, ROW_H, Space, TypeRole, file_row_icon, parent_crumbs};
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::widget::inline::image::image::image_url;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};
use crate::tab::markdown_editor::{MdEdit, MdRender, TouchTarget};
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

    /// Where shown tabs' own tap targets lie, such as their links: a touch
    /// there is the tab's, not the slot's.
    pub fn inner_targets(&self) -> Vec<Rect> {
        let slots = self.slots.borrow();
        let mut targets = vec![];
        for (key, slot) in slots.iter() {
            let SlotState::Open(tab) = &slot.state else { continue };
            let Some(md) = tab.markdown() else { continue };
            if key.card || !slot.touched || !slot.shown {
                continue;
            }
            let inner = md.edit.renderer.touch_targets.iter();
            let taps = inner.filter(|(_, target)| matches!(target, TouchTarget::Tap(_)));
            targets.extend(taps.map(|(rect, _)| rect.intersect(slot.rect)));
        }
        targets.retain(|rect| rect.is_positive());
        targets
    }

    /// Closes the tabs whose slots didn't paint, near the view or not.
    pub fn end_frame(&self) {
        self.slots.borrow_mut().retain(|_, slot| slot.touched);
    }

    /// The place `title` embeds, if it can: a document past no cycle and
    /// within the depth, while something can build its tab. A note shows a
    /// heading of its own once: the copy shows none of itself.
    fn place(&self, renderer: &MdRender, title: &str) -> Option<Place> {
        let place = self.reach(renderer, title)?;
        let id = place.dest.backing_file()?;
        let shown = self.path.iter().filter(|shown| **shown == id).count();
        let own_heading = shown == 1
            && self.path.last() == Some(&id)
            && matches!(place.fragment, Some(Fragment::Heading(_)));
        (self.path.len() < EMBED_DEPTH && (shown == 0 || own_heading)).then_some(place)
    }

    /// The document `title` names, for a card, which nests nothing.
    fn reach(&self, renderer: &MdRender, title: &str) -> Option<Place> {
        let res = self.res.as_ref()?;
        let id = renderer.resolve_wikilink(title)?;
        let named_whole = res.files.read().unwrap().wiki_file_part(title) == title;
        let heading = wiki_fragment(title).1.filter(|_| !named_whole);
        self.place_of(res, id, heading)
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

    /// Draws the slot `key` names into `rect`, opening its tab first, the
    /// tab `band` lower than the slot's pad for what heads the slot.
    fn paint(&self, ui: &mut egui::Ui, key: SlotKey, place: Place, rect: Rect, band: f32) {
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
        let inner = inner.with_min_y(inner.top() + band);
        match &mut slot.state {
            SlotState::Open(tab) => {
                // the pad takes what hangs past the content: a list's numbers, a last line's descent
                let clip = rect.intersect(ui.clip_rect());
                let child = &mut ui.new_child(UiBuilder::new().max_rect(inner).scope_id(tab.salt));
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
        }
    }
}

impl<'ast> MdEdit {
    /// A click on a tab embed, wherever its tab has no widget of its own,
    /// does what one on an image does ([`MdEdit::handle_embed_tap`]).
    pub fn handle_tab_embed_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui);
        let click = EmbedClick::take(ui.ctx());
        for node in root.descendants() {
            let Some(title) = tab_embed_title(node) else { continue };
            let range = self.renderer.node_range(node);
            let salt = MdRender::tab_embed_interaction_id_salt(range);
            self.handle_embed_tap(root, node, ui, id, ops, &wiki_embed_key(&title), salt, open);
            // a click on the tab's body, which its editor handed on
            let rects = self
                .renderer
                .interaction_rects
                .get(&ui.scope_id().with(salt));
            let Some(click) =
                click.filter(|c| rects.is_some_and(|r| r.iter().any(|r| r.contains(c.pos))))
            else {
                continue;
            };
            EmbedClick::clear(ui.ctx());
            if self.renderer.readonly || click.command {
                self.renderer.open_dest(&title, true, ui.ctx());
            } else {
                ui.memory_mut(|m| m.request_focus(id));
                let select = Event::Select { region: range.into() };
                self.calc_operations(ui.ctx(), root, select, ops);
            }
        }
    }
}

/// A click on an embedded editor that nothing in it took, handed to the
/// editor whose embed is under it (on the next frame: hosts act before
/// their embeds paint). Kept in the context, which every pass shares.
#[derive(Clone, Copy)]
pub struct EmbedClick {
    pub pos: egui::Pos2,
    pub command: bool,
    pass: u64,
}

impl EmbedClick {
    fn id() -> egui::Id {
        egui::Id::unique("md_embed_click")
    }

    pub fn hand_on(ctx: &egui::Context, pos: egui::Pos2) {
        let command = ctx.input(|i| i.modifiers.command);
        let click = EmbedClick { pos, command, pass: ctx.cumulative_pass_nr() };
        ctx.data_mut(|d| d.insert_temp(Self::id(), click));
    }

    /// Last frame's click, if there is one.
    fn take(ctx: &egui::Context) -> Option<Self> {
        let click = ctx.data(|d| d.get_temp::<Self>(Self::id()))?;
        (click.pass + 1 == ctx.cumulative_pass_nr()).then_some(click)
    }

    fn clear(ctx: &egui::Context) {
        ctx.data_mut(|d| d.remove::<Self>(Self::id()));
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
        // a width after the pipe, as an image takes one, within its block's
        let room = self.embed_room(block);
        let width = self
            .image_dims(node)
            .width
            .map_or(room, |width| width.min(room));
        let pad = 2.0 * CARD_PAD.pts();
        if width <= pad {
            return None;
        }
        let height = self
            .tab_embeds
            .content_height(&title, &place, width - pad, false);
        let band = self.tab_embed_band_height(&place);
        Some((place, vec2(width, band + height + pad)))
    }

    /// What heads a note embed: its file's row, as its hover card shows it,
    /// with the row's own pad above and below.
    fn tab_embed_band_height(&self, place: &Place) -> f32 {
        FileRow::height_for(self.card_folder(place).is_some())
    }

    /// The band atop the note embed of `title` drawn in `rect`: where a
    /// hold lifts it, the inside being the embedded note's.
    pub(crate) fn tab_embed_band(&self, title: &str, rect: Rect) -> Option<Rect> {
        let place = self.tab_embeds.place(self, title)?;
        Some(rect.with_max_y(rect.top() + self.tab_embed_band_height(&place)))
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
        let node_range = self.node_range(node);
        layout.interaction_open(Self::tab_embed_interaction_id_salt(node_range), Sense::click());
        layout.push_embed(EmbedSpec {
            advance: size.x,
            ascent: size.y,
            descent: 0.0,
            source_range: node_range,
            url,
            kind: EmbedKind::Tab,
        });
        layout.interaction_close();
        true
    }

    pub fn tab_embed_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_tab_embed", node_range))
    }

    /// Paints the tab an embed slot holds; `at` is where its source starts,
    /// which tells repeated embeds of one title apart.
    pub fn paint_tab_embed(&mut self, ui: &mut egui::Ui, title: &str, rect: Rect, at: Grapheme) {
        let name = title;
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
        let band = self.tab_embed_band_height(&place);
        // the title: the file's row on the secondary ground, a rule under it
        let t = ui.ctx().get_lb_theme();
        // the page's ground, so the selection slot behind hides as it does behind an image
        let ground = egui::Frame::canvas(ui.style()).fill;
        ui.painter().rect_filled(rect, CARD_RADIUS.corner(), ground);
        let title = rect.with_max_y(rect.top() + band);
        let r = CARD_RADIUS.corner();
        let top = CornerRadius { nw: r.nw, ne: r.ne, sw: 0, se: 0 };
        ui.painter()
            .rect_filled(title, top, t.neutral_bg_secondary());
        // The Open button shows while a pointer is over the embed, or on
        // touch once a tap on the title shows it, the next tap hiding it.
        // each note keeps its own: two boards embedding one ticket don't share
        let note = self.tab_embeds.path.last().copied();
        let shown = egui::Id::unique(("md_embed_open", note, name, nth));
        if self.touch_mode && !self.painting_drag_float {
            // egui takes the tap, not the platform's text view, ahead of the
            // embed's own target under it: the first that holds a point wins
            self.touch_targets.insert(0, (title, TouchTarget::Popup));
            if ui
                .interact(title, shown.with("tap"), Sense::click())
                .clicked()
            {
                ui.data_mut(|d| {
                    let on = d.get_temp_mut_or_default::<bool>(shown);
                    *on = !*on;
                });
                ui.ctx().request_repaint();
            }
        }
        let show_open = if self.touch_mode {
            ui.data(|d| d.get_temp::<bool>(shown).unwrap_or(false))
        } else {
            ui.rect_contains_pointer(rect)
        };
        // the row stops short of the Open button at its right
        let open = if show_open { self.open_button_width() + Space::Sm.pts() } else { 0.0 };
        let header = Rect::from_min_max(
            pos2(rect.left() + CARD_PAD.pts(), title.top() + row_pad()),
            pos2(rect.right() - CARD_PAD.pts() - open, title.bottom() - row_pad()),
        );
        let header_ui = &mut ui.new_child(UiBuilder::new().max_rect(header));
        let folder = self.card_folder(&place);
        let heading = Self::named_heading(&place, wiki_fragment(name).1);
        self.card_header(header_ui, &place, heading, folder, header.width());
        let rule = egui::Stroke::new(1.0, t.neutral_bg_tertiary());
        ui.painter().hline(rect.x_range(), title.bottom(), rule);
        self.tab_embeds.paint(ui, key, place.clone(), rect, band);
        if show_open {
            open_button(ui, title, &place);
        }
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
        crate::style::tip_card_reachable(
            ctx,
            response,
            response.rect,
            FILE_CARD.x - 2.0 * CARD_PAD.pts(),
            egui::Margin::same(CARD_PAD.pts() as i8),
            |ui, width| {
                let folder = self.card_folder(&place);
                let height = self
                    .tab_embeds
                    .content_height(url, &place, width, true)
                    .min(card_view_height(folder.is_some()));
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = HEADER_GAP.pts();
                let heading = Self::named_heading(&place, url.split_once('#').map(|(_, h)| h));
                self.card_header(ui, &place, heading, folder, width);
                ui.spacing_mut().item_spacing.y = Space::Sm.pts();
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                let key = SlotKey { url: url.into(), nth: 0, card: true };
                self.tab_embeds.paint(ui, key, place.clone(), rect, 0.0);
                if let Some(action) = self.card_file_footer(ui, &place) {
                    *self.card_action.borrow_mut() = Some((url.to_string(), action));
                    crate::style::dismiss_tip(ui.ctx());
                    ui.ctx().request_repaint();
                }
            },
        );
        true
    }

    /// The quiet Open button's width: its label and the control's pads.
    fn open_button_width(&self) -> f32 {
        self.body_width("Open") + 2.0 * crate::style::space::control::PAD_X.pts()
    }

    fn body_width(&self, text: &str) -> f32 {
        let label = GlyphonLabel::new(text, egui::Color32::WHITE)
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height());
        label.build(&self.ctx).size.x.ceil()
    }

    /// The heading a link names after its `#`, when it lands on one.
    fn named_heading<'u>(place: &Place, fragment: Option<&'u str>) -> Option<&'u str> {
        fragment.filter(|_| matches!(place.fragment, Some(Fragment::Heading(_))))
    }

    /// The folders `place`'s file sits in, as crumbs, or Home at the root.
    fn card_folder(&self, place: &Place) -> Option<String> {
        let files = self.files.read().unwrap();
        let file = files.get_by_id(place.dest.backing_file()?)?;
        Some(parent_crumbs(&files.path(file.id)))
    }

    /// A file card's top: `place`'s file as a row, under its name the
    /// `folder` it's in, its icon in line with the tab's content under it.
    /// A `heading` the link names heads the row instead, the note under it.
    fn card_header(
        &self, ui: &mut egui::Ui, place: &Place, heading: Option<&str>, folder: Option<String>,
        width: f32,
    ) {
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
        let (name, under) = match heading {
            Some(heading) => {
                let note = file.name.strip_suffix(".md").unwrap_or(&file.name);
                (heading.to_string(), note.to_string())
            }
            None => (file.name.clone(), folder.unwrap_or_default()),
        };
        FileRow::new(&t, name)
            .icon(file_row_icon(&file.name, false))
            .subtitle(under)
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

/// Opens the embedded file from the right of the slot's `title`.
fn open_button(ui: &mut egui::Ui, title: Rect, place: &Place) {
    let Some(id) = place.dest.backing_file() else { return };
    let t = ui.ctx().get_lb_theme();
    // the hover card's Open, quiet: its plate is the page's, on the tinted title
    let side = crate::style::control_height();
    let area = Rect::from_x_y_ranges(
        title.left()..=title.right() - CARD_PAD.pts(),
        title.center().y - side / 2.0..=title.center().y + side / 2.0,
    );
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    let ui = &mut ui.new_child(UiBuilder::new().max_rect(area).layout(layout));
    let resp = Button::quiet(&t, "Open").show(ui);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ws::TestWs;
    use lb_rs::model::file_metadata::FileType;
    use web_time::{Duration, Instant};

    /// A pdf of `pages` blank letter pages.
    fn blank_pdf(pages: usize) -> Vec<u8> {
        let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")),
        ];
        for _ in 0..pages {
            objects.push("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".into());
        }
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![];
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
        for offset in offsets {
            pdf.extend(format!("{offset:010} 00000 n \n").bytes());
        }
        let trailer = format!("trailer\n<< /Size {} /Root 1 0 R >>\n", objects.len() + 1);
        pdf.extend(format!("{trailer}startxref\n{xref}\n%%EOF\n").bytes());
        pdf
    }

    /// The page the note's one pdf embed shows, once it shows one.
    fn embedded_page(t: &mut TestWs) -> Option<usize> {
        let md = t.ws.current_tab_mut()?.markdown_mut()?;
        let slots = md.edit.renderer.tab_embeds.slots.borrow();
        let slot = slots.values().find(|slot| slot.shown)?;
        let SlotState::Open(tab) = &slot.state else { return None };
        let ContentState::Open(TabContent::Pdf(pdf)) = &tab.content else { return None };
        Some(pdf.current_page())
    }

    fn frames_until(t: &mut TestWs, done: impl Fn(&mut TestWs) -> bool) {
        let start = Instant::now();
        while !done(t) {
            assert!(start.elapsed() < Duration::from_secs(10), "never got there");
            t.frame();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn click(t: &mut TestWs, pos: egui::Pos2) {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        t.frame_with(vec![egui::Event::PointerMoved(pos), button(true), button(false)]);
        t.frame();
    }

    /// A note embedding `Plan`, which links to `Other`, once the embed shows.
    fn embed_of_a_link(t: &mut TestWs) -> (Uuid, Uuid) {
        let other = t.doc("Other.md", "elsewhere\n");
        let plan = t.doc("Plan.md", "# Goals\n\nShip it, then [[Other]].\n");
        let note = t.doc("note.md", "above\n\n![[Plan]]\n\nbelow\n");
        t.ws.open_file(note, true, true);
        frames_until(t, |t| shown(t).is_some());
        t.frame();
        (plan, other)
    }

    /// The shown slot's rect and its tab's fragments of text, in screen space.
    fn shown(t: &mut TestWs) -> Option<(Rect, Vec<(String, Rect)>)> {
        let md = t.ws.current_tab_mut()?.markdown_mut()?;
        let slots = md.edit.renderer.tab_embeds.slots.borrow();
        let slot = slots.values().find(|slot| slot.shown)?;
        let SlotState::Open(tab) = &slot.state else { return None };
        let ContentState::Open(TabContent::Markdown(inner)) = &tab.content else { return None };
        let r = &inner.edit.renderer;
        let texts = r
            .fragments
            .iter()
            .map(|f| (r.buffer[f.source_range].to_string(), f.rect));
        Some((slot.rect, texts.collect()))
    }

    /// `![[#Chairs]]` embeds the note's own heading, as `![[Board#Chairs]]` does.
    #[test]
    fn a_heading_alone_embeds_the_notes_own_heading() {
        let mut t = TestWs::new();
        let board = t.doc("Board.md", "# Chairs\n\nfour\n\n# Tables\n\n![[#Chairs]]\n");
        t.ws.open_file(board, true, true);
        frames_until(&mut t, |t| {
            t.ws.current_tab_mut()
                .and_then(|tab| tab.markdown_mut())
                .is_some()
        });
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        let r = &md.edit.renderer;
        let named = r.tab_embeds.place(r, "Board#Chairs");
        assert!(named.is_some());
        assert_eq!(r.tab_embeds.place(r, "#Chairs"), named);
    }

    /// A `#` that is part of a file's name names no heading.
    #[test]
    fn a_hash_in_a_files_name_is_no_heading() {
        let mut t = TestWs::new();
        t.doc("Plan.md", "# Q1\n");
        let named = t.doc("Plan#Q1.md", "body\n");
        let note = t.doc("note.md", "![[Plan#Q1]]\n");
        t.ws.open_file(note, true, true);
        frames_until(&mut t, |t| {
            t.ws.current_tab_mut()
                .and_then(|tab| tab.markdown_mut())
                .is_some()
        });
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        let r = &md.edit.renderer;
        assert_eq!(
            r.tab_embeds.place(r, "Plan#Q1"),
            Some(Place::at(Destination::File(named), None))
        );
    }

    /// A width after the pipe sizes a note embed, as it does an image.
    #[test]
    fn a_width_after_the_pipe_sizes_an_embed() {
        let mut t = TestWs::new();
        t.doc("Plan.md", "# Goals\n\nShip it.\n");
        let note = t.doc("note.md", "above\n\n![[Plan|200]]\n\nbelow\n");
        t.ws.open_file(note, true, true);
        frames_until(&mut t, |t| shown(t).is_some());
        let (rect, _) = shown(&mut t).unwrap();
        assert_eq!(rect.width(), 200.0);
    }

    /// A click on a note embed's body, away from its links, selects the embed.
    #[test]
    fn click_on_an_embed_selects_it() {
        let mut t = TestWs::new();
        embed_of_a_link(&mut t);
        let (rect, _) = shown(&mut t).unwrap();
        click(&mut t, rect.center_bottom() - vec2(0.0, 20.0));
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert_eq!(md.edit.renderer.buffer.current.selection, (Grapheme(7), Grapheme(16)));
    }

    /// Backspace after an embed selects it, as after an image; a second deletes it.
    #[test]
    fn backspace_after_an_embed_selects_it() {
        use crate::tab::ExtendedInput;
        use crate::tab::markdown_editor::input::{Advance, Event, Increment, Region};
        let mut t = TestWs::new();
        embed_of_a_link(&mut t);
        let backspace = || Event::Delete {
            region: Region::SelectionOrAdvance {
                advance: Advance::By(Increment::Char),
                backwards: true,
            },
        };
        t.ctx
            .push_markdown_event(Event::Select { region: (Grapheme(16), Grapheme(16)).into() });
        t.frame();
        t.ctx.push_markdown_event(backspace());
        t.frame();
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert_eq!(md.edit.renderer.buffer.current.selection, (Grapheme(7), Grapheme(16)));
        t.ctx.push_markdown_event(backspace());
        t.frame();
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert_eq!(md.edit.renderer.buffer.current.text, "above\n\n\n\nbelow\n");
    }

    /// A drag across an embed's text selects it there, for Copy, and leaves the
    /// note's own selection alone.
    #[test]
    fn drag_in_an_embed_selects_its_text() {
        let mut t = TestWs::new();
        embed_of_a_link(&mut t);
        let before = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        let outer = before.edit.renderer.buffer.current.selection;
        let (_, texts) = shown(&mut t).unwrap();
        let (_, words) = texts
            .iter()
            .find(|(text, _)| text.contains("Ship"))
            .unwrap();
        let (from, to) = (words.left_center() + vec2(2.0, 0.0), words.right_center());
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        t.frame_with(vec![egui::Event::PointerMoved(from), button(from, true)]);
        for i in 1..=4 {
            t.frame_with(vec![egui::Event::PointerMoved(from.lerp(to, i as f32 / 4.0))]);
        }
        t.frame_with(vec![button(to, false)]);
        t.frame();
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert_eq!(md.edit.renderer.buffer.current.selection, outer);
        assert!(
            md.edit
                .renderer
                .tab_embeds
                .copy()
                .is_some_and(|text| text.contains("Ship"))
        );
    }

    /// A link in an embed still opens: the embed's click sits beneath it.
    #[test]
    fn link_in_an_embed_opens() {
        let mut t = TestWs::new();
        let (_, other) = embed_of_a_link(&mut t);
        let (_, texts) = shown(&mut t).unwrap();
        let (_, link) = texts
            .iter()
            .find(|(text, _)| text.contains("Other"))
            .unwrap();
        click(&mut t, link.center());
        frames_until(&mut t, |t| {
            t.ws.current_tab()
                .and_then(|tab| tab.destination.backing_file())
                == Some(other)
        });
    }

    /// On touch too, where the embedded note has no menu of its own.
    #[test]
    fn link_in_an_embed_opens_on_touch() {
        let mut t = TestWs::new();
        t.ctx.set_os(egui::os::OperatingSystem::Android);
        let (_, other) = embed_of_a_link(&mut t);
        let (_, texts) = shown(&mut t).unwrap();
        let (_, link) = texts
            .iter()
            .find(|(text, _)| text.contains("Other"))
            .unwrap();
        click(&mut t, link.center());
        frames_until(&mut t, |t| {
            t.ws.current_tab()
                .and_then(|tab| tab.destination.backing_file())
                == Some(other)
        });
    }

    /// On iOS a tap goes where the touch targets say: on an embed's link it is
    /// egui's, which opens it; elsewhere on the embed it selects it.
    #[test]
    fn touch_targets_on_an_embed() {
        let mut t = TestWs::new();
        t.ctx.set_os(egui::os::OperatingSystem::IOS);
        embed_of_a_link(&mut t);
        let (rect, texts) = shown(&mut t).unwrap();
        let (_, link) = texts
            .iter()
            .find(|(text, _)| text.contains("Other"))
            .unwrap();
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        let on_link = md.edit.touch_target_at(link.center());
        assert!(matches!(on_link, Some(TouchTarget::Popup)), "{on_link:?}");
        // below the title band, which is egui's
        let on_body = md
            .edit
            .touch_target_at(rect.left_center() + vec2(20.0, 0.0));
        assert!(matches!(on_body, Some(TouchTarget::Tap(_))), "{on_body:?}");
    }

    /// On touch, a tap on a note embed's title band is the band's, which shows
    /// Open, not the embed's, which would select it.
    #[test]
    fn a_tap_on_an_embeds_band_leaves_the_selection() {
        use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, FragmentContent};
        let mut t = TestWs::new();
        t.ctx.set_os(egui::os::OperatingSystem::IOS);
        embed_of_a_link(&mut t);
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        let r = &md.edit.renderer;
        let selection = r.buffer.current.selection;
        let embed = r
            .fragments
            .iter()
            .find(|f| matches!(f.content, FragmentContent::Embed { kind: EmbedKind::Tab, .. }));
        let band = embed.expect("the embed draws").rect.center_top() + vec2(0.0, 10.0);
        let on_band = md.edit.touch_target_at(band);
        assert!(matches!(on_band, Some(TouchTarget::Popup)), "{on_band:?}");
        click(&mut t, band);
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert_eq!(md.edit.renderer.buffer.current.selection, selection);
    }

    /// An embed stays on the page it names while the note is edited above it.
    #[test]
    fn pdf_embed_keeps_its_page_through_an_edit_above() {
        let mut t = TestWs::new();
        let root = t.ws.core.get_root().unwrap().id;
        let pdf =
            t.ws.create_file_at(root, "paper.pdf", FileType::Document, false)
                .unwrap();
        t.ws.core.write_document(pdf.id, &blank_pdf(3)).unwrap();
        let note = t.doc("note.md", "above\n\n![[paper.pdf#page=2]]\n");
        t.ws.open_file(note, true, true);
        frames_until(&mut t, |t| embedded_page(t) == Some(1));

        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        t.frame_with(vec![enter]);
        let md = t.ws.current_tab_mut().unwrap().markdown_mut().unwrap();
        assert!(md.edit.renderer.buffer.current.text.starts_with("\nabove"));
        for _ in 0..10 {
            t.frame();
        }
        assert_eq!(embedded_page(&mut t), Some(1));
    }
}
