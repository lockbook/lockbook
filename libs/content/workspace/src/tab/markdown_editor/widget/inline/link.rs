use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{OpenUrl, Pos2, Sense, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt as _, RangeExt as _};

use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::resolvers::{LinkPreview, LinkState, ResolvedLink};
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::MdLabel;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format, Wrap};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;

pub enum DestinationTitle {
    Ready(String),
    Loading,
    Absent,
}

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn text_format_link(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        let parent_text_format = self.text_format(parent);
        let theme = self.ctx.get_lb_theme();
        let color = match state {
            LinkState::Normal => theme.fg().blue,
            LinkState::Warning => theme.fg().yellow,
            LinkState::Broken => theme.fg().red,
        };
        Format { color, underline: true, ..parent_text_format }
    }

    pub fn text_format_link_button(&self, parent: &AstNode<'_>) -> Format {
        Format { family: FontFamily::Icons, ..self.text_format_link(parent, LinkState::Normal) }
    }

    fn link_is_auto(&self, node: &'ast AstNode<'ast>, url: &str) -> bool {
        self.infix_range(node)
            .is_some_and(|r| &self.buffer[r] == url)
    }

    fn link_is_revealed(&self, node: &'ast AstNode<'ast>, is_auto: bool) -> bool {
        let node_range = self.node_range(node);
        let selection = &self.buffer.current.selection;
        // auto links also reveal when cursor sits at a boundary, so backspacing
        // from the right side doesn't repeatedly replace the display text
        node_range.intersects(selection, is_auto)
    }

    pub fn span_link(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let mut tmp_wrap = wrap.clone();
        let node_range = self.node_range(node);
        let url = node_link_url(node);
        let is_auto = self.link_is_auto(node, &url);

        let used_override = (node.children().next().is_none() || is_auto)
            && !self.link_is_revealed(node, is_auto)
            && !node_range.trim(&range).is_empty()
            && match self.get_link_title(&url) {
                DestinationTitle::Ready(t) => {
                    tmp_wrap.offset += self.span_override_section(
                        &tmp_wrap,
                        &t,
                        self.text_format_link(node.parent().unwrap(), LinkState::Normal),
                    );
                    true
                }
                DestinationTitle::Loading => {
                    tmp_wrap.offset += self.span_override_section(
                        &tmp_wrap,
                        "Loading...",
                        self.text_format_syntax(),
                    );
                    true
                }
                DestinationTitle::Absent => false,
            };

        if !used_override {
            tmp_wrap.offset += self.circumfix_span(node, &tmp_wrap, range);
        }

        if range.contains_inclusive(node_range.end()) && self.touch_mode {
            tmp_wrap.offset += self.span_override_section(
                &tmp_wrap,
                " ",
                self.text_format(node.parent().unwrap()),
            );
            tmp_wrap.offset += self.span_override_section(
                &tmp_wrap,
                Icon::OPEN_IN_NEW.icon,
                self.text_format_link_button(node.parent().unwrap()),
            );
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_link(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        node_link: &NodeLink, range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node);
        let is_auto = self.link_is_auto(node, &node_link.url);
        let mut response = if (node.children().next().is_none() || is_auto)
            && !self.link_is_revealed(node, is_auto)
        {
            // empty or auto link: show the fetched title in place of the URL
            let trimmed = node_range.trim(&range);
            if !trimmed.is_empty() {
                match self.get_link_title(&node_link.url) {
                    DestinationTitle::Ready(t) => self.show_override_section(
                        ui,
                        top_left,
                        wrap,
                        trimmed,
                        self.text_format_link(node.parent().unwrap(), LinkState::Normal),
                        Some(&t),
                        Sense::hover(),
                    ),
                    DestinationTitle::Loading => self.show_override_section(
                        ui,
                        top_left,
                        wrap,
                        trimmed,
                        self.text_format_syntax(),
                        Some("Loading..."),
                        Sense::hover(),
                    ),
                    DestinationTitle::Absent => {
                        // destination has no title
                        self.show_circumfix(ui, node, top_left, wrap, range)
                    }
                }
            } else {
                // has title
                self.show_circumfix(ui, node, top_left, wrap, range)
            }
        } else {
            // has children or is revealed
            self.show_circumfix(ui, node, top_left, wrap, range)
        };

        response.hovered &= self.inline_clickable(ui, node);

        if range.contains_inclusive(self.node_range(node).end()) && self.touch_mode {
            response |= self.show_override_section(
                ui,
                top_left,
                wrap,
                self.node_range(node).end().into_range(),
                self.text_format(node.parent().unwrap()),
                Some(" "),
                Sense::focusable_noninteractive(),
            );
            response |= self.show_override_section(
                ui,
                top_left,
                wrap,
                self.node_range(node).end().into_range(),
                self.text_format_link_button(node.parent().unwrap()),
                Some(Icon::OPEN_IN_NEW.icon),
                Sense::click(),
            );
        }

        if response.hovered {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            if self.link_resolver.link_state(&node_link.url) == LinkState::Warning {
                if let Some(pos) = ui.ctx().pointer_hover_pos() {
                    egui::Area::new(ui.id().with("link_warning"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(pos + egui::vec2(8.0, 16.0))
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label("Some collaborators cannot access this link target");
                            });
                        });
                }
            }
        }
        if response.clicked {
            let cmd = ui.input(|i| i.modifiers.command);
            match self.link_resolver.resolve_link(&node_link.url) {
                Some(ResolvedLink::File(id)) => {
                    ui.ctx().open_file(id, cmd);
                }
                Some(ResolvedLink::External(url)) => {
                    ui.ctx().open_url(OpenUrl { url, new_tab: cmd });
                }
                None => {
                    ui.ctx()
                        .open_url(OpenUrl { url: node_link.url.clone(), new_tab: cmd });
                }
            }
        }

        response
    }

    pub fn open_links_in_selection(&self, root: &'ast AstNode<'ast>, ctx: &egui::Context) {
        let selection = self.buffer.current.selection;

        let mut file_ids = vec![];
        let mut urls = vec![];

        for node in root.descendants() {
            let node_range = self.node_range(node);
            if !node_range.intersects(&selection, true) {
                continue;
            }
            match &node.data.borrow().value {
                NodeValue::Link(link) => match self.link_resolver.resolve_link(&link.url) {
                    Some(ResolvedLink::File(id)) => file_ids.push(id),
                    Some(ResolvedLink::External(url)) => urls.push(url),
                    None => urls.push(link.url.clone()),
                },
                NodeValue::WikiLink(wl) => {
                    if let Some(id) = self.link_resolver.resolve_wikilink(&wl.url) {
                        file_ids.push(id);
                    } else {
                        urls.push(wl.url.clone());
                    }
                }
                _ => {}
            }
        }

        let cmd = ctx.input(|i| i.modifiers.command);
        for id in file_ids {
            ctx.open_file(id, cmd);
        }
        for url in urls {
            ctx.open_url(OpenUrl { url, new_tab: cmd });
        }
    }

    pub fn get_link_title(&self, url: &str) -> DestinationTitle {
        match self.link_resolver.link_preview(url) {
            LinkPreview::Loading => DestinationTitle::Loading,
            LinkPreview::Ready(data) => match data.title {
                Some(t) => DestinationTitle::Ready(t),
                None => DestinationTitle::Absent,
            },
            LinkPreview::Unavailable => DestinationTitle::Absent,
        }
    }
}

fn node_link_url<'ast>(node: &'ast AstNode<'ast>) -> String {
    match &node.data.borrow().value {
        NodeValue::Link(link) | NodeValue::Image(link) => link.url.clone(),
        _ => String::new(),
    }
}
