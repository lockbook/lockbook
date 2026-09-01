//! Shell action vocabulary — one door into `ShellApp::apply`.

use lb::Uuid;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SidebarPane {
    #[default]
    Files,
    Recents,
    Shared,
}

impl SidebarPane {
    pub const ALL: [Self; 3] = [Self::Files, Self::Recents, Self::Shared];

    pub fn title(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Recents => "Recents",
            // Titlebar tip + chrome: inbound pending shares only.
            Self::Shared => "Shared with me",
        }
    }

    pub fn icon(self) -> &'static str {
        use crate::components::phosphor;
        match self {
            Self::Files => phosphor::FOLDER,
            Self::Recents => phosphor::CLOCK,
            Self::Shared => phosphor::USERS,
        }
    }
}

/// Settings rail — Account · App · Editor · Debug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsCat {
    Account,
    App,
    Editor,
    Debug,
}

impl SettingsCat {
    pub const ALL: [Self; 4] = [Self::Account, Self::App, Self::Editor, Self::Debug];

    pub fn title(self) -> &'static str {
        match self {
            Self::Account => "Account",
            Self::App => "App",
            Self::Editor => "Editor",
            Self::Debug => "Debug",
        }
    }

    pub fn icon(self) -> &'static str {
        use crate::components::phosphor;
        match self {
            Self::Account => phosphor::USER,
            Self::App => phosphor::GEAR,
            Self::Editor => phosphor::MARKDOWN_LOGO,
            Self::Debug => phosphor::CODE,
        }
    }
}

/// Username field / staged-person lookup state (share invite).
/// `Found` = that username exists and can be invited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ShareLookup {
    #[default]
    Idle,
    Checking,
    Found,
    NotFound,
    Error(String),
}

/// Create-account username availability (signed-out `username_exists`).
/// `Available` = name is free; `Taken` = already claimed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OnboardLookup {
    #[default]
    Idle,
    Checking,
    Available,
    Taken,
    Error(String),
}

/// One person in the multi-add stage (batch invite, one access mode for all).
#[derive(Clone, Debug)]
pub struct ShareStaged {
    pub username: String,
    pub lookup: ShareLookup,
}

/// Open sheet state. **Not `Clone`:** live edit strings must not be snapshotted
/// each frame for `Field` (in-place modal fields). Copy cheap identity by value.
#[derive(Debug)]
pub enum Modal {
    Settings {
        cat: SettingsCat,
    },
    Delete {
        ids: Vec<Uuid>,
    },
    /// Share sheet: access list + multi-add stage + one access mode for the batch.
    Share {
        id: Uuid,
        /// Trailing field text (incomplete token after last comma).
        query: String,
        /// 0 = Can edit (Write), 1 = Can view (Read) for everyone in the stage.
        mode: usize,
        /// People to invite on Share (comma / paste accumulates here).
        staged: Vec<ShareStaged>,
        /// Lookup for the current field token only.
        lookup: ShareLookup,
        /// Last field query we ran verify against.
        lookup_for: String,
        err: Option<String>,
    },
    /// Multi-step create sheet (type → name → location).
    Create {
        name: String,
        kind: CreateKind,
        /// Destination parent (resolved from the selected location plate).
        parent: Option<Uuid>,
        loc: CreateLoc,
        /// Document to create next to: (parent folder, document name).
        /// Shown whenever a document tab is open or Create was invoked on a file.
        alongside: Option<(Uuid, String)>,
        /// Folder from Choose… / folder-context Create. Independent of `loc`.
        /// Never the account root — Home is its own plate.
        chosen: Option<Uuid>,
        /// Nested folder list for "Choose…".
        picking: bool,
        error: Option<String>,
        /// User edited the name — stop auto-refreshing when kind/loc changes.
        name_dirty: bool,
    },
    Move {
        ids: Vec<Uuid>,
        dest: Option<Uuid>,
    },
    /// Rename sheet: editable **stem**; optional static extension (create-style).
    Rename {
        id: Uuid,
        /// Editable buffer (stem only when `ext` is set).
        name: String,
        /// Trailing extension including the dot (e.g. `".md"`). Not in the field.
        ext: Option<String>,
    },
    /// Accept pending share: pick parent folder.
    AcceptShare {
        id: Uuid,
        name: String,
        dest: Option<Uuid>,
    },
    /// Decline pending share (confirm first — not a remote delete).
    DeclineShare {
        id: Uuid,
        name: String,
    },
    /// Import dropped/picked files: pick parent folder.
    ImportParent {
        paths: Vec<PathBuf>,
        dest: Option<Uuid>,
    },
    Help,
    Onboard {
        mode: OnboardMode,
        uname: String,
        /// Create: debounced server availability.
        uname_lookup: OnboardLookup,
        /// Username last settled by network / invalid local check.
        uname_lookup_for: String,
        /// Compact key or 24-word phrase — `import_account` accepts either.
        account_key: String,
        busy: bool,
        err: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OnboardMode {
    #[default]
    Choice,
    Create,
    Import,
}

/// Stripe upgrade sheet stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UpgradeStage {
    #[default]
    EnterCard,
    Confirm,
    Paying,
}

/// Create-sheet file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CreateKind {
    #[default]
    Note,
    Drawing,
    Folder,
    Other,
}

