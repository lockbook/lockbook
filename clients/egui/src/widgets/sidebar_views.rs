//! Sidebar view panes beyond the file tree: Recents and Shared with me.
//!
//! Recents mirrors Apple `RecentsView`: documents sorted by `last_modified`,
//! bucketed into Today / Yesterday / Previous 7 Days / …, click opens the doc.

use std::collections::HashSet;

use chrono::{Datelike, Local, TimeZone};
use egui::{FontId, Id, Rect, ScrollArea, Sense, Ui, pos2, vec2};
use lb::Uuid;
use lb::model::file::ShareMode;
use lb::subscribers::status::Status;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::show::{DocType, ElapsedHumanString};
use workspace_rs::widgets::{GlyphonLabel, TextOverflow, tip_text, tip_ui_rich};
use workspace_rs::GlyphonRendererCallback;

use crate::theme::{icons, tokens::Tokens};

/// Match file-tree name metrics so emoji baseline matches rename/tree.
const NAME_FONT: f32 = 14.0;
const NAME_LINE_H: f32 = 20.0;
const CRUMB_FONT: f32 = 12.0;
const CRUMB_LINE_H: f32 = 16.0;

/// Side air matching file-tree `SCROLL_INSET` — hover fill sits off the panel edge.
const SIDE_INSET: f32 = 5.0;
/// Horizontal content pad inside the inset row (matches tree `INDENT_BASE`).
const ROW_PAD_X: f32 = 12.0;
/// Vertical inset above name and below breadcrumb.
const ROW_PAD_Y: f32 = 10.0;
/// Gap between name line and breadcrumb.
const NAME_CRUMB_GAP: f32 = 2.0;
/// Icon column width (glyph + trailing gap before text).
const ICON_COL: f32 = 28.0;
/// Gap between name and trailing meta (relative time).
const NAME_META_GAP: f32 = 10.0;
/// Gap between name and trailing push-pin (Apple pin-after-name).
const NAME_PIN_GAP: f32 = 6.0;
/// Quiet pin glyph size (matches tree meta marks; not a loud badge).
const PIN_ICON_SIZE: f32 = 12.0;
/// Don't paint the timestamp if it would leave the name narrower than this.
const MIN_NAME_W: f32 = 56.0;

/// Collaborator chip (Apple: capsule + person.fill + username).
const COLLAB_PAD_X: f32 = 8.0;
const COLLAB_PAD_Y: f32 = 3.0;
const COLLAB_ICON_GAP: f32 = 4.0;
const COLLAB_ABOVE: f32 = 6.0; // VStack spacing after breadcrumb
const COLLAB_FONT: f32 = 11.0;
const COLLAB_ICON_SIZE: f32 = 11.0;

/// Which primary sidebar body is showing (Apple `SidebarTab`; Zed-style
/// toolbar toggles, not a hide-all control).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SidebarPane {
    #[default]
    Files,
    Recents,
    Shared,
}

impl SidebarPane {
    pub const ALL: [SidebarPane; 3] = [Self::Files, Self::Recents, Self::Shared];

    pub fn title(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Recents => "Recents",
            Self::Shared => "Shared",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Files => icons::FOLDER,
            Self::Recents => icons::CLOCK,
            Self::Shared => icons::USERS,
        }
    }
}

/// Escape from a sidebar pane (shell opens files, etc.).
#[derive(Debug, Clone)]
pub enum Op {
    Open { id: Uuid, new_tab: bool },
    /// Accept a pending share by creating a link under a chosen folder.
    AcceptShare { id: Uuid, name: String },
    /// Decline a pending share permanently (lose access until reshared).
    RejectShare { id: Uuid },
    /// Context-menu actions (recents / shared) — shell-handled paths.
    Rename { id: Uuid },
    Share { id: Uuid },
    CopyLink { id: Uuid },
    TogglePin { id: Uuid },
    Duplicate { id: Uuid },
    Export { id: Uuid },
    Move { id: Uuid },
    Cut { id: Uuid },
    Copy { id: Uuid },
    /// Paste clipboard into the parent folder of `id`.
    PasteIntoParent { id: Uuid },
    Delete { id: Uuid },
}

/// Recents pane — all documents in `files`, newest first, sectioned by age.
/// `me` is the signed-in username; when set, rows last-touched by someone else
/// show a collaborator chip (Apple recents). `pinned` drives the quiet push-pin
/// after the name (same mark as the file tree). `has_clip` shows Paste when the
/// tree clipboard is non-empty. Returns `Op::Open` on row click.
pub fn show_recents(
    ui: &mut Ui,
    t: &Tokens,
    files: &impl FilesExt,
    me: Option<&str>,
    pinned: &HashSet<Uuid>,
    has_clip: bool,
    status: Option<&Status>,
) -> Option<Op> {
    let mut docs: Vec<&lb::model::file::File> =
        files.iter_files().filter(|f| f.is_document()).collect();
    docs.sort_by(|a, b| b.last_modified.cmp(&a.last_modified).then(a.id.cmp(&b.id)));

    let mut open: Option<Op> = None;
    crate::widgets::scroll_overlay::with_overlay_scroll(
        ui,
        Id::new("sidebar_recents_overlay_scroll"),
        |ui| {
            let out = ScrollArea::vertical()
                .id_salt("sidebar_recents")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Rows are packed like the file tree — no inter-row gap.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(4.0);
                    pane_title(ui, t, "Recents");

                    if docs.is_empty() {
                        empty_state(
                            ui,
                            t,
                            "No documents yet",
                            "Documents you edit will appear here.",
                        );
                        return;
                    }

                    let sections = sectioned_docs(&docs);
                    for (title, rows) in sections {
                        section_header(ui, t, &title);
                        for file in rows {
                            let when = file.last_modified.elapsed_human_string();
                            let crumb = parent_breadcrumb(files, file.id);
                            let dt = DocType::from_name(&file.name);
                            let label = dt.display_name(&file.name);
                            let collab = collaborator_label(me, &file.last_modified_by);
                            // Organized share *roots* only (parent is ours), not
                            // nested files under a shared folder.
                            let is_organized_share = me.is_some_and(|u| {
                                crate::widgets::file_tree::is_organized_share(files, file, u)
                            });
                            if let Some(op) = recent_row(
                                ui,
                                t,
                                files,
                                file,
                                label,
                                &when,
                                &crumb,
                                collab,
                                pinned.contains(&file.id),
                                has_clip,
                                is_organized_share,
                                me,
                                status,
                            ) {
                                open = Some(op);
                            }
                        }
                    }
                    ui.add_space(20.0);
                });
            ((), out.state.offset.y)
        },
    );
    open
}

