//! The tabs notes show in their embed slots. A note lays its slots out at the
//! heights measured here and asks for their tabs as it draws
//! ([`EmbedRequest`]); the workspace keeps those tabs in its tab cache, like
//! any session off the tab strip, and draws them into their slots once the
//! note is done. Each frame, before a note draws, its embeds are measured at
//! the widths it last asked for, deepest first, so it lays out at their
//! heights; what changes them while they draw shows the frame after.
//!
//! An embed of a note this account can write edits it: its tab saves like
//! any other, and one whose note closes with edits unsaved stays in the cache
//! until they are. An embed of a heading of the note it's in shows the note's
//! buffer instead, and hands what it edits to the note.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use egui::{Rect, UiBuilder};

use crate::show::DocType;
use crate::tab::markdown_editor::Syntax;
use crate::tab::markdown_editor::input::Event;
use lb_rs::model::text::offset_types::{Byte, Grapheme};
use lb_rs::model::text::operation_types::{Operation, Replace};

use crate::tab::markdown_editor::fragment::{find_heading, headings_in};
use crate::tab::markdown_editor::widget::inline::tab_embed::{
    EmbedKey, EmbedRequest, EmbedState, Leave, lacks_heading, load, missing_heading,
};
use crate::tab::{
    ContentState, Doc, ExtendedOutput as _, Fragment, Markdown, Place, Presentation, Reveal,
    SessionId, Tab, TabContent, TabFailure, TabResources,
};
use crate::widgets::tab_cache::TabCache;

type Read = Receiver<Result<Option<Doc>, TabFailure>>;

/// Which note embeds are filed under: its tab's salt and its file, so a tab
/// that moves to another note, or a preview of another, starts afresh.
pub type Host = (egui::Id, lb_rs::Uuid);

#[derive(Default)]
pub struct EmbedHost {
    embeds: HashMap<(Host, EmbedKey), Embed>,
    /// What each note asked for when it last drew.
    asked: HashMap<Host, Vec<EmbedRequest>>,
    /// The embed the last press landed in: its selection is what Copy takes.
    active: Option<SessionId>,
    /// Tabs of embeds no note shows any more, kept until their edits save.
    orphans: Vec<SessionId>,
    /// Documents written, so the tabs showing them reload.
    #[cfg(not(target_family = "wasm"))]
    writes: Option<lb_rs::service::events::Receiver<lb_rs::service::events::Event>>,
}

struct Embed {
    /// Where its tab is in the cache.
    session: SessionId,
    place: Place,
    /// Its tab opened to edit what it shows.
    writable: bool,
    /// It shows a heading of the note it's in.
    own: bool,
    load: Option<Read>,
    /// A newer version of the document, to take in place.
    reload: Option<Read>,
    failed: Option<String>,
    /// The tab has settled once; the skeleton is gone for good.
    shown: bool,
    /// Where its tab last drew.
    rect: Rect,
    /// Its tab drew this frame, or last frame until the next begins.
    drawn: bool,
}