impl CreateKind {
    pub const ALL: [CreateKind; 4] =
        [CreateKind::Note, CreateKind::Drawing, CreateKind::Folder, CreateKind::Other];

    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Drawing => "Drawing",
            Self::Folder => "Folder",
            Self::Other => "Other",
        }
    }

    pub fn ext(self) -> Option<&'static str> {
        match self {
            Self::Note => Some(".md"),
            Self::Drawing => Some(".svg"),
            Self::Folder | Self::Other => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Note => 0,
            Self::Drawing => 1,
            Self::Folder => 2,
            Self::Other => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(Self::Note)
    }
}

/// Where the new file will land.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CreateLoc {
    #[default]
    Root,
    Alongside,
    Custom,
}

/// App-level intents. Widgets only enqueue; `apply` mutates.
#[derive(Clone, Debug)]
pub enum Action {
    SelectPane(SidebarPane),
    ToggleSidebar,
    SelectFile(Uuid),
    /// Replace selection with ids (multi-select).
    SetSelection(Vec<Uuid>),
    ToggleSelect(Uuid),
    /// Shift-click: select range from anchor to id.
    SelectRange(Uuid),
    ToggleExpand(Uuid),
    ExpandSubtree(Uuid),
    CollapseSubtree(Uuid),
    OpenFile(Uuid),
    /// Open document in a new tab.
    OpenFileNewTab(Uuid),
    /// Per-tab workspace history (same as mobile `nav_back` / `nav_forward`).
    NavBack,
    NavForward,
    /// Open documents (filters folders). Multi: first reuses tab path unless
    /// `new_tab`; further docs always open as new tabs.
    OpenDocuments {
        ids: Vec<Uuid>,
        new_tab: bool,
    },
    SelectTab(usize),
    CloseTab(usize),
    /// Close every tab except `keep` (workspace `close_other_tabs`).
    CloseOtherTabs(usize),
    CloseTabsToLeft(usize),
    CloseTabsToRight(usize),
    CloseAllTabs,
    /// LIFO restore of most recently closed tab.
    ReopenClosedTab,
    /// Drag-reorder in the shell tab strip. `dst` is insert-before index in
    /// **pre-remove** coordinates (drop left of tab `i` → `dst = i`; drop right
    /// → `dst = i + 1`). No-ops when `dst == src` or `dst == src + 1`.
    ReorderTab {
        src: usize,
        dst: usize,
    },
    OpenSettings,
    CloseModal,
    SetSettingsCat(SettingsCat),
    OpenDelete(Vec<Uuid>),
    ConfirmDelete,
    OpenShare(Uuid),
    /// Field text already on `Modal::Share.query`; stage commas + local-known stamp.
    ShareQuery,
    ShareMode(usize),
    /// Debounced existence check for the **field** token.
    ShareVerify,
    /// ⏎ in the field: stage the current token (like a comma), do not invite.
    ShareStageField,
    /// Share all Found staged people (+ Found field token) with the sheet mode.
    /// Keyboard: ⌘⏎ / Ctrl+⏎ (plain ⏎ stages instead).
    ShareInvite,
    /// Drop someone from the multi-add stage (chip dismiss).
    ShareUnstage(String),
    /// Open create sheet. Folder-context fills `folder` and selects Choose;
    /// file-context fills `alongside` and selects that plate (Choose stays empty).
    /// Chrome / ⌘N leaves both None and follows the open document tab.
    OpenCreate {
        folder: Option<Uuid>,
        alongside: Option<Uuid>,
        is_folder: bool,
    },
    CreateSetKind(CreateKind),
    CreateSetLoc(CreateLoc),
    /// Expand/collapse the full folder browser under Location.
    CreateSetPicking(bool),
    CreatePickFolder(Uuid),
    ConfirmCreate,
    OpenMove(Vec<Uuid>),
    MoveSelect(Uuid),
    ConfirmMove,
    OpenRename(Uuid),
    ConfirmRename,
    OpenAcceptShare {
        id: Uuid,
        name: String,
    },
    AcceptShareDest(Uuid),
    ConfirmAcceptShare,
    /// Open decline confirm sheet.
    OpenDeclineShare {
        id: Uuid,
        name: String,
    },
    /// Confirmed decline (delete pending share link only).
    ConfirmDeclineShare(Uuid),
    OpenHelp,
    OnboardSetMode(OnboardMode),
    /// Debounced create-username availability (share-style network verify).
    OnboardVerifyUname,
    /// `show_error`: false for auto-submit (silent fail until the secret changes).
    OnboardSubmit {
        show_error: bool,
    },
    RequestSync,
    TogglePin(Uuid),
    /// Toggle pin on each id (own state).
    TogglePinMany(Vec<Uuid>),
    /// Drag-drop: move ids into parent.
    MoveInto {
        ids: Vec<Uuid>,
        parent: Uuid,
    },
    /// Document-only; folders skipped (`FileNotDocument`).
    Duplicate(Vec<Uuid>),
    /// Export each id into one picked destination folder.
    Export(Vec<Uuid>),
    /// Copy lb:// or https file link to OS clipboard.
    CopyLink(Uuid),
    Import,
    ImportPaths {
        paths: Vec<PathBuf>,
        parent: Uuid,
    },
    /// Dropped files: pick parent folder first.
    OpenImportParent {
        paths: Vec<PathBuf>,
    },
    ImportParentSelect(Uuid),
    ConfirmImportParent,
    OpenSearch,
    /// Open cancel-subscription confirm.
    CancelSubscription,
    ConfirmCancelSub,
    SetThemeMode(crate::components::ModePreference),
    SetThemeFamily(crate::components::ThemeFamily),
    SetPrefLinkPreviews(bool),
    SetPrefSidebarUsage(bool),