/// Username to show on the collaborator chip, or `None` if we / unknown.
fn collaborator_label<'a>(me: Option<&str>, last_modified_by: &'a str) -> Option<&'a str> {
    if last_modified_by.is_empty() {
        return None;
    }
    match me {
        Some(u) if last_modified_by != u => Some(last_modified_by),
        _ => None,
    }
}

/// Local UI state for Shared with me (expand + reject confirm).
#[derive(Default, Clone)]
pub struct SharedUi {
    expanded: std::collections::HashSet<Uuid>,
    /// Pending reject confirmation `(id, name)`.
    pub confirm_reject: Option<(Uuid, String)>,
}

/// Shared-with-me pane — pending share roots from `FileCache`, grouped by
/// sharer (Apple `SharedWithMeView`). Folder roots expand; docs open; root
/// rows expose accept / reject.
pub fn show_shared(
    ui: &mut Ui,
    t: &Tokens,
    files: &workspace_rs::file_cache::FileCache,
    me: &str,
    state: &mut SharedUi,
) -> Option<Op> {
    let mut op: Option<Op> = None;

    // Decline a pending share for good (not “remove from files” — that is
    // FOLDER_MINUS on organized links in the tree).
    if let Some((id, name)) = state.confirm_reject.clone() {
        use crate::widgets::modals::{
            ConfirmOutcome, ConfirmSheet, show_confirm_sheet, show_modal_dim,
        };
        // Same scrim as Share / Move / Delete.
        if show_modal_dim(
            ui.ctx(),
            Id::new("lb_modal_dim_decline"),
            egui::LayerId::new(egui::Order::Foreground, Id::new("lb_decline_share_confirm")),
        ) {
            state.confirm_reject = None;
        }
        match show_confirm_sheet(
            ui.ctx(),
            t,
            &ConfirmSheet {
                area_id: Id::new("lb_decline_share_confirm"),
                focus_id: None,
                action: "Decline",
                subject: &name,
                body: "You’ll lose access until it’s shared with you again.",
                // Short primary — name is already the header subject.
                primary: "Decline",
                danger: true,
                width: 340.0,
            },
        ) {
            ConfirmOutcome::Confirm => {
                op = Some(Op::RejectShare { id });
                state.confirm_reject = None;
            }
            ConfirmOutcome::Closed => {
                state.confirm_reject = None;
            }
            ConfirmOutcome::Open => {}
        }
    }

    crate::widgets::scroll_overlay::with_overlay_scroll(
        ui,
        Id::new("sidebar_shared_overlay_scroll"),
        |ui| {
            let out = ScrollArea::vertical()
                .id_salt("sidebar_shared")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Packed rows — no egui inter-row gap (matches file tree).
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(4.0);
                    pane_title(ui, t, "Shared with me");

                    let groups = pending_shares_by_user(files, me);
                    if groups.is_empty() {
                        empty_state(
                            ui,
                            t,
                            "Nothing shared yet",
                            "Files shared with you will appear here.",
                        );
                    } else {
                        for (user, roots) in groups {
                            sharer_section_header(ui, t, &user, roots.len());
                            for root in roots {
                                let by = share_from(root, me).unwrap_or(user.as_str());
                                if let Some(o) = shared_subtree(
                                    ui, t, files, root, 0, true, state, me, by,
                                ) {
                                    op = Some(o);
                                }
                            }
                        }
                    }
                    ui.add_space(20.0);
                });
            ((), out.state.offset.y)
        },
    );
    op
}

/// Empty Shared pane for demo / signed-out (no live cache).
pub fn show_shared_empty(ui: &mut Ui, t: &Tokens) {
    crate::widgets::scroll_overlay::with_overlay_scroll(
        ui,
        Id::new("sidebar_shared_overlay_scroll_demo"),
        |ui| {
            let out = ScrollArea::vertical()
                .id_salt("sidebar_shared_demo")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    pane_title(ui, t, "Shared with me");
                    empty_state(
                        ui,
                        t,
                        "Nothing shared yet",
                        "Files shared with you will appear here.",
                    );
                    ui.add_space(20.0);
                });
            ((), out.state.offset.y)
        },
    );
}

/// Pending share roots grouped by `shared_by` (Apple `pendingSharesByUsername`).
fn pending_shares_by_user<'a>(
    files: &'a workspace_rs::file_cache::FileCache, me: &str,
) -> Vec<(String, Vec<&'a lb::model::file::File>)> {
    let mut map: std::collections::BTreeMap<String, Vec<&'a lb::model::file::File>> =
        std::collections::BTreeMap::new();
    for root in &files.shared_roots {
        let Some(by) = share_from(root, me) else {
            continue;
        };
        map.entry(by.to_owned()).or_default().push(root);
    }
    for roots in map.values_mut() {
        roots.sort_by(|a, b| a.name.cmp(&b.name));
    }
    map.into_iter().collect()
}