impl EmbedHost {
    /// Keeps the embeds of the notes still open and the tabs they show,
    /// reads again the documents written since, and gives Copy the selection
    /// in the embed last pressed.
    pub fn begin_frame(
        &mut self, ctx: &egui::Context, tabs: &mut TabCache, res: &TabResources, open: &[Host],
    ) {
        // the notes still open, and the tabs in their embeds, all the way down
        let mut kept: HashSet<Host> = HashSet::new();
        let mut hosts: Vec<Host> = open.to_vec();
        while let Some(host) = hosts.pop() {
            if !kept.insert(host) {
                continue;
            }
            for request in self.asked.get(&host).into_iter().flatten() {
                let Some(embed) = self.embeds.get(&(host, request.key.clone())) else { continue };
                hosts.extend(tabs.get_any(&embed.session).and_then(Tab::embed_host));
            }
        }
        let orphans = &mut self.orphans;
        self.embeds.retain(|(host, _), embed| {
            let keep = kept.contains(host);
            if !keep {
                embed.close(tabs, orphans);
            }
            keep
        });
        self.asked.retain(|host, _| kept.contains(host));
        for embed in self.embeds.values() {
            tabs.promote(&embed.session);
        }
        // the cache keeps an unsaved tab, and drops it once it's saved
        self.orphans.retain(|session| tabs.contains_key(session));
        #[cfg(not(target_family = "wasm"))]
        self.reload_written(res);
        self.copy(ctx, tabs);
        for embed in self.embeds.values_mut() {
            embed.drawn = false;
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn reload_written(&mut self, res: &TabResources) {
        use lb_rs::service::events::{Actor, Event};
        let writes = self.writes.get_or_insert_with(|| res.core.subscribe());
        while let Ok(event) = writes.try_recv() {
            let Event::DocumentWritten(id, actor) = event else { continue };
            for embed in self.embeds.values_mut() {
                let own_write = actor == Actor::User(Some(embed.session.as_uuid()));
                // a heading of the note shows the note's buffer, not its disk
                if embed.place.dest.backing_file() == Some(id) && !own_write && !embed.own {
                    embed.reload = Some(load(res, embed.place.dest.clone()));
                }
            }
        }
    }

    fn copy(&mut self, ctx: &egui::Context, tabs: &mut TabCache) {
        let press = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()));
        // the innermost embed drawn under the press: a nested one's own
        if let Some(press) = press {
            let pressed = |e: &&Embed| e.drawn && press.is_some_and(|p| e.rect.contains(p));
            let pressed = self.embeds.values().filter(pressed);
            let innermost = pressed.min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()));
            self.active = innermost.map(|e| e.session);
        }
        if !ctx.input(|i| i.events.contains(&egui::Event::Copy)) {
            return;
        }
        let Some(md) = self
            .active
            .and_then(|id| tabs.get_any_mut(&id))
            .and_then(|tab| tab.markdown())
        else {
            return;
        };
        let buffer = &md.edit.renderer.buffer;
        let (a, b) = buffer.current.selection;
        if a != b {
            ctx.copy_text(buffer[(a.min(b), a.max(b))].to_string());
            ctx.input_mut(|i| i.events.retain(|e| *e != egui::Event::Copy));
        }
    }

    /// What `host`'s embeds' tabs show, measured at the widths it last asked
    /// for: theirs first, so each lays out at its own embeds' heights.
    pub fn states(
        &mut self, host: Host, tabs: &mut TabCache, res: &TabResources,
    ) -> HashMap<EmbedKey, EmbedState> {
        let mut states = HashMap::new();
        for request in self.asked.get(&host).cloned().unwrap_or_default() {
            let key = (host, request.key.clone());
            let Some(embed) = self.embeds.get_mut(&key) else { continue };
            embed.poll(tabs, res, &request);
            let (session, shown) = (embed.session, embed.shown);
            let state = match (&embed.failed, &embed.load) {
                (Some(msg), _) => EmbedState::Failed(msg.clone()),
                (None, Some(_)) => EmbedState::Loading,
                (None, None) => {
                    let inner = match tabs.get_any(&session).and_then(Tab::embed_host) {
                        Some(inner) => self.states(inner, tabs, res),
                        None => HashMap::new(),
                    };
                    match tabs.get_any_mut(&session) {
                        Some(tab) => {
                            tab.set_embed_states(inner);
                            let fragment = request.place.fragment.as_ref();
                            let height = shown
                                .then(|| tab.region_height(fragment, request.rect.width()))
                                .flatten();
                            EmbedState::Open { shown, height }
                        }
                        None => EmbedState::Loading,
                    }
                }
            };
            states.insert(request.key, state);
        }
        states
    }

    /// Draws the tabs `host` asked for this frame into their slots, theirs
    /// after each, then has each paint over its own slots. Returns where the
    /// tabs have targets of their own, for `host`'s touches. `host_tab` is
    /// the session of the note asking, whose own headings show its buffer.
    pub fn draw(
        &mut self, ui: &mut egui::Ui, host: Host, host_tab: Option<SessionId>,
        requests: Vec<EmbedRequest>, tabs: &mut TabCache, res: &TabResources,
    ) -> Vec<Rect> {
        let asked: HashSet<&EmbedKey> = requests.iter().map(|r| &r.key).collect();
        let orphans = &mut self.orphans;
        self.embeds.retain(|(h, key), embed| {
            let keep = *h != host || asked.contains(key);
            if !keep {
                embed.close(tabs, orphans);
            }
            keep
        });
        let mut targets = vec![];
        // what the note was sent while an embed had the caret, that embed's
        let mut to_focused = host_md(tabs, host_tab)
            .map(|md| md.edit.renderer.tab_embeds.to_focused.take())
            .unwrap_or_default();
        let mut caret = false;
        // a tab draws once a frame: a lifted embed's last ask, under the pointer, wins
        let last = |i: usize, r: &EmbedRequest| !requests[i + 1..].iter().any(|n| n.key == r.key);
        let drawn = requests.iter().enumerate().filter(|(i, r)| last(*i, r));
        for (_, request) in drawn {
            let key = (host, request.key.clone());
            let own = request.place.dest.backing_file() == Some(host.1);
            let open = || Embed::open(res, request.place.clone(), request.writable, own);
            let embed = self.embeds.entry(key.clone()).or_insert_with(open);
            // a share granted or taken while it shows opens it again, as it now may be
            if embed.place != request.place || embed.writable != request.writable {
                embed.close(tabs, &mut self.orphans);
                *embed = open();
            }
            embed.poll(tabs, res, request);
            if own {
                if let Some(host_tab) = host_tab {
                    embed.mirror(tabs, host_tab);
                }
            }
            let (session, shown) = (embed.session, embed.shown);
            embed.rect = request.clip;
            tabs.promote(&session);
            // its slot says why it shows none
            if embed.failed.is_some() {
                continue;
            }
            let Some(tab) = tabs.get_any_mut(&session) else { continue };
            // a printed note's embeds print with it
            tab.presentation = match crate::printing(ui.ctx()) {
                true => Presentation::Print,
                false => Presentation::Embedded,
            };
            if let Some(md) = tab.markdown_mut() {
                if !to_focused.is_empty() && md.focused(ui.ctx()) {
                    md.edit.event.internal_events.append(&mut to_focused);
                } else if md.edit.renderer.tab_embeds.has_caret() {
                    let nested = &md.edit.renderer.tab_embeds.to_focused;
                    nested.borrow_mut().append(&mut to_focused);
                }
            }
            let builder = UiBuilder::new()
                .max_rect(request.rect)
                .layer_id(request.layer)
                .scope_id(tab.salt);
            let child = &mut ui.new_child(builder);
            child.set_clip_rect(request.clip);
            if !shown {
                child.set_invisible();
            }
            let before = tab.markdown().map(|md| {
                let r = &md.edit.renderer;
                (r.text_seq, r.buffer.current.selection)
            });
            let resp = match res.watched {
                true => res.guard.show(tab, |tab| tab.show(child)),
                false => tab.show(child),
            };
            if let (Some(before), Some(host_tab)) = (before, host_tab) {
                hand_over(ui.ctx(), tabs, session, host_tab, request, own, before);
            }
            let Some(tab) = tabs.get_any_mut(&session) else { continue };
            if let Some(id) = resp.open_file {
                ui.ctx().open_file(id, false);
            }
            let settled = !shown && tab.settled();
            let (inner, inner_host) = (tab.embed_requests(), tab.embed_host());
            let inner = match inner_host {
                Some(inner_host) => self.draw(ui, inner_host, Some(session), inner, tabs, res),
                None => vec![],
            };
            let Some(tab) = tabs.get_mut(&session) else { continue };
            caret |= tab
                .markdown()
                .is_some_and(|md| md.focused(ui.ctx()) || md.edit.renderer.tab_embeds.has_caret());
            tab.paint_embed_overs(ui);
            tab.embed_targets(inner);
            // a tab drawn hidden while it settles takes no touches
            if shown {
                let slot = request.clip;
                let taps = tab
                    .tap_targets()
                    .into_iter()
                    .map(|rect| rect.intersect(slot));
                targets.extend(taps.filter(|rect| rect.is_positive()));
            }
            let embed = self.embeds.get_mut(&key).expect("drawn this frame");
            embed.drawn = true;
            if settled {
                embed.shown = true;
                let (fragment, width) = (request.place.fragment.as_ref(), request.rect.width());
                if let Some(height) = tab.region_height(fragment, width) {
                    res.remember_height(&request.place.dest, fragment, width, height);
                }
                ui.ctx().request_repaint();
            }
        }
        if let Some(md) = host_md(tabs, host_tab) {
            md.edit.renderer.tab_embeds.caret.set(caret);
        }
        self.asked.insert(host, requests);
        targets
    }

    /// The file of the embedded editor that has the caret, and whether a
    /// pasted image goes in as a link there: a markdown note that takes
    /// typing.
    pub fn focused_file(
        &self, ctx: &egui::Context, tabs: &TabCache,
    ) -> Option<(lb_rs::Uuid, bool)> {
        self.embeds.values().find_map(|embed| {
            let md = tabs.get_any(&embed.session).and_then(Tab::markdown)?;
            let r = &md.edit.renderer;
            let images = r.kind == Syntax::Markdown && r.edits_text();
            md.focused(ctx).then_some((md.edit.file_id, images))
        })
    }

    /// The sessions of the tabs embeds edit and save: those shown, and
    /// those whose notes closed with their edits unsaved.
    pub fn saving(&self) -> Vec<SessionId> {
        let shown = self.embeds.values().filter(|e| e.writable && !e.own);
        shown
            .map(|e| e.session)
            .chain(self.orphans.iter().copied())
            .collect()
    }

    /// The task checkboxes the shown embeds' tabs draw, top to bottom: the
    /// mark each toggles, and a point on the box.
    #[cfg(test)]
    pub fn shown_checkboxes(&self, tabs: &TabCache) -> Vec<(Grapheme, egui::Pos2)> {
        use crate::tab::markdown_editor::TouchTarget;
        let mut boxes = vec![];
        for embed in self.embeds.values().filter(|e| e.shown && e.drawn) {
            let Some(md) = tabs.get_any(&embed.session).and_then(Tab::markdown) else { continue };
            for (rect, target) in &md.edit.renderer.touch_targets {
                if let TouchTarget::Tap(Event::ToggleTask { check }) = target {
                    // the finger's target reaches 11 left of the box and 4 right
                    boxes.push((*check, rect.center() + egui::vec2(3.5, 0.0)));
                }
            }
        }
        boxes.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        boxes
    }

    /// The shown embeds' fragments of text, on screen.
    #[cfg(test)]
    pub fn shown_words(&self, tabs: &TabCache) -> Vec<(String, Rect)> {
        let mut words = vec![];
        for embed in self.embeds.values().filter(|e| e.shown && e.drawn) {
            let Some(md) = tabs.get_any(&embed.session).and_then(Tab::markdown) else { continue };
            let r = &md.edit.renderer;
            let inside = r
                .fragments
                .iter()
                .filter(|f| embed.rect.contains_rect(f.rect));
            words.extend(inside.map(|f| (r.buffer[f.source_range].to_string(), f.rect)));
        }
        words
    }

    /// The text of each shown embed's tab.
    #[cfg(test)]
    pub fn shown_texts(&self, tabs: &TabCache) -> Vec<String> {
        let shown = self.embeds.values().filter(|e| e.shown && e.drawn);
        let md = |e: &Embed| tabs.get_any(&e.session).and_then(Tab::markdown);
        let texts = shown.filter_map(|e| Some(md(e)?.edit.renderer.buffer.current.text.clone()));
        texts.collect()
    }

    /// What Copy takes from the embed last pressed.
    #[cfg(test)]
    pub fn copied(&self, tabs: &mut TabCache) -> Option<String> {
        let md = tabs.get_any_mut(&self.active?)?.markdown()?;
        let buffer = &md.edit.renderer.buffer;
        let (a, b) = buffer.current.selection;
        (a != b).then(|| buffer[(a.min(b), a.max(b))].to_string())
    }

    /// The tabs embeds show, with the slots they last drew in.
    #[cfg(test)]
    pub fn shown(&self) -> Vec<(SessionId, Rect)> {
        let shown = self.embeds.values().filter(|e| e.shown);
        shown.map(|e| (e.session, e.rect)).collect()
    }
}