    /// Linux only — persisted; takes effect after restart.
    SetPrefAllowWayland(bool),
    /// Settings → Account: show 24-word phrase in-content.
    RevealPhrase,
    /// Settings → Account: show account key QR in-content.
    OpenAccountQr,
    /// Close in-content Account panel (phrase / QR / manage) — Esc / Back.
    HideAccountKey,
    CopyPhrase,
    RevealDebugInfo,
    HideDebugInfo,
    /// Kick off background `Lb::debug_info` if idle (Debug settings tab).
    EnsureDebugInfo,
    /// Force-regenerate debug info JSON.
    RefreshDebugInfo,
    /// Copy ready debug info to the OS clipboard.
    CopyDebugInfo,
    /// Open Stripe upgrade sheet (free tier).
    OpenUpgrade,
    UpgradeBack,
    UpgradeNext,
    UpgradeConfirmPay,
    /// After success/error, leave Paying for Settings → Plan.
    UpgradeDone,
    OpenLogout,
    LogoutAck(bool),
    ConfirmLogout,
    OpenDeleteAccount,
    ConfirmDeleteAccount,
    /// Sidebar Create chip / ⌘N — open create sheet (default Note).
    Create,
    SaveAll,
}

impl Action {
    /// Variant name only — safe for traces (no paths, secrets, or ids).
    pub fn name(&self) -> &'static str {
        match self {
            Self::SelectPane(_) => "SelectPane",
            Self::ToggleSidebar => "ToggleSidebar",
            Self::SelectFile(_) => "SelectFile",
            Self::SetSelection(_) => "SetSelection",
            Self::ToggleSelect(_) => "ToggleSelect",
            Self::SelectRange(_) => "SelectRange",
            Self::ToggleExpand(_) => "ToggleExpand",
            Self::ExpandSubtree(_) => "ExpandSubtree",
            Self::CollapseSubtree(_) => "CollapseSubtree",
            Self::OpenFile(_) => "OpenFile",
            Self::OpenFileNewTab(_) => "OpenFileNewTab",
            Self::NavBack => "NavBack",
            Self::NavForward => "NavForward",
            Self::OpenDocuments { .. } => "OpenDocuments",
            Self::SelectTab(_) => "SelectTab",
            Self::CloseTab(_) => "CloseTab",
            Self::CloseOtherTabs(_) => "CloseOtherTabs",
            Self::CloseTabsToLeft(_) => "CloseTabsToLeft",
            Self::CloseTabsToRight(_) => "CloseTabsToRight",
            Self::CloseAllTabs => "CloseAllTabs",
            Self::ReopenClosedTab => "ReopenClosedTab",
            Self::ReorderTab { .. } => "ReorderTab",
            Self::OpenSettings => "OpenSettings",
            Self::CloseModal => "CloseModal",
            Self::SetSettingsCat(_) => "SetSettingsCat",
            Self::OpenDelete(_) => "OpenDelete",
            Self::ConfirmDelete => "ConfirmDelete",
            Self::OpenShare(_) => "OpenShare",
            Self::ShareQuery => "ShareQuery",
            Self::ShareMode(_) => "ShareMode",
            Self::ShareVerify => "ShareVerify",
            Self::ShareStageField => "ShareStageField",
            Self::ShareInvite => "ShareInvite",
            Self::ShareUnstage(_) => "ShareUnstage",
            Self::OpenCreate { .. } => "OpenCreate",
            Self::CreateSetKind(_) => "CreateSetKind",
            Self::CreateSetLoc(_) => "CreateSetLoc",
            Self::CreateSetPicking(_) => "CreateSetPicking",
            Self::CreatePickFolder(_) => "CreatePickFolder",
            Self::ConfirmCreate => "ConfirmCreate",
            Self::OpenMove(_) => "OpenMove",
            Self::MoveSelect(_) => "MoveSelect",
            Self::ConfirmMove => "ConfirmMove",
            Self::OpenRename(_) => "OpenRename",
            Self::ConfirmRename => "ConfirmRename",
            Self::OpenAcceptShare { .. } => "OpenAcceptShare",
            Self::AcceptShareDest(_) => "AcceptShareDest",
            Self::ConfirmAcceptShare => "ConfirmAcceptShare",
            Self::OpenDeclineShare { .. } => "OpenDeclineShare",
            Self::ConfirmDeclineShare(_) => "ConfirmDeclineShare",
            Self::OpenHelp => "OpenHelp",
            Self::OnboardSetMode(_) => "OnboardSetMode",
            Self::OnboardVerifyUname => "OnboardVerifyUname",
            Self::OnboardSubmit { .. } => "OnboardSubmit",
            Self::RequestSync => "RequestSync",
            Self::TogglePin(_) => "TogglePin",
            Self::TogglePinMany(_) => "TogglePinMany",
            Self::MoveInto { .. } => "MoveInto",
            Self::Duplicate(_) => "Duplicate",
            Self::Export(_) => "Export",
            Self::CopyLink(_) => "CopyLink",
            Self::Import => "Import",
            Self::ImportPaths { .. } => "ImportPaths",
            Self::OpenImportParent { .. } => "OpenImportParent",
            Self::ImportParentSelect(_) => "ImportParentSelect",
            Self::ConfirmImportParent => "ConfirmImportParent",
            Self::OpenSearch => "OpenSearch",
            Self::CancelSubscription => "CancelSubscription",
            Self::ConfirmCancelSub => "ConfirmCancelSub",
            Self::SetThemeMode(_) => "SetThemeMode",
            Self::SetThemeFamily(_) => "SetThemeFamily",
            Self::SetPrefLinkPreviews(_) => "SetPrefLinkPreviews",
            Self::SetPrefSidebarUsage(_) => "SetPrefSidebarUsage",
            Self::SetPrefAllowWayland(_) => "SetPrefAllowWayland",
            Self::RevealPhrase => "RevealPhrase",
            Self::OpenAccountQr => "OpenAccountQr",
            Self::HideAccountKey => "HideAccountKey",
            Self::CopyPhrase => "CopyPhrase",
            Self::RevealDebugInfo => "RevealDebugInfo",
            Self::HideDebugInfo => "HideDebugInfo",
            Self::EnsureDebugInfo => "EnsureDebugInfo",
            Self::RefreshDebugInfo => "RefreshDebugInfo",
            Self::CopyDebugInfo => "CopyDebugInfo",
            Self::OpenUpgrade => "OpenUpgrade",
            Self::UpgradeBack => "UpgradeBack",
            Self::UpgradeNext => "UpgradeNext",
            Self::UpgradeConfirmPay => "UpgradeConfirmPay",
            Self::UpgradeDone => "UpgradeDone",
            Self::OpenLogout => "OpenLogout",
            Self::LogoutAck(_) => "LogoutAck",
            Self::ConfirmLogout => "ConfirmLogout",
            Self::OpenDeleteAccount => "OpenDeleteAccount",
            Self::ConfirmDeleteAccount => "ConfirmDeleteAccount",
            Self::Create => "Create",
            Self::SaveAll => "SaveAll",
        }
    }
}