/// Who shared this file with `me` (Apple `shareFrom(to:)`).
fn share_from<'a>(file: &'a lb::model::file::File, me: &str) -> Option<&'a str> {
    file.shares
        .iter()
        .find(|s| s.shared_with == me)
        .map(|s| s.shared_by.as_str())
}

/// Recursive pending-share row + children when expanded.
#[allow(clippy::too_many_arguments)]
fn shared_subtree(
    ui: &mut Ui,
    t: &Tokens,
    files: &workspace_rs::file_cache::FileCache,
    file: &lb::model::file::File,
    depth: usize,
    is_root_share: bool,
    state: &mut SharedUi,
    me: &str,
    shared_by: &str,
) -> Option<Op> {
    let children = files.children(file.id);
    let has_kids = !children.is_empty();
    let expanded = state.expanded.contains(&file.id);

    let mut op = None;
    let row = shared_row(
        ui,
        t,
        files,
        file,
        depth,
        is_root_share,
        has_kids,
        expanded,
        me,
        shared_by,
    );

    if row.toggle_expand && has_kids {
        if expanded {
            state.expanded.remove(&file.id);
        } else {
            state.expanded.insert(file.id);
        }
    }
    // Decline goes through confirm (same as the trailing ghost button).
    if row.confirm_reject {
        state.confirm_reject = Some((file.id, file.name.clone()));
    }
    if let Some(o) = row.op {
        op = Some(o);
    }

    if expanded {
        for child in children {
            if let Some(o) =
                shared_subtree(ui, t, files, child, depth + 1, false, state, me, shared_by)
            {
                op = Some(o);
            }
        }
    }
    op
}

struct SharedRowResponse {
    /// Escaping op (open / accept / menu choice).
    op: Option<Op>,
    toggle_expand: bool,
    /// Root-share decline → confirm dialog.
    confirm_reject: bool,
}

const SHARED_ACTION_BTN: f32 = 26.0;

#[allow(clippy::too_many_arguments)]
fn shared_row(
    ui: &mut Ui,
    t: &Tokens,
    files: &impl FilesExt,
    file: &lb::model::file::File,
    _depth: usize,
    is_root_share: bool,
    has_kids: bool,
    expanded: bool,
    _me: &str,
    shared_by: &str,
) -> SharedRowResponse {
    // Same vertical metrics as recents: pad + name + secondary line + pad.
    // Roots: name + access mode. Nested children: single-line (no mode).
    let mode_label = if is_root_share {
        share_mode_label(file)
    } else {
        None
    };
    let stack_h = if mode_label.is_some() {
        NAME_LINE_H + NAME_CRUMB_GAP + CRUMB_LINE_H
    } else {
        NAME_LINE_H
    };
    let h = ROW_PAD_Y * 2.0 + stack_h;

    let rect = allocate_inset_row(ui, h);
    let resp = ui.interact(rect, Id::new(("shared_row", file.id)), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    paint_row_hover(ui, t, rect, hover);

    let is_folder = file.is_folder();
    let icon = if is_folder {
        if expanded {
            icons::FOLDER_OPEN
        } else {
            icons::FOLDER
        }
    } else {
        icons::for_doc_type(DocType::from_name(&file.name))
    };
    // Recents uses fg for doc icons; folders keep accent (tree convention).
    let icon_ink = if is_folder { t.accent() } else { t.fg() };
    let icon_g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(18.0), icon_ink);

    // Trailing organize actions — ghost; vertically centered on the row.
    let mut accept = false;
    let mut reject = false;
    let mut right = rect.right() - ROW_PAD_X;
    let cy = rect.center().y;
    if is_root_share {
        let btn = SHARED_ACTION_BTN;
        let reject_rect =
            Rect::from_min_size(pos2(right - btn, cy - btn / 2.0), vec2(btn, btn));
        right -= btn + 2.0;
        let accept_rect =
            Rect::from_min_size(pos2(right - btn, cy - btn / 2.0), vec2(btn, btn));
        right -= btn + 6.0;

        let a = ui.interact(accept_rect, Id::new(("share_accept", file.id)), Sense::click());
        let r = ui.interact(reject_rect, Id::new(("share_reject", file.id)), Sense::click());
        tip_text(ui.ctx(), &a, "Add to files");
        tip_text(ui.ctx(), &r, "Decline");
        // Plus = organize into tree; X = forsake access (not folder-minus —
        // that pairs with plus for remove-from-files on organized links).
        paint_share_ghost(ui, t, accept_rect, icons::FOLDER_PLUS, a.hovered());
        paint_share_ghost(ui, t, reject_rect, icons::X_CIRCLE, r.hovered());
        accept = a.clicked();
        reject = r.clicked();
    }

    // Recents-aligned leading edge: pad + icon column, no chevron/depth indent.
    let icon_x = rect.left() + ROW_PAD_X;
    ui.painter()
        .galley(pos2(icon_x, cy - icon_g.size().y / 2.0), icon_g, icon_ink);
    let text_left = icon_x + ICON_COL;
    let text_band = (right - text_left).max(0.0);

    let name_y = rect.top() + ROW_PAD_Y;
    let label = DocType::from_name(&file.name).display_name(&file.name);
    let name_rect =
        Rect::from_min_size(pos2(text_left, name_y), vec2(text_band, NAME_LINE_H));
    let text_clip = Rect::from_min_max(
        pos2(text_left, rect.top()),
        pos2(right, rect.bottom()),
    )
    .intersect(ui.clip_rect());
    let (_truncated, _) = paint_glyphon(
        ui,
        label,
        t.fg(),
        NAME_FONT,
        NAME_LINE_H,
        name_rect,
        text_clip,
    );

    // Second line: access mode for roots (same slot as recents breadcrumb).
    if let Some(mode) = mode_label {
        let mode_rect = Rect::from_min_size(
            pos2(text_left, name_y + NAME_LINE_H + NAME_CRUMB_GAP),
            vec2(text_band, CRUMB_LINE_H),
        );
        let _ = paint_glyphon(
            ui,
            mode,
            t.text_muted(),
            CRUMB_FONT,
            CRUMB_LINE_H,
            mode_rect,
            text_clip,
        );
    }

    // Rich tip — sharer/access first (this pane’s job), then place.
    {
        let path = files.path(file.id);
        let mut meta: Vec<String> = Vec::new();
        if !shared_by.is_empty() {
            meta.push(format!("Shared by {shared_by}"));
        }
        if let Some(mode) = mode_label {
            // Align with share-sheet phrasing in the tip.
            let mode_tip = match mode {
                "Read & write" => "Can edit",
                "Read only" => "Can view",
                other => other,
            };
            meta.push(mode_tip.into());
        }
        show_pane_tip(ui.ctx(), &resp, t, label, Some(path.as_str()), &meta);
    }

    let id = file.id;
    // Context menu — Shared-focused (not full Files/Recents arrange).
    // Open | Expand | Add to files / Decline (roots) | Link/export for docs.
    let menu_choice = crate::widgets::context_menu::show(&resp, t, |m| {
        if !is_folder {
            m.item(
                icons::ARROW_SQUARE_OUT,
                "Open",
                SharedMenu::Op(Op::Open {
                    id,
                    new_tab: false,
                }),
            );
            m.item(
                icons::APP_WINDOW,
                "Open in new tab",
                SharedMenu::Op(Op::Open { id, new_tab: true }),
            );
        }
        if is_folder && has_kids {
            if expanded {
                m.item(icons::CARET_RIGHT, "Collapse", SharedMenu::ToggleExpand);
            } else {
                m.item(icons::CARET_DOWN, "Expand", SharedMenu::ToggleExpand);
            }
        }
        if is_root_share {
            m.separator();
            m.item(
                icons::FOLDER_PLUS,
                "Add to files",
                SharedMenu::Op(Op::AcceptShare {
                    id,
                    name: file.name.clone(),
                }),
            );
            m.item_danger(icons::X_CIRCLE, "Decline", SharedMenu::Decline);
        }
        if !is_folder {
            m.separator();
            m.item(icons::LINK, "Copy link", SharedMenu::Op(Op::CopyLink { id }));
            m.item(icons::EXPORT, "Export", SharedMenu::Op(Op::Export { id }));
        }
    });

    if let Some(choice) = menu_choice {
        return match choice {
            SharedMenu::Op(op) => SharedRowResponse {
                op: Some(op),
                toggle_expand: false,
                confirm_reject: false,
            },
            SharedMenu::ToggleExpand => SharedRowResponse {
                op: None,
                toggle_expand: true,
                confirm_reject: false,
            },
            SharedMenu::Decline => SharedRowResponse {
                op: None,
                toggle_expand: false,
                confirm_reject: true,
            },
        };
    }

    if accept {
        return SharedRowResponse {
            op: Some(Op::AcceptShare {
                id,
                name: file.name.clone(),
            }),
            toggle_expand: false,
            confirm_reject: false,
        };
    }
    if reject {
        return SharedRowResponse {
            op: None,
            toggle_expand: false,
            confirm_reject: true,
        };
    }

    // Folder expand / doc open via whole-row primary click.
    let clicked = resp.clicked();
    let toggle_expand = clicked && has_kids && is_folder;
    let open_doc = clicked && !is_folder;

    SharedRowResponse {
        op: open_doc.then_some(Op::Open {
            id,
            new_tab: false,
        }),
        toggle_expand,
        confirm_reject: false,
    }
}