impl Embed {
    /// Reads the document on a thread; the tab builds when it lands.
    fn open(res: &TabResources, place: Place, writable: bool, own: bool) -> Self {
        let load = Some(load(res, place.dest.clone()));
        Self {
            session: SessionId::new(),
            place,
            writable,
            own,
            load,
            reload: None,
            failed: None,
            shown: false,
            rect: Rect::NOTHING,
            drawn: false,
        }
    }

    /// Builds the tab once its document has been read, and takes a newer
    /// version in place.
    fn poll(&mut self, tabs: &mut TabCache, res: &TabResources, request: &EmbedRequest) {
        if let Some(Ok(Ok(Some(doc)))) = self.reload.as_ref().map(|rx| rx.try_recv()) {
            self.reload = None;
            match tabs.get_any_mut(&self.session).map(|tab| &mut tab.content) {
                Some(ContentState::Open(content)) => {
                    if !content.reload(doc.hmac, &doc.bytes) {
                        tabs.remove(&self.session);
                        *self = Self::open(res, self.place.clone(), self.writable, self.own);
                        return;
                    }
                    self.failed = missing_heading(&self.place, content);
                }
                _ if self.failed.is_some() => self.opened(tabs, res, request, Ok(Some(doc))),
                _ => {}
            }
        }
        let Some(rx) = &self.load else { return };
        let Ok(doc) = rx.try_recv() else { return };
        self.load = None;
        self.opened(tabs, res, request, doc);
    }

