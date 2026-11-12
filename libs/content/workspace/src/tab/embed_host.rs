//! The tabs notes show in their embed slots. A note lays its slots out at the
//! heights measured here and asks for their tabs as it draws
//! ([`EmbedRequest`]); the workspace keeps those tabs in its tab cache, like
//! any session off the tab strip, and draws them into their slots once the
//! note is done. Each frame, before a note draws, its embeds are measured at
//! the widths it last asked for, deepest first, so it lays out at their
//! heights; what changes them while they draw shows the frame after.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use egui::{Rect, UiBuilder};

use crate::tab::markdown_editor::widget::inline::tab_embed::{
    EmbedKey, EmbedRequest, EmbedState, load, missing_heading,
};
use crate::tab::{
    ContentState, Doc, ExtendedOutput as _, Place, Presentation, Reveal, SessionId, Tab,
    TabContent, TabFailure, TabResources,
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
    /// Documents written, so the tabs showing them reload.
    #[cfg(not(target_family = "wasm"))]
    writes: Option<lb_rs::service::events::Receiver<lb_rs::service::events::Event>>,
}

struct Embed {
    /// Where its tab is in the cache.
    session: SessionId,
    place: Place,
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
        self.embeds.retain(|(host, _), embed| {
            let keep = kept.contains(host);
            if !keep {
                tabs.remove(&embed.session);
            }
            keep
        });
        self.asked.retain(|host, _| kept.contains(host));
        for embed in self.embeds.values() {
            tabs.promote(&embed.session);
        }
        #[cfg(not(target_family = "wasm"))]
        self.reload_written(res);
        self.copy(ctx, tabs);
        for embed in self.embeds.values_mut() {
            embed.drawn = false;
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn reload_written(&mut self, res: &TabResources) {
        use lb_rs::service::events::Event;
        let writes = self.writes.get_or_insert_with(|| res.core.subscribe());
        while let Ok(event) = writes.try_recv() {
            let Event::DocumentWritten(id, _) = event else { continue };
            for embed in self.embeds.values_mut() {
                if embed.place.dest.backing_file() == Some(id) {
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
    /// tabs have targets of their own, for `host`'s touches.
    pub fn draw(
        &mut self, ui: &mut egui::Ui, host: Host, requests: Vec<EmbedRequest>, tabs: &mut TabCache,
        res: &TabResources,
    ) -> Vec<Rect> {
        let asked: HashSet<&EmbedKey> = requests.iter().map(|r| &r.key).collect();
        self.embeds.retain(|(h, key), embed| {
            let keep = *h != host || asked.contains(key);
            if !keep {
                tabs.remove(&embed.session);
            }
            keep
        });
        let mut targets = vec![];
        // a tab draws once a frame: a lifted embed's last ask, under the pointer, wins
        let last = |i: usize, r: &EmbedRequest| !requests[i + 1..].iter().any(|n| n.key == r.key);
        let drawn = requests.iter().enumerate().filter(|(i, r)| last(*i, r));
        for (_, request) in drawn {
            let key = (host, request.key.clone());
            let embed = self
                .embeds
                .entry(key.clone())
                .or_insert_with(|| Embed::open(res, request.place.clone()));
            if embed.place != request.place {
                tabs.remove(&embed.session);
                *embed = Embed::open(res, request.place.clone());
            }
            embed.poll(tabs, res, request);
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
            let builder = UiBuilder::new()
                .max_rect(request.rect)
                .layer_id(request.layer)
                .scope_id(tab.salt);
            let child = &mut ui.new_child(builder);
            child.set_clip_rect(request.clip);
            if !shown {
                child.set_invisible();
            }
            let resp = match res.watched {
                true => res.guard.show(tab, |tab| tab.show(child)),
                false => tab.show(child),
            };
            if let Some(id) = resp.open_file {
                ui.ctx().open_file(id, false);
            }
            let settled = !shown && tab.settled();
            let (inner, inner_host) = (tab.embed_requests(), tab.embed_host());
            let inner = match inner_host {
                Some(inner_host) => self.draw(ui, inner_host, inner, tabs, res),
                None => vec![],
            };
            let Some(tab) = tabs.get_mut(&session) else { continue };
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
        self.asked.insert(host, requests);
        targets
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
    fn open(res: &TabResources, place: Place) -> Self {
        let load = Some(load(res, place.dest.clone()));
        Self {
            session: SessionId::new(),
            place,
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
                        *self = Self::open(res, self.place.clone());
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
        let content = doc.and_then(|doc| TabContent::open(dest, doc, res, salt, true));
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
        }
        let mut tab = Tab::new(dest.clone(), ContentState::Open(content), salt, true);
        tab.presentation = Presentation::Embedded;
        tab.reveal = Some(Reveal { fragment: self.place.fragment.clone(), flash: false });
        tabs.insert(self.session, tab);
    }
}