/// Context-menu payload for a Shared row (mix of shell `Op`s and local UI).
#[derive(Clone, Debug)]
enum SharedMenu {
    Op(Op),
    ToggleExpand,
    Decline,
}

fn share_mode_label(file: &lb::model::file::File) -> Option<&'static str> {
    // Prefer write if any write share targets us; else read.
    // Compact meta labels (not share-sheet "Can …" phrasing).
    use lb::model::file::ShareMode;
    let modes: Vec<_> = file.shares.iter().map(|s| s.mode).collect();
    if modes.is_empty() {
        return None;
    }
    if modes.iter().any(|m| matches!(m, ShareMode::Write)) {
        Some("Read & write")
    } else {
        Some("Read only")
    }
}

/// Sharer section — same size/spacing as recents time-span headers (`section_header`):
/// 13pt, 14 top / 2 bottom, muted. Person icon + username; count stays subtle.
fn sharer_section_header(ui: &mut Ui, t: &Tokens, username: &str, count: usize) {
    ui.add_space(14.0);
    let font = FontId::proportional(13.0);
    let icon_g = ui
        .painter()
        .layout_no_wrap(icons::USER.into(), icons::font(13.0), t.text_muted());
    let name_g = ui
        .painter()
        .layout_no_wrap(username.into(), font.clone(), t.text_muted());
    let count_g = ui
        .painter()
        .layout_no_wrap(format!("{count}"), font, t.text_muted());
    // Same vertical pad as `section_header` (text height + 6).
    let h = icon_g.size().y.max(name_g.size().y) + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let cy = rect.center().y;
    let mut x = rect.left() + ROW_PAD_X;
    ui.painter().galley(
        pos2(x, cy - icon_g.size().y / 2.0),
        icon_g.clone(),
        t.text_muted(),
    );
    x += icon_g.size().x + 5.0;
    ui.painter()
        .galley(pos2(x, cy - name_g.size().y / 2.0), name_g, t.text_muted());
    ui.painter().galley(
        pos2(
            rect.right() - ROW_PAD_X - count_g.size().x,
            cy - count_g.size().y / 2.0,
        ),
        count_g,
        t.text_muted(),
    );
    ui.add_space(2.0);
}