    /// Puts the tab of `doc` as read in the cache, or why there is none.
    fn opened(
        &mut self, tabs: &mut TabCache, res: &TabResources, request: &EmbedRequest,
        doc: Result<Option<Doc>, TabFailure>,
    ) {
        let dest = &self.place.dest;
        let salt = egui::Id::unique(lb_rs::Uuid::new_v4());
        // what an embed edits is text: a drawing or a chat in one stays as it is
        let text = |doc: &Option<Doc>| {
            let kind = doc.as_ref().map(|doc| DocType::from_name(&doc.name));
            matches!(kind, Some(DocType::Markdown | DocType::PlainText | DocType::Code))
        };
        let read_only = !self.writable || !doc.as_ref().is_ok_and(text);
        let content = doc.and_then(|doc| TabContent::open(dest, doc, res, salt, read_only));
        let mut content = match content {
            Ok(content) => content,
            Err(failure) => {
                self.failed = Some(failure.msg());
                return;
            }
        };
        self.failed = missing_heading(&self.place, &content);
        if let TabContent::Markdown(md) = &mut content {
            let mut nested = request.path.clone();
            nested.extend(dest.backing_file());
            md.edit.renderer.tab_embeds.path = nested;
            md.edit.renderer.tab_embeds.edits = request.edits;
            md.edit.hosted_history = self.own;
        }
        // a heading of the note is the note's to save
        let tab_read_only = read_only || self.own;
        let mut tab = Tab::new(dest.clone(), ContentState::Open(content), salt, tab_read_only);
        tab.presentation = Presentation::Embedded;
        tab.reveal = Some(Reveal { fragment: self.place.fragment.clone(), flash: false });
        tabs.insert(self.session, tab);
    }
}

