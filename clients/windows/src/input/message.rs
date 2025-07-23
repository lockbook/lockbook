use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::file_drop;

#[derive(Clone, Copy, Debug)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl From<LPARAM> for Point<u16> {
    fn from(lparam: LPARAM) -> Self {
        Point { x: loword_l(lparam), y: hiword_l(lparam) }
    }
}

/// Windows message parsed to properly interpret or ignore wparam and lparam (but not to redefine any structs defined
/// in winapi) for clarity and exhaustive matching.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Message<'a> {
    Unknown { msg: u32 },
    Unhandled { const_name: &'static str },
    NoDeps(MessageNoDeps<'a>),
    WindowDep(MessageWindowDep),
    AppDep(MessageAppDep),
    FileDrop(file_drop::Message),
}

#[derive(Clone, Copy, Debug)]
pub enum MessageNoDeps<'a> {
    Create { create_struct: &'a CREATESTRUCTA },
    Destroy,
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub enum MessageWindowDep {
    DpiChanged { dpi: u16, suggested_rect: RECT },
    Size { width: u16, height: u16 },
}

#[derive(Clone, Copy, Debug)]
pub enum MessageAppDep {
    KeyDown { key: VIRTUAL_KEY },
    KeyUp { key: VIRTUAL_KEY },
    LButtonDown { pos: Point<u16> },
    LButtonUp { pos: Point<u16> },
    MouseHWheel { delta: i16 },
    MouseMove { pos: Point<u16> },
    MouseWheel { delta: i16 },
    Paint,
    PointerDown { pointer_id: u16 },
    PointerUp { pointer_id: u16 },
    PointerUpdate { pointer_id: u16 },
    RButtonDown { pos: Point<u16> },
    RButtonUp { pos: Point<u16> },
    SetCursor,
}

impl Message<'_> {
    pub fn new(msg: u32, wparam: WPARAM, lparam: LPARAM) -> Self {
        let lparam_loword = loword_l(lparam);
        let lparam_hiword = hiword_l(lparam);
        let wparam_loword = loword_w(wparam);
        let wparam_hiword = hiword_w(wparam);

        match msg {
            WM_ACTIVATE => Message::Unhandled { const_name: "WM_ACTIVATE" },
            WM_ACTIVATEAPP => Message::Unhandled { const_name: "WM_ACTIVATEAPP" },
            WM_AFXFIRST => Message::Unhandled { const_name: "WM_AFXFIRST" },
            WM_AFXLAST => Message::Unhandled { const_name: "WM_AFXLAST" },
            WM_APP => Message::Unhandled { const_name: "WM_APP" },
            WM_APPCOMMAND => Message::Unhandled { const_name: "WM_APPCOMMAND" },
            WM_ASKCBFORMATNAME => Message::Unhandled { const_name: "WM_ASKCBFORMATNAME" },
            WM_CANCELJOURNAL => Message::Unhandled { const_name: "WM_CANCELJOURNAL" },
            WM_CANCELMODE => Message::Unhandled { const_name: "WM_CANCELMODE" },
            WM_CAPTURECHANGED => Message::Unhandled { const_name: "WM_CAPTURECHANGED" },
            WM_CHANGECBCHAIN => Message::Unhandled { const_name: "WM_CHANGECBCHAIN" },
            WM_CHANGEUISTATE => Message::Unhandled { const_name: "WM_CHANGEUISTATE" },
            WM_CHAR => Message::Unhandled { const_name: "WM_CHAR" },
            WM_CHARTOITEM => Message::Unhandled { const_name: "WM_CHARTOITEM" },
            WM_CHILDACTIVATE => Message::Unhandled { const_name: "WM_CHILDACTIVATE" },
            WM_CLEAR => Message::Unhandled { const_name: "WM_CLEAR" },
            WM_CLIPBOARDUPDATE => Message::Unhandled { const_name: "WM_CLIPBOARDUPDATE" },
            WM_CLOSE => Message::Unhandled { const_name: "WM_CLOSE" },
            WM_COMMAND => Message::Unhandled { const_name: "WM_COMMAND" },
            WM_COMMNOTIFY => Message::Unhandled { const_name: "WM_COMMNOTIFY" },
            WM_COMPACTING => Message::Unhandled { const_name: "WM_COMPACTING" },
            WM_COMPAREITEM => Message::Unhandled { const_name: "WM_COMPAREITEM" },
            WM_CONTEXTMENU => Message::Unhandled { const_name: "WM_CONTEXTMENU" },
            WM_COPY => Message::Unhandled { const_name: "WM_COPY" },
            WM_COPYDATA => Message::Unhandled { const_name: "WM_COPYDATA" },
            WM_CREATE => Message::NoDeps(MessageNoDeps::Create {
                create_struct: unsafe {
                    std::mem::transmute::<windows::Win32::Foundation::LPARAM, &CREATESTRUCTA>(
                        lparam,
                    )
                },
            }),
            WM_CTLCOLORBTN => Message::Unhandled { const_name: "WM_CTLCOLORBTN" },
            WM_CTLCOLORDLG => Message::Unhandled { const_name: "WM_CTLCOLORDLG" },
            WM_CTLCOLOREDIT => Message::Unhandled { const_name: "WM_CTLCOLOREDIT" },
            WM_CTLCOLORLISTBOX => Message::Unhandled { const_name: "WM_CTLCOLORLISTBOX" },
            WM_CTLCOLORMSGBOX => Message::Unhandled { const_name: "WM_CTLCOLORMSGBOX" },
            WM_CTLCOLORSCROLLBAR => Message::Unhandled { const_name: "WM_CTLCOLORSCROLLBAR" },
            WM_CTLCOLORSTATIC => Message::Unhandled { const_name: "WM_CTLCOLORSTATIC" },
            WM_CUT => Message::Unhandled { const_name: "WM_CUT" },
            WM_DEADCHAR => Message::Unhandled { const_name: "WM_DEADCHAR" },
            WM_DELETEITEM => Message::Unhandled { const_name: "WM_DELETEITEM" },
            WM_DESTROY => Message::NoDeps(MessageNoDeps::Destroy),
            WM_DESTROYCLIPBOARD => Message::Unhandled { const_name: "WM_DESTROYCLIPBOARD" },
            WM_DEVICECHANGE => Message::Unhandled { const_name: "WM_DEVICECHANGE" },
            WM_DEVMODECHANGE => Message::Unhandled { const_name: "WM_DEVMODECHANGE" },
            WM_DISPLAYCHANGE => Message::Unhandled { const_name: "WM_DISPLAYCHANGE" },
            WM_DPICHANGED => Message::WindowDep(MessageWindowDep::DpiChanged {
                dpi: wparam_loword,
                suggested_rect: unsafe { *(lparam.0 as *const RECT) },
            }),
            WM_DPICHANGED_AFTERPARENT => {
                Message::Unhandled { const_name: "WM_DPICHANGED_AFTERPARENT" }
            }
            WM_DPICHANGED_BEFOREPARENT => {
                Message::Unhandled { const_name: "WM_DPICHANGED_BEFOREPARENT" }
            }
            WM_DRAWCLIPBOARD => Message::Unhandled { const_name: "WM_DRAWCLIPBOARD" },
            WM_DRAWITEM => Message::Unhandled { const_name: "WM_DRAWITEM" },
            WM_DROPFILES => Message::Unhandled { const_name: "WM_DROPFILES" },
            WM_DWMCOLORIZATIONCOLORCHANGED => {
                Message::Unhandled { const_name: "WM_DWMCOLORIZATIONCOLORCHANGED" }
            }
            WM_DWMCOMPOSITIONCHANGED => {
                Message::Unhandled { const_name: "WM_DWMCOMPOSITIONCHANGED" }
            }
            WM_DWMNCRENDERINGCHANGED => {
                Message::Unhandled { const_name: "WM_DWMNCRENDERINGCHANGED" }
            }
            WM_DWMSENDICONICLIVEPREVIEWBITMAP => {
                Message::Unhandled { const_name: "WM_DWMSENDICONICLIVEPREVIEWBITMAP" }
            }
            WM_DWMSENDICONICTHUMBNAIL => {
                Message::Unhandled { const_name: "WM_DWMSENDICONICTHUMBNAIL" }
            }
            WM_DWMWINDOWMAXIMIZEDCHANGE => {
                Message::Unhandled { const_name: "WM_DWMWINDOWMAXIMIZEDCHANGE" }
            }
            WM_ENABLE => Message::Unhandled { const_name: "WM_ENABLE" },
            WM_ENDSESSION => Message::Unhandled { const_name: "WM_ENDSESSION" },
            WM_ENTERIDLE => Message::Unhandled { const_name: "WM_ENTERIDLE" },
            WM_ENTERMENULOOP => Message::Unhandled { const_name: "WM_ENTERMENULOOP" },
            WM_ENTERSIZEMOVE => Message::Unhandled { const_name: "WM_ENTERSIZEMOVE" },
            WM_ERASEBKGND => Message::Unhandled { const_name: "WM_ERASEBKGND" },
            WM_EXITMENULOOP => Message::Unhandled { const_name: "WM_EXITMENULOOP" },
            WM_EXITSIZEMOVE => Message::Unhandled { const_name: "WM_EXITSIZEMOVE" },
            WM_FONTCHANGE => Message::Unhandled { const_name: "WM_FONTCHANGE" },
            WM_GESTURE => Message::Unhandled { const_name: "WM_GESTURE" },
            WM_GESTURENOTIFY => Message::Unhandled { const_name: "WM_GESTURENOTIFY" },
            WM_GETDLGCODE => Message::Unhandled { const_name: "WM_GETDLGCODE" },
            WM_GETDPISCALEDSIZE => Message::Unhandled { const_name: "WM_GETDPISCALEDSIZE" },
            WM_GETFONT => Message::Unhandled { const_name: "WM_GETFONT" },
            WM_GETHOTKEY => Message::Unhandled { const_name: "WM_GETHOTKEY" },
            WM_GETICON => Message::Unhandled { const_name: "WM_GETICON" },
            WM_GETMINMAXINFO => Message::Unhandled { const_name: "WM_GETMINMAXINFO" },
            WM_GETOBJECT => Message::Unhandled { const_name: "WM_GETOBJECT" },
            WM_GETTEXT => Message::Unhandled { const_name: "WM_GETTEXT" },
            WM_GETTEXTLENGTH => Message::Unhandled { const_name: "WM_GETTEXTLENGTH" },
            WM_GETTITLEBARINFOEX => Message::Unhandled { const_name: "WM_GETTITLEBARINFOEX" },
            WM_HANDHELDFIRST => Message::Unhandled { const_name: "WM_HANDHELDFIRST" },
            WM_HANDHELDLAST => Message::Unhandled { const_name: "WM_HANDHELDLAST" },
            WM_HELP => Message::Unhandled { const_name: "WM_HELP" },
            WM_HOTKEY => Message::Unhandled { const_name: "WM_HOTKEY" },
            WM_HSCROLL => Message::Unhandled { const_name: "WM_HSCROLL" },
            WM_HSCROLLCLIPBOARD => Message::Unhandled { const_name: "WM_HSCROLLCLIPBOARD" },
            WM_ICONERASEBKGND => Message::Unhandled { const_name: "WM_ICONERASEBKGND" },
            WM_IME_CHAR => Message::Unhandled { const_name: "WM_IME_CHAR" },
            WM_IME_COMPOSITION => Message::Unhandled { const_name: "WM_IME_COMPOSITION" },
            WM_IME_COMPOSITIONFULL => Message::Unhandled { const_name: "WM_IME_COMPOSITIONFULL" },
            WM_IME_CONTROL => Message::Unhandled { const_name: "WM_IME_CONTROL" },
            WM_IME_ENDCOMPOSITION => Message::Unhandled { const_name: "WM_IME_ENDCOMPOSITION" },
            WM_IME_KEYDOWN => Message::Unhandled { const_name: "WM_IME_KEYDOWN" },
            WM_IME_KEYUP => Message::Unhandled { const_name: "WM_IME_KEYUP" },
            WM_IME_NOTIFY => Message::Unhandled { const_name: "WM_IME_NOTIFY" },
            WM_IME_REQUEST => Message::Unhandled { const_name: "WM_IME_REQUEST" },
            WM_IME_SELECT => Message::Unhandled { const_name: "WM_IME_SELECT" },
            WM_IME_SETCONTEXT => Message::Unhandled { const_name: "WM_IME_SETCONTEXT" },
            WM_IME_STARTCOMPOSITION => Message::Unhandled { const_name: "WM_IME_STARTCOMPOSITION" },
            WM_INITDIALOG => Message::Unhandled { const_name: "WM_INITDIALOG" },
            WM_INITMENU => Message::Unhandled { const_name: "WM_INITMENU" },
            WM_INITMENUPOPUP => Message::Unhandled { const_name: "WM_INITMENUPOPUP" },
            WM_INPUT => Message::Unhandled { const_name: "WM_INPUT" },
            WM_INPUTLANGCHANGE => Message::Unhandled { const_name: "WM_INPUTLANGCHANGE" },
            WM_INPUTLANGCHANGEREQUEST => {
                Message::Unhandled { const_name: "WM_INPUTLANGCHANGEREQUEST" }
            }
            WM_INPUT_DEVICE_CHANGE => Message::Unhandled { const_name: "WM_INPUT_DEVICE_CHANGE" },
            WM_KEYDOWN => {
                Message::AppDep(MessageAppDep::KeyDown { key: VIRTUAL_KEY(wparam_loword) })
            }
            WM_KEYUP => Message::AppDep(MessageAppDep::KeyUp { key: VIRTUAL_KEY(wparam_loword) }),
            WM_KILLFOCUS => Message::Unhandled { const_name: "WM_KILLFOCUS" },
            WM_LBUTTONDBLCLK => Message::Unhandled { const_name: "WM_LBUTTONDBLCLK" },
            WM_LBUTTONDOWN => Message::AppDep(MessageAppDep::LButtonDown { pos: lparam.into() }),
            WM_LBUTTONUP => Message::AppDep(MessageAppDep::LButtonUp { pos: lparam.into() }),
            WM_MBUTTONDBLCLK => Message::Unhandled { const_name: "WM_MBUTTONDBLCLK" },
            WM_MBUTTONDOWN => Message::Unhandled { const_name: "WM_MBUTTONDOWN" },
            WM_MBUTTONUP => Message::Unhandled { const_name: "WM_MBUTTONUP" },
            WM_MDIACTIVATE => Message::Unhandled { const_name: "WM_MDIACTIVATE" },
            WM_MDICASCADE => Message::Unhandled { const_name: "WM_MDICASCADE" },
            WM_MDICREATE => Message::Unhandled { const_name: "WM_MDICREATE" },
            WM_MDIDESTROY => Message::Unhandled { const_name: "WM_MDIDESTROY" },
            WM_MDIGETACTIVE => Message::Unhandled { const_name: "WM_MDIGETACTIVE" },
            WM_MDIICONARRANGE => Message::Unhandled { const_name: "WM_MDIICONARRANGE" },
            WM_MDIMAXIMIZE => Message::Unhandled { const_name: "WM_MDIMAXIMIZE" },
            WM_MDINEXT => Message::Unhandled { const_name: "WM_MDINEXT" },
            WM_MDIREFRESHMENU => Message::Unhandled { const_name: "WM_MDIREFRESHMENU" },
            WM_MDIRESTORE => Message::Unhandled { const_name: "WM_MDIRESTORE" },
            WM_MDISETMENU => Message::Unhandled { const_name: "WM_MDISETMENU" },
            WM_MDITILE => Message::Unhandled { const_name: "WM_MDITILE" },
            WM_MEASUREITEM => Message::Unhandled { const_name: "WM_MEASUREITEM" },
            WM_MENUCHAR => Message::Unhandled { const_name: "WM_MENUCHAR" },
            WM_MENUCOMMAND => Message::Unhandled { const_name: "WM_MENUCOMMAND" },
            WM_MENUDRAG => Message::Unhandled { const_name: "WM_MENUDRAG" },
            WM_MENUGETOBJECT => Message::Unhandled { const_name: "WM_MENUGETOBJECT" },
            WM_MENURBUTTONUP => Message::Unhandled { const_name: "WM_MENURBUTTONUP" },
            WM_MENUSELECT => Message::Unhandled { const_name: "WM_MENUSELECT" },
            WM_MOUSEACTIVATE => Message::Unhandled { const_name: "WM_MOUSEACTIVATE" },
            WM_MOUSEHWHEEL => Message::AppDep(MessageAppDep::MouseHWheel { delta: wparam_hiword }),
            WM_MOUSEMOVE => Message::AppDep(MessageAppDep::MouseMove { pos: lparam.into() }),
            WM_MOUSEWHEEL => Message::AppDep(MessageAppDep::MouseWheel { delta: wparam_hiword }),
            WM_MOVE => Message::Unhandled { const_name: "WM_MOVE" },
            WM_MOVING => Message::Unhandled { const_name: "WM_MOVING" },
            WM_NCACTIVATE => Message::Unhandled { const_name: "WM_NCACTIVATE" },
            WM_NCCALCSIZE => Message::Unhandled { const_name: "WM_NCCALCSIZE" },
            WM_NCCREATE => Message::Unhandled { const_name: "WM_NCCREATE" },
            WM_NCDESTROY => Message::Unhandled { const_name: "WM_NCDESTROY" },
            WM_NCHITTEST => Message::Unhandled { const_name: "WM_NCHITTEST" },
            WM_NCLBUTTONDBLCLK => Message::Unhandled { const_name: "WM_NCLBUTTONDBLCLK" },
            WM_NCLBUTTONDOWN => Message::Unhandled { const_name: "WM_NCLBUTTONDOWN" },
            WM_NCLBUTTONUP => Message::Unhandled { const_name: "WM_NCLBUTTONUP" },
            WM_NCMBUTTONDBLCLK => Message::Unhandled { const_name: "WM_NCMBUTTONDBLCLK" },
            WM_NCMBUTTONDOWN => Message::Unhandled { const_name: "WM_NCMBUTTONDOWN" },
            WM_NCMBUTTONUP => Message::Unhandled { const_name: "WM_NCMBUTTONUP" },
            WM_NCMOUSEHOVER => Message::Unhandled { const_name: "WM_NCMOUSEHOVER" },
            WM_NCMOUSELEAVE => Message::Unhandled { const_name: "WM_NCMOUSELEAVE" },
            WM_NCMOUSEMOVE => Message::Unhandled { const_name: "WM_NCMOUSEMOVE" },
            WM_NCPAINT => Message::Unhandled { const_name: "WM_NCPAINT" },
            WM_NCPOINTERDOWN => Message::Unhandled { const_name: "WM_NCPOINTERDOWN" },
            WM_NCPOINTERUP => Message::Unhandled { const_name: "WM_NCPOINTERUP" },
            WM_NCPOINTERUPDATE => Message::Unhandled { const_name: "WM_NCPOINTERUPDATE" },
            WM_NCRBUTTONDBLCLK => Message::Unhandled { const_name: "WM_NCRBUTTONDBLCLK" },
            WM_NCRBUTTONDOWN => Message::Unhandled { const_name: "WM_NCRBUTTONDOWN" },
            WM_NCRBUTTONUP => Message::Unhandled { const_name: "WM_NCRBUTTONUP" },
            WM_NCXBUTTONDBLCLK => Message::Unhandled { const_name: "WM_NCXBUTTONDBLCLK" },
            WM_NCXBUTTONDOWN => Message::Unhandled { const_name: "WM_NCXBUTTONDOWN" },
            WM_NCXBUTTONUP => Message::Unhandled { const_name: "WM_NCXBUTTONUP" },
            WM_NEXTDLGCTL => Message::Unhandled { const_name: "WM_NEXTDLGCTL" },
            WM_NEXTMENU => Message::Unhandled { const_name: "WM_NEXTMENU" },
            WM_NOTIFY => Message::Unhandled { const_name: "WM_NOTIFY" },
            WM_NOTIFYFORMAT => Message::Unhandled { const_name: "WM_NOTIFYFORMAT" },
            WM_NULL => Message::Unhandled { const_name: "WM_NULL" },
            WM_PAINT => Message::AppDep(MessageAppDep::Paint),
            WM_PAINTCLIPBOARD => Message::Unhandled { const_name: "WM_PAINTCLIPBOARD" },
            WM_PAINTICON => Message::Unhandled { const_name: "WM_PAINTICON" },
            WM_PALETTECHANGED => Message::Unhandled { const_name: "WM_PALETTECHANGED" },
            WM_PALETTEISCHANGING => Message::Unhandled { const_name: "WM_PALETTEISCHANGING" },
            WM_PARENTNOTIFY => Message::Unhandled { const_name: "WM_PARENTNOTIFY" },
            WM_PASTE => Message::Unhandled { const_name: "WM_PASTE" },
            WM_PENWINFIRST => Message::Unhandled { const_name: "WM_PENWINFIRST" },
            WM_PENWINLAST => Message::Unhandled { const_name: "WM_PENWINLAST" },
            WM_POINTERACTIVATE => Message::Unhandled { const_name: "WM_POINTERACTIVATE" },
            WM_POINTERCAPTURECHANGED => {
                Message::Unhandled { const_name: "WM_POINTERCAPTURECHANGED" }
            }
            WM_POINTERDEVICECHANGE => Message::Unhandled { const_name: "WM_POINTERDEVICECHANGE" },
            WM_POINTERDEVICEINRANGE => Message::Unhandled { const_name: "WM_POINTERDEVICEINRANGE" },
            WM_POINTERDEVICEOUTOFRANGE => {
                Message::Unhandled { const_name: "WM_POINTERDEVICEOUTOFRANGE" }
            }
            WM_POINTERDOWN => {
                Message::AppDep(MessageAppDep::PointerDown { pointer_id: wparam_loword })
            }
            WM_POINTERENTER => Message::Unhandled { const_name: "WM_POINTERENTER" },
            WM_POINTERHWHEEL => Message::Unhandled { const_name: "WM_POINTERHWHEEL" },
            WM_POINTERLEAVE => Message::Unhandled { const_name: "WM_POINTERLEAVE" },
            WM_POINTERROUTEDAWAY => Message::Unhandled { const_name: "WM_POINTERROUTEDAWAY" },
            WM_POINTERROUTEDRELEASED => {
                Message::Unhandled { const_name: "WM_POINTERROUTEDRELEASED" }
            }
            WM_POINTERROUTEDTO => Message::Unhandled { const_name: "WM_POINTERROUTEDTO" },
            WM_POINTERUP => Message::AppDep(MessageAppDep::PointerUp { pointer_id: wparam_loword }),
            WM_POINTERUPDATE => {
                Message::AppDep(MessageAppDep::PointerUpdate { pointer_id: wparam_loword })
            }
            WM_POINTERWHEEL => Message::Unhandled { const_name: "WM_POINTERWHEEL" },
            WM_POWER => Message::Unhandled { const_name: "WM_POWER" },
            WM_POWERBROADCAST => Message::Unhandled { const_name: "WM_POWERBROADCAST" },
            WM_PRINT => Message::Unhandled { const_name: "WM_PRINT" },
            WM_PRINTCLIENT => Message::Unhandled { const_name: "WM_PRINTCLIENT" },
            WM_QUERYDRAGICON => Message::Unhandled { const_name: "WM_QUERYDRAGICON" },
            WM_QUERYENDSESSION => Message::Unhandled { const_name: "WM_QUERYENDSESSION" },
            WM_QUERYNEWPALETTE => Message::Unhandled { const_name: "WM_QUERYNEWPALETTE" },
            WM_QUERYOPEN => Message::Unhandled { const_name: "WM_QUERYOPEN" },
            WM_QUERYUISTATE => Message::Unhandled { const_name: "WM_QUERYUISTATE" },
            WM_QUEUESYNC => Message::Unhandled { const_name: "WM_QUEUESYNC" },
            WM_QUIT => Message::NoDeps(MessageNoDeps::Quit),
            WM_RBUTTONDBLCLK => Message::Unhandled { const_name: "WM_RBUTTONDBLCLK" },
            WM_RBUTTONDOWN => Message::AppDep(MessageAppDep::RButtonDown { pos: lparam.into() }),
            WM_RBUTTONUP => Message::AppDep(MessageAppDep::RButtonUp { pos: lparam.into() }),
            WM_RENDERALLFORMATS => Message::Unhandled { const_name: "WM_RENDERALLFORMATS" },
            WM_RENDERFORMAT => Message::Unhandled { const_name: "WM_RENDERFORMAT" },
            WM_SETCURSOR => Message::AppDep(MessageAppDep::SetCursor),
            WM_SETFOCUS => Message::Unhandled { const_name: "WM_SETFOCUS" },
            WM_SETFONT => Message::Unhandled { const_name: "WM_SETFONT" },
            WM_SETHOTKEY => Message::Unhandled { const_name: "WM_SETHOTKEY" },
            WM_SETICON => Message::Unhandled { const_name: "WM_SETICON" },
            WM_SETREDRAW => Message::Unhandled { const_name: "WM_SETREDRAW" },
            WM_SETTEXT => Message::Unhandled { const_name: "WM_SETTEXT" },
            WM_SETTINGCHANGE => Message::Unhandled { const_name: "WM_SETTINGCHANGE" },
            WM_SHOWWINDOW => Message::Unhandled { const_name: "WM_SHOWWINDOW" },
            WM_SIZE => Message::WindowDep(MessageWindowDep::Size {
                width: lparam_loword,
                height: lparam_hiword,
            }),
            WM_SIZECLIPBOARD => Message::Unhandled { const_name: "WM_SIZECLIPBOARD" },
            WM_SIZING => Message::Unhandled { const_name: "WM_SIZING" },
            WM_SPOOLERSTATUS => Message::Unhandled { const_name: "WM_SPOOLERSTATUS" },
            WM_STYLECHANGED => Message::Unhandled { const_name: "WM_STYLECHANGED" },
            WM_STYLECHANGING => Message::Unhandled { const_name: "WM_STYLECHANGING" },
            WM_SYNCPAINT => Message::Unhandled { const_name: "WM_SYNCPAINT" },
            WM_SYSCHAR => Message::Unhandled { const_name: "WM_SYSCHAR" },
            WM_SYSCOLORCHANGE => Message::Unhandled { const_name: "WM_SYSCOLORCHANGE" },
            WM_SYSCOMMAND => Message::Unhandled { const_name: "WM_SYSCOMMAND" },
            WM_SYSDEADCHAR => Message::Unhandled { const_name: "WM_SYSDEADCHAR" },
            WM_SYSKEYDOWN => Message::Unhandled { const_name: "WM_SYSKEYDOWN" },
            WM_SYSKEYUP => Message::Unhandled { const_name: "WM_SYSKEYUP" },
            WM_TABLET_FIRST => Message::Unhandled { const_name: "WM_TABLET_FIRST" },
            WM_TABLET_LAST => Message::Unhandled { const_name: "WM_TABLET_LAST" },
            WM_TCARD => Message::Unhandled { const_name: "WM_TCARD" },
            WM_THEMECHANGED => Message::Unhandled { const_name: "WM_THEMECHANGED" },
            WM_TIMECHANGE => Message::Unhandled { const_name: "WM_TIMECHANGE" },
            WM_TIMER => Message::Unhandled { const_name: "WM_TIMER" },
            WM_TOOLTIPDISMISS => Message::Unhandled { const_name: "WM_TOOLTIPDISMISS" },
            WM_TOUCH => Message::Unhandled { const_name: "WM_TOUCH" },
            WM_TOUCHHITTESTING => Message::Unhandled { const_name: "WM_TOUCHHITTESTING" },
            WM_UNDO => Message::Unhandled { const_name: "WM_UNDO" },
            WM_UNICHAR => Message::Unhandled { const_name: "WM_UNICHAR" },
            WM_UNINITMENUPOPUP => Message::Unhandled { const_name: "WM_UNINITMENUPOPUP" },
            WM_UPDATEUISTATE => Message::Unhandled { const_name: "WM_UPDATEUISTATE" },
            WM_USER => Message::Unhandled { const_name: "WM_USER" },
            WM_USERCHANGED => Message::Unhandled { const_name: "WM_USERCHANGED" },
            WM_VKEYTOITEM => Message::Unhandled { const_name: "WM_VKEYTOITEM" },
            WM_VSCROLL => Message::Unhandled { const_name: "WM_VSCROLL" },
            WM_VSCROLLCLIPBOARD => Message::Unhandled { const_name: "WM_VSCROLLCLIPBOARD" },
            WM_WINDOWPOSCHANGED => Message::Unhandled { const_name: "WM_WINDOWPOSCHANGED" },
            WM_WINDOWPOSCHANGING => Message::Unhandled { const_name: "WM_WINDOWPOSCHANGING" },
            WM_WTSSESSION_CHANGE => Message::Unhandled { const_name: "WM_WTSSESSION_CHANGE" },
            WM_XBUTTONDBLCLK => Message::Unhandled { const_name: "WM_XBUTTONDBLCLK" },
            WM_XBUTTONDOWN => Message::Unhandled { const_name: "WM_XBUTTONDOWN" },
            WM_XBUTTONUP => Message::Unhandled { const_name: "WM_XBUTTONUP" },
            _ => Message::Unknown { msg },
        }
    }
}

// https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/mod.rs#L144C1-L152C2
#[inline(always)]
const fn loword_l(lparam: LPARAM) -> u16 {
    (lparam.0 & 0xFFFF) as _
}

#[inline(always)]
const fn hiword_l(lparam: LPARAM) -> u16 {
    ((lparam.0 >> 16) & 0xFFFF) as _
}

#[inline(always)]
const fn loword_w(wparam: WPARAM) -> u16 {
    (wparam.0 & 0xFFFF) as _
}

#[inline(always)]
const fn hiword_w(wparam: WPARAM) -> i16 {
    ((wparam.0 >> 16) & 0xFFFF) as _
}
