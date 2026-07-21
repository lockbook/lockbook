//! Desktop markdown context menu.
//!
//! Full format surface so the toolbar can stay hidden. Dense styles live in
//! **submenus** (Format / Paragraph) so the root stays short — flyouts use the
//! floating menu’s safe-corner hover bridge.
//!
//! ```text
//! [link under cursor]
//! clipboard
//! Format ▶          inline styles
//! Paragraph ▶       headings · lists · quote · code · indent
//! Link…
//! fold · find
//! undo · redo
//! ```
//!
//! Call site: [`MdEdit::show_desktop_context_menu`] from `show.rs`.

mod icons;

use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::{Response, Ui, ViewportCommand};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::file_cache::ResolvedLink;
use crate::tab::markdown_editor::input::{Bound, Event, Location, Region};
use crate::tab::markdown_editor::widget::inline::link::LinkMenuTarget;
use crate::tab::markdown_editor::MdEdit;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::widgets::{MenuEntries, is_menu_open, show_menu};

// ── Actions ──────────────────────────────────────────────────────────────────

/// One menu choice. Side-effecting items (open URL, clipboard paste, find)
/// are applied in [`dispatch`]; the rest become [`Event`]s.
#[derive(Clone, Debug)]
pub enum Action {
    // Link under cursor
    OpenLink { new_tab: bool },
    CopyLink,
    EditLink,
    RefreshLink,

    // Clipboard / selection
    Cut,
    Copy,
    Paste,
    SelectAll,

    // History
    Undo,
    Redo,

    // Inline styles (selection / word under caret via ToggleStyle)
    Bold,
    Italic,
    Underline,
    Strikethrough,
    InlineCode,
    Highlight,
    Spoiler,
    Subscript,
    Superscript,

    // Blocks / structure
    Heading { level: u8 },
    BulletList,
    NumberedList,
    TaskList,
    Quote,
    CodeBlock,
    Indent,
    Outdent,

    // Insert
    InsertLink,

    // Doc chrome
    ToggleFold,
    Find,
}

// ── Snapshot (build-time inputs) ─────────────────────────────────────────────

/// Immutable inputs for building the menu this frame.
struct Snapshot {
    readonly: bool,
    has_selection: bool,
    link: Option<LinkSnap>,
}

struct LinkSnap {
    /// Resolves to an in-app file (wiki or path) → offer “Open in new tab”.
    is_file: bool,
    is_image: bool,
}

impl Snapshot {
    fn from_editor(editor: &MdEdit, link: Option<&LinkMenuTarget>) -> Self {
        let has_selection = !editor.renderer.buffer.current.selection.is_empty();
        let link = link.map(|target| LinkSnap {
            is_file: link_is_file(editor, target),
            is_image: target.is_image,
        });
        Self { readonly: editor.renderer.readonly, has_selection, link }
    }
}

fn link_is_file(editor: &MdEdit, t: &LinkMenuTarget) -> bool {
    if t.is_wikilink {
        return editor.renderer.resolve_wikilink(&t.url).is_some();
    }
    matches!(editor.renderer.resolve_link(&t.url), Some(ResolvedLink::File(_)))
}

// ── Build ────────────────────────────────────────────────────────────────────