/// Ghost icon control (toolbar-like): no fill at rest, muted ink, soft hover.
fn paint_share_ghost(ui: &mut Ui, t: &Tokens, rect: Rect, icon: &str, hovered: bool) {
    let hover = if hovered { 1.0 } else { 0.0 };
    if hover > 0.0 {
        ui.painter().rect_filled(
            rect,
            6.0,
            t.canvas().lerp_to_gamma(t.surface_raised(), hover),
        );
    }
    let color = t.text_muted().lerp_to_gamma(t.fg(), hover);
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(15.0), color);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, color);
}

/// Parent path as `a / b / c` (no leaf name). Root children → `/`.
fn parent_breadcrumb(files: &impl FilesExt, id: Uuid) -> String {
    let Some(file) = files.get_by_id(id) else {
        return String::new();
    };
    if file.is_root() || file.parent == file.id {
        return "/".into();
    }
    let parent = file.parent;
    if files.get_by_id(parent).is_some_and(|p| p.is_root()) {
        return "/".into();
    }
    let path = files.path(parent);
    // Own-tree folder paths may end with `/`; trim for display.
    let path = path.trim_matches('/');
    if path.is_empty() {
        "/".into()
    } else {
        path.replace('/', " / ")
    }
}

/// Bucket docs into Apple-style age sections (order preserved: already sorted).
fn sectioned_docs<'a>(
    docs: &[&'a lb::model::file::File],
) -> Vec<(String, Vec<&'a lb::model::file::File>)> {
    let now = Local::now();
    let start_of_today = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let start_of_today = Local.from_local_datetime(&start_of_today).single().unwrap_or(now);

    let mut out: Vec<(String, Vec<&'a lb::model::file::File>)> = Vec::new();
    for file in docs {
        let title = section_title(file.last_modified, start_of_today);
        if out.last().is_some_and(|(t, _)| t == &title) {
            out.last_mut().unwrap().1.push(file);
        } else {
            out.push((title, vec![file]));
        }
    }
    out
}

/// Age bucket for a last-modified timestamp (ms since epoch). Matches Apple
/// `RecentsView.sectionTitle`: local calendar start-of-day boundaries, then
/// month name (same year) or year — never a current calendar month that still
/// falls inside "Previous 30 Days".
fn section_title(modified_ms: u64, start_of_today: chrono::DateTime<Local>) -> String {
    let Some(modified) = Local.timestamp_millis_opt(modified_ms as i64).single() else {
        return "Older".into();
    };

    if modified >= start_of_today {
        return "Today".into();
    }

    let yesterday = start_of_today - chrono::Duration::days(1);
    if modified >= yesterday {
        return "Yesterday".into();
    }

    let week_ago = start_of_today - chrono::Duration::days(7);
    if modified >= week_ago {
        return "Previous 7 Days".into();
    }

    let month_ago = start_of_today - chrono::Duration::days(30);
    if modified >= month_ago {
        return "Previous 30 Days".into();
    }

    // Older than 30 days: month name if same calendar year, else year.
    if modified.year() == start_of_today.year() {
        modified.format("%B").to_string() // e.g. "June"
    } else {
        modified.year().to_string()
    }
}

/// File-tree style row hover — canvas body, rounded 5, 5% fg wash.
fn paint_row_hover(ui: &Ui, t: &Tokens, rect: Rect, hover: f32) {
    if hover <= 0.0 {
        return;
    }
    ui.painter().rect_filled(
        rect,
        5.0,
        t.canvas().lerp_to_gamma(t.fg(), 0.05 * hover),
    );
}

/// Full-width allocation with file-tree side inset for the interactive/hit rect.
fn allocate_inset_row(ui: &mut Ui, h: f32) -> Rect {
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    Rect::from_min_max(
        pos2(outer.left() + SIDE_INSET, outer.top()),
        pos2(outer.right() - SIDE_INSET, outer.bottom()),
    )
}

/// Large pane name (“Recents”, “Shared with me”) — full `fg` so it stays
/// readable on canvas; contrast vs the workspace is the sidebar edge, not a
/// surface band.
fn pane_title(ui: &mut Ui, t: &Tokens, title: &str) {
    let font = FontId::proportional(20.0);
    let ink = t.fg();
    let g = ui.painter().layout_no_wrap(title.into(), font, ink);
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), g.size().y + 8.0), Sense::hover());
    ui.painter().galley(
        pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0),
        g,
        ink,
    );
}

fn section_header(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.add_space(14.0);
    let font = FontId::proportional(13.0);
    let g = ui
        .painter()
        .layout_no_wrap(title.into(), font, t.text_muted());
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), g.size().y + 6.0), Sense::hover());
    ui.painter().galley(
        pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0),
        g,
        t.text_muted(),
    );
    ui.add_space(2.0);
}