impl Embed {
    /// Closes its tab, or leaves it in the cache, which keeps it while its
    /// edits save.
    fn close(&self, tabs: &mut TabCache, orphans: &mut Vec<SessionId>) {
        let unsaved = |tab: &Tab| tab.last_changed > tab.last_saved;
        if !self.own && tabs.get_any(&self.session).is_some_and(unsaved) {
            orphans.push(self.session);
        } else {
            tabs.remove(&self.session);
        }
    }

    /// Shows the buffer of the note `host` in this embed of a heading of it,
    /// so the two share offsets, and says so if the heading is gone.
    fn mirror(&mut self, tabs: &mut TabCache, host: SessionId) {
        let host_md = tabs.get_any(&host).and_then(Tab::markdown);
        let Some(host_md) = host_md else { return };
        let toggled = host_md.edit.renderer.task_toggled;
        let text = &host_md.edit.renderer.buffer.current.text;
        let md = tabs.get_any(&self.session).and_then(Tab::markdown);
        let Some(md) = md else { return };
        let changed = (md.edit.renderer.buffer.current.text != *text).then(|| text.clone());
        let Some(tab) = tabs.get_any_mut(&self.session) else { return };
        let ContentState::Open(content) = &mut tab.content else { return };
        let TabContent::Markdown(md) = content else { return };
        md.edit.renderer.task_toggled = toggled;
        if let Some(text) = changed {
            let hmac = md.hmac();
            md.reload(text, hmac);
            self.failed = missing_heading(&self.place, content);
        }
    }
}

/// Hands the note `host` what its embed `session` did this frame, given its
/// text's seq and selection `before`: the caret, if it left the embed, and,
/// for an `own` heading of the note, its edits, which the note makes at the
/// same offsets and keeps in its history. An embed of another note's heading
/// takes back an edit that lost the heading.
fn hand_over(
    ctx: &egui::Context, tabs: &mut TabCache, session: SessionId, host: SessionId,
    request: &EmbedRequest, own: bool, before: (u64, (Grapheme, Grapheme)),
) {
    let (Some(host_text), Some(typed)) = (text_of(tabs, &host), text_of(tabs, &session)) else {
        return;
    };
    let host_text = (own && host_text != typed).then(|| host_text.clone());
    let Some(md) = tabs.get_any_mut(&session).and_then(Tab::markdown_mut) else { return };
    let (text_seq, selection) = before;
    let edited = md.edit.renderer.text_seq != text_seq;
    let mut handed = vec![];
    if own {
        handed = take_edits(md);
        if let Some(host_text) = host_text {
            handed.extend(take_typing(md, &request.place, host_text, selection));
        }
    } else if edited && lacks_heading(&request.place, &md.edit.renderer.buffer.current.text) {
        // the edit took the heading the embed names: it goes back
        md.edit.event.internal_events.push(Event::Undo);
    }
    let moved = edited || md.edit.renderer.buffer.current.selection != selection;
    let left = left_section(ctx, md, &request.place, moved);
    if left.is_some() {
        md.surrender_focus(ctx);
    }
    let Some(host_md) = tabs.get_any_mut(&host).and_then(Tab::markdown_mut) else { return };
    if let Some(leave) = left {
        host_md
            .edit
            .renderer
            .tab_embeds
            .left
            .set(Some((request.at, leave)));
    }
    if !handed.is_empty() || left.is_some() {
        host_md.edit.event.internal_events.extend(handed);
        ctx.request_repaint();
    }
}