fn populate(m: &mut MenuEntries<Action>, snap: &Snapshot) {
    // ── Link under cursor ────────────────────────────────────────────────
    if let Some(link) = &snap.link {
        let open_label = if link.is_image { "Open image" } else { "Open link" };
        m.item_icon(icons::ARROW_SQUARE_OUT, open_label, Action::OpenLink { new_tab: false });
        if link.is_file {
            m.item_icon(
                icons::APP_WINDOW,
                "Open in new tab",
                Action::OpenLink { new_tab: true },
            );
        }
        m.item_icon(
            icons::LINK,
            if link.is_image { "Copy URL" } else { "Copy link" },
            Action::CopyLink,
        );
        if !snap.readonly {
            m.item_icon(
                icons::PENCIL_SIMPLE,
                if link.is_image { "Edit image" } else { "Edit link" },
                Action::EditLink,
            );
        }
        if link.is_image {
            m.item_icon(icons::ARROWS_CLOCKWISE, "Refresh preview", Action::RefreshLink);
        }
        m.separator();
    }

    // ── Clipboard ────────────────────────────────────────────────────────
    if !snap.readonly && snap.has_selection {
        m.item_icon(icons::SCISSORS, "Cut", Action::Cut);
    }
    if snap.has_selection {
        m.item_icon(icons::COPY, "Copy", Action::Copy);
    }
    if !snap.readonly {
        m.item_icon(icons::CLIPBOARD, "Paste", Action::Paste);
    }
    m.item_icon(icons::SELECTION_ALL, "Select all", Action::SelectAll);

    if snap.readonly {
        m.separator();
        m.item_icon(icons::MAGNIFYING_GLASS, "Find…", Action::Find);
        return;
    }

    // ── Format / Paragraph (submenus) ────────────────────────────────────
    m.separator();
    m.submenu_icon(icons::TEXT_AA, "Format", |sub| {
        sub.item_icon(icons::TEXT_B, "Bold", Action::Bold);
        sub.item_icon(icons::TEXT_ITALIC, "Italic", Action::Italic);
        sub.item_icon(icons::TEXT_UNDERLINE, "Underline", Action::Underline);
        sub.item_icon(icons::TEXT_STRIKETHROUGH, "Strikethrough", Action::Strikethrough);
        sub.item_icon(icons::CODE, "Code", Action::InlineCode);
        sub.item_icon(icons::HIGHLIGHTER_CIRCLE, "Highlight", Action::Highlight);
        sub.item_icon(icons::EYE_SLASH, "Spoiler", Action::Spoiler);
        sub.separator();
        sub.item_icon(icons::TEXT_SUBSCRIPT, "Subscript", Action::Subscript);
        sub.item_icon(icons::TEXT_SUPERSCRIPT, "Superscript", Action::Superscript);
    });
    m.submenu_icon(icons::PARAGRAPH, "Paragraph", |sub| {
        sub.item_icon(icons::TEXT_H_ONE, "Heading 1", Action::Heading { level: 1 });
        sub.item_icon(icons::TEXT_H_TWO, "Heading 2", Action::Heading { level: 2 });
        sub.item_icon(icons::TEXT_H_THREE, "Heading 3", Action::Heading { level: 3 });
        sub.item_icon(icons::TEXT_H_FOUR, "Heading 4", Action::Heading { level: 4 });
        sub.item_icon(icons::TEXT_H_FIVE, "Heading 5", Action::Heading { level: 5 });
        sub.item_icon(icons::TEXT_H_SIX, "Heading 6", Action::Heading { level: 6 });
        sub.separator();
        sub.item_icon(icons::LIST_BULLETS, "Bullet list", Action::BulletList);
        sub.item_icon(icons::LIST_NUMBERS, "Numbered list", Action::NumberedList);
        sub.item_icon(icons::LIST_CHECKS, "Task list", Action::TaskList);
        sub.separator();
        sub.item_icon(icons::QUOTES, "Quote", Action::Quote);
        sub.item_icon(icons::CODE_BLOCK, "Code block", Action::CodeBlock);
        sub.separator();
        sub.item_icon(icons::TEXT_INDENT, "Indent", Action::Indent);
        sub.item_icon(icons::TEXT_OUTDENT, "Outdent", Action::Outdent);
    });

    // ── Insert + doc ─────────────────────────────────────────────────────
    m.separator();
    m.item_icon(icons::LINK, "Link…", Action::InsertLink);
    m.item_icon(icons::CARET_DOUBLE_UP, "Fold section", Action::ToggleFold);
    m.item_icon(icons::MAGNIFYING_GLASS, "Find…", Action::Find);

    // ── History ──────────────────────────────────────────────────────────
    m.separator();
    m.item_icon(icons::ARROW_U_UP_LEFT, "Undo", Action::Undo);
    m.item_icon(icons::ARROW_U_UP_RIGHT, "Redo", Action::Redo);
}

// ── Style → Event helpers ────────────────────────────────────────────────────
//
// Same *payload* as the toolbar (`Region::Selection` + `ToggleStyle`), and the
// same *delivery*: `ctx.push_markdown_event` → drained into `internal_events`
// at the start of the next `Editor::show` → `handle_input` → `calc_operations`
// with a fresh parse. Applying styles mid-`pre_render` via `calc_operations`
// races pointer selection and skips the buffer/OT path the toolbar relies on.

fn toggle_style(style: NodeValue) -> Event {
    Event::ToggleStyle { region: Region::Selection, style }
}

fn list(list_type: ListType, is_task_list: bool) -> Event {
    toggle_style(NodeValue::List(NodeList {
        list_type,
        is_task_list,
        ..Default::default()
    }))
}