/// One recent doc row. Returns an `Op` on primary click (open) or context-menu.
///
/// Overflow policy (narrow → wide):
/// 1. Icon always reserved.
/// 2. Timestamp right-aligned only if name still has ≥ `MIN_NAME_W`.
/// 3. Pin mark sits after the name (reserved width); name ellipsizes first.
/// 4. Name + breadcrumb flex with end ellipsis inside the text band.
/// 5. Hover tip always (rich card — name/path/modified/status).
/// 6. Optional collaborator chip under the breadcrumb when last editor ≠ me.
#[allow(clippy::too_many_arguments)]
fn recent_row(
    ui: &mut Ui,
    t: &Tokens,
    files: &impl FilesExt,
    file: &lb::model::file::File,
    label: &str,
    when: &str,
    crumb: &str,
    collaborator: Option<&str>,
    is_pinned: bool,
    has_clip: bool,
    // Organized share (not owned by me) → “Remove from files”, not Delete.
    is_organized_share: bool,
    me: Option<&str>,
    status: Option<&Status>,
) -> Option<Op> {
    let id = file.id;
    let raw_name = file.name.as_str();
    // Height from content + equal top/bottom pad.
    let stack_h = if crumb.is_empty() {
        NAME_LINE_H
    } else {
        NAME_LINE_H + NAME_CRUMB_GAP + CRUMB_LINE_H
    };
    let collab_h = collaborator.map(|_| collab_chip_height(ui, t)).unwrap_or(0.0);
    let collab_block = if collab_h > 0.0 { COLLAB_ABOVE + collab_h } else { 0.0 };
    let h = ROW_PAD_Y * 2.0 + stack_h + collab_block;
    let rect = allocate_inset_row(ui, h);
    // Stable id so hover animation doesn't thrash when the list reorders.
    let resp = ui.interact(rect, Id::new(("recent_row", id)), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    paint_row_hover(ui, t, rect, hover);

    let icon = icons::for_doc_type(DocType::from_name(raw_name));
    let icon_g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(18.0), t.fg());
    // Relative time stays egui (ASCII); names/crumbs use Glyphon for emoji.
    let when_g = ui.painter().layout_no_wrap(
        when.into(),
        FontId::proportional(12.0),
        t.text_muted(),
    );
    // Quiet pin after the name (same muted push-pin as the file tree).
    let pin_g = if is_pinned {
        Some(ui.painter().layout_no_wrap(
            icons::PUSH_PIN.into(),
            icons::font(PIN_ICON_SIZE),
            t.text_muted(),
        ))
    } else {
        None
    };
    let pin_slot = pin_g
        .as_ref()
        .map(|g| NAME_PIN_GAP + g.size().x)
        .unwrap_or(0.0);

    let text_left = rect.left() + ROW_PAD_X + ICON_COL;
    let text_right = rect.right() - ROW_PAD_X;
    let text_band = (text_right - text_left).max(0.0);

    // Prefer keeping a readable name over the trailing meta.
    let when_w = when_g.size().x;
    let show_when = text_band >= MIN_NAME_W + pin_slot + NAME_META_GAP + when_w;
    let name_w = if show_when {
        (text_band - pin_slot - NAME_META_GAP - when_w).max(0.0)
    } else {
        (text_band - pin_slot).max(0.0)
    };

    // Hard clip for all text so nothing bleeds under the scrollbar / past the panel.
    let text_clip = Rect::from_min_max(
        pos2(text_left, rect.top()),
        pos2(text_right, rect.bottom()),
    )
    .intersect(ui.clip_rect());

    let icon_x = rect.left() + ROW_PAD_X;
    let cy = rect.center().y;
    ui.painter().galley(
        pos2(icon_x, cy - icon_g.size().y / 2.0),
        icon_g,
        t.fg(),
    );

    let name_y = rect.top() + ROW_PAD_Y;
    let name_rect = Rect::from_min_size(pos2(text_left, name_y), vec2(name_w, NAME_LINE_H));
    let (name_trunc, name_drawn_w) = paint_glyphon(
        ui,
        label,
        t.fg(),
        NAME_FONT,
        NAME_LINE_H,
        name_rect,
        text_clip,
    );

    if let Some(pg) = pin_g {
        let pin_x = text_left + name_drawn_w + NAME_PIN_GAP;
        let pin_y = name_y + (NAME_LINE_H - pg.size().y) * 0.5;
        let pin_clip = Rect::from_min_max(
            pos2(text_left, rect.top()),
            pos2(text_right, rect.bottom()),
        )
        .intersect(ui.clip_rect());
        if pin_clip.width() > 0.0 {
            ui.painter().with_clip_rect(pin_clip).galley(
                pos2(pin_x, pin_y),
                pg,
                t.text_muted(),
            );
        }
    }

    if show_when {
        let when_x = text_right - when_w;
        // Vertically center the timestamp on the name line.
        let when_y = name_y + (NAME_LINE_H - when_g.size().y) * 0.5;
        let when_clip = Rect::from_min_max(
            pos2(text_left + name_w + pin_slot + NAME_META_GAP * 0.5, rect.top()),
            pos2(text_right, rect.bottom()),
        )
        .intersect(ui.clip_rect());
        if when_clip.width() > 0.0 {
            ui.painter().with_clip_rect(when_clip).galley(
                pos2(when_x, when_y),
                when_g,
                t.text_muted(),
            );
        }
    }

    let mut crumb_trunc = false;
    let crumb_bottom = if !crumb.is_empty() && text_band > 0.0 {
        let crumb_y = name_y + NAME_LINE_H + NAME_CRUMB_GAP;
        let crumb_rect =
            Rect::from_min_size(pos2(text_left, crumb_y), vec2(text_band, CRUMB_LINE_H));
        let (trunc, _) = paint_glyphon(
            ui,
            crumb,
            t.text_muted(),
            CRUMB_FONT,
            CRUMB_LINE_H,
            crumb_rect,
            text_clip,
        );
        crumb_trunc = trunc;
        crumb_y + CRUMB_LINE_H
    } else {
        name_y + NAME_LINE_H
    };

    if let Some(user) = collaborator {
        let chip_y = crumb_bottom + COLLAB_ABOVE;
        paint_collab_chip(ui, t, text_left, chip_y, text_band, user);
    }
    let _ = (name_trunc, crumb_trunc);

    // Rich tip — same family as file tree (name+path tight, meta grouped).
    {
        let path = files.path(id);
        let modified_by = file.last_modified_by.as_str();
        let when_line = if !modified_by.is_empty() && modified_by != "<unknown>" {
            format!("Modified {when} · {modified_by}")
        } else {
            format!("Modified {when}")
        };
        let mut meta: Vec<String> = vec![when_line];
        if is_pinned {
            meta.push("Pinned".into());
        }
        if let Some(c) = collab_summary(&file.shares) {
            meta.push(c);
        }
        if me.is_some_and(|u| my_access_is_view_only(files, id, u)) {
            meta.push("View only".into());
        }
        if let Some(s) = status {
            if let Some(line) = sync_tip_line(id, s) {
                meta.push(line.into());
            }
        }
        // Display name only — no “(full.filename)” (row already shows the label).
        show_pane_tip(ui.ctx(), &resp, t, label, Some(path.as_str()), &meta);
    }

    // Context menu — same groups/order as file tree *documents*
    // (Open | Arrange | Share/export | Delete). No create / expand.
    let menu = crate::widgets::context_menu::show(&resp, t, |m| {
        // Open
        m.item(
            icons::ARROW_SQUARE_OUT,
            "Open",
            Op::Open {
                id,
                new_tab: false,
            },
        );
        m.item(
            icons::APP_WINDOW,
            "Open in new tab",
            Op::Open {
                id,
                new_tab: true,
            },
        );
        // Arrange — rename / place / pin / clipboard / duplicate
        m.separator();
        m.item(icons::PENCIL_SIMPLE, "Rename", Op::Rename { id });
        m.item(icons::FOLDERS, "Move", Op::Move { id });
        let (pin_icon, pin_label) = if is_pinned {
            (icons::PUSH_PIN_SLASH, "Unpin")
        } else {
            (icons::PUSH_PIN, "Pin")
        };
        m.item(pin_icon, pin_label, Op::TogglePin { id });
        m.item(icons::SCISSORS, "Cut", Op::Cut { id });
        m.item(icons::COPY, "Copy", Op::Copy { id });
        if has_clip {
            m.item(icons::CLIPBOARD, "Paste", Op::PasteIntoParent { id });
        }
        m.item(icons::FILES, "Duplicate", Op::Duplicate { id });
        // Share / export
        m.separator();
        m.item(icons::SHARE_NETWORK, "Share", Op::Share { id });
        m.item(icons::LINK, "Copy link", Op::CopyLink { id });
        m.item(icons::EXPORT, "Export", Op::Export { id });
        // Delete vs remove organized share from files.
        m.separator();
        if is_organized_share {
            m.item(icons::FOLDER_MINUS, "Remove from files", Op::Delete { id });
        } else {
            m.item_danger(icons::TRASH, "Delete", Op::Delete { id });
        }
    });
    if menu.is_some() {
        return menu;
    }

    if resp.clicked() {
        Some(Op::Open {
            id,
            new_tab: false,
        })
    } else {
        None
    }
}