fn host_md(tabs: &mut TabCache, host: Option<SessionId>) -> Option<&mut Markdown> {
    tabs.get_any_mut(&host?)?.markdown_mut()
}

fn text_of<'a>(tabs: &'a TabCache, session: &SessionId) -> Option<&'a String> {
    let md = tabs.get_any(session).and_then(Tab::markdown)?;
    Some(&md.edit.renderer.buffer.current.text)
}

/// The edits `md`, a heading of the note it's in, queued this frame: the note
/// makes them, and keeps their history.
fn take_edits(md: &mut Markdown) -> Vec<Event> {
    let queued = std::mem::take(&mut md.edit.event.internal_events);
    let (edits, kept) = queued
        .into_iter()
        .partition(|event| matches!(event, Event::ToggleTask { .. } | Event::Undo | Event::Redo));
    md.edit.event.internal_events = kept;
    edits
}

/// What was typed into `md`, a heading of the note whose text is `host`,
/// this frame, as the note's replacements of its own text. An edit that
/// leaves the note without the heading `place` names is refused: the card's
/// heading is its band, retitled in the note itself.
fn take_typing(
    md: &mut Markdown, place: &Place, host: String, selection: (Grapheme, Grapheme),
) -> Vec<Event> {
    let buffer = &mut md.edit.renderer.buffer;
    let typed = buffer.current.text.clone();
    // the card's next edits build on what it has now
    buffer.flush();
    buffer.saved(typed.clone());
    if lacks_heading(place, &typed) {
        let hmac = md.hmac();
        md.reload(host, hmac);
        // the caret stays where the refused edit found it
        let r = &mut md.edit.renderer;
        let now = r.now();
        r.buffer.queue(vec![Operation::Select(selection)], now);
        r.buffer.update();
        return vec![];
    }
    // last first, so each range is still the note's when it lands
    let ops = lb_rs::model::text::diff(&host, &typed);
    let replace = |op: Replace| Event::Replace {
        region: op.range.into(),
        text: op.text,
        advance_cursor: false,
    };
    ops.into_iter().rev().map(replace).collect()
}

/// Whether the caret in `md`, a focused embed, left it this frame, and which
/// way: Escape, or a caret that `moved` out of the heading's section an
/// embed of a heading shows.
fn left_section(
    ctx: &egui::Context, md: &mut Markdown, place: &Place, moved: bool,
) -> Option<Leave> {
    if !md.focused(ctx) {
        return None;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(Leave::Select);
    }
    let Some(Fragment::Heading(slug)) = &place.fragment else { return None };
    if !moved {
        return None;
    }
    let r = &mut md.edit.renderer;
    let arena = crate::tab::markdown_editor::Arena::new();
    let root = r.parse(&arena);
    let headings = headings_in(root);
    let infos: Vec<_> = headings.iter().map(|(_, h)| h.clone()).collect();
    let i = find_heading(&infos, slug)?;
    let heading_end = r.node_range(headings[i].0).1;
    // the section shows from its first text: blank lines under the heading are spacing
    let segs = &r.buffer.current.segs;
    let text = &r.buffer.current.text;
    let after = segs.offset_to_byte(heading_end).0;
    let blank = text[after..].len() - text[after..].trim_start().len();
    let first = segs.byte_to_char_floor(Byte(after + blank));
    let next = headings[i + 1..]
        .iter()
        .find(|(_, h)| h.level <= infos[i].level)
        .map(|(node, _)| r.node_range(node).0);
    let (a, b) = r.buffer.current.selection;
    let (start, end) = (a.min(b), a.max(b));
    if start < first {
        Some(Leave::Before)
    } else if next.is_some_and(|next| end >= next) {
        Some(Leave::After)
    } else {
        None
    }
}