fn action_to_event(action: &Action) -> Option<Event> {
    Some(match action {
        Action::Cut => Event::Cut,
        Action::Copy => Event::Copy,
        Action::SelectAll => {
            Event::Select { region: Region::Bound { bound: Bound::Doc, backwards: true } }
        }
        Action::Undo => Event::Undo,
        Action::Redo => Event::Redo,
        Action::Bold => toggle_style(NodeValue::Strong),
        Action::Italic => toggle_style(NodeValue::Emph),
        Action::Underline => toggle_style(NodeValue::Underline),
        Action::Strikethrough => toggle_style(NodeValue::Strikethrough),
        Action::InlineCode => toggle_style(NodeValue::Code(Default::default())),
        Action::Highlight => toggle_style(NodeValue::Highlight),
        Action::Spoiler => toggle_style(NodeValue::SpoileredText),
        Action::Subscript => toggle_style(NodeValue::Subscript),
        Action::Superscript => toggle_style(NodeValue::Superscript),
        Action::Heading { level } => toggle_style(NodeValue::Heading(NodeHeading {
            level: *level,
            ..Default::default()
        })),
        Action::BulletList => list(ListType::Bullet, false),
        Action::NumberedList => list(ListType::Ordered, false),
        Action::TaskList => list(ListType::Bullet, true),
        Action::Quote => toggle_style(NodeValue::BlockQuote),
        Action::CodeBlock => toggle_style(NodeValue::CodeBlock(Default::default())),
        Action::Indent => Event::Indent { deindent: false },
        Action::Outdent => Event::Indent { deindent: true },
        Action::InsertLink => toggle_style(NodeValue::Link(Default::default())),
        Action::ToggleFold => Event::ToggleFold,
        // Side effects handled in dispatch
        Action::Paste
        | Action::OpenLink { .. }
        | Action::CopyLink
        | Action::EditLink
        | Action::RefreshLink
        | Action::Find => return None,
    })
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

fn open_link(editor: &MdEdit, t: &LinkMenuTarget, new_tab: bool, ui: &Ui) {
    let ctx = ui.ctx();
    if t.is_wikilink {
        if let Some(file_id) = editor.renderer.resolve_wikilink(&t.url) {
            ctx.open_file(file_id, new_tab);
        }
        return;
    }
    match editor.renderer.resolve_link(&t.url) {
        Some(ResolvedLink::File(file_id)) => ctx.open_file(file_id, new_tab),
        Some(ResolvedLink::External(url)) => {
            ctx.open_url(egui::OpenUrl { url, new_tab: true });
        }
        None => {
            ctx.open_url(egui::OpenUrl { url: t.url.clone(), new_tab: true });
        }
    }
}

/// Apply a chosen action.
///
/// Editor mutations (styles, cut/copy, undo, fold, …) go through
/// [`ExtendedInput::push_markdown_event`] — identical to the toolbar — so the
/// buffer OT's selection and `handle_input` runs a consistent parse.
/// Side effects that aren't buffer ops (open URL, system paste, find UI) run
/// immediately.
fn dispatch(editor: &mut MdEdit, action: Action, link: Option<&LinkMenuTarget>, ui: &Ui) {
    match action {
        Action::OpenLink { new_tab } => {
            if let Some(t) = link {
                open_link(editor, t, new_tab, ui);
            }
        }
        Action::CopyLink => {
            if let Some(t) = link {
                ui.ctx().copy_text(t.url.clone());
            }
        }
        Action::RefreshLink => {
            if let Some(t) = link {
                editor.renderer.refresh_link_meta(&t.url);
            }
        }
        Action::EditLink => {
            if let Some(t) = link {
                if t.force_reveal {
                    editor.renderer.entered_atom = Some(t.node_range);
                }
                ui.ctx().push_markdown_event(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(t.select.start()),
                        end: Location::Grapheme(t.select.end()),
                    },
                });
            }
        }
        Action::Paste => {
            ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
        }
        Action::Find => {
            editor.open_find_requested = true;
        }
        other => {
            if let Some(ev) = action_to_event(&other) {
                ui.ctx().push_markdown_event(ev);
            }
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

impl MdEdit {
    /// Desktop right-click menu over the editor surface. Captures the link
    /// under the click, builds the menu, and applies the choice.
    ///
    /// Returns `true` when the editor must **ignore pointer selection** this
    /// frame (menu open, or a leaf was chosen). A primary click on a style item
    /// must not also place the caret under the menu.
    ///
    /// Style / edit actions use [`ExtendedInput::push_markdown_event`] (toolbar
    /// path), not in-frame `calc_operations`.
    pub(crate) fn show_desktop_context_menu<'ast>(
        &mut self, response: &Response, root: &'ast AstNode<'ast>, ui: &Ui,
    ) -> bool {
        if response.secondary_clicked() {
            self.context_menu_link = response
                .interact_pointer_pos()
                .and_then(|pos| self.link_target_at_pos(root, pos));
        }
        let link_target = self.context_menu_link.clone();
        let snap = Snapshot::from_editor(self, link_target.as_ref());

        let choice = show_menu(response, |m| populate(m, &snap));
        let chose = choice.is_some();

        if let Some(action) = choice {
            dispatch(self, action, link_target.as_ref(), ui);
            // Toolbar events land next frame via drain_workspace_events; repaint
            // so the edit isn't stuck until the next pointer move.
            ui.ctx().request_repaint();
        }

        // Still open (hovering root/flyout) or just acted → block editor click.
        chose || is_menu_open(ui.ctx())
    }
}