fn collab_chip_height(ui: &Ui, t: &Tokens) -> f32 {
    let icon_h = ui
        .painter()
        .layout_no_wrap(icons::USER.into(), icons::font(COLLAB_ICON_SIZE), t.accent())
        .size()
        .y;
    let text_h = ui
        .painter()
        .layout_no_wrap("Ag".into(), FontId::proportional(COLLAB_FONT), t.accent())
        .size()
        .y;
    icon_h.max(text_h) + COLLAB_PAD_Y * 2.0
}

/// Capsule chip: person icon + collaborator username (Apple recents badge).
fn paint_collab_chip(
    ui: &mut Ui, t: &Tokens, x: f32, y: f32, max_w: f32, username: &str,
) {
    if max_w <= 0.0 {
        return;
    }
    let accent = t.accent();
    let icon_g = ui
        .painter()
        .layout_no_wrap(icons::USER.into(), icons::font(COLLAB_ICON_SIZE), accent);
    let name_g = ui.painter().layout_no_wrap(
        username.into(),
        FontId::proportional(COLLAB_FONT),
        accent,
    );

    let inner_h = icon_g.size().y.max(name_g.size().y);
    let chip_h = inner_h + COLLAB_PAD_Y * 2.0;
    let natural_w =
        COLLAB_PAD_X * 2.0 + icon_g.size().x + COLLAB_ICON_GAP + name_g.size().x;
    let chip_w = natural_w.min(max_w).max(0.0);
    if chip_w <= 0.0 {
        return;
    }

    let chip = Rect::from_min_size(pos2(x, y), vec2(chip_w, chip_h));
    let radius = chip_h * 0.5; // capsule
    ui.painter()
        .rect_filled(chip, radius, t.surface_raised());

    let cy = chip.center().y;
    let mut cx = chip.left() + COLLAB_PAD_X;
    ui.painter().galley(
        pos2(cx, cy - icon_g.size().y / 2.0),
        icon_g.clone(),
        accent,
    );
    cx += icon_g.size().x + COLLAB_ICON_GAP;

    // Truncate username if the chip is width-capped.
    let name_max = (chip.right() - COLLAB_PAD_X - cx).max(0.0);
    if name_max <= 0.0 {
        return;
    }
    if name_g.size().x <= name_max {
        ui.painter()
            .galley(pos2(cx, cy - name_g.size().y / 2.0), name_g, accent);
    } else {
        // Re-layout with wrap/ellipsis via LayoutJob so long handles still fit.
        use egui::text::{LayoutJob, TextFormat, TextWrapping};
        let mut job = LayoutJob {
            wrap: TextWrapping {
                max_width: name_max,
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            },
            ..Default::default()
        };
        job.append(
            username,
            0.0,
            TextFormat {
                font_id: FontId::proportional(COLLAB_FONT),
                color: accent,
                ..Default::default()
            },
        );
        let g = ui.fonts(|f| f.layout_job(job));
        ui.painter()
            .galley(pos2(cx, cy - g.size().y / 2.0), g, accent);
    }
}



/// Single-line Glyphon label (emoji-safe), ellipsis on overflow.
/// Returns `(truncated, drawn_width)` so callers can hang meta (pin) after the name.
fn paint_glyphon(
    ui: &mut Ui,
    text: &str,
    color: egui::Color32,
    font_size: f32,
    line_height: f32,
    rect: Rect,
    extra_clip: Rect,
) -> (bool, f32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || text.is_empty() {
        return (false, 0.0);
    }
    let clip = ui.clip_rect().intersect(rect).intersect(extra_clip);
    if clip.width() <= 0.0 || clip.height() <= 0.0 {
        return (false, 0.0);
    }

    let full_w = GlyphonLabel::new(text, color)
        .font_size(font_size)
        .line_height(line_height)
        .max_width(f32::MAX)
        .measure(ui)
        .x;
    let truncated = full_w > rect.width() + 0.5;

    let shaped = GlyphonLabel::new(text, color)
        .font_size(font_size)
        .line_height(line_height)
        .max_width(rect.width())
        .text_overflow(TextOverflow::EndEllipsis)
        .build(ui.ctx());
    let drawn_w = shaped.size.x.min(rect.width());
    // Place at the label rect; clip hard so overflow never escapes the slot.
    let area = shaped.text_area(rect, ui.ctx(), clip);
    ui.painter().add(
        egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
            clip,
            GlyphonRendererCallback::new(vec![area]),
        ),
    );
    (truncated, drawn_w)
}

fn empty_state(ui: &mut Ui, t: &Tokens, title: &str, subtitle: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        let title_g = ui
            .painter()
            .layout_no_wrap(title.into(), FontId::proportional(16.0), t.fg());
        let sub_g = ui.painter().layout(
            subtitle.into(),
            FontId::proportional(13.0),
            t.text_muted(),
            220.0,
        );
        let (tr, _) = ui.allocate_exact_size(title_g.size(), Sense::hover());
        ui.painter().galley(tr.min, title_g, t.fg());
        ui.add_space(6.0);
        let (sr, _) = ui.allocate_exact_size(sub_g.size(), Sense::hover());
        ui.painter().galley(sr.min, sub_g, t.text_muted());
    });
}

// ── Rich hover tips (same family as file tree) ─────────────────────────────

/// Name + place tight; meta block after a gap (matches tree tip spacing).
fn show_pane_tip(
    ctx: &egui::Context, resp: &egui::Response, t: &Tokens, title: &str,
    path: Option<&str>, meta: &[String],
) {
    tip_ui_rich(ctx, resp, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(egui::RichText::new(title).size(14.0).strong().color(t.fg()));
        if let Some(p) = path {
            if !p.is_empty() {
                ui.label(egui::RichText::new(p).size(12.5).color(t.text_muted()));
            }
        }
        if !meta.is_empty() {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for line in meta {
                ui.label(egui::RichText::new(line).size(12.5).color(t.text_muted()));
            }
        }
    });
}

fn collab_summary(shares: &[lb::model::file::Share]) -> Option<String> {
    if shares.is_empty() {
        return None;
    }
    let n = shares.len();
    let writes = shares
        .iter()
        .filter(|s| matches!(s.mode, ShareMode::Write))
        .count();
    let people = if n == 1 {
        "1 collaborator".to_string()
    } else {
        format!("{n} collaborators")
    };
    Some(if writes == n {
        format!("{people} can edit")
    } else if writes == 0 {
        format!("{people} can view")
    } else {
        people
    })
}

fn my_access_is_view_only(files: &impl FilesExt, id: Uuid, me: &str) -> bool {
    let Some(file) = files.get_by_id(id) else {
        return false;
    };
    if file.owner.eq_ignore_ascii_case(me) {
        return false;
    }
    let mut best: Option<ShareMode> = None;
    for fid in std::iter::once(id).chain(files.ancestors(id)) {
        let Some(f) = files.get_by_id(fid) else {
            continue;
        };
        for s in &f.shares {
            if !s.shared_with.eq_ignore_ascii_case(me) {
                continue;
            }
            best = Some(match (best, s.mode) {
                (Some(ShareMode::Write), _) | (_, ShareMode::Write) => ShareMode::Write,
                _ => ShareMode::Read,
            });
        }
    }
    matches!(best, Some(ShareMode::Read))
}

fn sync_tip_line(id: Uuid, status: &Status) -> Option<&'static str> {
    if status.pulling_files.contains(&id) {
        return Some("Downloading…");
    }
    if status.pushing_files.contains(&id) {
        return Some("Uploading…");
    }
    if status.dirty_locally.contains(&id) {
        return Some("Not synced yet");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    /// Midday local on a calendar day, as epoch millis (test helper).
    fn local_ms(year: i32, month: u32, day: u32) -> u64 {
        Local
            .with_ymd_and_hms(year, month, day, 12, 0, 0)
            .single()
            .expect("valid local datetime")
            .timestamp_millis() as u64
    }

    fn start_of(year: i32, month: u32, day: u32) -> chrono::DateTime<Local> {
        let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        Local.from_local_datetime(&naive).single().unwrap()
    }

    #[test]
    fn section_buckets_prefer_rolling_windows_over_current_month() {
        // "Today" = 19 July 2026 local — same situation as the reported bug.
        let today = start_of(2026, 7, 19);

        assert_eq!(section_title(local_ms(2026, 7, 19), today), "Today");
        assert_eq!(section_title(local_ms(2026, 7, 18), today), "Yesterday");
        assert_eq!(section_title(local_ms(2026, 7, 14), today), "Previous 7 Days");
        // Still July, but >7 days ago → Previous 30 Days, never "July".
        assert_eq!(section_title(local_ms(2026, 7, 1), today), "Previous 30 Days");
        // Within last 30 days, calendar June → still Previous 30 Days.
        assert_eq!(section_title(local_ms(2026, 6, 25), today), "Previous 30 Days");
        // Older than 30 days, same year → month name.
        assert_eq!(section_title(local_ms(2026, 6, 1), today), "June");
        assert_eq!(section_title(local_ms(2026, 5, 15), today), "May");
        // Prior year → year label.
        assert_eq!(section_title(local_ms(2025, 12, 1), today), "2025");
    }
}
